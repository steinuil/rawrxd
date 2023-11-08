mod decompress;

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

pub fn is_archive<R: io::Read>(file: &mut R) -> Result<(Format, usize), io::Error> {
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

fn read_u8<R: io::Read>(r: &mut R) -> io::Result<u8> {
    let mut buf = [0; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16<R: io::Read>(r: &mut R) -> io::Result<u16> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32<R: io::Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

#[derive(Debug)]
pub struct MainBlock {
    pub position: u64,
    pub header_size: u64,
    crc: u16,
    pub flags: MainBlockFlags,
    high_pos_av: u16,
    pos_av: u32,
}

impl MainBlock {
    pub fn new(
        position: u64,
        header_size: u64,
        crc: u16,
        flags: MainBlockFlags,
        high_pos_av: u16,
        pos_av: u32,
    ) -> Self {
        MainBlock {
            position,
            header_size,
            crc,
            flags,
            high_pos_av,
            pos_av,
        }
    }

    pub fn print_info(&self) {
        println!("- type: main");
        println!("  position: {:#x}", self.position);
        println!("  size: {}", self.header_size);
        println!("  crc: {:#04x}", self.crc);
        println!("  flags: {:#016b}", self.flags.0);
        println!("  high_pos_av: {}", self.high_pos_av);
        println!("  pos_av: {:#x}", self.pos_av);
    }
}

#[derive(Debug, Clone)]
pub struct FileBlock {
    pub position: u64,
    pub header_size: u64,
    pub file_size: u64,
    pub mtime: time::PrimitiveDateTime,
    pub file_name: Vec<u8>,
    pub host_system: HostSystem,
}

impl FileBlock {
    pub fn print_info(&self) {
        println!("- type: file");
        println!("  position: {:#x}", self.position);
        println!("  header_size: {}", self.header_size);
        println!("  host_system: {:?}", self.host_system);
        println!(
            "  file_name: {}",
            std::str::from_utf8(&self.file_name).unwrap()
        );
        println!("  file_size: {}", self.file_size);
        println!("  mtime: {}", self.mtime);
    }
}

#[derive(Debug, Clone)]
pub struct ServiceBlock {
    pub position: u64,
    pub header_size: u64,
    pub name: Vec<u8>,
    pub data: Vec<u8>,
}

impl ServiceBlock {
    pub fn print_info(&self) {
        println!("- type: service");
        println!("  position: {:#x}", self.position);
        println!("  header_size: {}", self.header_size);
        println!("  data_size: {}", self.data.len());
        println!("  name: {}", std::str::from_utf8(&self.name).unwrap());
    }
}

/// Might not be present in older archives
#[derive(Debug, Clone)]
pub struct EndArchiveBlock {
    pub position: u64,
    pub header_size: u64,
    pub flags: EndArchiveBlockFlags,
    pub data_crc: Option<u32>,
    pub volume_number: Option<u16>,
}

impl EndArchiveBlock {
    pub fn print_info(&self) {
        println!("- type: end_archive");
        println!("  position: {:#x}", self.position);
        println!("  header_size: {}", self.header_size);
        println!("  is_last_volume: {}", self.flags.is_last_volume());
        println!("  has_rev_space: {}", self.flags.has_rev_space());
        if let Some(data_crc) = self.data_crc {
            println!("  crc: {:#08x}", data_crc);
        }
        if let Some(volume_number) = self.volume_number {
            println!("  volume_number: {}", volume_number);
        }
    }
}

pub enum Block {
    Main(MainBlock),
    File(FileBlock),
    Service(ServiceBlock),
    EndArchive(EndArchiveBlock),
}

impl Block {
    pub fn position(&self) -> u64 {
        match self {
            Block::Main(block) => block.position,
            Block::File(block) => block.position,
            Block::Service(block) => block.position,
            Block::EndArchive(block) => block.position,
        }
    }

    pub fn size(&self) -> u64 {
        match self {
            Block::Main(block) => block.header_size,
            Block::File(block) => block.header_size + block.file_size,
            Block::Service(block) => block.header_size,
            Block::EndArchive(block) => block.header_size,
        }
    }

    pub fn print_info(&self) {
        match self {
            Block::Main(block) => block.print_info(),
            Block::File(block) => block.print_info(),
            Block::Service(block) => block.print_info(),
            Block::EndArchive(block) => block.print_info(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MainBlockFlags(u16);

impl MainBlockFlags {
    const VOLUME: u16 = 0x0001;
    const PACK_COMMENT: u16 = 0x0002;
    const LOCK: u16 = 0x0004;
    const SOLID: u16 = 0x0008;
    const NEWNUMBERING: u16 = 0x0010;
    const AUTHENTICITY: u16 = 0x0020;
    const PROTECT: u16 = 0x0040;
    const PASSWORD: u16 = 0x0080;
    const FIRSTVOLUME: u16 = 0x0100;

    pub fn new(flags: u16) -> Self {
        MainBlockFlags(flags)
    }

    /// Old style (up to RAR 2.9) main archive comment embedded into
    /// the main archive header.
    pub fn has_old_style_comment(&self) -> bool {
        self.0 & Self::PACK_COMMENT != 0
    }

    /// TODO does this mean that authenticity is set in the main header?
    /// or just that it exists in the archive?
    /// Only present up to RAR 3.0
    pub fn has_authenticity_information(&self) -> bool {
        self.0 & Self::AUTHENTICITY != 0
    }

    /// A multi-volume archive is an archive split into multiple files.
    pub fn is_volume(&self) -> bool {
        self.0 & Self::VOLUME != 0
    }

    /// https://en.wikipedia.org/wiki/Solid_compression
    pub fn is_solid(&self) -> bool {
        self.0 & Self::SOLID != 0
    }

    /// A locked archive is just an archive with this flag set,
    /// and when it is set WinRAR will refuse to modify it.
    pub fn is_locked(&self) -> bool {
        self.0 & Self::LOCK != 0
    }

    pub fn is_password_protected(&self) -> bool {
        self.0 & Self::PROTECT != 0
    }

    /// Block headers are encrypted
    pub fn is_encrypted(&self) -> bool {
        self.0 & Self::PASSWORD != 0
    }

    /// Set only by RAR 3.0+
    pub fn is_first_volume(&self) -> bool {
        self.0 & Self::FIRSTVOLUME != 0
    }

    /// In multi-volume archives, old numbering looks like this:
    ///
    /// - archive.rar
    /// - archive.r00
    /// - archive.r01
    /// - ...
    ///
    /// With the new numbering scheme, all volumes use the .rar extension.
    ///
    /// - archive.part01.rar
    /// - archive.part02.rar
    /// - ...
    pub fn uses_new_numbering(&self) -> bool {
        self.0 & Self::NEWNUMBERING != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EndArchiveBlockFlags(u16);

impl EndArchiveBlockFlags {
    const NEXT_VOLUME: u16 = 0x0001;
    const DATACRC: u16 = 0x0002;
    const REVSPACE: u16 = 0x0004;
    const VOLNUMBER: u16 = 0x0008;

    pub fn new(flags: u16) -> Self {
        EndArchiveBlockFlags(flags)
    }

    pub fn is_last_volume(&self) -> bool {
        self.0 & Self::NEXT_VOLUME == 0
    }

    /// Store CRC32 of RAR archive (now is used only in volumes).
    pub fn has_data_crc(&self) -> bool {
        self.0 & Self::DATACRC != 0
    }

    /// Reserve space for end of REV file 7 byte record.
    pub fn has_rev_space(&self) -> bool {
        self.0 & Self::REVSPACE != 0
    }

    /// Store a number of current volume.
    pub fn has_volume_number(&self) -> bool {
        self.0 & Self::VOLNUMBER != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HostSystem {
    MSDOS = 0,
    OS2 = 1,
    Win32 = 2,
    Unix = 3,
    MacOS = 4,
    BeOS = 5,
}

impl TryFrom<u8> for HostSystem {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            v if v == HostSystem::MSDOS as u8 => Ok(HostSystem::MSDOS),
            v if v == HostSystem::OS2 as u8 => Ok(HostSystem::OS2),
            v if v == HostSystem::Win32 as u8 => Ok(HostSystem::Win32),
            v if v == HostSystem::Unix as u8 => Ok(HostSystem::Unix),
            v if v == HostSystem::MacOS as u8 => Ok(HostSystem::MacOS),
            v if v == HostSystem::BeOS as u8 => Ok(HostSystem::BeOS),
            _ => Err(()),
        }
    }
}

// RAR 1.5 - 4.x header types.
mod rar15 {
    pub const MARKER: u8 = 0x72;
    pub const MAIN: u8 = 0x73;
    pub const FILE: u8 = 0x74;
    pub const SERVICE: u8 = 0x7a;
    pub const END_ARCHIVE: u8 = 0x7b;

    // RAR 2.9 and earlier
    pub const COMMENT: u8 = 0x75;
    pub const AUTHENTICITY_VERIFICATION: u8 = 0x76;
    pub const OLD_SERVICE: u8 = 0x77;

    // ?? also called RR_OLD
    pub const PROTECT: u8 = 0x78;
    // old AV?
    pub const SIGN: u8 = 0x79;
}

pub fn read_block15<T: io::Read + Seek>(reader: &mut T) -> io::Result<Block> {
    let position = reader.stream_position()?;

    let crc = read_u16(reader)?;
    let header_type = read_u8(reader)?;
    let flags = read_u16(reader)?;
    let head_size = read_u16(reader)?;

    // TODO: Check that head_size is >= 7

    match header_type {
        rar15::MAIN if head_size >= 13 => {
            // TODO main header could be > 13?

            let flags = MainBlockFlags::new(flags);

            let high_pos_av = read_u16(reader)?;
            let pos_av = read_u32(reader)?;

            let block = MainBlock::new(
                position,
                if flags.has_old_style_comment() {
                    13
                } else {
                    head_size as u64
                },
                crc,
                flags,
                high_pos_av,
                pos_av,
            );

            Ok(Block::Main(block))
        }
        rar15::FILE => {
            let data_size = read_u32(reader)?;
            let low_unp_size = read_u32(reader)?;
            let host_system: HostSystem = read_u8(reader)?.try_into().unwrap();

            let file_crc32 = read_u32(reader)?;
            let file_time = {
                let dos_time = read_u32(reader)?;
                let second = ((dos_time & 0x1f) * 2) as u8;
                let minute = ((dos_time >> 5) & 0x3f) as u8;
                let hour = ((dos_time >> 11) & 0x1f) as u8;
                let time = time::Time::from_hms(hour, minute, second).unwrap();
                let day = ((dos_time >> 16) & 0x1f) as u8;
                let month = ((dos_time >> 21) & 0x0f) as u8;
                let year = ((dos_time >> 25) + 1980) as i32;
                let date =
                    time::Date::from_calendar_date(year, month.try_into().unwrap(), day).unwrap();
                time::PrimitiveDateTime::new(date, time)
            };
            let unp_ver = read_u8(reader)?;

            let method = read_u8(reader)?;
            let name_size = read_u16(reader)? as usize;
            let file_attr = read_u32(reader)?;

            // Large file
            if flags & 0x100 != 0 {
                let high_pack_size = read_u32(reader)?;
                let high_unp_size = read_u32(reader)?;
            }

            let mut file_name = vec![0; name_size];
            reader.read_exact(&mut file_name)?;
            // let name = String::from_utf8(file_name).unwrap();
            // println!("{}", name);

            // Ok(head_size as usize + data_size as usize)
            Ok(Block::File(FileBlock {
                position,
                header_size: head_size as u64,
                file_size: data_size as u64,
                mtime: file_time,
                file_name,
                host_system,
            }))
        }
        rar15::SERVICE => {
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

            // let name = String::from_utf8(file_name.to_owned()).unwrap();
            // println!("{}", name);

            // Ok(head_size as usize + data_size as usize)
            // panic!("a");

            Ok(Block::Service(ServiceBlock {
                position,
                header_size: head_size as u64,
                // data_size: data_size as u64,
                name: file_name,
                data: vec![],
            }))
        }
        rar15::END_ARCHIVE => {
            let flags = EndArchiveBlockFlags::new(flags);

            let data_crc = if flags.has_data_crc() {
                Some(read_u32(reader)?)
            } else {
                None
            };

            let volume_number = if flags.has_volume_number() {
                Some(read_u16(reader)?)
            } else {
                None
            };

            Ok(Block::EndArchive(EndArchiveBlock {
                position,
                header_size: head_size as u64,
                flags,
                data_crc,
                volume_number,
            }))
        }

        rar15::AUTHENTICITY_VERIFICATION => Ok(Block::Service(ServiceBlock {
            position,
            header_size: head_size as u64,
            name: b"AV".to_vec(),
            data: vec![],
        })),

        rar15::COMMENT => {
            // uncompressed file size
            let unp_size = read_u16(reader)?;
            // RAR version needed to extract
            let unp_ver = read_u8(reader)?;
            let method = read_u8(reader)?;
            let comm_crc = read_u16(reader)?;

            let size = head_size - 13;

            let mut data = vec![0; size as usize];

            reader.read(&mut data)?;

            Ok(Block::Service(ServiceBlock {
                position,
                header_size: head_size as u64,
                name: b"CMT".to_vec(),
                data,
            }))
        }

        _ => todo!("other header types: 0x{:x}", header_type),
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
    let f = fs::File::open(&filename).unwrap();
    let file_len = f.metadata().unwrap().len();
    let mut f = BufReader::new(f);

    let (format, start) = is_archive(&mut f).unwrap();

    println!("format: {:?}", format);
    println!();

    f.seek(io::SeekFrom::Start(
        (start + format.signature_size()) as u64,
    ))
    .unwrap();

    loop {
        let block = read_block15(&mut f).unwrap();
        block.print_info();
        println!();
        if let Block::EndArchive(_) = block {
            break;
        }
        f.seek(SeekFrom::Start(block.position() + block.size()))
            .unwrap();

        let pos = f.stream_position().unwrap();

        if pos == file_len {
            break;
        }
    }
}
