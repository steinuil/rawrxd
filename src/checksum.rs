use std::io;

pub trait Checksum {
    type Output;

    fn new() -> Self;

    fn write(&mut self, data: &[u8]);

    fn finish(&self) -> Self::Output;
}

#[derive(Debug)]
pub struct Checksumming<'a, R: io::Read, C: Checksum> {
    inner: &'a mut R,
    checksum: C,
    frozen: bool,
}

impl<'a, R: io::Read, C: Checksum> Checksumming<'a, R, C> {
    pub fn new(inner: &'a mut R) -> Self {
        Self {
            inner,
            checksum: C::new(),
            frozen: false,
        }
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn checksum(&self) -> C::Output {
        self.checksum.finish()
    }

    pub fn release(self) -> &'a mut R {
        self.inner
    }
}

impl<'a, R: io::Read, C: Checksum> io::Read for Checksumming<'a, R, C> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;

        if !self.frozen {
            self.checksum.write(buf);
        }

        Ok(n)
    }
}
