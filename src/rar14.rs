use std::{ffi::OsString, io, ops::Deref, os::unix::ffi::OsStringExt};

use crate::{
    dos_time,
    read::*,
    size::{DataSize, HeaderSize},
};

#[derive(Debug)]
pub struct MainHeader {
    pub position: u64,
    pub header_size: u16,
    pub flags: MainHeaderFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct MainHeaderFlags(u8);

impl MainHeaderFlags {
    const VOLUME: u8 = 0x0001;
    const COMMENT: u8 = 0x0002;
    const LOCK: u8 = 0x0004;
    const SOLID: u8 = 0x0008;
    const PACK_COMMENT: u8 = 0x0010;

    pub fn new(flags: u8) -> Self {
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

    /// Contains a comment
    pub fn has_comment(&self) -> bool {
        self.0 & Self::COMMENT != 0
    }

    /// Is the main header comment packed?
    pub fn is_comment_packed(&self) -> bool {
        self.0 & Self::PACK_COMMENT != 0
    }
}

impl Deref for MainHeaderFlags {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MainHeader {
    const SIGNATURE_SIZE: u64 = 4;
    const SIZE: u64 = Self::SIGNATURE_SIZE + 3;

    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let position = reader.stream_position()? - Self::SIGNATURE_SIZE;

        let header_size = read_u16(reader)?;
        let flags = read_u8(reader)?;

        Ok(MainHeader {
            position,
            header_size,
            flags: MainHeaderFlags::new(flags),
        })
    }

    pub fn read_comment<R: io::Read + io::Seek>(
        &self,
        reader: &mut R,
    ) -> io::Result<Option<Vec<u8>>> {
        if !self.flags.has_comment() {
            return Ok(None);
        }

        reader.seek(io::SeekFrom::Start(self.position + Self::SIZE))?;

        let size = read_u16(reader)? as usize;

        // TODO comment encoding?

        if !self.flags.is_comment_packed() {
            if size == 0 {
                return Ok(None);
            }

            let mut comment = vec![0; size];
            reader.read_exact(&mut comment)?;
            return Ok(Some(comment));
        }

        if size < 2 {
            return Ok(None);
        }

        let _unpacked_comment_size = read_u16(reader)? - 2;

        // TODO the comment is compressed
        // arccmt.cpp:70

        Ok(None)
    }
}

impl DataSize for MainHeader {
    fn data_size(&self) -> u64 {
        0
    }
}

impl HeaderSize for MainHeader {
    fn header_size(&self) -> u64 {
        self.header_size as u64
    }
}

#[derive(Debug)]
pub struct FileHeader {
    pub position: u64,
    pub header_size: u16,
    pub flags: FileHeaderFlags,
    pub packed_data_size: u32,
    pub unpacked_data_size: u32,
    pub crc16: u16,
    pub mtime: time::PrimitiveDateTime,
    pub attributes: u8,
    pub unpack_version: u8,
    pub method: u8,
    pub name: OsString,
}

#[derive(Debug, Clone, Copy)]
pub struct FileHeaderFlags(u8);

impl FileHeaderFlags {
    const SPLIT_BEFORE: u8 = 0x01;
    const SPLIT_AFTER: u8 = 0x02;
    const PASSWORD: u8 = 0x04;

    pub fn new(flags: u8) -> Self {
        FileHeaderFlags(flags)
    }

    /// The first file is split from the previous volume
    pub fn split_before(&self) -> bool {
        self.0 & Self::SPLIT_BEFORE != 0
    }

    /// The last file is split into the next volume
    pub fn split_after(&self) -> bool {
        self.0 & Self::SPLIT_AFTER != 0
    }

    /// The file is encrypted
    pub fn has_password(&self) -> bool {
        self.0 & Self::PASSWORD != 0
    }
}

impl Deref for FileHeaderFlags {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FileHeader {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<FileHeader> {
        let position = reader.stream_position()?;

        let packed_data_size = read_u32(reader)?;
        let unpacked_data_size = read_u32(reader)?;
        let crc16 = read_u16(reader)?;
        let header_size = read_u16(reader)?;
        let mtime = read_u32(reader)?;
        let attributes = read_u8(reader)?;
        let flags = read_u8(reader)?; // | LONG_BLOCK?
        let unpack_version = if read_u8(reader)? == 2 { 13 } else { 10 };
        let name_size = read_u8(reader)? as usize;
        let method = read_u8(reader)?;

        let mut name = vec![0; name_size];
        reader.read_exact(&mut name)?;

        // TODO this should be OS-agnostic.
        let name = OsString::from_vec(name);

        Ok(FileHeader {
            position,
            header_size,
            flags: FileHeaderFlags::new(flags),
            packed_data_size,
            unpacked_data_size,
            crc16,
            mtime: dos_time::parse(mtime),
            attributes,
            unpack_version,
            method,
            name,
        })
    }
}

impl HeaderSize for FileHeader {
    fn header_size(&self) -> u64 {
        self.header_size as u64
    }
}

impl DataSize for FileHeader {
    fn data_size(&self) -> u64 {
        self.packed_data_size as u64
    }
}