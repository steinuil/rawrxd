pub fn parse(dos_time: u32) -> Result<time::PrimitiveDateTime, time::error::ComponentRange> {
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
