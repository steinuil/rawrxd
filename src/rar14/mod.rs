//! RAR archive compressed with RAR 1.4x.
//!
//! The RAR14 archive format was introduced in 1994 with RAR version 1.40
//! (or was it RAR 1.39?) and only supported MS-DOS. RAR14 archives contain the signature,
//! followed by the main block, followed by one or more file blocks.
//!
//! Since RAR14 only supported MS-DOS, some care must be taken to correctly handle filenames
//! and comments, which are encoded using an ANSI/OEM code page and may contain characters not
//! in the ASCII range.

mod block_iterator;
mod blocks;

pub use block_iterator::*;
pub use blocks::*;
