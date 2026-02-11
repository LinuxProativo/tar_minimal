//! Integration Tests for tar_minimal
//!
//! These tests validate the full lifecycle of archive creation and extraction
//! using Zstd compression, ensuring metadata and content integrity.
//! Integration Tests for tar_minimal
//!
//! These tests validate the full lifecycle of archive creation and extraction
//! using Zstd compression, ensuring metadata and content integrity.

use crate::{Builder, Decoder};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
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

    // fs::remove_dir_all(&temp_base)?;
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

    assert!(extraction_dir.join("bundle/file1.txt").exists());
    assert!(extraction_dir.join("bundle/subdir/file2.txt").exists());

    // fs::remove_dir_all(&temp_base)?;
    Ok(())
}