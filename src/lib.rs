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
