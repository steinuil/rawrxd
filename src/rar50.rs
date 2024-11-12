use std::{io, ops::Deref};

use crate::{
    read::*,
    size::{DataSize, HeaderSize},
};

#[derive(Debug)]
pub struct Block {
    pub position: u64,
    pub flags: CommonBlockFlags,
    pub header_crc32: u32,
    pub header_size: u64,
    pub extra_area_size: Option<u64>,
    pub data_size: Option<u64>,
    pub kind: u64,
}

#[derive(Debug)]
pub struct CommonBlockFlags(u64);

impl CommonBlockFlags {
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

    pub fn has_extra_area(&self) -> bool {
        self.0 & Self::EXTRA != 0
    }

    pub fn has_data_area(&self) -> bool {
        self.0 & Self::DATA != 0
    }

    pub fn skip_if_unknown(&self) -> bool {
        self.0 & Self::SKIP_IF_UNKNOWN != 0
    }

    pub fn split_before(&self) -> bool {
        self.0 & Self::SPLIT_BEFORE != 0
    }

    pub fn split_after(&self) -> bool {
        self.0 & Self::SPLIT_AFTER != 0
    }

    pub fn is_child(&self) -> bool {
        self.0 & Self::CHILD != 0
    }

    pub fn is_inherited(&self) -> bool {
        self.0 & Self::INHERITED != 0
    }
}

impl Deref for CommonBlockFlags {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Block {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let position = reader.stream_position()?;

        let header_crc32 = read_u32(reader)?;

        let (header_size, vint_size) = read_vint(reader)?;
        let full_header_size = header_size + vint_size as u64 + 4;

        let (header_type, _) = read_vint(reader)?;

        let (flags, _) = read_vint(reader)?;
        let flags = CommonBlockFlags::new(flags);

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
            kind: header_type,
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
