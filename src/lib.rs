//! A library for reading RAR archives.
//!
//! This library provides a decoder for the RAR archive format(s) by Eugene Roshal.
//! It *does and will not implement an encoder* as it is forbidden by the RARLAB products'
//! licenses.
//!
//! RAR is a collection of several incompatible formats:
//!
//! - RAR13: supported by RAR version 1.3 (and older?)
//! - RAR14: supported by RAR version 1.4
//! - RAR15: supported from RAR version 1.5 to 4.x
//! - RAR50: supported from RAR version 5.0 onwards
//!
//! This library aims to document and provide support for analysis and decompression of files
//! of all of these formats, as well as a convenient compatibility layer for decompressing
//! all of these without handling the specifics of each format separately.
//!
//! At the moment it supports:
//!
//! - [ ] RAR13
//!   - Currently not supported due to lack of information on this format. If you have any
//!     RAR files this old lying around please get in touch!
//! - [ ] RAR14:
//!   - [x] Metadata
//!   - [ ] Decompression
//!   - [ ] Decryption
//! - RAR15:
//!   - [x] Metadata
//!   - [ ] Decompression
//!   - [ ] Decryption
//! - RAR50:
//!   - [x] Metadata
//!   - [ ] Decompression
//!   - [ ] Decryption
//!
//! We aim for 100% compatibility with all files generated by RARLAB products.
//! If you have a RAR file that you can extract with any version of WinRAR/UnRAR but not with
//! this library, please raise a bug!

#[macro_use]
mod macros;
pub mod compat;
pub mod error;
pub mod rar14;
pub mod rar15;
pub mod rar50;
mod read;
mod signature;
mod size;
mod time_conv;

pub use signature::Signature;
