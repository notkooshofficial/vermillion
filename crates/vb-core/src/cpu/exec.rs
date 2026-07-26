use crate::bus::Bus;
use crate::cpu::decode::{DecodeError, Instruction, Op, decode, instruction_width};
use crate::cpu::state::*;

pub const EXC_ADDRESS_TRAP: u16 = 0xFFC0;
pub const EXC_ILLEGAL_OPCODE: u16 = 0xFF90;
pub const EXC_ZERO_DIVISION: u16 = 0xFF80;
pub const EXC_TRAP_BASE: u16 = 0xFFA0;

pub const HANDLER_DUPLEXED: u32 = 0xFFFF_FFD0;
pub const HANDLER_ADDRESS_TRAP: u32 = 0xFFFF_FFC0;
pub const HANDLER_ILLEGAL_OPCODE: u32 = 0xFFFF_FF90;
pub const HANDLER_ZERO_DIVISION: u32 = 0xFFFF_FF80;
pub const HANDLER_TRAP_LOW: u32 = 0xFFFF_FFA0;
pub const HANDLER_TRAP_HIGH: u32 = 0xFFFF_FFB0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Halted,
    Fatal { code: u16, pc: u32 },
    Unimplemented { op: Op, pc: u32 },
}

// an exception is not a stop, the cpu keeps running at the handler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Executed(Instruction),
    Exception { code: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    pub code: u16,
    pub handler: u32,
    pub restore_pc: u32,
    pub level: Option<u32>,
}

fn sign_extend_byte(value: u8) -> u32 {
    (u32::from(value) ^ 0x80).wrapping_sub(0x80)
}

fn sign_extend_halfword(value: u16) -> u32 {
    (u32::from(value) ^ 0x8000).wrapping_sub(0x8000)
}

fn sign_extend_17(value: u32) -> u32 {
    ((value & 0x1_FFFF) ^ 0x1_0000).wrapping_sub(0x1_0000)
}

fn field_index(imm: i32) -> u8 {
    u8::try_from(imm & 0x1F).unwrap_or(0)
}

fn low_word(value: u64) -> u32 {
    u32::try_from(value & 0xFFFF_FFFF).unwrap_or(0)
}

fn high_word(value: u64) -> u32 {
    u32::try_from(value >> 32).unwrap_or(0)
}

impl Cpu {
    pub fn step(&mut self, bus: &mut Bus) -> Result<StepOutcome, Stop> {
        if self.halted {
            return Err(Stop::Halted);
        }

        if self.flag(PSW_AE) && self.pc == self.adtre {
            self.raise(
                bus,
                Exception {
                    code: EXC_ADDRESS_TRAP,
                    handler: HANDLER_ADDRESS_TRAP,
                    restore_pc: self.pc,
                    level: None,
                },
            )?;
            return Ok(StepOutcome::Exception {
                code: EXC_ADDRESS_TRAP,
            });
        }

        let pc = self.pc;
        let word0 = bus.read_u16(pc);
        let width = instruction_width(word0);
        let word1 = if width == 4 {
            bus.read_u16(pc.wrapping_add(2))
        } else {
            0
        };

        let instruction = match decode(word0, word1) {
            Ok(instruction) => instruction,
            Err(DecodeError::IllegalOpcode(_) | DecodeError::IllegalSubOpcode { .. }) => {
                self.raise(
                    bus,
                    Exception {
                        code: EXC_ILLEGAL_OPCODE,
                        handler: HANDLER_ILLEGAL_OPCODE,
                        restore_pc: pc,
                        level: None,
                    },
                )?;
                return Ok(StepOutcome::Exception {
                    code: EXC_ILLEGAL_OPCODE,
                });
            }
            Err(DecodeError::Truncated { .. }) => unreachable_truncation(),
        };

        let next_pc = pc.wrapping_add(width);
        self.pc = next_pc;
        self.execute(bus, instruction, pc, next_pc)?;
        Ok(StepOutcome::Executed(instruction))
    }

    pub fn raise(&mut self, bus: &mut Bus, exception: Exception) -> Result<(), Stop> {
        if self.flag(PSW_NP) {
            bus.write_u32(0x0000_0000, 0xFFFF_0000 | u32::from(exception.code));
            bus.write_u32(0x0000_0004, self.psw);
            bus.write_u32(0x0000_0008, self.pc);
            self.halted = true;
            return Err(Stop::Fatal {
                code: exception.code,
                pc: exception.restore_pc,
            });
        }

        if self.flag(PSW_EP) {
            self.ecr = (self.ecr & 0x0000_FFFF) | (u32::from(exception.code) << 16);
            self.fepsw = self.psw;
            self.fepc = exception.restore_pc & !1;
            self.set_flag(PSW_NP, true);
            self.pc = HANDLER_DUPLEXED;
        } else {
            self.ecr = (self.ecr & 0xFFFF_0000) | u32::from(exception.code);
            self.eipsw = self.psw;
            self.eipc = exception.restore_pc & !1;
            self.set_flag(PSW_EP, true);
            self.pc = exception.handler;
        }

        if let Some(level) = exception.level {
            self.set_interrupt_level(level.saturating_add(1));
            self.halted = false;
        }

        self.set_flag(PSW_ID, true);
        self.set_flag(PSW_AE, false);
        Ok(())
    }

    fn set_zs(&mut self, result: u32) {
        self.set_flag(PSW_Z, result == 0);
        self.set_flag(PSW_S, result & 0x8000_0000 != 0);
    }

    fn add_flags(&mut self, left: u32, right: u32) -> u32 {
        let (result, carry) = left.overflowing_add(right);
        let overflow = (left ^ result) & (right ^ result) & 0x8000_0000 != 0;
        self.set_zs(result);
        self.set_flag(PSW_OV, overflow);
        self.set_flag(PSW_CY, carry);
        result
    }

    fn sub_flags(&mut self, left: u32, right: u32) -> u32 {
        let (result, borrow) = left.overflowing_sub(right);
        let overflow = (left ^ right) & (left ^ result) & 0x8000_0000 != 0;
        self.set_zs(result);
        self.set_flag(PSW_OV, overflow);
        self.set_flag(PSW_CY, borrow);
        result
    }

    fn shift_left(&mut self, value: u32, amount: u32) -> u32 {
        let amount = amount & 0x1F;
        let (result, carry) = if amount == 0 {
            (value, false)
        } else {
            (value << amount, (value >> (32 - amount)) & 1 != 0)
        };
        self.finish_shift(result, carry);
        result
    }

    fn shift_right(&mut self, value: u32, amount: u32) -> u32 {
        let amount = amount & 0x1F;
        let (result, carry) = if amount == 0 {
            (value, false)
        } else {
            (value >> amount, (value >> (amount - 1)) & 1 != 0)
        };
        self.finish_shift(result, carry);
        result
    }

    fn shift_arithmetic_right(&mut self, value: u32, amount: u32) -> u32 {
        let amount = amount & 0x1F;
        let (result, carry) = if amount == 0 {
            (value, false)
        } else {
            let signed = i32::from_ne_bytes(value.to_ne_bytes()) >> amount;
            (
                u32::from_ne_bytes(signed.to_ne_bytes()),
                (value >> (amount - 1)) & 1 != 0,
            )
        };
        self.finish_shift(result, carry);
        result
    }

    fn finish_shift(&mut self, result: u32, carry: bool) {
        self.set_zs(result);
        self.set_flag(PSW_OV, false);
        self.set_flag(PSW_CY, carry);
    }

    fn set_logic_flags(&mut self, result: u32) {
        self.set_zs(result);
        self.set_flag(PSW_OV, false);
    }

    fn condition_holds(&self, instruction: Instruction) -> bool {
        instruction.cond.is_some_and(|cond| {
            cond.evaluate(
                self.flag(PSW_Z),
                self.flag(PSW_S),
                self.flag(PSW_OV),
                self.flag(PSW_CY),
            )
        })
    }

    fn effective_address(&self, instruction: Instruction) -> u32 {
        self.reg(instruction.reg1)
            .wrapping_add_signed(instruction.imm)
    }

    fn load_cycles(&mut self) -> u64 {
        let cycles = if self.prev_was_load { 4 } else { 5 };
        self.prev_was_load = true;
        self.consecutive_stores = 0;
        cycles
    }

    fn store_cycles(&mut self) -> u64 {
        let cycles = if self.consecutive_stores < 2 { 1 } else { 4 };
        self.consecutive_stores = self.consecutive_stores.saturating_add(1);
        self.prev_was_load = false;
        cycles
    }

    fn plain_cycles(&mut self, cycles: u64) -> u64 {
        self.prev_was_load = false;
        self.consecutive_stores = 0;
        cycles
    }

    #[allow(clippy::too_many_lines)]
    fn execute(
        &mut self,
        bus: &mut Bus,
        instruction: Instruction,
        pc: u32,
        next_pc: u32,
    ) -> Result<(), Stop> {
        let reg1 = self.reg(instruction.reg1);
        let reg2 = self.reg(instruction.reg2);
        let imm = instruction.imm;

        let cycles = match instruction.op {
            Op::Mov => {
                self.set_reg(instruction.reg2, reg1);
                self.plain_cycles(1)
            }
            Op::MovImm => {
                self.set_reg(instruction.reg2, imm.cast_unsigned());
                self.plain_cycles(1)
            }
            Op::Movea => {
                self.set_reg(instruction.reg2, reg1.wrapping_add_signed(imm));
                self.plain_cycles(1)
            }
            Op::Movhi => {
                self.set_reg(
                    instruction.reg2,
                    reg1.wrapping_add(imm.cast_unsigned() << 16),
                );
                self.plain_cycles(1)
            }

            Op::Add => {
                let result = self.add_flags(reg2, reg1);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::AddImm => {
                let result = self.add_flags(reg2, imm.cast_unsigned());
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Addi => {
                let result = self.add_flags(reg1, imm.cast_unsigned());
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Sub => {
                let result = self.sub_flags(reg2, reg1);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Cmp => {
                self.sub_flags(reg2, reg1);
                self.plain_cycles(1)
            }
            Op::CmpImm => {
                self.sub_flags(reg2, imm.cast_unsigned());
                self.plain_cycles(1)
            }

            Op::Mul => {
                let left = i64::from(reg2.cast_signed());
                let right = i64::from(reg1.cast_signed());
                let product = left.wrapping_mul(right);
                let low = low_word(product.cast_unsigned());
                let high = high_word(product.cast_unsigned());
                self.set_zs(low);
                self.set_flag(PSW_OV, product != i64::from(low.cast_signed()));
                // r30 first, then reg2, the order shows when reg2 is r30
                self.set_reg(30, high);
                self.set_reg(instruction.reg2, low);
                self.plain_cycles(13)
            }
            Op::Mulu => {
                let product = u64::from(reg2).wrapping_mul(u64::from(reg1));
                let low = low_word(product);
                let high = high_word(product);
                self.set_zs(low);
                self.set_flag(PSW_OV, product != u64::from(low));
                self.set_reg(30, high);
                self.set_reg(instruction.reg2, low);
                self.plain_cycles(13)
            }
            Op::Div => {
                let left = reg2.cast_signed();
                let right = reg1.cast_signed();
                if right == 0 {
                    return self.raise_zero_division(bus, pc);
                }
                let (quotient, remainder, overflow) = if left == i32::MIN && right == -1 {
                    (i32::MIN, 0, true)
                } else {
                    (left.wrapping_div(right), left.wrapping_rem(right), false)
                };
                self.set_zs(quotient.cast_unsigned());
                self.set_flag(PSW_OV, overflow);
                self.set_reg(30, remainder.cast_unsigned());
                self.set_reg(instruction.reg2, quotient.cast_unsigned());
                self.plain_cycles(38)
            }
            Op::Divu => {
                if reg1 == 0 {
                    return self.raise_zero_division(bus, pc);
                }
                let quotient = reg2 / reg1;
                let remainder = reg2 % reg1;
                self.set_zs(quotient);
                self.set_flag(PSW_OV, false);
                self.set_reg(30, remainder);
                self.set_reg(instruction.reg2, quotient);
                self.plain_cycles(36)
            }

            Op::And => {
                let result = reg2 & reg1;
                self.set_logic_flags(result);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Andi => {
                let result = reg1 & imm.cast_unsigned();
                // andi clears s, unlike and, the immediate is zero-extended
                self.set_flag(PSW_Z, result == 0);
                self.set_flag(PSW_S, false);
                self.set_flag(PSW_OV, false);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Or => {
                let result = reg2 | reg1;
                self.set_logic_flags(result);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Ori => {
                let result = reg1 | imm.cast_unsigned();
                self.set_logic_flags(result);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Xor => {
                let result = reg2 ^ reg1;
                self.set_logic_flags(result);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Xori => {
                let result = reg1 ^ imm.cast_unsigned();
                self.set_logic_flags(result);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Not => {
                let result = !reg1;
                self.set_logic_flags(result);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }

            Op::Shl => {
                let result = self.shift_left(reg2, reg1);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::ShlImm => {
                let result = self.shift_left(reg2, imm.cast_unsigned());
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Shr => {
                let result = self.shift_right(reg2, reg1);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::ShrImm => {
                let result = self.shift_right(reg2, imm.cast_unsigned());
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::Sar => {
                let result = self.shift_arithmetic_right(reg2, reg1);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }
            Op::SarImm => {
                let result = self.shift_arithmetic_right(reg2, imm.cast_unsigned());
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(1)
            }

            Op::Jmp => {
                self.pc = reg1 & !1;
                self.plain_cycles(3)
            }
            Op::Jr => {
                self.pc = pc.wrapping_add_signed(imm) & !1;
                self.plain_cycles(3)
            }
            Op::Jal => {
                self.set_reg(31, pc.wrapping_add(4));
                self.pc = pc.wrapping_add_signed(imm) & !1;
                self.plain_cycles(3)
            }
            Op::Bcond => {
                if self.condition_holds(instruction) {
                    self.pc = pc.wrapping_add_signed(imm) & !1;
                    self.plain_cycles(3)
                } else {
                    self.plain_cycles(1)
                }
            }
            Op::Setf => {
                let value = u32::from(self.condition_holds(instruction));
                self.set_reg(instruction.reg2, value);
                self.plain_cycles(1)
            }

            Op::LdB => {
                let value = sign_extend_byte(bus.read_u8(self.effective_address(instruction)));
                self.set_reg(instruction.reg2, value);
                self.load_cycles()
            }
            Op::InB => {
                let value = u32::from(bus.read_u8(self.effective_address(instruction)));
                self.set_reg(instruction.reg2, value);
                self.load_cycles()
            }
            Op::LdH => {
                let value = sign_extend_halfword(bus.read_u16(self.effective_address(instruction)));
                self.set_reg(instruction.reg2, value);
                self.load_cycles()
            }
            Op::InH => {
                let value = u32::from(bus.read_u16(self.effective_address(instruction)));
                self.set_reg(instruction.reg2, value);
                self.load_cycles()
            }
            Op::LdW | Op::InW => {
                let value = bus.read_u32(self.effective_address(instruction));
                self.set_reg(instruction.reg2, value);
                self.load_cycles()
            }

            Op::StB | Op::OutB => {
                let address = self.effective_address(instruction);
                bus.write_u8(address, (reg2 & 0xFF) as u8);
                self.store_cycles()
            }
            Op::StH | Op::OutH => {
                let address = self.effective_address(instruction);
                bus.write_u16(address, (reg2 & 0xFFFF) as u16);
                self.store_cycles()
            }
            Op::StW | Op::OutW => {
                let address = self.effective_address(instruction);
                bus.write_u32(address, reg2);
                self.store_cycles()
            }

            Op::Caxi => {
                let address = self.effective_address(instruction);
                let value = bus.read_u32(address);
                self.sub_flags(reg2, value);
                if self.flag(PSW_Z) {
                    bus.write_u32(address, self.reg(30));
                } else {
                    bus.write_u32(address, value);
                }
                self.set_reg(instruction.reg2, value);
                self.plain_cycles(26)
            }

            Op::Ldsr => {
                self.write_system_register(field_index(imm), reg2);
                self.plain_cycles(8)
            }
            Op::Stsr => {
                let value = self.read_system_register(field_index(imm));
                self.set_reg(instruction.reg2, value);
                self.plain_cycles(8)
            }
            Op::Reti => {
                if self.flag(PSW_NP) {
                    self.pc = self.fepc;
                    let psw = self.fepsw;
                    self.set_psw(psw);
                } else {
                    self.pc = self.eipc;
                    let psw = self.eipsw;
                    self.set_psw(psw);
                }
                self.plain_cycles(10)
            }
            Op::Trap => {
                let vector = u16::from(field_index(imm));
                let handler = if vector < 16 {
                    HANDLER_TRAP_LOW
                } else {
                    HANDLER_TRAP_HIGH
                };
                let spent = self.plain_cycles(15);
                self.cycles += spent;
                return self.raise(
                    bus,
                    Exception {
                        code: EXC_TRAP_BASE + vector,
                        handler,
                        restore_pc: next_pc,
                        level: None,
                    },
                );
            }
            Op::Halt => {
                self.halted = true;
                self.plain_cycles(1)
            }
            Op::Cli => {
                self.set_flag(PSW_ID, false);
                self.plain_cycles(12)
            }
            Op::Sei => {
                self.set_flag(PSW_ID, true);
                self.plain_cycles(12)
            }

            Op::Xb => {
                let result = (reg2 & 0xFFFF_0000) | ((reg2 << 8) & 0xFF00) | ((reg2 >> 8) & 0x00FF);
                self.set_reg(instruction.reg2, result);
                self.plain_cycles(6)
            }
            Op::Xh => {
                self.set_reg(instruction.reg2, reg2.rotate_right(16));
                self.plain_cycles(1)
            }
            Op::Rev => {
                self.set_reg(instruction.reg2, reg1.reverse_bits());
                self.plain_cycles(22)
            }
            Op::Mpyhw => {
                let right = sign_extend_17(reg1);
                let result = reg2.cast_signed().wrapping_mul(right.cast_signed());
                self.set_reg(instruction.reg2, result.cast_unsigned());
                self.plain_cycles(9)
            }

            other => return Err(Stop::Unimplemented { op: other, pc }),
        };

        self.cycles += cycles;
        Ok(())
    }

    fn raise_zero_division(&mut self, bus: &mut Bus, pc: u32) -> Result<(), Stop> {
        self.cycles += self.plain_cycles(38);
        self.raise(
            bus,
            Exception {
                code: EXC_ZERO_DIVISION,
                handler: HANDLER_ZERO_DIVISION,
                restore_pc: pc,
                level: None,
            },
        )
    }
}

fn unreachable_truncation() -> ! {
    panic!("decode() never reports truncation; only decode_slice() can")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::{Cart, tests::test_rom};

    const ROM_BASE: u32 = 0x0700_0000;

    fn short(opcode: u8, reg2: u8, reg1: u8) -> u16 {
        (u16::from(opcode) << 10) | (u16::from(reg2) << 5) | u16::from(reg1)
    }

    fn branch(cond: u8, disp: i16) -> u16 {
        let raw = u16::from_le_bytes(disp.to_le_bytes());
        0b100 << 13 | (u16::from(cond) << 9) | (raw & 0x01FF)
    }

    fn machine(program: &[u16]) -> (Cpu, Bus) {
        let mut rom = test_rom(0x1000);
        for (index, word) in program.iter().enumerate() {
            let offset = index * 2;
            rom[offset..offset + 2].copy_from_slice(&word.to_le_bytes());
        }
        let mut cpu = Cpu::new();
        cpu.pc = ROM_BASE;
        cpu.set_flag(PSW_NP, false);
        (cpu, Bus::new(Cart::new(rom).unwrap()))
    }

    #[test]
    fn executes_a_register_add_and_advances_pc() {
        let (mut cpu, mut bus) = machine(&[short(0b000001, 1, 2)]);
        cpu.regs[1] = 5;
        cpu.regs[2] = 7;

        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.regs[1], 12);
        assert_eq!(cpu.pc, ROM_BASE + 2);
        assert_eq!(cpu.cycles, 1);
    }

    #[test]
    fn add_sets_carry_and_overflow_independently() {
        let (mut cpu, mut bus) = machine(&[short(0b000001, 1, 2)]);
        cpu.regs[1] = 0xFFFF_FFFF;
        cpu.regs[2] = 1;
        cpu.step(&mut bus).unwrap();
        assert!(cpu.flag(PSW_CY));
        assert!(!cpu.flag(PSW_OV));
        assert!(cpu.flag(PSW_Z));

        let (mut cpu, mut bus) = machine(&[short(0b000001, 1, 2)]);
        cpu.regs[1] = 0x7FFF_FFFF;
        cpu.regs[2] = 1;
        cpu.step(&mut bus).unwrap();
        assert!(cpu.flag(PSW_OV));
        assert!(!cpu.flag(PSW_CY));
        assert!(cpu.flag(PSW_S));
    }

    #[test]
    fn sub_sets_carry_on_unsigned_borrow() {
        let (mut cpu, mut bus) = machine(&[short(0b000010, 1, 2)]);
        cpu.regs[1] = 1;
        cpu.regs[2] = 2;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0xFFFF_FFFF);
        assert!(cpu.flag(PSW_CY));
        assert!(cpu.flag(PSW_S));
        assert!(!cpu.flag(PSW_OV));
    }

    #[test]
    fn cmp_discards_the_result_but_keeps_flags() {
        let (mut cpu, mut bus) = machine(&[short(0b000011, 1, 2)]);
        cpu.regs[1] = 4;
        cpu.regs[2] = 4;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 4);
        assert!(cpu.flag(PSW_Z));
    }

    #[test]
    fn andi_always_clears_the_sign_flag() {
        let (mut cpu, mut bus) = machine(&[short(0b101101, 1, 2), 0xFFFF]);
        cpu.regs[2] = 0xFFFF_FFFF;
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.regs[1], 0x0000_FFFF);
        assert!(!cpu.flag(PSW_S));
        assert!(!cpu.flag(PSW_Z));
        assert!(!cpu.flag(PSW_OV));
    }

    #[test]
    fn and_keeps_the_sign_flag_unlike_andi() {
        let (mut cpu, mut bus) = machine(&[short(0b001101, 1, 2)]);
        cpu.regs[1] = 0xFFFF_FFFF;
        cpu.regs[2] = 0x8000_0000;
        cpu.step(&mut bus).unwrap();
        assert!(cpu.flag(PSW_S));
    }

    #[test]
    fn shifts_report_the_last_bit_shifted_out() {
        let (mut cpu, mut bus) = machine(&[short(0b010101, 1, 1)]);
        cpu.regs[1] = 0b11;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0b1);
        assert!(cpu.flag(PSW_CY));

        let (mut cpu, mut bus) = machine(&[short(0b010100, 1, 0)]);
        cpu.regs[1] = 0xFFFF_FFFF;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0xFFFF_FFFF);
        assert!(!cpu.flag(PSW_CY));
    }

    #[test]
    fn arithmetic_shift_propagates_the_sign() {
        let (mut cpu, mut bus) = machine(&[short(0b010111, 1, 4)]);
        cpu.regs[1] = 0x8000_0000;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0xF800_0000);
    }

    #[test]
    fn multiply_writes_r30_before_the_destination() {
        let (mut cpu, mut bus) = machine(&[short(0b001000, 30, 2)]);
        cpu.regs[30] = 0x0001_0000;
        cpu.regs[2] = 0x0001_0000;
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.regs[30], 0);
        assert!(cpu.flag(PSW_OV));
    }

    #[test]
    fn divide_stores_remainder_then_quotient() {
        let (mut cpu, mut bus) = machine(&[short(0b001001, 1, 2)]);
        cpu.regs[1] = 17;
        cpu.regs[2] = 5;
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.regs[1], 3);
        assert_eq!(cpu.regs[30], 2);
        assert!(!cpu.flag(PSW_OV));
        assert_eq!(cpu.cycles, 38);
    }

    #[test]
    fn divide_remainder_takes_the_dividend_sign() {
        let (mut cpu, mut bus) = machine(&[short(0b001001, 1, 2)]);
        cpu.regs[1] = (-17i32).cast_unsigned();
        cpu.regs[2] = 5;
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.regs[1].cast_signed(), -3);
        assert_eq!(cpu.regs[30].cast_signed(), -2);
    }

    #[test]
    fn divide_overflow_case_is_special_cased() {
        let (mut cpu, mut bus) = machine(&[short(0b001001, 1, 2)]);
        cpu.regs[1] = 0x8000_0000;
        cpu.regs[2] = 0xFFFF_FFFF;
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.regs[1], 0x8000_0000);
        assert_eq!(cpu.regs[30], 0);
        assert!(cpu.flag(PSW_OV));
    }

    #[test]
    fn divide_by_zero_raises_an_exception() {
        let (mut cpu, mut bus) = machine(&[short(0b001001, 1, 2)]);
        cpu.regs[1] = 1;
        cpu.regs[2] = 0;
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, HANDLER_ZERO_DIVISION);
        assert_eq!(cpu.ecr & 0xFFFF, u32::from(EXC_ZERO_DIVISION));
        assert_eq!(cpu.eipc, ROM_BASE);
        assert!(cpu.flag(PSW_EP));
        assert!(cpu.flag(PSW_ID));
    }

    #[test]
    fn divu_never_sets_overflow() {
        let (mut cpu, mut bus) = machine(&[short(0b001011, 1, 2)]);
        cpu.regs[1] = 0x8000_0000;
        cpu.regs[2] = 0xFFFF_FFFF;
        cpu.step(&mut bus).unwrap();
        assert!(!cpu.flag(PSW_OV));
        assert_eq!(cpu.regs[1], 0);
        assert_eq!(cpu.regs[30], 0x8000_0000);
    }

    #[test]
    fn branch_displacement_is_relative_to_the_instruction() {
        let (mut cpu, mut bus) = machine(&[branch(5, 8)]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, ROM_BASE + 8);
        assert_eq!(cpu.cycles, 3);
    }

    #[test]
    fn untaken_branch_falls_through_cheaply() {
        let (mut cpu, mut bus) = machine(&[branch(13, 8)]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, ROM_BASE + 2);
        assert_eq!(cpu.cycles, 1);
    }

    #[test]
    fn jal_links_to_the_instruction_after_itself() {
        let (mut cpu, mut bus) = machine(&[short(0b101011, 0, 0), 0x0010]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[31], ROM_BASE + 4);
        assert_eq!(cpu.pc, ROM_BASE + 0x10);
    }

    #[test]
    fn jump_targets_are_forced_even() {
        let (mut cpu, mut bus) = machine(&[short(0b000110, 0, 5)]);
        cpu.regs[5] = ROM_BASE + 0x21;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, ROM_BASE + 0x20);
    }

    #[test]
    fn load_sign_extends_where_input_zero_extends() {
        let (mut cpu, mut bus) = machine(&[short(0b110000, 1, 2), 0x0000]);
        cpu.regs[2] = 0x0500_0000;
        bus.write_u8(0x0500_0000, 0x80);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0xFFFF_FF80);

        let (mut cpu, mut bus) = machine(&[short(0b111000, 1, 2), 0x0000]);
        cpu.regs[2] = 0x0500_0000;
        bus.write_u8(0x0500_0000, 0x80);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0x0000_0080);
    }

    #[test]
    fn store_masks_to_the_access_width() {
        let (mut cpu, mut bus) = machine(&[short(0b110100, 1, 2), 0x0000]);
        cpu.regs[1] = 0xDEAD_BEEF;
        cpu.regs[2] = 0x0500_0000;
        cpu.step(&mut bus).unwrap();
        assert_eq!(bus.read_u8(0x0500_0000), 0xEF);
        assert_eq!(bus.read_u8(0x0500_0001), 0x00);
    }

    #[test]
    fn consecutive_loads_are_cheaper_than_isolated_ones() {
        let (mut cpu, mut bus) =
            machine(&[short(0b110011, 1, 2), 0x0000, short(0b110011, 3, 2), 0x0000]);
        cpu.regs[2] = 0x0500_0000;

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.cycles, 5);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.cycles, 9);
    }

    #[test]
    fn ldsr_to_psw_changes_flags_but_other_targets_do_not() {
        let (mut cpu, mut bus) = machine(&[short(0b011100, 1, SR_PSW)]);
        cpu.regs[1] = PSW_Z | PSW_CY;
        cpu.step(&mut bus).unwrap();
        assert!(cpu.flag(PSW_Z));
        assert!(cpu.flag(PSW_CY));

        let (mut cpu, mut bus) = machine(&[short(0b011100, 1, SR_ECR)]);
        let before = cpu.psw;
        cpu.regs[1] = 0xFFFF_FFFF;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.psw, before);
        assert_eq!(cpu.ecr, crate::RESET_ECR);
    }

    #[test]
    fn stsr_reads_the_fixed_processor_id() {
        let (mut cpu, mut bus) = machine(&[short(0b011101, 1, SR_PIR)]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], PIR_VALUE);
    }

    #[test]
    fn trap_dispatches_by_vector_and_saves_the_next_pc() {
        let (mut cpu, mut bus) = machine(&[short(0b011000, 0, 3)]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, HANDLER_TRAP_LOW);
        assert_eq!(cpu.ecr & 0xFFFF, u32::from(EXC_TRAP_BASE + 3));
        assert_eq!(cpu.eipc, ROM_BASE + 2);

        let (mut cpu, mut bus) = machine(&[short(0b011000, 0, 20)]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.pc, HANDLER_TRAP_HIGH);
    }

    #[test]
    fn reti_restores_from_eipc_when_np_is_clear() {
        let (mut cpu, mut bus) = machine(&[short(0b011001, 0, 0)]);
        cpu.eipc = 0x0700_0100;
        cpu.eipsw = PSW_Z;
        cpu.set_flag(PSW_EP, true);
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, 0x0700_0100);
        assert!(cpu.flag(PSW_Z));
        assert!(!cpu.flag(PSW_EP));
    }

    #[test]
    fn reti_restores_from_fepc_when_np_is_set() {
        let (mut cpu, mut bus) = machine(&[short(0b011001, 0, 0)]);
        cpu.fepc = 0x0700_0200;
        cpu.fepsw = 0;
        cpu.set_flag(PSW_NP, true);
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, 0x0700_0200);
        assert!(!cpu.flag(PSW_NP));
    }

    #[test]
    fn illegal_opcode_raises_with_the_current_pc() {
        let (mut cpu, mut bus) = machine(&[short(0b011011, 0, 0)]);
        assert_eq!(
            cpu.step(&mut bus).unwrap(),
            StepOutcome::Exception {
                code: EXC_ILLEGAL_OPCODE
            }
        );

        assert_eq!(cpu.pc, HANDLER_ILLEGAL_OPCODE);
        assert_eq!(cpu.ecr & 0xFFFF, u32::from(EXC_ILLEGAL_OPCODE));
        assert_eq!(cpu.eipc, ROM_BASE);
    }

    #[test]
    fn exception_during_exception_becomes_duplexed() {
        let (mut cpu, mut bus) = machine(&[short(0b011000, 0, 1)]);
        cpu.set_flag(PSW_EP, true);
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.pc, HANDLER_DUPLEXED);
        assert_eq!(cpu.ecr >> 16, u32::from(EXC_TRAP_BASE + 1));
        assert!(cpu.flag(PSW_NP));
        assert_eq!(cpu.fepc, ROM_BASE + 2);
    }

    #[test]
    fn exception_while_np_is_set_is_fatal() {
        let (mut cpu, mut bus) = machine(&[short(0b011000, 0, 1)]);
        cpu.set_flag(PSW_NP, true);
        let stop = cpu.step(&mut bus).unwrap_err();

        assert_eq!(
            stop,
            Stop::Fatal {
                code: EXC_TRAP_BASE + 1,
                pc: ROM_BASE + 2
            }
        );
        assert!(cpu.halted);
    }

    #[test]
    fn reset_state_makes_the_first_exception_fatal() {
        let (mut cpu, mut bus) = machine(&[short(0b011000, 0, 1)]);
        cpu.psw = crate::RESET_PSW;
        assert!(cpu.flag(PSW_NP));
        assert!(matches!(
            cpu.step(&mut bus).unwrap_err(),
            Stop::Fatal { .. }
        ));
    }

    #[test]
    fn halt_stops_the_cpu() {
        let (mut cpu, mut bus) = machine(&[short(0b011010, 0, 0)]);
        cpu.step(&mut bus).unwrap();
        assert!(cpu.halted);
        assert_eq!(cpu.step(&mut bus).unwrap_err(), Stop::Halted);
    }

    #[test]
    fn interrupt_flag_instructions_toggle_id() {
        let (mut cpu, mut bus) = machine(&[short(0b011110, 0, 0), short(0b010110, 0, 0)]);
        cpu.step(&mut bus).unwrap();
        assert!(cpu.flag(PSW_ID));
        cpu.step(&mut bus).unwrap();
        assert!(!cpu.flag(PSW_ID));
    }

    #[test]
    fn nvc_byte_and_halfword_exchanges() {
        let (mut cpu, mut bus) = machine(&[short(0b111110, 1, 0), 0b001000 << 10]);
        cpu.regs[1] = 0x1234_5678;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0x1234_7856);

        let (mut cpu, mut bus) = machine(&[short(0b111110, 1, 0), 0b001001 << 10]);
        cpu.regs[1] = 0x1234_5678;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0x5678_1234);
    }

    #[test]
    fn nvc_reverse_and_multiply_halfword() {
        let (mut cpu, mut bus) = machine(&[short(0b111110, 1, 2), 0b001010 << 10]);
        cpu.regs[2] = 0x8000_0001;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0x8000_0001u32.reverse_bits());

        let (mut cpu, mut bus) = machine(&[short(0b111110, 1, 2), 0b001100 << 10]);
        cpu.regs[1] = 3;
        cpu.regs[2] = 0x0001_FFFF;
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1].cast_signed(), -3);
    }

    #[test]
    fn setf_materialises_a_condition() {
        let (mut cpu, mut bus) = machine(&[short(0b010010, 1, 2)]);
        cpu.set_flag(PSW_Z, true);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 1);

        let (mut cpu, mut bus) = machine(&[short(0b010010, 1, 2)]);
        cpu.set_flag(PSW_Z, false);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[1], 0);
    }

    #[test]
    fn caxi_always_writes_back() {
        let (mut cpu, mut bus) = machine(&[short(0b111010, 1, 2), 0x0000]);
        cpu.regs[1] = 0xAAAA_AAAA;
        cpu.regs[2] = 0x0500_0000;
        cpu.regs[30] = 0xBBBB_BBBB;
        bus.write_u32(0x0500_0000, 0xAAAA_AAAA);
        cpu.step(&mut bus).unwrap();

        assert_eq!(bus.read_u32(0x0500_0000), 0xBBBB_BBBB);
        assert_eq!(cpu.regs[1], 0xAAAA_AAAA);
        assert!(cpu.flag(PSW_Z));
    }

    #[test]
    fn unimplemented_instructions_stop_rather_than_lie() {
        let (mut cpu, mut bus) = machine(&[short(0b011111, 0, 0b01011)]);
        assert_eq!(
            cpu.step(&mut bus).unwrap_err(),
            Stop::Unimplemented {
                op: Op::Movbsu,
                pc: ROM_BASE
            }
        );

        let (mut cpu, mut bus) = machine(&[short(0b111110, 1, 2), 0b000100 << 10]);
        assert!(matches!(
            cpu.step(&mut bus).unwrap_err(),
            Stop::Unimplemented { op: Op::AddfS, .. }
        ));
    }

    #[test]
    fn address_trap_fires_before_the_fetch() {
        let (mut cpu, mut bus) = machine(&[short(0b000000, 1, 2)]);
        cpu.adtre = ROM_BASE;
        cpu.set_flag(PSW_AE, true);
        assert_eq!(
            cpu.step(&mut bus).unwrap(),
            StepOutcome::Exception {
                code: EXC_ADDRESS_TRAP
            }
        );

        assert_eq!(cpu.pc, HANDLER_ADDRESS_TRAP);
        assert_eq!(cpu.eipc, ROM_BASE);
        assert!(!cpu.flag(PSW_AE));
    }

    #[test]
    fn r0_stays_zero_through_execution() {
        let (mut cpu, mut bus) = machine(&[short(0b010000, 0, 0x1F)]);
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.regs[0], 0);
    }
}
