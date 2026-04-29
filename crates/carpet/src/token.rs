use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Let,
    Is,
    Be,
    Say,

    Number(f64),
    StringLiteral(String),

    Identifier(String),

    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    LeftParen,
    RightParen,

    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
