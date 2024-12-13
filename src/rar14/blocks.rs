use std::{io, ops::Deref};

use crate::{read::*, size::BlockSize, time_conv};

#[derive(Debug)]
/// A generic RAR14 block.
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
/// Block containing archive metadata.
pub struct MainBlock {
    /// Offset of this block from the start of the file.
    pub offset: u64,

    /// Size of the header.
    pub header_size: u16,

    /// Flags containing archive metadata.
    pub flags: MainBlockFlags,
}

flags! {
    /// Flags containing archive metadata.
    pub struct MainBlockFlags(u8) {
        /// Archive spans [multiple volumes][1].
        ///
        /// [1]: https://www.win-rar.com/split-files-archive.html?&L=0
        pub is_volume = 0x01;

        /// Header contains a comment.
        pub has_comment = 0x02;

        /// WinRAR will not modify this archive.
        pub is_locked = 0x04;

        /// Archive uses [solid compression][1].
        ///
        /// [1]: https://en.wikipedia.org/wiki/Solid_compression
        pub is_solid = 0x08;

        /// The comment in the header is packed.
        is_comment_packed = 0x10;

        // TODO document this.
        pub has_supplementary_field = 0x20;
    }
}

impl MainBlock {
    const SIGNATURE_SIZE: u16 = 4;
    const HEADER_FIELDS_SIZE: u64 = 3;

    pub(super) fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<Self> {
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

    /// Read the archive comment, if present.
    // TODO inline into read?
    pub fn read_comment<R: io::Read + io::Seek>(
        &self,
        reader: &mut R,
    ) -> io::Result<Option<OemString>> {
        if !self.flags.has_comment() {
            return Ok(None);
        }

        // The comment is considered part of the main block header, and it comes after all the
        // other fields.
        reader.seek(io::SeekFrom::Start(self.offset + Self::HEADER_FIELDS_SIZE))?;

        let size = read_u16(reader)? as usize;

        // TODO comment encoding?

        if !self.flags.is_comment_packed() {
            if size == 0 {
                return Ok(None);
            }

            let comment = read_vec(reader, size)?;
            return Ok(Some(OemString::parse(comment)));
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
/// Block containing a file.
pub struct FileBlock {
    /// Offset of this block from the start of the file.
    pub offset: u64,

    /// Size of the header.
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
    ///
    /// MS-DOS timestamps are not timezone aware, so this is provided as a PrimitiveDateTime
    /// and converting to an OffsetDateTime is left up to the calling code.
    pub modification_time: Result<time::PrimitiveDateTime, u32>,

    /// DOS attributes of the file.
    pub attributes: DosFileAttributes,

    // TODO enumerate the versions
    pub unpack_version: u8,

    // TODO enumerate the methods
    pub method: u8,

    /// File comment.
    pub comment: Option<OemString>,

    /// Filename of the file.
    // TODO should have a method to convert the filename into its path components.
    pub name: OemString,
}

flags! {
    /// Flags containing metadata for the file.
    pub struct FileBlockFlags(u8) {
        /// File is continuing from previous volume.
        pub split_before = 0x01;

        /// File is continuing in next volume.
        pub split_after = 0x02;

        /// File is encrypted with a password.
        pub is_encrypted = 0x04;

        /// Header contains comment
        pub has_comment = 0x08;
    }
}

impl FileBlock {
    pub(super) fn read<R: io::Read + io::Seek>(reader: &mut R) -> io::Result<FileBlock> {
        let offset = reader.stream_position()?;

        let packed_data_size = read_u32(reader)?;
        let unpacked_data_size = read_u32(reader)?;
        let crc16 = read_u16(reader)?;
        let header_size = read_u16(reader)?;

        let modification_time = read_u32(reader)?;
        let modification_time =
            time_conv::parse_dos_datetime(modification_time).map_err(|_| modification_time);

        let attributes = read_u8(reader)?;
        let attributes = DosFileAttributes::new(attributes);

        let flags = read_u8(reader)?;
        let flags = FileBlockFlags::new(flags);

        let unpack_version = if read_u8(reader)? == 2 { 13 } else { 10 };
        let name_size = read_u8(reader)? as usize;
        let method = read_u8(reader)?;

        // UnRAR doesn't even read this, but the documentation for RAR 1.4
        // says it might be present.
        let comment = if flags.has_comment() {
            let comment_size = read_u16(reader)?;
            let comment = read_vec(reader, comment_size as usize)?;
            Some(OemString::parse(comment))
        } else {
            None
        };

        let name = read_vec(reader, name_size)?;
        let name = OemString::parse(name);

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

    /// Entry is a directory.
    pub fn is_directory(&self) -> bool {
        self.attributes.is_directory()
    }
}

#[derive(Debug)]
/// A string that was encoded using the host system's [OEM code page](https://en.wikipedia.org/wiki/Windows_code_page#OEM).
pub enum OemString {
    /// The string only contains characters in the ASCII range and can be safely decoded into UTF-8.
    Ascii(String),

    /// The string was encoded using the host system's OEM code page and cannot be decoded
    /// correctly on its own. The user must select an encoding and use
    /// [`encoding_rs`](https://crates.io/crates/encoding_rs) or
    /// [`oem_cp`](https://crates.io/crates/oem_cp) to decode it correctly.
    Oem(Vec<u8>),
}

impl OemString {
    pub(crate) fn parse(buf: Vec<u8>) -> Self {
        if buf.is_ascii() {
            let Ok(string) = String::from_utf8(buf) else {
                unreachable!("we already checked that all characters are in the ASCII range");
            };

            OemString::Ascii(string)
        } else {
            OemString::Oem(buf)
        }
    }
}

flags! {
    /// MS-DOS file attributes.
    ///
    /// <https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants>
    pub struct DosFileAttributes(u8) {
        /// File or directory is read-only.
        pub is_read_only = 0x01;

        /// File or directory is hidden.
        pub is_hidden = 0x02;

        /// File or directory that is used by the OS.
        pub is_system_file = 0x04;

        /// File stores the name of the disk.
        pub is_volume_label = 0x08;

        /// Entry is a directory.
        pub is_directory = 0x10;

        /// Entry is an archive file or directory.
        pub is_archive = 0x20;
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
