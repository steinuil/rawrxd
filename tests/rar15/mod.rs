use std::{fs, io};

use rawrxd::{rar15, Signature};

mod corrupt_header;

fn block_iterator(file_name: &str) -> rar15::BlockIterator<io::BufReader<fs::File>> {
    let reader =
        io::BufReader::new(fs::File::open(format!("tests/fixtures/rar15/{file_name}")).unwrap());
    rar15::BlockIterator::new(reader, Signature::Rar15.size()).unwrap()
}
