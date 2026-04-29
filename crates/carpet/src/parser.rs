use crate::ast::{BinOp, Expression, Program, Statement};
use crate::error::{CarpetError, ErrorKind};
use crate::span::Span;
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, CarpetError> {
        let mut statements = Vec::new();

        self.skip_newlines();

        while !self.is_at_end() {
            let stmt = self.parse_statement()?;
            statements.push(stmt);
            self.skip_newlines();
        }

        Ok(Program { statements })
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &Token {
        let token = &self.tokens[self.pos];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    fn is_at_end(&self) -> bool {
        self.tokens[self.pos].kind == TokenKind::Eof
    }

    fn skip_newlines(&mut self) {
        while self.tokens[self.pos].kind == TokenKind::Newline {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<&Token, CarpetError> {
        let token = &self.tokens[self.pos];
        if std::mem::discriminant(&token.kind) == std::mem::discriminant(expected) {
            Ok(self.advance())
        } else {
            Err(CarpetError::new(
                ErrorKind::UnexpectedToken,
                format!("expected {:?}, found {:?}", expected, token.kind),
                token.span,
            ))
        }
    }

    fn expect_newline_or_eof(&mut self) -> Result<(), CarpetError> {
        let token = &self.tokens[self.pos];
        match token.kind {
            TokenKind::Newline => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            _ => Err(CarpetError::new(
                ErrorKind::UnexpectedToken,
                format!("expected end of statement, found {:?}", token.kind),
                token.span,
            )),
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, CarpetError> {
        let token = self.peek();
        match token.kind.clone() {
            TokenKind::Let => self.parse_let(),
            TokenKind::Say => self.parse_say(),
            TokenKind::Identifier(name) => {
                let span = token.span;
                self.advance();
                if self.peek().kind == TokenKind::Be {
                    self.parse_reassign(name, span)
                } else {
                    Err(CarpetError::new(
                        ErrorKind::UnexpectedToken,
                        format!(
                            "expected 'be' after identifier '{}', found {:?}",
                            name,
                            self.peek().kind
                        ),
                        self.peek().span,
                    ))
                }
            }
            _ => Err(CarpetError::new(
                ErrorKind::UnexpectedToken,
                format!("expected statement, found {:?}", token.kind),
                token.span,
            )),
        }
    }

    fn parse_let(&mut self) -> Result<Statement, CarpetError> {
        let start_span = self.advance().span; // consume 'let'

        let name_token = self.peek().clone();
        let name = match name_token.kind {
            TokenKind::Identifier(ref n) => n.clone(),
            _ => {
                return Err(CarpetError::new(
                    ErrorKind::UnexpectedToken,
                    format!(
                        "expected identifier after 'let', found {:?}",
                        name_token.kind
                    ),
                    name_token.span,
                ));
            }
        };
        self.advance();

        self.expect(&TokenKind::Is)?;

        let value = self.parse_expression()?;
        let span = start_span.merge(value.span());

        self.expect_newline_or_eof()?;

        Ok(Statement::Let { name, value, span })
    }

    fn parse_reassign(&mut self, name: String, name_span: Span) -> Result<Statement, CarpetError> {
        self.advance(); // consume 'be'

        let value = self.parse_expression()?;
        let span = name_span.merge(value.span());

        self.expect_newline_or_eof()?;

        Ok(Statement::Reassign { name, value, span })
    }

    fn parse_say(&mut self) -> Result<Statement, CarpetError> {
        let start_span = self.advance().span; // consume 'say'

        self.expect(&TokenKind::LeftParen)?;
        let value = self.parse_expression()?;
        let end_token = self.expect(&TokenKind::RightParen)?;
        let span = start_span.merge(end_token.span);

        self.expect_newline_or_eof()?;

        Ok(Statement::Say { value, span })
    }

    fn parse_expression(&mut self) -> Result<Expression, CarpetError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<Expression, CarpetError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            let span = left.span().merge(right.span());
            left = Expression::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, CarpetError> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let span = left.span().merge(right.span());
            left = Expression::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expression, CarpetError> {
        if self.peek().kind == TokenKind::Minus {
            let start_span = self.advance().span;
            let expr = self.parse_unary()?;
            let span = start_span.merge(expr.span());
            return Ok(Expression::UnaryNeg {
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, CarpetError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Number(value) => {
                self.advance();
                Ok(Expression::Number {
                    value,
                    span: token.span,
                })
            }
            TokenKind::StringLiteral(ref value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression::StringLit {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Identifier(ref name) => {
                let name = name.clone();
                self.advance();
                Ok(Expression::Identifier {
                    name,
                    span: token.span,
                })
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RightParen)?;
                Ok(expr)
            }
            _ => Err(CarpetError::new(
                ErrorKind::UnexpectedToken,
                format!("expected expression, found {:?}", token.kind),
                token.span,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(input: &str) -> Result<Program, CarpetError> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_let_number() {
        let prog = parse("let x is 42").unwrap();
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(&prog.statements[0], Statement::Let { name, .. } if name == "x"));
    }

    #[test]
    fn test_let_string() {
        let prog = parse("let msg is \"hello\"").unwrap();
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(&prog.statements[0], Statement::Let { name, .. } if name == "msg"));
    }

    #[test]
    fn test_say_expression() {
        let prog = parse("say(1 + 2 * 3)").unwrap();
        assert_eq!(prog.statements.len(), 1);
        assert!(matches!(&prog.statements[0], Statement::Say { .. }));
    }

    #[test]
    fn test_reassignment() {
        let prog = parse("let x is 1\nx be 2").unwrap();
        assert_eq!(prog.statements.len(), 2);
        assert!(matches!(&prog.statements[1], Statement::Reassign { name, .. } if name == "x"));
    }

    #[test]
    fn test_unary_neg() {
        let prog = parse("say(-5)").unwrap();
        assert_eq!(prog.statements.len(), 1);
        if let Statement::Say { value, .. } = &prog.statements[0] {
            assert!(matches!(value, Expression::UnaryNeg { .. }));
        } else {
            panic!("expected Say");
        }
    }

    #[test]
    fn test_multi_statement() {
        let prog = parse("let x is 10\nlet y is 20\nsay(x + y)").unwrap();
        assert_eq!(prog.statements.len(), 3);
    }

    #[test]
    fn test_parenthesized_expression() {
        let prog = parse("say((1 + 2) * 3)").unwrap();
        assert_eq!(prog.statements.len(), 1);
    }
}
