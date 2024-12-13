use std::time::Duration;

/// Parse an MS-DOS datetime value.
///
/// Note: the time value only has a precision of two seconds.
/// https://learn.microsoft.com/en-us/windows/win32/sysinfo/ms-dos-date-and-time
pub fn parse_dos_datetime(
    dos_time: u32,
) -> Result<time::PrimitiveDateTime, time::error::ComponentRange> {
    let second = ((dos_time & 0x1f) * 2) as u8;
    let minute = ((dos_time >> 5) & 0x3f) as u8;
    let hour = ((dos_time >> 11) & 0x1f) as u8;
    let time = time::Time::from_hms(hour, minute, second)?;

    let day = ((dos_time >> 16) & 0x1f) as u8;
    let month = ((dos_time >> 21) & 0x0f) as u8;
    let year = ((dos_time >> 25) + 1980) as i32;
    let date = time::Date::from_calendar_date(year, month.try_into()?, day)?;

    Ok(time::PrimitiveDateTime::new(date, time))
}

const ONE_SECOND_NS: i128 = Duration::from_secs(1).as_nanos() as _;

// Values from this StackOverflow answer
// https://stackoverflow.com/questions/6161776/convert-windows-filetime-to-second-in-unix-linux
const WINDOWS_TICK_NS: i128 = 100;
const WINDOWS_EPOCH_DIFFERENCE: i128 = 11_644_473_600 * ONE_SECOND_NS;

/// Parse a Windows FILETIME structure.
///
/// https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-filetime
pub fn parse_windows_filetime(
    filetime: u64,
) -> Result<time::OffsetDateTime, time::error::ComponentRange> {
    let unix_timestamp_ns = (filetime as i128) * WINDOWS_TICK_NS - WINDOWS_EPOCH_DIFFERENCE;
    time::OffsetDateTime::from_unix_timestamp_nanos(unix_timestamp_ns)
}

#[test]
fn test_parse_windows_filetime() {
    assert_eq!(
        format!("{}", parse_windows_filetime(128166372003061629).unwrap()),
        "2007-02-22 17:00:00.3061629 +00:00:00",
    );
    assert_eq!(
        format!("{}", parse_windows_filetime(0).unwrap()),
        "1601-01-01 0:00:00.0 +00:00:00"
    );
}

pub fn parse_unix_timestamp_sec(
    seconds: u32,
) -> Result<time::OffsetDateTime, time::error::ComponentRange> {
    time::OffsetDateTime::from_unix_timestamp(seconds.into())
}

pub fn parse_unix_timestamp_ns(
    nanoseconds: u64,
) -> Result<time::OffsetDateTime, time::error::ComponentRange> {
    time::OffsetDateTime::from_unix_timestamp_nanos(nanoseconds.into())
}
