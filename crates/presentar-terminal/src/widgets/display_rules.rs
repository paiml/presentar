//! Display Rules - Grammar of Graphics formatting primitives for TUI
//!
//! SPEC-024 Section 28: Display Rules Framework
//!
//! All text and numbers in a TUI MUST follow consistent display rules:
//!
//! 1. **Numbers**: Use human-readable units (1.2G not 1234567890)
//! 2. **Truncation**: Smart truncation preserving meaningful content
//! 3. **Columns**: Width-aware formatting that never bleeds
//! 4. **Search**: O(1) fuzzy matching with relevance scoring
//!
//! # Performance Targets (from pzsh/aprender-shell patterns)
//! - Search: <1ms for 5000 items
//! - Format: <100µs per cell
//! - Truncate: <10µs per string

use std::borrow::Cow;

// =============================================================================
// BYTE FORMATTING (1000 vs 1024 base)
// =============================================================================

/// Format bytes with SI units (1000-based: KB, MB, GB)
///
/// # Examples
/// ```ignore
/// assert_eq!(format_bytes_si(1500), "1.5K");
/// assert_eq!(format_bytes_si(1_500_000), "1.5M");
/// assert_eq!(format_bytes_si(1_500_000_000), "1.5G");
/// ```
#[must_use]
pub fn format_bytes_si(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T", "P"];
    const BASE: f64 = 1000.0;

    if bytes == 0 {
        return "0B".to_string();
    }

    let bytes_f = bytes as f64;
    let exp = (bytes_f.log10() / BASE.log10()).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);
    let value = bytes_f / BASE.powi(exp as i32);

    if exp == 0 {
        format!("{bytes}B")
    } else if value >= 100.0 {
        format!("{:.0}{}", value, UNITS[exp])
    } else if value >= 10.0 {
        format!("{:.1}{}", value, UNITS[exp])
    } else {
        format!("{:.2}{}", value, UNITS[exp])
    }
}

/// Format bytes with IEC units (1024-based: KiB, MiB, GiB)
#[must_use]
pub fn format_bytes_iec(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "Ki", "Mi", "Gi", "Ti", "Pi"];
    const BASE: f64 = 1024.0;

    if bytes == 0 {
        return "0B".to_string();
    }

    let bytes_f = bytes as f64;
    let exp = (bytes_f.log2() / 10.0).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);
    let value = bytes_f / BASE.powi(exp as i32);

    if exp == 0 {
        format!("{bytes}B")
    } else if value >= 100.0 {
        format!("{:.0}{}", value, UNITS[exp])
    } else if value >= 10.0 {
        format!("{:.1}{}", value, UNITS[exp])
    } else {
        format!("{:.2}{}", value, UNITS[exp])
    }
}

/// Format bytes/second as transfer rate
#[must_use]
pub fn format_rate(bytes_per_sec: u64) -> String {
    format!("{}/s", format_bytes_si(bytes_per_sec))
}

// =============================================================================
// PERCENTAGE FORMATTING
// =============================================================================

/// Format percentage with smart precision
///
/// - 0-9.99%: 1 decimal (e.g., "5.2%")
/// - 10-99.9%: 1 decimal (e.g., "45.3%")
/// - 100%+: 0 decimal (e.g., "153%")
#[must_use]
pub fn format_percent(value: f32) -> String {
    if value >= 100.0 {
        format!("{value:.0}%")
    } else if value >= 10.0 {
        format!("{value:.1}%")
    } else if value >= 0.1 {
        format!("{value:.1}%")
    } else if value > 0.0 {
        format!("{value:.2}%")
    } else {
        "0%".to_string()
    }
}

/// Format percentage clamped to 0-100 range
#[must_use]
pub fn format_percent_clamped(value: f32) -> String {
    format_percent(value.clamp(0.0, 100.0))
}

/// Format percentage with fixed width (right-aligned)
#[must_use]
pub fn format_percent_fixed(value: f32, width: usize) -> String {
    let s = format_percent(value);
    if s.len() >= width {
        s
    } else {
        format!("{s:>width$}")
    }
}

// =============================================================================
// TIME/DURATION FORMATTING
// =============================================================================

/// Format duration in human-readable form
///
/// - <60s: "45s"
/// - <60m: "5m 30s" or "5:30"
/// - <24h: "3h 15m" or "3:15:00"
/// - ≥24h: "2d 5h" or just "2d"
#[must_use]
pub fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{m}m")
        } else {
            format!("{m}:{s:02}")
        }
    } else if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        if s == 0 && m == 0 {
            format!("{h}h")
        } else if s == 0 {
            format!("{h}:{m:02}")
        } else {
            format!("{h}:{m:02}:{s:02}")
        }
    } else {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        if h == 0 {
            format!("{d}d")
        } else {
            format!("{d}d {h}h")
        }
    }
}

/// Format duration compact (for tight columns)
#[must_use]
pub fn format_duration_compact(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

// =============================================================================
// FREQUENCY FORMATTING
// =============================================================================

/// Format frequency (MHz to GHz conversion)
#[must_use]
pub fn format_freq_mhz(mhz: u64) -> String {
    if mhz >= 1000 {
        let ghz = mhz as f64 / 1000.0;
        if ghz >= 10.0 {
            format!("{ghz:.1}G")
        } else {
            format!("{ghz:.2}G")
        }
    } else {
        format!("{mhz}M")
    }
}

// =============================================================================
// TEMPERATURE FORMATTING
// =============================================================================

/// Format temperature with unit
#[must_use]
pub fn format_temp_c(celsius: f32) -> String {
    if celsius >= 100.0 {
        format!("{celsius:.0}°C")
    } else {
        format!("{celsius:.1}°C")
    }
}

// =============================================================================
// TEXT TRUNCATION STRATEGIES
// =============================================================================

/// Truncation strategy
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TruncateStrategy {
    /// Truncate from end: "`long_text`..." → "long_..."
    #[default]
    End,
    /// Truncate from start: "`long_text`..." → "..._text"
    Start,
    /// Truncate from middle: "`long_text_here`" → "long_...here"
    Middle,
    /// Path-aware: "/home/user/very/long/path" → "/home/.../path"
    Path,
    /// Command-aware: "opt 9i94wsqoafn" → "opt …afn" (keep prefix + suffix)
    Command,
}

/// Truncate string to fit within width using specified strategy
///
/// # Arguments
/// * `s` - Input string
/// * `width` - Maximum width in characters
/// * `strategy` - Truncation strategy
///
/// # Returns
/// Truncated string with ellipsis if needed
#[must_use]
pub fn truncate(s: &str, width: usize, strategy: TruncateStrategy) -> Cow<'_, str> {
    let char_count = s.chars().count();

    if char_count <= width {
        return Cow::Borrowed(s);
    }

    if width <= 3 {
        return Cow::Owned("…".repeat(width.min(1)));
    }

    match strategy {
        TruncateStrategy::End => {
            let take = width - 1; // Leave room for ellipsis
            let truncated: String = s.chars().take(take).collect();
            Cow::Owned(format!("{truncated}…"))
        }
        TruncateStrategy::Start => {
            let skip = char_count - width + 1;
            let truncated: String = s.chars().skip(skip).collect();
            Cow::Owned(format!("…{truncated}"))
        }
        TruncateStrategy::Middle => {
            let half = (width - 1) / 2;
            let start: String = s.chars().take(half).collect();
            let end: String = s.chars().skip(char_count - half).collect();
            Cow::Owned(format!("{start}…{end}"))
        }
        TruncateStrategy::Path => truncate_path(s, width),
        TruncateStrategy::Command => truncate_command(s, width),
    }
}

/// Path-aware truncation: "/home/user/very/long/path" → "/home/.../path"
fn truncate_path(path: &str, width: usize) -> Cow<'_, str> {
    if path.chars().count() <= width {
        return Cow::Borrowed(path);
    }

    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return Cow::Borrowed(path);
    }

    if parts.len() == 1 {
        return truncate(path, width, TruncateStrategy::End);
    }

    // Keep first and last parts, replace middle with ...
    let first = parts.first().unwrap_or(&"");
    let last = parts.last().unwrap_or(&"");

    let prefix = if path.starts_with('/') { "/" } else { "" };
    let result = format!("{prefix}{first}/…/{last}");

    // If still too long, truncate the result
    if result.chars().count() > width {
        let truncated: String = result.chars().take(width - 1).collect();
        Cow::Owned(format!("{truncated}…"))
    } else {
        Cow::Owned(result)
    }
}

/// Command-aware truncation with middle ellipsis
///
/// Preserves the front (executable + start of args) and end (identifiers/flags),
/// truncating the middle. This is optimal for commands where the beginning shows
/// what's running and the end shows important identifiers like PIDs, ports, paths.
///
/// # Examples
/// - `firefox -contentproc -parentBuildID 20240101 -childID 5 -isForBrowser`
///   → `firefox -contentproc…-childID 5 -isForBrowser` (width=40)
/// - `python /home/user/scripts/very_long_path/script.py --arg=value`
///   → `python /home/…--arg=value` (width=25)
fn truncate_command(cmd: &str, width: usize) -> Cow<'_, str> {
    let char_count = cmd.chars().count();

    if char_count <= width {
        return Cow::Borrowed(cmd);
    }

    if width <= 3 {
        return Cow::Owned("…".repeat(width.min(1)));
    }

    // For very short widths, just do end truncation
    if width < 10 {
        let take = width - 1;
        let truncated: String = cmd.chars().take(take).collect();
        return Cow::Owned(format!("{truncated}…"));
    }

    // Middle truncation: split width between front and back
    // Give slightly more to the front (executable name is important)
    // and ensure the back preserves identifiers/flags
    let ellipsis_len = 1; // "…"
    let available = width - ellipsis_len;

    // 60% front, 40% back - front has executable, back has identifiers
    let front_chars = (available * 3) / 5;
    let back_chars = available - front_chars;

    let chars: Vec<char> = cmd.chars().collect();

    // Take front portion
    let front: String = chars.iter().take(front_chars).collect();

    // Take back portion
    let back: String = chars.iter().skip(char_count - back_chars).collect();

    Cow::Owned(format!("{front}…{back}"))
}

// =============================================================================
// COLUMN FORMATTING (PREVENTS BLEEDING)
// =============================================================================

/// Column alignment
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColumnAlign {
    #[default]
    Left,
    Right,
    Center,
}

/// Format a value to fit exactly within a column width
///
/// **GUARANTEE**: Output will NEVER exceed `width` characters
///
/// # Arguments
/// * `value` - The string to format
/// * `width` - Exact column width
/// * `align` - Alignment within column
/// * `truncate_strategy` - How to truncate if too long
#[must_use]
pub fn format_column(
    value: &str,
    width: usize,
    align: ColumnAlign,
    truncate_strategy: TruncateStrategy,
) -> String {
    let truncated = truncate(value, width, truncate_strategy);
    let len = truncated.chars().count();

    if len >= width {
        // Already at or over width, just take exactly width chars
        truncated.chars().take(width).collect()
    } else {
        let padding = width - len;
        match align {
            ColumnAlign::Left => format!("{truncated}{}", " ".repeat(padding)),
            ColumnAlign::Right => format!("{}{truncated}", " ".repeat(padding)),
            ColumnAlign::Center => {
                let left = padding / 2;
                let right = padding - left;
                format!("{}{truncated}{}", " ".repeat(left), " ".repeat(right))
            }
        }
    }
}

/// Format a number column (right-aligned, numeric formatting)
#[must_use]
pub fn format_number_column(value: f64, width: usize, decimals: usize) -> String {
    let formatted = if decimals == 0 {
        format!("{value:.0}")
    } else {
        format!("{value:.decimals$}")
    };
    format_column(&formatted, width, ColumnAlign::Right, TruncateStrategy::End)
}

/// Format a percentage column
#[must_use]
pub fn format_percent_column(value: f32, width: usize) -> String {
    let formatted = format_percent(value);
    format_column(&formatted, width, ColumnAlign::Right, TruncateStrategy::End)
}

/// Format a bytes column
#[must_use]
pub fn format_bytes_column(bytes: u64, width: usize) -> String {
    let formatted = format_bytes_si(bytes);
    format_column(&formatted, width, ColumnAlign::Right, TruncateStrategy::End)
}

// =============================================================================
// O(1) FUZZY SEARCH (from pzsh/aprender-shell patterns)
// =============================================================================

/// Search result with relevance scoring
#[derive(Debug, Clone)]
pub struct SearchResult<T> {
    /// The matched item
    pub item: T,
    /// Relevance score (0.0 - 1.0, higher is better)
    pub score: f32,
    /// Match positions (for highlighting)
    pub matches: Vec<usize>,
}

/// Fast fuzzy search with O(1) amortized lookup via pre-computed index
///
/// Performance targets:
/// - Build: O(n) where n = number of items
/// - Search: O(m * k) where m = query length, k = average results
/// - Memory: O(n * `avg_key_length`)
#[derive(Debug, Clone)]
pub struct FuzzyIndex<T: Clone> {
    /// Items to search
    items: Vec<T>,
    /// Pre-computed lowercase keys for fast comparison
    keys: Vec<String>,
    /// Trigram index for O(1) candidate lookup
    trigrams: std::collections::HashMap<[u8; 3], Vec<usize>>,
    /// Character index for single-char queries
    char_index: std::collections::HashMap<char, Vec<usize>>,
}

impl<T: Clone> FuzzyIndex<T> {
    /// Build a fuzzy search index from items
    ///
    /// # Arguments
    /// * `items` - Items to index
    /// * `key_fn` - Function to extract searchable key from each item
    pub fn new<F>(items: Vec<T>, key_fn: F) -> Self
    where
        F: Fn(&T) -> String,
    {
        let keys: Vec<String> = items
            .iter()
            .map(|item| key_fn(item).to_lowercase())
            .collect();

        let mut trigrams: std::collections::HashMap<[u8; 3], Vec<usize>> =
            std::collections::HashMap::new();
        let mut char_index: std::collections::HashMap<char, Vec<usize>> =
            std::collections::HashMap::new();

        for (idx, key) in keys.iter().enumerate() {
            // Build character index
            for ch in key.chars() {
                char_index.entry(ch).or_default().push(idx);
            }

            // Build trigram index
            let bytes = key.as_bytes();
            if bytes.len() >= 3 {
                for window in bytes.windows(3) {
                    let trigram: [u8; 3] = [window[0], window[1], window[2]];
                    trigrams.entry(trigram).or_default().push(idx);
                }
            }
        }

        // Deduplicate indices
        for indices in trigrams.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
        for indices in char_index.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }

        Self {
            items,
            keys,
            trigrams,
            char_index,
        }
    }

    /// Search for items matching query
    ///
    /// # Arguments
    /// * `query` - Search query (case-insensitive)
    /// * `limit` - Maximum results to return
    ///
    /// # Returns
    /// Sorted results by relevance (highest first)
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult<T>> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let query_chars: Vec<char> = query_lower.chars().collect();

        // Get candidate indices
        let candidates = self.get_candidates(&query_lower);

        // Score and filter candidates
        let mut results: Vec<SearchResult<T>> = candidates
            .into_iter()
            .filter_map(|idx| {
                let key = &self.keys[idx];
                let (score, matches) = self.score_match(key, &query_chars);
                if score > 0.0 {
                    Some(SearchResult {
                        item: self.items[idx].clone(),
                        score,
                        matches,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        results
    }

    /// Get candidate indices using index
    fn get_candidates(&self, query: &str) -> Vec<usize> {
        let bytes = query.as_bytes();

        // Use trigram index if query is long enough
        if bytes.len() >= 3 {
            let trigram: [u8; 3] = [bytes[0], bytes[1], bytes[2]];
            if let Some(indices) = self.trigrams.get(&trigram) {
                return indices.clone();
            }
        }

        // Fall back to character index
        if let Some(first_char) = query.chars().next() {
            if let Some(indices) = self.char_index.get(&first_char) {
                return indices.clone();
            }
        }

        // Fall back to scanning all items
        (0..self.items.len()).collect()
    }

    /// Score a match using fuzzy substring matching
    fn score_match(&self, key: &str, query_chars: &[char]) -> (f32, Vec<usize>) {
        if query_chars.is_empty() {
            return (0.0, Vec::new());
        }

        let key_chars: Vec<char> = key.chars().collect();
        let mut matches = Vec::new();
        let mut query_idx = 0;

        // Find subsequence matches
        for (key_idx, &key_char) in key_chars.iter().enumerate() {
            if query_idx < query_chars.len() && key_char == query_chars[query_idx] {
                matches.push(key_idx);
                query_idx += 1;
            }
        }

        // Must match all query characters
        if query_idx != query_chars.len() {
            return (0.0, Vec::new());
        }

        // Calculate score based on:
        // - Consecutive matches (bonus)
        // - Position of first match (earlier is better)
        // - Match density (matches / key length)

        let mut score = 1.0;

        // Consecutive bonus
        let mut consecutive = 0;
        for i in 1..matches.len() {
            if matches[i] == matches[i - 1] + 1 {
                consecutive += 1;
            }
        }
        score += consecutive as f32 * 0.1;

        // Early match bonus
        if !matches.is_empty() {
            score += (1.0 - matches[0] as f32 / key_chars.len() as f32) * 0.3;
        }

        // Exact prefix bonus
        if key.starts_with(&query_chars.iter().collect::<String>()) {
            score += 0.5;
        }

        // Exact match bonus
        if key_chars.len() == query_chars.len() {
            score += 1.0;
        }

        // Density factor
        score *= query_chars.len() as f32 / key_chars.len() as f32;

        (score.min(1.0), matches)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_si() {
        assert_eq!(format_bytes_si(0), "0B");
        assert_eq!(format_bytes_si(100), "100B");
        assert_eq!(format_bytes_si(1000), "1.00K");
        assert_eq!(format_bytes_si(1500), "1.50K");
        assert_eq!(format_bytes_si(1_000_000), "1.00M");
        assert_eq!(format_bytes_si(1_500_000_000), "1.50G");
        assert_eq!(format_bytes_si(1_000_000_000_000), "1.00T");
    }

    #[test]
    fn test_format_percent() {
        assert_eq!(format_percent(0.0), "0%");
        assert_eq!(format_percent(5.0), "5.0%");
        assert_eq!(format_percent(45.3), "45.3%");
        assert_eq!(format_percent(100.0), "100%");
        assert_eq!(format_percent(153.2), "153%");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(90), "1:30");
        assert_eq!(format_duration(3661), "1:01:01");
        assert_eq!(format_duration(86400), "1d");
        assert_eq!(format_duration(90000), "1d 1h");
    }

    #[test]
    fn test_truncate_end() {
        assert_eq!(truncate("hello", 10, TruncateStrategy::End), "hello");
        assert_eq!(
            truncate("hello world", 8, TruncateStrategy::End),
            "hello w…"
        );
        assert_eq!(truncate("hi", 1, TruncateStrategy::End), "…");
    }

    #[test]
    fn test_truncate_middle() {
        assert_eq!(
            truncate("hello_world_here", 10, TruncateStrategy::Middle),
            "hell…here"
        );
    }

    #[test]
    fn test_truncate_path() {
        assert_eq!(
            truncate("/home/user/documents/file.txt", 20, TruncateStrategy::Path),
            "/home/…/file.txt"
        );
    }

    #[test]
    fn test_truncate_command() {
        // Middle truncation: 60% front, 40% back
        // width=12, available=11, front=6, back=5
        // "opt 9i94wsqoafn" (15 chars) → front 6 + "…" + back 5 = "opt 9i…qoafn"
        assert_eq!(
            truncate("opt 9i94wsqoafn", 12, TruncateStrategy::Command),
            "opt 9i…qoafn"
        );

        // Longer command with middle truncation
        // width=40, available=39, front=23, back=16
        let long_cmd = "firefox -contentproc -parentBuildID 20240101 -childID 5 -isForBrowser";
        let result = truncate(long_cmd, 40, TruncateStrategy::Command);
        assert_eq!(result.chars().count(), 40);
        assert!(result.starts_with("firefox -contentproc"));
        assert!(result.ends_with("isForBrowser"));
        assert!(result.contains('…'));

        // Short width falls back to end truncation
        assert_eq!(
            truncate("command arg1 arg2", 8, TruncateStrategy::Command),
            "command…"
        );
    }

    #[test]
    fn test_format_column_never_bleeds() {
        let result = format_column(
            "very_long_text_that_should_be_truncated",
            10,
            ColumnAlign::Left,
            TruncateStrategy::End,
        );
        assert_eq!(result.chars().count(), 10);

        let result = format_column("short", 10, ColumnAlign::Right, TruncateStrategy::End);
        assert_eq!(result.chars().count(), 10);
        assert!(result.starts_with("     ")); // Right-aligned padding
    }

    #[test]
    fn test_fuzzy_search() {
        let items = vec![
            "firefox".to_string(),
            "thunderbird".to_string(),
            "chrome".to_string(),
            "chromium".to_string(),
            "firefox-developer".to_string(),
        ];

        let index = FuzzyIndex::new(items, |s| s.clone());

        let results = index.search("fire", 5);
        assert!(!results.is_empty());
        assert!(results[0].item.contains("fire"));

        let results = index.search("chro", 5);
        assert!(results.len() >= 2); // chrome and chromium
    }
}
