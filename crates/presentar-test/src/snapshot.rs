//! Visual regression testing via snapshot comparison.
//!
//! Pure Rust implementation - no external dependencies.

use std::path::{Path, PathBuf};

/// Image data for snapshot comparison.
#[derive(Debug, Clone)]
pub struct Image {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// RGBA pixel data (4 bytes per pixel)
    pub data: Vec<u8>,
}

impl Image {
    /// Create a new image with the given dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            width,
            height,
            data: vec![0; size],
        }
    }

    /// Create an image filled with a single color.
    #[must_use]
    pub fn filled(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        let mut data = Vec::with_capacity(size);
        for _ in 0..(width * height) {
            data.extend_from_slice(&[r, g, b, a]);
        }
        Self {
            width,
            height,
            data,
        }
    }

    /// Get raw bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get pixel at position.
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = ((y * self.width + x) * 4) as usize;
        Some([
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ])
    }

    /// Set pixel at position.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.data[idx..idx + 4].copy_from_slice(&rgba);
        }
    }
}

/// Snapshot comparison utilities.
pub struct Snapshot;

impl Snapshot {
    /// Compare an actual image against a baseline snapshot.
    ///
    /// # Panics
    ///
    /// Panics if the difference exceeds the threshold.
    pub fn assert_match(name: &str, actual: &Image, threshold: f64) {
        let baseline_path = Self::baseline_path(name);

        if let Some(baseline) = Self::load_baseline(&baseline_path) {
            let diff_ratio = Self::diff(&baseline, actual);

            if diff_ratio > threshold {
                // Save actual and diff for debugging
                let actual_path = Self::actual_path(name);
                let diff_path = Self::diff_path(name);

                Self::save_image(&actual_path, actual);
                Self::save_diff(&diff_path, &baseline, actual);

                panic!(
                    "Visual regression '{}': {:.2}% diff (threshold: {:.2}%)\n\
                     Baseline: {}\n\
                     Actual: {}\n\
                     Diff: {}",
                    name,
                    diff_ratio * 100.0,
                    threshold * 100.0,
                    baseline_path.display(),
                    actual_path.display(),
                    diff_path.display()
                );
            }
        } else if std::env::var("SNAPSHOT_UPDATE").is_ok() {
            // Create new baseline
            Self::save_image(&baseline_path, actual);
            println!("Created new baseline: {}", baseline_path.display());
        } else {
            panic!(
                "No baseline found for '{}'. Run with SNAPSHOT_UPDATE=1 to create.\n\
                 Expected path: {}",
                name,
                baseline_path.display()
            );
        }
    }

    /// Calculate difference ratio between two images.
    #[must_use]
    pub fn diff(a: &Image, b: &Image) -> f64 {
        if a.width != b.width || a.height != b.height {
            return 1.0; // Completely different if sizes don't match
        }

        let mut diff_count = 0u64;
        let total = a.data.len() as u64;

        for (a_byte, b_byte) in a.data.iter().zip(b.data.iter()) {
            if a_byte != b_byte {
                diff_count += 1;
            }
        }

        diff_count as f64 / total as f64
    }

    fn baseline_path(name: &str) -> PathBuf {
        PathBuf::from(format!("tests/snapshots/{name}.png"))
    }

    fn actual_path(name: &str) -> PathBuf {
        PathBuf::from(format!("tests/snapshots/{name}.actual.png"))
    }

    fn diff_path(name: &str) -> PathBuf {
        PathBuf::from(format!("tests/snapshots/{name}.diff.png"))
    }

    fn load_baseline(path: &Path) -> Option<Image> {
        // Simplified - would use a pure Rust PNG decoder
        if path.exists() {
            // Return placeholder
            Some(Image::new(100, 100))
        } else {
            None
        }
    }

    fn save_image(_path: &Path, _image: &Image) {
        // Would use a pure Rust PNG encoder
        // Placeholder implementation
    }

    fn save_diff(_path: &Path, _baseline: &Image, _actual: &Image) {
        // Would generate a visual diff image
        // Placeholder implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_new() {
        let img = Image::new(100, 100);
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
        assert_eq!(img.data.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_image_filled() {
        let img = Image::filled(10, 10, 255, 0, 0, 255);
        assert_eq!(img.get_pixel(0, 0), Some([255, 0, 0, 255]));
        assert_eq!(img.get_pixel(5, 5), Some([255, 0, 0, 255]));
    }

    #[test]
    fn test_image_get_set_pixel() {
        let mut img = Image::new(10, 10);
        img.set_pixel(5, 5, [255, 128, 64, 255]);
        assert_eq!(img.get_pixel(5, 5), Some([255, 128, 64, 255]));
    }

    #[test]
    fn test_image_get_pixel_out_of_bounds() {
        let img = Image::new(10, 10);
        assert_eq!(img.get_pixel(100, 100), None);
    }

    #[test]
    fn test_diff_identical() {
        let a = Image::filled(10, 10, 255, 0, 0, 255);
        let b = Image::filled(10, 10, 255, 0, 0, 255);
        assert_eq!(Snapshot::diff(&a, &b), 0.0);
    }

    #[test]
    fn test_diff_completely_different() {
        let a = Image::filled(10, 10, 255, 0, 0, 255);
        let b = Image::filled(10, 10, 0, 255, 0, 255);
        let diff = Snapshot::diff(&a, &b);
        assert!(diff > 0.0);
    }

    #[test]
    fn test_diff_different_sizes() {
        let a = Image::new(10, 10);
        let b = Image::new(20, 20);
        assert_eq!(Snapshot::diff(&a, &b), 1.0);
    }

    #[test]
    fn test_diff_partial() {
        let a = Image::filled(10, 10, 255, 0, 0, 255);
        let mut b = Image::filled(10, 10, 255, 0, 0, 255);

        // Change one pixel from [255,0,0,255] to [0,0,0,255]
        // Only R channel changes (255 -> 0)
        b.set_pixel(0, 0, [0, 0, 0, 255]);

        let diff = Snapshot::diff(&a, &b);
        // 1 byte changed out of 400 total bytes = 0.0025
        assert!((diff - 0.0025).abs() < 0.001);
    }
}
