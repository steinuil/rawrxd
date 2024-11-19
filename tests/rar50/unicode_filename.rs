use rawrxd::rar50::{Block, BlockKind};

use super::block_iterator;

#[test]
fn unicode_filename() {
    let mut iter = block_iterator("unix_high_ascii_filename.rar");
    let f = iter
        .find_map(|block| match block {
            Ok(Block {
                kind: BlockKind::File(file),
                ..
            }) => Some(file),
            _ => None,
        })
        .unwrap();

    assert_eq!(f.name, Ok("Æ".to_string()));
}
