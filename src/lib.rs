//! # tar_minimal
//!
//! A minimalist, Unix-focused TAR library designed for high-performance streaming.
//! This crate provides a lean alternative to the standard `tar` crate, specifically
//! optimized for integration with compression tools like `zstd` on Unix-like systems.

mod builder;
mod decoder;
mod header;

/// A builder for creating TAR archives, compatible with any `Write` implementation.
pub use builder::Builder;

/// A decoder for extracting TAR archives, compatible with any `Read` implementation.
pub use decoder::Decoder;

#[cfg(test)]
mod tests;