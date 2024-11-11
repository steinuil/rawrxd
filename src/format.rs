#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Version14,
    Version15,
    Version50,
}

impl Format {
    pub const fn signature_size(&self) -> usize {
        match self {
            Format::Version14 => 4,
            Format::Version15 => 7,
            Format::Version50 => 8,
        }
    }
}
