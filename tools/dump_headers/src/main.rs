use std::{fs, io};

use rawrxd::{RarResult, Signature, rar14, rar15, rar50};

#[derive(serde::Serialize)]
struct RarFile<T> {
    filename: String,
    format: Signature,
    blocks: Vec<T>,
}

fn dump_headers(filename: &str) -> RarResult<()> {
    let f = fs::File::open(filename)?;
    let mut f = io::BufReader::new(f);

    let Some((format, offset)) = Signature::search_stream(&mut f)? else {
        eprintln!("RAR signature not found");
        return Ok(());
    };

    match format {
        Signature::Rar14 => {
            let block_reader = rar14::BlockIterator::new(f, offset)?;
            let blocks = block_reader.into_iter().collect::<Result<_, _>>()?;

            let rar_file = RarFile {
                filename: filename.to_owned(),
                format: Signature::Rar14,
                blocks,
            };

            serde_json::to_writer_pretty(io::stdout(), &rar_file).unwrap();
        }
        Signature::Rar15 => {
            let block_reader = rar15::BlockIterator::new(f, offset)?;
            let blocks = block_reader.into_iter().collect::<Result<_, _>>()?;

            let rar_file = RarFile {
                filename: filename.to_owned(),
                format: Signature::Rar15,
                blocks,
            };

            serde_json::to_writer_pretty(io::stdout(), &rar_file).unwrap();
        }
        Signature::Rar50 => {
            let block_reader = rar50::BlockIterator::new(f, offset)?;
            let blocks = block_reader.into_iter().collect::<Result<_, _>>()?;

            let rar_file = RarFile {
                filename: filename.to_owned(),
                format: Signature::Rar50,
                blocks,
            };

            serde_json::to_writer_pretty(io::stdout(), &rar_file).unwrap();
        }
    }

    Ok(())
}

pub fn main() {
    let args = std::env::args();

    for filename in args.skip(1) {
        if let Err(e) = dump_headers(&filename) {
            eprintln!("{e:?}")
        }
    }
}
