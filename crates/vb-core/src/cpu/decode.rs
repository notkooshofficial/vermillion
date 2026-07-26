use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    I,
    II,
    III,
    IV,
    V,
    VI,
    VII,
}

impl Format {
    pub fn width(self) -> u32 {
        match self {
            Format::I | Format::II | Format::III => 2,
            Format::IV | Format::V | Format::VI | Format::VII => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    Overflow,
    Carry,
    Zero,
    NotHigher,
    Negative,
    Always,
    LessThan,
    LessOrEqual,
    NotOverflow,
    NotCarry,
    NotZero,
    Higher,
    Positive,
    Never,
    GreaterOrEqual,
    GreaterThan,
}

impl Condition {
    pub fn from_bits(bits: u8) -> Condition {
        match bits & 0xF {
            0 => Condition::Overflow,
            1 => Condition::Carry,
            2 => Condition::Zero,
            3 => Condition::NotHigher,
            4 => Condition::Negative,
            5 => Condition::Always,
            6 => Condition::LessThan,
            7 => Condition::LessOrEqual,
            8 => Condition::NotOverflow,
            9 => Condition::NotCarry,
            10 => Condition::NotZero,
            11 => Condition::Higher,
            12 => Condition::Positive,
            13 => Condition::Never,
            14 => Condition::GreaterOrEqual,
            _ => Condition::GreaterThan,
        }
    }

    pub fn evaluate(self, z: bool, s: bool, ov: bool, cy: bool) -> bool {
        match self {
            Condition::Overflow => ov,
            Condition::Carry => cy,
            Condition::Zero => z,
            Condition::NotHigher => cy || z,
            Condition::Negative => s,
            Condition::Always => true,
            Condition::LessThan => ov != s,
            Condition::LessOrEqual => (ov != s) || z,
            Condition::NotOverflow => !ov,
            Condition::NotCarry => !cy,
            Condition::NotZero => !z,
            Condition::Higher => !(cy || z),
            Condition::Positive => !s,
            Condition::Never => false,
            Condition::GreaterOrEqual => ov == s,
            Condition::GreaterThan => !((ov != s) || z),
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            Condition::Overflow => "v",
            Condition::Carry => "c",
            Condition::Zero => "e",
            Condition::NotHigher => "nh",
            Condition::Negative => "n",
            Condition::Always => "r",
            Condition::LessThan => "lt",
            Condition::LessOrEqual => "le",
            Condition::NotOverflow => "nv",
            Condition::NotCarry => "nc",
            Condition::NotZero => "ne",
            Condition::Higher => "h",
            Condition::Positive => "p",
            Condition::Never => "nop",
            Condition::GreaterOrEqual => "ge",
            Condition::GreaterThan => "gt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    Mov,
    Add,
    Sub,
    Cmp,
    Shl,
    Shr,
    Jmp,
    Sar,
    Mul,
    Div,
    Mulu,
    Divu,
    Or,
    And,
    Xor,
    Not,

    MovImm,
    AddImm,
    Setf,
    CmpImm,
    ShlImm,
    ShrImm,
    Cli,
    SarImm,
    Trap,
    Reti,
    Halt,
    Ldsr,
    Stsr,
    Sei,

    Sch0bsu,
    Sch0bsd,
    Sch1bsu,
    Sch1bsd,
    Orbsu,
    Andbsu,
    Xorbsu,
    Movbsu,
    Ornbsu,
    Andnbsu,
    Xornbsu,
    Notbsu,

    Bcond,

    Jr,
    Jal,

    Movea,
    Addi,
    Ori,
    Andi,
    Xori,
    Movhi,

    LdB,
    LdH,
    LdW,
    StB,
    StH,
    StW,
    InB,
    InH,
    InW,
    OutB,
    OutH,
    OutW,
    Caxi,

    CmpfS,
    CvtWs,
    CvtSw,
    AddfS,
    SubfS,
    MulfS,
    DivfS,
    TrncSw,
    Xb,
    Xh,
    Rev,
    Mpyhw,
}

impl Op {
    pub fn mnemonic(self) -> &'static str {
        match self {
            Op::Mov | Op::MovImm => "mov",
            Op::Add | Op::AddImm => "add",
            Op::Sub => "sub",
            Op::Cmp | Op::CmpImm => "cmp",
            Op::Shl | Op::ShlImm => "shl",
            Op::Shr | Op::ShrImm => "shr",
            Op::Jmp => "jmp",
            Op::Sar | Op::SarImm => "sar",
            Op::Mul => "mul",
            Op::Div => "div",
            Op::Mulu => "mulu",
            Op::Divu => "divu",
            Op::Or => "or",
            Op::And => "and",
            Op::Xor => "xor",
            Op::Not => "not",
            Op::Setf => "setf",
            Op::Cli => "cli",
            Op::Trap => "trap",
            Op::Reti => "reti",
            Op::Halt => "halt",
            Op::Ldsr => "ldsr",
            Op::Stsr => "stsr",
            Op::Sei => "sei",
            Op::Sch0bsu => "sch0bsu",
            Op::Sch0bsd => "sch0bsd",
            Op::Sch1bsu => "sch1bsu",
            Op::Sch1bsd => "sch1bsd",
            Op::Orbsu => "orbsu",
            Op::Andbsu => "andbsu",
            Op::Xorbsu => "xorbsu",
            Op::Movbsu => "movbsu",
            Op::Ornbsu => "ornbsu",
            Op::Andnbsu => "andnbsu",
            Op::Xornbsu => "xornbsu",
            Op::Notbsu => "notbsu",
            Op::Bcond => "b",
            Op::Jr => "jr",
            Op::Jal => "jal",
            Op::Movea => "movea",
            Op::Addi => "addi",
            Op::Ori => "ori",
            Op::Andi => "andi",
            Op::Xori => "xori",
            Op::Movhi => "movhi",
            Op::LdB => "ld.b",
            Op::LdH => "ld.h",
            Op::LdW => "ld.w",
            Op::StB => "st.b",
            Op::StH => "st.h",
            Op::StW => "st.w",
            Op::InB => "in.b",
            Op::InH => "in.h",
            Op::InW => "in.w",
            Op::OutB => "out.b",
            Op::OutH => "out.h",
            Op::OutW => "out.w",
            Op::Caxi => "caxi",
            Op::CmpfS => "cmpf.s",
            Op::CvtWs => "cvt.ws",
            Op::CvtSw => "cvt.sw",
            Op::AddfS => "addf.s",
            Op::SubfS => "subf.s",
            Op::MulfS => "mulf.s",
            Op::DivfS => "divf.s",
            Op::TrncSw => "trnc.sw",
            Op::Xb => "xb",
            Op::Xh => "xh",
            Op::Rev => "rev",
            Op::Mpyhw => "mpyhw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub op: Op,
    pub format: Format,
    pub width: u32,
    pub reg1: u8,
    pub reg2: u8,
    pub imm: i32,
    pub cond: Option<Condition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    IllegalOpcode(u8),
    IllegalSubOpcode { opcode: u8, sub: u8 },
    Truncated { available: usize, needed: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DecodeError::IllegalOpcode(opcode) => {
                write!(f, "illegal opcode {opcode:#08b}")
            }
            DecodeError::IllegalSubOpcode { opcode, sub } => {
                write!(f, "illegal sub-opcode {sub:#08b} for opcode {opcode:#08b}")
            }
            DecodeError::Truncated { available, needed } => {
                write!(f, "need {needed} bytes to decode, {available} available")
            }
        }
    }
}

impl Error for DecodeError {}

const FORMAT_III_MASK: u8 = 0b111_000;
const FORMAT_III_MATCH: u8 = 0b100_000;
const FIRST_32_BIT_OPCODE: u8 = 0b101_000;

fn opcode_of(word0: u16) -> u8 {
    ((word0 >> 10) & 0x3F) as u8
}

fn reg2_of(word0: u16) -> u8 {
    ((word0 >> 5) & 0x1F) as u8
}

fn reg1_of(word0: u16) -> u8 {
    (word0 & 0x1F) as u8
}

fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

pub fn instruction_width(word0: u16) -> u32 {
    if opcode_of(word0) < FIRST_32_BIT_OPCODE {
        2
    } else {
        4
    }
}

pub fn decode(word0: u16, word1: u16) -> Result<Instruction, DecodeError> {
    let opcode = opcode_of(word0);

    if opcode & FORMAT_III_MASK == FORMAT_III_MATCH {
        return Ok(Instruction {
            op: Op::Bcond,
            format: Format::III,
            width: 2,
            reg1: 0,
            reg2: 0,
            // disp counts from this instruction's own address, not the next one
            imm: sign_extend(u32::from(word0 & 0x01FF), 9),
            cond: Some(Condition::from_bits(((word0 >> 9) & 0xF) as u8)),
        });
    }

    match opcode {
        0b000000..=0b001111 => decode_format_i(opcode, word0),
        0b010000..=0b011111 => decode_format_ii(opcode, word0),
        0b101010 | 0b101011 => decode_format_iv(opcode, word0, word1),
        0b101000 | 0b101001 | 0b101100..=0b101111 => decode_format_v(opcode, word0, word1),
        0b111110 => decode_format_vii(opcode, word0, word1),
        0b110000..=0b111101 | 0b111111 => decode_format_vi(opcode, word0, word1),
        _ => Err(DecodeError::IllegalOpcode(opcode)),
    }
}

pub fn decode_slice(bytes: &[u8]) -> Result<Instruction, DecodeError> {
    if bytes.len() < 2 {
        return Err(DecodeError::Truncated {
            available: bytes.len(),
            needed: 2,
        });
    }

    let word0 = u16::from_le_bytes([bytes[0], bytes[1]]);
    if instruction_width(word0) == 2 {
        return decode(word0, 0);
    }

    if bytes.len() < 4 {
        return Err(DecodeError::Truncated {
            available: bytes.len(),
            needed: 4,
        });
    }

    decode(word0, u16::from_le_bytes([bytes[2], bytes[3]]))
}

fn decode_format_i(opcode: u8, word0: u16) -> Result<Instruction, DecodeError> {
    let op = match opcode {
        0b000000 => Op::Mov,
        0b000001 => Op::Add,
        0b000010 => Op::Sub,
        0b000011 => Op::Cmp,
        0b000100 => Op::Shl,
        0b000101 => Op::Shr,
        0b000110 => Op::Jmp,
        0b000111 => Op::Sar,
        0b001000 => Op::Mul,
        0b001001 => Op::Div,
        0b001010 => Op::Mulu,
        0b001011 => Op::Divu,
        0b001100 => Op::Or,
        0b001101 => Op::And,
        0b001110 => Op::Xor,
        0b001111 => Op::Not,
        _ => return Err(DecodeError::IllegalOpcode(opcode)),
    };

    Ok(Instruction {
        op,
        format: Format::I,
        width: 2,
        reg1: reg1_of(word0),
        reg2: reg2_of(word0),
        imm: 0,
        cond: None,
    })
}

fn decode_format_ii(opcode: u8, word0: u16) -> Result<Instruction, DecodeError> {
    let field = u32::from(word0 & 0x1F);
    let signed = sign_extend(field, 5);
    let unsigned = field as i32;

    let (op, imm, cond) = match opcode {
        0b010000 => (Op::MovImm, signed, None),
        0b010001 => (Op::AddImm, signed, None),
        0b010010 => (
            Op::Setf,
            unsigned,
            Some(Condition::from_bits((field & 0xF) as u8)),
        ),
        0b010011 => (Op::CmpImm, signed, None),
        0b010100 => (Op::ShlImm, unsigned, None),
        0b010101 => (Op::ShrImm, unsigned, None),
        0b010110 => (Op::Cli, 0, None),
        0b010111 => (Op::SarImm, unsigned, None),
        0b011000 => (Op::Trap, unsigned, None),
        0b011001 => (Op::Reti, 0, None),
        0b011010 => (Op::Halt, 0, None),
        0b011100 => (Op::Ldsr, unsigned, None),
        0b011101 => (Op::Stsr, unsigned, None),
        0b011110 => (Op::Sei, 0, None),
        0b011111 => return decode_bit_string(opcode, word0),
        _ => return Err(DecodeError::IllegalOpcode(opcode)),
    };

    Ok(Instruction {
        op,
        format: Format::II,
        width: 2,
        reg1: 0,
        reg2: reg2_of(word0),
        imm,
        cond,
    })
}

fn decode_bit_string(opcode: u8, word0: u16) -> Result<Instruction, DecodeError> {
    let sub = reg1_of(word0);
    let op = match sub {
        0b00000 => Op::Sch0bsu,
        0b00001 => Op::Sch0bsd,
        0b00010 => Op::Sch1bsu,
        0b00011 => Op::Sch1bsd,
        0b01000 => Op::Orbsu,
        0b01001 => Op::Andbsu,
        0b01010 => Op::Xorbsu,
        0b01011 => Op::Movbsu,
        0b01100 => Op::Ornbsu,
        0b01101 => Op::Andnbsu,
        0b01110 => Op::Xornbsu,
        0b01111 => Op::Notbsu,
        _ => return Err(DecodeError::IllegalSubOpcode { opcode, sub }),
    };

    Ok(Instruction {
        op,
        format: Format::II,
        width: 2,
        reg1: sub,
        reg2: reg2_of(word0),
        imm: 0,
        cond: None,
    })
}

fn decode_format_iv(opcode: u8, word0: u16, word1: u16) -> Result<Instruction, DecodeError> {
    let raw = (u32::from(word0 & 0x03FF) << 16) | u32::from(word1);

    Ok(Instruction {
        op: if opcode == 0b101010 { Op::Jr } else { Op::Jal },
        format: Format::IV,
        width: 4,
        reg1: 0,
        reg2: 0,
        imm: sign_extend(raw, 26),
        cond: None,
    })
}

fn decode_format_v(opcode: u8, word0: u16, word1: u16) -> Result<Instruction, DecodeError> {
    let raw = u32::from(word1);
    let signed = sign_extend(raw, 16);
    let unsigned = raw as i32;

    let (op, imm) = match opcode {
        0b101000 => (Op::Movea, signed),
        0b101001 => (Op::Addi, signed),
        0b101100 => (Op::Ori, unsigned),
        0b101101 => (Op::Andi, unsigned),
        0b101110 => (Op::Xori, unsigned),
        0b101111 => (Op::Movhi, unsigned),
        _ => return Err(DecodeError::IllegalOpcode(opcode)),
    };

    Ok(Instruction {
        op,
        format: Format::V,
        width: 4,
        reg1: reg1_of(word0),
        reg2: reg2_of(word0),
        imm,
        cond: None,
    })
}

fn decode_format_vi(opcode: u8, word0: u16, word1: u16) -> Result<Instruction, DecodeError> {
    let op = match opcode {
        0b110000 => Op::LdB,
        0b110001 => Op::LdH,
        0b110011 => Op::LdW,
        0b110100 => Op::StB,
        0b110101 => Op::StH,
        0b110111 => Op::StW,
        0b111000 => Op::InB,
        0b111001 => Op::InH,
        0b111010 => Op::Caxi,
        0b111011 => Op::InW,
        0b111100 => Op::OutB,
        0b111101 => Op::OutH,
        0b111111 => Op::OutW,
        _ => return Err(DecodeError::IllegalOpcode(opcode)),
    };

    Ok(Instruction {
        op,
        format: Format::VI,
        width: 4,
        reg1: reg1_of(word0),
        reg2: reg2_of(word0),
        imm: sign_extend(u32::from(word1), 16),
        cond: None,
    })
}

fn decode_format_vii(opcode: u8, word0: u16, word1: u16) -> Result<Instruction, DecodeError> {
    let sub = ((word1 >> 10) & 0x3F) as u8;
    let op = match sub {
        0b000000 => Op::CmpfS,
        0b000010 => Op::CvtWs,
        0b000011 => Op::CvtSw,
        0b000100 => Op::AddfS,
        0b000101 => Op::SubfS,
        0b000110 => Op::MulfS,
        0b000111 => Op::DivfS,
        0b001000 => Op::Xb,
        0b001001 => Op::Xh,
        0b001010 => Op::Rev,
        0b001011 => Op::TrncSw,
        0b001100 => Op::Mpyhw,
        _ => return Err(DecodeError::IllegalSubOpcode { opcode, sub }),
    };

    Ok(Instruction {
        op,
        format: Format::VII,
        width: 4,
        reg1: reg1_of(word0),
        reg2: reg2_of(word0),
        imm: 0,
        cond: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short(opcode: u8, reg2: u8, reg1: u8) -> u16 {
        (u16::from(opcode) << 10) | (u16::from(reg2) << 5) | u16::from(reg1)
    }

    fn branch(cond: u8, disp: i16) -> u16 {
        let raw = u16::from_le_bytes(disp.to_le_bytes());
        0b100 << 13 | (u16::from(cond) << 9) | (raw & 0x01FF)
    }

    #[test]
    fn widths_follow_the_opcode_boundary() {
        assert_eq!(instruction_width(short(0b000000, 0, 0)), 2);
        assert_eq!(instruction_width(short(0b011111, 0, 0)), 2);
        assert_eq!(instruction_width(branch(5, 0)), 2);
        assert_eq!(instruction_width(short(0b100111, 0, 0)), 2);
        assert_eq!(instruction_width(short(0b101000, 0, 0)), 4);
        assert_eq!(instruction_width(short(0b111111, 0, 0)), 4);
    }

    #[test]
    fn format_width_agrees_with_instruction_width() {
        let cases = [
            short(0b000001, 3, 4),
            short(0b010000, 3, 4),
            branch(2, 8),
            short(0b101010, 0, 0),
            short(0b101000, 3, 4),
            short(0b110000, 3, 4),
        ];
        for word0 in cases {
            let decoded = decode(word0, 0).unwrap();
            assert_eq!(decoded.width, decoded.format.width());
            assert_eq!(decoded.width, instruction_width(word0));
        }
    }

    #[test]
    fn decodes_format_i() {
        let decoded = decode(short(0b000010, 30, 7), 0).unwrap();
        assert_eq!(decoded.op, Op::Sub);
        assert_eq!(decoded.format, Format::I);
        assert_eq!(decoded.reg2, 30);
        assert_eq!(decoded.reg1, 7);
        assert_eq!(decoded.imm, 0);
    }

    #[test]
    fn format_ii_sign_extends_arithmetic_immediates() {
        let decoded = decode(short(0b010000, 1, 0x1F), 0).unwrap();
        assert_eq!(decoded.op, Op::MovImm);
        assert_eq!(decoded.imm, -1);

        let decoded = decode(short(0b010001, 1, 0x10), 0).unwrap();
        assert_eq!(decoded.op, Op::AddImm);
        assert_eq!(decoded.imm, -16);
    }

    #[test]
    fn format_ii_zero_extends_shift_immediates() {
        for (opcode, op) in [
            (0b010100u8, Op::ShlImm),
            (0b010101, Op::ShrImm),
            (0b010111, Op::SarImm),
        ] {
            let decoded = decode(short(opcode, 1, 0x1F), 0).unwrap();
            assert_eq!(decoded.op, op);
            assert_eq!(decoded.imm, 31);
        }
    }

    #[test]
    fn format_ii_zero_extends_regid_and_vector() {
        assert_eq!(decode(short(0b011100, 2, 0x1F), 0).unwrap().imm, 31);
        assert_eq!(decode(short(0b011101, 2, 0x18), 0).unwrap().imm, 24);
        assert_eq!(decode(short(0b011000, 0, 0x1F), 0).unwrap().imm, 31);
    }

    #[test]
    fn setf_carries_its_condition() {
        let decoded = decode(short(0b010010, 9, 0b0_1010), 0).unwrap();
        assert_eq!(decoded.op, Op::Setf);
        assert_eq!(decoded.cond, Some(Condition::NotZero));
        assert_eq!(decoded.reg2, 9);
    }

    #[test]
    fn operandless_format_ii_instructions_clear_imm() {
        for (opcode, op) in [
            (0b010110u8, Op::Cli),
            (0b011001, Op::Reti),
            (0b011010, Op::Halt),
            (0b011110, Op::Sei),
        ] {
            let decoded = decode(short(opcode, 0, 0x1F), 0).unwrap();
            assert_eq!(decoded.op, op);
            assert_eq!(decoded.imm, 0);
        }
    }

    #[test]
    fn decodes_bit_strings() {
        assert_eq!(decode(short(0b011111, 0, 0), 0).unwrap().op, Op::Sch0bsu);
        assert_eq!(
            decode(short(0b011111, 0, 0b01011), 0).unwrap().op,
            Op::Movbsu
        );
        assert_eq!(
            decode(short(0b011111, 0, 0b01111), 0).unwrap().op,
            Op::Notbsu
        );

        assert_eq!(
            decode(short(0b011111, 0, 0b00100), 0).unwrap_err(),
            DecodeError::IllegalSubOpcode {
                opcode: 0b011111,
                sub: 0b00100
            }
        );
        assert!(decode(short(0b011111, 0, 0b10000), 0).is_err());
    }

    #[test]
    fn decodes_branches_with_signed_byte_displacement() {
        let decoded = decode(branch(6, -4), 0).unwrap();
        assert_eq!(decoded.op, Op::Bcond);
        assert_eq!(decoded.format, Format::III);
        assert_eq!(decoded.cond, Some(Condition::LessThan));
        assert_eq!(decoded.imm, -4);

        assert_eq!(decode(branch(0, 255), 0).unwrap().imm, 255);
        assert_eq!(decode(branch(0, -256), 0).unwrap().imm, -256);
    }

    #[test]
    fn every_branch_condition_decodes() {
        for bits in 0u8..16 {
            let decoded = decode(branch(bits, 0), 0).unwrap();
            assert_eq!(decoded.cond, Some(Condition::from_bits(bits)));
        }
    }

    #[test]
    fn decodes_format_iv_across_both_halfwords() {
        let decoded = decode(short(0b101011, 0, 0) | 0x0001, 0x0000).unwrap();
        assert_eq!(decoded.op, Op::Jal);
        assert_eq!(decoded.imm, 0x0001_0000);

        let all_ones = decode(short(0b101010, 0, 0) | 0x03FF, 0xFFFF).unwrap();
        assert_eq!(all_ones.op, Op::Jr);
        assert_eq!(all_ones.imm, -1);

        let max_positive = decode(short(0b101010, 0, 0) | 0x01FF, 0xFFFF).unwrap();
        assert_eq!(max_positive.imm, 0x01FF_FFFF);
    }

    #[test]
    fn format_v_extension_depends_on_the_instruction() {
        assert_eq!(decode(short(0b101000, 1, 2), 0xFFFF).unwrap().imm, -1);
        assert_eq!(decode(short(0b101001, 1, 2), 0x8000).unwrap().imm, -32768);
        assert_eq!(decode(short(0b101100, 1, 2), 0xFFFF).unwrap().imm, 0xFFFF);
        assert_eq!(decode(short(0b101101, 1, 2), 0x8000).unwrap().imm, 0x8000);
        assert_eq!(decode(short(0b101111, 1, 2), 0xFFFF).unwrap().imm, 0xFFFF);
    }

    #[test]
    fn format_vi_sign_extends_displacement() {
        let decoded = decode(short(0b110011, 5, 6), 0xFFFC).unwrap();
        assert_eq!(decoded.op, Op::LdW);
        assert_eq!(decoded.reg2, 5);
        assert_eq!(decoded.reg1, 6);
        assert_eq!(decoded.imm, -4);
    }

    #[test]
    fn decodes_format_vii_sub_opcodes() {
        let word1 = |sub: u16| sub << 10;
        assert_eq!(
            decode(short(0b111110, 1, 2), word1(0b000100)).unwrap().op,
            Op::AddfS
        );
        assert_eq!(
            decode(short(0b111110, 1, 2), word1(0b001010)).unwrap().op,
            Op::Rev
        );
        assert_eq!(
            decode(short(0b111110, 1, 2), word1(0b001100)).unwrap().op,
            Op::Mpyhw
        );

        assert_eq!(
            decode(short(0b111110, 1, 2), word1(0b000001)).unwrap_err(),
            DecodeError::IllegalSubOpcode {
                opcode: 0b111110,
                sub: 0b000001
            }
        );
        assert!(decode(short(0b111110, 1, 2), word1(0b111111)).is_err());
    }

    #[test]
    fn rejects_the_documented_illegal_opcodes() {
        for opcode in [0b011011u8, 0b110010, 0b110110] {
            assert_eq!(
                decode(short(opcode, 0, 0), 0).unwrap_err(),
                DecodeError::IllegalOpcode(opcode)
            );
        }
    }

    #[test]
    fn every_opcode_either_decodes_or_reports_illegal() {
        for opcode in 0u8..64 {
            let word0 = short(opcode, 1, 2);
            match decode(word0, 0) {
                Ok(decoded) => assert_eq!(decoded.width, instruction_width(word0)),
                Err(DecodeError::IllegalOpcode(reported)) => assert_eq!(reported, opcode),
                Err(DecodeError::IllegalSubOpcode { .. }) => {}
                Err(other) => panic!("unexpected error for {opcode:#08b}: {other}"),
            }
        }
    }

    #[test]
    fn decode_slice_reports_truncation() {
        assert_eq!(
            decode_slice(&[]).unwrap_err(),
            DecodeError::Truncated {
                available: 0,
                needed: 2
            }
        );
        assert_eq!(
            decode_slice(&[0x00, 0xA8]).unwrap_err(),
            DecodeError::Truncated {
                available: 2,
                needed: 4
            }
        );
    }

    #[test]
    fn decode_slice_reads_little_endian_halfwords() {
        let word0 = short(0b101000, 1, 2);
        let bytes = [word0.to_le_bytes()[0], word0.to_le_bytes()[1], 0xFF, 0xFF];
        let decoded = decode_slice(&bytes).unwrap();
        assert_eq!(decoded.op, Op::Movea);
        assert_eq!(decoded.imm, -1);
    }

    #[test]
    fn format_vii_wins_over_the_format_vi_range() {
        let vii = decode(short(0b111110, 1, 2), 0b001010 << 10).unwrap();
        assert_eq!(vii.format, Format::VII);
        assert_eq!(vii.op, Op::Rev);

        let vi = decode(short(0b111111, 1, 2), 0x0004).unwrap();
        assert_eq!(vi.format, Format::VI);
        assert_eq!(vi.op, Op::OutW);
    }

    #[test]
    fn condition_truth_table_matches_the_reference() {
        let cases: [(Condition, bool, bool, bool, bool, bool); 16] = [
            (Condition::Overflow, false, false, true, false, true),
            (Condition::Carry, false, false, false, true, true),
            (Condition::Zero, true, false, false, false, true),
            (Condition::NotHigher, true, false, false, false, true),
            (Condition::Negative, false, true, false, false, true),
            (Condition::Always, false, false, false, false, true),
            (Condition::LessThan, false, true, false, false, true),
            (Condition::LessOrEqual, true, false, false, false, true),
            (Condition::NotOverflow, false, false, true, false, false),
            (Condition::NotCarry, false, false, false, true, false),
            (Condition::NotZero, true, false, false, false, false),
            (Condition::Higher, true, false, false, false, false),
            (Condition::Positive, false, true, false, false, false),
            (Condition::Never, false, false, false, false, false),
            (Condition::GreaterOrEqual, false, true, false, false, false),
            (Condition::GreaterThan, true, false, false, false, false),
        ];

        for (cond, z, s, ov, cy, expected) in cases {
            assert_eq!(cond.evaluate(z, s, ov, cy), expected, "{cond:?}");
        }
    }

    #[test]
    fn opposite_conditions_are_complementary() {
        let pairs = [
            (Condition::Overflow, Condition::NotOverflow),
            (Condition::Carry, Condition::NotCarry),
            (Condition::Zero, Condition::NotZero),
            (Condition::NotHigher, Condition::Higher),
            (Condition::Negative, Condition::Positive),
            (Condition::Always, Condition::Never),
            (Condition::LessThan, Condition::GreaterOrEqual),
            (Condition::LessOrEqual, Condition::GreaterThan),
        ];

        for flags in 0u8..16 {
            let z = flags & 1 != 0;
            let s = flags & 2 != 0;
            let ov = flags & 4 != 0;
            let cy = flags & 8 != 0;
            for (a, b) in pairs {
                assert_ne!(
                    a.evaluate(z, s, ov, cy),
                    b.evaluate(z, s, ov, cy),
                    "{a:?} vs {b:?} with flags {flags:#06b}"
                );
            }
        }
    }
}
