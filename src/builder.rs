//! TAR Archive Builder
//!
//! This module provides the `Builder` struct, which is responsible for constructing
//! a TAR archive stream. It handles the conversion of filesystem metadata to
//! USTAR headers and manages byte-alignment (padding) required by the format.

use crate::header::TarHeader;
use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// A minimalist TAR builder designed for Unix-like systems and high-performance compression.
///
/// The builder wraps any type implementing `Write`, such as a `File` or a
/// `zstd::Encoder`, allowing for real-time compression of the archive stream.
pub struct Builder<W: Write> {
    /// The underlying writer where the TAR stream is sent.
    writer: W,
    /// Indicates if the termination blocks (1024 zero bytes) have been written.
    finished: bool,
}

impl<W: Write> Builder<W> {
    /// Creates a new `Builder` instance.
    ///
    /// # Parameters
    /// * `writer`: An object implementing the `Write` trait (e.g., `File`, `TcpStream`, or `zstd::Encoder`).
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            finished: false,
        }
    }

    /// Appends a file from the filesystem to the archive.
    ///
    /// This method reads the file metadata (UID, GID, permissions, size),
    /// constructs a 512-byte USTAR header, and streams the file content.
    ///
    /// # Parameters
    /// * `path`: The relative or absolute path to the file on the host system.
    ///
    /// # Errors
    /// Returns an `io::Error` if the file cannot be opened, metadata cannot be read,
    /// or if the writing process to the underlying stream fails.
    pub fn append_path<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let path = path.as_ref();
        let path_bytes = path.as_os_str().as_bytes();

        let clean_bytes = if !path_bytes.is_empty() && path_bytes[0] == b'/' {
            &path_bytes[1..]
        } else {
            path_bytes
        };

        self.append_path_as_bytes(path, clean_bytes)
    }

    /// Recursively appends the contents of a directory to the archive.
    ///
    /// This method traverses the filesystem starting at the given path and adds
    /// every file and subdirectory found to the TAR stream, preserving the
    /// internal directory structure under a specified prefix.
    ///
    /// # Parameters
    /// * `prefix_in_tar`: The base directory path to be used inside the archive.
    /// * `path`: The source directory on the host filesystem to be archived.
    ///
    /// # Errors
    /// Returns an `io::Error` if the directory cannot be read, or if any
    /// underlying file operation or write process fails.
    pub fn append_dir_all<P: AsRef<Path>>(
        &mut self,
        prefix_in_tar: &str,
        path: P,
    ) -> io::Result<()> {
        let path = path.as_ref();
        let prefix_bytes = prefix_in_tar.as_bytes();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let file_name = entry.file_name();
            let file_name_bytes = file_name.as_bytes();

            let mut internal_name =
                Vec::with_capacity(prefix_bytes.len() + 1 + file_name_bytes.len());
            internal_name.extend_from_slice(prefix_bytes);
            internal_name.push(b'/');
            internal_name.extend_from_slice(file_name_bytes);

            if entry_path.is_dir() {
                let next_prefix = String::from_utf8_lossy(&internal_name);
                self.append_dir_all(&next_prefix, &entry_path)?;
            } else {
                self.append_path_as_bytes(&entry_path, &internal_name)?;
            }
        }
        Ok(())
    }

    /// Appends a file to the archive using a custom name for the internal entry.
    ///
    /// Similar to `append_path`, but allows explicitly defining the path/name
    /// that will represent the file within the TAR archive, regardless of its
    /// actual location on the host disk.
    ///
    /// # Parameters
    /// * `source`: The physical path of the file to be read.
    /// * `name_in_tar`: The virtual path/name to be assigned within the archive.
    ///
    /// # Errors
    /// Returns an `io::Error` if the source file is inaccessible or if the
    /// header/content cannot be written to the stream.
    pub fn append_path_as<P: AsRef<Path>>(
        &mut self,
        source: P,
        name_in_tar: &str,
    ) -> io::Result<()> {
        self.append_path_as_bytes(source.as_ref(), name_in_tar.as_bytes())
    }

    /// Core internal function that handles the actual TAR encoding using byte slices.
    ///
    /// This method is the engine of the builder. It performs the low-level operations
    /// required to translate a file's filesystem presence into a TAR-compliant stream,
    /// working directly with bytes to ensure compatibility with non-UTF-8 paths.
    ///
    /// # Process Flow:
    /// 1. **Metadata Retrieval**: Opens the source file and extracts Unix-specific
    ///    metadata (UID, GID, Mode, Size, Mtime).
    /// 2. **Header Preparation**: Initializes a `TarHeader` and populates it using
    ///    octal encoding for numeric values.
    /// 3. **Path Sanitization**: Ensures the path inside the archive does not start
    ///    with a `/` to maintain portability.
    /// 4. **Checksum Calculation**: Computes the header checksum as required by
    ///    the POSIX USTAR standard.
    /// 5. **Data Streaming**: Writes the 512-byte header, followed by the file's
    ///    raw content, and finally adds null-byte padding to align to the 512-byte
    ///    block boundary.
    ///
    /// # Parameters
    /// * `source`: The reference to the physical `Path` on the host disk.
    /// * `name_in_tar`: A byte slice representing the name/path of the file
    ///   within the archive.
    ///
    /// # Errors
    /// Returns an `io::Error` if the file cannot be read, if the writer fails,
    /// or if the filesystem metadata is inaccessible.
    fn append_path_as_bytes(&mut self, source: &Path, name_in_tar: &[u8]) -> io::Result<()> {
        let mut file = File::open(source)?;
        let metadata = file.metadata()?;
        let mut header = TarHeader::new();

        let clean_name = if !name_in_tar.is_empty() && name_in_tar[0] == b'/' {
            &name_in_tar[1..]
        } else {
            name_in_tar
        };

        let name_len = clean_name.len().min(100);
        header.name[..name_len].copy_from_slice(&clean_name[..name_len]);

        TarHeader::set_octal(&mut header.mode, metadata.mode() as u64);
        TarHeader::set_octal(&mut header.uid, metadata.uid() as u64);
        TarHeader::set_octal(&mut header.gid, metadata.gid() as u64);
        TarHeader::set_octal(&mut header.size, metadata.len());
        TarHeader::set_octal(&mut header.mtime, metadata.mtime() as u64);
        header.typeflag = b'0';

        let cksum = header.calculate_checksum();
        TarHeader::set_octal(&mut header.checksum, cksum as u64);

        let header_ptr = &header as *const _ as *const u8;
        let header_slice = unsafe { std::slice::from_raw_parts(header_ptr, 512) };
        self.writer.write_all(header_slice)?;

        let n = io::copy(&mut file, &mut self.writer)?;
        let remainder = n % 512;
        if remainder > 0 {
            let padding = [0u8; 512];
            self.writer
                .write_all(&padding[..(512 - remainder as usize)])?;
        }
        Ok(())
    }

    /// Finalizes the TAR archive by writing the required termination blocks.
    ///
    /// According to the POSIX/USTAR standard, an archive must end with two
    /// consecutive 512-byte blocks of zero bytes. This method ensures these
    /// blocks are written and flushes the underlying writer.
    ///
    /// # Returns
    /// `Ok(())` if the archive was successfully finalized or was already finished.
    ///
    /// # Errors
    /// Returns an `io::Error` if writing the termination blocks or flushing fails.
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.finished {
            self.writer.write_all(&[0u8; 1024])?;
            self.writer.flush()?;
            self.finished = true;
        }
        Ok(())
    }
}
