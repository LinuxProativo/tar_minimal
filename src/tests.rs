//! Integration Tests for tar_minimal
//!
//! These tests validate the full lifecycle of archive creation and extraction
//! using Zstd compression, ensuring metadata and content integrity.
//! Integration Tests for tar_minimal
//!
//! These tests validate the full lifecycle of archive creation and extraction
//! using Zstd compression, ensuring metadata and content integrity.

use crate::{Builder, Decoder};
use brotli::Decompressor as BrotliDecoder;
use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use lz4_flex::frame::FrameDecoder as Lz4Decoder;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;
use zstd::stream::{read::Decoder as ZstdDecoder, write::Encoder as ZstdEncoder};

#[test]
fn test_full_compression_cycle_in_temp() -> std::io::Result<()> {
    let temp_base = env::temp_dir().join("tar_minimal_test_as");
    let test_dir = temp_base.join("test_env");
    let extraction_dir = temp_base.join("extracted_logs");
    let output_tar_zst = temp_base.join("archive.tar.zst");

    let content = b"Minimalist Rust TAR + Zstd implementation test.";
    let file_name = "test_file.txt";

    if temp_base.exists() {
        fs::remove_dir_all(&temp_base)?;
    }
    fs::create_dir_all(&test_dir)?;

    let file_path = test_dir.join(file_name);
    {
        let mut f = File::create(&file_path)?;
        f.write_all(content)?;
        let mut perms = f.metadata()?.permissions();
        perms.set_mode(0o644);
        f.set_permissions(perms)?;
    }

    {
        let file_writer = File::create(&output_tar_zst)?;
        let mut enc = ZstdEncoder::new(file_writer, 3)?;
        enc.long_distance_matching(true)?;
        let mut encoder = enc.auto_finish();

        {
            let mut builder = Builder::new(&mut encoder);
            let name_in_tar = format!("test_env/{}", file_name);
            builder.append_path_as(&file_path, &name_in_tar)?;
            builder.finish()?;
        }
    }

    {
        let file_reader = File::open(&output_tar_zst)?;
        let zstd_decoder = ZstdDecoder::new(file_reader)?;

        let mut decoder = Decoder::new(zstd_decoder);
        decoder.unpack(extraction_dir.to_str().unwrap())?;
    }

    let extracted_path = extraction_dir.join("test_env").join(file_name);

    let mut extracted_content = Vec::new();
    let mut f = File::open(&extracted_path)?;
    f.read_to_end(&mut extracted_content)?;

    assert_eq!(content.to_vec(), extracted_content, "Content mismatch!");

    let metadata = fs::metadata(&extracted_path)?;
    assert_eq!(
        metadata.permissions().mode() & 0o777,
        0o644,
        "Permissions mismatch!"
    );

    fs::remove_dir_all(&temp_base)?;
    Ok(())
}

#[test]
fn test_recursive_directory_compression() -> std::io::Result<()> {
    let temp_base = env::temp_dir().join("tar_minimal_recursive_test");
    let test_dir = temp_base.join("source_folder");
    let sub_dir = test_dir.join("subdir");
    let extraction_dir = temp_base.join("extracted_result");
    let output_tar_zst = temp_base.join("recursive.tar.zst");

    fs::create_dir_all(&sub_dir)?;
    fs::write(test_dir.join("file1.txt"), b"data 1")?;
    fs::write(sub_dir.join("file2.txt"), b"data 2")?;

    {
        let file_writer = File::create(&output_tar_zst)?;
        let enc = ZstdEncoder::new(file_writer, 3)?;
        let mut encoder = enc.auto_finish();
        {
            let mut builder = Builder::new(&mut encoder);
            builder.append_dir_all("source_folder", &test_dir)?;
            builder.finish()?;
        }
    }

    {
        let file_reader = File::open(&output_tar_zst)?;
        let mut decoder = Decoder::new(ZstdDecoder::new(file_reader)?);
        decoder.unpack(extraction_dir.to_str().unwrap())?;
    }

    assert!(extraction_dir.join("source_folder/file1.txt").exists());
    assert!(
        extraction_dir
            .join("source_folder/subdir/file2.txt")
            .exists()
    );

    fs::remove_dir_all(&temp_base)?;
    Ok(())
}

#[test]
fn test_non_utf8_path_handling() -> std::io::Result<()> {
    let temp_base = env::temp_dir().join("tar_non_utf8_test");

    if temp_base.exists() {
        fs::remove_dir_all(&temp_base)?;
    }
    fs::create_dir_all(&temp_base)?;

    // Criamos um nome de arquivo com bytes inválidos para UTF-8 (0xFF não existe no UTF-8)
    let non_utf8_name =
        std::ffi::OsString::from_vec(vec![b'b', b'a', b'd', 0xff, b'.', b't', b'x', b't']);
    let file_path = temp_base.join(&non_utf8_name);

    let content = b"Data in a non-UTF8 filename";
    fs::write(&file_path, content)?;

    let output_tar = temp_base.join("non_utf8.tar");

    {
        let file_writer = File::create(&output_tar)?;
        let mut builder = Builder::new(file_writer);
        builder.append_path(&file_path)?;
        builder.finish()?;
    }

    assert!(output_tar.exists());

    let mut f = File::open(&output_tar)?;
    let mut header_buf = [0u8; 512];
    f.read_exact(&mut header_buf)?;

    assert!(header_buf.windows(4).any(|w| w == [b'b', b'a', b'd', 0xff]));

    fs::remove_dir_all(&temp_base)?;
    Ok(())
}

#[test]
fn test_compression_formats_compatibility() {
    let temp_base = env::temp_dir().join("tar_multi_compress_test");
    if temp_base.exists() {
        fs::remove_dir_all(&temp_base).expect("Cleanup failed");
    }
    fs::create_dir_all(&temp_base).expect("Dir creation failed");

    let file_path = temp_base.join("data.txt");
    let original_content = b"Multi-format compression test content";
    fs::write(&file_path, original_content).expect("Write failed");

    let relative_path_in_tar = file_path.strip_prefix("/").expect("Failed to strip prefix");

    let gz_path = temp_base.join("archive.tar.gz");
    {
        let tar_gz = File::create(&gz_path).expect("GZ: Create failed");
        let mut enc = GzEncoder::new(tar_gz, Compression::default());
        Builder::new(&mut enc).append_path(&file_path).expect("GZ: Append failed");
        enc.finish().expect("GZ: Finish failed");
    }
    {
        let file = File::open(&gz_path).expect("GZ: Open failed");
        let mut decoder = Decoder::new(GzDecoder::new(file));
        let extract_path = temp_base.join("extract_gz");
        decoder.unpack(&extract_path).expect("GZ: Unpack failed");

        assert_eq!(fs::read(extract_path.join(relative_path_in_tar)).expect("GZ: Not found"), original_content);
    }

    let xz_path = temp_base.join("archive.tar.xz");
    {
        let tar_xz = File::create(&xz_path).expect("XZ: Create failed");
        let mut enc = XzEncoder::new(tar_xz, 6);
        Builder::new(&mut enc).append_path(&file_path).expect("XZ: Append failed");
        enc.finish().expect("XZ: Finish failed");
    }
    {
        let file = File::open(&xz_path).expect("XZ: Open failed");
        let mut decoder = Decoder::new(XzDecoder::new(file));
        let extract_path = temp_base.join("extract_xz");
        decoder.unpack(&extract_path).expect("XZ: Unpack failed");
        assert_eq!(fs::read(extract_path.join(relative_path_in_tar)).expect("XZ: Not found"), original_content);
    }

    let bz2_path = temp_base.join("archive.tar.bz2");
    {
        let tar_bz2 = File::create(&bz2_path).expect("BZ2: Create failed");
        let mut enc = BzEncoder::new(tar_bz2, bzip2::Compression::best());
        Builder::new(&mut enc).append_path(&file_path).expect("BZ2: Append failed");
        enc.finish().expect("BZ2: Finish failed");
    }
    {
        let file = File::open(&bz2_path).expect("BZ2: Open failed");
        let mut decoder = Decoder::new(BzDecoder::new(file));
        let extract_path = temp_base.join("extract_bz2");
        decoder.unpack(&extract_path).expect("BZ2: Unpack failed");
        assert_eq!(fs::read(extract_path.join(relative_path_in_tar)).expect("BZ2: Not found"), original_content);
    }

    let lz4_path = temp_base.join("test.tar.lz4");
    {
        let f = File::create(&lz4_path).expect("LZ4: Create failed");
        let mut enc = lz4_flex::frame::FrameEncoder::new(f);
        Builder::new(&mut enc).append_path(&file_path).expect("LZ4: Append failed");
        enc.finish().expect("LZ4: Finish failed");
    }
    {
        let file = File::open(&lz4_path).expect("LZ4: Open failed");
        let mut decoder = Decoder::new(Lz4Decoder::new(file));
        let extract_path = temp_base.join("extract_lz4");
        decoder.unpack(&extract_path).expect("LZ4: Unpack failed");
        assert_eq!(fs::read(extract_path.join(relative_path_in_tar)).expect("LZ4: Not found"), original_content);
    }

    let br_path = temp_base.join("test.tar.br");
    {
        let f = File::create(&br_path).expect("BR: Create failed");
        let mut enc = brotli::CompressorWriter::new(f, 4096, 6, 20);
        Builder::new(&mut enc).append_path(&file_path).expect("BR: Append failed");
    }
    {
        let file = File::open(&br_path).expect("BR: Open failed");
        let mut decoder = Decoder::new(BrotliDecoder::new(file, 4096));
        let extract_path = temp_base.join("extract_br");
        decoder.unpack(&extract_path).expect("BR: Unpack failed");
        assert_eq!(fs::read(extract_path.join(relative_path_in_tar)).expect("BR: Not found"), original_content);
    }

    fs::remove_dir_all(&temp_base).expect("Final cleanup failed");
}
