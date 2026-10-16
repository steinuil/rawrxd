use crate::unpack::bit_input::BitInput;

/// Size of the distance alphabet, which bounds the largest encodable distance.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceCodes {
    Base = 64,
    Extended = 80,
}

impl DistanceCodes {
    pub const fn alphabet_size(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone)]
pub struct DecodeTable {
    /// Size of the alphabet and `decode_pos` array.
    max_num: u32,
    decode_len: [u32; 16],
    decode_pos: [u32; 16],
    quick_bits: u32,
    quick_len: [u8; 1 << Self::MAX_QUICK_DECODE_BITS],
    quick_num: [u16; 1 << Self::MAX_QUICK_DECODE_BITS],
    decode_num: [u16; Self::LARGEST_TABLE_SIZE],
}

impl Default for DecodeTable {
    fn default() -> Self {
        Self {
            max_num: Default::default(),
            decode_len: Default::default(),
            decode_pos: Default::default(),
            quick_bits: Default::default(),
            quick_len: [0; 1 << Self::MAX_QUICK_DECODE_BITS],
            quick_num: [0; 1 << Self::MAX_QUICK_DECODE_BITS],
            decode_num: [0; Self::LARGEST_TABLE_SIZE],
        }
    }
}

impl DecodeTable {
    const BC: usize = 20;

    const NC: usize = 306;
    const DCX: usize = 80;
    const LDC: usize = 16;
    const RC: usize = 44;

    const BC_ESCAPE: u8 = 0xf;

    const NC20: usize = 298;
    const NC30: usize = 299;

    const MAX_QUICK_DECODE_BITS: usize = 9;
    const LARGEST_TABLE_SIZE: usize = 306;

    fn read_bit_lengths(inp: &mut BitInput) -> Self {
        let mut bit_length = [0u8; Self::BC];

        let mut slots = bit_length.iter_mut();
        while let Some(slot) = slots.next() {
            let length = inp.read_u8(4);

            if length != Self::BC_ESCAPE {
                *slot = length;
                continue;
            }

            // 0xf is interpreted as an escape code, whose meaning depends
            // on the next 4 bits.
            match inp.read_u8(4) {
                // If they are 0, the length is actually 15.
                0 => *slot = 15,

                // Otherwise they encode a number of consecutive zero lengths.
                zero_count => {
                    for _ in 0..zero_count + 1 {
                        slots.next();
                    }
                }
            }
        }

        Self::new(&bit_length)
    }

    pub fn new(lengths: &[u8]) -> Self {
        // Build a map of code length -> how many codes have that length.
        let mut length_counts = [0u32; 16];
        for &length in lengths.iter() {
            length_counts[(length & 0xf) as usize] += 1;
        }
        // Exclude zero length codes.
        length_counts[0] = 0;

        let mut decode_len = [0u32; 16];
        let mut decode_pos = [0u32; 16];

        let mut upper_limit = 0u32;
        for length in 1..16 {
            upper_limit += length_counts[length];
            decode_len[length] = upper_limit << (16 - length);
            upper_limit <<= 1;
            decode_pos[length] = decode_pos[length - 1] + length_counts[length - 1];
        }

        let mut next_pos = decode_pos;

        // Build the code list, sorted by bit length and then by code.
        let mut decode_num = [0u16; Self::LARGEST_TABLE_SIZE];
        for (code, &length) in lengths.iter().enumerate() {
            let length = (length & 0xf) as usize;

            if length != 0 {
                decode_num[next_pos[length] as usize] = code as u16;
                next_pos[length] += 1;
            }
        }

        let quick_bits = match lengths.len() {
            Self::NC | Self::NC20 | Self::NC30 => Self::MAX_QUICK_DECODE_BITS,
            _ => Self::MAX_QUICK_DECODE_BITS.saturating_sub(3),
        } as u32;

        let mut quick_len = [0u8; 1 << Self::MAX_QUICK_DECODE_BITS];
        let mut quick_num = [0u16; 1 << Self::MAX_QUICK_DECODE_BITS];

        let mut cur_bit_length = 1;
        for code in 0..1usize << quick_bits {
            let bit_field = (code as u32) << (16 - quick_bits);

            while cur_bit_length < 16 && bit_field >= decode_len[cur_bit_length] {
                cur_bit_length += 1;
            }
            quick_len[code] = cur_bit_length as u8;

            let distance = (bit_field - decode_len[cur_bit_length - 1]) >> (16 - cur_bit_length);
            quick_num[code] = decode_pos
                .get(cur_bit_length)
                .map(|&first| first + distance)
                .filter(|&pos| pos < lengths.len() as u32)
                .map_or(0, |pos| decode_num[pos as usize]);
        }

        Self {
            max_num: lengths.len() as _,
            decode_len,
            decode_pos,
            quick_bits,
            quick_len,
            quick_num,
            decode_num,
        }
    }

    pub fn decode_number(&self, inp: &mut BitInput) -> u32 {
        // Left aligned 15 bit length raw bit field.
        let bit_field: u32 = inp.get_bits16() & 0xfffe;

        if bit_field < self.decode_len[self.quick_bits as usize] {
            let code = (bit_field >> (16 - self.quick_bits)) as usize;
            inp.add_bits(self.quick_len[code] as u32);
            return self.quick_num[code] as u32;
        }

        // Detect the real bit length for current code.
        let bits = (self.quick_bits + 1..15)
            .find(|&i| bit_field < self.decode_len[i as usize])
            .unwrap_or(15);
        inp.add_bits(bits);

        // Calculate the distance from the start code for current bit length.
        let mut dist: u32 = bit_field - self.decode_len[bits as usize - 1];

        // Start codes are left aligned, but we need the normal right aligned
        // number. So we shift the distance to the right.
        dist >>= 16 - bits;

        // Now we can calculate the position in the code list. It is the sum
        // of first position for current bit length and right aligned distance
        // between our bit field and start code for current bit length.
        let mut pos: u32 = self.decode_pos[bits as usize] + dist;

        // Out of bounds safety check required for damaged archives.
        if pos >= self.max_num {
            pos = 0;
        }

        self.decode_num[pos as usize] as u32
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockTables {
    pub ld: DecodeTable,
    pub dd: DecodeTable,
    pub ldd: DecodeTable,
    pub rd: DecodeTable,
    pub bd: DecodeTable,
}

impl BlockTables {
    const MAX_TABLE_SIZE: usize =
        DecodeTable::NC + DecodeTable::DCX + DecodeTable::LDC + DecodeTable::RC;

    pub fn read(inp: &mut BitInput, distance_codes: DistanceCodes) -> Option<Self> {
        let bit_lengths = DecodeTable::read_bit_lengths(inp);

        let mut buf = [0u8; Self::MAX_TABLE_SIZE];

        let lengths = &mut buf[..DecodeTable::NC
            + distance_codes.alphabet_size()
            + DecodeTable::LDC
            + DecodeTable::RC];

        let mut i = 0;
        while i < lengths.len() {
            let code = bit_lengths.decode_number(inp);

            let (count, length) = match code {
                // Literal length.
                0..=15 => (1, code as u8),

                // Repeat the previous length for `count` times.
                // Returns `None` when `i` is 0, because we obviously can't repeat
                // the -1st length.
                16 => {
                    let count = 3 + inp.read_u8(3) as usize;
                    let length = lengths[i.checked_sub(1)?];

                    (count, length)
                }
                17 => {
                    let count = 11 + inp.read_u8(7) as usize;
                    let length = lengths[i.checked_sub(1)?];

                    (count, length)
                }

                // Fill with `count` zeroes.
                18 => {
                    let count = 3 + inp.read_u8(3) as usize;
                    (count, 0)
                }
                19 => {
                    let count = 11 + inp.read_u8(7) as usize;
                    (count, 0)
                }

                // We constructed the bit_lengths table so it's our fault
                // if `decode_number` returns a number outside that range.
                _ => unreachable!("bit_lengths can only contain 20 codes"),
            };

            let end = (i + count).min(lengths.len());
            lengths[i..end].fill(length);

            i = end;
        }

        let (literals, rest) = lengths.split_at(DecodeTable::NC);
        let (distances, rest) = rest.split_at(distance_codes.alphabet_size());
        let (distance_lower_bits, repeating_distances) = rest.split_at(DecodeTable::LDC);

        Some(Self {
            ld: DecodeTable::new(literals),
            dd: DecodeTable::new(distances),
            ldd: DecodeTable::new(distance_lower_bits),
            rd: DecodeTable::new(repeating_distances),
            bd: bit_lengths,
        })
    }
}
