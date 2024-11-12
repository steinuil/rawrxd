use std::io;

use crate::{dos_time, read::*};

const NAME_MAX_SIZE: u16 = 1000;

pub trait BlockRead: Sized {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self>;
}

pub trait DataSize: Sized {
    fn data_size(&self) -> u64;
}

#[derive(Debug)]
pub struct MainBlock {
    pub flags: MainBlockFlags,
    pub high_pos_av: u16,
    pub pos_av: u32,
    pub encrypt_version: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct MainBlockFlags(u16);

impl MainBlockFlags {
    const VOLUME: u16 = 0x0001;
    const COMMENT: u16 = 0x0002;
    const LOCK: u16 = 0x0004;
    const SOLID: u16 = 0x0008;
    const NEWNUMBERING: u16 = 0x0010;
    const AV: u16 = 0x0020;
    const PROTECT: u16 = 0x0040;
    const PASSWORD: u16 = 0x0080;
    const FIRSTVOLUME: u16 = 0x0100;
    const ENCRYPTVER: u16 = 0x0200;

    pub fn new(flags: u16) -> Self {
        Self(flags)
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
    /// and it only serves to prevent WinRAR from modifying it.
    pub fn is_locked(&self) -> bool {
        self.0 & Self::LOCK != 0
    }

    /// Contains an old-style (up to RAR 2.9) comment
    pub fn has_comment(&self) -> bool {
        self.0 & Self::COMMENT != 0
    }

    // TODO document this
    pub fn is_protected(&self) -> bool {
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

    // TODO document this
    pub fn has_authenticity_verification(&self) -> bool {
        self.0 & Self::AV != 0
    }

    /// Indicates whether encryption is present in the archive
    pub fn has_encrypt_version(&self) -> bool {
        self.0 & Self::ENCRYPTVER != 0
    }
}

impl BlockRead for MainBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self> {
        let flags = MainBlockFlags::new(flags);

        let high_pos_av = read_u16(reader)?;
        let pos_av = read_u32(reader)?;

        // This is not even read in newer versions of unrar
        let encrypt_version = if flags.has_encrypt_version() {
            let encrypt_version = read_u8(reader)?;
            Some(encrypt_version)
        } else {
            None
        };

        Ok(MainBlock {
            flags,
            high_pos_av,
            pos_av,
            encrypt_version,
        })
    }
}

impl DataSize for MainBlock {
    fn data_size(&self) -> u64 {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HostOs {
    MsDos = 0,
    Os2 = 1,
    Win32 = 2,
    Unix = 3,
    MacOs = 4,
    BeOs = 5,
}

impl TryFrom<u8> for HostOs {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            v if v == HostOs::MsDos as u8 => Ok(HostOs::MsDos),
            v if v == HostOs::Os2 as u8 => Ok(HostOs::Os2),
            v if v == HostOs::Win32 as u8 => Ok(HostOs::Win32),
            v if v == HostOs::Unix as u8 => Ok(HostOs::Unix),
            v if v == HostOs::MacOs as u8 => Ok(HostOs::MacOs),
            v if v == HostOs::BeOs as u8 => Ok(HostOs::BeOs),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct FileBlock {
    pub packed_data_size: u64,
    pub unpacked_data_size: u64,
    pub host_os: HostOs,
    pub file_crc32: u32,
    pub mtime: time::PrimitiveDateTime,
    pub unpack_version: u8,
    pub method: u8,
    pub attributes: u32,
    pub file_name: Vec<u8>,
    pub salt: Option<[u8; Self::SALT_SIZE]>,
}

#[derive(Debug, Clone, Copy)]
pub struct FileBlockFlags(u16);

impl FileBlockFlags {
    const COMMENT: u16 = 0x0002;
    const LARGE: u16 = 0x0100;
    const UNICODE: u16 = 0x0200;
    const SALT: u16 = 0x0400;
    const VERSION: u16 = 0x0800;
    const EXTTIME: u16 = 0x1000;
    const EXTAREA: u16 = 0x2000;

    pub fn new(flags: u16) -> Self {
        Self(flags)
    }

    pub fn has_large_size(&self) -> bool {
        self.0 & Self::LARGE != 0
    }

    pub fn has_unicode_filename(&self) -> bool {
        self.0 & Self::UNICODE != 0
    }

    pub fn has_salt(&self) -> bool {
        self.0 & Self::SALT != 0
    }

    pub fn has_extended_time(&self) -> bool {
        self.0 & Self::EXTTIME != 0
    }

    pub fn has_comment(&self) -> bool {
        self.0 & Self::COMMENT != 0
    }

    // TODO document this
    pub fn parse_version(&self) -> bool {
        self.0 & Self::VERSION != 0
    }

    // TODO not sure this is used
    pub fn has_extended_area(&self) -> bool {
        self.0 & Self::EXTAREA != 0
    }
}

impl FileBlock {
    const SALT_SIZE: usize = 8;
}

impl BlockRead for FileBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self> {
        let flags = FileBlockFlags::new(flags);

        let low_packed_data_size = read_u32(reader)? as u64;
        let low_unpacked_data_size = read_u32(reader)? as u64;
        let host_os = read_u8(reader)?;
        let file_crc32 = read_u32(reader)?;
        let mtime = read_u32(reader)?;
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let name_size = read_u16(reader)? as usize;
        let attributes = read_u32(reader)?;

        let (packed_data_size, unpacked_data_size) = if flags.has_large_size() {
            let high_packed_data_size = read_u32(reader)? as u64;
            let high_unpacked_data_size = read_u32(reader)? as u64;

            (
                (high_packed_data_size >> 4) | low_packed_data_size,
                (high_unpacked_data_size >> 4) | low_unpacked_data_size,
            )
        } else {
            (low_packed_data_size, low_unpacked_data_size)
        };

        let mut file_name = vec![0; name_size];
        reader.read_exact(&mut file_name)?;

        if flags.has_unicode_filename() {
            // TODO decode the filename to unicode?
        }

        let salt = if flags.has_salt() {
            let mut salt = [0; Self::SALT_SIZE];
            reader.read_exact(&mut salt)?;
            Some(salt)
        } else {
            None
        };

        // TODO parse exttime

        Ok(FileBlock {
            packed_data_size,
            unpacked_data_size,
            host_os: host_os.try_into().unwrap(),
            file_crc32,
            mtime: dos_time::parse(mtime),
            unpack_version,
            method,
            attributes,
            file_name,
            salt,
        })
    }
}

impl DataSize for FileBlock {
    fn data_size(&self) -> u64 {
        self.packed_data_size
    }
}

// TODO the service block has basically the same subheads
// found in SubBlock, so we should parse them accordingly.
#[derive(Debug)]
pub struct ServiceBlock {
    pub packed_data_size: u64,
    pub unpacked_data_size: u64,
    pub host_os: HostOs,
    pub file_crc32: u32,
    pub mtime: time::PrimitiveDateTime,
    pub unpack_version: u8,
    pub method: u8,
    pub sub_flags: u32,
    pub name: Vec<u8>,
    pub sub_data: Option<Vec<u8>>,
    pub salt: Option<[u8; 8]>,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceBlockFlags(u16);

impl ServiceBlockFlags {
    const COMMENT: u16 = 0x0002;
    const LARGE: u16 = 0x0100;
    const SALT: u16 = 0x0400;
    const VERSION: u16 = 0x0800;
    const EXTTIME: u16 = 0x1000;
    const EXTAREA: u16 = 0x2000;

    pub fn new(flags: u16) -> Self {
        Self(flags)
    }

    pub fn has_large_size(&self) -> bool {
        self.0 & Self::LARGE != 0
    }

    pub fn has_salt(&self) -> bool {
        self.0 & Self::SALT != 0
    }

    pub fn has_extended_time(&self) -> bool {
        self.0 & Self::EXTTIME != 0
    }

    pub fn has_comment(&self) -> bool {
        self.0 & Self::COMMENT != 0
    }

    // TODO document this
    pub fn parse_version(&self) -> bool {
        self.0 & Self::VERSION != 0
    }

    // TODO not sure this is used
    pub fn has_extended_area(&self) -> bool {
        self.0 & Self::EXTAREA != 0
    }
}

impl ServiceBlock {
    const SIZE: usize = 32;
    const SALT_SIZE: usize = 8;

    pub fn read<R: io::Read + io::Seek>(
        reader: &mut R,
        flags: u16,
        header_size: u16,
    ) -> io::Result<Self> {
        let flags = ServiceBlockFlags::new(flags);

        let low_packed_data_size = read_u32(reader)? as u64;
        let low_unpacked_data_size = read_u32(reader)? as u64;
        let host_os = read_u8(reader)?;
        let file_crc32 = read_u32(reader)?;
        let mtime = read_u32(reader)?;
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let name_size = read_u16(reader)? as usize;
        let sub_flags = read_u32(reader)?;

        let (packed_data_size, unpacked_data_size) = if flags.has_large_size() {
            let high_packed_data_size = read_u32(reader)? as u64;
            let high_unpacked_data_size = read_u32(reader)? as u64;

            (
                (high_packed_data_size >> 4) | low_packed_data_size,
                (high_unpacked_data_size >> 4) | low_unpacked_data_size,
            )
        } else {
            (low_packed_data_size, low_unpacked_data_size)
        };

        let mut name = vec![0; name_size];
        reader.read_exact(&mut name)?;

        let sub_data_size = (header_size as usize)
            - name_size
            - Self::SIZE
            - if flags.has_salt() { Self::SALT_SIZE } else { 0 };

        let sub_data = if sub_data_size > 0 {
            let mut sub_data = vec![0; sub_data_size];
            reader.read_exact(&mut sub_data)?;
            Some(sub_data)
        } else {
            None
        };

        let salt = if flags.has_salt() {
            let mut salt = [0; Self::SALT_SIZE];
            reader.read_exact(&mut salt)?;
            Some(salt)
        } else {
            None
        };

        // TODO parse exttime

        Ok(ServiceBlock {
            packed_data_size,
            unpacked_data_size,
            host_os: host_os.try_into().unwrap(),
            file_crc32,
            mtime: dos_time::parse(mtime),
            unpack_version,
            method,
            sub_flags,
            name,
            sub_data,
            salt,
        })
    }
}

impl DataSize for ServiceBlock {
    fn data_size(&self) -> u64 {
        self.packed_data_size
    }
}

#[derive(Debug)]
pub struct CommentBlock {
    // TODO do we need flags?
    pub unpacked_data_size: u16,
    pub unpack_version: u8,
    pub method: u8,
    pub crc16: u16,
}

impl BlockRead for CommentBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let unpacked_data_size = read_u16(reader)?;
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let crc16 = read_u16(reader)?;

        Ok(CommentBlock {
            unpacked_data_size,
            unpack_version,
            method,
            crc16,
        })
    }
}

impl DataSize for CommentBlock {
    fn data_size(&self) -> u64 {
        0
    }
}

#[derive(Debug)]
pub struct ProtectBlock {
    // TODO do we need flags?
    pub data_size: u32,
    pub version: u8,
    pub recovery_sectors: u16,
    pub total_blocks: u32,
    pub mark: [u8; Self::MARK_SIZE],
}

impl ProtectBlock {
    const MARK_SIZE: usize = 8;
}

impl BlockRead for ProtectBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let data_size = read_u32(reader)?;
        let version = read_u8(reader)?;
        let recovery_sectors = read_u16(reader)?;
        let total_blocks = read_u32(reader)?;
        let mut mark = [0; Self::MARK_SIZE];
        reader.read_exact(&mut mark)?;

        Ok(ProtectBlock {
            data_size,
            version,
            recovery_sectors,
            total_blocks,
            mark,
        })
    }
}

impl DataSize for ProtectBlock {
    fn data_size(&self) -> u64 {
        self.data_size as u64
    }
}

#[derive(Debug)]
#[repr(u16)]
pub enum SubBlockType {
    // EA_HEAD
    Os2ExtendedAttributes = 0x100,
    // UO_HEAD
    UnixOwner = 0x101,
    // MAC_HEAD
    MacOsInfo = 0x102,
    // BEEA_HEAD
    BeOsExtendedAttributes = 0x103,
    // NTACL_HEAD
    NtfsAcl = 0x104,
    // STREAM_HEAD
    NtfsStream = 0x105,
}

impl TryFrom<u16> for SubBlockType {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            v if v == Self::Os2ExtendedAttributes as u16 => Ok(Self::Os2ExtendedAttributes),
            v if v == Self::UnixOwner as u16 => Ok(Self::UnixOwner),
            v if v == Self::MacOsInfo as u16 => Ok(Self::MacOsInfo),
            v if v == Self::BeOsExtendedAttributes as u16 => Ok(Self::BeOsExtendedAttributes),
            v if v == Self::NtfsAcl as u16 => Ok(Self::NtfsAcl),
            v if v == Self::NtfsStream as u16 => Ok(Self::NtfsStream),
            _ => Err(value),
        }
    }
}

#[derive(Debug)]
pub struct UnixOwnerSubBlock {
    pub user: Vec<u8>,
    pub group: Vec<u8>,
}

impl UnixOwnerSubBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let user_size = read_u16(reader)?.clamp(0, NAME_MAX_SIZE - 1) as usize;
        let group_size = read_u16(reader)?.clamp(0, NAME_MAX_SIZE - 1) as usize;

        let mut user = vec![0; user_size];
        reader.read_exact(&mut user)?;

        let mut group = vec![0; group_size];
        reader.read_exact(&mut group)?;

        Ok(UnixOwnerSubBlock { user, group })
    }
}

#[derive(Debug)]
pub struct MacOsInfoSubBlock {
    pub file_type: u16,
    pub file_creator: u16,
}

impl MacOsInfoSubBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let file_type = read_u16(reader)?;
        let file_creator = read_u16(reader)?;

        Ok(MacOsInfoSubBlock {
            file_type,
            file_creator,
        })
    }
}

#[derive(Debug)]
pub struct ExtendedAttributesSubBlock {
    pub filesystem: ExtendedAttributesFs,
    pub unpacked_data_size: u32,
    pub unpack_version: u8,
    pub method: u8,
    pub extended_attributes_crc32: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum ExtendedAttributesFs {
    Os2,
    BeOs,
    Ntfs,
}

impl ExtendedAttributesSubBlock {
    pub fn read<R: io::Read + io::Seek>(
        reader: &mut R,
        filesystem: ExtendedAttributesFs,
    ) -> io::Result<Self> {
        let unpacked_data_size = read_u32(reader)?;
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let extended_attributes_crc32 = read_u32(reader)?;

        Ok(ExtendedAttributesSubBlock {
            filesystem,
            unpacked_data_size,
            unpack_version,
            method,
            extended_attributes_crc32,
        })
    }
}

#[derive(Debug)]
pub struct NtfsStreamSubBlock {
    pub unpacked_data_size: u32,
    pub unpack_version: u8,
    pub method: u8,
    pub stream_crc32: u32,
    pub stream_name: Vec<u8>,
}

impl NtfsStreamSubBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let unpacked_data_size = read_u32(reader)?;
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let stream_crc32 = read_u32(reader)?;
        let stream_name_size = read_u16(reader)?.clamp(0, NAME_MAX_SIZE - 1) as usize;
        let mut stream_name = vec![0; stream_name_size];
        reader.read_exact(&mut stream_name)?;

        Ok(NtfsStreamSubBlock {
            unpacked_data_size,
            unpack_version,
            method,
            stream_crc32,
            stream_name,
        })
    }
}

#[derive(Debug)]
pub enum Sub {
    UnixOwner(UnixOwnerSubBlock),
    MacOsInfo(MacOsInfoSubBlock),
    ExtendedAttributes(ExtendedAttributesSubBlock),
    NtfsStream(NtfsStreamSubBlock),
    Unknown(u16),
}

#[derive(Debug)]
pub struct SubBlock {
    pub data_size: u32,
    pub level: u8,
    pub sub_block: Sub,
}

impl BlockRead for SubBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let data_size = read_u32(reader)?;
        let sub_type = read_u16(reader)?;
        let level = read_u8(reader)?;

        let sub_block = match sub_type.try_into() {
            Ok(t) => match t {
                SubBlockType::UnixOwner => {
                    let sub_block = UnixOwnerSubBlock::read(reader)?;
                    Sub::UnixOwner(sub_block)
                }
                SubBlockType::MacOsInfo => {
                    let sub_block = MacOsInfoSubBlock::read(reader)?;
                    Sub::MacOsInfo(sub_block)
                }
                SubBlockType::Os2ExtendedAttributes => {
                    let sub_block =
                        ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::Os2)?;
                    Sub::ExtendedAttributes(sub_block)
                }
                SubBlockType::BeOsExtendedAttributes => {
                    let sub_block =
                        ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::BeOs)?;
                    Sub::ExtendedAttributes(sub_block)
                }
                SubBlockType::NtfsAcl => {
                    let sub_block =
                        ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::Ntfs)?;
                    Sub::ExtendedAttributes(sub_block)
                }
                SubBlockType::NtfsStream => {
                    let sub_block = NtfsStreamSubBlock::read(reader)?;
                    Sub::NtfsStream(sub_block)
                }
            },
            Err(_) => Sub::Unknown(sub_type),
        };

        Ok(SubBlock {
            data_size,
            level,
            sub_block,
        })
    }
}

impl DataSize for SubBlock {
    fn data_size(&self) -> u64 {
        self.data_size as u64
    }
}

#[derive(Debug)]
pub struct SignBlock {
    // TODO flags?
    pub creation_time: u32,
    pub archive_name_size: u16,
    pub user_name_size: u16,
}

impl BlockRead for SignBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let creation_time = read_u32(reader)?;
        let archive_name_size = read_u16(reader)?;
        let user_name_size = read_u16(reader)?;

        Ok(SignBlock {
            creation_time,
            archive_name_size,
            user_name_size,
        })
    }
}

impl DataSize for SignBlock {
    fn data_size(&self) -> u64 {
        0
    }
}

#[derive(Debug)]
pub struct AvBlock {
    // TODO flags?
    pub unpack_version: u8,
    pub method: u8,
    pub av_version: u8,
    pub av_info_crc32: u32,
}

impl BlockRead for AvBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let av_version = read_u8(reader)?;
        let av_info_crc32 = read_u32(reader)?;

        Ok(AvBlock {
            unpack_version,
            method,
            av_version,
            av_info_crc32,
        })
    }
}

impl DataSize for AvBlock {
    fn data_size(&self) -> u64 {
        0
    }
}

#[derive(Debug)]
pub struct EndArchiveBlock {
    pub flags: EndArchiveBlockFlags,
    pub archive_data_crc32: Option<u32>,
    pub volume_number: Option<u16>,
}

#[derive(Debug, Clone, Copy)]
pub struct EndArchiveBlockFlags(u16);

impl EndArchiveBlockFlags {
    const NEXT_VOLUME: u16 = 0x0001;
    const DATACRC: u16 = 0x0002;
    const REVSPACE: u16 = 0x0004;
    const VOLNUMBER: u16 = 0x0008;

    pub fn new(flags: u16) -> Self {
        Self(flags)
    }

    pub fn has_next_volume(&self) -> bool {
        self.0 & Self::NEXT_VOLUME != 0
    }

    /// Store CRC32 of RAR archive (now is used only in volumes).
    pub fn has_crc32(&self) -> bool {
        self.0 & Self::DATACRC != 0
    }

    /// Reserve space for end of REV file 7 byte record.
    pub fn reserve_space(&self) -> bool {
        self.0 & Self::REVSPACE != 0
    }

    /// Store a number of current volume.
    pub fn has_volume_number(&self) -> bool {
        self.0 & Self::VOLNUMBER != 0
    }
}

impl BlockRead for EndArchiveBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self> {
        let flags = EndArchiveBlockFlags::new(flags);

        let archive_data_crc32 = if flags.has_crc32() {
            let archive_data_crc32 = read_u32(reader)?;
            Some(archive_data_crc32)
        } else {
            None
        };

        let volume_number = if flags.has_volume_number() {
            let volume_number = read_u16(reader)?;
            Some(volume_number)
        } else {
            None
        };

        Ok(EndArchiveBlock {
            flags,
            archive_data_crc32,
            volume_number,
        })
    }
}

impl DataSize for EndArchiveBlock {
    fn data_size(&self) -> u64 {
        0
    }
}

#[derive(Debug)]
pub struct UnknownBlock {
    pub tag: u8,
    pub data_size: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct UnknownBlockFlags(u16);

impl UnknownBlockFlags {
    const SKIP_IF_UNKNOWN: u16 = 0x4000;
    const LONG_BLOCK: u16 = 0x8000;

    pub fn new(flags: u16) -> Self {
        Self(flags)
    }

    pub fn skip_if_unknown(&self) -> bool {
        self.0 & Self::SKIP_IF_UNKNOWN != 0
    }

    pub fn contains_data(&self) -> bool {
        self.0 & Self::LONG_BLOCK != 0
    }
}

impl UnknownBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16, tag: u8) -> io::Result<Self> {
        let flags = UnknownBlockFlags::new(flags);

        let data_size = if flags.contains_data() {
            let data_size = read_u32(reader)?;
            Some(data_size)
        } else {
            None
        };

        Ok(UnknownBlock { tag, data_size })
    }
}

impl DataSize for UnknownBlock {
    fn data_size(&self) -> u64 {
        self.data_size.unwrap_or(0) as u64
    }
}

#[derive(Debug)]
pub enum BlockKind {
    Main(MainBlock),
    File(FileBlock),
    Service(ServiceBlock),
    EndArchive(EndArchiveBlock),
    Comment(CommentBlock),
    Av(AvBlock),
    Sub(SubBlock),
    Protect(ProtectBlock),
    Sign(SignBlock),
    Unknown(UnknownBlock),
}

mod block {
    pub const MAIN: u8 = 0x73;
    pub const FILE: u8 = 0x74;
    pub const COMMENT: u8 = 0x75;
    pub const AV: u8 = 0x76;
    pub const SUB: u8 = 0x77;
    pub const PROTECT: u8 = 0x78;
    pub const SIGN: u8 = 0x79;
    pub const SERVICE: u8 = 0x7a;
    pub const ENDARC: u8 = 0x7b;
}

#[derive(Debug)]
pub struct Block {
    pub position: u64,
    pub header_crc16: u16,
    pub header_size: u16,
    pub kind: BlockKind,
}

impl DataSize for Block {
    fn data_size(&self) -> u64 {
        match &self.kind {
            BlockKind::Main(b) => b.data_size(),
            BlockKind::File(b) => b.data_size(),
            BlockKind::Service(b) => b.data_size(),
            BlockKind::EndArchive(b) => b.data_size(),
            BlockKind::Comment(b) => b.data_size(),
            BlockKind::Av(b) => b.data_size(),
            BlockKind::Sub(b) => b.data_size(),
            BlockKind::Protect(b) => b.data_size(),
            BlockKind::Sign(b) => b.data_size(),
            BlockKind::Unknown(b) => b.data_size(),
        }
    }
}

impl Block {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let position = reader.stream_position()?;

        let header_crc16 = read_u16(reader)?;
        let block_type = read_u8(reader)?;
        let flags = read_u16(reader)?;
        let header_size = read_u16(reader)?;

        let kind = match block_type {
            block::MAIN => BlockKind::Main(MainBlock::read(reader, flags)?),
            block::FILE => BlockKind::File(FileBlock::read(reader, flags)?),
            block::SERVICE => BlockKind::Service(ServiceBlock::read(reader, flags, header_size)?),
            block::COMMENT => BlockKind::Comment(CommentBlock::read(reader, flags)?),
            block::AV => BlockKind::Av(AvBlock::read(reader, flags)?),
            block::SUB => BlockKind::Sub(SubBlock::read(reader, flags)?),
            block::PROTECT => BlockKind::Protect(ProtectBlock::read(reader, flags)?),
            block::SIGN => BlockKind::Sign(SignBlock::read(reader, flags)?),
            block::ENDARC => BlockKind::EndArchive(EndArchiveBlock::read(reader, flags)?),
            _ => BlockKind::Unknown(UnknownBlock::read(reader, flags, block_type)?),
        };

        Ok(Block {
            position,
            header_crc16,
            header_size,
            kind,
        })
    }

    pub fn header_size(&self) -> u64 {
        self.header_size as u64
    }

    pub fn full_size(&self) -> u64 {
        self.header_size() + self.data_size()
    }
}
