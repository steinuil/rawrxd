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

    /// Moves forward by `bits` bits.
    pub fn add_bits(&mut self, bits: u32) {
        let bits = bits + self.bit;
        self.addr += (bits >> 3) as usize;
        self.bit = bits & 0xb111
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
