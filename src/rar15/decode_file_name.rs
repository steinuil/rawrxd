pub fn decode_file_name(mut file_name: Vec<u8>) -> Result<String, Vec<u8>> {
    let split_off_index = match file_name.iter().position(|c| c == &0) {
        // Nothing after the 0 byte
        Some(i) if file_name.len() == i + 1 => {
            file_name.pop();
            return String::from_utf8(file_name).map_err(|e| e.into_bytes());
        }

        // You are safe from this mess
        None => return String::from_utf8(file_name).map_err(|e| e.into_bytes()),

        Some(i) => i,
    };

    // RAR15 could have just encoded the filenames in UTF-8 but noooooo it had to come up
    // with its own weird encoding. Thank you RAR!
    decode_rar_encoded_string(&file_name, split_off_index).ok_or(file_name)
}

// TODO UnRAR uses a wstring as output, which is 32bit on Unix but 16bit on Windows.
// Does this mean that this is actually decoding to UTF-16 rather than UTF-32?
fn decode_rar_encoded_string(file_name: &[u8], split_off_index: usize) -> Option<String> {
    let (name, enc) = file_name.split_at(split_off_index);
    let mut enc = enc[1..].iter().copied().peekable();

    // We need to know the number of chars we pushed to the string in a few cases,
    // so we're using a Vec<char> instead of a String to avoid the O(N) cost
    // of .chars().count().
    let mut out_name = vec![];

    let high_byte = enc.next()? as u32;

    'outer: while enc.peek().is_some() {
        let instructions = enc.next()?;

        for i in 0..4 {
            if enc.peek().is_none() {
                break 'outer;
            }

            let instruction = Instruction::new(instructions, i);

            match instruction {
                Instruction::Byte => {
                    let char = enc.next()? as char;
                    out_name.push(char)
                }
                Instruction::ByteWithHigh => {
                    let low_char = enc.next()? as u32;
                    let char = char::from_u32(low_char | (high_byte << 8))?;
                    out_name.push(char)
                }
                Instruction::TwoBytes => {
                    let low_char = enc.next()? as u32;
                    let high_char = enc.next()? as u32;
                    let char = char::from_u32(low_char | (high_char << 8))?;
                    out_name.push(char)
                }
                Instruction::NameChunk => {
                    let length = enc.next()?;

                    match LengthInstruction::new(length) {
                        LengthInstruction::Chunk(length) => {
                            for _ in 0..length {
                                let char = *name.get(out_name.len())? as char;
                                out_name.push(char)
                            }
                        }
                        LengthInstruction::ChunkWithCorrection(length) => {
                            let correction = enc.next()? as u32;

                            for _ in 0..length {
                                let low_char = *name.get(out_name.len())? as u32;
                                let corrected_char = (low_char + correction) & 0xFF;
                                let char = char::from_u32(corrected_char | (high_byte << 8))?;
                                out_name.push(char)
                            }
                        }
                    }
                }
            }
        }
    }

    Some(String::from_iter(out_name))
}

#[derive(Debug)]
enum Instruction {
    /// Read the next byte from the encoded section.
    Byte,

    /// Read the next byte from the encoded section and prefix it with the high byte.
    ByteWithHigh,

    /// Read the next two bytes from the encoded section.
    TwoBytes,

    /// The next byte in the encoded section tells how many bytes to read from the
    /// name section and whether to apply an additional correction byte from the
    /// encoded section.
    NameChunk,
}

impl Instruction {
    fn new(instructions: u8, pos: u8) -> Self {
        // Decode instructions are stored in 2 bit chunks in highest to lowest bit order.
        let shift = (3 - pos) * 2;
        let instruction = (instructions >> shift) & 0x3;

        match instruction {
            0 => Self::Byte,
            1 => Self::ByteWithHigh,
            2 => Self::TwoBytes,
            3 => Self::NameChunk,
            _ => unreachable!("should not happen since flags has been masked with 3"),
        }
    }
}

#[derive(Debug)]
enum LengthInstruction {
    /// Read length + 2 characters from the name section.
    Chunk(u8),

    /// Read (length & !0x80) characters from the name section;
    /// the next byte in the encoded section contains a correction which needs
    /// to be added to the characters from the name section, along with the high byte.
    ChunkWithCorrection(u8),
}

impl LengthInstruction {
    const HAS_CORRECTION: u8 = 0x80;

    fn new(length: u8) -> Self {
        if length & Self::HAS_CORRECTION != 0 {
            Self::ChunkWithCorrection(length & !Self::HAS_CORRECTION)
        } else {
            Self::Chunk(length + 2)
        }
    }
}

#[test]
fn test_decode_file_name_shift_jis() {
    let file_name = b"(\x88\xEA\x94\xCA\x83Q\x81[\x83\x80)\
                      [PC][DVD][050617] Ever17 -the out of infinity- PE DVD Edition(iso+mds)\
                      \\EVER17_DVD.iso\x00N\x1A(\x00,\x82\xB20\xA0\xFC0\xE00)[\x00PC]\
                      [\x03DVD\x00\x000506\x0017] \x00Ever\x0017 -\x00the \x00out \x00of \
                      i\x00nfin\x00ity-\x00 PE \x00DVD \x00Edit\x00ion(\x00iso+\x00mds)\
                      \x00\\EVE\x00R17_\x00DVD.\x00iso"
        .to_vec();

    assert_eq!(
        decode_file_name(file_name).unwrap(),
        "(一般ゲーム)[PC][DVD][050617] Ever17 -the out of infinity- \
         PE DVD Edition(iso+mds)\\EVER17_DVD.iso"
    );
}

#[test]
fn test_decode_file_name_with_0_byte_end() {
    let file_name = b"test.rar\x00".to_vec();

    assert_eq!(decode_file_name(file_name).unwrap(), "test.rar");
}
