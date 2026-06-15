use std::io;

#[cfg(not(target_arch = "wasm32"))]
use memmap2::Mmap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;

use memchr::{memchr, memchr2};

use super::super::ParsedHeapSnapshot;
use crate::types::{EdgeRecord, NodeRecord, SnapshotHeader, SnapshotMeta};

// --- Mini JSON value type for metadata parsing ---

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s),
            _ => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        self.as_f64().map(|n| n as u64)
    }

    fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

// --- Mini JSON parser for metadata (works on a byte slice) ---

struct JsonParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        JsonParser { data, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.data.len() {
            match self.data[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn advance(&mut self) -> u8 {
        let b = self.data[self.pos];
        self.pos += 1;
        b
    }

    fn expect(&mut self, ch: u8) -> io::Result<()> {
        self.skip_ws();
        if self.peek() != Some(ch) {
            return Err(parse_err(&format!(
                "expected '{}' at pos {}",
                ch as char, self.pos
            )));
        }
        self.advance();
        Ok(())
    }

    fn expect_literal(&mut self, lit: &[u8]) -> io::Result<()> {
        for &b in lit {
            if self.peek() != Some(b) {
                return Err(parse_err(&format!("expected literal at pos {}", self.pos)));
            }
            self.advance();
        }
        Ok(())
    }

    fn parse_value(&mut self) -> io::Result<JsonValue> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => Ok(JsonValue::Str(self.parse_string()?)),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b) if b == b'-' || b.is_ascii_digit() => self.parse_number(),
            Some(b) => Err(parse_err(&format!(
                "unexpected byte '{}' at pos {}",
                b as char, self.pos
            ))),
            None => Err(parse_err("unexpected EOF")),
        }
    }

    fn parse_string(&mut self) -> io::Result<String> {
        self.expect(b'"')?;
        let mut result = Vec::new();
        loop {
            if self.pos >= self.data.len() {
                return Err(parse_err("unterminated string"));
            }
            let b = self.advance();
            match b {
                b'"' => return String::from_utf8(result).map_err(|e| parse_err(&e.to_string())),
                b'\\' => {
                    if self.pos >= self.data.len() {
                        return Err(parse_err("unterminated escape"));
                    }
                    let esc = self.advance();
                    match esc {
                        b'"' => result.push(b'"'),
                        b'\\' => result.push(b'\\'),
                        b'/' => result.push(b'/'),
                        b'n' => result.push(b'\n'),
                        b'r' => result.push(b'\r'),
                        b't' => result.push(b'\t'),
                        b'b' => result.push(0x08),
                        b'f' => result.push(0x0C),
                        b'u' => {
                            let ch = self.parse_unicode_escape()?;
                            let mut buf = [0u8; 4];
                            let s = ch.encode_utf8(&mut buf);
                            result.extend_from_slice(s.as_bytes());
                        }
                        _ => {
                            result.push(b'\\');
                            result.push(esc);
                        }
                    }
                }
                _ => result.push(b),
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> io::Result<char> {
        let hex = self.read_hex4()?;
        let code = u16::from_str_radix(&hex, 16).map_err(|e| parse_err(&e.to_string()))?;
        if (0xD800..=0xDBFF).contains(&code) {
            if self.peek() == Some(b'\\') {
                self.advance();
                if self.peek() == Some(b'u') {
                    self.advance();
                    let hex2 = self.read_hex4()?;
                    let code2 =
                        u16::from_str_radix(&hex2, 16).map_err(|e| parse_err(&e.to_string()))?;
                    let cp = 0x10000 + ((code as u32 - 0xD800) << 10) + (code2 as u32 - 0xDC00);
                    return Ok(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                }
            }
            Ok('\u{FFFD}')
        } else {
            Ok(char::from_u32(code as u32).unwrap_or('\u{FFFD}'))
        }
    }

    fn read_hex4(&mut self) -> io::Result<String> {
        let mut s = String::with_capacity(4);
        for _ in 0..4 {
            if self.pos >= self.data.len() {
                return Err(parse_err("unterminated unicode escape"));
            }
            s.push(self.advance() as char);
        }
        Ok(s)
    }

    fn parse_number(&mut self) -> io::Result<JsonValue> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.advance();
        }
        while self.pos < self.data.len() && self.data[self.pos].is_ascii_digit() {
            self.advance();
        }
        if self.peek() == Some(b'.') {
            self.advance();
            while self.pos < self.data.len() && self.data[self.pos].is_ascii_digit() {
                self.advance();
            }
        }
        if self.peek() == Some(b'e') || self.peek() == Some(b'E') {
            self.advance();
            if self.peek() == Some(b'+') || self.peek() == Some(b'-') {
                self.advance();
            }
            while self.pos < self.data.len() && self.data[self.pos].is_ascii_digit() {
                self.advance();
            }
        }
        let s = std::str::from_utf8(&self.data[start..self.pos])
            .map_err(|e| parse_err(&e.to_string()))?;
        let n: f64 = s
            .parse()
            .map_err(|e: std::num::ParseFloatError| parse_err(&e.to_string()))?;
        Ok(JsonValue::Number(n))
    }

    fn parse_array(&mut self) -> io::Result<JsonValue> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.advance();
            return Ok(JsonValue::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.advance();
                }
                Some(b']') => {
                    self.advance();
                    return Ok(JsonValue::Array(items));
                }
                _ => {
                    return Err(parse_err(&format!(
                        "expected ',' or ']' at pos {}",
                        self.pos
                    )));
                }
            }
        }
    }

    fn parse_object(&mut self) -> io::Result<JsonValue> {
        self.expect(b'{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.advance();
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.expect(b':')?;
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.advance();
                }
                Some(b'}') => {
                    self.advance();
                    return Ok(JsonValue::Object(pairs));
                }
                _ => {
                    return Err(parse_err(&format!(
                        "expected ',' or '}}' at pos {}",
                        self.pos
                    )));
                }
            }
        }
    }
}

fn parse_err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

// --- Slice parser ---

struct SliceParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SliceParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Search forward for a byte sequence.
    fn find_token(&mut self, token: &[u8]) -> io::Result<()> {
        while let Some(offset) = memchr(token[0], &self.data[self.pos..]) {
            let idx = self.pos + offset;
            if self.data[idx..].starts_with(token) {
                self.pos = idx + token.len();
                return Ok(());
            }
            self.pos = idx + 1;
        }

        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "token not found: {:?}",
                std::str::from_utf8(token).unwrap_or("?")
            ),
        ))
    }

    /// Search forward for a single byte.
    fn find_byte(&mut self, target: u8) -> io::Result<()> {
        if let Some(offset) = memchr(target, &self.data[self.pos..]) {
            self.pos += offset + 1;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("byte '{}' not found", target as char),
            ))
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.data.len() {
            match self.data[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    /// Extract a balanced JSON object or array as bytes.
    fn extract_balanced(&mut self) -> io::Result<&'a [u8]> {
        self.skip_whitespace();
        if self.pos >= self.data.len() {
            return Err(parse_err("unexpected EOF before balanced object"));
        }

        let start = self.pos;
        let opening = self.data[self.pos];
        if opening != b'{' && opening != b'[' {
            return Err(parse_err(&format!(
                "expected '{{' or '[', got '{}'",
                opening as char
            )));
        }

        let mut balance = 0i32;
        let mut in_string = false;
        let mut escape = false;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;

            if escape {
                escape = false;
                continue;
            }

            if in_string {
                if b == b'\\' {
                    escape = true;
                } else if b == b'"' {
                    in_string = false;
                }
                continue;
            }

            match b {
                b'"' => in_string = true,
                b'{' | b'[' => balance += 1,
                b'}' | b']' => {
                    balance -= 1;
                    if balance == 0 {
                        return Ok(&self.data[start..self.pos]);
                    }
                }
                _ => {}
            }
        }

        Err(parse_err("unexpected EOF in balanced object"))
    }

    /// Parse an array of unsigned integers. Searches forward for '[' first.
    fn parse_uint_array(&mut self, capacity: usize) -> io::Result<Vec<u32>> {
        self.find_byte(b'[')?;
        let mut result = if capacity > 0 {
            Vec::with_capacity(capacity)
        } else {
            Vec::new()
        };

        loop {
            while self.pos < self.data.len() {
                let b = self.data[self.pos];
                if b.is_ascii_digit() {
                    break;
                }
                self.pos += 1;
                if b == b']' {
                    return Ok(result);
                }
            }
            if self.pos >= self.data.len() {
                return Err(parse_err("unexpected EOF in uint array"));
            }

            let mut num: u32 = 0;
            while self.pos < self.data.len() {
                let b = self.data[self.pos];
                if !b.is_ascii_digit() {
                    break;
                }
                num = num * 10 + (b - b'0') as u32;
                self.pos += 1;
            }
            result.push(num);
        }
    }

    fn uint_array_tail(&mut self) -> io::Result<&'a [u8]> {
        self.find_byte(b'[')?;
        Ok(&self.data[self.pos..])
    }

    /// Parse a JSON string array. Searches forward for '[' first.
    fn parse_string_array(&mut self) -> io::Result<Vec<String>> {
        self.find_byte(b'[')?;
        let mut result = Vec::new();

        loop {
            self.skip_whitespace();
            if self.pos >= self.data.len() {
                return Err(parse_err("unexpected EOF in string array"));
            }

            match self.data[self.pos] {
                b']' => {
                    self.pos += 1;
                    return Ok(result);
                }
                b',' => {
                    self.pos += 1;
                }
                b'"' => {
                    result.push(self.parse_json_string()?);
                }
                b => {
                    return Err(parse_err(&format!(
                        "unexpected byte '{}' in string array",
                        b as char
                    )));
                }
            }
        }
    }

    /// Parse a single JSON string. Expects current position at opening '"'.
    fn parse_json_string(&mut self) -> io::Result<String> {
        if self.pos >= self.data.len() {
            return Err(parse_err("unexpected EOF"));
        }
        if self.data[self.pos] != b'"' {
            return Err(parse_err("expected '\"'"));
        }
        self.pos += 1;
        let start = self.pos;

        let Some(offset) = memchr2(b'"', b'\\', &self.data[self.pos..]) else {
            return Err(parse_err("unterminated string"));
        };
        let idx = self.pos + offset;
        if self.data[idx] == b'"' {
            self.pos = idx + 1;
            return String::from_utf8(self.data[start..idx].to_vec())
                .map_err(|e| parse_err(&e.to_string()));
        }

        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&self.data[start..idx]);
        self.pos = idx + 1;

        loop {
            if self.pos >= self.data.len() {
                return Err(parse_err("unterminated escape in string"));
            }
            let esc = self.read_slice_byte()?;
            match esc {
                b'"' => bytes.push(b'"'),
                b'\\' => bytes.push(b'\\'),
                b'/' => bytes.push(b'/'),
                b'n' => bytes.push(b'\n'),
                b'r' => bytes.push(b'\r'),
                b't' => bytes.push(b'\t'),
                b'b' => bytes.push(0x08),
                b'f' => bytes.push(0x0C),
                b'u' => {
                    let ch = self.parse_slice_unicode_escape()?;
                    let mut buf = [0u8; 4];
                    let s = ch.encode_utf8(&mut buf);
                    bytes.extend_from_slice(s.as_bytes());
                }
                _ => {
                    bytes.push(b'\\');
                    bytes.push(esc);
                }
            }

            let segment_start = self.pos;
            let Some(offset) = memchr2(b'"', b'\\', &self.data[self.pos..]) else {
                return Err(parse_err("unterminated string"));
            };
            let idx = self.pos + offset;
            bytes.extend_from_slice(&self.data[segment_start..idx]);
            self.pos = idx + 1;
            if self.data[idx] == b'"' {
                return String::from_utf8(bytes).map_err(|e| parse_err(&e.to_string()));
            }
        }
    }

    fn read_slice_byte(&mut self) -> io::Result<u8> {
        if self.pos >= self.data.len() {
            return Err(parse_err("unexpected EOF"));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn parse_slice_unicode_escape(&mut self) -> io::Result<char> {
        if self.pos + 4 > self.data.len() {
            return Err(parse_err("unterminated unicode escape"));
        }
        let hex = &self.data[self.pos..self.pos + 4];
        self.pos += 4;
        let hex_str = std::str::from_utf8(hex).map_err(|e| parse_err(&e.to_string()))?;
        let code = u16::from_str_radix(hex_str, 16).map_err(|e| parse_err(&e.to_string()))?;

        if (0xD800..=0xDBFF).contains(&code) {
            if self.pos + 6 <= self.data.len()
                && self.data[self.pos] == b'\\'
                && self.data[self.pos + 1] == b'u'
            {
                self.pos += 2;
                let hex2 = &self.data[self.pos..self.pos + 4];
                self.pos += 4;
                let hex2_str = std::str::from_utf8(hex2).map_err(|e| parse_err(&e.to_string()))?;
                let code2 =
                    u16::from_str_radix(hex2_str, 16).map_err(|e| parse_err(&e.to_string()))?;
                let cp = 0x10000 + ((code as u32 - 0xD800) << 10) + (code2 as u32 - 0xDC00);
                return Ok(char::from_u32(cp).unwrap_or('\u{FFFD}'));
            }
            Ok('\u{FFFD}')
        } else {
            Ok(char::from_u32(code as u32).unwrap_or('\u{FFFD}'))
        }
    }
}

#[inline(always)]
fn read_next_uint(data: &[u8], pos: &mut usize) -> Option<u32> {
    while *pos < data.len() && !data[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if *pos >= data.len() {
        return None;
    }

    let mut num = 0u32;
    while *pos < data.len() {
        let b = data[*pos];
        if !b.is_ascii_digit() {
            break;
        }
        num = num * 10 + (b - b'0') as u32;
        *pos += 1;
    }
    Some(num)
}

#[inline(always)]
fn read_required_uint(data: &[u8], pos: &mut usize, context: &str) -> io::Result<u32> {
    read_next_uint(data, pos).ok_or_else(|| parse_err(context))
}

#[allow(clippy::too_many_arguments)]
fn parse_node_records_slice(
    data: &[u8],
    node_count: usize,
    node_field_count: usize,
    node_type_offset: usize,
    node_name_offset: usize,
    node_id_offset: usize,
    node_self_size_offset: usize,
    node_edge_count_offset: usize,
    node_detachedness_offset: Option<usize>,
    node_trace_node_id_offset: Option<usize>,
) -> io::Result<Vec<NodeRecord>> {
    if node_field_count == 6
        && node_type_offset == 0
        && node_name_offset == 1
        && node_id_offset == 2
        && node_self_size_offset == 3
        && node_edge_count_offset == 4
        && node_detachedness_offset == Some(5)
        && node_trace_node_id_offset.is_none()
    {
        return parse_node_records_6(data, node_count);
    }

    let mut records = Vec::with_capacity(node_count);
    // NodeRecord is Copy and contains only integer fields. Every slot is
    // written before the vector is returned.
    unsafe {
        records.set_len(node_count);
    }

    let mut pos = 0usize;
    let mut first_edge = 0u32;
    for record in &mut records {
        let mut type_id = 0u32;
        let mut name = 0u32;
        let mut id = 0u32;
        let mut self_size = 0u32;
        let mut edge_count = 0u32;
        let mut detachedness = 0u32;
        let mut trace_node_id = 0u32;

        for field in 0..node_field_count {
            let value = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;
            if field == node_type_offset {
                type_id = value;
            } else if field == node_name_offset {
                name = value;
            } else if field == node_id_offset {
                id = value;
            } else if field == node_self_size_offset {
                self_size = value;
            } else if field == node_edge_count_offset {
                edge_count = value;
            } else if Some(field) == node_detachedness_offset {
                detachedness = value;
            } else if Some(field) == node_trace_node_id_offset {
                trace_node_id = value;
            }
        }

        *record = NodeRecord {
            type_id,
            name,
            id,
            self_size,
            edge_count,
            detachedness,
            trace_node_id,
            first_edge,
        };
        first_edge = first_edge
            .checked_add(edge_count)
            .ok_or_else(|| parse_err("node edge count overflow"))?;
    }

    Ok(records)
}

fn parse_node_records_6(data: &[u8], node_count: usize) -> io::Result<Vec<NodeRecord>> {
    let mut records = Vec::with_capacity(node_count);
    // NodeRecord is Copy and contains only integer fields. Every slot is
    // written before the vector is returned.
    unsafe {
        records.set_len(node_count);
    }

    let mut pos = 0usize;
    let mut first_edge = 0u32;
    for record in &mut records {
        let type_id = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;
        let name = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;
        let id = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;
        let self_size = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;
        let edge_count = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;
        let detachedness = read_required_uint(data, &mut pos, "unexpected EOF in node array")?;

        *record = NodeRecord {
            type_id,
            name,
            id,
            self_size,
            edge_count,
            detachedness,
            trace_node_id: 0,
            first_edge,
        };
        first_edge = first_edge
            .checked_add(edge_count)
            .ok_or_else(|| parse_err("node edge count overflow"))?;
    }

    Ok(records)
}

fn parse_edge_records_slice(
    data: &[u8],
    edge_count: usize,
    edge_field_count: usize,
    edge_type_offset: usize,
    edge_name_offset: usize,
    edge_to_node_offset: usize,
    node_field_count: usize,
) -> io::Result<Vec<EdgeRecord>> {
    if edge_field_count == 3
        && edge_type_offset == 0
        && edge_name_offset == 1
        && edge_to_node_offset == 2
    {
        if node_field_count == 6 {
            return parse_edge_records_3_node6(data, edge_count);
        }
        return parse_edge_records_3(data, edge_count, node_field_count);
    }

    let mut records = Vec::with_capacity(edge_count);
    // EdgeRecord is Copy and contains only integer fields. Every slot is
    // written before the vector is returned.
    unsafe {
        records.set_len(edge_count);
    }

    let mut pos = 0usize;
    for record in &mut records {
        let mut type_id = 0u32;
        let mut name_or_index = 0u32;
        let mut to_node = 0u32;

        for field in 0..edge_field_count {
            let value = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;
            if field == edge_type_offset {
                type_id = value;
            } else if field == edge_name_offset {
                name_or_index = value;
            } else if field == edge_to_node_offset {
                to_node = value;
            }
        }

        *record = EdgeRecord {
            type_id,
            name_or_index,
            to_node_ordinal: (to_node as usize / node_field_count) as u32,
            _padding: 0,
        };
    }

    Ok(records)
}

fn parse_edge_records_3(
    data: &[u8],
    edge_count: usize,
    node_field_count: usize,
) -> io::Result<Vec<EdgeRecord>> {
    let mut records = Vec::with_capacity(edge_count);
    // EdgeRecord is Copy and contains only integer fields. Every slot is
    // written before the vector is returned.
    unsafe {
        records.set_len(edge_count);
    }

    let mut pos = 0usize;
    for record in &mut records {
        let type_id = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;
        let name_or_index = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;
        let to_node = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;

        *record = EdgeRecord {
            type_id,
            name_or_index,
            to_node_ordinal: (to_node as usize / node_field_count) as u32,
            _padding: 0,
        };
    }

    Ok(records)
}

fn parse_edge_records_3_node6(data: &[u8], edge_count: usize) -> io::Result<Vec<EdgeRecord>> {
    let mut records = Vec::with_capacity(edge_count);
    // EdgeRecord is Copy and contains only integer fields. Every slot is
    // written before the vector is returned.
    unsafe {
        records.set_len(edge_count);
    }

    let mut pos = 0usize;
    for record in &mut records {
        let type_id = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;
        let name_or_index = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;
        let to_node = read_required_uint(data, &mut pos, "unexpected EOF in edge array")?;

        *record = EdgeRecord {
            type_id,
            name_or_index,
            to_node_ordinal: to_node / 6,
            _padding: 0,
        };
    }

    Ok(records)
}

// --- Header parsing ---

fn parse_snapshot_header(data: &[u8]) -> io::Result<SnapshotHeader> {
    let mut parser = JsonParser::new(data);
    let value = parser.parse_value()?;

    let meta_val = value
        .get("meta")
        .ok_or_else(|| parse_err("missing 'meta' in snapshot header"))?;

    let get_string_array = |obj: &JsonValue, key: &str| -> Vec<String> {
        obj.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };

    let node_fields = get_string_array(meta_val, "node_fields");
    let edge_fields = get_string_array(meta_val, "edge_fields");

    // Extract type enum arrays: node_types[type_offset] and edge_types[type_offset]
    let node_type_offset = node_fields.iter().position(|f| f == "type").unwrap_or(0);
    let edge_type_offset = edge_fields.iter().position(|f| f == "type").unwrap_or(0);

    let extract_type_enum = |obj: &JsonValue, key: &str, offset: usize| -> Vec<String> {
        obj.get(key)
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.get(offset))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default()
    };

    let node_type_enum = extract_type_enum(meta_val, "node_types", node_type_offset);
    let edge_type_enum = extract_type_enum(meta_val, "edge_types", edge_type_offset);

    let meta = SnapshotMeta {
        node_fields,
        node_type_enum,
        edge_fields,
        edge_type_enum,
        location_fields: get_string_array(meta_val, "location_fields"),
        sample_fields: get_string_array(meta_val, "sample_fields"),
        trace_function_info_fields: get_string_array(meta_val, "trace_function_info_fields"),
        trace_node_fields: get_string_array(meta_val, "trace_node_fields"),
    };

    let node_count = value
        .get("node_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let edge_count = value
        .get("edge_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let trace_function_count = value
        .get("trace_function_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let root_index = value
        .get("root_index")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let extra_native_bytes = value.get("extra_native_bytes").and_then(|v| v.as_u64());

    Ok(SnapshotHeader {
        meta,
        node_count,
        edge_count,
        trace_function_count,
        root_index,
        extra_native_bytes,
    })
}

// --- Trace tree flattening ---

/// Parse the nested trace_tree JSON and flatten into parent-pointer arrays.
///
/// The trace tree format is a nested array:
///   `[id, function_info_index, count, size, [children...]]`
/// where children is a flat array of child nodes (each 5 elements + their nested children).
///
/// Returns `(parents, func_idxs)` indexed by trace node ID.
fn flatten_trace_tree(raw: &[u8]) -> io::Result<(Vec<u32>, Vec<u32>)> {
    let mut jp = JsonParser::new(raw);
    let tree = jp.parse_value()?;

    let arr = tree
        .as_array()
        .ok_or_else(|| parse_err("trace_tree: expected array"))?;

    // First pass: find max ID to size the output vectors.
    let mut max_id = 0u32;
    fn find_max_id(arr: &[JsonValue], max: &mut u32) {
        // The array is a flattened list of fields: [id, func_idx, count, size, [children], ...]
        // Each node is 5 elements: id, func_idx, count, size, children_array
        let mut i = 0;
        while i + 4 < arr.len() {
            if let Some(id) = arr[i].as_u64() {
                *max = (*max).max(id as u32);
            }
            if let Some(children) = arr[i + 4].as_array() {
                find_max_id(children, max);
            }
            i += 5;
        }
    }
    find_max_id(arr, &mut max_id);

    let size = (max_id as usize) + 1;
    let mut parents = vec![0u32; size];
    let mut func_idxs = vec![0u32; size];

    fn walk(arr: &[JsonValue], parent_id: u32, parents: &mut [u32], func_idxs: &mut [u32]) {
        let mut i = 0;
        while i + 4 < arr.len() {
            let id = arr[i].as_u64().unwrap_or(0) as u32;
            let func_idx = arr[i + 1].as_u64().unwrap_or(0) as u32;
            // arr[i+2] = count, arr[i+3] = size — not needed for stack reconstruction
            let idx = id as usize;
            if idx < parents.len() {
                parents[idx] = parent_id;
                func_idxs[idx] = func_idx;
            }
            if let Some(children) = arr[i + 4].as_array() {
                walk(children, id, parents, func_idxs);
            }
            i += 5;
        }
    }
    walk(arr, 0, &mut parents, &mut func_idxs);

    Ok((parents, func_idxs))
}

// --- Main entry point ---

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn parse(file: File) -> io::Result<ParsedHeapSnapshot> {
    // The parser only borrows this slice while producing owned records and strings.
    let data = unsafe { Mmap::map(&file)? };
    parse_from_slice(&data)
}

pub(super) fn parse_from_slice(data: &[u8]) -> io::Result<ParsedHeapSnapshot> {
    let mut p = SliceParser::new(data);

    p.find_token(b"\"snapshot\"")?;
    p.find_byte(b':')?;
    let meta_bytes = p.extract_balanced()?;
    let header = parse_snapshot_header(meta_bytes)?;

    let node_field_count = header.meta.node_fields.len();
    let node_type_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "type")
        .unwrap();
    let node_name_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "name")
        .unwrap();
    let node_id_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "id")
        .unwrap();
    let node_self_size_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "self_size")
        .unwrap();
    let node_edge_count_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "edge_count")
        .unwrap();
    let node_detachedness_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "detachedness");
    let node_trace_node_id_offset = header
        .meta
        .node_fields
        .iter()
        .position(|f| f == "trace_node_id");

    let edge_field_count = header.meta.edge_fields.len();
    let edge_type_offset = header
        .meta
        .edge_fields
        .iter()
        .position(|f| f == "type")
        .unwrap();
    let edge_name_offset = header
        .meta
        .edge_fields
        .iter()
        .position(|f| f == "name_or_index")
        .unwrap();
    let edge_to_node_offset = header
        .meta
        .edge_fields
        .iter()
        .position(|f| f == "to_node")
        .unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    let (
        nodes,
        edges,
        strings,
        locations,
        trace_function_infos,
        trace_tree_parents,
        trace_tree_func_idxs,
        samples,
    ) = std::thread::scope(|scope| -> io::Result<_> {
        p.find_token(b"\"nodes\"")?;
        let node_data = p.uint_array_tail()?;
        let nodes_worker = scope.spawn(|| {
            parse_node_records_slice(
                node_data,
                header.node_count,
                node_field_count,
                node_type_offset,
                node_name_offset,
                node_id_offset,
                node_self_size_offset,
                node_edge_count_offset,
                node_detachedness_offset,
                node_trace_node_id_offset,
            )
        });

        p.find_token(b"\"edges\"")?;
        let edge_data = p.uint_array_tail()?;
        let edges_worker = scope.spawn(|| {
            parse_edge_records_slice(
                edge_data,
                header.edge_count,
                edge_field_count,
                edge_type_offset,
                edge_name_offset,
                edge_to_node_offset,
                node_field_count,
            )
        });

        let (trace_function_infos, trace_tree_parents, trace_tree_func_idxs) =
            if header.trace_function_count > 0 {
                let tfi_fields = header.meta.trace_function_info_fields.len().max(6);
                let tfi_capacity = header.trace_function_count * tfi_fields;
                p.find_token(b"\"trace_function_infos\"")?;
                let tfi = p.parse_uint_array(tfi_capacity)?;

                p.find_token(b"\"trace_tree\"")?;
                p.find_byte(b':')?;
                let tree_bytes = p.extract_balanced()?;
                let (parents, func_idxs) = flatten_trace_tree(tree_bytes)?;

                (tfi, parents, func_idxs)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        let samples = if !header.meta.sample_fields.is_empty() && header.trace_function_count > 0 {
            p.find_token(b"\"samples\"")?;
            p.parse_uint_array(0)?
        } else {
            Vec::new()
        };

        let locations = if !header.meta.location_fields.is_empty() {
            p.find_token(b"\"locations\"")?;
            p.parse_uint_array(0)?
        } else {
            Vec::new()
        };

        p.find_token(b"\"strings\"")?;
        let strings = p.parse_string_array()?;

        let nodes = nodes_worker
            .join()
            .map_err(|_| parse_err("nodes parser thread panicked"))??;
        let edges = edges_worker
            .join()
            .map_err(|_| parse_err("edges parser thread panicked"))??;

        Ok((
            nodes,
            edges,
            strings,
            locations,
            trace_function_infos,
            trace_tree_parents,
            trace_tree_func_idxs,
            samples,
        ))
    })?;

    #[cfg(target_arch = "wasm32")]
    let (
        nodes,
        edges,
        strings,
        locations,
        trace_function_infos,
        trace_tree_parents,
        trace_tree_func_idxs,
        samples,
    ) = {
        p.find_token(b"\"nodes\"")?;
        let node_data = p.uint_array_tail()?;
        let nodes = parse_node_records_slice(
            node_data,
            header.node_count,
            node_field_count,
            node_type_offset,
            node_name_offset,
            node_id_offset,
            node_self_size_offset,
            node_edge_count_offset,
            node_detachedness_offset,
            node_trace_node_id_offset,
        )?;

        p.find_token(b"\"edges\"")?;
        let edge_data = p.uint_array_tail()?;
        let edges = parse_edge_records_slice(
            edge_data,
            header.edge_count,
            edge_field_count,
            edge_type_offset,
            edge_name_offset,
            edge_to_node_offset,
            node_field_count,
        )?;

        let (trace_function_infos, trace_tree_parents, trace_tree_func_idxs) =
            if header.trace_function_count > 0 {
                let tfi_fields = header.meta.trace_function_info_fields.len().max(6);
                let tfi_capacity = header.trace_function_count * tfi_fields;
                p.find_token(b"\"trace_function_infos\"")?;
                let tfi = p.parse_uint_array(tfi_capacity)?;

                p.find_token(b"\"trace_tree\"")?;
                p.find_byte(b':')?;
                let tree_bytes = p.extract_balanced()?;
                let (parents, func_idxs) = flatten_trace_tree(tree_bytes)?;

                (tfi, parents, func_idxs)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        let samples = if !header.meta.sample_fields.is_empty() && header.trace_function_count > 0 {
            p.find_token(b"\"samples\"")?;
            p.parse_uint_array(0)?
        } else {
            Vec::new()
        };

        let locations = if !header.meta.location_fields.is_empty() {
            p.find_token(b"\"locations\"")?;
            p.parse_uint_array(0)?
        } else {
            Vec::new()
        };

        p.find_token(b"\"strings\"")?;
        let strings = p.parse_string_array()?;

        (
            nodes,
            edges,
            strings,
            locations,
            trace_function_infos,
            trace_tree_parents,
            trace_tree_func_idxs,
            samples,
        )
    };

    Ok(ParsedHeapSnapshot {
        snapshot: header,
        nodes,
        edges,
        strings,
        locations,
        trace_function_infos,
        trace_tree_parents,
        trace_tree_func_idxs,
        samples,
    })
}
