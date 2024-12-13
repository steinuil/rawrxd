//! "Extended time" is stored in a slightly cursed format due to the fact
//! that timestamps in RAR15 are stored in DOS format, which has a precision
//! of two seconds.
//!
//! First we have an u16 containing 4 bitfields, one for each timestamp type:
//!
//! | Bits  | Name               |
//! | ----- | ------------------ |
//! | 15-12 | mtime flags        |
//! | 11-8  | ctime flags        |
//! | 7-4   | atime flags        |
//! | 3-0   | archive time flags |
//!
//! Each of these bitfields has this format:
//!
//! | Mask | Name                                  |
//! | ---- | ------------------------------------- |
//! | 0x08 | Timestamp is present                  |
//! | 0x04 | Add one second to the timestamp       |
//! | 0x03 | Precision of the nanosecond timestamp |
//!
//! If the 0x08 flag is not set, the timestamp and all other fields are not present.
//!
//! If the 0x04 flag is set, add one second to the timestamp.
//!
//! The two lower bits indicate the byte size of the nanoseconds field that follows,
//! for a maximum of 3 bytes.
//!
//! Following the flags, if the 0x08 flag is set and the current timestamp is not
//! the mtime (which has already been read before), we have a u32 containing
//! the base timestamp in DOS format.
//!
//! After this we have N bytes containing an increment in 100ns, where N
//! is the precision field from the bitfield.
//! This increment is then left shifted by (3 - N) * 8 and added to
//! the resulting timestamp.

// TODO need to test this part on a real archive with this field.

use std::io;

use crate::{read::*, time_conv};

#[derive(Debug)]
pub struct ExtendedTime {
    pub modification_time: Result<time::PrimitiveDateTime, u32>,
    pub creation_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub access_time: Option<Result<time::PrimitiveDateTime, u32>>,

    // UnRAR says this is never used, but it doesn't hurt to try.
    pub archive_time: Option<Result<time::PrimitiveDateTime, u32>>,
}

#[derive(Debug)]
struct ExtendedTimeFlags(u8);

impl ExtendedTimeFlags {
    const EXISTS: u8 = 0x8;
    const ADD_SECOND: u8 = 0x4;
    const PRECISION_MASK: u8 = 0x3;

    const MAX_PRECISION: u8 = 3;

    fn shifted(flags: u16, shift: u8) -> Self {
        Self((flags >> (shift * 4)) as u8 & 0xF)
    }

    fn exists(&self) -> bool {
        self.0 & Self::EXISTS != 0
    }

    fn add_second(&self) -> bool {
        self.0 & Self::ADD_SECOND != 0
    }

    fn hundred_nanos_increment_precision(&self) -> u8 {
        self.0 & Self::PRECISION_MASK
    }
}

impl ExtendedTime {
    pub fn read<R: io::Read>(
        reader: &mut R,
        modification_time: Result<time::PrimitiveDateTime, u32>,
    ) -> io::Result<Self> {
        let all_flags = read_u16(reader)?;

        // We don't need to read mtime because it's already been read before.
        let flags = ExtendedTimeFlags::shifted(all_flags, 3);
        let modification_time = match (modification_time, flags.exists()) {
            (Ok(t), true) => Ok(read_extended_time_increments(reader, t, flags)?),
            (t, _) => t,
        };

        let creation_time = read_extended_time(reader, ExtendedTimeFlags::shifted(all_flags, 2))?;
        let access_time = read_extended_time(reader, ExtendedTimeFlags::shifted(all_flags, 1))?;
        let archive_time = read_extended_time(reader, ExtendedTimeFlags::shifted(all_flags, 0))?;

        Ok(ExtendedTime {
            modification_time,
            creation_time,
            access_time,
            archive_time,
        })
    }
}

/// Read a u32 DOS timestamp and add the extended time increments.
fn read_extended_time<R: io::Read>(
    reader: &mut R,
    flags: ExtendedTimeFlags,
) -> io::Result<Option<Result<time::PrimitiveDateTime, u32>>> {
    Ok(if flags.exists() {
        let time = read_u32(reader)?;

        Some(match time_conv::parse_dos_datetime(time) {
            Ok(time) => Ok(read_extended_time_increments(reader, time, flags)?),
            Err(_) => Err(time),
        })
    } else {
        None
    })
}

/// Read the extended time increments and add them to the timestamp.
fn read_extended_time_increments<R: io::Read>(
    reader: &mut R,
    mut t: time::PrimitiveDateTime,
    flags: ExtendedTimeFlags,
) -> io::Result<time::PrimitiveDateTime> {
    if flags.add_second() {
        t = t.saturating_add(time::Duration::SECOND);
    }

    let precision = flags.hundred_nanos_increment_precision();
    let hundred_nanos = read_extended_time_hundred_nanos(reader, precision)?;
    let nanos = hundred_nanos * 100;

    Ok(t.saturating_add(time::Duration::nanoseconds(nanos as _)))
}

/// Read a `size`-sized int and shift it by `ExtendedTimeFlags::MAX_PRECISION - size` bytes.
fn read_extended_time_hundred_nanos<R: io::Read>(reader: &mut R, size: u8) -> io::Result<u32> {
    let mut num = read_vint_sized(reader, size)? as u32;
    num <<= (ExtendedTimeFlags::MAX_PRECISION - size) * 8;
    Ok(num)
}

#[test]
fn test_read_extended_time_nanos() -> io::Result<()> {
    let mut reader = io::Cursor::new(vec![0xFF, 0xEE, 0xDD]);
    assert_eq!(read_extended_time_hundred_nanos(&mut reader, 1)?, 0xFF0000);
    reader.set_position(0);
    assert_eq!(read_extended_time_hundred_nanos(&mut reader, 3)?, 0xDDEEFF);
    Ok(())
}
