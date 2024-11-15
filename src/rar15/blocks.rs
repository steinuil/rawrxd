use std::{io, ops::Deref};

use crate::{read::*, size::BlockSize, time_conv};

use super::{decode_file_name::decode_file_name, extended_time::ExtendedTime, NAME_MAX_SIZE};

#[derive(Debug)]
pub struct Block {
    pub position: u64,
    pub header_crc16: u16,
    pub flags: CommonFlags,
    pub header_size: u16,
    pub kind: BlockKind,
}

flags! {
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
        let position = reader.stream_position()?;

        let header_crc16 = read_u16(reader)?;
        let block_type = read_u8(reader)?;
        let flags = read_u16(reader)?;
        let header_size = read_u16(reader)?;

        let common_flags = CommonFlags::new(flags);

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
            position,
            header_crc16,
            flags: common_flags,
            header_size,
            kind,
        })
    }
}

impl BlockSize for Block {
    fn position(&self) -> u64 {
        self.position
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
pub struct MainBlock {
    pub flags: MainBlockFlags,
    pub high_pos_av: u16,
    pub pos_av: u32,
    pub encrypt_version: Option<u8>,
}

flags! {
    pub struct MainBlockFlags(u16) {
        /// Archive is part of a multi-volume archive.
        pub is_volume = 0x0001;

        /// Main header contains a comment.
        /// This called is an old-style (up to RAR2.9) comment.
        pub has_comment = 0x0002;

        /// WinRAR will not modify this archive.
        pub is_locked = 0x0004;

        /// https://en.wikipedia.org/wiki/Solid_compression
        pub is_solid = 0x0008;

        /// In a multi-volume archive, indicates that the filenames end with
        /// {.part01.rar, .part02.rar, ..., .partNN.rar} rather than with
        /// {.rar, .r00, .r01, ... .rNN}
        pub uses_new_numbering = 0x0010;

        /// The archive includes some additional metadata like archive name,
        /// creation date and owner of the WinRAR license.
        pub has_authenticity_verification = 0x0020;

        /// Contains a recovery record.
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

impl Deref for MainBlock {
    type Target = MainBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

int_enum! {
    pub enum HostOs : u8 {
        MsDos = 0,
        Os2 = 1,
        Win32 = 2,
        Unix = 3,
        MacOs = 4,
        BeOs = 5,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct FileBlock {
    pub flags: FileBlockFlags,
    pub packed_data_size: u64,
    pub unpacked_data_size: u64,
    pub host_os: HostOs,
    pub file_crc32: u32,
    pub modification_time: Result<time::PrimitiveDateTime, u32>,
    pub creation_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub access_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub archive_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub unpack_version: u8,
    pub method: u8,
    pub attributes: u32,
    pub file_name: Result<String, Vec<u8>>,
    pub salt: Option<[u8; Self::SALT_SIZE]>,
}

flags! {
    pub struct FileBlockFlags(u16) {
        /// File block contains a comment in the header.
        pub has_comment = 0x0002;

        /// The file size is larger than u32::MAX.
        pub(self) has_large_size = 0x0100;

        /// Filename is in UTF-8 format.
        pub(self) is_filename_unicode = 0x0200;

        /// File is encrypted with salt.
        pub(self) has_salt = 0x0400;

        // TODO document this
        pub has_version = 0x0800;

        /// File may contain modification time, ctime and atime info in the header.
        pub has_extended_time = 0x1000;

        // TODO not sure how this is used.
        // Seems to indicate that there's an extra area in the header
        // like the one in RAR5 blocks?
        pub has_extra_area = 0x2000;
    }
}

impl FileBlock {
    const SALT_SIZE: usize = 8;

    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self> {
        let flags = FileBlockFlags::new(flags);

        let low_packed_data_size = read_u32(reader)? as u64;
        let low_unpacked_data_size = read_u32(reader)? as u64;
        let host_os = read_u8(reader)?;
        let file_crc32 = read_u32(reader)?;
        let modification_time = read_u32(reader)?;
        let mut modification_time =
            time_conv::parse_dos(modification_time).map_err(|_| modification_time);

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
                (high_packed_data_size >> 4) | low_packed_data_size,
                (high_unpacked_data_size >> 4) | low_unpacked_data_size,
            )
        } else {
            (low_packed_data_size, low_unpacked_data_size)
        };

        let file_name = if flags.is_filename_unicode() {
            let name = read_vec(reader, name_size as usize)?;
            decode_file_name(name)
        } else {
            // TODO decode the filename when it's not unicode.
            // On Unix it uses CharToWide, which is guaranteed to return garbage
            // if we get a string with a character > 127 or if the OEM code page does
            // not map ASCII characters to their normal meaning.
            // On Windows  it uses the current OEM code page (which I think is set by locale?)
            // so we could use this crate or at least suggest to use it:
            // https://crates.io/crates/oem_cp
            // For now we can just try to read unicode.
            read_string(reader, name_size as usize)?
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
            host_os: host_os.into(),
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
pub struct ServiceBlock {
    pub flags: ServiceBlockFlags,
    pub packed_data_size: u64,
    pub unpacked_data_size: u64,
    pub host_os: HostOs,
    pub file_crc32: u32,
    pub modification_time: Result<time::PrimitiveDateTime, u32>,
    pub creation_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub access_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub archive_time: Option<Result<time::PrimitiveDateTime, u32>>,
    pub unpack_version: u8,
    pub method: u8,
    pub sub_flags: u32,
    pub name: Result<String, Vec<u8>>,
    pub sub_data: Option<Vec<u8>>,
    pub salt: Option<[u8; 8]>,
}

flags! {
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
        pub has_extended_time = 0x1000;

        // TODO not sure how this is used.
        // Seems to indicate that there's an extra area in the header
        // like the one in RAR5 blocks?
        pub has_extra_area = 0x2000;
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
        let modification_time = read_u32(reader)?;
        let mut modification_time =
            time_conv::parse_dos(modification_time).map_err(|_| modification_time);
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

        let name = read_string(reader, name_size)?;

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
            host_os: host_os.into(),
            file_crc32,
            modification_time,
            creation_time,
            access_time,
            archive_time,
            unpack_version,
            method,
            sub_flags,
            name,
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

#[derive(Debug)]
pub struct CommentBlock {
    // TODO do we need flags?
    pub unpacked_data_size: u16,
    pub unpack_version: u8,
    pub method: u8,
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
    pub enum SubBlockType : u16 {
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
            SubBlockType::NtfsAcl => {
                let sub_block =
                    ExtendedAttributesSubBlock::read(reader, ExtendedAttributesFs::Ntfs)?;
                SubBlockKind::ExtendedAttributes(sub_block)
            }
            SubBlockType::NtfsStream => {
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

#[derive(Debug)]
pub struct SignBlock {
    // TODO flags?
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
pub struct EndArchiveBlock {
    pub flags: EndArchiveBlockFlags,
    pub archive_data_crc32: Option<u32>,
    pub volume_number: Option<u16>,
}

flags! {
    pub struct EndArchiveBlockFlags(u16) {
        /// Archive is part of a volume and continues in the next volume.
        pub has_next_volume = 0x0001;

        /// Store CRC32 of RAR archive (only used in volumes).
        pub has_crc32 = 0x0002;

        /// Reserve space for end of REV file 7 byte record.
        pub reserve_space = 0x0004;

        /// Store the number of the current volume.
        pub has_volume_number = 0x0008;
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
pub struct UnknownBlock {
    pub tag: u8,
    pub flags: CommonFlags,
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
