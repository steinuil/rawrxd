#![no_main]

use std::io::Cursor;

use libfuzzer_sys::fuzz_target;
use rawrxd::{rar14, rar15, rar50, Signature};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };

    let format = match selector % 3 {
        0 => Signature::Rar14,
        1 => Signature::Rar15,
        _ => Signature::Rar50,
    };

    let mut file = format.signature().to_vec();
    file.extend_from_slice(rest);
    let file_len = file.len();
    let reader = Cursor::new(file);
    let offset = format.size();

    macro_rules! drain {
        ($iter:expr) => {{
            let mut count = 0;

            for block in $iter {
                let _ = format!("{block:?}");
                count += 1;

                assert!(count <= file_len, "iterator does not terminate");
            }
        }};
    }

    match format {
        Signature::Rar14 => drain!(rar14::BlockIterator::new(reader, offset).unwrap()),
        Signature::Rar15 => drain!(rar15::BlockIterator::new(reader, offset).unwrap()),
        Signature::Rar50 => drain!(rar50::BlockIterator::new(reader, offset).unwrap()),
    }
});
