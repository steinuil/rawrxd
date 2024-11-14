use std::{ffi::OsString, io, ops::Deref, os::unix::ffi::OsStringExt};

use crate::{
    block::RarBlock,
    read::*,
    size::{DataSize, FullSize, HeaderSize},
    time_conv,
};

#[derive(Debug)]
pub struct BlockIterator<R: io::Read + io::Seek> {
    pub reader: R,
    pub file_size: u64,
    has_read_main_block: bool,
    next_block_position: u64,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    pub(crate) fn new(mut reader: R, file_size: u64) -> io::Result<Self> {
        let next_block_position = reader.stream_position()?;

        Ok(Self {
            reader,
            file_size,
            has_read_main_block: false,
            next_block_position,
        })
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = io::Result<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_block_position == self.file_size {
            return None;
        }

        if let Err(e) = self
            .reader
            .seek(io::SeekFrom::Start(self.next_block_position))
        {
            return Some(Err(e));
        }

        let block = if !self.has_read_main_block {
            let block = match MainBlock::read(&mut self.reader) {
                Ok(block) => block,
                Err(e) => return Some(Err(e)),
            };

            self.has_read_main_block = true;

            Block {
                kind: BlockKind::Main(block),
            }
        } else {
            let block = match FileBlock::read(&mut self.reader) {
                Ok(block) => block,
                Err(e) => return Some(Err(e)),
            };

            Block {
                kind: BlockKind::File(block),
            }
        };

        self.next_block_position = block.position() + block.full_size();

        Some(Ok(block))
    }
}

#[derive(Debug)]
pub struct Block {
    pub kind: BlockKind,
}

impl HeaderSize for Block {
    fn header_size(&self) -> u64 {
        match &self.kind {
            BlockKind::Main(block) => block.header_size(),
            BlockKind::File(block) => block.header_size(),
        }
    }
}

impl DataSize for Block {
    fn data_size(&self) -> u64 {
        match &self.kind {
            BlockKind::Main(block) => block.data_size(),
            BlockKind::File(block) => block.data_size(),
        }
    }
}

impl RarBlock for Block {
    fn position(&self) -> u64 {
        match &self.kind {
            BlockKind::Main(block) => block.position,
            BlockKind::File(block) => block.position,
        }
    }
}

#[derive(Debug)]
pub enum BlockKind {
    Main(MainBlock),
    File(FileBlock),
}

#[derive(Debug)]
/// The main block is located right after the RAR 1.4 file signature
/// and contains metadata for the whole archive.
pub struct MainBlock {
    /// Position in the file of this block.
    pub position: u64,

    /// Full size of the header from `position`.
    pub header_size: u16,

    /// Main block header flags.
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
        is_comment_packed = 0x10;
    }
}

impl MainBlock {
    const SIGNATURE_SIZE: u16 = 4;
    const HEADER_FIELDS_SIZE: u64 = 3;

    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let position = reader.stream_position()?;

        let header_size = read_u16(reader)? - Self::SIGNATURE_SIZE;
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

        reader.seek(io::SeekFrom::Start(
            self.position + Self::HEADER_FIELDS_SIZE,
        ))?;

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

impl Deref for MainBlock {
    type Target = MainBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

impl HeaderSize for MainBlock {
    fn header_size(&self) -> u64 {
        self.header_size as u64
    }
}

impl DataSize for MainBlock {}

#[derive(Debug)]
pub struct FileBlock {
    /// Position in the file of this block.
    pub position: u64,

    /// Full size of the header from `position`.
    pub header_size: u16,

    /// File block header flags.
    pub flags: FileBlockFlags,

    /// Size of the data area of the block.
    pub packed_data_size: u32,

    /// Size of the file after unpacking.
    pub unpacked_data_size: u32,

    /// CRC16 hash of the unpacked file.
    pub crc16: u16,

    /// Modification time of the file.
    pub modification_time: Result<time::PrimitiveDateTime, u32>,

    /// DOS attributes of the file.
    pub attributes: u8,

    // TODO enumerate the versions
    pub unpack_version: u8,

    // TODO enumerate the methods
    pub method: u8,

    /// Filename of the file.
    pub name: OsString,
}

flags! {
    pub struct FileBlockFlags(u8) {
        /// File is continuing from previous volume.
        pub split_before = 0x01;

        /// File is continuing in next volume.
        pub split_after = 0x02;

        /// File is encrypted with a password.
        pub is_encrypted = 0x04;
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
        let modification_time =
            time_conv::parse_dos(modification_time).map_err(|_| modification_time);

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

impl Deref for FileBlock {
    type Target = FileBlockFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
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
