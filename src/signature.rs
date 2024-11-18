use std::io;

use aho_corasick::AhoCorasick;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    /// RAR 1.4
    Rar14,

    /// RAR 1.5 to 4
    Rar15,

    /// RAR 5+
    Rar50,
}

impl Signature {
    pub const RAR14: &[u8; 4] = b"RE\x7e\x5e";
    pub const RAR15: &[u8; 7] = b"Rar!\x1a\x07\x00";
    pub const RAR50: &[u8; 8] = b"Rar!\x1a\x07\x01\x00";

    /// Byte size of the signature.
    pub const fn size(&self) -> u64 {
        self.signature().len() as u64
    }

    /// The byte signature corresponding to the format.
    pub const fn signature(&self) -> &'static [u8] {
        match self {
            Self::Rar14 => Self::RAR14,
            Self::Rar15 => Self::RAR15,
            Self::Rar50 => Self::RAR50,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(Self::RAR14) {
            Some(Self::Rar14)
        } else if bytes.starts_with(Self::RAR15) {
            Some(Self::Rar15)
        } else if bytes.starts_with(Self::RAR50) {
            Some(Self::Rar50)
        } else {
            None
        }
    }

    /// The maximum size of the SFX binary embedded before the archive signature, including
    /// the signature size. If the end of the signature exceeds this offset then this is not
    /// a valid RAR archive.
    pub const MAX_SFX_SIZE: u64 = 0x200000;

    /// Search for a RAR signature in the stream up to `MAX_SFX_SIZE` and return the format
    /// version and the offset of the signature in the file.
    ///
    /// The first block of the archive starts at `offset + format.size()`.
    ///
    /// The RAR signature generally starts at offset 0 in a normal .rar archive, but RAR files
    /// can also be constructed as a *SFX* (self-extracting archive) which embed the binary
    /// needed to extract the archive before the archive itself. This binary may have a size up to
    /// `MAX_SFX_SIZE`, so we need to search for the signature within these bounds.
    pub fn search_stream<R: io::Read>(reader: R) -> Result<Option<(Self, u64)>, io::Error> {
        let patterns = [&Self::RAR14[..], &Self::RAR15[..], &Self::RAR50[..]];

        let Ok(ac) = AhoCorasick::new(patterns) else {
            unreachable!("Aho-Corasick pattern not constructed correctly")
        };

        // Avoid reading the whole file in case we don't find the signature within the
        // MAX_SFX_SIZE.
        let bounded_reader = &mut reader.take(Self::MAX_SFX_SIZE);

        match ac.stream_find_iter(bounded_reader).next() {
            None => Ok(None),
            Some(Err(e)) => Err(e),
            Some(Ok(m)) => {
                let start = m.start();

                let format = match m.pattern().as_i32() {
                    0 => Self::Rar14,
                    1 => Self::Rar15,
                    2 => Self::Rar50,
                    i => unreachable!("invalid Aho-Corasick pattern ID: {i}"),
                };

                Ok(Some((format, start as u64)))
            }
        }
    }
}
