//! URL pattern expansion
//!
//! Expands URL patterns like `[xx-yy]` into multiple URLs.
//! Supports zero-padding based on the input format.
//!
//! # Examples
//!
//! ```ignore
//! use ggg::util::url_expansion::expand_url;
//!
//! // Without padding
//! let urls = expand_url("https://foo/bar[9-11].jpg");
//! // ["https://foo/bar9.jpg", "https://foo/bar10.jpg", "https://foo/bar11.jpg"]
//!
//! // With zero-padding
//! let urls = expand_url("https://foo/bar[009-011].jpg");
//! // ["https://foo/bar009.jpg", "https://foo/bar010.jpg", "https://foo/bar011.jpg"]
//! ```

use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for matching `[start-end]` range patterns
static RANGE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(\d+)-(\d+)\]").expect("Invalid regex pattern")
});

/// Represents a parsed range pattern
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangePattern {
    /// Start value of the range
    pub start: u64,
    /// End value of the range (inclusive)
    pub end: u64,
    /// Width for zero-padding (0 means no padding)
    pub width: usize,
    /// Full match text (e.g., "[009-011]")
    pub full_match: String,
}

impl RangePattern {
    /// Format a number according to this pattern's padding
    pub fn format(&self, value: u64) -> String {
        if self.width > 0 {
            format!("{:0>width$}", value, width = self.width)
        } else {
            value.to_string()
        }
    }

    /// Get the count of values in this range
    pub fn count(&self) -> u64 {
        if self.end >= self.start {
            self.end - self.start + 1
        } else {
            0
        }
    }
}

/// Parse a range pattern from a URL
///
/// Returns the first found pattern, or None if no pattern found.
pub fn parse_range_pattern(url: &str) -> Option<RangePattern> {
    let captures = RANGE_PATTERN.captures(url)?;

    let full_match = captures.get(0)?.as_str().to_string();
    let start_str = captures.get(1)?.as_str();
    let end_str = captures.get(2)?.as_str();

    let start: u64 = start_str.parse().ok()?;
    let end: u64 = end_str.parse().ok()?;

    // Determine padding width from the start value's string length
    // Only apply padding if the start value has leading zeros
    let width = if start_str.starts_with('0') && start_str.len() > 1 {
        start_str.len()
    } else {
        0
    };

    Some(RangePattern {
        start,
        end,
        width,
        full_match,
    })
}

/// Expand a URL containing a range pattern into multiple URLs
///
/// Returns a vector of expanded URLs. If no pattern is found, returns
/// a vector containing only the original URL.
///
/// # Arguments
///
/// * `url` - URL potentially containing a `[start-end]` pattern
///
/// # Returns
///
/// Vector of expanded URLs, or single-element vector with original URL if no pattern found.
///
/// # Limits
///
/// - Maximum 1000 URLs per expansion to prevent memory issues
/// - Patterns where end < start return empty vector (invalid range)
pub fn expand_url(url: &str) -> Vec<String> {
    const MAX_EXPANSION: u64 = 1000;

    let pattern = match parse_range_pattern(url) {
        Some(p) => p,
        None => return vec![url.to_string()],
    };

    // Validate range
    if pattern.end < pattern.start {
        tracing::warn!(
            "Invalid URL range pattern: end ({}) < start ({})",
            pattern.end,
            pattern.start
        );
        return vec![];
    }

    // Check expansion limit
    let count = pattern.count();
    if count > MAX_EXPANSION {
        tracing::warn!(
            "URL range pattern too large: {} URLs (max {})",
            count,
            MAX_EXPANSION
        );
        return vec![];
    }

    // Expand URLs
    (pattern.start..=pattern.end)
        .map(|n| url.replace(&pattern.full_match, &pattern.format(n)))
        .collect()
}

/// Check if a URL contains an expandable pattern
pub fn has_range_pattern(url: &str) -> bool {
    RANGE_PATTERN.is_match(url)
}

/// Get the count of URLs that would be generated from expansion
pub fn expansion_count(url: &str) -> usize {
    match parse_range_pattern(url) {
        Some(pattern) if pattern.end >= pattern.start => pattern.count() as usize,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_pattern_simple() {
        let pattern = parse_range_pattern("https://foo/bar[9-11].jpg").unwrap();
        assert_eq!(pattern.start, 9);
        assert_eq!(pattern.end, 11);
        assert_eq!(pattern.width, 0);
        assert_eq!(pattern.full_match, "[9-11]");
    }

    #[test]
    fn test_parse_range_pattern_padded() {
        let pattern = parse_range_pattern("https://foo/bar[009-011].jpg").unwrap();
        assert_eq!(pattern.start, 9);
        assert_eq!(pattern.end, 11);
        assert_eq!(pattern.width, 3);
        assert_eq!(pattern.full_match, "[009-011]");
    }

    #[test]
    fn test_parse_range_pattern_none() {
        assert!(parse_range_pattern("https://foo/bar.jpg").is_none());
        assert!(parse_range_pattern("https://foo/bar[abc].jpg").is_none());
    }

    #[test]
    fn test_expand_url_simple() {
        let urls = expand_url("https://foo/bar[9-11].jpg");
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://foo/bar9.jpg");
        assert_eq!(urls[1], "https://foo/bar10.jpg");
        assert_eq!(urls[2], "https://foo/bar11.jpg");
    }

    #[test]
    fn test_expand_url_padded() {
        let urls = expand_url("https://foo/bar[009-011].jpg");
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://foo/bar009.jpg");
        assert_eq!(urls[1], "https://foo/bar010.jpg");
        assert_eq!(urls[2], "https://foo/bar011.jpg");
    }

    #[test]
    fn test_expand_url_single() {
        let urls = expand_url("https://foo/bar[5-5].jpg");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://foo/bar5.jpg");
    }

    #[test]
    fn test_expand_url_no_pattern() {
        let urls = expand_url("https://foo/bar.jpg");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://foo/bar.jpg");
    }

    #[test]
    fn test_expand_url_invalid_range() {
        let urls = expand_url("https://foo/bar[11-9].jpg");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_expand_url_larger_padding() {
        let urls = expand_url("https://foo/bar[0001-0003].jpg");
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://foo/bar0001.jpg");
        assert_eq!(urls[1], "https://foo/bar0002.jpg");
        assert_eq!(urls[2], "https://foo/bar0003.jpg");
    }

    #[test]
    fn test_has_range_pattern() {
        assert!(has_range_pattern("https://foo/[1-10].jpg"));
        assert!(!has_range_pattern("https://foo/bar.jpg"));
    }

    #[test]
    fn test_expansion_count() {
        assert_eq!(expansion_count("https://foo/[1-10].jpg"), 10);
        assert_eq!(expansion_count("https://foo/bar.jpg"), 1);
        assert_eq!(expansion_count("https://foo/[5-5].jpg"), 1);
    }

    #[test]
    fn test_format_padding() {
        let pattern = RangePattern {
            start: 1,
            end: 100,
            width: 4,
            full_match: "[0001-0100]".to_string(),
        };
        assert_eq!(pattern.format(1), "0001");
        assert_eq!(pattern.format(10), "0010");
        assert_eq!(pattern.format(100), "0100");
    }
}
