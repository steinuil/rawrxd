use std::{num::NonZero, ops::Deref};

use crate::{
    unpack::{bit_input::BitInput, huffman::BlockTables},
    Error, RarResult,
};

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

    pub fn push(&mut self, byte: u8) {
        self.buf[self.unp_ptr] = byte;
        self.unp_ptr += 1;
    }

    pub fn copy_string(&mut self, length: u32, distance: usize) {
        let length = length as usize;

        let mut src_ptr = match self.unp_ptr.checked_sub(distance) {
            Some(src_ptr) => src_ptr,
            None if distance <= self.max_win_size && self.first_win_done => {
                self.max_win_size - (distance - self.unp_ptr)
            }
            None => {
                for _ in 0..length {
                    self.buf[self.unp_ptr] = 0;
                    self.unp_ptr = self.wrap_up(self.unp_ptr + 1);
                }
                return;
            }
        };

        let limit = self.max_win_size - Self::MAX_INC_LZ_MATCH;
        if src_ptr < limit && self.unp_ptr < limit {
            let dst = self.unp_ptr;

            if distance >= length {
                self.buf.copy_within(src_ptr..src_ptr + length, dst);
            } else {
                for i in 0..length {
                    self.buf[dst + i] = self.buf[src_ptr + i];
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
    }

    pub fn wrap_up(&self, win_pos: usize) -> usize {
        win_pos.checked_sub(self.max_win_size).unwrap_or(win_pos)
    }

    pub fn ring_distance(&self, from: usize, to: usize) -> usize {
        to.checked_sub(from)
            .unwrap_or_else(|| to + self.max_win_size - from)
    }
}

#[derive(Debug, Clone)]
pub struct RepeatState {
    old_distances: [usize; 4],
    last_length: u32,
}

impl RepeatState {
    fn insert_old_dist(&mut self, distance: usize) {
        self.old_distances.rotate_right(1);
        self.old_distances[0] = distance;
    }

    fn last_distance(&self) -> usize {
        self.old_distances[0]
    }
}

#[derive(Debug, Clone)]
pub struct Unpack5 {
    inp: BitInput,
    tables: BlockTables,
    window: Window,
    reps: RepeatState,
    filters: Vec<UnpackFilter>,
    wr_ptr: usize,
}

impl Unpack5 {
    const MAX_UNPACK_FILTERS: usize = 0x2000;

    fn decode_symbol(&mut self) -> bool {
        let main_slot = self.tables.ld.decode_number(&mut self.inp);

        match main_slot {
            0..=255 => {
                self.window.push(main_slot as u8);
                true
            }

            256 => {
                let filter = UnpackFilter::read(&mut self.inp);
                self.add_filter(filter)
            }

            257 => {
                if self.reps.last_length != 0 {
                    self.window
                        .copy_string(self.reps.last_length, self.reps.last_distance());
                }

                true
            }

            258..=261 => {
                let dist_num = (main_slot - 258) as usize;

                self.reps.old_distances[..=dist_num].rotate_right(1);
                let distance = self.reps.last_distance();

                let length_slot = self.tables.rd.decode_number(&mut self.inp);
                let length = slot_to_length(&mut self.inp, length_slot);
                self.reps.last_length = length;
                self.window.copy_string(length, distance);
                true
            }

            262.. => {
                let mut length = slot_to_length(&mut self.inp, main_slot - 262);

                let mut distance: u64 = 1;
                let dist_slot = self.tables.dd.decode_number(&mut self.inp);
                let d_bits = if dist_slot < 4 {
                    distance += dist_slot as u64;
                    0
                } else {
                    let d_bits = dist_slot / 2 - 1;
                    distance += ((2 | (dist_slot & 1)) as u64) << d_bits;
                    d_bits
                };

                if d_bits > 0 {
                    if d_bits >= 4 {
                        if d_bits > 4 {
                            distance += self.inp.read_u64_be(d_bits - 4) << 4;
                        }
                        let low_dist = self.tables.ldd.decode_number(&mut self.inp);
                        distance += low_dist as u64;
                    } else {
                        distance += self.inp.read_u16_be(d_bits) as u64;
                    }
                }

                let distance = usize::try_from(distance).unwrap_or(usize::MAX);

                if distance > 0x100 {
                    length += 1;

                    if distance > 0x2000 {
                        length += 1;

                        if distance > 0x40000 {
                            length += 1;
                        }
                    }
                }

                self.reps.insert_old_dist(distance);
                self.reps.last_length = length;
                self.window.copy_string(length, distance);
                true
            }
        }
    }

    fn add_filter(&mut self, mut filter: UnpackFilter) -> bool {
        if self.filters.len() >= Self::MAX_UNPACK_FILTERS {
            todo!()
        }

        let w = &self.window;
        filter.next_window = self.wr_ptr != w.unp_ptr
            && w.ring_distance(w.unp_ptr, self.wr_ptr) <= filter.block_start;

        filter.block_start = filter.block_start.wrapping_add(w.unp_ptr) % w.max_win_size;
        self.filters.push(filter);
        true
    }
}

#[derive(Debug, Clone)]
pub struct Input {
    pub bits: BitInput,
    read_top: isize,
    read_border: isize,
    block_start: isize,
    block_size: isize,
    block_bit_size: u32,
}

impl Input {
    pub fn new() -> Self {
        Self {
            bits: BitInput::new(),
            read_top: 0,
            read_border: 0,
            block_start: 0,
            block_size: -1,
            block_bit_size: 0,
        }
    }

    pub fn reset(&mut self) {
        self.bits.reset();
        self.read_top = 0;
        self.read_border = 0;
        self.block_start = 0;
        self.block_size = -1;
        self.block_bit_size = 0;
    }

    pub fn refill(&mut self, input: &mut &[u8], finished: bool) -> RarResult<bool> {
        let addr = self.bits.byte_position() as isize;

        // Data left to process.
        let mut data_size = self.read_top - addr;
        if data_size < 0 {
            return Err(Error::UnexpectedEof);
        }
        self.block_size -= addr - self.block_start;

        if addr > (BitInput::MAX_SIZE / 2) as isize {
            // If we already processed more than half of buffer, let's move
            // remaining data into beginning to free more space for new data
            // and ensure that calling function does not cross the buffer border
            // even if we did not read anything here. Also it esure that read size
            // is not less than CRYPT_BLOCK_SIZE, so we can align it without risk
            // to make it zero.
            self.bits.discard_consumed(self.read_top as usize);
            self.read_top = data_size;
        } else {
            data_size = self.read_top;
        }

        self.read_top += self.bits.fill(data_size as usize, input) as isize;

        self.read_border = self.read_top - 30;
        self.block_start = self.bits.byte_position() as isize;

        // -1 means not defined yet.
        if self.block_size != -1 {
            // We may need to quit from main extraction loop and read new block header
            // and trees earlier than data in input buffer ends.
            self.read_border = self.read_border.min(self.end())
        }

        Ok(self.read_top as usize == BitInput::MAX_SIZE || finished)
    }

    #[inline]
    fn end(&self) -> isize {
        self.block_start + self.block_size - 1
    }

    pub fn start_block(&mut self, header: &UnpackBlockHeader) {
        self.block_start = header.offset as isize;
        self.block_size = header.block_size as isize;
        self.block_bit_size = header.block_bit_size() as u32;

        self.read_border = self.read_border.min(self.end());
    }

    pub fn at_border(&self) -> bool {
        self.bits.byte_position() as isize >= self.read_border
    }

    pub fn past_block_end(&self) -> bool {
        let addr = self.bits.byte_position() as isize;
        addr > self.end() || addr == self.end() && self.bits.bit_position() >= self.block_bit_size
    }

    pub fn past_read_top(&self) -> bool {
        self.bits.byte_position() as isize > self.read_top
    }
}
