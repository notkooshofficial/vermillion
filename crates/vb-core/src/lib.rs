pub mod bus;
pub mod cart;
pub mod cpu;

pub use bus::{Bus, Region};
pub use cart::{Cart, CartError, Header};
pub use cpu::{Condition, DecodeError, Format, Instruction, Op};

pub const SCREEN_WIDTH: usize = 384;
pub const SCREEN_HEIGHT: usize = 224;
pub const FRAME_BUFFER_HEIGHT: usize = 256;

pub const RESET_PC: u32 = 0xFFFF_FFF0;
pub const RESET_PSW: u32 = 0x0000_8000;
pub const RESET_ECR: u32 = 0x0000_FFF0;
