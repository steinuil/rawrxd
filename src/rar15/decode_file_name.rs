// RAR15 could have just encoded the filenames in UTF-8 but noooooo it had to come up
// with its own weird encoding. Thank you RAR!
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

    let name_size = split_off_index;
    let name = &file_name[..split_off_index];

    let enc_name = &file_name[split_off_index + 1..];
    let enc_size = file_name.len() - split_off_index - 1;

    let mut out_name = String::new();

    let mut enc_pos = 0;
    let mut dec_pos = 0;

    let mut flags = 0;
    let mut counter = 0;

    let high_byte = enc_name[enc_pos] as u32;
    enc_pos += 1;

    while enc_pos < enc_size {
        if counter % 4 == 0 {
            flags = enc_name[enc_pos];
            enc_pos += 1;
        }

        if enc_pos >= enc_size {
            break;
        }

        match flags >> 6 {
            0 => {
                let char = enc_name[enc_pos] as char;
                enc_pos += 1;
                dec_pos += 1;
                out_name.push(char);
            }
            1 => {
                let char = char::from_u32(enc_name[enc_pos] as u32 + (high_byte << 8)).unwrap();
                enc_pos += 1;
                dec_pos += 1;
                out_name.push(char);
            }
            2 => {
                if enc_pos + 1 < enc_size {
                    let char = char::from_u32(
                        enc_name[enc_pos] as u32 + ((enc_name[enc_pos + 1] as u32) << 8),
                    )
                    .unwrap();
                    enc_pos += 2;
                    dec_pos += 1;
                    out_name.push(char);
                }
            }
            3 => {
                let length = enc_name[enc_pos];
                enc_pos += 1;

                if length & 0x80 != 0 {
                    if enc_pos < enc_size {
                        let correction = enc_name[enc_pos] as u32;
                        enc_pos += 1;

                        let mut length = (length & 0x7f) + 2;
                        loop {
                            if !(length > 0 && dec_pos < name_size) {
                                break;
                            }

                            let char = char::from_u32(
                                ((name[dec_pos] as u32 + correction) & 0xFF) + (high_byte << 8),
                            )
                            .unwrap();
                            out_name.push(char);

                            length -= 1;
                            dec_pos += 1;
                        }
                    }
                } else {
                    let mut length = length + 2;

                    loop {
                        if !(length > 0 && dec_pos < name_size) {
                            break;
                        }

                        let char = name[dec_pos] as char;
                        out_name.push(char);

                        length -= 1;
                        dec_pos += 1;
                    }
                }
            }
            n => panic!("{n}"),
        }

        flags <<= 2;
        counter += 1;
    }

    Ok(out_name)
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
