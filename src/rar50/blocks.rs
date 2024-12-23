use std::{io, ops::Deref};

use crate::{read::*, size::BlockSize};

use super::{helpers::*, record_iterator::*, MAX_PATH_SIZE};

#[derive(Debug)]
pub struct Block {
    pub offset: u64,
    pub flags: CommonFlags,
    pub header_crc32: u32,
    pub header_size: u64,
    pub extra_area_size: Option<u64>,
    pub data_size: Option<u64>,
    pub kind: BlockKind,
}

flags! {
    pub struct CommonFlags(u16) {
        /// Additional extra area is present at the end of the block header.
        pub has_extra_area = 0x0001;

        /// Additional data area is present at the end of the block header.
        pub has_data_area = 0x0002;

        /// Unknown blocks with this flag must be skipped when updating an archive.
        pub skip_if_unknown = 0x0004;

        /// Data area of this block is continuing from the previous volume.
        pub split_before = 0x0008;

        /// Data area of this block is continuing in the next volume.
        pub split_after = 0x0010;

        /// Block depends on preceding file block.
        pub is_child = 0x0020;

        /// Preserve a child block if host is modified.
        pub is_inherited = 0x0040;
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

#[derive(Debug)]
struct CommonHeader {
    pub extra_area_size: Option<u64>,
}

impl Block {
    // const MARKER: u64 = 0x00;
    const MAIN: u64 = 0x01;
    const FILE: u64 = 0x02;
    const SERVICE: u64 = 0x03;
    const CRYPT: u64 = 0x04;
    const ENDARC: u64 = 0x05;

    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let offset = reader.stream_position()?;

        let header_crc32 = read_u32(reader)?;

        let (header_size, vint_size) = read_vint(reader)?;
        let full_header_size = header_size + vint_size as u64 + 4;

        let (header_type, _) = read_vint(reader)?;

        let (flags, _) = read_vint(reader)?;
        let flags = CommonFlags::new(flags as u16);

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

        let common_header = CommonHeader { extra_area_size };

        let kind = match header_type {
            Self::MAIN => BlockKind::Main(MainBlock::read(reader, &common_header)?),
            Self::FILE => BlockKind::File(FileBlock::read(reader, &common_header)?),
            Self::SERVICE => BlockKind::Service(ServiceBlock::read(reader, &common_header)?),
            Self::CRYPT => BlockKind::Crypt(CryptBlock::read(reader)?),
            Self::ENDARC => BlockKind::EndArchive(EndArchiveBlock::read(reader)?),
            _ => BlockKind::Unknown(UnknownBlock::read(reader, header_type)?),
        };

        Ok(Block {
            offset,
            flags,
            header_crc32,
            header_size: full_header_size,
            extra_area_size,
            data_size,
            kind,
        })
    }
}

impl Deref for Block {
    type Target = CommonFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

impl BlockSize for Block {
    fn offset(&self) -> u64 {
        self.offset
    }

    fn header_size(&self) -> u64 {
        self.header_size
    }

    fn data_size(&self) -> u64 {
        self.data_size.unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct MainBlock {
    pub flags: MainBlockFlags,
    pub volume_number: Option<u64>,
    pub locator: Option<LocatorRecord>,
    pub metadata: Option<MetadataRecord>,
    pub unknown_records: Vec<UnknownRecord>,
}

flags! {
    pub struct MainBlockFlags(u16) {
        /// Archive is part of a multi-volume archive.
        pub is_volume = 0x0001;

        /// Volume number field is present. True for all volumes except first.
        has_volume_number = 0x0002;

        /// https://en.wikipedia.org/wiki/Solid_compression
        pub is_solid = 0x0004;

        /// Contains a recovery record.
        // TODO document this better
        pub has_recovery_record = 0x0008;

        /// WinRAR will not modify this archive.
        pub is_locked = 0x0010;
    }
}

impl MainBlock {
    const LOCATOR: u64 = 0x0001;
    const METADATA: u64 = 0x0002;

    pub(self) fn read<R: io::Read + io::Seek>(
        reader: &mut R,
        common_header: &CommonHeader,
    ) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = MainBlockFlags::new(flags as u16);

        let volume_number = if flags.has_volume_number() {
            Some(read_vint(reader)?.0)
        } else {
            None
        };

        parse_records! {
            reader,
            common_header,
            unknown_records,

            let {
                locator: LocatorRecord = Self::LOCATOR,
                metadata: MetadataRecord = Self::METADATA,
            }
        }

        Ok(MainBlock {
            flags,
            volume_number,
            locator,
            metadata,
            unknown_records,
        })
    }
}

impl Deref for MainBlock {
    type Target = MainBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

#[derive(Debug)]
pub struct LocatorRecord {
    pub quick_open_record_offset: Option<u64>,
    pub recovery_record_offset: Option<u64>,
}

flags! {
    struct LocatorRecordFlags(u8) {
        has_quick_open_record_offset = 0x01;
        has_recovery_record_offset = 0x02;
    }
}

impl LocatorRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = LocatorRecordFlags::new(flags as u8);

        let quick_open_record_offset = if flags.has_quick_open_record_offset() {
            let (offset, _) = read_vint(reader)?;
            if offset == 0 {
                None
            } else {
                Some(offset)
            }
        } else {
            None
        };

        let recovery_record_offset = if flags.has_recovery_record_offset() {
            let (offset, _) = read_vint(reader)?;
            if offset == 0 {
                None
            } else {
                Some(offset)
            }
        } else {
            None
        };

        Ok(LocatorRecord {
            quick_open_record_offset,
            recovery_record_offset,
        })
    }
}

#[derive(Debug)]
pub struct MetadataRecord {
    pub name: Option<String>,
    pub creation_time: Option<Result<time::OffsetDateTime, u64>>,
}

flags! {
    struct MetadataRecordFlags(u8) {
        has_archive_name = 0x01;
        has_creation_time = 0x02;
        uses_unix_time = 0x04;
        is_unix_time_nanoseconds = 0x08;
    }
}

impl MetadataRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = MetadataRecordFlags::new(flags as u8);

        let name = if flags.has_archive_name() {
            let (name_size, _) = read_vint(reader)?;
            let name = read_vec(reader, name_size as usize)?;
            let name: Vec<_> = name.into_iter().take_while(|n| n != &0).collect();
            if name.is_empty() {
                None
            } else {
                Some(String::from_utf8(name).unwrap())
            }
        } else {
            None
        };

        let creation_time = if flags.has_creation_time() {
            let time = if flags.uses_unix_time() {
                if flags.is_unix_time_nanoseconds() {
                    read_unix_time_nanos(reader)?
                } else {
                    read_unix_time_sec(reader)?.map_err(|s| s as u64)
                }
            } else {
                if flags.is_unix_time_nanoseconds() {
                    // TODO log warning?
                }

                read_windows_time(reader)?
            };

            Some(time)
        } else {
            if flags.uses_unix_time() || flags.is_unix_time_nanoseconds() {
                // TODO log warning?
            }

            None
        };

        Ok(MetadataRecord {
            name,
            creation_time,
        })
    }
}

#[derive(Debug)]
pub struct FileBlock {
    pub flags: FileBlockFlags,

    /// Size of the file after decompression.
    /// May be unknown if the actual file size is larger than reported by OS
    /// or if file size is unknown such as for all volumes except last when archiving
    /// from stdin to multi-volume archive.
    pub unpacked_size: Option<u64>,

    /// OS-specific file attributes.
    pub attributes: u64,

    /// File modification time.
    pub modification_time: Option<Result<time::OffsetDateTime, u32>>,

    /// CRC32 of unpacked file.
    pub unpacked_data_crc32: Option<u32>,

    /// Compression settings for this file.
    pub compression_info: CompressionInfo,

    /// OS used to create the archive.
    pub host_os: HostOs,

    /// Name of the archived file.
    /// Forward slash is used as path separator for both Unix and Windows.
    pub name: Result<String, Vec<u8>>,

    pub encryption: Option<FileEncryptionRecord>,

    /// Hash of unpacked file.
    pub hash: Option<FileHashRecord>,

    pub extended_time: Option<FileTimeRecord>,

    pub version: Option<FileVersionRecord>,

    pub filesystem_redirection: Option<FileSystemRedirectionRecord>,

    pub unix_owner: Option<UnixOwnerRecord>,

    pub unknown_records: Vec<UnknownRecord>,
}

flags! {
    pub struct FileBlockFlags(u16) {
        pub is_directory = 0x0001;
        pub has_modification_time = 0x0002;
        pub has_crc32 = 0x0004;
        pub unknown_unpacked_size = 0x0008;
    }
}

int_enum! {
    pub enum HostOs : u8 {
        Windows = 0,
        Unix = 1,
    }
}

pub struct CompressionInfo(u64);

impl CompressionInfo {
    const ALGORITHM_MASK: u64 = 0x003f;
    const SOLID_MASK: u64 = 0x0040;
    const METHOD_MASK: u64 = 0x0380;
    const MIN_DICT_SIZE_MASK: u64 = 0x7c00;
    const DICT_SIZE_FACTOR_MASK: u64 = 0xf8000;
    const USES_PACK_5_ALGORITHM_MASK: u64 = 0x100_000;

    pub const MIN_DICT_SIZE: u64 = 0x20_000;
    pub const MAX_DICT_SIZE: u64 = 0x1_000_000_000;

    pub fn new(info: u64) -> Self {
        Self(info)
    }

    /// Version of WinRAR required to unpack the file.
    fn version(&self) -> CompressionAlgorithm {
        ((self.0 & Self::ALGORITHM_MASK) as u8).into()
    }

    fn uses_pack_5_algorithm(&self) -> bool {
        self.0 & Self::USES_PACK_5_ALGORITHM_MASK != 0
    }

    /// Actual version compression algorithm used to compress the file.
    pub fn algorithm(&self) -> CompressionAlgorithm {
        match self.version() {
            CompressionAlgorithm::Pack7 if self.uses_pack_5_algorithm() => {
                CompressionAlgorithm::Pack5
            }
            algo => algo,
        }
    }

    /// File spans multiple volumes.
    pub fn is_solid(&self) -> bool {
        self.0 & Self::SOLID_MASK != 0
    }

    pub fn method(&self) -> CompressionMethod {
        (((self.0 & Self::METHOD_MASK) >> 7) as u8).into()
    }

    /// Minimum dictionary size required to extract data.
    /// UnRAR seems to have a maximum dict size of 64GiB, so if we get more than that
    /// we return an error with the reported size.
    pub fn min_dictionary_size(&self) -> Result<u64, u64> {
        let factor = (self.0 & Self::MIN_DICT_SIZE_MASK) >> 10;

        let size = if self.version() == CompressionAlgorithm::Pack7 {
            let extra_factor = (self.0 & Self::DICT_SIZE_FACTOR_MASK) >> 15;

            let size = Self::MIN_DICT_SIZE << (factor & 0x1f);
            size + size / 32 * extra_factor
        } else {
            Self::MIN_DICT_SIZE << (factor & 0x0f)
        };

        if size <= Self::MAX_DICT_SIZE {
            Ok(size)
        } else {
            Err(size)
        }
    }
}

impl std::fmt::Debug for CompressionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompressionInfo")
            .field("algorithm", &self.algorithm())
            .field("is_solid", &self.is_solid())
            .field("method", &self.method())
            .field("min_dictionary_size", &self.min_dictionary_size())
            .finish()
    }
}

int_enum! {
    pub enum CompressionAlgorithm : u8 {
        Pack5 = 0x00,
        Pack7 = 0x01,
    }
}

int_enum! {
    pub enum CompressionMethod : u8 {
        NoCompression = 0x00,
        Method1 = 0x01,
        Method2 = 0x02,
        Method3 = 0x03,
        Method4 = 0x04,
        Method5 = 0x05,
    }
}

impl FileBlock {
    const CRYPT: u64 = 0x01;
    const HASH: u64 = 0x02;
    const HTIME: u64 = 0x03;
    const VERSION: u64 = 0x04;
    const REDIR: u64 = 0x05;
    const UOWNER: u64 = 0x06;

    pub(self) fn read<R: io::Read + io::Seek>(
        reader: &mut R,
        common_header: &CommonHeader,
    ) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = FileBlockFlags::new(flags as u16);

        let (unpacked_size, _) = read_vint(reader)?;
        let unpacked_size = if flags.unknown_unpacked_size() {
            None
        } else {
            Some(unpacked_size)
        };

        let (attributes, _) = read_vint(reader)?;

        let modification_time = if flags.has_modification_time() {
            Some(read_unix_time_sec(reader)?)
        } else {
            None
        };

        let unpacked_data_crc32 = if flags.has_crc32() {
            Some(read_u32(reader)?)
        } else {
            None
        };

        let (compression_info, _) = read_vint(reader)?;
        let compression_info = CompressionInfo::new(compression_info);

        let (host_os, _) = read_vint(reader)?;
        let (name_length, _) = read_vint(reader)?;

        let name = read_vec(reader, name_length.clamp(0, MAX_PATH_SIZE) as usize)?;
        let name = unmap_high_ascii_chars(name);

        parse_records! {
            reader,
            common_header,
            unknown_records,

            let {
                encryption: FileEncryptionRecord = Self::CRYPT,
                hash: FileHashRecord = Self::HASH,
                extended_time: FileTimeRecord = Self::HTIME,
                version: FileVersionRecord = Self::VERSION,
                filesystem_redirection: FileSystemRedirectionRecord = Self::REDIR,
                unix_owner: UnixOwnerRecord = Self::UOWNER,
            }
        }

        Ok(FileBlock {
            flags,
            unpacked_size,
            attributes,
            modification_time,
            unpacked_data_crc32,
            compression_info,
            host_os: (host_os as u8).into(),
            name,
            encryption,
            hash,
            extended_time,
            version,
            filesystem_redirection,
            unix_owner,
            unknown_records,
        })
    }

    pub fn modification_time(&self) -> Option<Result<time::OffsetDateTime, u64>> {
        if let Some(t) = &self.extended_time {
            if let Some(t) = &t.modification_time {
                return Some(*t);
            }
        }

        self.modification_time.map(|r| r.map_err(|t| t as u64))
    }
}

impl Deref for FileBlock {
    type Target = FileBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}
#[derive(Debug)]
pub struct ServiceBlock {
    pub flags: ServiceBlockFlags,
    pub unpacked_size: Option<u64>,
    pub modification_time: Option<Result<time::OffsetDateTime, u32>>,
    pub data_crc32: Option<u32>,
    pub compression_info: CompressionInfo,
    pub host_os: HostOs,

    pub encryption: Option<FileEncryptionRecord>,

    pub hash: Option<FileHashRecord>,

    pub extended_time: Option<FileTimeRecord>,

    pub version: Option<FileVersionRecord>,

    pub filesystem_redirection: Option<FileSystemRedirectionRecord>,

    pub unix_owner: Option<UnixOwnerRecord>,

    pub unknown_records: Vec<UnknownRecord>,

    pub kind: ServiceBlockKind,
}

flags! {
    pub struct ServiceBlockFlags(u16) {
        pub has_modification_time = 0x0002;
        pub has_crc32 = 0x0004;
        pub unknown_unpacked_size = 0x0008;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ServiceBlockType {
    Comment,
    QuickOpen,
    NtfsFilePermissions,
    NtfsAlternateDataStream,
    RecoveryRecord,
}

impl ServiceBlockType {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"CMT" => Some(Self::Comment),
            b"QO" => Some(Self::QuickOpen),
            b"ACL" => Some(Self::NtfsFilePermissions),
            b"STM" => Some(Self::NtfsAlternateDataStream),
            b"RR" => Some(Self::RecoveryRecord),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ServiceBlockKind {
    Comment(CommentServiceBlock),
    QuickOpen(QuickOpenServiceBlock),
    NtfsFilePermissions,
    NtfsAlternateDataStream,
    RecoveryRecord(RecoveryRecordServiceBlock),
    Unknown(Vec<u8>),
}

#[derive(Debug)]
// Does not contain any records.
pub struct QuickOpenServiceBlock;

#[derive(Debug)]
// Does not contain any records.
pub struct CommentServiceBlock;

#[derive(Debug)]
pub struct RecoveryRecordServiceBlock {
    // It is probably illegal for this to be missing.
    pub info: Option<RecoveryRecordInfo>,
}

#[derive(Debug)]
/// The recovery record is not used in WinRAR.
/// Here is more information about it.
/// https://www.win-rar.com/faq-passwords.html?&L=0
pub struct RecoveryRecordInfo {
    /// Percentage of the record size in relation to the archive.
    pub percentage: u8,

    /// Usually two bytes, unrelated to the size of the archive.
    pub unknown: Vec<u8>,
}

impl RecoveryRecordInfo {
    fn read<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let percentage = read_u8(reader)?;
        let mut unknown = vec![];
        // Assumes we're reading from the cursor.
        reader.read_to_end(&mut unknown)?;

        Ok(RecoveryRecordInfo {
            percentage,
            unknown,
        })
    }
}

impl ServiceBlock {
    const CRYPT: u64 = 0x01;
    const HASH: u64 = 0x02;
    const HTIME: u64 = 0x03;
    const VERSION: u64 = 0x04;
    const REDIR: u64 = 0x05;
    const UOWNER: u64 = 0x06;
    const SERVICE_DATA: u64 = 0x07;

    fn read<R: io::Read + io::Seek>(
        reader: &mut R,
        common_header: &CommonHeader,
    ) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = ServiceBlockFlags::new(flags as u16);

        let (unpacked_size, _) = read_vint(reader)?;
        let unpacked_size = if flags.unknown_unpacked_size() {
            None
        } else {
            Some(unpacked_size)
        };

        let (attributes, _) = read_vint(reader)?;
        if attributes != 0 {
            // log a warning or something
        }

        let modification_time = if flags.has_modification_time() {
            Some(read_unix_time_sec(reader)?)
        } else {
            None
        };

        let data_crc32 = if flags.has_crc32() {
            Some(read_u32(reader)?)
        } else {
            None
        };

        let (compression_info, _) = read_vint(reader)?;
        let compression_info = CompressionInfo::new(compression_info);

        let (host_os, _) = read_vint(reader)?;

        let (name_length, _) = read_vint(reader)?;
        let name = read_vec(reader, name_length as usize)?;
        let name = ServiceBlockType::from_bytes(&name).ok_or(name);

        let mut recovery_record = None;

        parse_records! {
            reader,
            common_header,
            unknown_records,

            let {
                encryption: FileEncryptionRecord = Self::CRYPT,
                hash: FileHashRecord = Self::HASH,
                extended_time: FileTimeRecord = Self::HTIME,
                version: FileVersionRecord = Self::VERSION,
                filesystem_redirection: FileSystemRedirectionRecord = Self::REDIR,
                unix_owner: UnixOwnerRecord = Self::UOWNER,
            }

            match record {
                Self::SERVICE_DATA => {
                    match name {
                        Ok(ServiceBlockType::RecoveryRecord) => {
                            recovery_record = Some(RecoveryRecordInfo::read(&mut record.data)?);
                        }
                        _ => {
                            unknown_records.push(UnknownRecord::new(Self::SERVICE_DATA))
                        }
                    }
                }
            }
        }

        let kind = match name {
            Ok(ServiceBlockType::Comment) => ServiceBlockKind::Comment(CommentServiceBlock),
            Ok(ServiceBlockType::QuickOpen) => ServiceBlockKind::QuickOpen(QuickOpenServiceBlock),
            Ok(ServiceBlockType::NtfsFilePermissions) => ServiceBlockKind::NtfsFilePermissions,
            Ok(ServiceBlockType::NtfsAlternateDataStream) => {
                ServiceBlockKind::NtfsAlternateDataStream
            }
            Ok(ServiceBlockType::RecoveryRecord) => {
                ServiceBlockKind::RecoveryRecord(RecoveryRecordServiceBlock {
                    info: recovery_record,
                })
            }
            Err(name) => ServiceBlockKind::Unknown(name),
        };

        Ok(ServiceBlock {
            flags,
            unpacked_size,
            modification_time,
            data_crc32,
            compression_info,
            host_os: (host_os as u8).into(),
            encryption,
            hash,
            extended_time,
            version,
            filesystem_redirection,
            unix_owner,
            unknown_records,
            kind,
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
pub struct FileEncryptionRecord {
    pub flags: FileEncryptionRecordFlags,
    pub kdf_count: u8,
    pub salt: [u8; 16],
    pub iv: [u8; 16],
    pub check_value: Option<[u8; 12]>,
}

flags! {
    pub struct FileEncryptionRecordFlags(u8) {
        pub has_password_check = 0x01;
        pub uses_mac_checksum = 0x02;
    }
}

impl FileEncryptionRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = FileEncryptionRecordFlags::new(flags as u8);

        let kdf_count = read_u8(reader)?;
        let salt = read_const_bytes(reader)?;
        let iv = read_const_bytes(reader)?;

        let check_value = if flags.has_password_check() {
            Some(read_const_bytes(reader)?)
        } else {
            None
        };

        Ok(FileEncryptionRecord {
            flags,
            kdf_count,
            salt,
            iv,
            check_value,
        })
    }
}

#[derive(Debug)]
pub struct FileHashRecord {
    pub hash: FileHash,
}

#[derive(Debug)]
pub enum FileHash {
    Blake2Sp([u8; 32]),
    Unknown(u64),
}

impl FileHash {
    pub(self) const BLAKE2SP: u64 = 0x00;
}

impl FileHashRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (hash_type, _) = read_vint(reader)?;

        let hash = match hash_type {
            FileHash::BLAKE2SP => FileHash::Blake2Sp(read_const_bytes(reader)?),
            _ => FileHash::Unknown(hash_type),
        };

        Ok(FileHashRecord { hash })
    }
}

#[derive(Debug)]
pub struct FileTimeRecord {
    pub modification_time: Option<Result<time::OffsetDateTime, u64>>,
    pub creation_time: Option<Result<time::OffsetDateTime, u64>>,
    pub access_time: Option<Result<time::OffsetDateTime, u64>>,
}

flags! {
    struct FileTimeRecordFlags(u8) {
        pub uses_unix_time = 0x01;
        pub has_modification_time = 0x02;
        pub has_creation_time = 0x04;
        pub has_access_time = 0x08;
        pub has_unix_time_nanoseconds = 0x10;
    }
}

impl FileTimeRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = FileTimeRecordFlags::new(flags as u8);

        let mut modification_time = None;
        let mut creation_time = None;
        let mut access_time = None;

        if flags.uses_unix_time() {
            if flags.has_modification_time() {
                modification_time = Some(read_unix_time_sec(reader)?.map_err(|s| s as u64))
            }

            if flags.has_creation_time() {
                creation_time = Some(read_unix_time_sec(reader)?.map_err(|s| s as u64))
            }

            if flags.has_access_time() {
                access_time = Some(read_unix_time_sec(reader)?.map_err(|s| s as u64))
            }

            if !flags.has_unix_time_nanoseconds() {
                return Ok(FileTimeRecord {
                    modification_time,
                    creation_time,
                    access_time,
                });
            }

            if let Some(t) = modification_time {
                let nanos = read_u32(reader)? as i64;
                modification_time =
                    Some(t.map(|x| x.saturating_add(time::Duration::nanoseconds(nanos))))
            }

            if let Some(t) = creation_time {
                let nanos = read_u32(reader)? as i64;
                creation_time =
                    Some(t.map(|x| x.saturating_add(time::Duration::nanoseconds(nanos))))
            }

            if let Some(t) = access_time {
                let nanos = read_u32(reader)? as i64;
                access_time = Some(t.map(|x| x.saturating_add(time::Duration::nanoseconds(nanos))))
            }

            Ok(FileTimeRecord {
                modification_time,
                creation_time,
                access_time,
            })
        } else {
            if flags.has_unix_time_nanoseconds() {
                // TODO log warning
            }

            if flags.has_modification_time() {
                modification_time = Some(read_windows_time(reader)?)
            }

            if flags.has_creation_time() {
                creation_time = Some(read_windows_time(reader)?)
            }

            if flags.has_access_time() {
                access_time = Some(read_windows_time(reader)?)
            }

            Ok(FileTimeRecord {
                modification_time,
                creation_time,
                access_time,
            })
        }
    }
}

#[derive(Debug)]
pub struct FileVersionRecord {
    pub version_number: u64,
}

impl FileVersionRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        // Unused as of now
        let (_flags, _) = read_vint(reader)?;
        let (version_number, _) = read_vint(reader)?;

        Ok(FileVersionRecord { version_number })
    }
}

#[derive(Debug)]
pub struct FileSystemRedirectionRecord {
    pub redirection_type: FileSystemRedirectionType,
    pub flags: FileSystemRedirectionRecordFlags,
    pub name: String,
}

int_enum! {
    pub enum FileSystemRedirectionType : u16 {
        UnixSymlink = 0x0001,
        WindowsSymlink = 0x0002,
        WindowsJunction = 0x0003,
        HardLink = 0x0004,
        FileCopy = 0x0005,
    }
}

flags! {
    pub struct FileSystemRedirectionRecordFlags(u16) {
        pub is_directory = 0x0001;
    }
}

impl FileSystemRedirectionRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (redirection_type, _) = read_vint(reader)?;
        let redirection_type = (redirection_type as u16).into();

        let (flags, _) = read_vint(reader)?;
        let flags = FileSystemRedirectionRecordFlags::new(flags as u16);

        let (name_length, _) = read_vint(reader)?;
        let name = read_vec(reader, name_length as usize)?;
        let name = String::from_utf8(name).unwrap();

        Ok(FileSystemRedirectionRecord {
            redirection_type,
            flags,
            name,
        })
    }
}

#[derive(Debug)]
pub struct UnixOwnerRecord {
    pub user_name: Option<Result<String, Vec<u8>>>,
    pub group_name: Option<Result<String, Vec<u8>>>,
    pub user_id: Option<u64>,
    pub group_id: Option<u64>,
}

flags! {
    struct UnixOwnerRecordFlags(u8) {
        pub has_user_name = 0x01;
        pub has_group_name = 0x02;
        pub has_user_id = 0x04;
        pub has_group_id = 0x08;
    }
}

impl UnixOwnerRecord {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = UnixOwnerRecordFlags::new(flags as u8);

        let mut user_name = None;
        let mut group_name = None;
        let mut user_id = None;
        let mut group_id = None;

        if flags.has_user_name() {
            let (size, _) = read_vint(reader)?;
            user_name = Some(read_string(reader, size as usize)?)
        }

        if flags.has_group_name() {
            let (size, _) = read_vint(reader)?;
            group_name = Some(read_string(reader, size as usize)?)
        }

        if flags.has_user_id() {
            user_id = Some(read_vint(reader)?.0)
        }

        if flags.has_group_id() {
            group_id = Some(read_vint(reader)?.0)
        }

        Ok(UnixOwnerRecord {
            user_name,
            group_name,
            user_id,
            group_id,
        })
    }
}

#[derive(Debug)]
pub struct CryptBlock {
    pub encryption_version: EncryptionVersion,
    pub kdf_count: u8,
    pub salt: [u8; 16],
    pub check_value: Option<[u8; 12]>,
}

flags! {
    struct CryptBlockFlags(u16) {
        has_password_check = 0x0001;
    }
}

int_enum! {
    pub enum EncryptionVersion : u8 {
        Aes256 = 0,
    }
}

impl CryptBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (encryption_version, _) = read_vint(reader)?;
        let encryption_version = (encryption_version as u8).into();

        let (flags, _) = read_vint(reader)?;
        let flags = CryptBlockFlags::new(flags as u16);

        let kdf_count = read_u8(reader)?;
        let salt = read_const_bytes(reader)?;

        let check_value = if flags.has_password_check() {
            Some(read_const_bytes(reader)?)
        } else {
            None
        };

        Ok(CryptBlock {
            encryption_version,
            kdf_count,
            salt,
            check_value,
        })
    }
}

#[derive(Debug)]
pub struct EndArchiveBlock {
    pub flags: EndArchiveBlockFlags,
}

flags! {
    pub struct EndArchiveBlockFlags(u16) {
        pub has_next_volume = 0x0001;
    }
}

impl EndArchiveBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let (flags, _) = read_vint(reader)?;
        let flags = EndArchiveBlockFlags::new(flags as u16);

        Ok(EndArchiveBlock { flags })
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
    pub tag: u64,
}

impl UnknownBlock {
    pub fn read<R: io::Read + io::Seek>(_reader: &mut R, tag: u64) -> io::Result<Self> {
        Ok(UnknownBlock { tag })
    }
}

#[derive(Debug)]
pub struct UnknownRecord {
    pub tag: u64,
}

impl UnknownRecord {
    pub fn new(tag: u64) -> Self {
        Self { tag }
    }
}
