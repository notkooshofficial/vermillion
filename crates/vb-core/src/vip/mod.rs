pub mod registers;

pub use registers::Registers;

// the whole map repeats above this, so every access folds into one 512k window
pub const VIP_MASK: u32 = 0x0007_FFFF;

pub const VRAM_LEN: usize = 0x0004_0000;

pub const LEFT_FRAME_0: u32 = 0x0000_0000;
pub const LEFT_FRAME_1: u32 = 0x0000_8000;
pub const RIGHT_FRAME_0: u32 = 0x0001_0000;
pub const RIGHT_FRAME_1: u32 = 0x0001_8000;
pub const FRAME_LEN: u32 = 0x6000;

pub const CHARACTER_TABLE_0: u32 = 0x0000_6000;
pub const CHARACTER_TABLE_STRIDE: u32 = 0x8000;
pub const CHARACTER_TABLE_LEN: u32 = 0x2000;

pub const CHARACTER_MIRROR: u32 = 0x0007_8000;
pub const CHARACTER_MIRROR_END: u32 = 0x0007_FFFF;

pub const BG_MAPS: u32 = 0x0002_0000;
pub const WORLD_ATTRIBUTES: u32 = 0x0003_D800;
pub const LEFT_COLUMN_TABLE: u32 = 0x0003_DC00;
pub const RIGHT_COLUMN_TABLE: u32 = 0x0003_DE00;
pub const OBJECT_ATTRIBUTES: u32 = 0x0003_E000;

pub const REGISTERS_START: u32 = 0x0005_E000;
pub const REGISTERS_END: u32 = 0x0005_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Vram(u32),
    Register(u32),
    Unmapped,
}

// the mirrors exist so character memory can be walked as one linear block
pub fn resolve(addr: u32) -> Target {
    let addr = addr & VIP_MASK;

    if (addr as usize) < VRAM_LEN {
        return Target::Vram(addr);
    }

    if (REGISTERS_START..=REGISTERS_END).contains(&addr) {
        return Target::Register(addr);
    }

    if (CHARACTER_MIRROR..=CHARACTER_MIRROR_END).contains(&addr) {
        let offset = addr - CHARACTER_MIRROR;
        let table = offset / CHARACTER_TABLE_LEN;
        let within = offset % CHARACTER_TABLE_LEN;
        return Target::Vram(CHARACTER_TABLE_0 + table * CHARACTER_TABLE_STRIDE + within);
    }

    Target::Unmapped
}

pub struct Vip {
    vram: Box<[u8]>,
    pub regs: Registers,
}

impl Default for Vip {
    fn default() -> Self {
        Self::new()
    }
}

impl Vip {
    pub fn new() -> Self {
        Self {
            vram: vec![0; VRAM_LEN].into_boxed_slice(),
            regs: Registers::new(),
        }
    }

    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }

    pub fn read_u8(&self, addr: u32) -> u8 {
        match resolve(addr) {
            Target::Vram(offset) => self.vram[offset as usize],
            Target::Register(reg) => self.regs.read(reg & !1).to_le_bytes()[(reg & 1) as usize],
            Target::Unmapped => 0,
        }
    }

    pub fn read_u16(&self, addr: u32) -> u16 {
        match resolve(addr & !1) {
            Target::Vram(offset) => {
                let at = offset as usize;
                u16::from_le_bytes([self.vram[at], self.vram[at + 1]])
            }
            Target::Register(reg) => self.regs.read(reg),
            Target::Unmapped => 0,
        }
    }

    pub fn read_u32(&self, addr: u32) -> u32 {
        let addr = addr & !3;
        u32::from(self.read_u16(addr)) | (u32::from(self.read_u16(addr.wrapping_add(2))) << 16)
    }

    pub fn write_u16(&mut self, addr: u32, value: u16) {
        match resolve(addr & !1) {
            Target::Vram(offset) => {
                let at = offset as usize;
                self.vram[at..at + 2].copy_from_slice(&value.to_le_bytes());
            }
            Target::Register(reg) => self.regs.write(reg, value),
            Target::Unmapped => {}
        }
    }

    pub fn write_u32(&mut self, addr: u32, value: u32) {
        let addr = addr & !3;
        self.write_u16(addr, (value & 0xFFFF) as u16);
        self.write_u16(addr.wrapping_add(2), (value >> 16) as u16);
    }

    // a byte store into the register file is really a halfword store, and it takes the
    // whole source register rather than the byte the cpu would normally hand over
    pub fn write_u8(&mut self, addr: u32, source: u32) {
        match resolve(addr) {
            Target::Vram(offset) => self.vram[offset as usize] = (source & 0xFF) as u8,
            Target::Register(reg) => {
                let low = u16::from_le_bytes([(source & 0xFF) as u8, ((source >> 8) & 0xFF) as u8]);
                let value = if reg & 1 == 0 {
                    low
                } else {
                    u16::from(low.to_le_bytes()[0]) << 8
                };
                self.regs.write(reg & !1, value);
            }
            Target::Unmapped => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::registers::{BKCOL, BRTA, VER, VER_VALUE};
    use super::*;

    #[test]
    fn regions_resolve_where_the_map_says() {
        assert_eq!(resolve(LEFT_FRAME_0), Target::Vram(0));
        assert_eq!(resolve(CHARACTER_TABLE_0), Target::Vram(CHARACTER_TABLE_0));
        assert_eq!(resolve(OBJECT_ATTRIBUTES), Target::Vram(OBJECT_ATTRIBUTES));
        assert_eq!(resolve(BRTA), Target::Register(BRTA));
        assert_eq!(resolve(0x0004_0000), Target::Unmapped);
        assert_eq!(resolve(0x0006_0000), Target::Unmapped);
    }

    #[test]
    fn the_character_mirrors_land_on_the_real_tables() {
        for table in 0..4u32 {
            let mirror = CHARACTER_MIRROR + table * CHARACTER_TABLE_LEN;
            let real = CHARACTER_TABLE_0 + table * CHARACTER_TABLE_STRIDE;
            assert_eq!(resolve(mirror), Target::Vram(real), "table {table}");
            assert_eq!(
                resolve(mirror + CHARACTER_TABLE_LEN - 1),
                Target::Vram(real + CHARACTER_TABLE_LEN - 1)
            );
        }
    }

    #[test]
    fn a_write_through_a_mirror_is_visible_at_the_real_address() {
        let mut vip = Vip::new();
        vip.write_u16(CHARACTER_MIRROR + 0x40, 0xBEEF);
        assert_eq!(vip.read_u16(CHARACTER_TABLE_0 + 0x40), 0xBEEF);

        vip.write_u16(0x0001_E000 + 0x20, 0x1234);
        assert_eq!(vip.read_u16(CHARACTER_MIRROR + 0x6020), 0x1234);
    }

    #[test]
    fn characters_are_contiguous_through_the_mirror() {
        let mut vip = Vip::new();
        vip.write_u16(CHARACTER_MIRROR + 0x1FF0, 0xAAAA);
        vip.write_u16(CHARACTER_MIRROR + 0x2000, 0x5555);

        assert_eq!(vip.read_u16(CHARACTER_TABLE_0 + 0x1FF0), 0xAAAA);
        assert_eq!(vip.read_u16(0x0000_E000), 0x5555);
    }

    #[test]
    fn the_whole_map_mirrors_every_512k() {
        let mut vip = Vip::new();
        vip.write_u16(0x0002_0000, 0x0F0F);

        assert_eq!(vip.read_u16(0x0008_0000 + 0x0002_0000), 0x0F0F);
        assert_eq!(vip.read_u16(0x00F0_0000 + 0x0002_0000), 0x0F0F);
    }

    #[test]
    fn vram_round_trips_at_every_width() {
        let mut vip = Vip::new();
        vip.write_u16(BG_MAPS, 0x1234);
        vip.write_u32(BG_MAPS + 4, 0xDEAD_BEEF);
        vip.write_u8(BG_MAPS + 8, 0x99);

        assert_eq!(vip.read_u16(BG_MAPS), 0x1234);
        assert_eq!(vip.read_u32(BG_MAPS + 4), 0xDEAD_BEEF);
        assert_eq!(vip.read_u8(BG_MAPS + 8), 0x99);
        assert_eq!(vip.read_u8(BG_MAPS), 0x34);
        assert_eq!(vip.read_u8(BG_MAPS + 1), 0x12);
    }

    #[test]
    fn unmapped_reads_zero_and_swallows_writes() {
        let mut vip = Vip::new();
        vip.write_u16(0x0004_0000, 0xFFFF);
        vip.write_u32(0x0006_0000, 0xFFFF_FFFF);
        assert_eq!(vip.read_u16(0x0004_0000), 0);
        assert_eq!(vip.read_u32(0x0006_0000), 0);
    }

    #[test]
    fn registers_are_reachable_through_the_vip() {
        let mut vip = Vip::new();
        vip.write_u16(BRTA, 0x0042);
        assert_eq!(vip.read_u16(BRTA), 0x0042);
        assert_eq!(vip.read_u16(VER), VER_VALUE);
    }

    #[test]
    fn an_even_byte_write_stores_the_whole_source_halfword() {
        let mut vip = Vip::new();
        vip.write_u8(BRTA, 0x0000_1234);
        assert_eq!(vip.read_u16(BRTA), 0x0034, "brta keeps only its low byte");

        vip.write_u8(BKCOL, 0x0000_FF03);
        assert_eq!(vip.read_u16(BKCOL), 0x0003);
    }

    #[test]
    fn an_odd_byte_write_shifts_the_source_into_the_high_byte() {
        let mut vip = Vip::new();
        vip.write_u8(0x0005_F849, 0x0000_0002);
        assert_eq!(vip.read_u16(0x0005_F848), 0x0200);
    }

    #[test]
    fn a_byte_write_to_vram_stays_a_byte() {
        let mut vip = Vip::new();
        vip.write_u8(BG_MAPS, 0x0000_1234);
        assert_eq!(vip.read_u8(BG_MAPS), 0x34);
        assert_eq!(
            vip.read_u8(BG_MAPS + 1),
            0x00,
            "no halfword anomaly outside the registers"
        );
    }

    #[test]
    fn frame_buffers_do_not_overlap_the_character_tables() {
        let mut vip = Vip::new();
        vip.write_u16(LEFT_FRAME_0 + FRAME_LEN - 2, 0x1111);
        vip.write_u16(CHARACTER_TABLE_0, 0x2222);
        vip.write_u16(LEFT_FRAME_1, 0x3333);
        vip.write_u16(RIGHT_FRAME_0, 0x4444);
        vip.write_u16(RIGHT_FRAME_1, 0x5555);

        assert_eq!(vip.read_u16(LEFT_FRAME_0 + FRAME_LEN - 2), 0x1111);
        assert_eq!(vip.read_u16(CHARACTER_TABLE_0), 0x2222);
        assert_eq!(vip.read_u16(LEFT_FRAME_1), 0x3333);
        assert_eq!(vip.read_u16(RIGHT_FRAME_0), 0x4444);
        assert_eq!(vip.read_u16(RIGHT_FRAME_1), 0x5555);
    }
}
