pub(crate) const TEXT: &str = "#0f172a";
pub(crate) const STROKE: &str = "#334155";
pub(crate) const FILL: &str = "#f8fafc";
pub(crate) const NOTE_FILL: &str = "#fef9c3";
pub(crate) const NOTE_STROKE: &str = "#a16207";
pub(crate) const MUTED: &str = "#94a3b8";
pub(crate) const PAD: i32 = 16;

pub(crate) struct Svg {
    body: String,
}

impl Svg {
    pub(crate) fn new() -> Self {
        Self {
            body: String::with_capacity(2048),
        }
    }

    pub(crate) fn finish(self, width: i32, height: i32) -> Vec<u8> {
        let w = width.max(8);
        let h = height.max(8);
        let svg = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"ui-sans-serif, Segoe UI, sans-serif\" font-size=\"14\">\n\
<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n\
{body}</svg>\n",
            body = self.body
        );
        svg.into_bytes()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        rx: i32,
        fill: &str,
        stroke: &str,
        dash: Option<&str>,
    ) {
        self.body.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{rx}\" fill=\"{fill}\" stroke=\"{stroke}\""
        ));
        if let Some(dash) = dash {
            self.body.push_str(&format!(" stroke-dasharray=\"{dash}\""));
        }
        self.body.push_str("/>\n");
    }

    pub(crate) fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32) {
        self.body.push_str(&format!(
            "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{rx}\" ry=\"{ry}\" fill=\"{FILL}\" stroke=\"{STROKE}\"/>\n"
        ));
    }

    pub(crate) fn polygon(&mut self, points: &[(i32, i32)], fill: &str, stroke: Option<&str>) {
        self.body.push_str("<polygon points=\"");
        for (i, (x, y)) in points.iter().enumerate() {
            if i > 0 {
                self.body.push(' ');
            }
            self.body.push_str(&format!("{x},{y}"));
        }
        self.body.push_str(&format!(
            "\" fill=\"{fill}\" stroke=\"{stroke}\"/>\n",
            stroke = stroke.unwrap_or("none")
        ));
    }

    pub(crate) fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, attrs: &str) {
        self.body.push_str(&format!(
            "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" fill=\"none\" {attrs}/>\n"
        ));
    }

    pub(crate) fn path(&mut self, d: &str, attrs: &str) {
        self.body
            .push_str(&format!("<path d=\"{d}\" fill=\"none\" {attrs}/>\n"));
    }

    pub(crate) fn text_at(&mut self, x: i32, y: i32, anchor: &str, fill: &str, text: &str) {
        let escaped = escape(text);
        self.body.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" dominant-baseline=\"middle\" fill=\"{fill}\">{escaped}</text>\n"
        ));
    }

    pub(crate) fn text_lines(&mut self, cx: i32, cy: i32, lines: &[String]) {
        let n = lines.len() as i32;
        let line_h = 16;
        let start = cy - (n - 1) * line_h / 2;
        for (i, line) in lines.iter().enumerate() {
            let y = start + i as i32 * line_h;
            self.text_at(cx, y, "middle", TEXT, line);
        }
    }
}

pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn text_width(text: &str) -> i32 {
    text.chars()
        .map(|c| if c.is_ascii() { 8 } else { 14 })
        .sum()
}

/// Turn `<br>`, `<br/>`, and `<br />` into newlines. Anything else stays.
pub(crate) fn label_lines(label: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(label.len());
    let bytes = label.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<'
            && let Some(skip) = br_len(&label[i..])
        {
            normalized.push('\n');
            i += skip;
            continue;
        }
        let ch = label[i..].chars().next().unwrap_or('\u{FFFD}');
        normalized.push(ch);
        i += ch.len_utf8();
    }
    normalized
        .split('\n')
        .map(|line| line.trim().to_string())
        .collect()
}

fn br_len(rest: &str) -> Option<usize> {
    let head: String = rest
        .chars()
        .take(6)
        .collect::<String>()
        .to_ascii_lowercase();
    if head.starts_with("<br/>") {
        return Some("<br/>".len());
    }
    if head.starts_with("<br />") {
        return Some("<br />".len());
    }
    if head.starts_with("<br>") {
        return Some("<br>".len());
    }
    None
}

pub(crate) fn arrow_head(x1: i32, y1: i32, x2: i32, y2: i32) -> [(i32, i32); 3] {
    let dx = i64::from(x2 - x1);
    let dy = i64::from(y2 - y1);
    let len = isqrt(dx * dx + dy * dy).max(1);
    let bx = i64::from(x2) - dx * 9 / len;
    let by = i64::from(y2) - dy * 9 / len;
    let p1x = bx + (-dy) * 4 / len;
    let p1y = by + dx * 4 / len;
    let p2x = bx - (-dy) * 4 / len;
    let p2y = by - dx * 4 / len;
    [
        (x2, y2),
        (i32_from_i64(p1x), i32_from_i64(p1y)),
        (i32_from_i64(p2x), i32_from_i64(p2y)),
    ]
}

/// Pull the line end back so the stroke stops at the base of the arrowhead.
pub(crate) fn shorten(x1: i32, y1: i32, x2: i32, y2: i32, by: i64) -> (i32, i32) {
    let dx = i64::from(x2 - x1);
    let dy = i64::from(y2 - y1);
    let len = isqrt(dx * dx + dy * dy);
    if len <= by {
        return (x2, y2);
    }
    let nx = i64::from(x2) - dx * by / len;
    let ny = i64::from(y2) - dy * by / len;
    (i32_from_i64(nx), i32_from_i64(ny))
}

fn i32_from_i64(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (n + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
