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

const MAX_VINT_SIZE: usize = 10;

/// Read a variable-size integer and return the int and size in bytes.
/// The lower 7 bits of every byte contain integer data, and the highest bit
/// acts as a continuation flag.
pub fn read_vint<R: io::Read>(r: &mut R) -> io::Result<(u64, u8)> {
    let mut vint: u64 = 0;

    for i in 0..MAX_VINT_SIZE {
        let shift = i * 7;
        let byte = read_u8(r)?;
        vint |= ((byte & !0x80) as u64) << shift;
        if (byte & 0x80) == 0 {
            return Ok((vint, i as u8 + 1));
        }
    }

    // TODO we should probably log a warning here
    Ok((vint, MAX_VINT_SIZE as u8))
}

pub fn read_const_bytes<const N: usize, R: io::Read>(r: &mut R) -> io::Result<[u8; N]> {
    let mut buf = [0; N];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_vec<R: io::Read>(r: &mut R, size: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0; size];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_string<R: io::Read>(
    reader: &mut R,
    size: usize,
) -> io::Result<Result<String, Vec<u8>>> {
    let str = read_vec(reader, size)?;
    Ok(String::from_utf8(str).map_err(|e| e.into_bytes()))
}

pub fn read_vint_sized<R: io::Read>(reader: &mut R, size: u8) -> io::Result<u64> {
    let mut num = 0;

    for i in 0..size {
        let byte = read_u8(reader)? as u64;
        num |= byte << (i * 8);
    }

    Ok(num)
}
