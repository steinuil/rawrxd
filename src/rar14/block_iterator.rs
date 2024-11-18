use std::io;

use crate::size::BlockSize as _;

use super::{Block, FileBlock, MainBlock};

#[derive(Debug)]
pub struct BlockIterator<R: io::Read + io::Seek> {
    reader: R,
    file_size: u64,
    has_read_main_block: bool,
    next_block_position: u64,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    pub fn new(reader: R, offset: u64, file_size: u64) -> io::Result<Self> {
        Ok(Self {
            reader,
            file_size,
            has_read_main_block: false,
            next_block_position: offset,
        })
    }

    fn read_block(&mut self) -> io::Result<Block> {
        self.reader
            .seek(io::SeekFrom::Start(self.next_block_position))?;

        let block = if !self.has_read_main_block {
            let main_block = MainBlock::read(&mut self.reader)?;
            self.has_read_main_block = true;
            Block::Main(main_block)
        } else {
            Block::File(FileBlock::read(&mut self.reader)?)
        };

        self.next_block_position = block.position() + block.full_size();

        Ok(block)
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = io::Result<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_block_position == self.file_size {
            return None;
        }

        Some(self.read_block())
    }
}
