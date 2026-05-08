// Terminal display-width helpers. Depends on `unicode-width`, which is only
// pulled in for the `cli` feature (TUI/CLI use only).

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Returns `min(display_width(s), max_width)` without scanning past the cap.
pub fn display_width_capped(s: &str, max_width: usize) -> usize {
    if max_width == 0 {
        return 0;
    }

    let mut width = 0usize;
    for c in s.chars() {
        width += UnicodeWidthChar::width(c).unwrap_or(0);
        if width >= max_width {
            return max_width;
        }
    }
    width
}

pub fn pad_str(s: &str, width: usize) -> String {
    let actual = display_width_capped(s, width);
    if actual >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - actual))
    }
}

pub fn truncate_str(s: &str, max_width: usize) -> String {
    let actual = display_width_capped(s, max_width + 1);
    if actual <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let ellipsis = "\u{2026}";
    let ellipsis_width = display_width(ellipsis);
    if max_width <= ellipsis_width {
        return ellipsis.to_string();
    }

    let target = max_width - ellipsis_width;
    let mut width = 0;
    let mut truncated = String::new();
    for c in s.chars() {
        let ch_width = UnicodeWidthChar::width(c).unwrap_or(0);
        if width + ch_width > target {
            break;
        }
        truncated.push(c);
        width += ch_width;
    }
    truncated.push_str(ellipsis);
    truncated
}

pub fn slice_str(s: &str, start_width: usize, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut skipped = 0;
    let mut taken = 0;
    let mut out = String::new();

    for c in s.chars() {
        let ch_width = UnicodeWidthChar::width(c).unwrap_or(0);
        if skipped + ch_width <= start_width {
            skipped += ch_width;
            continue;
        }
        if taken + ch_width > max_width {
            break;
        }
        out.push(c);
        taken += ch_width;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_capped_returns_exact_width_below_cap() {
        assert_eq!(display_width_capped("abc", 10), 3);
        assert_eq!(display_width_capped("a中", 10), 3);
    }

    #[test]
    fn display_width_capped_stops_at_cap() {
        assert_eq!(display_width_capped("abcdef", 3), 3);
        assert_eq!(display_width_capped("中abc", 1), 1);
        assert_eq!(display_width_capped("abc", 0), 0);
    }

    #[test]
    fn truncate_str_does_not_need_full_width_for_long_strings() {
        assert_eq!(truncate_str("abcdef", 4), "abc\u{2026}");
        assert_eq!(truncate_str("abc", 4), "abc");
    }
}
