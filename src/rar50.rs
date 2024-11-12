use std::{io, ops::Deref};

use crate::{
    read::*,
    size::{DataSize, HeaderSize},
};

#[derive(Debug)]
pub struct Block {
    pub position: u64,
    pub flags: BaseHeaderFlags,
    pub header_crc32: u32,
    pub header_size: u64,
    pub extra_area_size: Option<u64>,
    pub data_size: Option<u64>,
    pub kind: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct BaseHeaderFlags(u64);

impl BaseHeaderFlags {
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

impl Deref for BaseHeaderFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

mod block {
    pub const MARKER: u64 = 0x00;
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

        let (kind, _) = read_vint(reader)?;

        let (flags, _) = read_vint(reader)?;
        let flags = BaseHeaderFlags::new(flags);

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
