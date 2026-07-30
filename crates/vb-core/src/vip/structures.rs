use crate::vip::{
    BG_MAPS, CHARACTER_MIRROR, LEFT_COLUMN_TABLE, OBJECT_ATTRIBUTES, RIGHT_COLUMN_TABLE, Vip,
    WORLD_ATTRIBUTES,
};

pub const CHARACTER_COUNT: u16 = 2048;
pub const CHARACTER_BYTES: u32 = 16;

pub const BG_MAP_CELLS: u16 = 4096;
pub const BG_MAP_BYTES: u32 = 8192;
pub const BG_MAP_SIDE: u16 = 64;

pub const OBJECT_COUNT: u16 = 1024;
pub const OBJECT_BYTES: u32 = 8;

pub const WORLD_COUNT: u8 = 32;
pub const WORLD_BYTES: u32 = 32;

pub const COLUMN_ENTRIES: u16 = 256;

pub const HBIAS_BYTES: u32 = 4;
pub const AFFINE_BYTES: u32 = 16;

pub const SOURCE_FRACTION_BITS: u32 = 3;
pub const DIRECTION_FRACTION_BITS: u32 = 9;

fn sign_extend(value: u16, bits: u32) -> i32 {
    let shift = 32 - bits;
    (u32::from(value) << shift).cast_signed() >> shift
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Character {
    pub rows: [u16; 8],
}

impl Character {
    pub fn read(vip: &Vip, index: u16) -> Character {
        let base = CHARACTER_MIRROR + u32::from(index % CHARACTER_COUNT) * CHARACTER_BYTES;
        let mut rows = [0u16; 8];
        for (row, slot) in rows.iter_mut().enumerate() {
            *slot = vip.read_u16(base + u32::try_from(row).unwrap_or(0) * 2);
        }
        Character { rows }
    }

    // low order bits sit to the left within a row
    pub fn pixel(&self, x: u8, y: u8) -> u8 {
        let row = self.rows[(y & 7) as usize];
        ((row >> ((x & 7) * 2)) & 3) as u8
    }

    pub fn flipped_pixel(&self, x: u8, y: u8, horizontal: bool, vertical: bool) -> u8 {
        let x = if horizontal { 7 - (x & 7) } else { x & 7 };
        let y = if vertical { 7 - (y & 7) } else { y & 7 };
        self.pixel(x, y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub character: u16,
    pub palette: u8,
    pub horizontal_flip: bool,
    pub vertical_flip: bool,
}

impl Cell {
    pub fn decode(raw: u16) -> Cell {
        Cell {
            character: raw & 0x07FF,
            palette: ((raw >> 14) & 3) as u8,
            horizontal_flip: raw & (1 << 13) != 0,
            vertical_flip: raw & (1 << 12) != 0,
        }
    }

    pub fn address(map: u16, cell: u16) -> u32 {
        BG_MAPS + u32::from(map & 0xF) * BG_MAP_BYTES + u32::from(cell % BG_MAP_CELLS) * 2
    }

    pub fn read(vip: &Vip, map: u16, cell: u16) -> Cell {
        Cell::decode(vip.read_u16(Cell::address(map, cell)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Object {
    pub x: i32,
    pub y: i32,
    pub parallax: i32,
    pub character: u16,
    pub palette: u8,
    pub left: bool,
    pub right: bool,
    pub horizontal_flip: bool,
    pub vertical_flip: bool,
}

impl Object {
    pub fn decode(words: [u16; 4]) -> Object {
        Object {
            x: sign_extend(words[0] & 0x03FF, 10),
            left: words[1] & (1 << 15) != 0,
            right: words[1] & (1 << 14) != 0,
            parallax: sign_extend(words[1] & 0x03FF, 10),
            y: decode_object_y(words[2] & 0x00FF),
            palette: ((words[3] >> 14) & 3) as u8,
            horizontal_flip: words[3] & (1 << 13) != 0,
            vertical_flip: words[3] & (1 << 12) != 0,
            character: words[3] & 0x07FF,
        }
    }

    pub fn address(index: u16) -> u32 {
        OBJECT_ATTRIBUTES + u32::from(index % OBJECT_COUNT) * OBJECT_BYTES
    }

    pub fn read(vip: &Vip, index: u16) -> Object {
        let base = Object::address(index);
        Object::decode([
            vip.read_u16(base),
            vip.read_u16(base + 2),
            vip.read_u16(base + 4),
            vip.read_u16(base + 6),
        ])
    }

    pub fn left_x(&self) -> i32 {
        self.x - self.parallax
    }

    pub fn right_x(&self) -> i32 {
        self.x + self.parallax
    }
}

// jy is not two's complement, it reaches +224 upward and only wraps negative at the very top
fn decode_object_y(raw: u16) -> i32 {
    if raw >= 0xF8 {
        i32::from(raw) - 256
    } else {
        i32::from(raw)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldKind {
    Normal,
    HBias,
    Affine,
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct World {
    pub kind: WorldKind,
    pub left: bool,
    pub right: bool,
    pub end: bool,
    pub overplane: bool,
    pub map_base: u16,
    pub map_width_power: u8,
    pub map_height_power: u8,
    pub destination_x: i32,
    pub destination_y: i32,
    pub destination_parallax: i32,
    pub source_x: i32,
    pub source_y: i32,
    pub source_parallax: i32,
    pub raw_width: u16,
    pub raw_height: u16,
    pub param_base: u16,
    pub overplane_character: u16,
}

impl World {
    pub fn decode(words: [u16; 16]) -> World {
        let header = words[0];
        let kind = match (header >> 12) & 3 {
            0 => WorldKind::Normal,
            1 => WorldKind::HBias,
            2 => WorldKind::Affine,
            _ => WorldKind::Object,
        };

        World {
            kind,
            left: header & (1 << 15) != 0,
            right: header & (1 << 14) != 0,
            map_width_power: ((header >> 10) & 3) as u8,
            map_height_power: ((header >> 8) & 3) as u8,
            overplane: header & (1 << 7) != 0,
            end: header & (1 << 6) != 0,
            map_base: header & 0xF,
            destination_x: sign_extend(words[1] & 0x03FF, 10),
            destination_parallax: sign_extend(words[2] & 0x03FF, 10),
            destination_y: sign_extend(words[3], 16),
            source_x: sign_extend(words[4] & 0x1FFF, 13),
            source_parallax: sign_extend(words[5] & 0x7FFF, 15),
            source_y: sign_extend(words[6] & 0x1FFF, 13),
            raw_width: words[7] & 0x1FFF,
            raw_height: words[8],
            param_base: words[9],
            overplane_character: words[10],
        }
    }

    pub fn address(index: u8) -> u32 {
        WORLD_ATTRIBUTES + u32::from(index % WORLD_COUNT) * WORLD_BYTES
    }

    pub fn read(vip: &Vip, index: u8) -> World {
        let base = World::address(index);
        let mut words = [0u16; 16];
        for (slot, word) in words.iter_mut().enumerate() {
            *word = vip.read_u16(base + u32::try_from(slot).unwrap_or(0) * 2);
        }
        World::decode(words)
    }

    pub fn dummy(&self) -> bool {
        !self.left && !self.right
    }

    pub fn width(&self) -> i32 {
        if matches!(self.kind, WorldKind::Affine) {
            i32::from(self.raw_width & 0x03FF) + 1
        } else {
            sign_extend(self.raw_width, 13) + 1
        }
    }

    // normal and h-bias worlds never draw shorter than one character
    pub fn height(&self) -> i32 {
        let height = sign_extend(self.raw_height, 16) + 1;
        match self.kind {
            WorldKind::Normal | WorldKind::HBias => height.max(8),
            _ => height,
        }
    }

    pub fn map_count(&self) -> u32 {
        (1 << self.map_width_power) * (1 << self.map_height_power)
    }

    // a background of more than eight maps is treated as eight when picking the base
    pub fn effective_base(&self) -> u16 {
        let span = u16::try_from(self.map_count().min(8)).unwrap_or(8);
        self.map_base & !(span - 1)
    }

    pub fn param_address(&self) -> u32 {
        BG_MAPS + u32::from(self.param_base) * 2
    }

    pub fn overplane_address(&self) -> u32 {
        BG_MAPS + u32::from(self.overplane_character) * 2
    }

    pub fn left_destination(&self) -> i32 {
        self.destination_x - self.destination_parallax
    }

    pub fn right_destination(&self) -> i32 {
        self.destination_x + self.destination_parallax
    }

    pub fn left_source(&self) -> i32 {
        self.source_x - self.source_parallax
    }

    pub fn right_source(&self) -> i32 {
        self.source_x + self.source_parallax
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HBias {
    pub left: i32,
    pub right: i32,
}

impl HBias {
    pub fn read(vip: &Vip, param_address: u32, row: u32) -> HBias {
        let left = param_address.wrapping_add(row * HBIAS_BYTES);
        // the vip reaches the right offset by or-ing with 2, so a misaligned base reuses the left
        let right = left | 2;
        HBias {
            left: sign_extend(vip.read_u16(left) & 0x1FFF, 13),
            right: sign_extend(vip.read_u16(right) & 0x1FFF, 13),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Affine {
    pub source_x: i32,
    pub source_y: i32,
    pub parallax: i32,
    pub direction_x: i32,
    pub direction_y: i32,
}

impl Affine {
    pub fn read(vip: &Vip, param_address: u32, row: u32) -> Affine {
        let base = param_address.wrapping_add(row * AFFINE_BYTES);
        // field addresses are or-ed rather than added, which is why elements must be
        // aligned to 16 bytes
        let field = |index: u32| vip.read_u16(base | (index * 2));

        Affine {
            source_x: sign_extend(field(0), 16),
            parallax: sign_extend(field(1), 16),
            source_y: sign_extend(field(2), 16),
            direction_x: sign_extend(field(3), 16),
            direction_y: sign_extend(field(4), 16),
        }
    }

    // the left eye is shifted when parallax is negative, the right eye when it is not
    pub fn column_source(&self, column: i32, right_eye: bool) -> (i32, i32) {
        let shifted = if self.parallax < 0 {
            if right_eye {
                column
            } else {
                column - self.parallax
            }
        } else if right_eye {
            column + self.parallax
        } else {
            column
        };

        (
            self.source_x + self.direction_x * shifted,
            self.source_y + self.direction_y * shifted,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnEntry {
    pub repeat: u16,
    pub length: u16,
}

impl ColumnEntry {
    pub fn decode(raw: u16) -> ColumnEntry {
        ColumnEntry {
            repeat: (raw >> 8) & 0xFF,
            length: raw & 0xFF,
        }
    }

    pub fn address(right_eye: bool, index: u16) -> u32 {
        let base = if right_eye {
            RIGHT_COLUMN_TABLE
        } else {
            LEFT_COLUMN_TABLE
        };
        base + u32::from(index % COLUMN_ENTRIES) * 2
    }

    pub fn read(vip: &Vip, right_eye: bool, index: u16) -> ColumnEntry {
        ColumnEntry::decode(vip.read_u16(ColumnEntry::address(right_eye, index)))
    }

    pub fn pulses(&self) -> u16 {
        self.repeat + 1
    }

    pub fn duration(&self) -> u16 {
        self.length + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vip::{CHARACTER_TABLE_0, WORLD_ATTRIBUTES};

    fn vip_with(pairs: &[(u32, u16)]) -> Vip {
        let mut vip = Vip::new();
        for (addr, value) in pairs {
            vip.write_u16(*addr, *value);
        }
        vip
    }

    #[test]
    fn character_pixels_run_left_to_right_from_the_low_bits() {
        let vip = vip_with(&[(CHARACTER_TABLE_0, 0b11_10_01_00)]);
        let character = Character::read(&vip, 0);

        assert_eq!(character.pixel(0, 0), 0);
        assert_eq!(character.pixel(1, 0), 1);
        assert_eq!(character.pixel(2, 0), 2);
        assert_eq!(character.pixel(3, 0), 3);
    }

    #[test]
    fn characters_index_through_the_linear_mirror() {
        let mut vip = Vip::new();
        // character 512 is the first of table 1, which is not adjacent in real memory
        vip.write_u16(0x0000_E000, 0xFFFF);
        assert_eq!(Character::read(&vip, 512).rows[0], 0xFFFF);
    }

    #[test]
    fn flipping_mirrors_both_axes() {
        let mut vip = Vip::new();
        vip.write_u16(CHARACTER_TABLE_0, 0b11);
        vip.write_u16(CHARACTER_TABLE_0 + 14, 0b10);
        let character = Character::read(&vip, 0);

        assert_eq!(character.flipped_pixel(7, 0, true, false), 3);
        assert_eq!(character.flipped_pixel(0, 7, false, true), 3);
        assert_eq!(character.flipped_pixel(7, 0, true, true), 2);
        assert_eq!(character.flipped_pixel(7, 7, true, true), 3);
    }

    #[test]
    fn cells_split_into_their_documented_fields() {
        let cell = Cell::decode(0b11_1_1_0_111_1111_1111);
        assert_eq!(cell.palette, 3);
        assert!(cell.horizontal_flip);
        assert!(cell.vertical_flip);
        assert_eq!(cell.character, 0x7FF);

        let plain = Cell::decode(0x0001);
        assert_eq!(plain.palette, 0);
        assert!(!plain.horizontal_flip);
        assert!(!plain.vertical_flip);
        assert_eq!(plain.character, 1);
    }

    #[test]
    fn background_maps_are_eight_kilobytes_apart() {
        assert_eq!(Cell::address(0, 0), BG_MAPS);
        assert_eq!(Cell::address(1, 0), BG_MAPS + 8192);
        assert_eq!(Cell::address(0, 4095), BG_MAPS + 8190);
        assert_eq!(Cell::address(13, 0), BG_MAPS + 13 * 8192);
    }

    #[test]
    fn object_parallax_splits_the_eyes() {
        let object = Object::decode([10, (1 << 15) | (1 << 14) | 3, 20, 0]);

        assert_eq!(object.x, 10);
        assert_eq!(object.parallax, 3);
        assert_eq!(object.left_x(), 7);
        assert_eq!(object.right_x(), 13);
        assert!(object.left);
        assert!(object.right);
    }

    #[test]
    fn object_coordinates_sign_extend_from_ten_bits() {
        let object = Object::decode([0x03FF, 0x03FF, 0, 0]);
        assert_eq!(object.x, -1);
        assert_eq!(object.parallax, -1);
    }

    #[test]
    fn object_y_reaches_far_positive_before_it_turns_negative() {
        assert_eq!(Object::decode([0, 0, 0x00, 0]).y, 0);
        assert_eq!(Object::decode([0, 0, 0xE0, 0]).y, 224);
        assert_eq!(Object::decode([0, 0, 0xF8, 0]).y, -8);
        assert_eq!(Object::decode([0, 0, 0xFF, 0]).y, -1);
    }

    #[test]
    fn object_attributes_are_eight_bytes_apart() {
        assert_eq!(Object::address(0), OBJECT_ATTRIBUTES);
        assert_eq!(Object::address(1023), OBJECT_ATTRIBUTES + 1023 * 8);
    }

    fn world_words(header: u16) -> [u16; 16] {
        let mut words = [0u16; 16];
        words[0] = header;
        words
    }

    #[test]
    fn world_kinds_come_from_bgm() {
        for (bgm, expected) in [
            (0, WorldKind::Normal),
            (1, WorldKind::HBias),
            (2, WorldKind::Affine),
            (3, WorldKind::Object),
        ] {
            let world = World::decode(world_words(bgm << 12));
            assert_eq!(world.kind, expected);
        }
    }

    #[test]
    fn dummy_and_control_worlds_are_independent_of_kind() {
        let dummy = World::decode(world_words(0));
        assert!(dummy.dummy());
        assert!(!dummy.end);

        let live = World::decode(world_words(1 << 15));
        assert!(!live.dummy());

        let control = World::decode(world_words((1 << 15) | (1 << 6)));
        assert!(control.end);
        assert!(!control.dummy(), "end is not the same as dummy");
    }

    #[test]
    fn world_fields_land_in_the_right_halfwords() {
        let mut words = [0u16; 16];
        words[0] = (1 << 15) | (1 << 14) | (2 << 10) | (1 << 8) | (1 << 7) | 0x000B;
        words[1] = 0x0064;
        words[2] = 0x0008;
        words[3] = 0xFFF0;
        words[4] = 0x0010;
        words[5] = 0x0004;
        words[6] = 0x0020;
        words[7] = 0x017F;
        words[8] = 0x00DF;
        words[9] = 0x1000;
        words[10] = 0x0040;

        let world = World::decode(words);

        assert!(world.left && world.right);
        assert_eq!(world.map_width_power, 2);
        assert_eq!(world.map_height_power, 1);
        assert!(world.overplane);
        assert_eq!(world.map_base, 11);
        assert_eq!(world.destination_x, 100);
        assert_eq!(world.destination_parallax, 8);
        assert_eq!(world.destination_y, -16);
        assert_eq!(world.source_x, 16);
        assert_eq!(world.source_parallax, 4);
        assert_eq!(world.source_y, 32);
        assert_eq!(world.width(), 384);
        assert_eq!(world.height(), 224);
        assert_eq!(world.left_destination(), 92);
        assert_eq!(world.right_destination(), 108);
        assert_eq!(world.param_address(), BG_MAPS + 0x2000);
        assert_eq!(world.overplane_address(), BG_MAPS + 0x80);
    }

    #[test]
    fn the_base_map_rounds_down_to_a_multiple_of_the_background_size() {
        let mut words = [0u16; 16];
        words[0] = (1 << 10) | (1 << 8) | 11;
        let world = World::decode(words);

        assert_eq!(world.map_count(), 4);
        assert_eq!(world.effective_base(), 8);
    }

    #[test]
    fn oversized_backgrounds_pick_a_base_of_zero_or_eight() {
        let mut words = [0u16; 16];
        words[0] = (3 << 10) | (3 << 8) | 11;
        let world = World::decode(words);

        assert_eq!(world.map_count(), 64);
        assert_eq!(world.effective_base(), 8);

        words[0] = (3 << 10) | (3 << 8) | 6;
        assert_eq!(World::decode(words).effective_base(), 0);
    }

    #[test]
    fn short_background_worlds_are_raised_to_eight_pixels() {
        let mut words = [0u16; 16];
        words[8] = 3;
        assert_eq!(World::decode(words).height(), 8, "normal clamps up");

        words[0] = 2 << 12;
        assert_eq!(World::decode(words).height(), 4, "affine does not");
    }

    #[test]
    fn affine_width_is_unsigned_where_background_width_is_not() {
        let mut words = [0u16; 16];
        words[7] = 0x1FFF;
        assert_eq!(
            World::decode(words).width(),
            0,
            "13 bit signed, so minus one"
        );

        words[0] = 2 << 12;
        assert_eq!(World::decode(words).width(), 1024, "10 bit unsigned");
    }

    #[test]
    fn worlds_are_thirty_two_bytes_apart() {
        assert_eq!(World::address(0), WORLD_ATTRIBUTES);
        assert_eq!(World::address(31), WORLD_ATTRIBUTES + 31 * 32);
    }

    #[test]
    fn a_world_round_trips_through_memory() {
        let mut vip = Vip::new();
        vip.write_u16(World::address(5), (1 << 15) | (2 << 12));
        vip.write_u16(World::address(5) + 2, 40);

        let world = World::read(&vip, 5);
        assert_eq!(world.kind, WorldKind::Affine);
        assert_eq!(world.destination_x, 40);
    }

    #[test]
    fn hbias_reads_two_offsets_per_row() {
        let mut vip = Vip::new();
        let base = BG_MAPS;
        vip.write_u16(base, 5);
        vip.write_u16(base + 2, 0x1FFF);

        let bias = HBias::read(&vip, base, 0);
        assert_eq!(bias.left, 5);
        assert_eq!(bias.right, -1);
    }

    #[test]
    fn a_misaligned_hbias_base_reuses_the_left_offset() {
        let mut vip = Vip::new();
        let base = BG_MAPS + 2;
        vip.write_u16(base, 7);

        let bias = HBias::read(&vip, base, 0);
        assert_eq!(bias.left, 7);
        assert_eq!(bias.right, 7, "or-ing with 2 lands back on the left offset");
    }

    #[test]
    fn affine_parameters_read_five_fields() {
        let mut vip = Vip::new();
        let base = BG_MAPS;
        for (index, value) in [8u16, 0xFFFF, 16, 512, 0].iter().enumerate() {
            vip.write_u16(base + u32::try_from(index).unwrap() * 2, *value);
        }

        let affine = Affine::read(&vip, base, 0);
        assert_eq!(affine.source_x, 8);
        assert_eq!(affine.parallax, -1);
        assert_eq!(affine.source_y, 16);
        assert_eq!(affine.direction_x, 512);
        assert_eq!(affine.direction_y, 0);
    }

    #[test]
    fn affine_rows_are_sixteen_bytes_apart() {
        let mut vip = Vip::new();
        vip.write_u16(BG_MAPS + 16, 99);
        assert_eq!(Affine::read(&vip, BG_MAPS, 1).source_x, 99);
    }

    #[test]
    fn negative_parallax_shifts_only_the_left_eye() {
        let affine = Affine {
            source_x: 0,
            source_y: 0,
            parallax: -4,
            direction_x: 1 << DIRECTION_FRACTION_BITS,
            direction_y: 0,
        };

        let (left, _) = affine.column_source(0, false);
        let (right, _) = affine.column_source(0, true);

        assert_eq!(left, 4 << DIRECTION_FRACTION_BITS);
        assert_eq!(right, 0);
    }

    #[test]
    fn positive_parallax_shifts_only_the_right_eye() {
        let affine = Affine {
            source_x: 0,
            source_y: 0,
            parallax: 4,
            direction_x: 1 << DIRECTION_FRACTION_BITS,
            direction_y: 0,
        };

        let (left, _) = affine.column_source(0, false);
        let (right, _) = affine.column_source(0, true);

        assert_eq!(left, 0);
        assert_eq!(right, 4 << DIRECTION_FRACTION_BITS);
    }

    #[test]
    fn column_entries_split_repeat_from_length() {
        let entry = ColumnEntry::decode(0x03FE);
        assert_eq!(entry.repeat, 3);
        assert_eq!(entry.length, 0xFE);
        assert_eq!(entry.pulses(), 4);
        assert_eq!(entry.duration(), 0xFF);
    }

    #[test]
    fn the_two_column_tables_are_separate() {
        assert_eq!(ColumnEntry::address(false, 0), LEFT_COLUMN_TABLE);
        assert_eq!(ColumnEntry::address(true, 0), RIGHT_COLUMN_TABLE);
        assert_eq!(ColumnEntry::address(false, 255), LEFT_COLUMN_TABLE + 510);

        let mut vip = Vip::new();
        vip.write_u16(LEFT_COLUMN_TABLE, 0x00FE);
        vip.write_u16(RIGHT_COLUMN_TABLE, 0x0100);

        assert_eq!(ColumnEntry::read(&vip, false, 0).length, 0xFE);
        assert_eq!(ColumnEntry::read(&vip, true, 0).repeat, 1);
    }
}
