mod decompress;
#[macro_use]
mod enum_macro;
#[macro_use]
mod flags;
mod block;
pub mod format;
pub mod rar14;
pub mod rar15;
pub mod rar50;
pub mod rar_file;
pub mod rarvm;
mod read;
mod size;
mod time_conv;

use std::{fs, io::BufReader};

use format::Format;

fn main() {
    let mut args = std::env::args();

    let filename = args.nth(1).unwrap();
    let f = fs::File::open(&filename).unwrap();
    let file_len = f.metadata().unwrap().len();
    let mut f = BufReader::new(f);

    let format = rar_file::read_signature(&mut f).unwrap().unwrap();

    println!("format: {:?}", format);
    println!();

    match format {
        Format::Rar14 => {
            let block_reader = rar14::BlockIterator::new(f, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
        Format::Rar15 => {
            let block_reader = rar15::BlockIterator::new(f, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
        Format::Rar50 => {
            let block_reader = rar50::BlockIterator::new(f, file_len).unwrap();

            for block in block_reader {
                let block = block.unwrap();
                println!("{block:#?}");
            }
        }
    }
}
