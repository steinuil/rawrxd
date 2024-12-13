//! RAR archive compressed with RAR 1.50 up to RAR 4.x.
//!
//! RAR15 was introduced with RAR 1.50 in 1994 and was used up to version 4.20 in 2012.
//! This version of the format has many revisions and "deprecated" fields and block types.

mod block_iterator;
mod blocks;
mod decode_file_name;
mod extended_time;

pub use block_iterator::*;
pub use blocks::*;

const NAME_MAX_SIZE: u16 = 1000;
