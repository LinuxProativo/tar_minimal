//! TAR Header Management
//!
//! This module defines the `TarHeader` structure and its associated methods
//! for handling the POSIX USTAR format. It provides utilities for checksum
//! calculation and octal encoding, which are fundamental to the TAR standard.

use std::mem;

/// Represents the POSIX USTAR header (512 bytes).
///
/// This structure is marked with `#[repr(C)]` to ensure the memory layout
/// matches the physical byte structure of a TAR header on disk.
#[repr(C)]
pub struct TarHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub checksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; 100],
    pub magic: [u8; 6],
    pub version: [u8; 2],
    pub uname: [u8; 32],
    pub gname: [u8; 32],
    pub devmajor: [u8; 8],
    pub devminor: [u8; 8],
    pub prefix: [u8; 155],
    pub padding: [u8; 12],
}

impl TarHeader {
    /// Creates a new header with default USTAR values.
    ///
    /// # Returns
    /// A `TarHeader` initialized with zeroed memory, except for the `magic`
    /// and `version` fields which are set to "ustar" as per the POSIX standard.
    pub fn new() -> Self {
        let mut h = unsafe { mem::zeroed::<TarHeader>() };
        h.magic.copy_from_slice(b"ustar ");
        h.version.copy_from_slice(b" \0");
        h
    }

    /// Calculates the checksum for the header.
    ///
    /// The checksum is the sum of all bytes in the 512-byte header.
    /// During calculation, the 8 bytes of the checksum field itself are
    /// treated as ASCII spaces (0x20).
    ///
    /// # Returns
    /// A `u32` representing the computed checksum value.
    pub fn calculate_checksum(&self) -> u32 {
        let ptr = self as *const _ as *const u8;
        let mut sum: u32 = 0;
        for i in 0..512 {
            if i >= 148 && i < 156 {
                sum += 32;
            } else {
                sum += unsafe { *ptr.add(i) } as u32;
            }
        }
        sum
    }

    /// Encodes a numeric value as an octal string into a byte slice.
    ///
    /// The TAR format requires most numeric fields (like size and mode)
    /// to be stored as octal strings rather than raw binary integers.
    ///
    /// # Parameters
    /// * `dst`: The destination byte slice where the octal string will be written.
    /// * `val`: The `u64` value to be converted to octal.
    pub fn set_octal(dst: &mut [u8], val: u64) {
        let len = dst.len();
        let s = format!("{:0>width$o}", val, width = len - 1);
        for (i, b) in s.as_bytes().iter().enumerate().take(len - 1) {
            dst[i] = *b;
        }
    }
}
