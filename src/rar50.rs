use std::{io, ops::Deref};

use crate::{
    read::*,
    size::{DataSize, HeaderSize},
};

#[derive(Debug)]
pub struct Block {
    pub position: u64,
    pub flags: CommonFlags,
    pub header_crc32: u32,
    pub header_size: u64,
    pub extra_area_size: Option<u64>,
    pub data_size: Option<u64>,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, Copy)]
pub struct CommonFlags(u64);

impl CommonFlags {
    const EXTRA: u64 = 0x0001;
    const DATA: u64 = 0x0002;
    const SKIP_IF_UNKNOWN: u64 = 0x0004;
    const SPLIT_BEFORE: u64 = 0x0008;
    const SPLIT_AFTER: u64 = 0x0010;
    const CHILD: u64 = 0x0020;
    const INHERITED: u64 = 0x0040;

    pub fn new(flags: u64) -> Self {
        Self(flags)
    }

    /// Additional extra area is present at the end of the block header.
    pub fn has_extra_area(&self) -> bool {
        self.0 & Self::EXTRA != 0
    }

    /// Additional data area is present at the end of the block header.
    pub fn has_data_area(&self) -> bool {
        self.0 & Self::DATA != 0
    }

    /// Unknown blocks with this flag must be skipped when updating an archive.
    pub fn skip_if_unknown(&self) -> bool {
        self.0 & Self::SKIP_IF_UNKNOWN != 0
    }

    /// Data area of this block is continuing from the previous volume.
    pub fn split_before(&self) -> bool {
        self.0 & Self::SPLIT_BEFORE != 0
    }

    /// Data area of this block is continuing in the next volume.
    pub fn split_after(&self) -> bool {
        self.0 & Self::SPLIT_AFTER != 0
    }

    /// Block depends on preceding file block.
    pub fn is_child(&self) -> bool {
        self.0 & Self::CHILD != 0
    }

    /// Preserve a child block if host is modified.
    pub fn is_inherited(&self) -> bool {
        self.0 & Self::INHERITED != 0
    }
}

impl Deref for CommonFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub enum BlockKind {
    Main(MainBlock),
    File(FileBlock),
    Service(ServiceBlock),
    Crypt(CryptBlock),
    EndArchive(EndArchiveBlock),
    Unknown(UnknownBlock),
}

mod block {
    // pub const MARKER: u64 = 0x00;
    pub const MAIN: u64 = 0x01;
    pub const FILE: u64 = 0x02;
    pub const SERVICE: u64 = 0x03;
    pub const CRYPT: u64 = 0x04;
    pub const ENDARC: u64 = 0x05;
}

impl Block {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let position = reader.stream_position()?;

        let header_crc32 = read_u32(reader)?;

        let (header_size, vint_size) = read_vint(reader)?;
        let full_header_size = header_size + vint_size as u64 + 4;

        let (header_type, _) = read_vint(reader)?;

        let (flags, _) = read_vint(reader)?;
        let flags = CommonFlags::new(flags);

        let extra_area_size = if flags.has_extra_area() {
            Some(read_vint(reader)?.0)
        } else {
            None
        };

        let data_size = if flags.has_data_area() {
            Some(read_vint(reader)?.0)
        } else {
            None
        };

        let kind = match header_type {
            block::MAIN => BlockKind::Main(MainBlock::read(reader)?),
            block::FILE => BlockKind::File(FileBlock::read(reader)?),
            block::SERVICE => BlockKind::Service(ServiceBlock::read(reader)?),
            block::CRYPT => BlockKind::Crypt(CryptBlock::read(reader)?),
            block::ENDARC => BlockKind::EndArchive(EndArchiveBlock::read(reader)?),
            _ => BlockKind::Unknown(UnknownBlock::read(reader, header_type)?),
        };

        Ok(Block {
            position,
            flags,
            header_crc32,
            header_size: full_header_size,
            extra_area_size,
            data_size,
            kind,
        })
    }
}

impl HeaderSize for Block {
    fn header_size(&self) -> u64 {
        self.header_size
    }
}

impl DataSize for Block {
    fn data_size(&self) -> u64 {
        self.data_size.unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct CryptBlock {
    pub encryption_version: u64,
    pub flags: CryptBlockFlags,
    pub kdf_count: u8,
    pub salt: [u8; 16],
    pub check_value: Option<[u8; 12]>,
}

#[derive(Debug, Clone, Copy)]
pub struct CryptBlockFlags(u64);

impl CryptBlockFlags {
    const PSWCHECK: u64 = 0x0001;

    pub fn new(flags: u64) -> Self {
        Self(flags)
    }

    /// Password check data is present.
    pub fn has_password_check(&self) -> bool {
        self.0 & Self::PSWCHECK != 0
    }
}

impl Deref for CryptBlockFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncryptionVersion {
    Aes256 = 0,
}

impl CryptBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (encryption_version, _) = read_vint(reader)?;
        let (flags, _) = read_vint(reader)?;
        let kdf_count = read_u8(reader)?;
        let salt = read_const_bytes(reader)?;

        let flags = CryptBlockFlags::new(flags);

        let check_value = if flags.has_password_check() {
            Some(read_const_bytes(reader)?)
        } else {
            None
        };

        Ok(CryptBlock {
            encryption_version,
            flags,
            kdf_count,
            salt,
            check_value,
        })
    }
}

#[derive(Debug)]
pub struct MainBlock {
    pub flags: MainBlockFlags,
    pub volume_number: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct MainBlockFlags(u64);

impl MainBlockFlags {
    const VOLUME: u64 = 0x0001;
    const VOLUME_NUMBER: u64 = 0x0002;
    const SOLID: u64 = 0x0004;
    const PROTECT: u64 = 0x0008;
    const LOCK: u64 = 0x0010;

    pub fn new(flags: u64) -> Self {
        Self(flags)
    }

    pub fn is_volume(&self) -> bool {
        self.0 & Self::VOLUME != 0
    }

    pub fn has_volume_number(&self) -> bool {
        self.0 & Self::VOLUME_NUMBER != 0
    }

    pub fn is_solid(&self) -> bool {
        self.0 & Self::SOLID != 0
    }

    pub fn has_recovery_record(&self) -> bool {
        self.0 & Self::PROTECT != 0
    }

    pub fn is_locked(&self) -> bool {
        self.0 & Self::LOCK != 0
    }
}

impl Deref for MainBlockFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MainBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = MainBlockFlags::new(flags);

        let volume_number = if flags.has_volume_number() {
            Some(read_vint(reader)?.0)
        } else {
            None
        };

        Ok(MainBlock {
            flags,
            volume_number,
        })
    }
}

#[derive(Debug)]
pub struct FileBlock {
    pub flags: FileBlockFlags,
    pub unpacked_size: u64,
    pub attributes: u64,
    pub mtime: Option<time::PrimitiveDateTime>,
    pub data_crc32: Option<u32>,
    pub compression_info: u64,
    pub host_os: HostOs,
    pub name: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct FileBlockFlags(u64);

impl FileBlockFlags {
    const DIRECTORY: u64 = 0x0001;
    const UTIME: u64 = 0x0002;
    const CRC32: u64 = 0x0004;
    const UNPUNKNOWN: u64 = 0x0008;

    pub fn new(flags: u64) -> Self {
        Self(flags)
    }

    pub fn is_directory(&self) -> bool {
        self.0 & Self::DIRECTORY != 0
    }

    pub fn has_mtime(&self) -> bool {
        self.0 & Self::UTIME != 0
    }

    pub fn has_crc32(&self) -> bool {
        self.0 & Self::CRC32 != 0
    }

    pub fn unknown_unpacked_size(&self) -> bool {
        self.0 & Self::UNPUNKNOWN != 0
    }
}

impl Deref for FileBlockFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HostOs {
    Windows = 0,
    Unix = 1,
}

impl TryFrom<u64> for HostOs {
    type Error = u64;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            v if v == HostOs::Windows as u64 => Ok(HostOs::Windows),
            v if v == HostOs::Unix as u64 => Ok(HostOs::Unix),
            _ => Err(value),
        }
    }
}

impl FileBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = FileBlockFlags::new(flags);

        // TODO should signal that this value might be garbage if the block
        // has the UNPUNKNOWN flag set
        let (unpacked_size, _) = read_vint(reader)?;

        let (attributes, _) = read_vint(reader)?;

        let mtime = if flags.has_mtime() {
            let mtime = read_u32(reader)?;
            let mtime = time::OffsetDateTime::from_unix_timestamp(mtime.into()).unwrap();
            let mtime = time::PrimitiveDateTime::new(mtime.date(), mtime.time());
            Some(mtime)
        } else {
            None
        };

        let data_crc32 = if flags.has_crc32() {
            Some(read_u32(reader)?)
        } else {
            None
        };

        let (compression_info, _) = read_vint(reader)?;

        let (host_os, _) = read_vint(reader)?;

        let (name_length, _) = read_vint(reader)?;
        let name = read_vec(reader, name_length as usize)?;

        Ok(FileBlock {
            flags,
            unpacked_size,
            attributes,
            mtime,
            data_crc32,
            compression_info,
            host_os: host_os.try_into().unwrap(),
            name,
        })
    }
}

#[derive(Debug)]
pub struct ServiceBlock {
    pub flags: ServiceBlockFlags,
    pub unpacked_size: u64,
    pub mtime: Option<time::PrimitiveDateTime>,
    pub data_crc32: Option<u32>,
    pub compression_info: u64,
    pub host_os: HostOs,
    pub name: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceBlockFlags(u64);

impl ServiceBlockFlags {
    const UTIME: u64 = 0x0002;
    const CRC32: u64 = 0x0004;
    const UNPUNKNOWN: u64 = 0x0008;

    pub fn new(flags: u64) -> Self {
        Self(flags)
    }

    pub fn has_mtime(&self) -> bool {
        self.0 & Self::UTIME != 0
    }

    pub fn has_crc32(&self) -> bool {
        self.0 & Self::CRC32 != 0
    }

    pub fn unknown_unpacked_size(&self) -> bool {
        self.0 & Self::UNPUNKNOWN != 0
    }
}

impl Deref for ServiceBlockFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ServiceBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = ServiceBlockFlags::new(flags);

        // TODO should signal that this value might be garbage if the block
        // has the UNPUNKNOWN flag set
        let (unpacked_size, _) = read_vint(reader)?;

        let (attributes, _) = read_vint(reader)?;
        if attributes != 0 {
            // log a warning or something
        }

        let mtime = if flags.has_mtime() {
            let mtime = read_u32(reader)?;
            let mtime = time::OffsetDateTime::from_unix_timestamp(mtime.into()).unwrap();
            let mtime = time::PrimitiveDateTime::new(mtime.date(), mtime.time());
            Some(mtime)
        } else {
            None
        };

        let data_crc32 = if flags.has_crc32() {
            Some(read_u32(reader)?)
        } else {
            None
        };

        let (compression_info, _) = read_vint(reader)?;

        let (host_os, _) = read_vint(reader)?;

        let (name_length, _) = read_vint(reader)?;
        let name = read_vec(reader, name_length as usize)?;

        Ok(ServiceBlock {
            flags,
            unpacked_size,
            mtime,
            data_crc32,
            compression_info,
            host_os: host_os.try_into().unwrap(),
            name,
        })
    }
}

#[derive(Debug)]
pub struct EndArchiveBlock {
    pub flags: EndArchiveBlockFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct EndArchiveBlockFlags(u64);

impl EndArchiveBlockFlags {
    const NEXTVOLUME: u64 = 0x0001;

    pub fn new(flags: u64) -> Self {
        Self(flags)
    }

    pub fn has_next_volume(&self) -> bool {
        self.0 & Self::NEXTVOLUME != 0
    }
}

impl Deref for EndArchiveBlockFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl EndArchiveBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = EndArchiveBlockFlags::new(flags);

        Ok(EndArchiveBlock { flags })
    }
}

#[derive(Debug)]
pub struct UnknownBlock {
    pub tag: u64,
}

impl UnknownBlock {
    pub fn read<R: io::Read + io::Seek>(_reader: &mut R, tag: u64) -> io::Result<Self> {
        Ok(UnknownBlock { tag })
    }
}
