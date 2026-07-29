pub const INTPND: u32 = 0x0005_F800;
pub const INTENB: u32 = 0x0005_F802;
pub const INTCLR: u32 = 0x0005_F804;
pub const DPSTTS: u32 = 0x0005_F820;
pub const DPCTRL: u32 = 0x0005_F822;
pub const BRTA: u32 = 0x0005_F824;
pub const BRTB: u32 = 0x0005_F826;
pub const BRTC: u32 = 0x0005_F828;
pub const REST: u32 = 0x0005_F82A;
pub const FRMCYC: u32 = 0x0005_F82E;
pub const CTA: u32 = 0x0005_F830;
pub const XPSTTS: u32 = 0x0005_F840;
pub const XPCTRL: u32 = 0x0005_F842;
pub const VER: u32 = 0x0005_F844;
pub const SPT0: u32 = 0x0005_F848;
pub const GPLT0: u32 = 0x0005_F860;
pub const JPLT0: u32 = 0x0005_F868;
pub const BKCOL: u32 = 0x0005_F870;

pub const INT_TIMEERR: u16 = 1 << 15;
pub const INT_XPEND: u16 = 1 << 14;
pub const INT_SBHIT: u16 = 1 << 13;
pub const INT_FRAMESTART: u16 = 1 << 4;
pub const INT_GAMESTART: u16 = 1 << 3;
pub const INT_RFBEND: u16 = 1 << 2;
pub const INT_LFBEND: u16 = 1 << 1;
pub const INT_SCANERR: u16 = 1 << 0;

pub const INT_MASK: u16 = INT_TIMEERR
    | INT_XPEND
    | INT_SBHIT
    | INT_FRAMESTART
    | INT_GAMESTART
    | INT_RFBEND
    | INT_LFBEND
    | INT_SCANERR;

pub const DP_LOCK: u16 = 1 << 10;
pub const DP_SYNCE: u16 = 1 << 9;
pub const DP_RE: u16 = 1 << 8;
pub const DP_FCLK: u16 = 1 << 7;
pub const DP_SCANRDY: u16 = 1 << 6;
pub const DP_R1BSY: u16 = 1 << 5;
pub const DP_L1BSY: u16 = 1 << 4;
pub const DP_R0BSY: u16 = 1 << 3;
pub const DP_L0BSY: u16 = 1 << 2;
pub const DP_DISP: u16 = 1 << 1;
pub const DP_DPRST: u16 = 1 << 0;

pub const XP_SBOUT: u16 = 1 << 15;
pub const XP_OVERTIME: u16 = 1 << 4;
pub const XP_F1BSY: u16 = 1 << 3;
pub const XP_F0BSY: u16 = 1 << 2;
pub const XP_XPEN: u16 = 1 << 1;
pub const XP_XPRST: u16 = 1 << 0;

const SB_SHIFT: u32 = 8;
const SB_MASK: u16 = 0x1F;

const DPRST_CLEARS: u16 =
    INT_TIMEERR | INT_FRAMESTART | INT_GAMESTART | INT_RFBEND | INT_LFBEND | INT_SCANERR;
const XPRST_CLEARS: u16 = INT_TIMEERR | INT_XPEND | INT_SBHIT;

pub const VER_VALUE: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    pub pending: u16,
    pub enabled: u16,

    pub lock: bool,
    pub sync_enabled: bool,
    pub refresh: bool,
    pub display_enabled: bool,
    pub frame_clock: bool,
    pub scan_ready: bool,
    pub displaying: u16,

    pub draw_enabled: bool,
    pub overtime: bool,
    pub row_group_out: bool,
    pub row_group: u16,
    pub row_group_compare: u16,
    pub drawing: u16,

    pub brightness: [u16; 3],
    pub rest: u16,
    pub frame_cycle: u16,
    pub column_start: u16,
    pub object_end: [u16; 4],
    pub bg_palette: [u16; 4],
    pub obj_palette: [u16; 4],
    pub background: u16,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    pub fn new() -> Self {
        Self {
            pending: 0,
            enabled: 0,
            lock: false,
            sync_enabled: false,
            refresh: false,
            display_enabled: false,
            frame_clock: false,
            scan_ready: false,
            displaying: 0,
            draw_enabled: false,
            overtime: false,
            row_group_out: false,
            row_group: 0,
            row_group_compare: 0,
            drawing: 0,
            brightness: [0; 3],
            rest: 0,
            frame_cycle: 0,
            column_start: 0,
            object_end: [0; 4],
            bg_palette: [0; 4],
            obj_palette: [0; 4],
            background: 0,
        }
    }

    pub fn interrupt_pending(&self) -> bool {
        self.pending & self.enabled != 0
    }

    pub fn raise(&mut self, flag: u16) {
        self.pending |= flag & INT_MASK;
    }

    fn display_status(&self) -> u16 {
        let mut value = self.displaying & (DP_R1BSY | DP_L1BSY | DP_R0BSY | DP_L0BSY);
        if self.lock {
            value |= DP_LOCK;
        }
        if self.sync_enabled {
            value |= DP_SYNCE;
        }
        if self.refresh {
            value |= DP_RE;
        }
        if self.frame_clock {
            value |= DP_FCLK;
        }
        if self.scan_ready {
            value |= DP_SCANRDY;
        }
        if self.display_enabled {
            value |= DP_DISP;
        }
        value
    }

    fn draw_status(&self) -> u16 {
        let mut value = self.drawing & (XP_F1BSY | XP_F0BSY);
        value |= (self.row_group & SB_MASK) << SB_SHIFT;
        if self.row_group_out {
            value |= XP_SBOUT;
        }
        if self.overtime {
            value |= XP_OVERTIME;
        }
        if self.draw_enabled {
            value |= XP_XPEN;
        }
        value
    }

    // write-only registers read as zero rather than mirroring their read-side twin
    pub fn read(&self, addr: u32) -> u16 {
        match addr {
            INTPND => self.pending,
            INTENB => self.enabled,
            DPSTTS => self.display_status(),
            BRTA => self.brightness[0],
            BRTB => self.brightness[1],
            BRTC => self.brightness[2],
            REST => self.rest,
            FRMCYC => self.frame_cycle,
            CTA => self.column_start,
            XPSTTS => self.draw_status(),
            VER => VER_VALUE,
            SPT0..=0x0005_F84E if addr & 1 == 0 => self.object_end[index_of(addr, SPT0)],
            GPLT0..=0x0005_F866 if addr & 1 == 0 => self.bg_palette[index_of(addr, GPLT0)],
            JPLT0..=0x0005_F86E if addr & 1 == 0 => self.obj_palette[index_of(addr, JPLT0)],
            BKCOL => self.background,
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u32, value: u16) {
        match addr {
            INTENB => self.enabled = value & INT_MASK,
            INTCLR => self.pending &= !(value & INT_MASK),
            DPCTRL => self.write_display(value),
            BRTA => self.brightness[0] = value & 0xFF,
            BRTB => self.brightness[1] = value & 0xFF,
            BRTC => self.brightness[2] = value & 0xFF,
            REST => self.rest = value & 0xFF,
            FRMCYC => self.frame_cycle = value & 0xF,
            XPCTRL => self.write_draw(value),
            SPT0..=0x0005_F84E if addr & 1 == 0 => {
                self.object_end[index_of(addr, SPT0)] = value & 0x3FF;
            }
            GPLT0..=0x0005_F866 if addr & 1 == 0 => {
                self.bg_palette[index_of(addr, GPLT0)] = value & 0xFC;
            }
            JPLT0..=0x0005_F86E if addr & 1 == 0 => {
                self.obj_palette[index_of(addr, JPLT0)] = value & 0xFC;
            }
            BKCOL => self.background = value & 0x3,
            _ => {}
        }
    }

    fn write_display(&mut self, value: u16) {
        self.lock = value & DP_LOCK != 0;
        self.sync_enabled = value & DP_SYNCE != 0;
        self.refresh = value & DP_RE != 0;
        self.display_enabled = value & DP_DISP != 0;

        if value & DP_DPRST != 0 {
            self.pending &= !DPRST_CLEARS;
            self.enabled &= !DPRST_CLEARS;
        }
    }

    fn write_draw(&mut self, value: u16) {
        self.row_group_compare = (value >> SB_SHIFT) & SB_MASK;
        self.draw_enabled = value & XP_XPEN != 0;

        if value & XP_XPRST != 0 {
            self.draw_enabled = false;
            self.pending &= !XPRST_CLEARS;
            self.enabled &= !XPRST_CLEARS;
        }
    }
}

fn index_of(addr: u32, base: u32) -> usize {
    (((addr - base) / 2) & 3) as usize
}

// character pixel 0 is transparent, so a palette only describes values 1 through 3
pub fn shade(palette: u16, pixel: u8) -> u8 {
    match pixel & 3 {
        1 => ((palette >> 2) & 3) as u8,
        2 => ((palette >> 4) & 3) as u8,
        3 => ((palette >> 6) & 3) as u8,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_fixed_at_two() {
        assert_eq!(Registers::new().read(VER), VER_VALUE);
    }

    #[test]
    fn write_only_registers_read_as_zero() {
        let mut regs = Registers::new();
        regs.write(DPCTRL, DP_DISP | DP_SYNCE);
        regs.write(XPCTRL, XP_XPEN);

        assert_eq!(regs.read(DPCTRL), 0);
        assert_eq!(regs.read(XPCTRL), 0);
        assert_eq!(regs.read(INTCLR), 0);
    }

    #[test]
    fn display_control_reaches_the_status_register() {
        let mut regs = Registers::new();
        regs.write(DPCTRL, DP_DISP | DP_SYNCE | DP_LOCK | DP_RE);

        let status = regs.read(DPSTTS);
        assert_ne!(status & DP_DISP, 0);
        assert_ne!(status & DP_SYNCE, 0);
        assert_ne!(status & DP_LOCK, 0);
        assert_ne!(status & DP_RE, 0);
    }

    #[test]
    fn read_only_display_bits_ignore_writes() {
        let mut regs = Registers::new();
        regs.write(DPCTRL, DP_FCLK | DP_SCANRDY | DP_L0BSY);

        let status = regs.read(DPSTTS);
        assert_eq!(status & DP_FCLK, 0);
        assert_eq!(status & DP_SCANRDY, 0);
        assert_eq!(status & DP_L0BSY, 0);
    }

    #[test]
    fn interrupts_only_fire_when_pending_and_enabled_agree() {
        let mut regs = Registers::new();
        regs.raise(INT_XPEND);
        assert!(!regs.interrupt_pending(), "pending alone is not enough");

        regs.write(INTENB, INT_FRAMESTART);
        assert!(!regs.interrupt_pending(), "a different flag does not count");

        regs.write(INTENB, INT_XPEND);
        assert!(regs.interrupt_pending());
    }

    #[test]
    fn intclr_clears_only_the_written_flags() {
        let mut regs = Registers::new();
        regs.raise(INT_XPEND);
        regs.raise(INT_FRAMESTART);

        regs.write(INTCLR, INT_XPEND);

        assert_eq!(regs.read(INTPND) & INT_XPEND, 0);
        assert_ne!(regs.read(INTPND) & INT_FRAMESTART, 0);
    }

    #[test]
    fn intpnd_ignores_writes() {
        let mut regs = Registers::new();
        regs.write(INTPND, INT_XPEND);
        assert_eq!(regs.read(INTPND), 0);
    }

    #[test]
    fn dprst_and_xprst_clear_different_flags() {
        let mut regs = Registers::new();
        let all = INT_TIMEERR | INT_XPEND | INT_SBHIT | INT_FRAMESTART | INT_SCANERR;

        regs.pending = all;
        regs.enabled = all;
        regs.write(DPCTRL, DP_DPRST);

        assert_ne!(
            regs.pending & INT_XPEND,
            0,
            "xpend survives a display reset"
        );
        assert_ne!(regs.pending & INT_SBHIT, 0);
        assert_eq!(regs.pending & INT_FRAMESTART, 0);
        assert_eq!(regs.pending & INT_TIMEERR, 0);
        assert_eq!(regs.enabled & INT_SCANERR, 0);

        regs.pending = all;
        regs.enabled = all;
        regs.write(XPCTRL, XP_XPRST);

        assert_eq!(regs.pending & INT_XPEND, 0);
        assert_eq!(regs.pending & INT_SBHIT, 0);
        assert_ne!(
            regs.pending & INT_FRAMESTART,
            0,
            "framestart survives a draw reset"
        );
    }

    #[test]
    fn xprst_also_disables_drawing() {
        let mut regs = Registers::new();
        regs.write(XPCTRL, XP_XPEN);
        assert_ne!(regs.read(XPSTTS) & XP_XPEN, 0);

        regs.write(XPCTRL, XP_XPEN | XP_XPRST);
        assert_eq!(regs.read(XPSTTS) & XP_XPEN, 0, "reset wins over enable");
    }

    #[test]
    fn the_row_group_compare_field_is_five_bits() {
        let mut regs = Registers::new();
        regs.write(XPCTRL, 0xFF << SB_SHIFT);
        assert_eq!(regs.row_group_compare, 0x1F);

        regs.row_group = 27;
        assert_eq!((regs.read(XPSTTS) >> SB_SHIFT) & SB_MASK, 27);
    }

    #[test]
    fn narrow_registers_drop_their_unused_bits() {
        let mut regs = Registers::new();
        regs.write(BRTA, 0xFFFF);
        regs.write(FRMCYC, 0xFFFF);
        regs.write(BKCOL, 0xFFFF);
        regs.write(SPT0, 0xFFFF);
        regs.write(GPLT0, 0xFFFF);

        assert_eq!(regs.read(BRTA), 0xFF);
        assert_eq!(regs.read(FRMCYC), 0xF);
        assert_eq!(regs.read(BKCOL), 0x3);
        assert_eq!(regs.read(SPT0), 0x3FF);
        assert_eq!(
            regs.read(GPLT0),
            0xFC,
            "the low two bits have no palette entry"
        );
    }

    #[test]
    fn each_palette_and_group_register_is_separate() {
        let mut regs = Registers::new();
        for index in 0..4u32 {
            regs.write(GPLT0 + index * 2, 0xFC);
            regs.write(JPLT0 + index * 2, 0x54);
            regs.write(SPT0 + index * 2, u16::try_from(index).unwrap() + 1);
        }

        for index in 0..4u32 {
            assert_eq!(regs.read(GPLT0 + index * 2), 0xFC);
            assert_eq!(regs.read(JPLT0 + index * 2), 0x54);
            assert_eq!(
                regs.read(SPT0 + index * 2),
                u16::try_from(index).unwrap() + 1
            );
        }
    }

    #[test]
    fn palette_lookup_skips_the_transparent_entry() {
        let palette = 0b1110_0100;
        assert_eq!(shade(palette, 0), 0, "pixel 0 never reaches a palette");
        assert_eq!(shade(palette, 1), 1);
        assert_eq!(shade(palette, 2), 2);
        assert_eq!(shade(palette, 3), 3);
    }

    #[test]
    fn unmapped_register_addresses_are_inert() {
        let mut regs = Registers::new();
        regs.write(0x0005_F806, 0xFFFF);
        regs.write(0x0005_F900, 0xFFFF);
        assert_eq!(regs.read(0x0005_F806), 0);
        assert_eq!(regs.read(0x0005_F900), 0);
    }
}
