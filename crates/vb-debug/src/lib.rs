use std::fmt::Write as _;

use vb_core::cpu::decode::{Condition, Format, Instruction, Op};

pub fn system_register_name(index: u8) -> &'static str {
    match index & 0x1F {
        0 => "eipc",
        1 => "eipsw",
        2 => "fepc",
        3 => "fepsw",
        4 => "ecr",
        5 => "psw",
        6 => "pir",
        7 => "tkcw",
        24 => "chcw",
        25 => "adtre",
        29 => "sr29",
        30 => "sr30",
        31 => "sr31",
        _ => "reserved",
    }
}

// condition 13 is written "nop" with no operand, every other condition is b + mnemonic
pub fn branch_mnemonic(condition: Condition) -> &'static str {
    match condition {
        Condition::Overflow => "bv",
        Condition::Carry => "bc",
        Condition::Zero => "be",
        Condition::NotHigher => "bnh",
        Condition::Negative => "bn",
        Condition::Always => "br",
        Condition::LessThan => "blt",
        Condition::LessOrEqual => "ble",
        Condition::NotOverflow => "bnv",
        Condition::NotCarry => "bnc",
        Condition::NotZero => "bne",
        Condition::Higher => "bh",
        Condition::Positive => "bp",
        Condition::Never => "nop",
        Condition::GreaterOrEqual => "bge",
        Condition::GreaterThan => "bgt",
    }
}

fn signed_hex(value: i32) -> String {
    if value < 0 {
        format!("-{:#x}", value.unsigned_abs())
    } else {
        format!("{value:#x}")
    }
}

fn memory_operand(instruction: Instruction) -> String {
    if instruction.imm == 0 {
        format!("[r{}]", instruction.reg1)
    } else {
        format!("{}[r{}]", signed_hex(instruction.imm), instruction.reg1)
    }
}

pub fn mnemonic(instruction: Instruction) -> &'static str {
    match instruction.op {
        Op::Bcond => instruction.cond.map_or("b", branch_mnemonic),
        other => other.mnemonic(),
    }
}

pub fn disassemble(instruction: Instruction, pc: u32) -> String {
    let name = mnemonic(instruction);
    let operands = operands(instruction, pc);

    if operands.is_empty() {
        name.to_string()
    } else {
        format!("{name:<7} {operands}")
    }
}

fn operands(instruction: Instruction, pc: u32) -> String {
    let mut text = String::new();
    let reg1 = instruction.reg1;
    let reg2 = instruction.reg2;
    let imm = instruction.imm;

    match instruction.op {
        Op::Jmp => {
            let _ = write!(text, "[r{reg1}]");
        }
        Op::Bcond => {
            if instruction.cond != Some(Condition::Never) {
                let _ = write!(text, "{:#010x}", pc.wrapping_add_signed(imm) & !1);
            }
        }
        Op::Jr | Op::Jal => {
            let _ = write!(text, "{:#010x}", pc.wrapping_add_signed(imm) & !1);
        }

        Op::LdB | Op::LdH | Op::LdW | Op::InB | Op::InH | Op::InW | Op::Caxi => {
            let _ = write!(text, "{}, r{reg2}", memory_operand(instruction));
        }
        Op::StB | Op::StH | Op::StW | Op::OutB | Op::OutH | Op::OutW => {
            let _ = write!(text, "r{reg2}, {}", memory_operand(instruction));
        }

        Op::Ldsr => {
            let _ = write!(
                text,
                "r{reg2}, {}",
                system_register_name(u8::try_from(imm & 0x1F).unwrap_or(0))
            );
        }
        Op::Stsr => {
            let _ = write!(
                text,
                "{}, r{reg2}",
                system_register_name(u8::try_from(imm & 0x1F).unwrap_or(0))
            );
        }
        Op::Setf => {
            let condition = instruction.cond.map_or("?", Condition::mnemonic);
            let _ = write!(text, "{condition}, r{reg2}");
        }
        Op::Trap => {
            let _ = write!(text, "{}", signed_hex(imm));
        }

        Op::Movea | Op::Addi | Op::Ori | Op::Andi | Op::Xori | Op::Movhi => {
            let _ = write!(text, "{}, r{reg1}, r{reg2}", signed_hex(imm));
        }

        Op::MovImm | Op::AddImm | Op::CmpImm | Op::ShlImm | Op::ShrImm | Op::SarImm => {
            let _ = write!(text, "{}, r{reg2}", signed_hex(imm));
        }

        Op::Reti | Op::Halt | Op::Cli | Op::Sei => {}

        Op::Sch0bsu
        | Op::Sch0bsd
        | Op::Sch1bsu
        | Op::Sch1bsd
        | Op::Orbsu
        | Op::Andbsu
        | Op::Xorbsu
        | Op::Movbsu
        | Op::Ornbsu
        | Op::Andnbsu
        | Op::Xornbsu
        | Op::Notbsu => {}

        _ => match instruction.format {
            Format::I | Format::VII => {
                let _ = write!(text, "r{reg1}, r{reg2}");
            }
            _ => {
                let _ = write!(text, "{}, r{reg2}", signed_hex(imm));
            }
        },
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use vb_core::cpu::decode::decode;

    fn short(opcode: u8, reg2: u8, reg1: u8) -> u16 {
        (u16::from(opcode) << 10) | (u16::from(reg2) << 5) | u16::from(reg1)
    }

    fn branch(cond: u8, disp: i16) -> u16 {
        let raw = u16::from_le_bytes(disp.to_le_bytes());
        0b100 << 13 | (u16::from(cond) << 9) | (raw & 0x01FF)
    }

    fn text(word0: u16, word1: u16, pc: u32) -> String {
        disassemble(decode(word0, word1).unwrap(), pc)
    }

    #[test]
    fn destination_is_on_the_right() {
        assert_eq!(text(short(0b000000, 2, 1), 0, 0), "mov     r1, r2");
        assert_eq!(text(short(0b000001, 5, 6), 0, 0), "add     r6, r5");
    }

    #[test]
    fn three_operand_forms_put_the_immediate_first() {
        assert_eq!(
            text(short(0b101001, 3, 2), 0x0010, 0),
            "addi    0x10, r2, r3"
        );
        assert_eq!(
            text(short(0b101000, 3, 2), 0xFFFF, 0),
            "movea   -0x1, r2, r3"
        );
    }

    #[test]
    fn loads_and_stores_bracket_the_base_register() {
        assert_eq!(
            text(short(0b110011, 1, 2), 0x0008, 0),
            "ld.w    0x8[r2], r1"
        );
        assert_eq!(
            text(short(0b110111, 1, 2), 0x0008, 0),
            "st.w    r1, 0x8[r2]"
        );
    }

    #[test]
    fn zero_displacement_is_omitted() {
        assert_eq!(text(short(0b110011, 1, 2), 0x0000, 0), "ld.w    [r2], r1");
        assert_eq!(text(short(0b110100, 1, 2), 0x0000, 0), "st.b    r1, [r2]");
    }

    #[test]
    fn negative_displacement_keeps_its_sign() {
        assert_eq!(
            text(short(0b110011, 1, 2), 0xFFFC, 0),
            "ld.w    -0x4[r2], r1"
        );
    }

    #[test]
    fn branches_print_absolute_targets() {
        assert_eq!(text(branch(6, 8), 0, 0x0700_0000), "blt     0x07000008");
        assert_eq!(text(branch(2, -4), 0, 0x0700_0010), "be      0x0700000c");
    }

    #[test]
    fn never_condition_is_a_bare_nop() {
        assert_eq!(text(branch(13, 8), 0, 0x0700_0000), "nop");
    }

    #[test]
    fn jumps_print_absolute_targets_and_jmp_brackets_its_register() {
        assert_eq!(text(short(0b000110, 0, 7), 0, 0), "jmp     [r7]");
        assert_eq!(
            text(short(0b101010, 0, 0), 0x0020, 0x0700_0000),
            "jr      0x07000020"
        );
    }

    #[test]
    fn system_registers_use_symbolic_names() {
        assert_eq!(text(short(0b011100, 4, 5), 0, 0), "ldsr    r4, psw");
        assert_eq!(text(short(0b011101, 4, 24), 0, 0), "stsr    chcw, r4");
        assert_eq!(text(short(0b011101, 4, 9), 0, 0), "stsr    reserved, r4");
    }

    #[test]
    fn setf_leads_with_its_condition() {
        assert_eq!(text(short(0b010010, 9, 10), 0, 0), "setf    ne, r9");
    }

    #[test]
    fn operandless_instructions_print_bare() {
        assert_eq!(text(short(0b011001, 0, 0), 0, 0), "reti");
        assert_eq!(text(short(0b011010, 0, 0), 0, 0), "halt");
        assert_eq!(text(short(0b010110, 0, 0), 0, 0), "cli");
        assert_eq!(text(short(0b011111, 0, 0b01011), 0, 0), "movbsu");
    }

    #[test]
    fn nvc_instructions_disassemble() {
        assert_eq!(
            text(short(0b111110, 1, 2), 0b001010 << 10, 0),
            "rev     r2, r1"
        );
        assert_eq!(
            text(short(0b111110, 1, 2), 0b001100 << 10, 0),
            "mpyhw   r2, r1"
        );
    }

    #[test]
    fn every_decodable_opcode_produces_output() {
        for opcode in 0u8..64 {
            let word0 = short(opcode, 1, 2);
            if let Ok(instruction) = decode(word0, 0) {
                let line = disassemble(instruction, 0x0700_0000);
                assert!(!line.is_empty(), "opcode {opcode:#08b}");
                assert!(!line.contains("??"), "opcode {opcode:#08b}: {line}");
            }
        }
    }
}
