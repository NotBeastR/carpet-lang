use crate::error::{CarpetError, ErrorKind};
use crate::span::Span;
use crate::token::{Token, TokenKind};

pub struct Lexer<'src> {
    source: &'src [u8],
    pos: usize,
    line: u32,
    column: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source: source.as_bytes(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, CarpetError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(ch) = self.peek() {
            match ch {
                b' ' | b'\t' | b'\r' => {
                    self.advance();
                }
                b'#' => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, CarpetError> {
        self.skip_whitespace_and_comments();

        let start = self.pos as u32;
        let line = self.line;
        let column = self.column;

        let Some(ch) = self.advance() else {
            return Ok(Token::new(
                TokenKind::Eof,
                Span::new(start, start, line, column),
            ));
        };

        match ch {
            b'\n' => Ok(Token::new(
                TokenKind::Newline,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'+' => Ok(Token::new(
                TokenKind::Plus,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'-' => Ok(Token::new(
                TokenKind::Minus,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'*' => Ok(Token::new(
                TokenKind::Star,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'/' => Ok(Token::new(
                TokenKind::Slash,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'%' => Ok(Token::new(
                TokenKind::Percent,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'(' => Ok(Token::new(
                TokenKind::LeftParen,
                Span::new(start, self.pos as u32, line, column),
            )),
            b')' => Ok(Token::new(
                TokenKind::RightParen,
                Span::new(start, self.pos as u32, line, column),
            )),
            b'"' => self.read_string(start, line, column),
            ch if ch.is_ascii_digit() || ch == b'.' => self.read_number(start, line, column),
            ch if ch.is_ascii_alphabetic() || ch == b'_' => {
                self.read_identifier(start, line, column)
            }
            _ => Err(CarpetError::new(
                ErrorKind::UnexpectedCharacter,
                format!("unexpected character '{}'", ch as char),
                Span::new(start, self.pos as u32, line, column),
            )),
        }
    }

    fn read_string(&mut self, start: u32, line: u32, column: u32) -> Result<Token, CarpetError> {
        let mut value = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    return Err(CarpetError::new(
                        ErrorKind::UnterminatedString,
                        "unterminated string literal".into(),
                        Span::new(start, self.pos as u32, line, column),
                    ));
                }
                Some(b'"') => {
                    self.advance();
                    break;
                }
                Some(b'\\') => {
                    self.advance();
                    match self.peek() {
                        Some(b'n') => {
                            self.advance();
                            value.push(b'\n');
                        }
                        Some(b't') => {
                            self.advance();
                            value.push(b'\t');
                        }
                        Some(b'\\') => {
                            self.advance();
                            value.push(b'\\');
                        }
                        Some(b'"') => {
                            self.advance();
                            value.push(b'"');
                        }
                        Some(b'r') => {
                            self.advance();
                            value.push(b'\r');
                        }
                        Some(b'0') => {
                            self.advance();
                            value.push(0);
                        }
                        _ => {
                            return Err(CarpetError::new(
                                ErrorKind::UnexpectedCharacter,
                                "invalid escape sequence".into(),
                                Span::new(start, self.pos as u32, line, column),
                            ));
                        }
                    }
                }
                Some(ch) => {
                    self.advance();
                    value.push(ch);
                }
            }
        }
        let s = String::from_utf8(value).map_err(|_| {
            CarpetError::new(
                ErrorKind::UnexpectedCharacter,
                "invalid UTF-8 in string literal".into(),
                Span::new(start, self.pos as u32, line, column),
            )
        })?;
        Ok(Token::new(
            TokenKind::StringLiteral(s),
            Span::new(start, self.pos as u32, line, column),
        ))
    }

    fn read_number(&mut self, start: u32, line: u32, column: u32) -> Result<Token, CarpetError> {
        let num_start = (start as usize).max(self.pos - 1);
        let mut has_dot = self.source[num_start] == b'.';

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance();
            } else if ch == b'.' && !has_dot {
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        let text = std::str::from_utf8(&self.source[num_start..self.pos]).map_err(|_| {
            CarpetError::new(
                ErrorKind::InvalidNumber,
                "invalid number literal".into(),
                Span::new(start, self.pos as u32, line, column),
            )
        })?;

        let value: f64 = text.parse().map_err(|_| {
            CarpetError::new(
                ErrorKind::InvalidNumber,
                format!("invalid number literal '{text}'"),
                Span::new(start, self.pos as u32, line, column),
            )
        })?;

        Ok(Token::new(
            TokenKind::Number(value),
            Span::new(start, self.pos as u32, line, column),
        ))
    }

    fn read_identifier(
        &mut self,
        start: u32,
        line: u32,
        column: u32,
    ) -> Result<Token, CarpetError> {
        let ident_start = (start as usize).max(self.pos - 1);

        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                self.advance();
            } else {
                break;
            }
        }

        let text = std::str::from_utf8(&self.source[ident_start..self.pos]).map_err(|_| {
            CarpetError::new(
                ErrorKind::UnexpectedCharacter,
                "invalid identifier".into(),
                Span::new(start, self.pos as u32, line, column),
            )
        })?;

        let kind = match text {
            "let" => TokenKind::Let,
            "is" => TokenKind::Is,
            "be" => TokenKind::Be,
            "say" => TokenKind::Say,
            _ => TokenKind::Identifier(text.to_string()),
        };

        Ok(Token::new(
            kind,
            Span::new(start, self.pos as u32, line, column),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_let_statement() {
        let mut lexer = Lexer::new("let x is 42");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert!(matches!(tokens[1].kind, TokenKind::Identifier(ref s) if s == "x"));
        assert_eq!(tokens[2].kind, TokenKind::Is);
        assert!(matches!(tokens[3].kind, TokenKind::Number(n) if n == 42.0));
    }

    #[test]
    fn test_say_string() {
        let mut lexer = Lexer::new("say(\"hello world\")");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Say);
        assert_eq!(tokens[1].kind, TokenKind::LeftParen);
        assert!(matches!(tokens[2].kind, TokenKind::StringLiteral(ref s) if s == "hello world"));
        assert_eq!(tokens[3].kind, TokenKind::RightParen);
    }

    #[test]
    fn test_arithmetic() {
        let mut lexer = Lexer::new("1 + 2 * 3");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Number(n) if n == 1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert!(matches!(tokens[2].kind, TokenKind::Number(n) if n == 2.0));
        assert_eq!(tokens[3].kind, TokenKind::Star);
        assert!(matches!(tokens[4].kind, TokenKind::Number(n) if n == 3.0));
    }

    #[test]
    fn test_reassignment() {
        let mut lexer = Lexer::new("x be 10");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(ref s) if s == "x"));
        assert_eq!(tokens[1].kind, TokenKind::Be);
        assert!(matches!(tokens[2].kind, TokenKind::Number(n) if n == 10.0));
    }

    #[test]
    fn test_escape_sequences() {
        let mut lexer = Lexer::new("\"hello\\nworld\"");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::StringLiteral(ref s) if s == "hello\nworld"));
    }

    #[test]
    fn test_decimal_number() {
        let mut lexer = Lexer::new("3.14");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Number(n) if (n - 3.14).abs() < f64::EPSILON));
    }

    #[test]
    fn test_comments() {
        let mut lexer = Lexer::new("# this is a comment\nlet x is 5");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Newline);
        assert_eq!(tokens[1].kind, TokenKind::Let);
    }
}
