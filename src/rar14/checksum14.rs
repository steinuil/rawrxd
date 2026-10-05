/// A custom checksum used in RAR 1.4 archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checksum14(pub u16);

/// Represents an in-progress Checksum14 computation.
#[derive(Debug, Clone, Default)]
pub struct Hasher(u16);

impl Hasher {
    /// Create a new `Hasher`.
    pub fn new() -> Self {
        Hasher(0)
    }

    /// Process the given byte slice and update the hash state.
    pub fn update(&mut self, data: &[u8]) {
        self.0 = data.iter().fold(self.0, |checksum, byte| {
            checksum.wrapping_add(*byte as u16).rotate_left(1)
        });
    }

    pub fn finalize(self) -> Checksum14 {
        Checksum14(self.0)
    }
}
