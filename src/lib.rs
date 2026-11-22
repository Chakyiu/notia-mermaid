//! Local Mermaid renderer for Notia.
//!
//! The host passes fence text and gets SVG bytes or a [`RenderError`].
//! This crate has no GPUI or QuickJS types, so the app can call it from the
//! reading view and unit-test it on its own.
//!
//! v1 draws `flowchart` / `graph` (TD, TB, LR) and `sequenceDiagram`.
//! Anything else returns an error whose message starts with `unsupported:`.

mod error;
mod flowchart;
mod limits;
mod scan;
mod sequence;
mod svg;

pub use error::RenderError;

/// Object-safe renderer. Input is Mermaid source; output is SVG bytes.
pub trait MermaidRenderer {
    fn render(&self, source: &str) -> Result<Vec<u8>, RenderError>;
}

/// v1 local renderer: flowchart and sequenceDiagram.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalRenderer;

impl MermaidRenderer for LocalRenderer {
    fn render(&self, source: &str) -> Result<Vec<u8>, RenderError> {
        render(source)
    }
}

/// Render `source` to SVG bytes.
///
/// # Errors
///
/// Returns [`RenderError`] for an empty diagram, a syntax error, an unsupported
/// statement, or a diagram over the size caps in the README.
pub fn render(source: &str) -> Result<Vec<u8>, RenderError> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    limits::check_source(source)?;
    match classify(source)? {
        Kind::Flowchart => flowchart::render(source),
        Kind::Sequence => sequence::render(source),
    }
}

enum Kind {
    Flowchart,
    Sequence,
}

fn classify(source: &str) -> Result<Kind, RenderError> {
    for (idx, raw) in source.lines().enumerate() {
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if scan::starts_with_word(line, "sequenceDiagram") {
            let rest = line["sequenceDiagram".len()..].trim();
            if rest.is_empty() {
                return Ok(Kind::Sequence);
            }
            return Err(RenderError::invalid(
                line_no,
                "unexpected text after sequenceDiagram",
                line,
            ));
        }
        if scan::starts_with_word(line, "flowchart") || scan::starts_with_word(line, "graph") {
            return Ok(Kind::Flowchart);
        }
        return Err(RenderError::unsupported(line_no, line));
    }
    Err(RenderError::empty())
}
