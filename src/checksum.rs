use core::fmt;

pub trait Checksum {
    type Output;

    fn new() -> Self;

    fn write(&mut self, data: &[u8]);

    fn finish(&self) -> Self::Output;
}

/// A CRC32 checksum.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Crc32(pub u32);

impl fmt::Debug for Crc32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Crc32 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{:08x}", self.0).serialize(serializer)
    }
}

/// Low 16 bits of a CRC32 checksum.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Crc32Low16(pub u16);

impl fmt::Debug for Crc32Low16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Crc32Low16 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{:04x}", self.0).serialize(serializer)
    }
}

/// Hasher for RAR15's CRC32 truncated to its low 16 bits.
#[derive(Debug)]
pub(crate) struct Crc32Low16Hasher {
    inner: crc32fast::Hasher,
}

impl Checksum for Crc32Low16Hasher {
    type Output = Crc32Low16;

    fn new() -> Self {
        Crc32Low16Hasher {
            inner: crc32fast::Hasher::new(),
        }
    }

    fn write(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finish(&self) -> Self::Output {
        Crc32Low16((self.inner.clone().finalize() & 0xFFFF) as u16)
    }
}

/// A custom checksum used in RAR 1.4 archives.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Checksum14(pub u16);

impl fmt::Debug for Checksum14 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Checksum14 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{:04x}", self.0).serialize(serializer)
    }
}

/// Represents an in-progress Checksum14 computation.
#[derive(Debug, Clone, Default)]
pub struct Checksum14Hasher(u16);

impl Checksum14Hasher {
    /// Create a new `Hasher`.
    pub fn new() -> Self {
        Checksum14Hasher(0)
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
