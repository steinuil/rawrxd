use std::io;

use crate::size::BlockSize as _;

use super::{Block, BlockKind};

#[derive(Debug)]
pub struct BlockIterator<R: io::Read + io::Seek> {
    reader: R,
    file_size: u64,
    next_block_position: u64,
    end_of_archive_reached: bool,
}

impl<R: io::Read + io::Seek> BlockIterator<R> {
    pub fn new(reader: R, offset: u64, file_size: u64) -> io::Result<Self> {
        Ok(Self {
            reader,
            file_size,
            next_block_position: offset,
            end_of_archive_reached: false,
        })
    }

    fn read_block(&mut self) -> io::Result<Block> {
        self.reader
            .seek(io::SeekFrom::Start(self.next_block_position))?;

        let block = Block::read(&mut self.reader)?;

        self.next_block_position = block.position() + block.full_size();

        if let BlockKind::EndArchive(_) = block.kind {
            self.end_of_archive_reached = true;
        }

        Ok(block)
    }
}

impl<R: io::Read + io::Seek> Iterator for BlockIterator<R> {
    type Item = io::Result<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.end_of_archive_reached {
            return None;
        }

        if self.next_block_position == self.file_size {
            return None;
        }

        Some(self.read_block())
    }
}
