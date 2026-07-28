#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Vip,
    Communication,
    GamePak,
    TimerZero,
    GamePad,
}

// highest priority first, the scan takes the first match
pub const SOURCES: [Source; 5] = [
    Source::Vip,
    Source::Communication,
    Source::GamePak,
    Source::TimerZero,
    Source::GamePad,
];

impl Source {
    pub fn code(self) -> u16 {
        match self {
            Source::Vip => 0xFE40,
            Source::Communication => 0xFE30,
            Source::GamePak => 0xFE20,
            Source::TimerZero => 0xFE10,
            Source::GamePad => 0xFE00,
        }
    }

    pub fn level(self) -> u32 {
        match self {
            Source::Vip => 4,
            Source::Communication => 3,
            Source::GamePak => 2,
            Source::TimerZero => 1,
            Source::GamePad => 0,
        }
    }

    pub fn handler(self) -> u32 {
        0xFFFF_0000 | u32::from(self.code())
    }

    pub fn name(self) -> &'static str {
        match self {
            Source::Vip => "vip",
            Source::Communication => "communication",
            Source::GamePak => "game pak",
            Source::TimerZero => "timer zero",
            Source::GamePad => "game pad",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_matches_the_reference() {
        assert_eq!(Source::Vip.code(), 0xFE40);
        assert_eq!(Source::Vip.level(), 4);
        assert_eq!(Source::Vip.handler(), 0xFFFF_FE40);

        assert_eq!(Source::TimerZero.code(), 0xFE10);
        assert_eq!(Source::TimerZero.level(), 1);
        assert_eq!(Source::TimerZero.handler(), 0xFFFF_FE10);

        assert_eq!(Source::GamePad.code(), 0xFE00);
        assert_eq!(Source::GamePad.level(), 0);
        assert_eq!(Source::GamePad.handler(), 0xFFFF_FE00);
    }

    #[test]
    fn priority_order_descends_by_level() {
        for pair in SOURCES.windows(2) {
            assert!(
                pair[0].level() > pair[1].level(),
                "{} must outrank {}",
                pair[0].name(),
                pair[1].name()
            );
        }
    }
}
