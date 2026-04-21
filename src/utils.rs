/// Converts a UTF-16 code-unit offset into a byte offset in the given UTF-8
/// string. V8 source positions (e.g. `start_position`, `end_position`) are
/// UTF-16 code units. Returns `None` if the offset is past the end of the
/// string or falls inside a surrogate pair.
pub fn utf16_offset_to_byte(s: &str, utf16_offset: u32) -> Option<usize> {
    let mut u16_pos = 0u32;
    for (byte_pos, ch) in s.char_indices() {
        if u16_pos == utf16_offset {
            return Some(byte_pos);
        }
        if u16_pos > utf16_offset {
            return None;
        }
        u16_pos += ch.len_utf16() as u32;
    }
    (u16_pos == utf16_offset).then_some(s.len())
}

/// Converts a UTF-16 code-unit offset into a zero-based (line, column) pair,
/// where column is also counted in UTF-16 code units. Line breaks are `\n`;
/// a preceding `\r` counts as one column unit (V8's behavior). Returns `None`
/// if the offset is past the end of the string or falls inside a surrogate
/// pair.
pub fn utf16_offset_to_line_column(s: &str, utf16_offset: u32) -> Option<(u32, u32)> {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut u16_pos = 0u32;
    for ch in s.chars() {
        if u16_pos == utf16_offset {
            return Some((line, col));
        }
        if u16_pos > utf16_offset {
            return None;
        }
        let w = ch.len_utf16() as u32;
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += w;
        }
        u16_pos += w;
    }
    (u16_pos == utf16_offset).then_some((line, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_offset_to_byte_ascii() {
        let s = "hello\nworld";
        assert_eq!(utf16_offset_to_byte(s, 0), Some(0));
        assert_eq!(utf16_offset_to_byte(s, 5), Some(5));
        assert_eq!(utf16_offset_to_byte(s, 6), Some(6));
        assert_eq!(utf16_offset_to_byte(s, 11), Some(11));
        assert_eq!(utf16_offset_to_byte(s, 12), None);
    }

    #[test]
    fn test_utf16_offset_to_byte_bmp() {
        // 'é' = 1 UTF-16 code unit, 2 UTF-8 bytes
        let s = "aébc";
        assert_eq!(utf16_offset_to_byte(s, 0), Some(0));
        assert_eq!(utf16_offset_to_byte(s, 1), Some(1));
        assert_eq!(utf16_offset_to_byte(s, 2), Some(3));
        assert_eq!(utf16_offset_to_byte(s, 3), Some(4));
        assert_eq!(utf16_offset_to_byte(s, 4), Some(5));
    }

    #[test]
    fn test_utf16_offset_to_byte_surrogate_pair() {
        // '🦀' (U+1F980) = 2 UTF-16 code units, 4 UTF-8 bytes
        let s = "a🦀b";
        assert_eq!(utf16_offset_to_byte(s, 0), Some(0));
        assert_eq!(utf16_offset_to_byte(s, 1), Some(1));
        assert_eq!(utf16_offset_to_byte(s, 2), None);
        assert_eq!(utf16_offset_to_byte(s, 3), Some(5));
        assert_eq!(utf16_offset_to_byte(s, 4), Some(6));
    }

    #[test]
    fn test_utf16_offset_to_line_column_ascii() {
        let s = "ab\ncd\nef";
        assert_eq!(utf16_offset_to_line_column(s, 0), Some((0, 0)));
        assert_eq!(utf16_offset_to_line_column(s, 2), Some((0, 2)));
        assert_eq!(utf16_offset_to_line_column(s, 3), Some((1, 0)));
        assert_eq!(utf16_offset_to_line_column(s, 5), Some((1, 2)));
        assert_eq!(utf16_offset_to_line_column(s, 6), Some((2, 0)));
        assert_eq!(utf16_offset_to_line_column(s, 8), Some((2, 2)));
        assert_eq!(utf16_offset_to_line_column(s, 9), None);
    }

    #[test]
    fn test_utf16_offset_to_line_column_bmp() {
        let s = "aé\nb";
        assert_eq!(utf16_offset_to_line_column(s, 0), Some((0, 0)));
        assert_eq!(utf16_offset_to_line_column(s, 1), Some((0, 1)));
        assert_eq!(utf16_offset_to_line_column(s, 2), Some((0, 2)));
        assert_eq!(utf16_offset_to_line_column(s, 3), Some((1, 0)));
    }

    #[test]
    fn test_utf16_offset_to_line_column_surrogate_pair() {
        let s = "a🦀b";
        assert_eq!(utf16_offset_to_line_column(s, 1), Some((0, 1)));
        assert_eq!(utf16_offset_to_line_column(s, 2), None);
        assert_eq!(utf16_offset_to_line_column(s, 3), Some((0, 3)));
        assert_eq!(utf16_offset_to_line_column(s, 4), Some((0, 4)));
    }

    #[test]
    fn test_utf16_offset_to_line_column_empty() {
        assert_eq!(utf16_offset_to_line_column("", 0), Some((0, 0)));
        assert_eq!(utf16_offset_to_line_column("", 1), None);
    }
}
