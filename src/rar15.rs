use std::io;

use crate::read::*;

const NAME_MAX_SIZE: u16 = 1000;

pub trait BlockRead: Sized {
    fn read<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<Self>;
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

#[derive(Debug)]
pub struct FileBlock {
    pub packed_file_size: u32,
    pub unpacked_file_size: u32,
    pub host_os: u8,
    pub file_crc32: u32,
    pub mtime: time::PrimitiveDateTime,
    pub unpack_version: u8,
    pub method: u8,
    pub attributes: u32,
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

#[derive(Debug)]
pub struct ProtectBlock {
    // TODO do we need flags?
    pub data_size: u32,
    pub version: u8,
    pub recovery_sectors: u16,
    pub total_blocks: u32,
    pub mark: [u8; 8],
}

impl BlockRead for ProtectBlock {
    fn read<R: io::Read + io::Seek>(reader: &mut R, _flags: u16) -> io::Result<Self> {
        let data_size = read_u32(reader)?;
        let version = read_u8(reader)?;
        let recovery_sectors = read_u16(reader)?;
        let total_blocks = read_u32(reader)?;
        let mut mark = [0; 8];
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

#[derive(Debug)]
pub struct BaseHeader {
    pub position: u64,
    pub header_size: u16,
    pub header_crc16: u16,
    pub flags: u16,
}

pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<()> {
    let header_crc16 = read_u16(reader)?;
    let header_type = read_u8(reader)?;
    let flags = read_u16(reader)?;
    let header_size = read_u16(reader)?;

    Ok(())
}
