use std::{fs, io::BufReader};

use rawrxd::{rar14, rar15, rar50, signature::Signature};

fn main() {
    let mut args = std::env::args();

    let filename = args.nth(1).unwrap();
    let f = fs::File::open(&filename).unwrap();
    let file_len = f.metadata().unwrap().len();
    let mut f = BufReader::new(f);

    let (format, offset) = Signature::search_stream(&mut f).unwrap().unwrap();

    println!("{:?}", (format, offset));

    let offset = offset + format.size();

    match format {
        Signature::Rar14 => {
            let block_reader = rar14::BlockIterator::new(f, offset, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
        Signature::Rar15 => {
            let block_reader = rar15::BlockIterator::new(f, offset, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
        Signature::Rar50 => {
            let block_reader = rar50::BlockIterator::new(f, offset, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
    }
}
