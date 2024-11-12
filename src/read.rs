use std::io;

pub fn read_u8<R: io::Read>(r: &mut R) -> io::Result<u8> {
    let mut buf = [0; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16<R: io::Read>(r: &mut R) -> io::Result<u16> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32<R: io::Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_u64<R: io::Read>(r: &mut R) -> io::Result<u64> {
    let mut buf = [0; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}
