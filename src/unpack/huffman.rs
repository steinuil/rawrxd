use crate::unpack::bit_input::BitInput;

const MAX_QUICK_DECODE_BITS: usize = 9;
const LARGEST_TABLE_SIZE: usize = 306;

const NC: u32 = 0;
const NC20: u32 = 298;
const NC30: u32 = 299;

const BC: usize = 20;
const DCB: usize = 64;
const DCX: usize = 80;
const LDC: usize = 16;
const RC: usize = 44;

const HUFF_TABLE_SIZEB: usize = NC as usize + DCB + RC + LDC;
const HUFF_TABLE_SIZEX: usize = NC as usize + DCX + RC + LDC;

#[derive(Debug, Clone)]
pub struct UnpackBlockHeader {
    pub block_size: i32,
    pub block_bit_size: i32,
    pub block_start: i32,
    pub header_size: i32,
    pub last_block_in_file: bool,
    pub table_present: bool,
}

#[derive(Debug, Clone)]
pub struct DecodeTable {
    max_num: u32,
    decode_len: [u32; 16],
    decode_pos: [u32; 16],
    quick_bits: u32,
    quick_len: [u8; 1 << MAX_QUICK_DECODE_BITS],
    quick_num: [u16; 1 << MAX_QUICK_DECODE_BITS],
    decode_num: [u16; LARGEST_TABLE_SIZE],
}

impl Default for DecodeTable {
    fn default() -> Self {
        Self {
            max_num: Default::default(),
            decode_len: Default::default(),
            decode_pos: Default::default(),
            quick_bits: Default::default(),
            quick_len: [0; 1 << MAX_QUICK_DECODE_BITS],
            quick_num: [0; 1 << MAX_QUICK_DECODE_BITS],
            decode_num: [0; LARGEST_TABLE_SIZE],
        }
    }
}

#[derive(Default)]
pub struct BlockTables {
    ld: DecodeTable,
    dd: DecodeTable,
    ldd: DecodeTable,
    rd: DecodeTable,
    bd: DecodeTable,
}

impl DecodeTable {
    pub fn read_tables(
        inp: &mut BitInput,
        header: &UnpackBlockHeader,
        tables: &mut BlockTables,
        extra_dist: bool,
    ) {
        let mut bit_length = [0u8; BC];

        for mut i in 0..BC {
            let length: u32 = ((inp.get_bits16() >> 16) as u8) as u32;
            inp.add_bits(4);

            if length == 15 {
                let mut zero_count = ((inp.get_bits16() >> 16) as u8) as u32;
                inp.add_bits(4);
                if zero_count == 0 {
                    bit_length[i] = 15;
                } else {
                    zero_count += 2;
                    while {
                        zero_count -= 1;
                        zero_count
                    } > 0
                        && i < bit_length.len()
                    {
                        bit_length[i] = 0;
                        i += 1;
                    }
                }
            } else {
                bit_length[i] = length as u8;
            }
        }

        DecodeTable::make(&bit_length, &mut tables.bd, BC as u32);

        let mut table = [0u8; HUFF_TABLE_SIZEX];
        let table = if extra_dist {
            &mut table[..]
        } else {
            &mut table[..HUFF_TABLE_SIZEB]
        };
    }

    pub fn make(length_table: &[u8], dec: &mut DecodeTable, size: u32) {
        // Size of alphabet and DecodePos array.
        dec.max_num = size;

        // Calculate how many entries for every bit length in LengthTable we have.
        let mut length_count = [0u32; 16];
        for i in 0..size as usize {
            length_count[length_table[i] as usize & 0xf] += 1;
        }

        // We must not calculate the number of zero length codes.
        length_count[0] = 0;

        // Set the entire DecodeNum to zero.
        dec.decode_num.fill(0);

        // Initialize not really used entry for zero length code.
        dec.decode_pos[0] = 0;

        // Start code for bit length 1 is 0.
        dec.decode_len[0] = 0;

        // Right aligned upper limit code for current bit length.
        let mut upper_limit = 0u32;

        for i in 1..16 {
            // Adjust the upper limit code.
            upper_limit += length_count[i];

            // Left aligned upper limit code.
            let left_aligned = upper_limit << (16 - i);

            // Prepare the upper limit code for the next bit length.
            upper_limit *= 2;

            // Store the left aligned upper limit code.
            dec.decode_len[i] = left_aligned;

            // Every item of this array contains the sum of all preceding items.
            // So it contains the start position in code list for every bit length.
            dec.decode_pos[i] = dec.decode_pos[i - 1] + length_count[i - 1];
        }

        // Prepare the copy of DecodePos. We'll modify this copy below,
        // so we cannot use the original DecodePos.
        let mut copy_decode_pos = dec.decode_pos;

        // For every bit length in the bit length table and so for every item
        // of the alphabet.
        for i in 0..size as usize {
            // Get the current bit length.
            let cur_bit_length = length_table[i] & 0xf;

            if cur_bit_length != 0 {
                // Last position in code list for current bit length.
                let last_pos = copy_decode_pos[cur_bit_length as usize];

                // Prepare the decode table, so this potition in code list will be
                // decoded to the current alphabet item number.
                dec.decode_num[last_pos as usize] = i as u16;

                // We'll use next position number for this bit length next time.
                // So we pass through the entire range of positions available
                // for every bit length.
                copy_decode_pos[cur_bit_length as usize] += 1;
            }
        }

        // Define the number of bits to process in quick mode. We use more bits
        // for large alphabets. More bits means that more codes will be processed
        // in quick mode, bit also that more time will be spent to preparation
        // of tables for quick decode.
        dec.quick_bits = match size {
            NC | NC20 | NC30 => MAX_QUICK_DECODE_BITS,
            _ => MAX_QUICK_DECODE_BITS.saturating_sub(3),
        } as u32;

        // Size of tables for quick mode.
        let quick_data_size = 1 << dec.quick_bits;

        // Bit length for current code, start from 1 bit codes. It is important
        // to use 1 bit instead of 0 for minimum code length, so we are moving
        // forward even when processing a corrupt archive.
        let mut cur_bit_length = 1;

        // For every right aligned bit string which supports the quick decoding.
        for code in 0..quick_data_size {
            // Left align the current code, so it will be in usual bit field format.
            let bit_field = code << (16 - dec.quick_bits);

            // Prepare the table for quick decoding of bit lengths.

            // Find the upper limit for current bit field and adjust the bit length
            // accordingly if necessary.
            while cur_bit_length < dec.decode_len.len()
                && bit_field >= dec.decode_len[cur_bit_length]
            {
                cur_bit_length += 1;
            }

            // Translation of right aligned bit string to bit length.
            dec.quick_len[code as usize] = cur_bit_length as u8;

            // Prepare the table for quick translation of position in code list
            // to position in alphabet.

            // Calculate the distance from the start code for current bit length.
            let mut dist = bit_field - dec.decode_len[cur_bit_length - 1];

            // Right align the distance.
            dist >>= 16 - cur_bit_length;

            // Now we calculate the position in the code list. It is the sum
            // of first position for current bit length and right aligned distance
            // between our bit field and start code for current bit length.
            let pos;
            if cur_bit_length < dec.decode_pos.len() && {
                pos = dec.decode_pos[cur_bit_length] + dist;
                pos
            } < size
            {
                // Define the code to alphabet number translation.
                dec.quick_num[code as usize] = dec.decode_num[pos as usize];
            } else {
                // Can be here for length table filled with zeroes only (empty).
                dec.quick_num[code as usize] = 0;
            }
        }

        todo!()
    }
}
