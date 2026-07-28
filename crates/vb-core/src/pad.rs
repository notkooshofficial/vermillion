pub const SDLR: u32 = 0x0200_0010;
pub const SDHR: u32 = 0x0200_0014;
pub const SCR: u32 = 0x0200_0028;

pub const SCR_S_ABT_DIS: u8 = 1 << 0;
pub const SCR_SI_STAT: u8 = 1 << 1;
pub const SCR_HW_SI: u8 = 1 << 2;
pub const SCR_SOFT_CK: u8 = 1 << 4;
pub const SCR_PARA_SI: u8 = 1 << 5;
pub const SCR_K_INT_INH: u8 = 1 << 7;

pub const PWR: u16 = 1 << 0;
pub const SGN: u16 = 1 << 1;
pub const A: u16 = 1 << 2;
pub const B: u16 = 1 << 3;
pub const RT: u16 = 1 << 4;
pub const LT: u16 = 1 << 5;
pub const RU: u16 = 1 << 6;
pub const RR: u16 = 1 << 7;
pub const LR: u16 = 1 << 8;
pub const LL: u16 = 1 << 9;
pub const LD: u16 = 1 << 10;
pub const LU: u16 = 1 << 11;
pub const STA: u16 = 1 << 12;
pub const SEL: u16 = 1 << 13;
pub const RL: u16 = 1 << 14;
pub const RD: u16 = 1 << 15;

// 512 us at 20 mhz
pub const HARDWARE_READ_CYCLES: u64 = 10_240;

const BUTTON_COUNT: u32 = 16;
const REGISTER_MASK: u32 = 0x3F;
const SCR_UNUSED: u8 = 0b0100_1000;
const KEY_TRIGGER: u16 = 0xFFF0;
const KEY_BLOCK: u16 = 0x000E;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamePad {
    buttons: u16,
    data: u16,
    shift: u16,
    shifted: u32,
    clock: bool,
    latched: bool,
    abort: bool,
    reading: bool,
    elapsed: u64,
    interrupt_inhibited: bool,
    pending: bool,
}

impl Default for GamePad {
    fn default() -> Self {
        Self::new()
    }
}

impl GamePad {
    pub fn new() -> Self {
        Self {
            buttons: SGN,
            data: 0,
            shift: 0,
            shifted: 0,
            clock: false,
            latched: false,
            abort: false,
            reading: false,
            elapsed: 0,
            interrupt_inhibited: false,
            pending: false,
        }
    }

    pub fn handles(addr: u32) -> bool {
        matches!(addr & REGISTER_MASK, 0x10 | 0x14 | 0x28)
    }

    // a standard pad always reports sgn, which is why the key interrupt can never fire with one
    pub fn set_buttons(&mut self, buttons: u16) {
        self.buttons = buttons;
    }

    pub fn buttons(&self) -> u16 {
        self.buttons
    }

    pub fn data(&self) -> u16 {
        self.data
    }

    pub fn reading(&self) -> bool {
        self.reading
    }

    pub fn interrupt_pending(&self) -> bool {
        self.pending
    }

    pub fn may_raise(&self) -> bool {
        self.pending || (self.reading && !self.interrupt_inhibited)
    }

    pub fn tick(&mut self, cycles: u64) {
        if !self.reading {
            return;
        }

        self.elapsed += cycles;
        if self.elapsed < HARDWARE_READ_CYCLES {
            return;
        }

        self.reading = false;
        self.elapsed = 0;
        self.shift = self.buttons;
        self.shifted = BUTTON_COUNT;
        self.data = self.buttons;
        self.raise_key_interrupt();
    }

    pub fn read(&self, addr: u32) -> u8 {
        match addr & REGISTER_MASK {
            0x10 => self.data.to_le_bytes()[0],
            0x14 => self.data.to_le_bytes()[1],
            0x28 => self.control(),
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        if addr & REGISTER_MASK == 0x28 {
            self.write_control(value);
        }
    }

    fn control(&self) -> u8 {
        let mut value = SCR_UNUSED | SCR_HW_SI;
        if self.interrupt_inhibited {
            value |= SCR_K_INT_INH;
        }
        if self.latched {
            value |= SCR_PARA_SI;
        }
        if self.clock {
            value |= SCR_SOFT_CK;
        }
        if self.reading {
            value |= SCR_SI_STAT;
        }
        if self.abort {
            value |= SCR_S_ABT_DIS;
        }
        value
    }

    fn write_control(&mut self, value: u8) {
        self.interrupt_inhibited = value & SCR_K_INT_INH != 0;
        if self.interrupt_inhibited {
            self.pending = false;
        }

        let latched = value & SCR_PARA_SI != 0;
        if latched {
            self.shift = 0;
            self.shifted = 0;
        }
        self.latched = latched;

        let clock = value & SCR_SOFT_CK != 0;
        // the pad sees the inverse of what is written, so clearing soft-ck clocks a bit
        if !latched && self.clock && !clock {
            self.clock_bit();
        }
        self.clock = clock;

        if value & SCR_HW_SI != 0 && !self.reading {
            self.reading = true;
            self.elapsed = 0;
        }

        self.abort = value & SCR_S_ABT_DIS != 0;
        if self.abort {
            self.reading = false;
            self.elapsed = 0;
        }
    }

    fn clock_bit(&mut self) {
        if self.shifted >= BUTTON_COUNT {
            return;
        }

        self.shift |= self.buttons & (1 << self.shifted);
        self.shifted += 1;

        if self.shifted == BUTTON_COUNT {
            self.data = self.shift;
        }
    }

    fn raise_key_interrupt(&mut self) {
        if self.interrupt_inhibited {
            return;
        }
        if self.data & KEY_TRIGGER != 0 && self.data & KEY_BLOCK == 0 {
            self.pending = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn software_read(pad: &mut GamePad) {
        pad.write(SCR, SCR_PARA_SI);
        pad.write(SCR, 0);
        for _ in 0..BUTTON_COUNT {
            pad.write(SCR, SCR_SOFT_CK);
            pad.write(SCR, 0);
        }
    }

    #[test]
    fn a_fresh_pad_reports_the_signature_only() {
        let pad = GamePad::new();
        assert_eq!(pad.buttons(), SGN);
        assert!(!pad.interrupt_pending());
    }

    #[test]
    fn a_software_read_shifts_the_whole_word_in() {
        let mut pad = GamePad::new();
        pad.set_buttons(SGN | A | STA | RD);
        software_read(&mut pad);

        assert_eq!(pad.data(), SGN | A | STA | RD);
        assert_eq!(pad.read(SDLR), (SGN | A).to_le_bytes()[0]);
        assert_eq!(pad.read(SDHR), (STA | RD).to_le_bytes()[1]);
    }

    #[test]
    fn the_data_registers_are_incomplete_until_the_sixteenth_bit() {
        let mut pad = GamePad::new();
        pad.set_buttons(RD);

        pad.write(SCR, SCR_PARA_SI);
        pad.write(SCR, 0);
        for _ in 0..BUTTON_COUNT - 1 {
            pad.write(SCR, SCR_SOFT_CK);
            pad.write(SCR, 0);
        }
        assert_eq!(pad.data(), 0, "rd has not been clocked in yet");

        pad.write(SCR, SCR_SOFT_CK);
        pad.write(SCR, 0);
        assert_eq!(pad.data(), RD);
    }

    #[test]
    fn a_held_latch_keeps_resetting_the_read() {
        let mut pad = GamePad::new();
        pad.set_buttons(SGN | A);

        for _ in 0..BUTTON_COUNT {
            pad.write(SCR, SCR_PARA_SI | SCR_SOFT_CK);
            pad.write(SCR, SCR_PARA_SI);
        }
        assert_eq!(pad.data(), 0, "nothing clocks while para/si is held");
    }

    #[test]
    fn a_hardware_read_takes_512_microseconds() {
        let mut pad = GamePad::new();
        pad.set_buttons(SGN | B);
        pad.write(SCR, SCR_HW_SI);

        assert!(pad.reading());
        assert_ne!(pad.read(SCR) & SCR_SI_STAT, 0);

        pad.tick(HARDWARE_READ_CYCLES - 1);
        assert!(pad.reading());
        assert_eq!(pad.data(), 0);

        pad.tick(1);
        assert!(!pad.reading());
        assert_eq!(pad.read(SCR) & SCR_SI_STAT, 0);
        assert_eq!(pad.data(), SGN | B);
    }

    #[test]
    fn aborting_cancels_a_hardware_read() {
        let mut pad = GamePad::new();
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES / 2);

        pad.write(SCR, SCR_S_ABT_DIS);
        assert!(!pad.reading());

        pad.tick(HARDWARE_READ_CYCLES);
        assert_eq!(pad.data(), 0, "the cancelled read never landed");
    }

    #[test]
    fn starting_a_read_that_is_already_running_does_nothing() {
        let mut pad = GamePad::new();
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES - 1);

        pad.write(SCR, SCR_HW_SI);
        pad.tick(1);
        assert!(
            !pad.reading(),
            "the second request did not restart the clock"
        );
    }

    #[test]
    fn hw_si_always_reads_set_and_unused_bits_too() {
        let pad = GamePad::new();
        assert_ne!(pad.read(SCR) & SCR_HW_SI, 0);
        assert_eq!(pad.read(SCR) & SCR_UNUSED, SCR_UNUSED);
    }

    #[test]
    fn a_standard_pad_can_never_raise_the_key_interrupt() {
        let mut pad = GamePad::new();
        pad.set_buttons(SGN | RD | STA);
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES);

        assert_eq!(pad.data(), SGN | RD | STA);
        assert!(!pad.interrupt_pending(), "sgn blocks it");
    }

    #[test]
    fn without_the_signature_a_high_button_does_raise() {
        let mut pad = GamePad::new();
        pad.set_buttons(RD);
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES);

        assert!(pad.interrupt_pending());
    }

    #[test]
    fn a_low_button_alone_never_raises() {
        let mut pad = GamePad::new();
        pad.set_buttons(A);
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES);

        assert!(!pad.interrupt_pending(), "nothing in 15 through 4 is set");
    }

    #[test]
    fn inhibiting_the_interrupt_acknowledges_it() {
        let mut pad = GamePad::new();
        pad.set_buttons(RD);
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES);
        assert!(pad.interrupt_pending());

        pad.write(SCR, SCR_K_INT_INH);
        assert!(!pad.interrupt_pending());
        assert_ne!(pad.read(SCR) & SCR_K_INT_INH, 0);
    }

    #[test]
    fn an_inhibited_read_never_raises_in_the_first_place() {
        let mut pad = GamePad::new();
        pad.set_buttons(RD);
        pad.write(SCR, SCR_K_INT_INH | SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES);

        assert_eq!(pad.data(), RD);
        assert!(!pad.interrupt_pending());
    }

    #[test]
    fn the_data_registers_ignore_writes() {
        let mut pad = GamePad::new();
        pad.set_buttons(RD);
        pad.write(SCR, SCR_HW_SI);
        pad.tick(HARDWARE_READ_CYCLES);

        pad.write(SDLR, 0xFF);
        pad.write(SDHR, 0xFF);
        assert_eq!(pad.data(), RD);
    }

    #[test]
    fn only_the_pad_registers_are_claimed() {
        for owned in [SDLR, SDHR, SCR] {
            assert!(GamePad::handles(owned), "{owned:#010X}");
        }
        for other in [0x0200_0018u32, 0x0200_0020, 0x0200_0024] {
            assert!(!GamePad::handles(other), "{other:#010X}");
        }
    }
}
