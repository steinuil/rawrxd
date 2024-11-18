use std::{fs, io, process};

use rawrxd::{rar14, rar15, rar50, Signature};

fn main() -> io::Result<()> {
    let mut args = std::env::args();

    let Some(filename) = args.nth(1) else {
        eprintln!("No filename specified");
        process::exit(1);
    };
    let f = fs::File::open(&filename)?;
    let file_len = f.metadata()?.len();
    let mut f = io::BufReader::new(f);

    let Some((format, offset)) = Signature::search_stream(&mut f)? else {
        eprintln!("RAR signature not found");
        process::exit(1);
    };

    println!("{:?}", (format, offset));

    let offset = offset + format.size();

    match format {
        Signature::Rar14 => {
            let block_reader = rar14::BlockIterator::new(f, offset, file_len)?;

            for block in block_reader {
                let block = block?;
                println!("{block:#?}");
            }
        }
        Signature::Rar15 => {
            let block_reader = rar15::BlockIterator::new(f, offset, file_len)?;

            for block in block_reader {
                let block = block?;
                println!("{block:#?}");
            }
        }
        Signature::Rar50 => {
            let block_reader = rar50::BlockIterator::new(f, offset, file_len)?;

            for block in block_reader {
                let block = block?;
                println!("{block:#?}");
            }
        }
    }

    Ok(())
}
