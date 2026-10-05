use crate::checksum::Checksum;

/// Hasher for RAR15's CRC32 truncated to its low 16 bits.
#[derive(Debug)]
pub struct Hasher {
    inner: crc32fast::Hasher,
}

impl Checksum for Hasher {
    type Output = u16;

    fn new() -> Self {
        Hasher {
            inner: crc32fast::Hasher::new(),
        }
    }

    fn write(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finish(&self) -> Self::Output {
        (self.inner.clone().finalize() & 0xFFFF) as u16
    }
}
