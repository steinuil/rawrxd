use std::{fs, io};

use rawrxd::Signature;

/// Not a RAR archive.
#[test]
fn bad_archive() {
    let mut reader =
        io::BufReader::new(fs::File::open("tests/fixtures/common/bad_archive.rar").unwrap());

    let signature = Signature::search_stream(&mut reader).unwrap();

    assert_eq!(signature, None);
}
