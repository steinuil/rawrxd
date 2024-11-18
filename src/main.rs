#[macro_use]
mod macros;
pub mod compat;
pub mod rar14;
pub mod rar15;
pub mod rar50;
mod read;
pub mod signature;
mod size;
mod time_conv;

use std::{
    fs,
    io::{BufReader, Seek, SeekFrom},
};

use signature::Signature;

fn main() {
    let mut args = std::env::args();

    let filename = args.nth(1).unwrap();
    let f = fs::File::open(&filename).unwrap();
    let file_len = f.metadata().unwrap().len();
    let mut f = BufReader::new(f);

    let (format, offset) = signature::search_signature(&mut f).unwrap().unwrap();

    println!("{:?}", (format, offset));

    f.seek(SeekFrom::Start(offset + format.size())).unwrap();

    match format {
        Signature::Rar14 => {
            let block_reader = rar14::BlockIterator::new(f, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
        Signature::Rar15 => {
            let block_reader = rar15::BlockIterator::new(f, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
        Signature::Rar50 => {
            let block_reader = rar50::BlockIterator::new(f, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
    }
}
