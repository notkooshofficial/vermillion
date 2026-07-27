pub const WCR: u32 = 0x0200_0024;

pub const WCR_ROM1W: u8 = 1 << 0;
pub const WCR_EXP1W: u8 = 1 << 1;

const REGISTER_MASK: u32 = 0x3F;
const WCR_OFFSET: u32 = 0x24;
const WCR_UNUSED: u8 = 0xFC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitController {
    rom_fast: bool,
    expansion_fast: bool,
}

impl Default for WaitController {
    fn default() -> Self {
        Self::new()
    }
}

impl WaitController {
    pub fn new() -> Self {
        Self {
            rom_fast: false,
            expansion_fast: false,
        }
    }

    pub fn handles(addr: u32) -> bool {
        addr & REGISTER_MASK == WCR_OFFSET
    }

    // clear is two waits, set is one
    pub fn rom_waits(&self) -> u32 {
        if self.rom_fast { 1 } else { 2 }
    }

    pub fn expansion_waits(&self) -> u32 {
        if self.expansion_fast { 1 } else { 2 }
    }

    pub fn read(&self) -> u8 {
        let mut value = WCR_UNUSED;
        if self.rom_fast {
            value |= WCR_ROM1W;
        }
        if self.expansion_fast {
            value |= WCR_EXP1W;
        }
        value
    }

    pub fn write(&mut self, value: u8) {
        self.rom_fast = value & WCR_ROM1W != 0;
        self.expansion_fast = value & WCR_EXP1W != 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_asks_for_the_slower_timing() {
        let wait = WaitController::new();
        assert_eq!(wait.rom_waits(), 2);
        assert_eq!(wait.expansion_waits(), 2);
    }

    #[test]
    fn a_set_bit_means_one_wait_not_two() {
        let mut wait = WaitController::new();
        wait.write(WCR_ROM1W);

        assert_eq!(wait.rom_waits(), 1);
        assert_eq!(wait.expansion_waits(), 2, "the two are independent");

        wait.write(WCR_ROM1W | WCR_EXP1W);
        assert_eq!(wait.expansion_waits(), 1);
    }

    #[test]
    fn unused_bits_read_as_set() {
        let wait = WaitController::new();
        assert_eq!(wait.read(), WCR_UNUSED);

        let mut wait = WaitController::new();
        wait.write(0xFF);
        assert_eq!(wait.read(), 0xFF);
    }

    #[test]
    fn only_the_wait_register_is_claimed() {
        assert!(WaitController::handles(WCR));
        for other in [0x0200_0018u32, 0x0200_001C, 0x0200_0020, 0x0200_0028] {
            assert!(!WaitController::handles(other), "{other:#010X}");
        }
    }
}
