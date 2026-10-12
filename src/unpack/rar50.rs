use std::ops::Deref;

use crate::unpack::bit_input::BitInput;

#[derive(Debug, Clone)]
pub struct UnpackBlockHeader {
    pub offset: usize,
    pub block_size: u32,
    pub flags: UnpackBlockHeaderFlags,
}

impl UnpackBlockHeader {
    pub fn read(inp: &mut BitInput) -> Option<Self> {
        inp.align_to_byte();

        let flags = UnpackBlockHeaderFlags(inp.read_u8(8));

        if flags.header_size() == 6 {
            return None;
        }

        let expected_checksum = inp.read_u8(8);

        let block_size = inp.read_u32_le(flags.block_size_byte_count() as u32 * 8);

        let checksum = 0x5a ^ flags.0 ^ block_size.to_le_bytes().into_iter().fold(0, |a, b| a ^ b);

        if checksum != expected_checksum {
            return None;
        }

        // We called Inp.faddbits(8) above, thus Header.BlockStar can't be 0 here.
        // So there is no overflow even if Header.BlockSize is 0.
        // TODO
        // let read_border = read_border.min(block_star + block_size - 1);

        Some(Self {
            offset: inp.byte_position(),
            block_size,
            flags,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnpackBlockHeaderFlags(u8);

impl UnpackBlockHeaderFlags {
    const LAST_BLOCK_IN_FILE: u8 = 0x40;
    const TABLE_PRESENT: u8 = 0x80;

    #[inline]
    fn block_size_byte_count(self) -> usize {
        (((self.0 >> 3) & 0b11) + 1) as usize
    }

    #[inline]
    pub fn header_size(self) -> usize {
        self.block_size_byte_count() + 2
    }

    #[inline]
    pub fn block_bit_size(self) -> usize {
        ((self.0 & 0b111) + 1) as usize
    }

    #[inline]
    pub fn is_last_block_in_file(self) -> bool {
        self.0 & Self::LAST_BLOCK_IN_FILE != 0
    }

    #[inline]
    pub fn has_table(self) -> bool {
        self.0 & Self::TABLE_PRESENT != 0
    }
}

impl Deref for UnpackBlockHeader {
    type Target = UnpackBlockHeaderFlags;

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Delta = 0,
    E8 = 1,
    E8E9 = 2,
    Arm = 3,
    Audio = 4,
    Rgb = 5,
    Itanium = 6,
    Text = 7,
}

pub struct UnpackFilter {
    kind: FilterKind,
    channels: Option<u8>,
    next_window: bool,
    block_start: usize,
    block_length: u32,
}

fn read_filter_data(inp: &mut BitInput) -> u32 {
    let byte_count = inp.read_u8(2) + 1;
    inp.read_u32_le(byte_count as u32 * 8)
}

impl UnpackFilter {
    const MAX_BLOCK_SIZE: u32 = 0x400000;

    pub fn read(inp: &mut BitInput) -> Self {
        let block_start = read_filter_data(inp) as usize;
        let block_length = read_filter_data(inp);
        let block_length = if block_length > Self::MAX_BLOCK_SIZE {
            0
        } else {
            block_length
        };

        let kind = match inp.read_u8(3) {
            0 => FilterKind::Delta,
            1 => FilterKind::E8,
            2 => FilterKind::E8E9,
            3 => FilterKind::Arm,
            4 => FilterKind::Audio,
            5 => FilterKind::Rgb,
            6 => FilterKind::Itanium,
            7 => FilterKind::Text,
            _ => unreachable!("three bits"),
        };

        let channels = match kind {
            FilterKind::Delta => Some(inp.read_u8(5) + 1),
            _ => None,
        };

        Self {
            kind,
            channels,
            next_window: false,
            block_start,
            block_length,
        }
    }
}
