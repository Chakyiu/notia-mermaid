use std::fmt;

/// Failure from [`crate::render`]. Library paths return this; they do not panic
/// on diagram text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderError {
    /// What went wrong. Unsupported syntax starts with `unsupported:`.
    pub message: String,
    /// 1-based source line, when the failure is tied to one.
    pub line: Option<u32>,
    /// The statement that was rejected, when there is one.
    pub statement: Option<String>,
}

impl RenderError {
    pub(crate) fn invalid(line: u32, message: impl Into<String>, statement: &str) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
            statement: Some(clip(statement)),
        }
    }

    pub(crate) fn unsupported(line: u32, statement: &str) -> Self {
        let statement = clip(statement);
        Self {
            message: format!("unsupported: {statement}"),
            line: Some(line),
            statement: Some(statement),
        }
    }

    pub(crate) fn limit(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
            statement: None,
        }
    }

    pub(crate) fn limit_at(line: u32, message: impl Into<String>, statement: &str) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
            statement: Some(clip(statement)),
        }
    }

    pub(crate) fn empty() -> Self {
        Self {
            message: "empty diagram".to_string(),
            line: None,
            statement: None,
        }
    }
}

fn clip(statement: &str) -> String {
    const MAX: usize = 300;
    let mut out = String::new();
    for (i, c) in statement.chars().enumerate() {
        if i == MAX {
            out.push_str("...");
            break;
        }
        out.push(c);
    }
    out
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {line})")?;
        }
        Ok(())
    }
}

impl std::error::Error for RenderError {}
