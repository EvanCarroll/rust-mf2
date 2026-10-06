//! The compressors behind B7 (P0.7's method), and the
//! alternatives the owner chooses between for CI:
//!
//! * **GNU gzip** — `gzip -9 -n -c` as a subprocess, what P0.7 measured. Its
//!   availability and version depend on the machine.
//! * **flate2** — `GzEncoder` with flate2's default backend (`miniz_oxide`, pure
//!   Rust) at [`FLATE2_LEVEL`], the best `flate2::Compression` offers.
//! * **zlib-rs** — the pure-Rust port of zlib, `DeflateConfig::best_compression()`
//!   (level 9) with a gzip wrapper.
//! * **brotli** — the `brotli` crate, quality 11, window 22 (as P0.7).
//!
//! Every gzip figure includes the 18 B of gzip framing (10 B header without a
//! name or time stamp, 8 B trailer), so the three are directly comparable.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::error::{Error, Result};

/// flate2's maximum level (`Compression::best()`).
pub const FLATE2_LEVEL: u32 = 9;

/// Which gzip implementation a B7 verdict uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Gz {
    /// GNU `gzip -9 -n` (subprocess), as P0.7.
    Gnu,
    /// flate2 (`miniz_oxide`) at level 9.
    Flate2,
    /// zlib-rs at level 9.
    ZlibRs,
}

impl Gz {
    /// Every implementation, in report order.
    pub const ALL: [Gz; 3] = [Gz::Gnu, Gz::Flate2, Gz::ZlibRs];

    /// Short label for tables.
    pub const fn label(self) -> &'static str {
        match self {
            Gz::Gnu => "GNU gzip -9 -n",
            Gz::Flate2 => "flate2 -9",
            Gz::ZlibRs => "zlib-rs -9",
        }
    }
}

/// One byte string, raw and compressed every way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Compressed {
    /// Uncompressed bytes.
    pub raw: usize,
    /// GNU gzip; `None` when `gzip` is not installed.
    pub gz_gnu: Option<usize>,
    /// flate2 (`miniz_oxide`).
    pub gz_flate2: usize,
    /// zlib-rs.
    pub gz_zlib_rs: usize,
    /// brotli 11 / 22.
    pub br: usize,
}

impl Compressed {
    /// The gzip size by implementation.
    pub fn gz(&self, which: Gz) -> Option<usize> {
        match which {
            Gz::Gnu => self.gz_gnu,
            Gz::Flate2 => Some(self.gz_flate2),
            Gz::ZlibRs => Some(self.gz_zlib_rs),
        }
    }
}

/// The compressors of one run.
#[derive(Debug, Clone)]
pub struct Compressors {
    /// `gzip --version`'s first line, if GNU gzip is installed.
    pub gnu_version: Option<String>,
}

impl Compressors {
    /// Finds GNU gzip (it may be absent: every other figure is still made).
    pub fn detect() -> Self {
        let gnu_version = Command::new("gzip")
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.lines().next().map(str::to_owned))
            // On Alpine `gzip` is a busybox applet, which is not what the
            // `GNU gzip -9 -n` figures were measured with and does not produce
            // the same bytes. A figure carries its compressor's name, so an
            // impostor is no compressor at all here.
            .filter(|line| !line.to_ascii_lowercase().contains("busybox"));
        Compressors { gnu_version }
    }

    /// `data`, raw and compressed every way.
    pub fn all(&self, data: &[u8]) -> Result<Compressed> {
        Ok(Compressed {
            raw: data.len(),
            gz_gnu: if self.gnu_version.is_some() {
                Some(gnu_gzip(data)?)
            } else {
                None
            },
            gz_flate2: flate2_gzip(data)?,
            gz_zlib_rs: zlib_rs_gzip(data)?,
            br: brotli(data)?,
        })
    }
}

/// `gzip -9 -n -c` over `data`: the length of its output.
pub fn gnu_gzip(data: &[u8]) -> Result<usize> {
    let command = "gzip -9 -n -c";
    let fail = |message: String| Error::Compressor {
        command: command.to_owned(),
        message,
    };
    let mut child = Command::new("gzip")
        .args(["-9", "-n", "-c"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| fail(e.to_string()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| fail("no stdin".to_owned()))?;
    // Feed stdin from another thread so a full stdout pipe cannot deadlock.
    let output = std::thread::scope(|s| {
        let feeder = s.spawn(move || stdin.write_all(data));
        let output = child.wait_with_output();
        let fed = feeder.join().unwrap_or(Ok(()));
        fed.and(output)
    })
    .map_err(|e| fail(e.to_string()))?;
    if !output.status.success() {
        return Err(fail(output.status.to_string()));
    }
    Ok(output.stdout.len())
}

/// flate2's `GzEncoder` (default backend) at [`FLATE2_LEVEL`].
pub fn flate2_gzip(data: &[u8]) -> Result<usize> {
    let mut enc = flate2::write::GzEncoder::new(
        Vec::with_capacity(data.len() / 2),
        flate2::Compression::new(FLATE2_LEVEL),
    );
    enc.write_all(data)?;
    Ok(enc.finish()?.len())
}

/// zlib-rs, `best_compression()` (level 9), gzip wrapper (window bits 16 + 15).
pub fn zlib_rs_gzip(data: &[u8]) -> Result<usize> {
    let mut config = zlib_rs::DeflateConfig::best_compression();
    config.window_bits = 16 + 15;
    // compress_bound assumes the 6 B zlib wrapper; gzip's is 18 B.
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len()) + 64];
    let (compressed, rc) = zlib_rs::compress_slice(&mut out, data, config);
    if rc != zlib_rs::ReturnCode::Ok {
        return Err(Error::Compressor {
            command: "zlib-rs compress_slice".to_owned(),
            message: format!("{rc:?}"),
        });
    }
    Ok(compressed.len())
}

/// brotli quality 11, window 22 (P0.7's call).
pub fn brotli(data: &[u8]) -> Result<usize> {
    let mut w = brotli::CompressorWriter::new(Vec::with_capacity(data.len() / 2), 4096, 11, 22);
    w.write_all(data)?;
    Ok(w.into_inner().len())
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::{brotli, flate2_gzip, zlib_rs_gzip};

    fn sample() -> Vec<u8> {
        (0..20_000u32)
            .flat_map(|i| format!("message {} of {}\0", i % 97, i % 13).into_bytes())
            .collect()
    }

    #[test]
    fn zlib_rs_writes_gzip_that_flate2_reads_back() {
        let data = sample();
        let mut config = zlib_rs::DeflateConfig::best_compression();
        config.window_bits = 31;
        let mut out = vec![0u8; zlib_rs::compress_bound(data.len()) + 64];
        let (gz, rc) = zlib_rs::compress_slice(&mut out, &data, config);
        assert_eq!(rc, zlib_rs::ReturnCode::Ok);
        assert_eq!(&gz[..2], &[0x1f, 0x8b]);
        let mut back = Vec::new();
        flate2::read::GzDecoder::new(&gz[..])
            .read_to_end(&mut back)
            .unwrap();
        assert_eq!(back, data);
        assert_eq!(zlib_rs_gzip(&data).unwrap(), gz.len());
    }

    #[test]
    fn every_compressor_shrinks_repetitive_input() {
        let data = sample();
        for n in [
            flate2_gzip(&data).unwrap(),
            zlib_rs_gzip(&data).unwrap(),
            brotli(&data).unwrap(),
        ] {
            assert!(n > 18 && n < data.len() / 4, "{n}");
        }
    }
}
