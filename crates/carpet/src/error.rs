use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone)]
pub struct CarpetError {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    UnexpectedCharacter,
    UnterminatedString,
    InvalidNumber,
    UnexpectedToken,
    UndefinedVariable,
    TypeMismatch,
    DivisionByZero,
}

impl CarpetError {
    pub fn new(kind: ErrorKind, message: String, span: Span) -> Self {
        Self {
            kind,
            message,
            span,
        }
    }

    pub fn format_with_source(&self, source: &str, filename: &str) -> String {
        let line_content = source
            .lines()
            .nth((self.span.line - 1) as usize)
            .unwrap_or("");
        let pointer = " ".repeat(self.span.column.saturating_sub(1) as usize) + "^";

        format!(
            "error: {}\n  --> {}:{}:{}\n   |\n{:>3}| {}\n   | {}\n",
            self.message,
            filename,
            self.span.line,
            self.span.column,
            self.span.line,
            line_content,
            pointer,
        )
    }
}

impl fmt::Display for CarpetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error at {}:{}: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

impl std::error::Error for CarpetError {}
