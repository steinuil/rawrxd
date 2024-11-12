#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// RAR 1.4
    Rar14,

    /// RAR 1.5 to 4
    Rar15,

    /// RAR 5+
    Rar50,
}

impl Format {
    pub const fn signature_size(&self) -> u64 {
        match self {
            Format::Rar14 => 4,
            Format::Rar15 => 7,
            Format::Rar50 => 8,
        }
    }
}
