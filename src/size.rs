/// Offset and size of the block in the file.
pub trait BlockSize {
    /// Offset of the block from the start of the file.
    fn offset(&self) -> u64;

    /// Size of the block's header from [`Self::offset`].
    fn header_size(&self) -> u64;

    /// Size of the data contained within the block from [`Self::offset`] + [`Self::header_size`].
    fn data_size(&self) -> u64;

    /// Full size of the block from [`Self::offset`].
    fn size(&self) -> u64 {
        self.header_size() + self.data_size()
    }
}
