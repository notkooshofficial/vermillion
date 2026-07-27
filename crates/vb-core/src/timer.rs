pub const TLR: u32 = 0x0200_0018;
pub const THR: u32 = 0x0200_001C;
pub const TCR: u32 = 0x0200_0020;

pub const TCR_T_ENB: u8 = 1 << 0;
pub const TCR_Z_STAT: u8 = 1 << 1;
pub const TCR_Z_STAT_CLR: u8 = 1 << 2;
pub const TCR_TIM_Z_INT: u8 = 1 << 3;
pub const TCR_T_CLK_SEL: u8 = 1 << 4;

pub const INT_TIMER_ZERO: u16 = 0xFE10;

// 20 mhz, so the 20 us tick is 400 cycles
pub const TICK_CYCLES: u64 = 400;

const TICK_MODULO: u8 = 5;
const REGISTER_MASK: u32 = 0x3F;
const TCR_UNUSED: u8 = 0xE0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    counter: u16,
    reload: u16,
    tick: u8,
    elapsed: u64,
    enabled: bool,
    fast: bool,
    interrupt_enabled: bool,
    zero_status: bool,
    pending: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counter: 0,
            reload: 0,
            tick: 0,
            elapsed: 0,
            enabled: false,
            fast: false,
            interrupt_enabled: false,
            zero_status: false,
            pending: false,
        }
    }

    pub fn handles(addr: u32) -> bool {
        matches!(addr & REGISTER_MASK, 0x18 | 0x1C | 0x20)
    }

    pub fn counter(&self) -> u16 {
        self.counter
    }

    pub fn reload(&self) -> u16 {
        self.reload
    }

    // unfinished but pushed anyway, interrupt acceptance is what connects this
    pub fn interrupt_pending(&self) -> bool {
        self.pending
    }

    // the tick counter runs even while the timer is disabled
    pub fn tick(&mut self, cycles: u64) {
        self.elapsed += cycles;
        while self.elapsed >= TICK_CYCLES {
            self.elapsed -= TICK_CYCLES;
            self.tick = (self.tick + 1) % TICK_MODULO;
            if self.enabled && (self.fast || self.tick == 0) {
                self.count_down();
            }
        }
    }

    pub fn read(&self, addr: u32) -> u8 {
        match addr & REGISTER_MASK {
            0x18 => self.counter.to_le_bytes()[0],
            0x1C => self.counter.to_le_bytes()[1],
            0x20 => self.control(),
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        let [low, high] = self.reload.to_le_bytes();
        match addr & REGISTER_MASK {
            0x18 => self.load(u16::from_le_bytes([value, high])),
            0x1C => self.load(u16::from_le_bytes([low, value])),
            0x20 => self.write_control(value),
            _ => {}
        }
    }

    fn control(&self) -> u8 {
        let mut value = TCR_UNUSED | TCR_Z_STAT_CLR;
        if self.enabled {
            value |= TCR_T_ENB;
        }
        if self.zero_status {
            value |= TCR_Z_STAT;
        }
        if self.interrupt_enabled {
            value |= TCR_TIM_Z_INT;
        }
        if self.fast {
            value |= TCR_T_CLK_SEL;
        }
        value
    }

    fn load(&mut self, reload: u16) {
        let was_running = self.counter != 0;
        self.reload = reload;
        self.counter = reload;
        self.tick = 0;
        self.elapsed = 0;

        if was_running && self.counter == 0 {
            self.reach_zero();
        }
        self.refresh_zero_status();
    }

    fn write_control(&mut self, value: u8) {
        let was_enabled = self.enabled;
        let was_fast = self.fast;

        self.enabled = value & TCR_T_ENB != 0;
        self.interrupt_enabled = value & TCR_TIM_Z_INT != 0;
        self.fast = value & TCR_T_CLK_SEL != 0;

        if !self.interrupt_enabled {
            self.pending = false;
        }

        if value & TCR_Z_STAT_CLR != 0 {
            // disabling and clearing in the same write disables without clearing
            let disabling = was_enabled && !self.enabled;
            if !disabling && (self.counter != 0 || !self.enabled) {
                self.zero_status = false;
                self.pending = false;
            }
        }

        // switching to the fast clock mid interval spends the pending tick straight away
        if !was_fast && self.fast && self.tick != 0 && self.enabled {
            self.count_down();
        }

        self.refresh_zero_status();
    }

    // at zero the next interval reloads instead of decrementing, so a reload of zero never raises
    fn count_down(&mut self) {
        if self.counter == 0 {
            self.counter = self.reload;
            return;
        }

        self.counter -= 1;
        if self.counter == 0 {
            self.reach_zero();
        }
    }

    fn reach_zero(&mut self) {
        if self.enabled {
            self.zero_status = true;
        }
        if self.interrupt_enabled {
            self.pending = true;
        }
    }

    fn refresh_zero_status(&mut self) {
        if self.enabled && self.counter == 0 {
            self.zero_status = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(counter: u16, control: u8) -> Timer {
        let mut timer = Timer::new();
        timer.write(TLR, counter.to_le_bytes()[0]);
        timer.write(THR, counter.to_le_bytes()[1]);
        timer.write(TCR, control);
        timer
    }

    #[test]
    fn the_fast_clock_counts_every_tick() {
        let mut timer = started(10, TCR_T_ENB | TCR_T_CLK_SEL);
        timer.tick(TICK_CYCLES);
        assert_eq!(timer.counter(), 9);
        timer.tick(TICK_CYCLES * 4);
        assert_eq!(timer.counter(), 5);
    }

    #[test]
    fn the_slow_clock_counts_once_every_five_ticks() {
        let mut timer = started(10, TCR_T_ENB);
        timer.tick(TICK_CYCLES * 4);
        assert_eq!(timer.counter(), 10);
        timer.tick(TICK_CYCLES);
        assert_eq!(timer.counter(), 9);
    }

    #[test]
    fn a_disabled_timer_holds_its_count() {
        let mut timer = started(10, 0);
        timer.tick(TICK_CYCLES * 20);
        assert_eq!(timer.counter(), 10);
    }

    #[test]
    fn reaching_zero_sets_status_and_raises() {
        let mut timer = started(2, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        timer.tick(TICK_CYCLES);
        assert!(!timer.interrupt_pending());

        timer.tick(TICK_CYCLES);
        assert_eq!(timer.counter(), 0);
        assert!(timer.interrupt_pending());
        assert_ne!(timer.read(TCR) & TCR_Z_STAT, 0);
    }

    #[test]
    fn the_interrupt_stays_quiet_when_it_is_not_enabled() {
        let mut timer = started(1, TCR_T_ENB | TCR_T_CLK_SEL);
        timer.tick(TICK_CYCLES);
        assert_eq!(timer.counter(), 0);
        assert!(!timer.interrupt_pending());
        assert_ne!(
            timer.read(TCR) & TCR_Z_STAT,
            0,
            "status is not the interrupt"
        );
    }

    #[test]
    fn the_counter_reloads_on_the_interval_after_zero() {
        let mut timer = started(2, TCR_T_ENB | TCR_T_CLK_SEL);
        timer.tick(TICK_CYCLES * 2);
        assert_eq!(timer.counter(), 0);

        timer.tick(TICK_CYCLES);
        assert_eq!(
            timer.counter(),
            2,
            "reload happens after zero is observable"
        );
    }

    #[test]
    fn a_reload_of_zero_never_raises() {
        let mut timer = started(0, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        assert!(!timer.interrupt_pending());

        timer.tick(TICK_CYCLES * 8);
        assert_eq!(timer.counter(), 0);
        assert!(!timer.interrupt_pending());
    }

    #[test]
    fn writing_a_reload_that_lands_on_zero_does_raise() {
        let mut timer = started(5, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        assert!(!timer.interrupt_pending());

        timer.write(TLR, 0);
        timer.write(THR, 0);
        assert!(timer.interrupt_pending());
    }

    #[test]
    fn writing_either_half_loads_the_whole_counter() {
        let mut timer = Timer::new();
        timer.write(TLR, 0x34);
        timer.write(THR, 0x12);

        assert_eq!(timer.reload(), 0x1234);
        assert_eq!(timer.counter(), 0x1234);
        assert_eq!(timer.read(TLR), 0x34);
        assert_eq!(timer.read(THR), 0x12);
    }

    #[test]
    fn a_write_restarts_the_interval() {
        let mut timer = started(10, TCR_T_ENB | TCR_T_CLK_SEL);
        timer.tick(TICK_CYCLES - 1);

        timer.write(TLR, 10);
        timer.tick(1);
        assert_eq!(timer.counter(), 10, "the interval restarted from the write");
    }

    #[test]
    fn switching_to_the_fast_clock_mid_interval_spends_the_tick() {
        let mut timer = started(10, TCR_T_ENB);
        timer.tick(TICK_CYCLES);
        assert_eq!(timer.counter(), 10);

        timer.write(TCR, TCR_T_ENB | TCR_T_CLK_SEL);
        assert_eq!(timer.counter(), 9);
    }

    #[test]
    fn switching_clocks_on_a_tick_boundary_costs_nothing() {
        let mut timer = started(10, TCR_T_ENB);
        timer.write(TCR, TCR_T_ENB | TCR_T_CLK_SEL);
        assert_eq!(timer.counter(), 10);
    }

    #[test]
    fn clearing_zero_status_acknowledges_the_interrupt() {
        let mut timer = started(1, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        timer.tick(TICK_CYCLES * 2);
        assert!(timer.interrupt_pending());
        assert_ne!(timer.counter(), 0, "reloaded, so the clear is allowed");

        timer.write(
            TCR,
            TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT | TCR_Z_STAT_CLR,
        );
        assert!(!timer.interrupt_pending());
        assert_eq!(timer.read(TCR) & TCR_Z_STAT, 0);
    }

    #[test]
    fn zero_status_cannot_be_cleared_while_it_is_still_zero_and_running() {
        let mut timer = started(1, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        timer.tick(TICK_CYCLES);
        assert_eq!(timer.counter(), 0);

        timer.write(
            TCR,
            TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT | TCR_Z_STAT_CLR,
        );
        assert_ne!(timer.read(TCR) & TCR_Z_STAT, 0);
        assert!(timer.interrupt_pending());
    }

    #[test]
    fn disabling_and_clearing_together_disables_without_clearing() {
        let mut timer = started(1, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        timer.tick(TICK_CYCLES);
        assert_ne!(timer.read(TCR) & TCR_Z_STAT, 0);

        timer.write(TCR, TCR_TIM_Z_INT | TCR_Z_STAT_CLR);

        assert_eq!(timer.read(TCR) & TCR_T_ENB, 0, "the timer did stop");
        assert_ne!(timer.read(TCR) & TCR_Z_STAT, 0, "but status survived");
        assert!(timer.interrupt_pending());
    }

    #[test]
    fn clearing_the_interrupt_enable_acknowledges_too() {
        let mut timer = started(1, TCR_T_ENB | TCR_T_CLK_SEL | TCR_TIM_Z_INT);
        timer.tick(TICK_CYCLES);
        assert!(timer.interrupt_pending());

        timer.write(TCR, TCR_T_ENB | TCR_T_CLK_SEL);
        assert!(!timer.interrupt_pending());
    }

    #[test]
    fn unused_control_bits_read_as_set() {
        let timer = Timer::new();
        assert_eq!(timer.read(TCR) & TCR_UNUSED, TCR_UNUSED);
        assert_ne!(timer.read(TCR) & TCR_Z_STAT_CLR, 0);
    }

    #[test]
    fn a_partial_interval_carries_over_between_calls() {
        let mut timer = started(10, TCR_T_ENB | TCR_T_CLK_SEL);
        for _ in 0..TICK_CYCLES {
            timer.tick(1);
        }
        assert_eq!(timer.counter(), 9);
    }
}
