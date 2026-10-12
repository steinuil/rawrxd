pub struct BitInput {
    buf: Box<[u8; Self::MAX_SIZE + Self::PADDING]>,
    addr: usize,
    bit: u32,
}

impl BitInput {
    pub const MAX_SIZE: usize = 0x8000;

    const PADDING: usize = 8;

    pub fn new() -> Self {
        Self {
            buf: vec![0; Self::MAX_SIZE + Self::PADDING]
                .into_boxed_slice()
                .try_into()
                .expect("boxed array of the correct size"),
            addr: 0,
            bit: 0,
        }
    }

    /// Aligns the cursor to the next byte if we're in the middle of one.
    pub fn align_to_byte(&mut self) {
        if self.bit != 0 {
            self.addr += 1;
            self.bit = 0;
        }
    }

    /// Returns the bye position in the buffer.
    pub fn byte_position(&self) -> usize {
        self.addr
    }

    /// Moves forward by `bits` bits.
    pub fn add_bits(&mut self, bits: u32) {
        let bits = bits + self.bit;
        self.addr += (bits >> 3) as usize;
        self.bit = bits & 0b111
    }

    /// Reads `bits` bits into a `u8` and advances the cursor.
    pub fn read_u8(&mut self, bits: u32) -> u8 {
        assert!((1..=8).contains(&bits));

        let field = u16::from_be_bytes([self.buf[self.addr], self.buf[self.addr + 1]]);
        let value = ((field << self.bit) >> (16 - bits)) as u8;
        self.add_bits(bits);
        value
    }

    pub fn read_u16_be(&mut self, bits: u32) -> u16 {
        assert!((1..=16).contains(&bits));

        let b = &self.buf[self.addr..self.addr + 3];
        let field = u32::from_be_bytes([b[0], b[1], b[2], 0]);
        let value = ((field << self.bit) >> (32 - bits)) as u16;
        self.add_bits(bits);
        value
    }

    pub fn read_u32_be(&mut self, bits: u32) -> u32 {
        assert!((1..=32).contains(&bits));

        let b = &self.buf[self.addr..self.addr + 5];
        let field = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], 0, 0, 0]);
        let value = ((field << self.bit) >> (32 - bits)) as u32;
        self.add_bits(bits);
        value
    }

    pub fn read_u32_le(&mut self, bits: u32) -> u32 {
        assert!(bits.is_multiple_of(8) && (8..=32).contains(&bits));

        self.read_u32_be(bits).swap_bytes() >> (32 - bits)
    }

    /// Gets the next 16 bits.
    ///
    /// The bit at the current position is the most significant.
    pub fn get_bits16(&self) -> u32 {
        debug_assert!(self.addr + 3 <= self.buf.len());

        let b = &self.buf[self.addr..self.addr + 3];
        let field = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        (field >> (8 - self.bit)) & 0xffff
    }

    /// Get the next 32 bits.
    ///
    /// The bit at the current position is the most significant.
    pub fn get_bits32(&self) -> u32 {
        debug_assert!(self.addr + 5 <= self.buf.len());

        let b = &self.buf[self.addr..self.addr + 5];
        let field = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        (field << self.bit) | ((b[4] as u32) >> (8 - self.bit))
    }

    pub fn get_bits64(&self) -> u64 {
        debug_assert!(self.addr + 9 <= self.buf.len());

        let b = &self.buf[self.addr..self.addr + 9];
        let field = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        (field << self.bit) | ((b[8] as u64) >> (8 - self.bit))
    }
}
