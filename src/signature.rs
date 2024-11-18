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
            Signature::Rar14 => RAR14,
            Signature::Rar15 => RAR15,
            Signature::Rar50 => RAR50,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Signature> {
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
}

pub fn read_signature<T: io::Read + io::Seek>(
    reader: &mut T,
) -> Result<Option<Signature>, io::Error> {
    let position = reader.stream_position()?;

    let marker: [u8; 8] = read_const_bytes(reader)?;

    let Some(format) = Signature::from_bytes(&marker) else {
        return Ok(None);
    };

    reader.seek(io::SeekFrom::Start(position + format.size()))?;
    Ok(Some(format))
}

pub const MAX_SFX_SIZE: usize = 0x200000;

pub fn search_signature<T: io::Read>(
    reader: &mut T,
) -> Result<Option<(Signature, u64)>, io::Error> {
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
                0 => Signature::Rar14,
                1 => Signature::Rar15,
                2 => Signature::Rar50,
                _ => unreachable!(),
            };

            Ok(Some((format, start as u64)))
        }
    }
}
