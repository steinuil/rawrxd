use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Version14,
    Version15,
    Version50,
}

impl Format {
    pub fn signature_size(&self) -> usize {
        match self {
            Format::Version14 => 4,
            Format::Version15 => 7,
            Format::Version50 => 8,
        }
    }
}

// const MAX_SFX_SIZE: usize = 0x200000;

pub fn is_archive(file: &mut fs::File) -> Result<(Format, usize), io::Error> {
    let mut header_mark = [0; 8];
    let read = file.read(&mut header_mark)?;
    match &header_mark[..] {
        [b'R', b'E', 0x7e, 0x5e, _, _, _, _] => Ok((Format::Version14, 0)),
        [b'R', b'a', b'r', b'!', 0x1a, 7, 0, _] if read >= 7 => Ok((Format::Version15, 0)),
        [b'R', b'a', b'r', b'!', 0x1a, 7, 1, 0] if read >= 8 => Ok((Format::Version50, 0)),
        [b'R', b'a', b'r', b'!', 0x1a, 7, v, 0] if read >= 8 && v > &1 && v < &5 => {
            todo!("future version of rar format")
        }
        _ => todo!("might be an SFX or not an archive"),
    }
}

fn read_u8<T: io::Read>(r: &mut BufReader<T>) -> io::Result<u8> {
    let mut buf = [0; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16<T: io::Read>(r: &mut BufReader<T>) -> io::Result<u16> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32<T: io::Read>(r: &mut BufReader<T>) -> io::Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

#[derive(Debug)]
pub struct MainBlock {
    crc: u16,
    flags: u16,
    high_pos_av: u16,
    pos_av: u32,
    comment: Option<Vec<u8>>,
}

impl MainBlock {
    pub fn new(
        crc: u16,
        flags: u16,
        high_pos_av: u16,
        pos_av: u32,
        comment: Option<Vec<u8>>,
    ) -> Self {
        MainBlock {
            crc,
            flags,
            high_pos_av,
            pos_av,
            comment,
        }
    }

    pub fn volume(&self) -> bool {
        self.flags & 0x0001 != 0
    }
}

pub fn read_block15<T: io::Read + Seek>(reader: &mut BufReader<T>) -> io::Result<usize> {
    let crc = read_u16(reader)?;
    let header_type = read_u8(reader)?;
    let flags = read_u16(reader)?;
    let head_size = read_u16(reader)?;

    // TODO: Check that head_size is >= 7

    match header_type {
        // HEAD3_MAIN
        0x73 if head_size >= 13 => {
            let high_pos_av = read_u16(reader)?;
            let pos_av = read_u32(reader)?;

            let block = MainBlock::new(crc, flags, high_pos_av, pos_av, None);

            dbg!(block);

            Ok(head_size as usize)
        }
        0x74 => {
            let data_size = read_u32(reader)?;
            let low_unp_size = read_u32(reader)?;
            let host_os = read_u8(reader)?;

            let file_crc32 = read_u32(reader)?;
            let file_time = read_u32(reader)?;
            let unp_ver = read_u8(reader)?;

            let method = read_u8(reader)? - 0x30;
            let name_size = read_u16(reader)? as usize;
            let file_attr = read_u32(reader)?;

            let mut file_name = vec![0; name_size];
            reader.read_exact(&mut file_name)?;
            let name = String::from_utf8(file_name.to_owned()).unwrap();
            println!("{}", name);

            Ok(head_size as usize + data_size as usize)
        }
        0x7a => {
            println!("HEAD3_SERVICE");
            let data_size = read_u32(reader)?;
            let low_unp_size = read_u32(reader)?;
            let host_os = read_u8(reader)?;

            let file_crc32 = read_u32(reader)?;
            let file_time = read_u32(reader)?;
            let unp_ver = read_u8(reader)?;

            let method = read_u8(reader)? - 0x30;
            let name_size = read_u16(reader)? as usize;
            let file_attr = read_u32(reader)?;

            // Large file
            if flags & 0x100 != 0 {
                let high_pack_size = read_u32(reader)?;
                let high_unp_size = read_u32(reader)?;
            }

            let mut file_name = vec![0; name_size];
            reader.read_exact(&mut file_name)?;

            let name = String::from_utf8(file_name.to_owned()).unwrap();
            println!("{}", name);

            Ok(head_size as usize + data_size as usize)
        }
        0x7b => {
            println!("HEAD3_ENDARC");
            Ok(head_size as usize)
        }
        _ => todo!("other header types: {:x}", header_type),
    }
}

#[test]
fn test_rar_version() {
    let mut f = fs::File::open("fixtures/testfile.rar3.av.rar").unwrap();
    assert_eq!((Format::Version15, 0), is_archive(&mut f).unwrap());
}

fn main() {
    let mut args = std::env::args();

    let filename = args.nth(1).unwrap();
    let mut f = fs::File::open(&filename).unwrap();
    let (format, start) = is_archive(&mut f).unwrap();
    f.seek(io::SeekFrom::Start(
        (start + format.signature_size()) as u64,
    ))
    .unwrap();

    let mut fbuf = BufReader::new(f);

    loop {
        let pos = fbuf.stream_position().unwrap();
        let size = read_block15(&mut fbuf).unwrap();
        fbuf.seek(SeekFrom::Start(pos + size as u64)).unwrap();
    }
}
