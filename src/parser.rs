use std::fs::File;
use std::io::{self, BufReader, Read};

use crate::types::{RawHeapSnapshot, SnapshotHeader, SnapshotMeta};

const CHUNK_SIZE: usize = 256 * 1024;

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

// --- Stream parser ---

struct StreamParser<R: Read> {
    reader: BufReader<R>,
    buf: Vec<u8>,
    pos: usize,
}

impl<R: Read> StreamParser<R> {
    fn new(reader: R) -> Self {
        StreamParser {
            reader: BufReader::with_capacity(CHUNK_SIZE, reader),
            buf: Vec::with_capacity(CHUNK_SIZE * 2),
            pos: 0,
        }
    }

    fn compact(&mut self) {
        if self.pos > 0 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
    }

    fn read_more(&mut self) -> io::Result<bool> {
        self.compact();
        let old_len = self.buf.len();
        self.buf.resize(old_len + CHUNK_SIZE, 0);
        let n = self.reader.read(&mut self.buf[old_len..])?;
        self.buf.truncate(old_len + n);
        Ok(n > 0)
    }

    fn ensure_data(&mut self) -> io::Result<bool> {
        if self.pos < self.buf.len() {
            return Ok(true);
        }
        self.read_more()
    }

    /// Search forward for a byte sequence.
    fn find_token(&mut self, token: &[u8]) -> io::Result<()> {
        loop {
            if self.buf.len() - self.pos >= token.len() {
                if let Some(offset) = self.buf[self.pos..]
                    .windows(token.len())
                    .position(|w| w == token)
                {
                    self.pos += offset + token.len();
                    return Ok(());
                }
            }
            // Keep overlap bytes for partial matches at boundary
            let keep = token.len().saturating_sub(1);
            let new_pos = self.buf.len().saturating_sub(keep);
            if new_pos > self.pos {
                self.pos = new_pos;
            }
            if !self.read_more()? {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "token not found: {:?}",
                        std::str::from_utf8(token).unwrap_or("?")
                    ),
                ));
            }
        }
    }

    /// Search forward for a single byte.
    fn find_byte(&mut self, target: u8) -> io::Result<()> {
        loop {
            while self.pos < self.buf.len() {
                if self.buf[self.pos] == target {
                    self.pos += 1;
                    return Ok(());
                }
                self.pos += 1;
            }
            if !self.read_more()? {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("byte '{}' not found", target as char),
                ));
            }
        }
    }

    fn skip_whitespace(&mut self) -> io::Result<()> {
        loop {
            while self.pos < self.buf.len() {
                match self.buf[self.pos] {
                    b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                    _ => return Ok(()),
                }
            }
            if !self.read_more()? {
                return Ok(());
            }
        }
    }

    /// Extract a balanced JSON object or array as bytes.
    fn extract_balanced(&mut self) -> io::Result<Vec<u8>> {
        self.skip_whitespace()?;
        if !self.ensure_data()? {
            return Err(parse_err("unexpected EOF before balanced object"));
        }

        let opening = self.buf[self.pos];
        if opening != b'{' && opening != b'[' {
            return Err(parse_err(&format!(
                "expected '{{' or '[', got '{}'",
                opening as char
            )));
        }

        let mut result = Vec::new();
        let mut balance = 0i32;
        let mut in_string = false;
        let mut escape = false;

        loop {
            while self.pos < self.buf.len() {
                let b = self.buf[self.pos];
                result.push(b);
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
                            return Ok(result);
                        }
                    }
                    _ => {}
                }
            }

            if !self.read_more()? {
                return Err(parse_err("unexpected EOF in balanced object"));
            }
        }
    }

    /// Parse an array of unsigned integers. Searches forward for '[' first.
    fn parse_uint_array(&mut self, capacity: usize) -> io::Result<Vec<u32>> {
        self.find_byte(b'[')?;
        let mut result = if capacity > 0 {
            Vec::with_capacity(capacity)
        } else {
            Vec::new()
        };

        'outer: loop {
            // Ensure we have data
            if self.pos >= self.buf.len() {
                if !self.read_more()? {
                    return Err(parse_err("unexpected EOF in uint array"));
                }
            }

            // Skip non-digit bytes (whitespace, commas)
            while self.pos < self.buf.len() {
                let b = self.buf[self.pos];
                if b.is_ascii_digit() {
                    break;
                }
                if b == b']' {
                    self.pos += 1;
                    break 'outer;
                }
                self.pos += 1;
            }

            if self.pos >= self.buf.len() {
                continue;
            }

            // Parse integer
            let mut num: u32 = 0;
            loop {
                while self.pos < self.buf.len() {
                    let b = self.buf[self.pos];
                    if b.is_ascii_digit() {
                        num = num * 10 + (b - b'0') as u32;
                        self.pos += 1;
                    } else {
                        result.push(num);
                        continue 'outer;
                    }
                }
                // Buffer exhausted mid-number, read more
                if !self.read_more()? {
                    result.push(num);
                    break 'outer;
                }
            }
        }

        Ok(result)
    }

    /// Parse a JSON string array. Searches forward for '[' first.
    fn parse_string_array(&mut self) -> io::Result<Vec<String>> {
        self.find_byte(b'[')?;
        let mut result = Vec::new();

        loop {
            self.skip_whitespace()?;
            if !self.ensure_data()? {
                return Err(parse_err("unexpected EOF in string array"));
            }

            match self.buf[self.pos] {
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
        if !self.ensure_data()? {
            return Err(parse_err("unexpected EOF"));
        }
        if self.buf[self.pos] != b'"' {
            return Err(parse_err("expected '\"'"));
        }
        self.pos += 1;

        let mut bytes = Vec::new();

        loop {
            if self.pos >= self.buf.len() {
                if !self.read_more()? {
                    return Err(parse_err("unterminated string"));
                }
            }

            let b = self.buf[self.pos];
            self.pos += 1;

            match b {
                b'"' => return String::from_utf8(bytes).map_err(|e| parse_err(&e.to_string())),
                b'\\' => {
                    if self.pos >= self.buf.len() {
                        if !self.read_more()? {
                            return Err(parse_err("unterminated escape in string"));
                        }
                    }
                    let esc = self.buf[self.pos];
                    self.pos += 1;
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
                            let ch = self.parse_stream_unicode_escape()?;
                            let mut buf = [0u8; 4];
                            let s = ch.encode_utf8(&mut buf);
                            bytes.extend_from_slice(s.as_bytes());
                        }
                        _ => {
                            bytes.push(b'\\');
                            bytes.push(esc);
                        }
                    }
                }
                _ => bytes.push(b),
            }
        }
    }

    fn read_stream_byte(&mut self) -> io::Result<u8> {
        if self.pos >= self.buf.len() {
            if !self.read_more()? {
                return Err(parse_err("unexpected EOF"));
            }
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn parse_stream_unicode_escape(&mut self) -> io::Result<char> {
        let mut hex = [0u8; 4];
        for h in &mut hex {
            *h = self.read_stream_byte()?;
        }
        let hex_str = std::str::from_utf8(&hex).map_err(|e| parse_err(&e.to_string()))?;
        let code = u16::from_str_radix(hex_str, 16).map_err(|e| parse_err(&e.to_string()))?;

        if (0xD800..=0xDBFF).contains(&code) {
            // High surrogate — expect \uXXXX low surrogate
            if self.pos < self.buf.len() || self.ensure_data().unwrap_or(false) {
                if self.buf[self.pos] == b'\\' {
                    self.pos += 1;
                    let next = self.read_stream_byte()?;
                    if next == b'u' {
                        let mut hex2 = [0u8; 4];
                        for h in &mut hex2 {
                            *h = self.read_stream_byte()?;
                        }
                        let hex2_str =
                            std::str::from_utf8(&hex2).map_err(|e| parse_err(&e.to_string()))?;
                        let code2 = u16::from_str_radix(hex2_str, 16)
                            .map_err(|e| parse_err(&e.to_string()))?;
                        let cp = 0x10000 + ((code as u32 - 0xD800) << 10) + (code2 as u32 - 0xDC00);
                        return Ok(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                    }
                }
            }
            Ok('\u{FFFD}')
        } else {
            Ok(char::from_u32(code as u32).unwrap_or('\u{FFFD}'))
        }
    }
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
    let extra_native_bytes = value.get("extra_native_bytes").and_then(|v| v.as_f64());

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

pub fn parse(file: File) -> io::Result<RawHeapSnapshot> {
    parse_from_reader(file)
}

pub fn parse_from_reader<R: Read>(reader: R) -> io::Result<RawHeapSnapshot> {
    let mut p = StreamParser::new(reader);

    // 1. Parse snapshot metadata
    p.find_token(b"\"snapshot\"")?;
    p.find_byte(b':')?;
    let meta_bytes = p.extract_balanced()?;
    let header = parse_snapshot_header(&meta_bytes)?;

    // 2. Parse nodes
    let node_capacity = header.node_count * header.meta.node_fields.len();
    p.find_token(b"\"nodes\"")?;
    let nodes = p.parse_uint_array(node_capacity)?;

    // 3. Parse edges
    let edge_capacity = header.edge_count * header.meta.edge_fields.len();
    p.find_token(b"\"edges\"")?;
    let edges = p.parse_uint_array(edge_capacity)?;

    // 4. Parse trace data (optional, appears between edges and locations)
    let (trace_function_infos, trace_tree_parents, trace_tree_func_idxs) =
        if header.trace_function_count > 0 {
            let tfi_fields = header.meta.trace_function_info_fields.len().max(6);
            let tfi_capacity = header.trace_function_count * tfi_fields;
            p.find_token(b"\"trace_function_infos\"")?;
            let tfi = p.parse_uint_array(tfi_capacity)?;

            p.find_token(b"\"trace_tree\"")?;
            p.find_byte(b':')?;
            let tree_bytes = p.extract_balanced()?;
            let (parents, func_idxs) = flatten_trace_tree(&tree_bytes)?;

            (tfi, parents, func_idxs)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

    // 4b. Parse samples (optional, appears after trace_tree)
    let samples = if !header.meta.sample_fields.is_empty() && header.trace_function_count > 0 {
        p.find_token(b"\"samples\"")?;
        p.parse_uint_array(0)?
    } else {
        Vec::new()
    };

    // 5. Parse locations (optional)
    let locations = if !header.meta.location_fields.is_empty() {
        p.find_token(b"\"locations\"")?;
        p.parse_uint_array(0)?
    } else {
        Vec::new()
    };

    // 6. Parse strings (always last)
    p.find_token(b"\"strings\"")?;
    let strings = p.parse_string_array()?;

    Ok(RawHeapSnapshot {
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
