use std::io;

use aho_corasick::AhoCorasick;

use crate::read::read_const_bytes;

pub const RAR14: &[u8; 4] = b"RE\x7e\x5e";
pub const RAR15: &[u8; 7] = b"Rar!\x1a\x07\x00";
pub const RAR50: &[u8; 8] = b"Rar!\x1a\x07\x01\x00";

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
    pub const fn size(&self) -> u64 {
        self.signature().len() as u64
    }

    pub const fn signature(&self) -> &'static [u8] {
        match self {
            Self::Rar14 => RAR14,
            Self::Rar15 => RAR15,
            Self::Rar50 => RAR50,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(RAR14) {
            Some(Self::Rar14)
        } else if bytes.starts_with(RAR15) {
            Some(Self::Rar15)
        } else if bytes.starts_with(RAR50) {
            Some(Self::Rar50)
        } else {
            None
        }
    }

    pub fn from_stream_start<R: io::Read>(reader: &mut R) -> Result<Option<Self>, io::Error> {
        let marker: [u8; 8] = read_const_bytes(reader)?;
        Ok(Signature::from_bytes(&marker))
    }

    pub const MAX_SFX_SIZE: usize = 0x200000;

    pub fn search_stream<T: io::Read>(reader: &mut T) -> Result<Option<(Self, u64)>, io::Error> {
        let patterns = [&RAR14[..], &RAR15[..], &RAR50[..]];

        let Ok(ac) = AhoCorasick::new(patterns) else {
            unreachable!()
        };

        match ac.stream_find_iter(reader).next() {
            None => Ok(None),
            Some(Err(e)) => Err(e),
            Some(Ok(m)) => {
                let start = m.start();

                let format = match m.pattern().as_i32() {
                    0 => Self::Rar14,
                    1 => Self::Rar15,
                    2 => Self::Rar50,
                    _ => unreachable!(),
                };

                Ok(Some((format, start as u64)))
            }
        }
    }
}
