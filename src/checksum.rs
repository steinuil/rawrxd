pub trait Checksum {
    type Output;

    fn new() -> Self;

    fn write(&mut self, data: &[u8]);

    fn finish(&self) -> Self::Output;
}
