use std::error::Error;
use std::fmt;

pub const HEADER_LEN: usize = 0x20;
pub const HEADER_OFFSET_FROM_END: usize = 0x220;

pub const TITLE_LEN: usize = 20;
pub const RESERVED_LEN: usize = 5;
pub const MAKER_CODE_LEN: usize = 2;
pub const GAME_CODE_LEN: usize = 4;

const TITLE_START: usize = 0;
const RESERVED_START: usize = TITLE_START + TITLE_LEN;
const MAKER_CODE_START: usize = RESERVED_START + RESERVED_LEN;
const GAME_CODE_START: usize = MAKER_CODE_START + MAKER_CODE_LEN;
const VERSION_START: usize = GAME_CODE_START + GAME_CODE_LEN;

pub const MIN_ROM_LEN: usize = 0x400;
pub const MAX_ROM_LEN: usize = 0x0100_0000;
pub const MAX_SRAM_LEN: usize = 0x0100_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartError {
    RomTooSmall(usize),
    RomTooLarge(usize),
    RomNotPowerOfTwo(usize),
    SramTooLarge(usize),
    SramNotPowerOfTwo(usize),
}

impl fmt::Display for CartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CartError::RomTooSmall(len) => {
                write!(f, "rom is {len} bytes, minimum is {MIN_ROM_LEN}")
            }
            CartError::RomTooLarge(len) => {
                write!(f, "rom is {len} bytes, maximum is {MAX_ROM_LEN}")
            }
            CartError::RomNotPowerOfTwo(len) => {
                write!(f, "rom is {len} bytes, which is not a power of two")
            }
            CartError::SramTooLarge(len) => {
                write!(f, "sram is {len} bytes, maximum is {MAX_SRAM_LEN}")
            }
            CartError::SramNotPowerOfTwo(len) => {
                write!(f, "sram is {len} bytes, which is not a power of two")
            }
        }
    }
}

impl Error for CartError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub title: [u8; TITLE_LEN],
    pub reserved: [u8; RESERVED_LEN],
    pub maker_code: [u8; MAKER_CODE_LEN],
    pub game_code: [u8; GAME_CODE_LEN],
    pub version: u8,
}

impl Header {
    pub fn title_ascii_lossy(&self) -> String {
        ascii_lossy(&self.title)
    }

    pub fn maker_code_ascii_lossy(&self) -> String {
        ascii_lossy(&self.maker_code)
    }

    pub fn game_code_ascii_lossy(&self) -> String {
        ascii_lossy(&self.game_code)
    }
}

fn ascii_lossy(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x00 => ' ',
            0x20..=0x7E => b as char,
            _ => '?',
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

pub struct Cart {
    rom: Box<[u8]>,
    sram: Box<[u8]>,
    rom_mask: u32,
    sram_mask: u32,
}

impl Cart {
    pub fn new(rom: Vec<u8>) -> Result<Self, CartError> {
        Self::with_sram(rom, 0)
    }

    pub fn with_sram(rom: Vec<u8>, sram_len: usize) -> Result<Self, CartError> {
        let rom_len = rom.len();

        if rom_len < MIN_ROM_LEN {
            return Err(CartError::RomTooSmall(rom_len));
        }
        if rom_len > MAX_ROM_LEN {
            return Err(CartError::RomTooLarge(rom_len));
        }
        if !rom_len.is_power_of_two() {
            return Err(CartError::RomNotPowerOfTwo(rom_len));
        }
        if sram_len > MAX_SRAM_LEN {
            return Err(CartError::SramTooLarge(sram_len));
        }
        if sram_len != 0 && !sram_len.is_power_of_two() {
            return Err(CartError::SramNotPowerOfTwo(sram_len));
        }

        let rom_mask = u32::try_from(rom_len - 1).map_err(|_| CartError::RomTooLarge(rom_len))?;
        let sram_mask = match sram_len {
            0 => 0,
            len => u32::try_from(len - 1).map_err(|_| CartError::SramTooLarge(len))?,
        };

        Ok(Self {
            rom: rom.into_boxed_slice(),
            sram: vec![0; sram_len].into_boxed_slice(),
            rom_mask,
            sram_mask,
        })
    }

    pub fn rom(&self) -> &[u8] {
        &self.rom
    }

    pub fn sram(&self) -> &[u8] {
        &self.sram
    }

    pub fn sram_mut(&mut self) -> &mut [u8] {
        &mut self.sram
    }

    pub fn has_sram(&self) -> bool {
        !self.sram.is_empty()
    }

    pub fn read_rom(&self, addr: u32) -> u8 {
        self.rom[(addr & self.rom_mask) as usize]
    }

    pub fn read_sram(&self, addr: u32) -> u8 {
        if self.sram.is_empty() {
            return 0;
        }
        self.sram[(addr & self.sram_mask) as usize]
    }

    pub fn write_sram(&mut self, addr: u32, value: u8) {
        if self.sram.is_empty() {
            return;
        }
        self.sram[(addr & self.sram_mask) as usize] = value;
    }

    pub fn header(&self) -> Header {
        let base = self.rom.len() - HEADER_OFFSET_FROM_END;
        let raw = &self.rom[base..base + HEADER_LEN];

        let mut title = [0u8; TITLE_LEN];
        let mut reserved = [0u8; RESERVED_LEN];
        let mut maker_code = [0u8; MAKER_CODE_LEN];
        let mut game_code = [0u8; GAME_CODE_LEN];

        title.copy_from_slice(&raw[TITLE_START..TITLE_START + TITLE_LEN]);
        reserved.copy_from_slice(&raw[RESERVED_START..RESERVED_START + RESERVED_LEN]);
        maker_code.copy_from_slice(&raw[MAKER_CODE_START..MAKER_CODE_START + MAKER_CODE_LEN]);
        game_code.copy_from_slice(&raw[GAME_CODE_START..GAME_CODE_START + GAME_CODE_LEN]);

        Header {
            title,
            reserved,
            maker_code,
            game_code,
            version: raw[VERSION_START],
        }
    }
}

impl fmt::Debug for Cart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cart")
            .field("rom_len", &self.rom.len())
            .field("sram_len", &self.sram.len())
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn test_rom(len: usize) -> Vec<u8> {
        let mut rom = vec![0u8; len];
        let base = len - HEADER_OFFSET_FROM_END;
        rom[base..base + TITLE_LEN].copy_from_slice(b"VERMILLION TEST ROM ");
        rom[base + MAKER_CODE_START..base + MAKER_CODE_START + MAKER_CODE_LEN]
            .copy_from_slice(b"01");
        rom[base + GAME_CODE_START..base + GAME_CODE_START + GAME_CODE_LEN]
            .copy_from_slice(b"VTRE");
        rom[base + VERSION_START] = 0x02;
        rom
    }

    #[test]
    fn header_offset_matches_bus_mirroring() {
        for shift in 10..=24u32 {
            let len: u32 = 1 << shift;
            let mirrored = 0x00FF_FDE0u32 & (len - 1);
            assert_eq!(
                mirrored as usize,
                len as usize - HEADER_OFFSET_FROM_END,
                "len {len:#x}"
            );
        }
    }

    #[test]
    fn parses_header() {
        let cart = Cart::new(test_rom(0x8000)).unwrap();
        let header = cart.header();

        assert_eq!(header.title_ascii_lossy(), "VERMILLION TEST ROM");
        assert_eq!(header.maker_code_ascii_lossy(), "01");
        assert_eq!(header.game_code_ascii_lossy(), "VTRE");
        assert_eq!(header.version, 0x02);
        assert_eq!(header.reserved, [0u8; RESERVED_LEN]);
    }

    #[test]
    fn parses_header_at_minimum_rom_size() {
        let cart = Cart::new(test_rom(MIN_ROM_LEN)).unwrap();
        assert_eq!(cart.header().game_code_ascii_lossy(), "VTRE");
    }

    #[test]
    fn rom_reads_mirror() {
        let mut rom = test_rom(0x1000);
        rom[0x0123] = 0xAB;
        let cart = Cart::new(rom).unwrap();

        assert_eq!(cart.read_rom(0x0000_0123), 0xAB);
        assert_eq!(cart.read_rom(0x0000_1123), 0xAB);
        assert_eq!(cart.read_rom(0x00FF_F123), 0xAB);
    }

    #[test]
    fn absent_sram_reads_zero_and_ignores_writes() {
        let mut cart = Cart::new(test_rom(0x1000)).unwrap();
        assert!(!cart.has_sram());

        cart.write_sram(0, 0xFF);
        assert_eq!(cart.read_sram(0), 0);
    }

    #[test]
    fn sram_round_trips_and_mirrors() {
        let mut cart = Cart::with_sram(test_rom(0x1000), 0x2000).unwrap();
        assert!(cart.has_sram());

        cart.write_sram(0x0010, 0x5A);
        assert_eq!(cart.read_sram(0x0010), 0x5A);
        assert_eq!(cart.read_sram(0x2010), 0x5A);
        assert_eq!(cart.read_sram(0x00FF_E010), 0x5A);
    }

    #[test]
    fn rejects_bad_rom_sizes() {
        assert_eq!(
            Cart::new(vec![0; 0x200]).unwrap_err(),
            CartError::RomTooSmall(0x200)
        );
        assert_eq!(
            Cart::new(vec![0; 0x1800]).unwrap_err(),
            CartError::RomNotPowerOfTwo(0x1800)
        );
    }

    #[test]
    fn rejects_bad_sram_sizes() {
        assert_eq!(
            Cart::with_sram(test_rom(0x1000), 0x1500).unwrap_err(),
            CartError::SramNotPowerOfTwo(0x1500)
        );
        assert_eq!(
            Cart::with_sram(test_rom(0x1000), MAX_SRAM_LEN * 2).unwrap_err(),
            CartError::SramTooLarge(MAX_SRAM_LEN * 2)
        );
    }

    #[test]
    fn ascii_lossy_replaces_unprintable_and_trims() {
        assert_eq!(ascii_lossy(b"OK\x00\x00\x00"), "OK");
        assert_eq!(ascii_lossy(b"A\x01B"), "A?B");
        assert_eq!(ascii_lossy(b"   "), "");
    }
}
