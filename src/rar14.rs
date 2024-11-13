use std::{ffi::OsString, io, os::unix::ffi::OsStringExt};

use crate::{
    dos_time,
    read::*,
    size::{DataSize, HeaderSize},
};

#[derive(Debug)]
pub struct MainBlock {
    pub position: u64,
    pub header_size: u16,
    pub flags: MainBlockFlags,
}

flags! {
    pub struct MainBlockFlags(u8) {
        /// Archive is part of a multi-volume archive.
        pub is_volume = 0x01;

        /// Main header contains a comment.
        pub has_comment = 0x02;

        /// WinRAR will not modify this archive.
        pub is_locked = 0x04;

        /// https://en.wikipedia.org/wiki/Solid_compression
        pub is_solid = 0x08;

        /// The comment in the header is packed.
        pub(self) is_comment_packed = 0x10;
    }
}

impl MainBlock {
    const SIGNATURE_SIZE: u64 = 4;
    const SIZE: u64 = Self::SIGNATURE_SIZE + 3;

    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let position = reader.stream_position()? - Self::SIGNATURE_SIZE;

        let header_size = read_u16(reader)?;
        let flags = read_u8(reader)?;
        let flags = MainBlockFlags::new(flags);

        Ok(MainBlock {
            position,
            header_size,
            flags,
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

            let comment = read_vec(reader, size)?;
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

impl DataSize for MainBlock {
    fn data_size(&self) -> u64 {
        0
    }
}

impl HeaderSize for MainBlock {
    fn header_size(&self) -> u64 {
        self.header_size as u64
    }
}

#[derive(Debug)]
pub struct FileBlock {
    pub position: u64,
    pub header_size: u16,
    pub flags: FileBlockFlags,
    pub packed_data_size: u32,
    pub unpacked_data_size: u32,
    pub crc16: u16,
    pub modification_time: time::PrimitiveDateTime,
    pub attributes: u8,
    pub unpack_version: u8,
    pub method: u8,
    pub name: OsString,
}

flags! {
    pub struct FileBlockFlags(u8) {
        /// File is continuing from previous volume.
        pub split_before = 0x01;

        /// File is continuing in next volume.
        pub split_after = 0x02;

        /// File is encrypted.
        pub has_password = 0x04;
    }
}

impl FileBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<FileBlock> {
        let position = reader.stream_position()?;

        let packed_data_size = read_u32(reader)?;
        let unpacked_data_size = read_u32(reader)?;
        let crc16 = read_u16(reader)?;
        let header_size = read_u16(reader)?;

        let modification_time = read_u32(reader)?;
        let modification_time = dos_time::parse(modification_time);

        let attributes = read_u8(reader)?;

        let flags = read_u8(reader)?;
        let flags = FileBlockFlags::new(flags);

        let unpack_version = if read_u8(reader)? == 2 { 13 } else { 10 };
        let name_size = read_u8(reader)? as usize;
        let method = read_u8(reader)?;
        let name = read_vec(reader, name_size)?;

        // TODO this should be OS-agnostic.
        let name = OsString::from_vec(name);

        Ok(FileBlock {
            position,
            header_size,
            flags,
            packed_data_size,
            unpacked_data_size,
            crc16,
            modification_time,
            attributes,
            unpack_version,
            method,
            name,
        })
    }
}

impl HeaderSize for FileBlock {
    fn header_size(&self) -> u64 {
        self.header_size as u64
    }
}

impl DataSize for FileBlock {
    fn data_size(&self) -> u64 {
        self.packed_data_size as u64
    }
}
