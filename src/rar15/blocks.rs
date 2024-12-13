use std::{io, ops::Deref};

use crate::{read::*, size::BlockSize, time_conv};

use super::{decode_file_name::decode_file_name, extended_time::ExtendedTime, NAME_MAX_SIZE};

#[derive(Debug)]
/// A generic RAR15 block.
pub struct Block {
    /// Offset of this block from the start of the file.
    pub offset: u64,

    /// CRC16 hash of the header.
    pub header_crc16: u16,

    /// Size of the header.
    pub header_size: u16,

    /// Specific type of this block.
    pub kind: BlockKind,
}

flags! {
    /// Flags that are common to all blocks.
    pub struct CommonFlags(u16) {
        /// Unknown blocks with this flag must be skipped when updating
        /// an archive.
        pub skip_if_unknown = 0x4000;

        /// Data area is present in the end of block header.
        pub contains_data = 0x8000;
    }
}

impl Block {
    const MAIN: u8 = 0x73;
    const FILE: u8 = 0x74;
    const COMMENT: u8 = 0x75;
    const AV: u8 = 0x76;
    const SUB: u8 = 0x77;
    const PROTECT: u8 = 0x78;
    const SIGN: u8 = 0x79;
    const SERVICE: u8 = 0x7a;
    const ENDARC: u8 = 0x7b;

    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let offset = reader.stream_position()?;

        let header_crc16 = read_u16(reader)?;
        let block_type = read_u8(reader)?;
        let flags = read_u16(reader)?;
        let header_size = read_u16(reader)?;

        let kind = match block_type {
            Self::MAIN => BlockKind::Main(MainBlock::read(reader, flags)?),
            Self::FILE => BlockKind::File(FileBlock::read(reader, flags)?),
            Self::SERVICE => BlockKind::Service(ServiceBlock::read(reader, flags, header_size)?),
            Self::COMMENT => BlockKind::Comment(CommentBlock::read(reader, flags)?),
            Self::AV => BlockKind::Av(AvBlock::read(reader, flags)?),
            Self::SUB => BlockKind::Sub(SubBlock::read(reader, flags)?),
            Self::PROTECT => BlockKind::Protect(ProtectBlock::read(reader, flags)?),
            Self::SIGN => BlockKind::Sign(SignBlock::read(reader, flags)?),
            Self::ENDARC => BlockKind::EndArchive(EndArchiveBlock::read(reader, flags)?),
            _ => BlockKind::Unknown(UnknownBlock::read(reader, flags, block_type)?),
        };

        Ok(Block {
            offset,
            header_crc16,
            header_size,
            kind,
        })
    }
}

impl BlockSize for Block {
    fn offset(&self) -> u64 {
        self.offset
    }

    fn header_size(&self) -> u64 {
        self.header_size as u64
    }

    fn data_size(&self) -> u64 {
        match &self.kind {
            BlockKind::File(b) => b.packed_data_size,
            BlockKind::Service(b) => b.packed_data_size,
            BlockKind::Sub(b) => b.data_size as u64,
            BlockKind::Protect(b) => b.data_size as u64,
            BlockKind::Unknown(b) => b.data_size.unwrap_or(0) as u64,
            BlockKind::Main(_)
            | BlockKind::EndArchive(_)
            | BlockKind::Comment(_)
            | BlockKind::Av(_)
            | BlockKind::Sign(_) => 0,
        }
    }
}

#[derive(Debug)]
/// Concrete block type.
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

#[derive(Debug)]
/// Main block containing archive metadata.
///
/// This should be the first block in the archive.
pub struct MainBlock {
    /// Flags containing archive metadata.
    pub flags: MainBlockFlags,

    /// Offset of the authenticity verification block in the archive.
    pub av_block_offset: Option<u64>,

    /// Version of the encryption used to encrypt the archive.
    pub encrypt_version: Option<u8>,
}

flags! {
    /// [`MainBlock`] flags.
    pub struct MainBlockFlags(u16) {
        /// Archive spans [multiple volumes][1].
        ///
        /// [1]: https://www.win-rar.com/split-files-archive.html?&L=0
        pub is_volume = 0x0001;

        /// Main header contains a comment.
        ///
        /// This called is an old-style (up to RAR 2.90) comment.
        pub has_comment = 0x0002;

        /// WinRAR will not modify this archive.
        pub is_locked = 0x0004;

        /// Archive uses [solid compression][1].
        ///
        /// [1]: https://en.wikipedia.org/wiki/Solid_compression
        pub is_solid = 0x0008;

        /// In a multi-volume archive, indicates that the filenames end with
        /// `{.part01.rar, .part02.rar, ..., .partNN.rar}` rather than with
        /// `{.rar, .r00, .r01, ... .rNN}`
        pub uses_new_numbering = 0x0010;

        /// The archive includes some additional metadata like archive name,
        /// creation date and owner of the WinRAR license.
        pub has_authenticity_verification = 0x0020;

        /// Archive contains a recovery record.
        // TODO document this better
        pub has_recovery_record = 0x0040;

        /// Archive is password-encrypted.
        pub has_password = 0x0080;

        /// Archive is the first volume in a multi-volume archive.
        /// Set only by RAR 3.0+
        pub is_first_volume = 0x0100;

        /// Indicates whether encryption is present in the archive.
        pub(self) has_encrypt_version = 0x0200;
    }
}

impl MainBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self> {
        let flags = MainBlockFlags::new(flags);

        let high_av_offset = read_u16(reader)? as u64;
        let low_av_offset = read_u32(reader)? as u64;
        let av_offset = low_av_offset | (high_av_offset << 32);
        let av_block_offset = if av_offset == 0 {
            None
        } else {
            Some(av_offset)
        };

        // This is not even read in newer versions of unrar
        let encrypt_version = if flags.has_encrypt_version() {
            let encrypt_version = read_u8(reader)?;
            Some(encrypt_version)
        } else {
            None
        };

        Ok(MainBlock {
            flags,
            av_block_offset,
            encrypt_version,
        })
    }
}

impl Deref for MainBlock {
    type Target = MainBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

int_enum! {
    /// OS of the host system used to add the file to the archive.
    pub enum HostOs : u8 {
        /// MS-DOS
        MsDos = 0,

        /// OS/2
        Os2 = 1,

        /// Windows
        Win32 = 2,

        /// Unix-like (Linux, OS X/macOS)
        Unix = 3,

        /// Classic Mac OS (not to be confused with OS X/macOS)
        MacOs = 4,

        /// BeOS
        BeOs = 5,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Encryption method used to encrypt the files in the archive.
pub enum EncryptionMethod {
    Rar13,
    Rar15,
    Rar20,
    Rar30,
}

impl From<u8> for EncryptionMethod {
    fn from(value: u8) -> Self {
        match value {
            13 => EncryptionMethod::Rar13,
            15 => EncryptionMethod::Rar15,
            20 | 26 => EncryptionMethod::Rar20,
            _ => EncryptionMethod::Rar30,
        }
    }
}

#[derive(Debug)]
/// Block containing a file or a directory.
///
/// The block(s?) following this one may contain additional metadata for the file.
pub struct FileBlock {
    /// File block flags.
    pub flags: FileBlockFlags,

    /// Size of the data section of the block.
    pub packed_data_size: u64,

    /// Size of the file after decompression.
    pub unpacked_data_size: u64,

    /// OS used to add this file the archive.
    pub host_os: HostOs,

    /// CRC32 hash of the file.
    pub file_crc32: u32,

    /// Modification time of the file.
    pub modification_time: Result<time::PrimitiveDateTime, u32>,

    /// Creation time of the file.
    pub creation_time: Option<Result<time::PrimitiveDateTime, u32>>,

    /// Access time of the file.
    pub access_time: Option<Result<time::PrimitiveDateTime, u32>>,

    /// Timestamp at which the file was added to or updated in the archive.
    pub archive_time: Option<Result<time::PrimitiveDateTime, u32>>,

    // TODO enumerate these
    pub unpack_version: u8,

    // TODO enumerate these
    pub method: u8,

    /// File attributes, dependent on the OS.
    pub attributes: u32,

    /// Filename of the file.
    pub file_name: Filename,

    // TODO document this
    pub salt: Option<[u8; Self::SALT_SIZE]>,
}

flags! {
    /// [`FileBlock`] flags.
    pub struct FileBlockFlags(u16) {
        /// File block contains a comment in the header.
        pub has_comment = 0x0002;

        /// The file size is larger than u32::MAX.
        pub(self) has_large_size = 0x0100;

        /// Filename contains bytecode to decode it to Unicode.
        pub(self) has_unicode_filename = 0x0200;

        /// File is encrypted with salt.
        pub(self) has_salt = 0x0400;

        // TODO document this
        pub has_version = 0x0800;

        /// File may contain modification time, ctime and atime info in the header.
        pub(self) has_extended_time = 0x1000;

        // TODO not sure how this is used.
        // Seems to indicate that there's an extra area in the header
        // like the one in RAR5 blocks?
        pub has_extra_area = 0x2000;
    }
}

#[derive(Debug)]
/// Filename encoded either in Unicode or using the OEM code page.
pub enum Filename {
    /// Filename is encoded in Unicode and can be correctly decoded into UTF-8.
    Unicode(Result<String, Vec<u8>>),

    /// Filename was encoded using the current OEM code page but only contains
    /// characters in the ASCII range, so it can be safely decoded into UTF-8.
    Ascii(String),

    /// Filename was encoded using the current OEM code page and cannot be decoded
    /// on its own. The user must select an encoding and use
    /// [`encoding_rs`](https://crates.io/crates/encoding_rs) or
    /// [`oem_cp`](https://crates.io/crates/oem_cp) to decode it correctly.
    Oem(Vec<u8>),
}

impl FileBlock {
    const SALT_SIZE: usize = 8;

    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self> {
        let flags = FileBlockFlags::new(flags);

        let low_packed_data_size = read_u32(reader)? as u64;
        let low_unpacked_data_size = read_u32(reader)? as u64;
        let host_os = read_u8(reader)?.into();
        let file_crc32 = read_u32(reader)?;
        let modification_time = read_u32(reader)?;
        let mut modification_time =
            time_conv::parse_dos_datetime(modification_time).map_err(|_| modification_time);

        // TODO map the possible values
        let unpack_version = read_u8(reader)?;

        // TODO map the possible values
        let method = read_u8(reader)?;

        let name_size = read_u16(reader)? as usize;

        // TODO map the attributes
        let attributes = read_u32(reader)?;

        let (packed_data_size, unpacked_data_size) = if flags.has_large_size() {
            let high_packed_data_size = read_u32(reader)? as u64;
            let high_unpacked_data_size = read_u32(reader)? as u64;

            (
                (high_packed_data_size >> 32) | low_packed_data_size,
                (high_unpacked_data_size >> 32) | low_unpacked_data_size,
            )
        } else {
            (low_packed_data_size, low_unpacked_data_size)
        };

        let file_name = read_vec(reader, name_size)?;

        let file_name = if flags.has_unicode_filename() {
            Filename::Unicode(decode_file_name(file_name))
        } else if file_name.is_ascii() {
            let Ok(string) = String::from_utf8(file_name) else {
                unreachable!("we already checked that all characters are in the ASCII range");
            };

            Filename::Ascii(string)
        } else {
            Filename::Oem(file_name)
        };

        let salt = if flags.has_salt() {
            Some(read_const_bytes(reader)?)
        } else {
            None
        };

        let mut creation_time = None;
        let mut access_time = None;
        let mut archive_time = None;

        if flags.has_extended_time() {
            let ext = ExtendedTime::read(reader, modification_time)?;

            modification_time = ext.modification_time;
            creation_time = ext.creation_time;
            access_time = ext.access_time;
            archive_time = ext.archive_time;
        }

        Ok(FileBlock {
            flags,
            packed_data_size,
            unpacked_data_size,
            host_os,
            file_crc32,
            modification_time,
            creation_time,
            access_time,
            archive_time,
            unpack_version,
            method,
            attributes,
            file_name,
            salt,
        })
    }
}

// TODO the service block has basically the same subheads
// found in SubBlock, so we should parse them accordingly.
#[derive(Debug)]
/// Block containing metadata for the previons file block.
pub struct ServiceBlock {
    /// Service block flags.
    pub flags: ServiceBlockFlags,

    /// Size of the data section of the block.
    pub packed_data_size: u64,

    /// Size of the data section after decompression.
    pub unpacked_data_size: u64,

    /// OS used to add this block to the archive.
    pub host_os: HostOs,

    /// CRC32 hash of the data section.
    pub data_crc32: u32,

    /// Modification time of the file.
    pub modification_time: Result<time::PrimitiveDateTime, u32>,

    /// Creation time of the file.
    pub creation_time: Option<Result<time::PrimitiveDateTime, u32>>,

    /// Access time of the file.
    pub access_time: Option<Result<time::PrimitiveDateTime, u32>>,

    /// Timestamp at which the file was added to the archive.
    pub archive_time: Option<Result<time::PrimitiveDateTime, u32>>,

    // TODO enumerate these
    pub unpack_version: u8,

    // TODO enumerate these
    pub method: u8,

    /// Generic flags for all service block types.
    pub sub_flags: SubHeadFlags,

    /// Concrete type of this service block.
    pub kind: ServiceBlockKind,

    // TODO parse
    pub sub_data: Option<Vec<u8>>,

    // TODO document this
    pub salt: Option<[u8; 8]>,
}

flags! {
    /// [`ServiceBlock`] flags.
    pub struct ServiceBlockFlags(u16) {
        /// Service block contains a comment in the header.
        pub has_comment = 0x0002;

        /// The file size is larger than u32::MAX.
        pub(self) has_large_size = 0x0100;

        /// Data is encrypted with salt.
        pub(self) has_salt = 0x0400;

        // TODO document this
        pub has_version = 0x0800;

        /// Data may contain modification time, ctime and atime info in the header.
        pub(self) has_extended_time = 0x1000;

        // TODO not sure how this is used.
        // Seems to indicate that there's an extra area in the header
        // like the one in RAR5 blocks?
        pub has_extra_area = 0x2000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceBlockType {
    Comment,
    NtfsFilePermissions,
    NtfsAlternateDataStream,
    UnixOwner,
    AuthenticationVerification,
    RecoveryRecord,
    Os2ExtendedAttributes,
    BeOsExtendedAttributes,
}

impl ServiceBlockType {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"CMT" => Some(Self::Comment),
            b"ACL" => Some(Self::NtfsFilePermissions),
            b"STM" => Some(Self::NtfsAlternateDataStream),
            b"UOW" => Some(Self::UnixOwner),
            b"AV" => Some(Self::AuthenticationVerification),
            b"RR" => Some(Self::RecoveryRecord),
            b"EA2" => Some(Self::Os2ExtendedAttributes),
            b"EABE" => Some(Self::BeOsExtendedAttributes),
            _ => None,
        }
    }
}

#[derive(Debug)]
/// Concrete service block type.
pub enum ServiceBlockKind {
    Comment,
    NtfsFilePermissions,
    NtfsAlternateDataStream,
    UnixOwner,
    AuthenticationVerification,
    RecoveryRecord,
    Os2ExtendedAttributes,
    BeOsExtendedAttributes,
    Unknown(Vec<u8>),
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
        let host_os = read_u8(reader)?.into();
        let data_crc32 = read_u32(reader)?;
        let modification_time = read_u32(reader)?;
        let mut modification_time =
            time_conv::parse_dos_datetime(modification_time).map_err(|_| modification_time);
        let unpack_version = read_u8(reader)?;
        let method = read_u8(reader)?;
        let name_size = read_u16(reader)? as usize;

        let sub_flags = read_u32(reader)?;
        let sub_flags = SubHeadFlags::new(sub_flags);

        let (packed_data_size, unpacked_data_size) = if flags.has_large_size() {
            let high_packed_data_size = read_u32(reader)? as u64;
            let high_unpacked_data_size = read_u32(reader)? as u64;

            (
                (high_packed_data_size >> 32) | low_packed_data_size,
                (high_unpacked_data_size >> 32) | low_unpacked_data_size,
            )
        } else {
            (low_packed_data_size, low_unpacked_data_size)
        };

        let kind = read_vec(reader, name_size)?;
        let kind = match ServiceBlockType::from_bytes(&kind) {
            Some(ServiceBlockType::Comment) => ServiceBlockKind::Comment,
            Some(ServiceBlockType::NtfsFilePermissions) => ServiceBlockKind::NtfsFilePermissions,
            Some(ServiceBlockType::NtfsAlternateDataStream) => {
                ServiceBlockKind::NtfsAlternateDataStream
            }
            Some(ServiceBlockType::UnixOwner) => ServiceBlockKind::UnixOwner,
            Some(ServiceBlockType::AuthenticationVerification) => {
                ServiceBlockKind::AuthenticationVerification
            }
            Some(ServiceBlockType::RecoveryRecord) => ServiceBlockKind::RecoveryRecord,
            Some(ServiceBlockType::Os2ExtendedAttributes) => {
                ServiceBlockKind::Os2ExtendedAttributes
            }
            Some(ServiceBlockType::BeOsExtendedAttributes) => {
                ServiceBlockKind::BeOsExtendedAttributes
            }
            None => ServiceBlockKind::Unknown(kind),
        };

        let sub_data_size = (header_size as usize)
            - name_size
            - Self::SIZE
            - if flags.has_salt() { Self::SALT_SIZE } else { 0 };

        let sub_data = if sub_data_size > 0 {
            Some(read_vec(reader, sub_data_size)?)
        } else {
            None
        };

        let salt = if flags.has_salt() {
            Some(read_const_bytes(reader)?)
        } else {
            None
        };

        let mut creation_time = None;
        let mut access_time = None;
        let mut archive_time = None;

        if flags.has_extended_time() {
            let ext = ExtendedTime::read(reader, modification_time)?;

            modification_time = ext.modification_time;
            creation_time = ext.creation_time;
            access_time = ext.access_time;
            archive_time = ext.archive_time;
        }

        Ok(ServiceBlock {
            flags,
            packed_data_size,
            unpacked_data_size,
            host_os,
            data_crc32,
            modification_time,
            creation_time,
            access_time,
            archive_time,
            unpack_version,
            method,
            sub_flags,
            kind,
            sub_data,
            salt,
        })
    }
}

impl Deref for ServiceBlock {
    type Target = ServiceBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

flags! {
    /// Flags that are common to all service block headers.
    pub struct SubHeadFlags(u32) {
        // TODO document this
        pub is_inherited = 0x80000000;

        // SUBHEAD_FLAGS_CMT_UNICODE
        pub is_comment_unicode = 0x01;
    }
}

#[derive(Debug)]
/// Block containing the archive comment.
pub struct CommentBlock {
    /// Size of the comment after decompression.
    pub unpacked_data_size: u16,

    // TODO enumerate these
    pub unpack_version: u8,

    // TODO enumerate these
    pub method: u8,

    /// CRC15t6 hash of the comment.
    // TODO before or after decompression?
    pub crc16: u16,
}

impl CommentBlock {
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

    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let data_size = read_u32(reader)?;
        let version = read_u8(reader)?;
        let recovery_sectors = read_u16(reader)?;
        let total_blocks = read_u32(reader)?;
        let mark = read_const_bytes(reader)?;

        Ok(ProtectBlock {
            data_size,
            version,
            recovery_sectors,
            total_blocks,
            mark,
        })
    }
}

int_enum! {
    enum SubBlockType : u16 {
        // EA_HEAD
        Os2ExtendedAttributes = 0x100,
        // UO_HEAD
        UnixOwner = 0x101,
        // MAC_HEAD
        MacOsInfo = 0x102,
        // BEEA_HEAD
        BeOsExtendedAttributes = 0x103,
        // NTACL_HEAD
        NtfsFilePermissions = 0x104,
        // STREAM_HEAD
        NtfsAlternateDataStream = 0x105,
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
        let user = read_vec(reader, user_size)?;
        let group = read_vec(reader, group_size)?;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
        let stream_name = read_vec(reader, stream_name_size)?;

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
pub enum SubBlockKind {
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
    pub kind: SubBlockKind,
}

impl SubBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let data_size = read_u32(reader)?;
        let sub_type = read_u16(reader)?;
        let level = read_u8(reader)?;

        let sub_block = match sub_type.into() {
            SubBlockType::UnixOwner => {
                let sub_block = UnixOwnerSubBlock::read(reader)?;
                SubBlockKind::UnixOwner(sub_block)
            }
            SubBlockType::MacOsInfo => {
                let sub_block = MacOsInfoSubBlock::read(reader)?;
                SubBlockKind::MacOsInfo(sub_block)
            }
            SubBlockType::Os2ExtendedAttributes => {
                let sub_block =
                    ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::Os2)?;
                SubBlockKind::ExtendedAttributes(sub_block)
            }
            SubBlockType::BeOsExtendedAttributes => {
                let sub_block =
                    ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::BeOs)?;
                SubBlockKind::ExtendedAttributes(sub_block)
            }
            SubBlockType::NtfsFilePermissions => {
                let sub_block =
                    ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::Ntfs)?;
                SubBlockKind::ExtendedAttributes(sub_block)
            }
            SubBlockType::NtfsAlternateDataStream => {
                let sub_block = NtfsStreamSubBlock::read(reader)?;
                SubBlockKind::NtfsStream(sub_block)
            }
            SubBlockType::Unknown(_) => SubBlockKind::Unknown(sub_type),
        };

        Ok(SubBlock {
            data_size,
            level,
            kind: sub_block,
        })
    }
}

// TODO supposedly the creation_time is in DOS format
// and the archive and user name sizes are used to read the archive and user name
// later in the header, but we don't have much information about this block right now.
#[derive(Debug)]
pub struct SignBlock {
    pub creation_time: u32,
    pub archive_name_size: u16,
    pub user_name_size: u16,
}

impl SignBlock {
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

#[derive(Debug)]
pub struct AvBlock {
    // TODO flags?
    pub unpack_version: u8,
    pub method: u8,
    pub av_version: u8,
    pub av_info_crc32: u32,
}

impl AvBlock {
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

#[derive(Debug)]
/// Block signaling the end of the archive.
///
/// Typically added in multi-volume archives or when there is trailing data not part of the
/// archive in the file.
pub struct EndArchiveBlock {
    /// End archive block flags.
    pub flags: EndArchiveBlockFlags,

    // TODO document this
    pub archive_data_crc32: Option<u32>,

    /// Number of the current volume.
    pub volume_number: Option<u16>,
}

flags! {
    /// [`EndArchiveBlock`] flags.
    pub struct EndArchiveBlockFlags(u16) {
        /// Archive continues in the next volume.
        pub has_next_volume = 0x0001;

        /// Store CRC32 of RAR archive (only used in volumes).
        // TODO what?
        pub(self) has_crc32 = 0x0002;

        /// Reserve space for end of REV file 7 byte record.
        // TODO what??
        pub reserve_space = 0x0004;

        /// Store the number of the current volume.
        pub(self) has_volume_number = 0x0008;
    }
}

impl EndArchiveBlock {
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

impl Deref for EndArchiveBlock {
    type Target = EndArchiveBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

#[derive(Debug)]
/// Block that couldn't be decoded.
pub struct UnknownBlock {
    /// Tag identifying the block.
    pub tag: u8,

    /// Generic flags.
    pub flags: CommonFlags,

    /// Size of the data section.
    pub data_size: Option<u32>,
}

impl UnknownBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16, tag: u8) -> io::Result<Self> {
        let flags = CommonFlags::new(flags);

        let data_size = if flags.contains_data() {
            let data_size = read_u32(reader)?;
            Some(data_size)
        } else {
            None
        };

        Ok(UnknownBlock {
            tag,
            flags,
            data_size,
        })
    }
}

impl Deref for UnknownBlock {
    type Target = CommonFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}
