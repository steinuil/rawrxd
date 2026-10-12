use std::{num::NonZero, ops::Deref};

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

#[derive(Debug, Clone)]
pub struct UnpackFilter {
    kind: FilterKind,
    channels: Option<NonZero<u8>>,
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
            FilterKind::Delta => NonZero::new(inp.read_u8(5) + 1),
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

    pub fn apply<'a>(
        &self,
        data: &'a mut [u8],
        written_file_size: u64,
        mut filter_dst_memory: &'a mut Vec<u8>,
    ) -> Option<&'a [u8]> {
        let data_size = data.len() as u32;

        match self.kind {
            FilterKind::E8 | FilterKind::E8E9 => {
                let file_offset = written_file_size as u32;

                const FILE_SIZE: u32 = 0x1000000;
                let cmp_byte2: u8 = if self.kind == FilterKind::E8E9 {
                    0xe9
                } else {
                    0xe8
                };
                // DataSize is unsigned, so we use "CurPos+4" and not "DataSize-4"
                // to avoid overflow for DataSize<4.
                let mut data_pos: usize = 0;
                let mut cur_pos: u32 = 0;
                while cur_pos + 4 < data_size {
                    let cur_byte = data[data_pos];
                    data_pos += 1;
                    cur_pos += 1;

                    if cur_byte == 0xe8 || cur_byte == cmp_byte2 {
                        let offset = cur_pos.wrapping_add(file_offset) % FILE_SIZE;
                        let addr = u32::from_le_bytes([
                            data[data_pos],
                            data[data_pos + 1],
                            data[data_pos + 2],
                            data[data_pos + 3],
                        ]);

                        // We check 0x80000000 bit instead of '< 0' comparison
                        // not assuming int32 presence or uint size and endianness.
                        if (addr & 0x80000000) != 0 {
                            if (addr.wrapping_add(offset) & 0x80000000) == 0 {
                                data[data_pos..data_pos + 4]
                                    .copy_from_slice(&addr.wrapping_add(FILE_SIZE).to_le_bytes());
                            }
                        } else if (addr.wrapping_sub(FILE_SIZE) & 0x80000000) != 0 {
                            data[data_pos..data_pos + 4]
                                .copy_from_slice(&addr.wrapping_sub(offset).to_le_bytes())
                        }

                        data_pos += 4;
                        cur_pos += 4;
                    }
                }

                Some(data)
            }
            FilterKind::Arm => {
                // 2019-11-15: we turned off ARM filter by default when compressing,
                // mostly because it is inefficient for modern 64 bit ARM binaries.
                // It was turned on by default in 5.0 - 5.80b3 , so we still need it
                // here for compatibility with some of previously created archives.
                let file_offset = written_file_size as u32;

                // DataSize is unsigned, so we use "CurPos+3" and not "DataSize-3"
                // to avoid overflow for DataSize<3.
                let mut cur_pos: u32 = 0;
                while cur_pos + 3 < data_size {
                    let d = &mut data[cur_pos as usize..];
                    if d[3] == 0xeb {
                        // BL command with '1110' (Always) condition.
                        let mut offset =
                            d[0] as u32 + (d[1] as u32) * 0x100 + (d[2] as u32) * 0x10000;
                        offset = offset.wrapping_sub(file_offset.wrapping_add(cur_pos) / 4);
                        d[0] = offset as u8;
                        d[1] = (offset >> 8) as u8;
                        d[2] = (offset >> 16) as u8;
                    }

                    cur_pos += 4;
                }

                Some(data)
            }
            FilterKind::Delta => {
                // Unlike RAR3, we do not need to reject excessive channel
                // values here, since RAR5 uses only 5 bits to store channel.
                let channels = self.channels.map_or_default(|n| n.get()) as u32;
                let mut src_pos: usize = 0;

                filter_dst_memory.resize(data_size as usize, 0);
                let dst_data = &mut filter_dst_memory;

                // Bytes from same channels are grouped to continual data blocks,
                // so we need to place them back to their interleaving positions.
                for cur_channel in 0..channels {
                    let mut prev_byte: u8 = 0;
                    let mut dest_pos: u32 = cur_channel;

                    while dest_pos < data_size {
                        prev_byte = prev_byte.wrapping_sub(data[src_pos]);
                        dst_data[dest_pos as usize] = prev_byte;

                        src_pos += 1;
                        dest_pos += channels;
                    }
                }

                Some(filter_dst_memory)
            }
            _ => None,
        }
    }
}

fn slot_to_length(inp: &mut BitInput, slot: u32) -> u32 {
    let (l_bits, length) = if slot < 8 {
        (0, slot + 2)
    } else {
        let l_bits = slot / 4 - 1;
        let length = 2 + ((4 | (slot & 0b11)) << l_bits);

        (l_bits, length)
    };

    if l_bits > 0 {
        length + inp.read_u16_be(l_bits) as u32
    } else {
        length
    }
}
