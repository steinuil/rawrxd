use std::io;

use crate::read::*;

#[derive(Debug, Clone, Copy)]
pub struct MainHeaderFlags(u16);

impl MainHeaderFlags {
    const VOLUME: u16 = 0x0001;
    const COMMENT: u16 = 0x0002;
    const LOCK: u16 = 0x0004;
    const SOLID: u16 = 0x0008;
    const NEWNUMBERING: u16 = 0x0010;
    const AV: u16 = 0x0020;
    const PROTECT: u16 = 0x0040;
    const PASSWORD: u16 = 0x0080;
    const FIRSTVOLUME: u16 = 0x0100;

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
}

pub fn read_main<R: io::Read + io::Seek>(reader: &mut R, base: BaseHeader) -> io::Result<()> {
    // These are not used for anything in unrar.
    let high_pos_av = read_u16(reader)?;
    let pos_av = read_u16(reader)?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct EndArchiveHeaderFlags(u16);

impl EndArchiveHeaderFlags {
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

pub fn read_end_archive<R: io::Read + io::Seek>(reader: &mut R, flags: u16) -> io::Result<()> {
    let flags = EndArchiveHeaderFlags::new(flags);

    let data_crc32 = if flags.has_crc32() {
        let data_crc32 = read_u32(reader)?;
        Some(data_crc32)
    } else {
        None
    };

    let volume_number = if flags.has_volume_number() {
        let volume_number = read_u16(reader)?;
        Some(volume_number)
    } else {
        None
    };

    Ok(())
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
