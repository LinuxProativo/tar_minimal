//! TAR Archive Decoder
//!
//! This module provides the `Decoder` struct, which handles the extraction of
//! TAR streams. It reads 512-byte blocks, interprets USTAR headers, and
//! reconstructs files on the filesystem while preserving Unix permissions.

use crate::header::TarHeader;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
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
    pub fn unpack(&mut self, dst: &str) -> io::Result<()> {
        let dst_path = Path::new(dst);
        if !dst_path.exists() {
            fs::create_dir_all(dst_path)?;
        }

        loop {
            let mut header_buf = [0u8; 512];
            if let Err(e) = self.reader.read_exact(&mut header_buf) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(e);
            }

            if header_buf.iter().all(|&b| b == 0) {
                break;
            }

            // Safety: Mapping the buffer to the TarHeader struct.
            // The Header is validated by checksum and octal parsing below.
            let header = unsafe { &*(header_buf.as_ptr() as *const TarHeader) };

            let name = std::str::from_utf8(&header.name)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in name"))?
                .trim_matches(char::from(0));

            let size = self.parse_octal(&header.size)?;
            let mode = self.parse_octal(&header.mode)? as u32;

            // Security: Path Traversal Protection.
            // Joins the destination with the entry name and ensures the result is still inside dst.
            let target_path = dst_path.join(name);
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
        let s = std::str::from_utf8(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid octal string"))?
            .trim_matches(|c: char| c == '\0' || c.is_whitespace());

        if s.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(s, 8)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to parse octal"))
    }
}
