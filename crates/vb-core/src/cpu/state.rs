use crate::cpu::cache::{CHCW_ICE, Cache};

pub const PSW_Z: u32 = 1 << 0;
pub const PSW_S: u32 = 1 << 1;
pub const PSW_OV: u32 = 1 << 2;
pub const PSW_CY: u32 = 1 << 3;
pub const PSW_FPR: u32 = 1 << 4;
pub const PSW_FUD: u32 = 1 << 5;
pub const PSW_FOV: u32 = 1 << 6;
pub const PSW_FZD: u32 = 1 << 7;
pub const PSW_FIV: u32 = 1 << 8;
pub const PSW_FRO: u32 = 1 << 9;
pub const PSW_ID: u32 = 1 << 12;
pub const PSW_AE: u32 = 1 << 13;
pub const PSW_EP: u32 = 1 << 14;
pub const PSW_NP: u32 = 1 << 15;

pub const PSW_I_SHIFT: u32 = 16;
pub const PSW_I_MASK: u32 = 0xF << PSW_I_SHIFT;

// rfu bits read as zero and cannot be written
pub const PSW_WRITABLE: u32 = 0x000F_F3FF;

pub const SR_EIPC: u8 = 0;
pub const SR_EIPSW: u8 = 1;
pub const SR_FEPC: u8 = 2;
pub const SR_FEPSW: u8 = 3;
pub const SR_ECR: u8 = 4;
pub const SR_PSW: u8 = 5;
pub const SR_PIR: u8 = 6;
pub const SR_TKCW: u8 = 7;
pub const SR_CHCW: u8 = 24;
pub const SR_ADTRE: u8 = 25;

pub const PIR_VALUE: u32 = 0x0000_5346;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    pub cache: Cache,
    pub regs: [u32; 32],
    pub pc: u32,
    pub psw: u32,
    pub eipc: u32,
    pub eipsw: u32,
    pub fepc: u32,
    pub fepsw: u32,
    pub ecr: u32,
    pub tkcw: u32,
    pub chcw: u32,
    pub adtre: u32,
    pub sr29: u32,
    pub sr30: u32,
    pub sr31: u32,
    pub halted: bool,
    pub cycles: u64,
    // load and store timing depends on the previous bus access
    pub prev_was_load: bool,
    pub consecutive_stores: u32,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
            regs: [0; 32],
            pc: crate::RESET_PC,
            psw: crate::RESET_PSW,
            eipc: 0,
            eipsw: 0,
            fepc: 0,
            fepsw: 0,
            ecr: crate::RESET_ECR,
            tkcw: 0,
            chcw: 0,
            adtre: 0,
            sr29: 0,
            sr30: 0,
            sr31: 0,
            halted: false,
            cycles: 0,
            prev_was_load: false,
            consecutive_stores: 0,
        }
    }

    pub fn reg(&self, index: u8) -> u32 {
        self.regs[(index & 0x1F) as usize]
    }

    pub fn set_reg(&mut self, index: u8, value: u32) {
        let index = (index & 0x1F) as usize;
        if index != 0 {
            self.regs[index] = value;
        }
    }

    pub fn flag(&self, mask: u32) -> bool {
        self.psw & mask != 0
    }

    pub fn set_flag(&mut self, mask: u32, value: bool) {
        if value {
            self.psw |= mask;
        } else {
            self.psw &= !mask;
        }
    }

    pub fn interrupt_level(&self) -> u32 {
        (self.psw & PSW_I_MASK) >> PSW_I_SHIFT
    }

    pub fn set_interrupt_level(&mut self, level: u32) {
        self.psw = (self.psw & !PSW_I_MASK) | ((level.min(15) << PSW_I_SHIFT) & PSW_I_MASK);
    }

    pub fn set_psw(&mut self, value: u32) {
        self.psw = value & PSW_WRITABLE;
    }

    pub fn read_system_register(&self, index: u8) -> u32 {
        match index & 0x1F {
            SR_EIPC => self.eipc,
            SR_EIPSW => self.eipsw,
            SR_FEPC => self.fepc,
            SR_FEPSW => self.fepsw,
            SR_ECR => self.ecr,
            SR_PSW => self.psw,
            SR_PIR => PIR_VALUE,
            SR_TKCW => self.tkcw,
            SR_CHCW => self.chcw,
            SR_ADTRE => self.adtre,
            29 => self.sr29,
            30 => self.sr30,
            31 => self.sr31,
            _ => 0,
        }
    }

    // ecr, pir, tkcw and sr30 ignore writes; reserved indices too
    pub fn write_system_register(&mut self, index: u8, value: u32) {
        match index & 0x1F {
            SR_EIPC => self.eipc = value & !1,
            SR_EIPSW => self.eipsw = value & PSW_WRITABLE,
            SR_FEPC => self.fepc = value & !1,
            SR_FEPSW => self.fepsw = value & PSW_WRITABLE,
            SR_PSW => self.set_psw(value),
            // the operation bits are write-only requests, not state; cache_control runs them
            SR_CHCW => self.chcw = value & CHCW_ICE,
            SR_ADTRE => self.adtre = value,
            29 => self.sr29 = value,
            31 => self.sr31 = value,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_state_matches_the_reference() {
        let cpu = Cpu::new();
        assert_eq!(cpu.pc, 0xFFFF_FFF0);
        assert_eq!(cpu.psw, 0x0000_8000);
        assert_eq!(cpu.ecr, 0x0000_FFF0);
        assert!(cpu.flag(PSW_NP));
        assert!(!cpu.flag(PSW_EP));
        assert!(!cpu.flag(PSW_ID));
        assert_eq!(cpu.interrupt_level(), 0);
    }

    #[test]
    fn psw_flag_bits_match_the_documented_layout() {
        assert_eq!(PSW_Z, 1 << 0);
        assert_eq!(PSW_S, 1 << 1);
        assert_eq!(PSW_OV, 1 << 2);
        assert_eq!(PSW_CY, 1 << 3);
        assert_eq!(PSW_ID, 1 << 12);
        assert_eq!(PSW_AE, 1 << 13);
        assert_eq!(PSW_EP, 1 << 14);
        assert_eq!(PSW_NP, 1 << 15);
        assert_eq!(PSW_I_MASK, 0x000F_0000);
    }

    #[test]
    fn r0_never_takes_a_write() {
        let mut cpu = Cpu::new();
        cpu.set_reg(0, 0xDEAD_BEEF);
        assert_eq!(cpu.reg(0), 0);

        cpu.set_reg(31, 0xDEAD_BEEF);
        assert_eq!(cpu.reg(31), 0xDEAD_BEEF);
    }

    #[test]
    fn psw_rejects_reserved_bits() {
        let mut cpu = Cpu::new();
        cpu.set_psw(0xFFFF_FFFF);
        assert_eq!(cpu.psw, PSW_WRITABLE);
        assert_eq!(cpu.psw & 0xFFF0_0000, 0);
        assert_eq!(cpu.psw & 0x0000_0C00, 0);
    }

    #[test]
    fn read_only_system_registers_ignore_writes() {
        let mut cpu = Cpu::new();
        let ecr = cpu.ecr;

        cpu.write_system_register(SR_ECR, 0xFFFF_FFFF);
        cpu.write_system_register(SR_PIR, 0xFFFF_FFFF);
        cpu.write_system_register(SR_TKCW, 0xFFFF_FFFF);
        cpu.write_system_register(30, 0xFFFF_FFFF);

        assert_eq!(cpu.ecr, ecr);
        assert_eq!(cpu.read_system_register(SR_PIR), PIR_VALUE);
        assert_eq!(cpu.tkcw, 0);
        assert_eq!(cpu.sr30, 0);
    }

    #[test]
    fn reserved_system_registers_read_zero() {
        let mut cpu = Cpu::new();
        for index in [8u8, 15, 23, 26, 28] {
            cpu.write_system_register(index, 0xFFFF_FFFF);
            assert_eq!(cpu.read_system_register(index), 0, "sr{index}");
        }
    }

    #[test]
    fn restore_registers_keep_pc_even() {
        let mut cpu = Cpu::new();
        cpu.write_system_register(SR_EIPC, 0xFFFF_FFFF);
        cpu.write_system_register(SR_FEPC, 0xFFFF_FFFF);
        assert_eq!(cpu.eipc & 1, 0);
        assert_eq!(cpu.fepc & 1, 0);
    }

    #[test]
    fn interrupt_level_clamps_to_15() {
        let mut cpu = Cpu::new();
        cpu.set_interrupt_level(20);
        assert_eq!(cpu.interrupt_level(), 15);
        cpu.set_interrupt_level(4);
        assert_eq!(cpu.interrupt_level(), 4);
    }
}
