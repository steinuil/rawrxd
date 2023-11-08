mod rar29 {
    const L_DECODE: [u8; 28] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96, 112,
        128, 160, 192, 224,
    ];

    const L_BITS: [u8; 28] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5,
    ];

    const D_BIT_LENGTH_COUNTS: [u8; 19] =
        [4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 14, 0, 12];

    const SD_DECODE: [u8; 8] = [0, 4, 8, 16, 32, 64, 128, 192];
    const SD_BITS: [u8; 8] = [2, 2, 3, 4, 5, 6, 6, 6];

    pub fn unpack(data: &[u8]) -> Vec<u8> {
        let mut l_decode = L_DECODE.to_vec();
        let mut l_bits = L_BITS.to_vec();

        let mut d_decode: Vec<u8> = vec![0; DC];
        let mut d_bits: Vec<u8> = vec![0; DC];
        let mut d_bit_length_counts = D_BIT_LENGTH_COUNTS.to_vec();

        let mut sd_decode = SD_DECODE.to_vec();
        let mut sd_bits = SD_BITS.to_vec();
    }
}

// pub fn decompress(data: &[u8]) -> Vec<u8> {}
