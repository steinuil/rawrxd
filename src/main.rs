mod decompress;
mod dos_time;
#[macro_use]
mod enum_macro;
#[macro_use]
mod flags;
mod block;
pub mod format;
mod parse_result;
pub mod rar14;
pub mod rar15;
pub mod rar50;
pub mod rar_file;
mod rarvm;
mod read;
mod size;

use std::{
    fs,
    io::{BufReader, Seek, SeekFrom},
};

use format::Format;
use size::FullSize;

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
        Format::Rar15 => loop {
            let block = rar15::Block::read(&mut f).unwrap();
            println!("{block:#?}");
            if let rar15::BlockKind::EndArchive(_) = block.kind {
                break;
            }
            f.seek(SeekFrom::Start(block.position + block.full_size()))
                .unwrap();

            let pos = f.stream_position().unwrap();

            if pos == file_len {
                break;
            }
        },
        Format::Rar50 => loop {
            let block = rar50::Block::read(&mut f).unwrap();
            println!("{block:#?}");
            if let rar50::BlockKind::EndArchive(_) = block.kind {
                break;
            }

            f.seek(SeekFrom::Start(block.position + block.full_size()))
                .unwrap();

            let pos = f.stream_position().unwrap();

            if pos == file_len {
                break;
            }
        },
    }
}
