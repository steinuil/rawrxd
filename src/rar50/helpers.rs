use std::io;

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
