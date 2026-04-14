use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn pad_str(s: &str, width: usize) -> String {
    let actual = display_width(s);
    if actual >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - actual))
    }
}

pub fn truncate_str(s: &str, max_width: usize) -> String {
    let actual = display_width(s);
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
