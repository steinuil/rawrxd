use std::io;

use thiserror::Error;

/// Specialized [`Result`] type for decoding RAR archives.
pub type RarResult<T> = core::result::Result<T, Error>;

#[derive(Debug, Error)]
/// A fatal error that may occur while decoding a RAR file.
pub enum Error {
    /// Tried to read after the end of the stream while decoding.
    #[error("expected more data")]
    UnexpectedEof,

    /// A block header reported a size equal to 0 or exceeding the EOF.
    #[error("header reported sizes are 0 or exceed the EOF")]
    CorruptHeader,

    /// Unknown I/O error.
    #[error(transparent)]
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            _ => Self::Io(value),
        }
    }
}
