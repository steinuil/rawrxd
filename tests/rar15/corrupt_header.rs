use crate::rar15::block_iterator;

use rawrxd::error::Error;
use rstest::rstest;

/// Archive whose header_size is set to 0, or whose offset + header_size or offset + size
/// exceed the EOF.
#[rstest]
#[case("corrupt_header_1")]
#[case("corrupt_header_2")]
#[case("corrupt_header_3")]
#[case("corrupt_header_4")]
#[case("corrupt_header_5")]
fn rar15_corrupt_header(#[case] name: &str) {
    let file_name = format!("{name}.rar");

    let mut iter = block_iterator(&file_name);

    let err = iter.find_map(|block| match block {
        Ok(_) => None,
        Err(e) => Some(e),
    });

    assert!(matches!(err, Some(Error::CorruptHeader)));
}
