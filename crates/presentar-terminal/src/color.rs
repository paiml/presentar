//! Color mode detection and conversion for terminals.

use crossterm::style::Color as CrosstermColor;
use presentar_core::Color;

/// Terminal color capability mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// 24-bit true color (COLORTERM=truecolor or 24bit).
    #[default]
    TrueColor,
    /// 256 color palette.
    Color256,
    /// 16 ANSI colors.
    Color16,
    /// Monochrome (no color).
    Mono,
}

impl ColorMode {
    /// Auto-detect terminal color capabilities.
    #[must_use]
    pub fn detect() -> Self {
        // Check COLORTERM first (most reliable)
        if let Ok("truecolor" | "24bit") = std::env::var("COLORTERM").as_deref() {
            return Self::TrueColor;
        }

        // Fall back to TERM
        match std::env::var("TERM").as_deref() {
            Ok(t) if t.contains("256color") => Self::Color256,
            Ok(t) if t.contains("color") || t.contains("xterm") => Self::Color16,
            Ok("dumb") | Err(_) => Self::Mono,
            _ => Self::Color16,
        }
    }

    /// Convert a presentar Color to crossterm Color based on this mode.
    #[must_use]
    pub fn to_crossterm(&self, color: Color) -> CrosstermColor {
        let r = (color.r * 255.0).round() as u8;
        let g = (color.g * 255.0).round() as u8;
        let b = (color.b * 255.0).round() as u8;

        match self {
            Self::TrueColor => CrosstermColor::Rgb { r, g, b },
            Self::Color256 => CrosstermColor::AnsiValue(Self::rgb_to_256(r, g, b)),
            Self::Color16 => Self::rgb_to_16(r, g, b),
            Self::Mono => CrosstermColor::White,
        }
    }

    /// Convert RGB to 256-color palette index.
    fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
        // Check for grayscale (r == g == b)
        if r == g && g == b {
            if r < 8 {
                return 16; // black
            }
            if r > 248 {
                return 231; // white
            }
            // Grayscale ramp: colors 232-255 (24 shades)
            return 232 + ((r - 8) / 10).min(23);
        }

        // 6x6x6 color cube (colors 16-231)
        let r_idx = (u16::from(r) * 5 / 255) as u8;
        let g_idx = (u16::from(g) * 5 / 255) as u8;
        let b_idx = (u16::from(b) * 5 / 255) as u8;
        16 + 36 * r_idx + 6 * g_idx + b_idx
    }

    /// Convert RGB to 16-color ANSI.
    fn rgb_to_16(r: u8, g: u8, b: u8) -> CrosstermColor {
        let luminance = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
        let bright = luminance > 127;

        let max = r.max(g).max(b);
        let threshold = max / 2;

        let has_r = r > threshold;
        let has_g = g > threshold;
        let has_b = b > threshold;

        match (has_r, has_g, has_b, bright) {
            (false, false, false, false) => CrosstermColor::Black,
            (false, false, false, true) => CrosstermColor::DarkGrey,
            (true, false, false, false) => CrosstermColor::DarkRed,
            (true, false, false, true) => CrosstermColor::Red,
            (false, true, false, false) => CrosstermColor::DarkGreen,
            (false, true, false, true) => CrosstermColor::Green,
            (true, true, false, false) => CrosstermColor::DarkYellow,
            (true, true, false, true) => CrosstermColor::Yellow,
            (false, false, true, false) => CrosstermColor::DarkBlue,
            (false, false, true, true) => CrosstermColor::Blue,
            (true, false, true, false) => CrosstermColor::DarkMagenta,
            (true, false, true, true) => CrosstermColor::Magenta,
            (false, true, true, false) => CrosstermColor::DarkCyan,
            (false, true, true, true) => CrosstermColor::Cyan,
            (true, true, true, false) => CrosstermColor::Grey,
            (true, true, true, true) => CrosstermColor::White,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_mode_default() {
        assert_eq!(ColorMode::default(), ColorMode::TrueColor);
    }

    #[test]
    fn test_truecolor_conversion() {
        let mode = ColorMode::TrueColor;
        let color = Color::new(0.5, 0.25, 0.75, 1.0);
        let result = mode.to_crossterm(color);
        assert_eq!(
            result,
            CrosstermColor::Rgb {
                r: 128,
                g: 64,
                b: 191
            }
        );
    }

    #[test]
    fn test_256_grayscale() {
        // Pure black
        assert_eq!(ColorMode::rgb_to_256(0, 0, 0), 16);
        // Pure white
        assert_eq!(ColorMode::rgb_to_256(255, 255, 255), 231);
        // Mid gray
        let mid = ColorMode::rgb_to_256(128, 128, 128);
        assert!(mid >= 232);
    }

    #[test]
    fn test_256_grayscale_near_black() {
        // Very dark gray (still mapped to black)
        assert_eq!(ColorMode::rgb_to_256(5, 5, 5), 16);
    }

    #[test]
    fn test_256_grayscale_ramp() {
        // Test various gray values
        let gray50 = ColorMode::rgb_to_256(50, 50, 50);
        assert!(gray50 >= 232);

        let gray100 = ColorMode::rgb_to_256(100, 100, 100);
        assert!(gray100 >= 232);

        let gray200 = ColorMode::rgb_to_256(200, 200, 200);
        assert!(gray200 >= 232);
    }

    #[test]
    fn test_256_color_cube() {
        // Pure red (should be in color cube)
        let red = ColorMode::rgb_to_256(255, 0, 0);
        assert!(red >= 16 && red <= 231);

        // Pure green
        let green = ColorMode::rgb_to_256(0, 255, 0);
        assert!(green >= 16 && green <= 231);

        // Pure blue
        let blue = ColorMode::rgb_to_256(0, 0, 255);
        assert!(blue >= 16 && blue <= 231);

        // Magenta
        let magenta = ColorMode::rgb_to_256(255, 0, 255);
        assert!(magenta >= 16 && magenta <= 231);
    }

    #[test]
    fn test_16_color_mapping() {
        // Black
        assert_eq!(ColorMode::rgb_to_16(0, 0, 0), CrosstermColor::Black);
        // White
        assert_eq!(ColorMode::rgb_to_16(255, 255, 255), CrosstermColor::White);
        // Pure red
        assert!(matches!(
            ColorMode::rgb_to_16(255, 0, 0),
            CrosstermColor::Red | CrosstermColor::DarkRed
        ));
    }

    #[test]
    fn test_16_color_green() {
        let result = ColorMode::rgb_to_16(0, 255, 0);
        assert!(matches!(
            result,
            CrosstermColor::Green | CrosstermColor::DarkGreen
        ));
    }

    #[test]
    fn test_16_color_blue() {
        let result = ColorMode::rgb_to_16(0, 0, 255);
        assert!(matches!(
            result,
            CrosstermColor::Blue | CrosstermColor::DarkBlue
        ));
    }

    #[test]
    fn test_16_color_yellow() {
        let result = ColorMode::rgb_to_16(255, 255, 0);
        assert!(matches!(
            result,
            CrosstermColor::Yellow | CrosstermColor::DarkYellow
        ));
    }

    #[test]
    fn test_16_color_cyan() {
        let result = ColorMode::rgb_to_16(0, 255, 255);
        assert!(matches!(
            result,
            CrosstermColor::Cyan | CrosstermColor::DarkCyan
        ));
    }

    #[test]
    fn test_16_color_magenta() {
        let result = ColorMode::rgb_to_16(255, 0, 255);
        assert!(matches!(
            result,
            CrosstermColor::Magenta | CrosstermColor::DarkMagenta
        ));
    }

    #[test]
    fn test_16_color_dark_gray() {
        let result = ColorMode::rgb_to_16(50, 50, 50);
        // With the luminance-based algorithm, dark gray maps to the dark variants
        assert!(matches!(
            result,
            CrosstermColor::Black | CrosstermColor::DarkGrey | CrosstermColor::Grey
        ));
    }

    #[test]
    fn test_16_color_gray() {
        let result = ColorMode::rgb_to_16(192, 192, 192);
        assert!(matches!(
            result,
            CrosstermColor::Grey | CrosstermColor::White
        ));
    }

    #[test]
    fn test_mono_conversion() {
        let mode = ColorMode::Mono;
        assert_eq!(mode.to_crossterm(Color::RED), CrosstermColor::White);
        assert_eq!(mode.to_crossterm(Color::BLUE), CrosstermColor::White);
        assert_eq!(mode.to_crossterm(Color::GREEN), CrosstermColor::White);
    }

    #[test]
    fn test_256_conversion() {
        let mode = ColorMode::Color256;
        let result = mode.to_crossterm(Color::new(0.5, 0.25, 0.75, 1.0));
        assert!(matches!(result, CrosstermColor::AnsiValue(_)));
    }

    #[test]
    fn test_16_conversion() {
        let mode = ColorMode::Color16;
        let result = mode.to_crossterm(Color::RED);
        assert!(matches!(
            result,
            CrosstermColor::Red | CrosstermColor::DarkRed
        ));
    }

    #[test]
    fn test_color_mode_eq() {
        assert_eq!(ColorMode::TrueColor, ColorMode::TrueColor);
        assert_eq!(ColorMode::Color256, ColorMode::Color256);
        assert_eq!(ColorMode::Color16, ColorMode::Color16);
        assert_eq!(ColorMode::Mono, ColorMode::Mono);
        assert_ne!(ColorMode::TrueColor, ColorMode::Color256);
    }

    #[test]
    fn test_color_mode_clone() {
        let mode = ColorMode::Color256;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_color_mode_debug() {
        let mode = ColorMode::TrueColor;
        assert!(format!("{:?}", mode).contains("TrueColor"));
    }

    #[test]
    fn test_16_color_dim_colors() {
        // Dark red (dim)
        let dark_red = ColorMode::rgb_to_16(128, 0, 0);
        assert!(matches!(
            dark_red,
            CrosstermColor::Red | CrosstermColor::DarkRed
        ));

        // Dark green (dim)
        let dark_green = ColorMode::rgb_to_16(0, 100, 0);
        assert!(matches!(
            dark_green,
            CrosstermColor::Green | CrosstermColor::DarkGreen
        ));

        // Dark blue (dim)
        let dark_blue = ColorMode::rgb_to_16(0, 0, 128);
        assert!(matches!(
            dark_blue,
            CrosstermColor::Blue | CrosstermColor::DarkBlue
        ));
    }

    #[test]
    fn test_256_near_white() {
        // Near white (should be in grayscale ramp, near 231)
        let near_white = ColorMode::rgb_to_256(250, 250, 250);
        assert_eq!(near_white, 231);
    }

    #[test]
    fn test_256_mixed_colors() {
        // Orange-ish
        let orange = ColorMode::rgb_to_256(255, 128, 0);
        assert!(orange >= 16 && orange <= 231);

        // Purple-ish
        let purple = ColorMode::rgb_to_256(128, 0, 255);
        assert!(purple >= 16 && purple <= 231);

        // Teal-ish
        let teal = ColorMode::rgb_to_256(0, 128, 128);
        assert!(teal >= 16 && teal <= 231);
    }
}
