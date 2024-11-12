trait BlockHeader {
    /// Offset in the file where this block header is located
    fn position(&self) -> u64;

    /// Size of the header
    fn header_size(&self) -> u64;
}

#[derive(Debug)]
pub enum Block {
    Main(MainBlock),
}

#[derive(Debug)]
pub struct MainBlock {
    pub position: u64,
    pub header_size: u64,
    pub crc: u16,
    pub flags: MainBlockFlags,
    pub high_pos_av: u16,
    pub pos_av: u32,
}

impl BlockHeader for MainBlock {
    fn position(&self) -> u64 {
        self.position
    }

    fn header_size(&self) -> u64 {
        self.header_size
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MainBlockFlags(pub u16);

impl MainBlockFlags {
    const VOLUME: u16 = 0x0001;
    const PACK_COMMENT: u16 = 0x0002;
    const LOCK: u16 = 0x0004;
    const SOLID: u16 = 0x0008;
    const NEWNUMBERING: u16 = 0x0010;
    const AUTHENTICITY: u16 = 0x0020;
    const PROTECT: u16 = 0x0040;
    const PASSWORD: u16 = 0x0080;
    const FIRSTVOLUME: u16 = 0x0100;

    pub fn new(flags: u16) -> Self {
        MainBlockFlags(flags)
    }

    /// Old style (up to RAR 2.9) main archive comment embedded into
    /// the main archive header.
    pub fn has_old_style_comment(&self) -> bool {
        self.0 & Self::PACK_COMMENT != 0
    }

    /// TODO does this mean that authenticity is set in the main header?
    /// or just that it exists in the archive?
    /// Only present up to RAR 3.0
    pub fn has_authenticity_information(&self) -> bool {
        self.0 & Self::AUTHENTICITY != 0
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
    /// and when it is set WinRAR will refuse to modify it.
    pub fn is_locked(&self) -> bool {
        self.0 & Self::LOCK != 0
    }

    pub fn is_password_protected(&self) -> bool {
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
