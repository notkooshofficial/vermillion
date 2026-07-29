use crate::cart::Cart;
use crate::interrupt::{SOURCES, Source};
use crate::pad::GamePad;
use crate::timer::Timer;
use crate::vip::Vip;
use crate::wait::WaitController;

pub const ADDRESS_MASK: u32 = 0x07FF_FFFF;
pub const REGION_LEN: u32 = 0x0100_0000;
pub const WRAM_LEN: usize = 0x1_0000;
pub const WRAM_MASK: u32 = 0x0000_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    Vip,
    Vsu,
    Misc,
    Unmapped,
    Expansion,
    Wram,
    CartRam,
    CartRom,
}

impl Region {
    pub fn of(addr: u32) -> Region {
        match (addr >> 24) & 0x7 {
            0 => Region::Vip,
            1 => Region::Vsu,
            2 => Region::Misc,
            3 => Region::Unmapped,
            4 => Region::Expansion,
            5 => Region::Wram,
            6 => Region::CartRam,
            _ => Region::CartRom,
        }
    }

    pub fn is_memory(self) -> bool {
        matches!(self, Region::Wram | Region::CartRam | Region::CartRom)
    }

    pub fn base(self) -> u32 {
        match self {
            Region::Vip => 0x0000_0000,
            Region::Vsu => 0x0100_0000,
            Region::Misc => 0x0200_0000,
            Region::Unmapped => 0x0300_0000,
            Region::Expansion => 0x0400_0000,
            Region::Wram => 0x0500_0000,
            Region::CartRam => 0x0600_0000,
            Region::CartRom => 0x0700_0000,
        }
    }
}

pub struct Bus {
    cart: Cart,
    wram: Box<[u8]>,
    timer: Timer,
    wait: WaitController,
    pad: GamePad,
    vip: Vip,
}

impl Bus {
    pub fn new(cart: Cart) -> Self {
        Self {
            cart,
            wram: vec![0; WRAM_LEN].into_boxed_slice(),
            timer: Timer::new(),
            wait: WaitController::new(),
            pad: GamePad::new(),
            vip: Vip::new(),
        }
    }

    pub fn vip(&self) -> &Vip {
        &self.vip
    }

    pub fn vip_mut(&mut self) -> &mut Vip {
        &mut self.vip
    }

    pub fn pad(&self) -> &GamePad {
        &self.pad
    }

    pub fn pad_mut(&mut self) -> &mut GamePad {
        &mut self.pad
    }

    pub fn timer(&self) -> &Timer {
        &self.timer
    }

    pub fn timer_mut(&mut self) -> &mut Timer {
        &mut self.timer
    }

    pub fn wait(&self) -> &WaitController {
        &self.wait
    }

    pub fn wait_mut(&mut self) -> &mut WaitController {
        &mut self.wait
    }

    pub fn tick(&mut self, cycles: u64) {
        self.timer.tick(cycles);
        self.pad.tick(cycles);
    }

    pub fn pending_interrupt(&self) -> Option<Source> {
        SOURCES.into_iter().find(|source| self.raised(*source))
    }

    fn raised(&self, source: Source) -> bool {
        match source {
            Source::TimerZero => self.timer.interrupt_pending(),
            Source::GamePad => self.pad.interrupt_pending(),
            // the vip, game pak and communication port do not exist yet
            Source::Vip | Source::Communication | Source::GamePak => false,
        }
    }

    pub fn may_raise(&self, source: Source) -> bool {
        match source {
            Source::TimerZero => self.timer.may_raise(),
            Source::GamePad => self.pad.may_raise(),
            Source::Vip | Source::Communication | Source::GamePak => false,
        }
    }

    pub fn cart(&self) -> &Cart {
        &self.cart
    }

    pub fn cart_mut(&mut self) -> &mut Cart {
        &mut self.cart
    }

    pub fn wram(&self) -> &[u8] {
        &self.wram
    }

    pub fn wram_mut(&mut self) -> &mut [u8] {
        &mut self.wram
    }

    pub fn read_u8(&self, addr: u32) -> u8 {
        let addr = addr & ADDRESS_MASK;
        match Region::of(addr) {
            Region::Wram => self.wram[(addr & WRAM_MASK) as usize],
            Region::CartRam => self.cart.read_sram(addr),
            Region::CartRom => self.cart.read_rom(addr),
            Region::Misc => self.read_misc(addr),
            Region::Vip => self.vip.read_u8(addr),
            Region::Vsu | Region::Unmapped | Region::Expansion => 0,
        }
    }

    fn read_misc(&self, addr: u32) -> u8 {
        if Timer::handles(addr) {
            self.timer.read(addr)
        } else if WaitController::handles(addr) {
            self.wait.read()
        } else if GamePad::handles(addr) {
            self.pad.read(addr)
        } else {
            0
        }
    }

    fn write_misc(&mut self, addr: u32, value: u8) {
        if Timer::handles(addr) {
            self.timer.write(addr, value);
        } else if WaitController::handles(addr) {
            self.wait.write(value);
        } else if GamePad::handles(addr) {
            self.pad.write(addr, value);
        }
    }

    // devices need whole-width access, vip byte writes are anomalous
    pub fn read_u16(&self, addr: u32) -> u16 {
        let addr = (addr & ADDRESS_MASK) & !1;
        if Region::of(addr) == Region::Vip {
            return self.vip.read_u16(addr);
        }
        if !Region::of(addr).is_memory() {
            return 0;
        }
        u16::from_le_bytes([self.read_u8(addr), self.read_u8(addr.wrapping_add(1))])
    }

    pub fn read_u32(&self, addr: u32) -> u32 {
        let addr = (addr & ADDRESS_MASK) & !3;
        if Region::of(addr) == Region::Vip {
            return self.vip.read_u32(addr);
        }
        if !Region::of(addr).is_memory() {
            return 0;
        }
        u32::from_le_bytes([
            self.read_u8(addr),
            self.read_u8(addr.wrapping_add(1)),
            self.read_u8(addr.wrapping_add(2)),
            self.read_u8(addr.wrapping_add(3)),
        ])
    }

    pub fn write_u8(&mut self, addr: u32, value: u8) {
        let addr = addr & ADDRESS_MASK;
        match Region::of(addr) {
            Region::Wram => self.wram[(addr & WRAM_MASK) as usize] = value,
            Region::CartRam => self.cart.write_sram(addr, value),
            Region::Misc => self.write_misc(addr, value),
            Region::Vip => self.vip.write_u8(addr, u32::from(value)),
            Region::CartRom | Region::Vsu | Region::Unmapped | Region::Expansion => {}
        }
    }

    pub fn store_u8(&mut self, addr: u32, source: u32) {
        let addr = addr & ADDRESS_MASK;
        if Region::of(addr) == Region::Vip {
            self.vip.write_u8(addr, source);
        } else {
            self.write_u8(addr, (source & 0xFF) as u8);
        }
    }

    pub fn write_u16(&mut self, addr: u32, value: u16) {
        let addr = (addr & ADDRESS_MASK) & !1;
        if Region::of(addr) == Region::Vip {
            self.vip.write_u16(addr, value);
            return;
        }
        if !Region::of(addr).is_memory() {
            return;
        }
        let bytes = value.to_le_bytes();
        self.write_u8(addr, bytes[0]);
        self.write_u8(addr.wrapping_add(1), bytes[1]);
    }

    pub fn write_u32(&mut self, addr: u32, value: u32) {
        let addr = (addr & ADDRESS_MASK) & !3;
        if Region::of(addr) == Region::Vip {
            self.vip.write_u32(addr, value);
            return;
        }
        if !Region::of(addr).is_memory() {
            return;
        }
        let bytes = value.to_le_bytes();
        self.write_u8(addr, bytes[0]);
        self.write_u8(addr.wrapping_add(1), bytes[1]);
        self.write_u8(addr.wrapping_add(2), bytes[2]);
        self.write_u8(addr.wrapping_add(3), bytes[3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::{HEADER_OFFSET_FROM_END, tests::test_rom};

    fn bus_with_rom(len: usize) -> Bus {
        Bus::new(Cart::new(test_rom(len)).unwrap())
    }

    #[test]
    fn classifies_regions() {
        assert_eq!(Region::of(0x0000_0000), Region::Vip);
        assert_eq!(Region::of(0x0100_0000), Region::Vsu);
        assert_eq!(Region::of(0x0200_0000), Region::Misc);
        assert_eq!(Region::of(0x0300_0000), Region::Unmapped);
        assert_eq!(Region::of(0x0400_0000), Region::Expansion);
        assert_eq!(Region::of(0x0500_0000), Region::Wram);
        assert_eq!(Region::of(0x0600_0000), Region::CartRam);
        assert_eq!(Region::of(0x0700_0000), Region::CartRom);
        assert_eq!(Region::of(0x07FF_FFFF), Region::CartRom);
    }

    #[test]
    fn region_base_round_trips() {
        for region in [
            Region::Vip,
            Region::Vsu,
            Region::Misc,
            Region::Unmapped,
            Region::Expansion,
            Region::Wram,
            Region::CartRam,
            Region::CartRom,
        ] {
            assert_eq!(Region::of(region.base()), region);
            assert_eq!(Region::of(region.base() + REGION_LEN - 1), region);
        }
    }

    #[test]
    fn wram_round_trips() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u8(0x0500_0000, 0x11);
        bus.write_u16(0x0500_0100, 0x2233);
        bus.write_u32(0x0500_0200, 0x4455_6677);

        assert_eq!(bus.read_u8(0x0500_0000), 0x11);
        assert_eq!(bus.read_u16(0x0500_0100), 0x2233);
        assert_eq!(bus.read_u32(0x0500_0200), 0x4455_6677);
    }

    #[test]
    fn wram_is_little_endian() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u32(0x0500_0000, 0xAABB_CCDD);

        assert_eq!(bus.read_u8(0x0500_0000), 0xDD);
        assert_eq!(bus.read_u8(0x0500_0001), 0xCC);
        assert_eq!(bus.read_u8(0x0500_0002), 0xBB);
        assert_eq!(bus.read_u8(0x0500_0003), 0xAA);
    }

    #[test]
    fn wram_mirrors_every_64k() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u8(0x0500_0042, 0x7E);

        assert_eq!(bus.read_u8(0x0501_0042), 0x7E);
        assert_eq!(bus.read_u8(0x05FF_0042), 0x7E);

        bus.write_u8(0x05AB_0042, 0x3C);
        assert_eq!(bus.read_u8(0x0500_0042), 0x3C);
    }

    #[test]
    fn unaligned_reads_round_down() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u32(0x0500_0000, 0xAABB_CCDD);

        assert_eq!(bus.read_u16(0x0500_0001), bus.read_u16(0x0500_0000));
        assert_eq!(bus.read_u32(0x0500_0001), bus.read_u32(0x0500_0000));
        assert_eq!(bus.read_u32(0x0500_0002), bus.read_u32(0x0500_0000));
        assert_eq!(bus.read_u32(0x0500_0003), bus.read_u32(0x0500_0000));
    }

    #[test]
    fn unaligned_writes_round_down() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u16(0x0500_0003, 0x1234);
        assert_eq!(bus.read_u16(0x0500_0002), 0x1234);

        bus.write_u32(0x0500_0007, 0x89AB_CDEF);
        assert_eq!(bus.read_u32(0x0500_0004), 0x89AB_CDEF);
    }

    #[test]
    fn rom_is_readable_and_write_protected() {
        let mut rom = test_rom(0x1000);
        rom[0x0040] = 0x99;
        let mut bus = Bus::new(Cart::new(rom).unwrap());

        assert_eq!(bus.read_u8(0x0700_0040), 0x99);
        bus.write_u8(0x0700_0040, 0x00);
        assert_eq!(bus.read_u8(0x0700_0040), 0x99);
    }

    #[test]
    fn reset_vector_resolves_to_rom_tail() {
        let len = 0x1000;
        let mut rom = test_rom(len);
        let reset = len - 0x10;
        rom[reset..reset + 4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let bus = Bus::new(Cart::new(rom).unwrap());

        assert_eq!(bus.read_u32(crate::RESET_PC), 0xDEAD_BEEF);
    }

    #[test]
    fn header_is_reachable_through_the_bus() {
        let len = 0x1000;
        let bus = bus_with_rom(len);
        let header_addr = 0xFFFF_FDE0u32;

        assert_eq!(bus.read_u8(header_addr), b'V');
        assert_eq!(
            bus.cart().rom()[len - HEADER_OFFSET_FROM_END],
            bus.read_u8(header_addr)
        );
    }

    #[test]
    fn cart_ram_round_trips_through_the_bus() {
        let mut bus = Bus::new(Cart::with_sram(test_rom(0x1000), 0x2000).unwrap());
        bus.write_u16(0x0600_0004, 0xBEEF);

        assert_eq!(bus.read_u16(0x0600_0004), 0xBEEF);
        assert_eq!(bus.read_u16(0x0600_2004), 0xBEEF);
    }

    #[test]
    fn unimplemented_regions_read_zero_and_swallow_writes() {
        let mut bus = bus_with_rom(0x1000);
        for base in [0x0100_0000u32, 0x0300_0000, 0x0400_0000] {
            bus.write_u32(base, 0xFFFF_FFFF);
            assert_eq!(bus.read_u32(base), 0, "region at {base:#010X}");
        }
    }

    #[test]
    fn device_regions_are_never_accessed_as_byte_pairs() {
        let mut bus = bus_with_rom(0x1000);
        for base in [0x0100_0000u32, 0x0200_0000, 0x0400_0000] {
            assert!(!Region::of(base).is_memory());
            bus.write_u16(base, 0xFFFF);
            bus.write_u32(base, 0xFFFF_FFFF);
            assert_eq!(bus.read_u16(base), 0);
            assert_eq!(bus.read_u32(base), 0);
        }
        assert!(Region::of(0x0500_0000).is_memory());
        assert!(Region::of(0x0600_0000).is_memory());
        assert!(Region::of(0x0700_0000).is_memory());
    }

    #[test]
    fn the_vip_is_reachable_at_every_width() {
        let mut bus = bus_with_rom(0x1000);

        bus.write_u16(0x0002_0000, 0x1234);
        bus.write_u32(0x0002_0004, 0xDEAD_BEEF);

        assert_eq!(bus.read_u16(0x0002_0000), 0x1234);
        assert_eq!(bus.read_u32(0x0002_0004), 0xDEAD_BEEF);
        assert_eq!(bus.read_u8(0x0002_0000), 0x34);
    }

    #[test]
    fn a_byte_store_to_a_vip_register_carries_the_whole_source() {
        let mut bus = bus_with_rom(0x1000);

        bus.store_u8(crate::vip::registers::BRTA, 0x0000_ABCD);
        assert_eq!(bus.read_u16(crate::vip::registers::BRTA), 0x00CD);

        bus.store_u8(crate::vip::registers::SPT0 + 1, 0x0000_0003);
        assert_eq!(bus.read_u16(crate::vip::registers::SPT0), 0x0300);
    }

    #[test]
    fn misc_devices_share_the_region_without_colliding() {
        let mut bus = bus_with_rom(0x1000);

        bus.write_u8(crate::timer::TLR, 0x42);
        bus.write_u8(crate::wait::WCR, crate::wait::WCR_ROM1W);

        assert_eq!(bus.read_u8(crate::timer::TLR), 0x42);
        assert_eq!(bus.wait().rom_waits(), 1);
        assert_eq!(bus.timer().counter(), 0x42);
    }

    #[test]
    fn unclaimed_misc_registers_still_read_zero() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u8(0x0200_0000, 0xFF);
        assert_eq!(bus.read_u8(0x0200_0000), 0);
    }

    #[test]
    fn addresses_above_27_bits_wrap() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u8(0x0500_0055, 0x6D);

        assert_eq!(bus.read_u8(0x0D00_0055), 0x6D);
        assert_eq!(bus.read_u8(0xFD00_0055), 0x6D);
        assert_eq!(Region::of(0x0800_0000 & ADDRESS_MASK), Region::Vip);
    }

    #[test]
    fn reads_spanning_a_mirror_boundary_wrap_within_the_region() {
        let mut bus = bus_with_rom(0x1000);
        bus.write_u8(0x0500_FFFF, 0x12);
        bus.write_u8(0x0500_0000, 0x34);

        assert_eq!(bus.read_u16(0x0500_FFFE), u16::from_le_bytes([0x00, 0x12]));
        assert_eq!(bus.read_u8(0x0501_0000), 0x34);
    }
}
