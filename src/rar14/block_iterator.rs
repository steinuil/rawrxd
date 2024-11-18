use std::io;

use crate::size::BlockSize as _;

use super::{Block, FileBlock, MainBlock};

#[derive(Debug)]
pub struct BlockIterator<R: io::Read + io::Seek> {
    reader: R,
    file_size: u64,
    next_offset: u64,
    has_read_main_block: bool,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    pub fn new(mut reader: R, offset: u64) -> io::Result<Self> {
        let file_size = reader.seek(io::SeekFrom::End(0))?;

        Ok(Self {
            reader,
            file_size,
            has_read_main_block: false,
            next_offset: offset,
        })
    }

    fn read_block(&mut self) -> io::Result<Block> {
        self.reader.seek(io::SeekFrom::Start(self.next_offset))?;

        let block = if !self.has_read_main_block {
            let main_block = MainBlock::read(&mut self.reader)?;
            self.has_read_main_block = true;
            Block::Main(main_block)
        } else {
            Block::File(FileBlock::read(&mut self.reader)?)
        };

        self.next_offset = block.offset() + block.size();

        if block.size() == 0 {
            todo!("Implement something like a MaliciousHeader error")
        }

        Ok(block)
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = io::Result<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_offset == self.file_size {
            return None;
        }

        Some(self.read_block())
    }
}
