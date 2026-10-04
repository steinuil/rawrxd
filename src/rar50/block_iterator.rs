use std::io;

use crate::{
    error::{Error, RarResult},
    size::next_block_offset,
};

use super::{Block, BlockKind};

#[derive(Debug)]
pub struct BlockIterator<R: io::Read + io::Seek> {
    reader: R,
    file_size: u64,
    next_offset: u64,
    done: bool,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    pub fn new(mut reader: R, offset: u64) -> RarResult<Self> {
        let file_size = reader.seek(io::SeekFrom::End(0))?;

        Ok(Self {
            reader,
            file_size,
            next_offset: offset,
            done: false,
        })
    }

    fn read_block(&mut self) -> RarResult<Block> {
        self.reader.seek(io::SeekFrom::Start(self.next_offset))?;

        let block = Block::read(&mut self.reader)?;

        self.next_offset = next_block_offset(&block, self.file_size).ok_or(Error::CorruptHeader)?;

        if let BlockKind::EndArchive(_) = block.kind {
            self.done = true;
        }

        Ok(block)
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = RarResult<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.next_offset == self.file_size {
            return None;
        }

        let block = self.read_block();
        if block.is_err() {
            self.done = true;
        }

        Some(block)
    }
}
