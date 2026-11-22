use crate::error::RenderError;

pub(crate) const MAX_SOURCE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_LINES: usize = 2_000;
pub(crate) const MAX_NODES: usize = 200;
pub(crate) const MAX_EDGES: usize = 400;
pub(crate) const MAX_STATEMENTS: usize = 500;
pub(crate) const MAX_NESTING: usize = 8;
pub(crate) const MAX_LABEL_CHARS: usize = 500;

pub(crate) fn check_source(source: &str) -> Result<(), RenderError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(RenderError::limit(format!(
            "diagram exceeds {MAX_SOURCE_BYTES} bytes"
        )));
    }
    if source.lines().count() > MAX_LINES {
        return Err(RenderError::limit(format!(
            "diagram exceeds {MAX_LINES} lines"
        )));
    }
    Ok(())
}

pub(crate) fn check_label(label: &str, line: u32, statement: &str) -> Result<(), RenderError> {
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(RenderError::invalid(line, "label is too long", statement));
    }
    Ok(())
}

pub(crate) struct Budget {
    statements: usize,
}

impl Budget {
    pub(crate) fn new() -> Self {
        Self { statements: 0 }
    }

    pub(crate) fn bump(&mut self, line: u32, statement: &str) -> Result<(), RenderError> {
        self.statements += 1;
        if self.statements > MAX_STATEMENTS {
            return Err(RenderError::limit_at(
                line,
                format!("diagram exceeds {MAX_STATEMENTS} statements"),
                statement,
            ));
        }
        Ok(())
    }
}
