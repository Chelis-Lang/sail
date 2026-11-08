// Profile three: calls to admitted functions, integer casts and match in bodies.

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Low,
    High,
}

#[derive(Debug)]
pub struct Meter {
    pub reading: u32,
    pub limit: u32,
}

#[sail_contracts::satisfies("BODY-CLAMP")]
#[sail_contracts::requires(limit > 0)]
#[sail_contracts::ensures(|ret| ret <= limit && (value > limit || ret == value))]
pub fn clamp(value: u32, limit: u32) -> u32 {
    if value > limit { limit } else { value }
}

#[sail_contracts::satisfies("BODY-LEVEL")]
#[sail_contracts::ensures(|ret| ret <= 255)]
pub fn level(value: u32) -> u16 {
    let clamped = clamp(value, 255);
    clamped as u16
}

#[sail_contracts::satisfies("BODY-RANK")]
#[sail_contracts::ensures(|ret| ret <= 2 && ((*mode == Mode::Off) == (ret == 0)))]
pub fn rank(mode: &Mode) -> u8 {
    match mode {
        Mode::Off => 0,
        Mode::Low => 1,
        Mode::High => 2,
    }
}

#[sail_contracts::satisfies("BODY-BUCKET")]
#[sail_contracts::ensures(|ret| ret <= 3 && (value != 0 || ret == 0) && (value < 12 || ret == 3))]
pub fn bucket(value: u8) -> u8 {
    match value {
        0 => 0,
        1..=9 => 1,
        10 | 11 => 2,
        _ => 3,
    }
}

impl Meter {
    #[sail_contracts::satisfies("BODY-HEADROOM")]
    #[sail_contracts::requires(self.reading <= self.limit)]
    #[sail_contracts::ensures(|ret| ret == self.limit - self.reading)]
    pub fn headroom(&self) -> u32 {
        self.limit - self.reading
    }

    #[sail_contracts::satisfies("BODY-FULL")]
    #[sail_contracts::requires(self.reading <= self.limit)]
    #[sail_contracts::ensures(|ret| ret == (self.reading == self.limit))]
    pub fn full(&self) -> bool {
        self.headroom() == 0
    }

    #[sail_contracts::satisfies("BODY-LEVEL")]
    #[sail_contracts::ensures(|ret| ret <= 255)]
    pub fn display(&self) -> u16 {
        level(Self::raw(self))
    }

    #[sail_contracts::satisfies("BODY-RAW")]
    #[sail_contracts::ensures(|ret| ret == meter.reading)]
    pub fn raw(meter: &Meter) -> u32 {
        meter.reading
    }
}
