use std::{
    fs,
    io::{self, Cursor},
};

use rawrxd::{rar50, Signature};

mod unicode_filename;

fn block_iterator(file_name: &str) -> rar50::BlockIterator<io::BufReader<fs::File>> {
    let reader =
        io::BufReader::new(fs::File::open(format!("tests/fixtures/rar50/{file_name}")).unwrap());
    rar50::BlockIterator::new(reader, Signature::Rar50.size()).unwrap()
}

#[test]
fn huge_filename() {
    let mut data = b"Rar!\x1a\x07\x01\x00\x00\x00\x00\x00\x10\x03\x00\x00\x00\x00\x00\x00\x80\x80\x80\x80\x80\x80\x80\x80\x40".to_vec();
    data.extend([0; 64]);

    let mut iterator = rar50::BlockIterator::new(Cursor::new(data), 8).unwrap();
    assert!(matches!(
        iterator.next(),
        Some(Err(rawrxd::Error::UnexpectedEof))
    ))
}
