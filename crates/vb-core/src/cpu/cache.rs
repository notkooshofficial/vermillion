use crate::bus::{ADDRESS_MASK, Bus, Region};
use crate::cpu::state::{Cpu, SR_CHCW};

pub const ENTRIES: u32 = 128;
pub const BLOCK_LEN: u32 = 8;
pub const TAG_LEN: u32 = 4;
pub const SPILL_LEN: u32 = ENTRIES * (BLOCK_LEN + TAG_LEN);

pub const CHCW_ICC: u32 = 1 << 0;
pub const CHCW_ICE: u32 = 1 << 1;
pub const CHCW_ICD: u32 = 1 << 4;
pub const CHCW_ICR: u32 = 1 << 5;

pub const TAG_VALID: u32 = 1 << 22;
pub const TAG_MASK: u32 = 0x003F_FFFF;

const CEN_SHIFT: u32 = 20;
const CEC_SHIFT: u32 = 8;
const CLEAR_FIELD: u32 = 0xFFF;

// sa shares bits with cen, so an address wider than the bus reads back as cen >= 128,
// which the reference says performs no dump or restore
const SPILL_LIMIT: u32 = ADDRESS_MASK + 1;

fn index_of(addr: u32) -> usize {
    ((addr >> 3) & 0x7F) as usize
}

fn tag_of(addr: u32) -> u32 {
    addr >> 10
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    blocks: [[u8; BLOCK_LEN as usize]; ENTRIES as usize],
    tags: [u32; ENTRIES as usize],
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self {
            blocks: [[0; BLOCK_LEN as usize]; ENTRIES as usize],
            tags: [0; ENTRIES as usize],
        }
    }

    pub fn tag(&self, index: u32) -> u32 {
        self.tags[(index % ENTRIES) as usize]
    }

    pub fn block(&self, index: u32) -> [u8; BLOCK_LEN as usize] {
        self.blocks[(index % ENTRIES) as usize]
    }

    // a restored tag may carry nec reserved bits, so compare the two fields separately
    pub fn contains(&self, addr: u32) -> bool {
        let entry = self.tags[index_of(addr)];
        entry & TAG_VALID != 0 && entry & TAG_MASK == tag_of(addr)
    }

    // blocks are modelled so a dump produces real data, but memory stays the authority
    // for what executes: a stale entry must never feed the cpu
    pub fn fill(&mut self, bus: &Bus, addr: u32) {
        if !Region::of(addr).is_memory() || self.contains(addr) {
            return;
        }

        let index = index_of(addr);
        let base = addr & !(BLOCK_LEN - 1);
        for offset in 0..BLOCK_LEN {
            self.blocks[index][offset as usize] = bus.read_u8(base.wrapping_add(offset));
        }
        self.tags[index] = TAG_VALID | tag_of(addr);
    }

    // clear only resets the valid bit, block and tag memory keep their contents
    pub fn clear(&mut self, first: u32, count: u32) {
        let last = first.saturating_add(count.min(ENTRIES)).min(ENTRIES);
        for index in first..last {
            self.tags[index as usize] &= !TAG_VALID;
        }
    }

    pub fn dump(&self, bus: &mut Bus, spill: u32) {
        for index in 0..ENTRIES {
            let base = spill.wrapping_add(index * BLOCK_LEN);
            for offset in 0..BLOCK_LEN {
                let byte = self.blocks[index as usize][offset as usize];
                bus.write_u8(base.wrapping_add(offset), byte);
            }
        }

        let tags = spill.wrapping_add(ENTRIES * BLOCK_LEN);
        for index in 0..ENTRIES {
            bus.write_u32(
                tags.wrapping_add(index * TAG_LEN),
                self.tags[index as usize],
            );
        }
    }

    pub fn restore(&mut self, bus: &Bus, spill: u32) {
        for index in 0..ENTRIES {
            let base = spill.wrapping_add(index * BLOCK_LEN);
            for offset in 0..BLOCK_LEN {
                self.blocks[index as usize][offset as usize] =
                    bus.read_u8(base.wrapping_add(offset));
            }
        }

        let tags = spill.wrapping_add(ENTRIES * BLOCK_LEN);
        for index in 0..ENTRIES {
            self.tags[index as usize] = bus.read_u32(tags.wrapping_add(index * TAG_LEN));
        }
    }
}

impl Cpu {
    pub fn cache_enabled(&self) -> bool {
        self.chcw & CHCW_ICE != 0
    }

    // dump and restore run to completion inside the ldsr, which is how the hardware
    // postpones interrupts until they finish. requesting more than one operation at
    // once is undefined, so clear wins, then dump, then restore
    pub fn cache_control(&mut self, bus: &mut Bus, value: u32) {
        self.write_system_register(SR_CHCW, value);

        if value & CHCW_ICC != 0 {
            let first = (value >> CEN_SHIFT) & CLEAR_FIELD;
            let count = (value >> CEC_SHIFT) & CLEAR_FIELD;
            self.cache.clear(first, count);
            return;
        }

        let spill = value & !0xFF;
        if spill >= SPILL_LIMIT {
            return;
        }

        if value & CHCW_ICD != 0 {
            self.cache.dump(bus, spill);
        } else if value & CHCW_ICR != 0 {
            self.cache.restore(bus, spill);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::{Cart, tests::test_rom};

    const ROM: u32 = 0x0700_0000;
    const RAM: u32 = 0x0500_0000;

    fn machine() -> (Cpu, Bus) {
        let mut rom = test_rom(0x1000);
        for (index, byte) in rom.iter_mut().take(0x400).enumerate() {
            *byte = u8::try_from(index & 0xFF).unwrap_or(0);
        }
        (Cpu::new(), Bus::new(Cart::new(rom).unwrap()))
    }

    #[test]
    fn a_new_cache_holds_nothing() {
        let cache = Cache::new();
        for index in 0..ENTRIES {
            assert_eq!(cache.tag(index), 0);
        }
        assert!(!cache.contains(ROM));
    }

    #[test]
    fn a_fill_captures_the_whole_block() {
        let (mut cpu, bus) = machine();
        cpu.cache.fill(&bus, ROM + 0x2A);

        assert!(cpu.cache.contains(ROM + 0x2A));
        assert!(
            cpu.cache.contains(ROM + 0x28),
            "the block covers eight bytes"
        );
        assert!(!cpu.cache.contains(ROM + 0x30));

        let index = u32::try_from(index_of(ROM + 0x2A)).unwrap();
        assert_eq!(cpu.cache.block(index)[2], bus.read_u8(ROM + 0x2A));
    }

    #[test]
    fn entries_are_selected_by_index_so_distant_blocks_evict() {
        let (mut cpu, bus) = machine();
        cpu.cache.fill(&bus, ROM);
        assert!(cpu.cache.contains(ROM));

        cpu.cache.fill(&bus, ROM + 0x400);
        assert!(cpu.cache.contains(ROM + 0x400));
        assert!(!cpu.cache.contains(ROM), "same index, different tag");
    }

    #[test]
    fn devices_are_never_cached() {
        let (mut cpu, bus) = machine();
        for base in [0x0000_0000u32, 0x0100_0000, 0x0200_0000, 0x0400_0000] {
            cpu.cache.fill(&bus, base);
            assert!(!cpu.cache.contains(base), "region at {base:#010X}");
        }
    }

    #[test]
    fn clear_frees_entries_without_wiping_them() {
        let (mut cpu, bus) = machine();
        cpu.cache.fill(&bus, ROM);
        let block = cpu.cache.block(0);

        cpu.cache.clear(0, 1);

        assert!(!cpu.cache.contains(ROM));
        assert_eq!(cpu.cache.block(0), block);
        assert_eq!(cpu.cache.tag(0) & TAG_MASK, tag_of(ROM));
    }

    #[test]
    fn clear_stops_after_the_last_entry() {
        let (mut cpu, bus) = machine();
        for index in 0..ENTRIES {
            cpu.cache.fill(&bus, ROM + index * BLOCK_LEN);
        }

        cpu.cache.clear(120, 0xFFF);

        assert!(cpu.cache.contains(ROM + 119 * BLOCK_LEN));
        assert!(!cpu.cache.contains(ROM + 127 * BLOCK_LEN));
    }

    #[test]
    fn a_clear_of_no_entries_clears_nothing() {
        let (mut cpu, bus) = machine();
        cpu.cache.fill(&bus, ROM);
        cpu.cache.clear(0, 0);
        assert!(cpu.cache.contains(ROM));
    }

    #[test]
    fn clear_beyond_the_last_entry_does_nothing() {
        let (mut cpu, bus) = machine();
        cpu.cache.fill(&bus, ROM);
        cpu.cache.clear(200, 64);
        assert!(cpu.cache.contains(ROM));
    }

    #[test]
    fn dump_and_restore_round_trip_through_memory() {
        let (mut cpu, mut bus) = machine();
        for index in 0..ENTRIES {
            cpu.cache.fill(&bus, ROM + index * BLOCK_LEN);
        }
        let before = cpu.cache.clone();

        cpu.cache.dump(&mut bus, RAM);
        cpu.cache = Cache::new();
        assert_ne!(cpu.cache, before);

        cpu.cache.restore(&bus, RAM);
        assert_eq!(cpu.cache, before);
    }

    #[test]
    fn a_dump_writes_blocks_then_tags() {
        let (mut cpu, mut bus) = machine();
        cpu.cache.fill(&bus, ROM);
        cpu.cache.dump(&mut bus, RAM);

        assert_eq!(bus.read_u8(RAM), bus.read_u8(ROM));
        assert_eq!(
            bus.read_u32(RAM + ENTRIES * BLOCK_LEN),
            TAG_VALID | tag_of(ROM)
        );
        assert_eq!(bus.read_u32(RAM + SPILL_LEN - TAG_LEN), 0);
    }

    #[test]
    fn ldsr_keeps_only_the_enable_bit() {
        let (mut cpu, mut bus) = machine();
        let no_operation = !(CHCW_ICC | CHCW_ICD | CHCW_ICR);
        cpu.cache_control(&mut bus, no_operation);
        assert_eq!(cpu.chcw, CHCW_ICE);
        assert_eq!(cpu.read_system_register(SR_CHCW), CHCW_ICE);
    }

    #[test]
    fn ldsr_performs_the_requested_operation() {
        let (mut cpu, mut bus) = machine();
        cpu.cache.fill(&bus, ROM);

        cpu.cache_control(&mut bus, RAM | CHCW_ICD);
        assert_eq!(bus.read_u8(RAM), bus.read_u8(ROM));

        cpu.cache_control(&mut bus, CHCW_ICC | (1 << CEC_SHIFT));
        assert!(!cpu.cache.contains(ROM));

        cpu.cache_control(&mut bus, RAM | CHCW_ICR);
        assert!(cpu.cache.contains(ROM));
    }

    #[test]
    fn a_spill_address_wider_than_the_bus_is_ignored() {
        let (mut cpu, mut bus) = machine();
        cpu.cache.fill(&bus, ROM);
        cpu.cache_control(&mut bus, 0x0800_0000 | CHCW_ICD);
        assert_eq!(bus.read_u8(RAM), 0);
    }

    #[test]
    fn a_clear_request_ignores_the_dump_bits() {
        let (mut cpu, mut bus) = machine();
        cpu.cache.fill(&bus, ROM);
        cpu.cache_control(&mut bus, CHCW_ICC | CHCW_ICD | (1 << CEC_SHIFT));

        assert!(!cpu.cache.contains(ROM));
        assert_eq!(bus.read_u8(RAM), 0, "no dump alongside a clear");
    }
}
