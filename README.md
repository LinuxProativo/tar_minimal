<p align="center">
  <img src="logo.png" width="300" alt="">
</p>

<h1 align="center">tar_minimal - Minimal Rust library for TAR archiving</h1>

### `tar_minimal` is a minimalist, high-performance Rust library for TAR archiving with Zstd compression, specifically designed for Unix-like environments.

It's a focused, "no-frills" alternative to robust archiving crates like
`tar-rs`. In a world of feature-heavy libraries, this crate carves out a niche for
developers who need exactly two things: packing a directory and unpacking a bundle,
with the best-in-class compression provided by Zstd.

It was engineered for automated deployment systems, log rotators, and payload
delivery mechanisms where build times, binary size, and execution speed are more
important than supporting legacy formats or cross-platform edge cases.

## 🛠 Key Features

* **Unix-Native Core**  
  Leverages Unix-specific traits for handling file permissions (mode) and metadata,
  ensuring fidelity on Linux/BSD.

* **Zstd-Centric**  
  Unlike other libraries that treat compression as an afterthought, `tar_minimal`
  was primarily designed around Zstd streams.

* **Safety First**  
  Built-in protection against Path Traversal attacks during extraction.

* **Lean Dependency Tree**  
  Zero bloat. We avoid heavy crates like walkdir or complex async runtimes by default.


## ⚠️ Scope & Critical Limitations

This library is highly specialized. To maintain its "minimalist" status, we
explicitly chose not to implement certain features:

* **No Random Access / Listing**  
  You cannot list files or read a single file from the archive. It's a "stream-in"
  (Builder) or "stream-out" (Decoder) architecture.

* **No Windows Support**  
  While it might compile, file permissions and path handling are not guaranteed or
  tested on Windows.

* **Strict Format Support**  
  Only integration with Zstd is currently guaranteed and tested. Other formats
  such as Gzip, Bzip2, or Xz have not yet been validated.

* **Basic Metadata Only**  
  We handle standard permissions (UID/GID/Mode/MTime), but we do not support
  extended attributes (xattrs), ACLs, or complex PAX headers.

* **No In-Place Updates**  
  Archives are immutable once created. You cannot append or delete files from
  an existing `.tar.zst` bundle.


## 📦 Usage

```toml
[dependencies]
tar_minimal = "0.1.0"
zstd = "0.13"
```

## 📁 Creating an Archive/Bunble (TAR + Zstd)

```rust
use tar_minimal::Builder;
use zstd::stream::write::Encoder;
use std::fs::File;

fn main() -> std::io::Result<()> {
    let file_writer = File::create("bundle.tar.zst")?;
    let mut enc = Encoder::new(file_writer, 5)?;
    enc.long_distance_matching(true)?;
    let mut encoder = enc.auto_finish();

    {
        let mut builder = Builder::new(&mut encoder);
        builder.append_dir_all("myapp_bundle", "/path/to/source")?;
        builder.finish()?;
    }

    Ok(())
}
```

## 📤 Extracting an Archive/Bunble

```rust
use tar_minimal::Decoder;
use zstd::stream::read::Decoder as ZstdDecoder;
use std::fs::File;

fn main() -> std::io::Result<()> {
    let file_reader = File::open("bundle.tar.zst")?;
    let zstd_decoder = ZstdDecoder::new(file_reader)?;

    let mut decoder = Decoder::new(zstd_decoder);
    decoder.unpack("/path/to/extract")?;

    Ok(())
}
```

## 🤝 Contributing

We welcome contributions, but we are very protective of the library's scope.
Please follow these guidelines carefully.

### ✅ What we are looking for

* **Security Audits**  
  Improvements to the `Decoder` to prevent zip-slip vulnerabilities, path traversal
  issues, and symlink-based attacks.

* **Stability & Robustness**  
  Fixes for I/O edge cases, error handling, and buffer management.

* **Zstd Performance Tuning**  
  Optimizations in how TAR blocks are streamed into Zstd frames for better
  compression ratios and throughput.

* **Documentation Enhancements**  
  Clearer examples, improved explanations, and more actionable error messages.

### 🔧 Optional & Feature-Gated Enhancements

The core functionality of `tar_minimal` will remain strictly minimal and
dependency-light. However, non-essential capabilities may be accepted
**behind optional Cargo feature flags**, allowing advanced users to extend
behavior without impacting the default experience.

1. **Possible feature-gated additions include:**
    * **Platform-Specific Behavior**  
      Optional compatibility layers for non-Unix systems (such as Windows),
      implemented in isolation to avoid polluting the Unix-focused core.

    * **Additional Compression Backends**  
      Support for alternative algorithms such as Gzip, Lz4, Xz, or others
      may be added as optional features or core integrations, depending
      on future architectural adjustments.

    * **Extended Metadata Support**  
      Optional handling of xattrs, ACLs, and PAX headers for users who need richer
      filesystem fidelity.

    * **Performance Helpers & Utilities**
      Enhancements that may introduce extra dependencies for speed, parallelism,
      or buffering strategies.

2. **All optional features must:**
    * **Be disabled by default**  
      The base crate must remain lean and predictable.

    * **Preserve a minimal dependency tree**  
      No heavy crates unless absolutely necessary and clearly justified.

    * **Maintain core security guarantees**  
      Optional functionality must not weaken extraction safety or validation logic.

### 📦 Design Philosophy (Feature-Based Expansion)

To preserve the minimalist nature of `tar_minimal`:

* **The core remains Unix + Zstd focused**  
  This is the guaranteed, stable, and optimized path.

* **All expanded functionality lives behind Cargo features**  
  No hidden dependencies or surprise behaviors.

* **Users only pay for what they enable**  
  Both in compile time and dependency complexity.

This ensures the crate scales from ultra-lightweight tooling to
more advanced use cases without compromising its original goals.

### ⚙️ Optional Features — Outside the Core Scope

Functionality beyond the minimal archive pipeline may be considered if:

* **It is cleanly isolated from the core logic**  
  Clear module boundaries and no cross-contamination.

* **It does not bloat default builds**  
  The zero-feature experience must remain fast and small.

* **It follows Rust feature best practices**  
  Predictable flags, no implicit enables, and clear documentation.


## 📄 License

This project is licensed under the MIT License. See the `LICENSE` file for details.
