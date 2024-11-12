use std::io;

use crate::format::Format;

pub fn detect_format(marker: &[u8; 8]) -> Option<Format> {
    match marker {
        [b'R', b'E', 0x7e, 0x5e, _, _, _, _] => Some(Format::Rar14),
        [b'R', b'a', b'r', b'!', 0x1a, 7, 0, _] => Some(Format::Rar15),
        [b'R', b'a', b'r', b'!', 0x1a, 7, 1, 0] => Some(Format::Rar50),
        _ => None,
    }
}

// TODO if it's an SFX (self-extracting executable) search for the markers
// const MAX_SFX_SIZE: usize = 0x200000;

pub fn read_signature<T: io::Read + io::Seek>(reader: &mut T) -> Result<Option<Format>, io::Error> {
    let position = reader.stream_position()?;

    let mut marker = [0; 8];
    reader.read_exact(&mut marker)?;

    let Some(format) = detect_format(&marker) else {
        return Ok(None);
    };

    reader.seek(io::SeekFrom::Start(position + format.signature_size()))?;
    Ok(Some(format))
}
