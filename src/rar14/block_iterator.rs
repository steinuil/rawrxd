use std::io;

use crate::{
    error::{Error, RarResult},
    size::BlockSize as _,
};

use super::{Block, FileBlock, MainBlock};

#[derive(Debug)]
/// Iterator over the blocks of a RAR14 file.
///
/// Wraps an [`io::Read`] with an [`io::Seek`] impl and yields blocks until the EOF.
pub struct BlockIterator<R: io::Read + io::Seek> {
    reader: R,
    file_size: u64,
    next_offset: u64,
    has_read_main_block: bool,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    /// Create a [`BlockIterator`] starting at `offset`.
    ///
    /// `offset` must be the offset in the file right after the RAR14 signature.
    pub fn new(mut reader: R, offset: u64) -> RarResult<Self> {
        let file_size = reader.seek(io::SeekFrom::End(0))?;

        Ok(Self {
            reader,
            file_size,
            has_read_main_block: false,
            next_offset: offset,
        })
    }

    fn read_block(&mut self) -> RarResult<Block> {
        self.reader.seek(io::SeekFrom::Start(self.next_offset))?;

        let block = if !self.has_read_main_block {
            let main_block = MainBlock::read(&mut self.reader)?;
            self.has_read_main_block = true;
            Block::Main(main_block)
        } else {
            Block::File(FileBlock::read(&mut self.reader)?)
        };

        if block.size() == 0
            || block.offset() + block.header_size() > self.file_size
            || block.offset() + block.size() > self.file_size
        {
            return Err(Error::CorruptHeader);
        }

        self.next_offset = block.offset() + block.size();

        Ok(block)
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = RarResult<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_offset == self.file_size {
            return None;
        }

        Some(self.read_block())
    }
}
