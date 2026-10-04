/// Offset and size of the block in the file.
pub trait BlockSize {
    /// Offset of the block from the start of the file.
    fn offset(&self) -> u64;

    /// Size of the block's header from [`Self::offset`].
    fn header_size(&self) -> u64;

    /// Size of the data contained within the block from [`Self::offset`] + [`Self::header_size`].
    fn data_size(&self) -> u64;

    /// Full size of the block from [`Self::offset`].
    ///
    /// Returns `None` when the addition overflows.
    fn size(&self) -> Option<u64> {
        self.header_size().checked_add(self.data_size())
    }
}

pub fn next_block_offset<B: BlockSize>(block: &B, file_size: u64) -> Option<u64> {
    let next_offset = block.offset().checked_add(block.size()?)?;

    let has_advanced = next_offset > block.offset();
    let is_within_file_bounds = next_offset <= file_size;

    (has_advanced && is_within_file_bounds).then_some(next_offset)
}
