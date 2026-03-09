//! TAR Archive Decoder
//!
//! This module provides the `Decoder` struct, which handles the extraction of
//! TAR streams. It reads 512-byte blocks, interprets USTAR headers, and
//! reconstructs files on the filesystem while preserving Unix permissions.

use crate::header::TarHeader;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// A minimalist TAR decoder designed for extracting archives on Unix-like systems.
///
/// The decoder reads from any type implementing `Read`, such as a `File` or
/// a `zstd::Decoder`, allowing for transparent decompression during extraction.
pub struct Decoder<R: Read> {
    /// The underlying reader containing the TAR stream.
    reader: R,
}

impl<R: Read> Decoder<R> {
    /// Creates a new `Decoder` instance.
    ///
    /// # Parameters
    /// * `reader`: An object implementing the `Read` trait (e.g., `File` or `zstd::Decoder`).
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Extracts the entire archive to the specified directory.
    ///
    /// This method iterates through the archive, creating directories and files
    /// as specified in the TAR headers. It includes protection against path
    /// traversal attacks.
    ///
    /// # Parameters
    /// * `dst`: The target base directory where the archive content will be extracted.
    ///
    /// # Errors
    /// Returns an `io::Error` if:
    /// * A header is malformed or contains invalid UTF-8.
    /// * A path traversal attempt is detected.
    /// * File or directory creation fails on the host system.
    pub fn unpack<P: AsRef<Path>>(&mut self, dst: P) -> io::Result<()> {
        let dst_path = dst.as_ref();
        if !dst_path.exists() {
            fs::create_dir_all(dst_path)?;
        }

        loop {
            let mut header_buf = [0u8; 512];
            match self.reader.read_exact(&mut header_buf) {
                Ok(_) => (),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }

            if header_buf.iter().all(|&b| b == 0) {
                break;
            }

            let header = unsafe { &*(header_buf.as_ptr() as *const TarHeader) };

            let name_bytes = header.name.split(|&b| b == 0).next().unwrap_or(&[]);
            let name_os_str = OsStr::from_bytes(name_bytes);

            let size = self.parse_octal(&header.size)?;
            let mode = self.parse_octal(&header.mode)? as u32;

            let target_path = dst_path.join(name_os_str);
            if !target_path.starts_with(dst_path) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Path traversal detected",
                ));
            }

            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if header.typeflag == b'0' || header.typeflag == b'\0' {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&target_path)?;

                file.set_permissions(fs::Permissions::from_mode(mode))?;

                let mut limit = self.reader.by_ref().take(size);
                io::copy(&mut limit, &mut file)?;

                let remainder = size % 512;
                if remainder > 0 {
                    let padding = 512 - remainder;
                    io::copy(&mut self.reader.by_ref().take(padding), &mut io::sink())?;
                }
            }
        }
        Ok(())
    }

    /// Parses octal strings from the TAR header fields.
    ///
    /// # Parameters
    /// * `bytes`: The byte slice from the header containing the octal ASCII string.
    ///
    /// # Returns
    /// The parsed `u64` value on success.
    fn parse_octal(&self, bytes: &[u8]) -> io::Result<u64> {
        let clean_bytes = bytes.split(|&b| b == 0 || b == b' ').next().unwrap_or(&[]);

        if clean_bytes.is_empty() {
            return Ok(0);
        }

        let s = std::str::from_utf8(clean_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid octal sequence"))?;

        u64::from_str_radix(s.trim(), 8)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to parse octal"))
    }
}
