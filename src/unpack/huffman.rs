use crate::unpack::bit_input::BitInput;

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
    const DCB: usize = 64;
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
    const SIZEX: usize = DecodeTable::NC + DecodeTable::DCX + DecodeTable::LDC + DecodeTable::RC;
    const SIZEB: usize = DecodeTable::NC + DecodeTable::DCB + DecodeTable::LDC + DecodeTable::RC;

    pub fn read(inp: &mut BitInput, extra_dist: bool) -> Option<Self> {
        let bit_lengths = DecodeTable::read_bit_lengths(inp);

        let mut table = [0u8; Self::SIZEX];
        let mut table = if extra_dist {
            &mut table[..]
        } else {
            &mut table[..Self::SIZEB]
        };

        let mut i = 0;
        while i < table.len() {
            let number: u32 = bit_lengths.decode_number(inp);

            if number < 16 {
                table[i] = number as u8;
                i += 1;
            } else {
                if number < 18 {
                    let mut n: u32;
                    if number == 16 {
                        n = (inp.get_bits16() >> 13) + 3;
                        inp.add_bits(3);
                    } else {
                        n = (inp.get_bits16() >> 9) + 11;
                        inp.add_bits(7);
                    }

                    if i == 0 {
                        // We cannot have "repeat previous" code at the first position.
                        // Multiple such code would shift Inp position without changing I,
                        // which can lead to reading beyond of Inp boundary in multithreading
                        // mode, where Inp.ExternalBuffer disables bounds check and we just
                        // reserve a lot of buffer space to not need such check normally.
                        return None;
                    } else {
                        while {
                            let old_n = n;
                            n = n.wrapping_sub(1);
                            old_n > 0 && i < table.len()
                        } {
                            table[i] = table[i - 1];
                            i += 1;
                        }
                    }
                } else {
                    let mut n: u32;
                    if number == 18 {
                        n = (inp.get_bits16() >> 13) + 3;
                        inp.add_bits(3);
                    } else {
                        n = (inp.get_bits16() >> 9) + 11;
                        inp.add_bits(7);
                    }

                    while {
                        let old_n = n;
                        n = n.wrapping_sub(1);
                        old_n > 0 && i < table.len()
                    } {
                        table[i] = 0;
                        i += 1;
                    }
                }
            }
        }

        let literals = DecodeTable::new(&table[..DecodeTable::NC]);
        table = &mut table[DecodeTable::NC..];

        let distance_len = if extra_dist {
            DecodeTable::DCX
        } else {
            DecodeTable::DCB
        };

        let distances = DecodeTable::new(&table[..distance_len]);
        table = &mut table[distance_len..];

        let distance_lower_bits = DecodeTable::new(&table[..DecodeTable::LDC]);
        table = &mut table[DecodeTable::LDC..];

        let repeating_distances = DecodeTable::new(table);

        Some(Self {
            ld: literals,
            dd: distances,
            ldd: distance_lower_bits,
            rd: repeating_distances,
            bd: bit_lengths,
        })
    }
}
