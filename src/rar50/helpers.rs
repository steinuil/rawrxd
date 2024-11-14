use std::{io, ops::Range};

use crate::{read::*, time_conv};

pub fn read_unix_time_nanos<R: io::Read>(
    reader: &mut R,
) -> io::Result<Result<time::OffsetDateTime, u64>> {
    let nanos = read_u64(reader)?;
    Ok(time_conv::parse_unix_timestamp_ns(nanos).map_err(|_| nanos))
}

pub fn read_unix_time_sec<R: io::Read>(
    reader: &mut R,
) -> io::Result<Result<time::OffsetDateTime, u32>> {
    let seconds = read_u32(reader)?;
    Ok(time_conv::parse_unix_timestamp_sec(seconds).map_err(|_| seconds))
}

pub fn read_windows_time<R: io::Read>(
    reader: &mut R,
) -> io::Result<Result<time::OffsetDateTime, u64>> {
    let filetime = read_u64(reader)?;
    Ok(time_conv::parse_windows_filetime(filetime).map_err(|_| filetime))
}

const MAPPED_STRING_MARK: char = '\u{fffe}';
const MAP_CHAR: char = '\u{e000}';
const MAP_RANGE: Range<char> = '\u{e080}'..'\u{e100}';

// This is not a bug-for-bug implementation of unrar's WideToCharMap,
// because it'll "fix" high ascii characters to their UTF-8 version.
pub fn unmap_high_ascii_chars(buf: Vec<u8>) -> Result<String, Vec<u8>> {
    let mut string = String::from_utf8(buf).map_err(|e| e.into_bytes())?;

    if string.contains(MAPPED_STRING_MARK) {
        string = string
            .chars()
            .filter_map(|c| {
                if MAP_RANGE.contains(&c) {
                    char::from_u32(c as u32 - MAP_CHAR as u32)
                } else if c == MAPPED_STRING_MARK {
                    None
                } else {
                    Some(c)
                }
            })
            .collect();
    }

    Ok(string)
}

#[test]
fn test_conv_file_name() {
    let high_ascii_file_name = vec![239, 191, 190, 238, 131, 134];
    assert_eq!(unmap_high_ascii_chars(high_ascii_file_name).unwrap(), "Æ");
}
