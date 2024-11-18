use std::{io, ops::Deref};

use crate::{read::*, size::BlockSize, time_conv};

#[derive(Debug)]
pub enum Block {
    Main(MainBlock),
    File(FileBlock),
}

impl BlockSize for Block {
    fn offset(&self) -> u64 {
        match self {
            Block::Main(b) => b.offset(),
            Block::File(b) => b.offset(),
        }
    }

    fn header_size(&self) -> u64 {
        match self {
            Block::Main(b) => b.header_size(),
            Block::File(b) => b.header_size(),
        }
    }

    fn data_size(&self) -> u64 {
        match self {
            Block::Main(b) => b.data_size(),
            Block::File(b) => b.data_size(),
        }
    }
}

#[derive(Debug)]
/// The main block is located right after the RAR 1.4 file signature
/// and contains metadata for the whole archive.
pub struct MainBlock {
    /// Offset of this block in the file.
    pub offset: u64,

    /// Full size of the header from `offset`.
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

        pub has_supplementary_field = 0x20;
    }
}

impl MainBlock {
    const SIGNATURE_SIZE: u16 = 4;
    const HEADER_FIELDS_SIZE: u64 = 3;

    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
        let offset = reader.stream_position()?;

        let header_size = read_u16(reader)? - Self::SIGNATURE_SIZE;
        let flags = read_u8(reader)?;
        let flags = MainBlockFlags::new(flags);

        Ok(MainBlock {
            offset,
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

        reader.seek(io::SeekFrom::Start(self.offset + Self::HEADER_FIELDS_SIZE))?;

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

impl BlockSize for MainBlock {
    fn offset(&self) -> u64 {
        self.offset
    }

    fn header_size(&self) -> u64 {
        self.header_size as u64
    }

    fn data_size(&self) -> u64 {
        0
    }
}

#[derive(Debug)]
pub struct FileBlock {
    /// Offset of this block in the file.
    pub offset: u64,

    /// Full size of the header from `offset`.
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

    pub comment: Option<Vec<u8>>,

    /// Filename of the file.
    pub name: Filename,
}

flags! {
    pub struct FileBlockFlags(u8) {
        /// File is continuing from previous volume.
        pub split_before = 0x01;

        /// File is continuing in next volume.
        pub split_after = 0x02;

        /// File is encrypted with a password.
        pub is_encrypted = 0x04;

        /// File header contains comment
        pub has_comment = 0x08;
    }
}

#[derive(Debug)]
pub enum Filename {
    /// Filename only contains characters in the ASCII range and can be safely
    /// decoded into UTF-8.
    Ascii(String),

    /// Filename was encoded using the current OEM code page and cannot be decoded
    /// correctly on its own. The user must select an encoding and use
    /// [`encoding_rs`](https://crates.io/crates/encoding_rs) or
    /// [`oem_cp`](https://crates.io/crates/oem_cp) to decode it correctly.
    Oem(Vec<u8>),
}

impl FileBlock {
    pub fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<FileBlock> {
        let offset = reader.stream_position()?;

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

        // UnRAR doesn't even read this, but the documentation for RAR 1.4
        // says it might be present.
        let comment = if flags.has_comment() {
            let comment_size = read_u16(reader)?;
            Some(read_vec(reader, comment_size as usize)?)
        } else {
            None
        };

        let name = read_vec(reader, name_size)?;

        let name = if name.is_ascii() {
            let Ok(name) = String::from_utf8(name) else {
                unreachable!("we already checked that all characters are in the ASCII range");
            };

            Filename::Ascii(name)
        } else {
            Filename::Oem(name)
        };

        Ok(FileBlock {
            offset,
            header_size,
            flags,
            packed_data_size,
            unpacked_data_size,
            crc16,
            modification_time,
            attributes,
            unpack_version,
            method,
            comment,
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

impl BlockSize for FileBlock {
    fn offset(&self) -> u64 {
        self.offset
    }

    fn header_size(&self) -> u64 {
        self.header_size as u64
    }

    fn data_size(&self) -> u64 {
        self.packed_data_size as u64
    }
}
