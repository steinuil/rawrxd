use core::fmt;
use std::{backtrace::Backtrace, io};

use thiserror::Error;

/// Specialized [`Result`] type for decoding RAR archives.
pub type RarResult<T> = core::result::Result<T, Error>;

#[derive(Debug, Error)]
/// A fatal error that may occur while decoding a RAR file.
pub enum ErrorKind {
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

pub struct Error {
    kind: ErrorKind,
    backtrace: Backtrace,
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt::Display::fmt(&self.kind, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}\n{}", self.kind, self.backtrace)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            io::ErrorKind::UnexpectedEof => ErrorKind::UnexpectedEof.into(),
            _ => ErrorKind::Io(value).into(),
        }
    }
}
