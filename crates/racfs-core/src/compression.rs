//! Compression trait and implementation for transparent compression in plugins.
//!
//! Plugins (e.g. StreamFS) can wrap read/write data with a [`Compression`]
//! implementation to reduce storage and transfer size. Compression is always
//! compiled in; enable or disable it at runtime via config (e.g. set
//! `compression: None` vs `compression: Some(compressor)` in StreamFS config).

use crate::error::FSError;

/// Compression level: speed vs ratio.
/// Zstd levels 1–22 (3 = default, 22 = best ratio).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompressionLevel {
    /// Use implementation default (zstd 3).
    #[default]
    Default,
    /// Favor speed over ratio (zstd 1).
    Fast,
    /// Favor ratio over speed (zstd 19).
    Best,
    /// Explicit level (zstd 1–22).
    Level(u8),
}

/// Trait for compressors used by plugins (e.g. StreamFS) for transparent compression.
pub trait Compression: Send + Sync {
    /// Compress data. Returns the compressed bytes.
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, FSError>;

    /// Decompress data. Returns the original bytes.
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, FSError>;

    /// Algorithm name for metadata (e.g. `"zstd"`).
    fn name(&self) -> &str;
}

/// Zstd compression. Use for transparent compression when enabled in config.
#[derive(Clone, Debug)]
pub struct ZstdCompression {
    level: i32,
}

impl ZstdCompression {
    /// Create a zstd compressor with the given level (1–22, 3 = default).
    pub fn new(level: CompressionLevel) -> Self {
        let l = match level {
            CompressionLevel::Default => 3,
            CompressionLevel::Fast => 1,
            CompressionLevel::Best => 19,
            CompressionLevel::Level(n) => (n as i32).clamp(1, 22),
        };
        Self { level: l }
    }
}

impl Compression for ZstdCompression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, FSError> {
        zstd::encode_all(std::io::Cursor::new(data), self.level).map_err(|e| {
            crate::error::FSError::Io {
                message: format!("zstd compress: {}", e),
            }
        })
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, FSError> {
        zstd::decode_all(std::io::Cursor::new(data)).map_err(|e| crate::error::FSError::Io {
            message: format!("zstd decompress: {}", e),
        })
    }

    fn name(&self) -> &str {
        "zstd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_roundtrip() {
        let c = ZstdCompression::new(CompressionLevel::Default);
        let data = b"hello world";
        let compressed = c.compress(data).unwrap();
        assert!(!compressed.is_empty() && compressed.len() <= data.len() + 32);
        let decompressed = c.decompress(&compressed).unwrap();
        assert_eq!(decompressed.as_slice(), data);
        assert_eq!(c.name(), "zstd");
    }

    #[test]
    fn test_zstd_levels() {
        let fast = ZstdCompression::new(CompressionLevel::Fast);
        let best = ZstdCompression::new(CompressionLevel::Best);
        let data = b"hello world";
        assert_eq!(fast.name(), "zstd");
        assert_eq!(best.name(), "zstd");
        assert_eq!(
            fast.decompress(&fast.compress(data).unwrap()).unwrap(),
            data
        );
        assert_eq!(
            best.decompress(&best.compress(data).unwrap()).unwrap(),
            data
        );
    }

    #[test]
    fn test_zstd_level_explicit_and_clamp() {
        let c = ZstdCompression::new(CompressionLevel::Level(10));
        let data = b"hello world";
        let compressed = c.compress(data).unwrap();
        assert_eq!(c.decompress(&compressed).unwrap(), data);
        // Level 0 and 255 clamp to 1..22
        let c_lo = ZstdCompression::new(CompressionLevel::Level(0));
        let c_hi = ZstdCompression::new(CompressionLevel::Level(255));
        assert_eq!(
            c_lo.decompress(&c_lo.compress(data).unwrap()).unwrap(),
            data
        );
        assert_eq!(
            c_hi.decompress(&c_hi.compress(data).unwrap()).unwrap(),
            data
        );
    }
}
