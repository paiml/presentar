//! Color representation with WCAG contrast calculations.

use serde::{Deserialize, Serialize};

/// RGBA color with values in the range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    /// Red component [0.0, 1.0]
    pub r: f32,
    /// Green component [0.0, 1.0]
    pub g: f32,
    /// Blue component [0.0, 1.0]
    pub b: f32,
    /// Alpha component [0.0, 1.0]
    pub a: f32,
}

impl Color {
    /// Create a new color, clamping values to [0.0, 1.0].
    #[must_use]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }

    /// Create an opaque color from RGB values.
    #[must_use]
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Parse a hex color string (e.g., "#ff0000" or "ff0000").
    ///
    /// Supports 6-character RGB and 8-character RGBA formats.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid hex color.
    pub fn from_hex(hex: &str) -> Result<Self, ColorParseError> {
        let hex = hex.trim_start_matches('#');

        match hex.len() {
            6 => {
                let r =
                    u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError::InvalidHex)?;
                let g =
                    u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError::InvalidHex)?;
                let b =
                    u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError::InvalidHex)?;
                Ok(Self::rgb(
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                ))
            }
            8 => {
                let r =
                    u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError::InvalidHex)?;
                let g =
                    u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError::InvalidHex)?;
                let b =
                    u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError::InvalidHex)?;
                let a =
                    u8::from_str_radix(&hex[6..8], 16).map_err(|_| ColorParseError::InvalidHex)?;
                Ok(Self::new(
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    f32::from(a) / 255.0,
                ))
            }
            _ => Err(ColorParseError::InvalidLength),
        }
    }

    /// Convert to hex string (RGB only).
    #[must_use]
    pub fn to_hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8
        )
    }

    /// Convert to hex string with alpha.
    #[must_use]
    pub fn to_hex_with_alpha(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            (self.a * 255.0).round() as u8
        )
    }

    /// Calculate relative luminance per WCAG 2.1.
    ///
    /// See: <https://www.w3.org/TR/WCAG21/#dfn-relative-luminance>
    #[must_use]
    pub fn relative_luminance(&self) -> f32 {
        let r = Self::linearize(self.r);
        let g = Self::linearize(self.g);
        let b = Self::linearize(self.b);

        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Calculate contrast ratio between two colors per WCAG 2.1.
    ///
    /// Returns a value between 1.0 (no contrast) and 21.0 (maximum contrast).
    ///
    /// See: <https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio>
    #[must_use]
    pub fn contrast_ratio(&self, other: &Self) -> f32 {
        let l1 = self.relative_luminance();
        let l2 = other.relative_luminance();

        let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };

        (lighter + 0.05) / (darker + 0.05)
    }

    /// Linear interpolation between two colors.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    /// Linearize sRGB component for luminance calculation.
    fn linearize(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    // Common colors
    /// Black color
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    /// White color
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    /// Transparent color
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Error type for color parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorParseError {
    /// Invalid hex characters
    InvalidHex,
    /// Invalid string length
    InvalidLength,
}

impl std::fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHex => write!(f, "invalid hex characters"),
            Self::InvalidLength => write!(f, "invalid hex string length (expected 6 or 8)"),
        }
    }
}

impl std::error::Error for ColorParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK.r, 0.0);
        assert_eq!(Color::WHITE.r, 1.0);
        assert_eq!(Color::TRANSPARENT.a, 0.0);
    }

    #[test]
    fn test_color_default() {
        let c = Color::default();
        assert_eq!(c, Color::BLACK);
    }

    #[test]
    fn test_color_parse_error_display() {
        assert_eq!(
            ColorParseError::InvalidHex.to_string(),
            "invalid hex characters"
        );
        assert_eq!(
            ColorParseError::InvalidLength.to_string(),
            "invalid hex string length (expected 6 or 8)"
        );
    }
}
