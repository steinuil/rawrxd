use std::{fs, io};

use rawrxd::{rar14, rar15, rar50, Signature};

fn main() -> io::Result<()> {
    let args = std::env::args();

    for filename in args.skip(1) {
        println!("{filename}");

        let f = fs::File::open(&filename)?;
        let mut f = io::BufReader::new(f);

        let Some((format, offset)) = Signature::search_stream(&mut f)? else {
            eprintln!("RAR signature not found");
            continue;
        };

        println!("{:?}", (format, offset));

        let offset = offset + format.size();

        match format {
            Signature::Rar14 => {
                let block_reader = rar14::BlockIterator::new(f, offset)?;

                for block in block_reader {
                    let block = block?;
                    println!("{block:#?}");
                }
            }
            Signature::Rar15 => {
                let block_reader = rar15::BlockIterator::new(f, offset)?;

                for block in block_reader {
                    let block = block?;
                    println!("{block:#?}");
                }
            }
            Signature::Rar50 => {
                let block_reader = rar50::BlockIterator::new(f, offset)?;

                for block in block_reader {
                    let block = block?;
                    println!("{block:#?}");
                }
            }
        }
    }

    Ok(())
}
