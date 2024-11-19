use std::{fs, io};

use rawrxd::{error::Error, rar15, Signature};
use rstest::rstest;

/// Not a RAR archive.
#[test]
fn bad_archive() {
    let mut reader =
        io::BufReader::new(fs::File::open("tests/fixtures/abnormal/bad_archive.rar").unwrap());

    let signature = Signature::search_stream(&mut reader).unwrap();

    assert_eq!(signature, None);
}

/// Archive whose header_size is set to 0, or whose offset + header_size or offset + size
/// exceed the EOF.
#[rstest]
#[case("rar15_corrupt_header_1")]
#[case("rar15_corrupt_header_2")]
#[case("rar15_corrupt_header_3")]
#[case("rar15_corrupt_header_4")]
#[case("rar15_corrupt_header_5")]
fn rar15_corrupt_header(#[case] name: &str) {
    let file_name = format!("tests/fixtures/abnormal/{name}.rar");

    let reader = io::BufReader::new(fs::File::open(file_name).unwrap());
    let mut iter = rar15::BlockIterator::new(reader, Signature::Rar15.size()).unwrap();

    let err = iter.find_map(|block| match block {
        Ok(_) => None,
        Err(e) => Some(e),
    });

    assert!(matches!(err, Some(Error::CorruptHeader)));
}
