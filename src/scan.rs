pub(crate) fn starts_with_word(line: &str, word: &str) -> bool {
    let Some(rest) = line.strip_prefix(word) else {
        return false;
    };
    match rest.chars().next() {
        None => true,
        Some(c) => !c.is_ascii_alphanumeric() && c != '_',
    }
}

/// Drop a `%%` comment that is outside quotes. The returned slice is trimmed.
pub(crate) fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' {
            quote = Some(b);
            i += 1;
            continue;
        }
        if b == b'%' && bytes.get(i + 1) == Some(&b'%') {
            return line[..i].trim_end();
        }
        i += 1;
    }
    line.trim_end()
}

pub(crate) fn is_ident(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) struct Cursor<'a> {
    s: &'a str,
    pub i: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }

    pub(crate) fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    pub(crate) fn eof(&self) -> bool {
        self.i >= self.s.len()
    }

    pub(crate) fn skip_ws(&mut self) {
        let rest = self.rest();
        let trimmed = rest.trim_start();
        self.i += rest.len() - trimmed.len();
    }

    pub(crate) fn bump(&mut self, n: usize) {
        self.i += n;
    }
}
