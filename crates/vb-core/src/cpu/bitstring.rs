use crate::bus::Bus;
use crate::cpu::decode::Op;
use crate::cpu::state::{Cpu, PSW_Z};

pub const SRC_WORD: u8 = 30;
pub const DST_WORD: u8 = 29;
pub const LENGTH: u8 = 28;
pub const SRC_OFFSET: u8 = 27;
pub const DST_OFFSET: u8 = 26;

fn combine(op: Op, src: u32, dst: u32) -> u32 {
    match op {
        Op::Orbsu => dst | src,
        Op::Andbsu => dst & src,
        Op::Xorbsu => dst ^ src,
        Op::Movbsu => src,
        Op::Ornbsu => dst | (!src & 1),
        Op::Andnbsu => dst & (!src & 1),
        Op::Xornbsu => dst ^ (!src & 1),
        _ => !src & 1,
    }
}

impl Cpu {
    fn normalise_source(&mut self) {
        self.set_reg(SRC_WORD, self.reg(SRC_WORD) & !3);
        self.set_reg(SRC_OFFSET, self.reg(SRC_OFFSET) & 31);
    }

    fn normalise_destination(&mut self) {
        self.set_reg(DST_WORD, self.reg(DST_WORD) & !3);
        self.set_reg(DST_OFFSET, self.reg(DST_OFFSET) & 31);
    }

    // one destination word per call, pc stays put until the string is done
    pub fn step_bit_string(&mut self, bus: &mut Bus, op: Op) -> bool {
        self.normalise_source();
        self.normalise_destination();

        let mut length = self.reg(LENGTH);
        if length == 0 {
            return true;
        }

        let mut src_word = self.reg(SRC_WORD);
        let mut src_offset = self.reg(SRC_OFFSET);
        let dst_word = self.reg(DST_WORD);
        let mut dst_offset = self.reg(DST_OFFSET);

        let mut source = bus.read_u32(src_word);
        let mut destination = bus.read_u32(dst_word);

        while dst_offset < 32 && length > 0 {
            let src_bit = (source >> src_offset) & 1;
            let dst_bit = (destination >> dst_offset) & 1;
            let result = combine(op, src_bit, dst_bit) & 1;

            destination = (destination & !(1 << dst_offset)) | (result << dst_offset);

            src_offset += 1;
            if src_offset == 32 {
                src_offset = 0;
                src_word = src_word.wrapping_add(4);
                source = bus.read_u32(src_word);
            }

            dst_offset += 1;
            length -= 1;
        }

        bus.write_u32(dst_word, destination);

        if dst_offset == 32 {
            dst_offset = 0;
            self.set_reg(DST_WORD, dst_word.wrapping_add(4));
        } else {
            self.set_reg(DST_WORD, dst_word);
        }

        self.set_reg(SRC_WORD, src_word);
        self.set_reg(SRC_OFFSET, src_offset);
        self.set_reg(DST_OFFSET, dst_offset);
        self.set_reg(LENGTH, length);

        length == 0
    }

    // one source word per call, same re-entry rule
    pub fn step_bit_search(&mut self, bus: &mut Bus, op: Op) -> bool {
        self.normalise_source();

        let upward = matches!(op, Op::Sch0bsu | Op::Sch1bsu);
        let wanted = u32::from(matches!(op, Op::Sch1bsu | Op::Sch1bsd));

        self.set_flag(PSW_Z, true);

        let mut length = self.reg(LENGTH);
        if length == 0 {
            return true;
        }

        let mut word = self.reg(SRC_WORD);
        let mut offset = self.reg(SRC_OFFSET);
        let mut skipped = self.reg(DST_WORD);
        let source = bus.read_u32(word);

        let mut found = false;

        loop {
            if length == 0 {
                break;
            }

            let bit = (source >> offset) & 1;
            length -= 1;

            if bit == wanted {
                found = true;
            } else {
                skipped = skipped.wrapping_add(1);
            }

            if upward {
                offset += 1;
                if offset == 32 {
                    offset = 0;
                    word = word.wrapping_add(4);
                }
            } else if offset == 0 {
                offset = 31;
                word = word.wrapping_sub(4);
            } else {
                offset -= 1;
            }

            if found {
                break;
            }

            // stop at a word boundary so interrupts can land between reads
            if (upward && offset == 0) || (!upward && offset == 31) {
                break;
            }
        }

        if found {
            self.set_flag(PSW_Z, false);
        }

        self.set_reg(SRC_WORD, word);
        self.set_reg(SRC_OFFSET, offset);
        self.set_reg(DST_WORD, skipped);
        self.set_reg(LENGTH, length);

        found || length == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::{Cart, tests::test_rom};

    const RAM: u32 = 0x0500_0000;

    fn machine() -> (Cpu, Bus) {
        (Cpu::new(), Bus::new(Cart::new(test_rom(0x1000)).unwrap()))
    }

    fn run_string(cpu: &mut Cpu, bus: &mut Bus, op: Op) -> u32 {
        let mut passes = 0;
        while !cpu.step_bit_string(bus, op) {
            passes += 1;
            assert!(passes < 200, "bit string never finished");
        }
        passes + 1
    }

    #[test]
    fn movbsu_copies_a_whole_word() {
        let (mut cpu, mut bus) = machine();
        bus.write_u32(RAM, 0xDEAD_BEEF);

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(DST_WORD, RAM + 8);
        cpu.set_reg(LENGTH, 32);
        run_string(&mut cpu, &mut bus, Op::Movbsu);

        assert_eq!(bus.read_u32(RAM + 8), 0xDEAD_BEEF);
        assert_eq!(cpu.reg(LENGTH), 0);
    }

    #[test]
    fn a_long_copy_takes_one_pass_per_destination_word() {
        let (mut cpu, mut bus) = machine();
        for i in 0..4 {
            bus.write_u32(RAM + i * 4, 0x1111_1111 * (i + 1));
        }

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(DST_WORD, RAM + 0x100);
        cpu.set_reg(LENGTH, 128);
        let passes = run_string(&mut cpu, &mut bus, Op::Movbsu);

        assert_eq!(passes, 4);
        for i in 0..4 {
            assert_eq!(bus.read_u32(RAM + 0x100 + i * 4), 0x1111_1111 * (i + 1));
        }
    }

    #[test]
    fn pc_stays_put_until_the_string_is_done() {
        let (mut cpu, mut bus) = machine();
        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(DST_WORD, RAM + 0x100);
        cpu.set_reg(LENGTH, 64);

        assert!(!cpu.step_bit_string(&mut bus, Op::Movbsu));
        assert_eq!(cpu.reg(LENGTH), 32);
        assert!(cpu.step_bit_string(&mut bus, Op::Movbsu));
        assert_eq!(cpu.reg(LENGTH), 0);
    }

    #[test]
    fn unaligned_offsets_are_honoured() {
        let (mut cpu, mut bus) = machine();
        bus.write_u32(RAM, 0b1111);

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(SRC_OFFSET, 0);
        cpu.set_reg(DST_WORD, RAM + 8);
        cpu.set_reg(DST_OFFSET, 4);
        cpu.set_reg(LENGTH, 4);
        run_string(&mut cpu, &mut bus, Op::Movbsu);

        assert_eq!(bus.read_u32(RAM + 8), 0b1111_0000);
    }

    #[test]
    fn logic_operations_combine_both_strings() {
        for (op, expected) in [
            (Op::Orbsu, 0b1110u32),
            (Op::Andbsu, 0b1000),
            (Op::Xorbsu, 0b0110),
            (Op::Notbsu, 0b0101),
        ] {
            let (mut cpu, mut bus) = machine();
            bus.write_u32(RAM, 0b1010);
            bus.write_u32(RAM + 8, 0b1100);

            cpu.set_reg(SRC_WORD, RAM);
            cpu.set_reg(DST_WORD, RAM + 8);
            cpu.set_reg(LENGTH, 4);
            run_string(&mut cpu, &mut bus, op);

            assert_eq!(bus.read_u32(RAM + 8) & 0b1111, expected, "{op:?}");
        }
    }

    #[test]
    fn zero_length_completes_immediately() {
        let (mut cpu, mut bus) = machine();
        cpu.set_reg(LENGTH, 0);
        assert!(cpu.step_bit_string(&mut bus, Op::Movbsu));
        assert!(cpu.step_bit_search(&mut bus, Op::Sch1bsu));
    }

    #[test]
    fn addresses_and_offsets_are_masked_before_use() {
        let (mut cpu, mut bus) = machine();
        cpu.set_reg(SRC_WORD, RAM + 3);
        cpu.set_reg(DST_WORD, RAM + 0x103);
        cpu.set_reg(SRC_OFFSET, 0xFFFF_FFE1);
        cpu.set_reg(DST_OFFSET, 0xFFFF_FFE2);
        cpu.set_reg(LENGTH, 1);

        cpu.step_bit_string(&mut bus, Op::Movbsu);

        assert_eq!(cpu.reg(SRC_WORD) & 3, 0);
        assert_eq!(cpu.reg(DST_WORD) & 3, 0);
    }

    #[test]
    fn search_finds_a_set_bit_and_clears_zero() {
        let (mut cpu, mut bus) = machine();
        bus.write_u32(RAM, 1 << 5);

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(LENGTH, 32);
        cpu.set_reg(DST_WORD, 0);

        while !cpu.step_bit_search(&mut bus, Op::Sch1bsu) {}

        assert!(!cpu.flag(PSW_Z));
        assert_eq!(cpu.reg(DST_WORD), 5);
        assert_eq!(cpu.reg(SRC_OFFSET), 6);
    }

    #[test]
    fn search_that_finds_nothing_leaves_zero_set() {
        let (mut cpu, mut bus) = machine();
        bus.write_u32(RAM, 0);

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(LENGTH, 32);
        cpu.set_reg(DST_WORD, 0);

        while !cpu.step_bit_search(&mut bus, Op::Sch1bsu) {}

        assert!(cpu.flag(PSW_Z));
        assert_eq!(cpu.reg(DST_WORD), 32);
        assert_eq!(cpu.reg(LENGTH), 0);
    }

    #[test]
    fn search_downward_walks_backwards() {
        let (mut cpu, mut bus) = machine();
        bus.write_u32(RAM, 1 << 20);

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(SRC_OFFSET, 31);
        cpu.set_reg(LENGTH, 32);
        cpu.set_reg(DST_WORD, 0);

        while !cpu.step_bit_search(&mut bus, Op::Sch1bsd) {}

        assert!(!cpu.flag(PSW_Z));
        assert_eq!(cpu.reg(DST_WORD), 11);
        assert_eq!(cpu.reg(SRC_OFFSET), 19);
    }

    #[test]
    fn search_for_zero_works_too() {
        let (mut cpu, mut bus) = machine();
        bus.write_u32(RAM, !(1 << 3));

        cpu.set_reg(SRC_WORD, RAM);
        cpu.set_reg(LENGTH, 32);
        cpu.set_reg(DST_WORD, 0);

        while !cpu.step_bit_search(&mut bus, Op::Sch0bsu) {}

        assert!(!cpu.flag(PSW_Z));
        assert_eq!(cpu.reg(DST_WORD), 3);
    }
}
