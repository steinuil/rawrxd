use std::{fs, io};

use rawrxd::{rar50, Signature};

mod unicode_filename;

fn block_iterator(file_name: &str) -> rar50::BlockIterator<io::BufReader<fs::File>> {
    let reader =
        io::BufReader::new(fs::File::open(format!("tests/fixtures/rar50/{file_name}")).unwrap());
    rar50::BlockIterator::new(reader, Signature::Rar50.size()).unwrap()
}
