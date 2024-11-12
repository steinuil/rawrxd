mod block;
mod decompress;
mod dos_time;
pub mod format;
pub mod rar14;
pub mod rar15;
pub mod rar_file;
mod rarvm;
mod read;

use std::{
    fs,
    io::{BufReader, Seek, SeekFrom},
};

use block::*;
use format::Format;

// const MAX_SFX_SIZE: usize = 0x200000;

impl MainBlock {
    pub fn print_info(&self) {
        println!("- type: main");
        println!("  position: {:#x}", self.position);
        println!("  size: {}", self.header_size);
        println!("  crc: {:04X}", self.crc);
        println!("  flags: {:#016b}", self.flags.0);
        println!(
            "    has_old_style_comment: {}",
            self.flags.has_old_style_comment()
        );
        println!(
            "    has_authenticity_information: {}",
            self.flags.has_authenticity_information()
        );
        println!("    is_volume: {}", self.flags.is_volume());
        println!("    is_solid: {}", self.flags.is_solid());
        println!("    is_locked: {}", self.flags.is_locked());
        println!(
            "    is_password_protected: {}",
            self.flags.is_password_protected()
        );
        println!("    is_first_volume: {}", self.flags.is_first_volume());
        println!("    is_encrypted: {}", self.flags.is_encrypted());
        println!(
            "    uses_new_numbering: {}",
            self.flags.uses_new_numbering()
        );
        println!("  high_pos_av: {}", self.high_pos_av);
        println!("  pos_av: {:#x}", self.pos_av);
    }
}

#[derive(Debug, Clone)]
pub struct FileBlock {
    pub position: u64,
    pub header_size: u64,

    pub packed_file_size: u64,

    pub unpacked_file_size: u64,

    pub mtime: time::PrimitiveDateTime,
    pub crc32: u32,
    pub attributes: u32,
    pub file_name: Vec<u8>,
    pub host_system: HostSystem,
}

impl FileBlock {
    pub fn print_info(&self) {
        println!("- type: file");
        println!("  position: {:#x}", self.position);
        println!("  header_size: {}", self.header_size);
        println!("  host_system: {:?}", self.host_system);
        println!("  crc32: {:08X}", self.crc32);
        println!("  attributes: {:032b}", self.attributes);
        println!(
            "  file_name: {}",
            std::str::from_utf8(&self.file_name).unwrap()
        );
        println!("  packed_file_size: {}", self.packed_file_size);
        println!("  unpacked_file_size: {}", self.unpacked_file_size);
        println!("  mtime: {}", self.mtime);
    }
}

#[derive(Debug, Clone)]
pub struct ServiceBlock {
    pub position: u64,
    pub header_size: u64,
    pub name: Vec<u8>,
    pub data_size: u64,
    // pub data: Vec<u8>,
}

impl ServiceBlock {
    pub fn print_info(&self) {
        println!("- type: service");
        println!("  position: {:#x}", self.position);
        println!("  header_size: {}", self.header_size);
        println!("  data_size: {}", self.data_size);
        println!(
            "  name: {}",
            std::str::from_utf8(&self.name).unwrap_or_default()
        );
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
            println!("  crc: {:08X}", data_crc);
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
    Unrecognized(u8, u64, u64),
}

impl Block {
    pub fn position(&self) -> u64 {
        match self {
            Block::Main(block) => block.position,
            Block::File(block) => block.position,
            Block::Service(block) => block.position,
            Block::EndArchive(block) => block.position,
            Block::Unrecognized(_, position, _) => *position,
        }
    }

    pub fn size(&self) -> u64 {
        match self {
            Block::Main(block) => block.header_size,
            Block::File(block) => block.header_size + block.packed_file_size,
            Block::Service(block) => block.header_size,
            Block::EndArchive(block) => block.header_size,
            Block::Unrecognized(_, _, size) => *size,
        }
    }

    pub fn print_info(&self) {
        match self {
            Block::Main(block) => block.print_info(),
            Block::File(block) => block.print_info(),
            Block::Service(block) => block.print_info(),
            Block::EndArchive(block) => block.print_info(),
            Block::Unrecognized(id, _, _) => {
                println!("- type: {:#x}", id);
            }
        }
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
// mod rar15 {
//     /// HEAD3_MARK
//     pub const MARKER: u8 = 0x72;

//     /// HEAD3_MAIN
//     pub const MAIN: u8 = 0x73;

//     /// HEAD3_FILE
//     pub const FILE: u8 = 0x74;

//     /// HEAD3_SERVICE
//     pub const SERVICE: u8 = 0x7a;

//     /// HEAD3_ENDARC
//     pub const END_ARCHIVE: u8 = 0x7b;

//     /// HEAD3_75
//     pub const OLD_COMMENT: u8 = 0x75;

//     /// HEAD3_AV
//     pub const OLD_AUTHENTICITY_VERIFICATION1: u8 = 0x76;

//     /// HEAD3_SIGN
//     pub const OLD_AUTHENTICITY_VERIFICATION2: u8 = 0x79;

//     /// HEAD3_OLDSERVICE
//     pub const OLD_SERVICE: u8 = 0x77;

//     /// HEAD3_PROTECT
//     pub const OLD_RECOVERY_RECORD: u8 = 0x78;

//     pub mod flags {
//         pub const LARGE_FILE: u16 = 0x100;
//     }
// }

// pub fn read_block15<T: io::Read + Seek>(reader: &mut T) -> io::Result<Block> {
//     let position = reader.stream_position()?;

//     let crc = read_u16(reader)?;
//     let header_type = read_u8(reader)?;
//     let flags = read_u16(reader)?;
//     let head_size = read_u16(reader)?;

//     // TODO: Check that head_size is >= 7

//     match header_type {
//         rar15::MAIN if head_size >= 13 => {
//             // TODO main header could be > 13?

//             let flags = MainBlockFlags::new(flags);

//             let high_pos_av = read_u16(reader)?;
//             let pos_av = read_u32(reader)?;

//             Ok(Block::Main(MainBlock {
//                 position,
//                 header_size: if flags.has_old_style_comment() {
//                     13
//                 } else {
//                     head_size as u64
//                 },
//                 crc,
//                 flags,
//                 high_pos_av,
//                 pos_av,
//             }))
//         }
//         rar15::FILE => {
//             let low_data_size = read_u32(reader)? as u64;
//             let low_unpacked_size = read_u32(reader)? as u64;
//             let host_system: HostSystem = read_u8(reader)?.try_into().unwrap();

//             let file_crc32 = read_u32(reader)?;
//             let file_time = {
//                 let dos_time = read_u32(reader)?;
//                 dos_time::parse(dos_time)
//             };
//             let unp_ver = read_u8(reader)?;

//             let method = read_u8(reader)?;
//             let name_size = read_u16(reader)? as usize;
//             let file_attr = read_u32(reader)?;

//             let (data_size, unpacked_size) = if flags & rar15::flags::LARGE_FILE != 0 {
//                 let high_data_size = read_u32(reader)? as u64;
//                 let high_unpacked_size = read_u32(reader)? as u64;

//                 (
//                     (high_data_size >> 4) | low_data_size,
//                     (high_unpacked_size >> 4) | low_unpacked_size,
//                 )
//             } else {
//                 (low_data_size, low_unpacked_size)
//             };

//             let mut file_name = vec![0; name_size];
//             reader.read_exact(&mut file_name)?;

//             Ok(Block::File(FileBlock {
//                 position,
//                 header_size: head_size as u64,
//                 packed_file_size: data_size,
//                 unpacked_file_size: unpacked_size,
//                 mtime: file_time,
//                 crc32: file_crc32,
//                 attributes: file_attr,
//                 file_name,
//                 host_system,
//             }))
//         }
//         rar15::SERVICE => {
//             let data_size = read_u32(reader)?;
//             let low_unp_size = read_u32(reader)?;
//             let host_os = read_u8(reader)?;

//             let file_crc32 = read_u32(reader)?;
//             let file_time = read_u32(reader)?;
//             let unp_ver = read_u8(reader)?;

//             let method = read_u8(reader)? - 0x30;
//             let name_size = read_u16(reader)? as usize;
//             let file_attr = read_u32(reader)?;

//             // Large file
//             if flags & 0x100 != 0 {
//                 let high_pack_size = read_u32(reader)?;
//                 let high_unp_size = read_u32(reader)?;
//             }

//             let mut file_name = vec![0; name_size];
//             reader.read_exact(&mut file_name)?;

//             // Ok(head_size as usize + data_size as usize)
//             // panic!("a");

//             Ok(Block::Service(ServiceBlock {
//                 position,
//                 header_size: head_size as u64,
//                 name: file_name,
//                 data_size: data_size as u64,
//             }))
//         }
//         rar15::END_ARCHIVE => {
//             let flags = EndArchiveBlockFlags::new(flags);

//             let data_crc = if flags.has_data_crc() {
//                 Some(read_u32(reader)?)
//             } else {
//                 None
//             };

//             let volume_number = if flags.has_volume_number() {
//                 Some(read_u16(reader)?)
//             } else {
//                 None
//             };

//             Ok(Block::EndArchive(EndArchiveBlock {
//                 position,
//                 header_size: head_size as u64,
//                 flags,
//                 data_crc,
//                 volume_number,
//             }))
//         }

//         rar15::OLD_AUTHENTICITY_VERIFICATION1 => Ok(Block::Service(ServiceBlock {
//             position,
//             header_size: head_size as u64,
//             name: b"AV".to_vec(),
//             data_size: 0,
//             // data: vec![],
//         })),

//         rar15::OLD_COMMENT => {
//             // uncompressed file size
//             let unp_size = read_u16(reader)?;
//             // RAR version needed to extract
//             let unp_ver = read_u8(reader)?;
//             let method = read_u8(reader)?;
//             let comm_crc = read_u16(reader)?;

//             let size = head_size - 13;

//             let mut data = vec![0; size as usize];

//             reader.read_exact(&mut data)?;

//             Ok(Block::Service(ServiceBlock {
//                 position,
//                 header_size: head_size as u64,
//                 name: b"CMT".to_vec(),
//                 data_size: size as u64,
//                 // data,
//             }))
//         }

//         rar15::OLD_RECOVERY_RECORD => {
//             let creation_time = read_u32(reader)?;
//             let arc_name_size = read_u16(reader)?;
//             let user_name_size = read_u16(reader)?;

//             println!("creation_time: {creation_time}");
//             println!("arc_name_size: {arc_name_size}");
//             println!("user_name_size: {user_name_size}");

//             Ok(Block::Service(ServiceBlock {
//                 position,
//                 header_size: head_size as u64,
//                 name: b"SIGN".to_vec(),
//                 data_size: (arc_name_size + user_name_size) as u64,
//             }))
//         }

//         _ => Ok(Block::Unrecognized(header_type, head_size as u64, 0)),
//     }
// }

fn main() {
    let mut args = std::env::args();

    let filename = args.nth(1).unwrap();
    let f = fs::File::open(&filename).unwrap();
    let file_len = f.metadata().unwrap().len();
    let mut f = BufReader::new(f);

    let format = rar_file::read_signature(&mut f).unwrap().unwrap();

    println!("format: {:?}", format);
    println!();

    match format {
        Format::Rar14 => {
            let block = rar14::MainHeader::read(&mut f).unwrap();
            println!("position: {}", block.position);
            println!("header_size: {}", block.header_size);
            println!("flags:");
            println!("  is_volume: {}", block.flags.is_volume());
            println!("  is_solid: {}", block.flags.is_solid());
            println!("  is_locked: {}", block.flags.is_locked());
            println!("  has_comment: {}", block.flags.has_comment());
            println!("  is_comment_packed: {}", block.flags.is_comment_packed());
            let comment = block.read_comment(&mut f).unwrap();
            println!("{:?}", comment);

            f.seek(SeekFrom::Start(block.position + block.header_size))
                .unwrap();

            loop {
                let pos = f.stream_position().unwrap();
                if pos == file_len {
                    break;
                }

                let block = rar14::FileHeader::read(&mut f).unwrap();
                println!("{:#?}", block);

                f.seek(SeekFrom::Start(
                    block.position + block.header_size + block.packed_data_size as u64,
                ))
                .unwrap();
            }
        }
        Format::Rar15 => loop {
            let block = rar15::Block::read(&mut f).unwrap();
            println!("{block:#?}");
            if let rar15::BlockKind::EndArchive(_) = block.kind {
                break;
            }
            f.seek(SeekFrom::Start(block.position + block.full_size()))
                .unwrap();

            let pos = f.stream_position().unwrap();

            if pos == file_len {
                break;
            }
        },
        Format::Rar50 => todo!(),
    }
}
