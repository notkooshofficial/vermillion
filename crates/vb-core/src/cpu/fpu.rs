// narrowing is the subject of this module: rounding f64 down to f32 is how
// precision loss and overflow are detected
#![allow(clippy::cast_possible_truncation)]

use crate::cpu::decode::Op;
use crate::cpu::state::{
    Cpu, PSW_CY, PSW_FIV, PSW_FOV, PSW_FPR, PSW_FRO, PSW_FUD, PSW_FZD, PSW_OV, PSW_S, PSW_Z,
};

pub const EXC_FP_RESERVED: u16 = 0xFF60;
pub const EXC_FP_INVALID: u16 = 0xFF70;
pub const EXC_FP_ZERO_DIVIDE: u16 = 0xFF68;
pub const EXC_FP_OVERFLOW: u16 = 0xFF64;

pub const HANDLER_FP: u32 = 0xFFFF_FF60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpFault {
    Reserved,
    Invalid,
    ZeroDivide,
    Overflow,
}

impl FpFault {
    pub fn code(self) -> u16 {
        match self {
            FpFault::Reserved => EXC_FP_RESERVED,
            FpFault::Invalid => EXC_FP_INVALID,
            FpFault::ZeroDivide => EXC_FP_ZERO_DIVIDE,
            FpFault::Overflow => EXC_FP_OVERFLOW,
        }
    }

    pub fn flag(self) -> u32 {
        match self {
            FpFault::Reserved => PSW_FRO,
            FpFault::Invalid => PSW_FIV,
            FpFault::ZeroDivide => PSW_FZD,
            FpFault::Overflow => PSW_FOV,
        }
    }
}

fn reserved_operand(value: f32) -> bool {
    value.is_nan() || (value != 0.0 && value.abs() < f32::MIN_POSITIVE)
}

impl Cpu {
    fn set_fp_condition(&mut self, result: u32) {
        self.set_flag(PSW_Z, result == 0);
        let sign = result & 0x8000_0000 != 0;
        self.set_flag(PSW_S, sign);
        // carry duplicates sign for floating point, unlike every integer op
        self.set_flag(PSW_CY, sign);
        self.set_flag(PSW_OV, false);
    }

    // these are sticky, software clears them with ldsr
    fn raise_fp_flag(&mut self, fault: FpFault) {
        self.psw |= fault.flag();
    }

    fn finish_float(&mut self, exact: f64, target: u8) -> Option<FpFault> {
        let rounded = exact as f32;

        if rounded.is_infinite() && exact.is_finite() {
            self.raise_fp_flag(FpFault::Overflow);
            return Some(FpFault::Overflow);
        }

        // f32 may already have flushed it to zero, so judge underflow by the exact value
        if exact != 0.0 && rounded.abs() < f32::MIN_POSITIVE {
            self.psw |= PSW_FUD;
            self.set_reg(target, 0);
            self.set_fp_condition(0);
            return None;
        }

        if f64::from(rounded) != exact {
            self.psw |= PSW_FPR;
        }

        let bits = rounded.to_bits();
        self.set_reg(target, bits);
        self.set_fp_condition(bits);
        None
    }

    fn finish_integer(&mut self, value: f64, target: u8) -> Option<FpFault> {
        if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
            self.raise_fp_flag(FpFault::Invalid);
            return Some(FpFault::Invalid);
        }

        let truncated = value as i32;
        if f64::from(truncated) != value {
            self.psw |= PSW_FPR;
        }

        let bits = truncated.cast_unsigned();
        self.set_reg(target, bits);
        self.set_fp_condition(bits);
        None
    }

    pub fn execute_float(&mut self, op: Op, reg1: u8, reg2: u8) -> Option<FpFault> {
        let right = f32::from_bits(self.reg(reg1));
        let left = f32::from_bits(self.reg(reg2));

        if matches!(op, Op::CvtWs) {
            let value = f64::from(self.reg(reg2).cast_signed());
            return self.finish_float(value, reg2);
        }

        if matches!(op, Op::CvtSw | Op::TrncSw) {
            if reserved_operand(right) {
                self.raise_fp_flag(FpFault::Reserved);
                return Some(FpFault::Reserved);
            }
            let value = f64::from(right);
            let approximated = if matches!(op, Op::CvtSw) {
                value.round_ties_even()
            } else {
                value.trunc()
            };
            return self.finish_integer(approximated, reg2);
        }

        if reserved_operand(left) || reserved_operand(right) {
            self.raise_fp_flag(FpFault::Reserved);
            return Some(FpFault::Reserved);
        }

        let a = f64::from(left);
        let b = f64::from(right);

        match op {
            Op::CmpfS => {
                let bits = ((a - b) as f32).to_bits();
                self.set_fp_condition(if a == b { 0 } else { bits });
                None
            }
            Op::AddfS => self.finish_float(a + b, reg2),
            Op::SubfS => self.finish_float(a - b, reg2),
            Op::MulfS => self.finish_float(a * b, reg2),
            Op::DivfS => {
                if b == 0.0 {
                    let fault = if a == 0.0 {
                        FpFault::Invalid
                    } else {
                        FpFault::ZeroDivide
                    };
                    self.raise_fp_flag(fault);
                    return Some(fault);
                }
                self.finish_float(a / b, reg2)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_with(reg1: f32, reg2: f32) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.set_reg(1, reg1.to_bits());
        cpu.set_reg(2, reg2.to_bits());
        cpu
    }

    fn result(cpu: &Cpu) -> f32 {
        f32::from_bits(cpu.reg(2))
    }

    #[test]
    fn arithmetic_works() {
        let mut cpu = cpu_with(2.0, 5.0);
        assert_eq!(cpu.execute_float(Op::AddfS, 1, 2), None);
        assert_eq!(result(&cpu), 7.0);

        let mut cpu = cpu_with(2.0, 5.0);
        cpu.execute_float(Op::SubfS, 1, 2);
        assert_eq!(result(&cpu), 3.0);

        let mut cpu = cpu_with(2.0, 5.0);
        cpu.execute_float(Op::MulfS, 1, 2);
        assert_eq!(result(&cpu), 10.0);

        let mut cpu = cpu_with(2.0, 5.0);
        cpu.execute_float(Op::DivfS, 1, 2);
        assert_eq!(result(&cpu), 2.5);
    }

    #[test]
    fn carry_duplicates_sign() {
        let mut cpu = cpu_with(5.0, 2.0);
        cpu.execute_float(Op::SubfS, 1, 2);
        assert_eq!(result(&cpu), -3.0);
        assert!(cpu.flag(PSW_S));
        assert!(cpu.flag(PSW_CY));
        assert!(!cpu.flag(PSW_OV));

        let mut cpu = cpu_with(2.0, 5.0);
        cpu.execute_float(Op::SubfS, 1, 2);
        assert!(!cpu.flag(PSW_S));
        assert!(!cpu.flag(PSW_CY));
    }

    #[test]
    fn zero_result_sets_zero_flag() {
        let mut cpu = cpu_with(5.0, 5.0);
        cpu.execute_float(Op::SubfS, 1, 2);
        assert!(cpu.flag(PSW_Z));
    }

    #[test]
    fn nan_and_denormal_operands_are_reserved() {
        let mut cpu = cpu_with(f32::NAN, 1.0);
        assert_eq!(cpu.execute_float(Op::AddfS, 1, 2), Some(FpFault::Reserved));
        assert!(cpu.flag(PSW_FRO));

        let denormal = f32::from_bits(1);
        let mut cpu = cpu_with(denormal, 1.0);
        assert_eq!(cpu.execute_float(Op::AddfS, 1, 2), Some(FpFault::Reserved));
    }

    #[test]
    fn division_distinguishes_zero_over_zero() {
        let mut cpu = cpu_with(0.0, 1.0);
        assert_eq!(
            cpu.execute_float(Op::DivfS, 1, 2),
            Some(FpFault::ZeroDivide)
        );
        assert!(cpu.flag(PSW_FZD));

        let mut cpu = cpu_with(0.0, 0.0);
        assert_eq!(cpu.execute_float(Op::DivfS, 1, 2), Some(FpFault::Invalid));
        assert!(cpu.flag(PSW_FIV));
    }

    #[test]
    fn overflow_is_reported() {
        let mut cpu = cpu_with(f32::MAX, f32::MAX);
        assert_eq!(cpu.execute_float(Op::MulfS, 1, 2), Some(FpFault::Overflow));
        assert!(cpu.flag(PSW_FOV));
    }

    #[test]
    fn underflow_flushes_to_zero_without_faulting() {
        let tiny = f32::MIN_POSITIVE;
        let mut cpu = cpu_with(1.0e30, tiny);
        assert_eq!(cpu.execute_float(Op::DivfS, 1, 2), None);
        assert_eq!(cpu.reg(2), 0);
        assert!(cpu.flag(PSW_FUD));
        assert!(cpu.flag(PSW_Z));
    }

    #[test]
    fn precision_loss_sets_fpr_but_does_not_fault() {
        let mut cpu = cpu_with(1.0, 16_777_216.0);
        assert_eq!(cpu.execute_float(Op::AddfS, 1, 2), None);
        assert!(cpu.flag(PSW_FPR));
    }

    #[test]
    fn conversions_round_and_truncate_differently() {
        let mut cpu = cpu_with(2.5, 0.0);
        cpu.execute_float(Op::CvtSw, 1, 2);
        assert_eq!(cpu.reg(2), 2);

        let mut cpu = cpu_with(2.9, 0.0);
        cpu.execute_float(Op::TrncSw, 1, 2);
        assert_eq!(cpu.reg(2), 2);

        let mut cpu = cpu_with(-2.9, 0.0);
        cpu.execute_float(Op::TrncSw, 1, 2);
        assert_eq!(cpu.reg(2).cast_signed(), -2);
    }

    #[test]
    fn conversion_out_of_word_range_is_invalid() {
        let mut cpu = cpu_with(1.0e30, 0.0);
        assert_eq!(cpu.execute_float(Op::CvtSw, 1, 2), Some(FpFault::Invalid));
        assert!(cpu.flag(PSW_FIV));
    }

    #[test]
    fn integer_to_float_conversion() {
        let mut cpu = Cpu::new();
        cpu.set_reg(2, (-42i32).cast_unsigned());
        cpu.execute_float(Op::CvtWs, 1, 2);
        assert_eq!(f32::from_bits(cpu.reg(2)), -42.0);
    }

    #[test]
    fn float_flags_are_sticky_across_operations() {
        let mut cpu = cpu_with(f32::NAN, 1.0);
        cpu.execute_float(Op::AddfS, 1, 2);
        assert!(cpu.flag(PSW_FRO));

        cpu.set_reg(1, 2.0f32.to_bits());
        cpu.set_reg(2, 3.0f32.to_bits());
        cpu.execute_float(Op::AddfS, 1, 2);
        assert!(cpu.flag(PSW_FRO), "fro must survive a clean operation");
    }

    #[test]
    fn compare_leaves_the_destination_alone() {
        let mut cpu = cpu_with(5.0, 5.0);
        let before = cpu.reg(2);
        cpu.execute_float(Op::CmpfS, 1, 2);
        assert_eq!(cpu.reg(2), before);
        assert!(cpu.flag(PSW_Z));
    }
}
