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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Delta { channels: NonZero<u8> },
    X86Call,
    X86CallJmp,
    ArmBl,
    Audio,
    Rgb,
    Itanium,
    Text,
}

#[derive(Debug, Clone)]
pub struct UnpackFilter {
    kind: FilterKind,
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

    const CALL_REL32: u8 = 0xe8;
    const JMP_REL32: u8 = 0xe9;

    pub fn read(inp: &mut BitInput) -> Self {
        let block_start = read_filter_data(inp) as usize;
        let block_length = read_filter_data(inp);
        let block_length = if block_length > Self::MAX_BLOCK_SIZE {
            0
        } else {
            block_length
        };

        let kind = match inp.read_u8(3) {
            0 => {
                let channels = NonZero::<u8>::MIN.saturating_add(inp.read_u8(5));
                FilterKind::Delta { channels }
            }
            1 => FilterKind::X86Call,
            2 => FilterKind::X86CallJmp,
            3 => FilterKind::ArmBl,
            4 => FilterKind::Audio,
            5 => FilterKind::Rgb,
            6 => FilterKind::Itanium,
            7 => FilterKind::Text,
            _ => unreachable!("three bits"),
        };

        Self {
            kind,
            next_window: false,
            block_start,
            block_length,
        }
    }

    pub fn apply<'a>(
        &self,
        data: &'a mut [u8],
        written_file_size: u64,
        dst: &'a mut Vec<u8>,
    ) -> Option<&'a [u8]> {
        match self.kind {
            FilterKind::X86Call | FilterKind::X86CallJmp => {
                let file_offset = written_file_size as u32;

                const FILE_SIZE: i32 = 0x1000000;
                let cmp_byte2: u8 = if self.kind == FilterKind::X86CallJmp {
                    Self::JMP_REL32
                } else {
                    Self::CALL_REL32
                };

                let mut i = 0;
                while i + 4 < data.len() {
                    let cur_byte = data[i];
                    i += 1;
                    if cur_byte != Self::CALL_REL32 && cur_byte != cmp_byte2 {
                        continue;
                    }

                    let offset = (file_offset.wrapping_add(i as u32) % FILE_SIZE as u32) as i32;
                    let field = &mut data[i..i + 4];
                    let addr = i32::from_le_bytes(field.try_into().unwrap());

                    if addr < 0 {
                        if addr + offset >= 0 {
                            field.copy_from_slice(&(addr + FILE_SIZE).to_le_bytes());
                        }
                    } else if addr < FILE_SIZE {
                        field.copy_from_slice(&(addr - offset).to_le_bytes());
                    }

                    i += 4;
                }

                Some(data)
            }

            FilterKind::ArmBl => {
                // 2019-11-15: we turned off ARM filter by default when compressing,
                // mostly because it is inefficient for modern 64 bit ARM binaries.
                // It was turned on by default in 5.0 - 5.80b3 , so we still need it
                // here for compatibility with some of previously created archives.
                let file_offset = written_file_size as u32;

                for (i, instr) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    // BL command with '1110' (Always) condition.
                    if instr[3] == 0xeb {
                        let pos = file_offset.wrapping_add(i as u32 * 4);
                        let offset = u32::from_le_bytes([instr[0], instr[1], instr[2], 0])
                            .wrapping_sub(pos / 4);
                        instr[..3].copy_from_slice(&offset.to_le_bytes()[..3]);
                    }
                }

                Some(data)
            }

            FilterKind::Delta { channels } => {
                let channels = channels.get() as usize;

                dst.clear();
                dst.resize(data.len(), 0);

                let mut src = data.iter();
                for ch in 0..channels {
                    let mut prev = 0u8;

                    for (d, source) in dst.iter_mut().skip(ch).step_by(channels).zip(&mut src) {
                        prev = prev.wrapping_sub(*source);
                        *d = prev;
                    }
                }

                Some(dst)
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

#[derive(Debug, Clone)]
pub struct Window {
    buf: Vec<u8>,
    max_win_size: usize,
    unp_ptr: usize,
    prev_ptr: usize,
    first_win_done: bool,
}

impl Window {
    const MAX_LZ_MATCH: usize = 0x1001;
    const MAX_INC_LZ_MATCH: usize = Self::MAX_LZ_MATCH + 3;

    pub fn copy_string(&mut self, length: u32, distance: usize) {
        let length = length as usize;

        let mut src_ptr = self.unp_ptr.strict_sub(distance);

        // Perform the correction here instead of "else", so matches crossing
        // the window beginning can also be processed by first "if" part.

        // Unlike SrcPtr >= MaxWinSize, it catches invalid distances like 0xfffffff0 in 32-bit build.
        if distance > self.unp_ptr {
            // Same as WrapDown(SrcPtr), needed because of UnpPtr-Distance above.
            // We need the same condition below, so we expanded WrapDown here.
            src_ptr += self.max_win_size;

            if distance > self.max_win_size || !self.first_win_done {
                for _ in 0..length {
                    self.buf[self.unp_ptr] = 0;
                    self.unp_ptr = self.wrap_up(self.unp_ptr + 1);
                }

                return;
            }
        }

        let limit = self.max_win_size - Self::MAX_INC_LZ_MATCH;
        if src_ptr < limit && self.unp_ptr < limit {
            let dst = self.unp_ptr;

            if distance >= length {
                self.buf.copy_within(src_ptr..src_ptr + length, dst);
            } else {
                for i in 0..length {
                    self.buf[dst + 1] = self.buf[src_ptr + i];
                }
            }

            self.unp_ptr += length;
        } else {
            for _ in 0..length {
                self.buf[self.unp_ptr] = self.buf[self.wrap_up(src_ptr)];
                src_ptr += 1;
                self.unp_ptr = self.wrap_up(self.unp_ptr + 1);
            }
        }

        todo!()
    }

    pub fn wrap_up(&self, win_pos: usize) -> usize {
        if win_pos >= self.max_win_size {
            win_pos.wrapping_sub(self.max_win_size)
        } else {
            win_pos
        }
    }

    pub fn wrap_down(&self, win_pos: usize) -> usize {
        if win_pos >= self.max_win_size {
            win_pos.wrapping_add(self.max_win_size)
        } else {
            win_pos
        }
    }
}
