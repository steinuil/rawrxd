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
