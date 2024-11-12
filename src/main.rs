mod blocks;
mod decompress;
mod dos_time;
pub mod format;
pub mod rar14;
pub mod rar15;
pub mod rar_file;
mod rarvm;
mod read;

use std::{
    fs,
    io::{BufReader, Seek, SeekFrom},
};

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
            let block = rar14::MainHeader::read(&mut f).unwrap();
            println!("position: {}", block.position);
            println!("header_size: {}", block.header_size);
            println!("flags:");
            println!("  is_volume: {}", block.flags.is_volume());
            println!("  is_solid: {}", block.flags.is_solid());
            println!("  is_locked: {}", block.flags.is_locked());
            println!("  has_comment: {}", block.flags.has_comment());
            println!("  is_comment_packed: {}", block.flags.is_comment_packed());
            let comment = block.read_comment(&mut f).unwrap();
            println!("{:?}", comment);

            f.seek(SeekFrom::Start(block.position + block.header_size))
                .unwrap();

            loop {
                let pos = f.stream_position().unwrap();
                if pos == file_len {
                    break;
                }

                let block = rar14::FileHeader::read(&mut f).unwrap();
                println!("{:#?}", block);

                f.seek(SeekFrom::Start(
                    block.position + block.header_size + block.packed_data_size as u64,
                ))
                .unwrap();
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
        Format::Rar50 => todo!(),
    }
}
