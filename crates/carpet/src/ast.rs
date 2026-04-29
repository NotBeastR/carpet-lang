use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let {
        name: String,
        value: Expression,
        span: Span,
    },
    Reassign {
        name: String,
        value: Expression,
        span: Span,
    },
    Say {
        value: Expression,
        span: Span,
    },
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::Let { span, .. }
            | Statement::Reassign { span, .. }
            | Statement::Say { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Number {
        value: f64,
        span: Span,
    },
    StringLit {
        value: String,
        span: Span,
    },
    Identifier {
        name: String,
        span: Span,
    },
    BinaryOp {
        op: BinOp,
        left: Box<Expression>,
        right: Box<Expression>,
        span: Span,
    },
    UnaryNeg {
        expr: Box<Expression>,
        span: Span,
    },
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Number { span, .. }
            | Expression::StringLit { span, .. }
            | Expression::Identifier { span, .. }
            | Expression::BinaryOp { span, .. }
            | Expression::UnaryNeg { span, .. } => *span,
        }
    }
}
