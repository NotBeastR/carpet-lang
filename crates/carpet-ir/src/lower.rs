use std::collections::HashMap;

use carpet::ast::{BinOp, Expression, Program, Statement};
use carpet::error::{CarpetError, ErrorKind};

use crate::ssa::{
    BasicBlock, BuiltinFunc, Constant, Function, IROp, Instruction, Module, StringConstant,
    StringId, Terminator, VReg, ValueType,
};

pub struct Lowerer {
    next_vreg: u32,
    variables: HashMap<String, (VReg, ValueType)>,
    instructions: Vec<Instruction>,
    strings: Vec<StringConstant>,
    next_string_id: u32,
    vreg_types: Vec<ValueType>,
}

impl Default for Lowerer {
    fn default() -> Self {
        Self::new()
    }
}

impl Lowerer {
    pub fn new() -> Self {
        Self {
            next_vreg: 0,
            variables: HashMap::new(),
            instructions: Vec::new(),
            strings: Vec::new(),
            next_string_id: 0,
            vreg_types: Vec::new(),
        }
    }

    pub fn lower(mut self, program: &Program) -> Result<Module, CarpetError> {
        for stmt in &program.statements {
            self.lower_statement(stmt)?;
        }

        let block = BasicBlock {
            label: "entry".into(),
            instructions: self.instructions,
            terminator: Terminator::Return,
        };

        let function = Function {
            name: "__carpet_main".into(),
            blocks: vec![block],
        };

        Ok(Module {
            functions: vec![function],
            strings: self.strings,
            vreg_types: self.vreg_types,
        })
    }

    fn alloc_vreg(&mut self, ty: ValueType) -> VReg {
        let vreg = self.next_vreg;
        self.next_vreg += 1;
        self.vreg_types.push(ty);
        vreg
    }

    fn alloc_string(&mut self, value: &str) -> StringId {
        let id = StringId(self.next_string_id);
        self.next_string_id += 1;
        self.strings.push(StringConstant {
            id,
            value: value.to_string(),
        });
        id
    }

    fn lower_statement(&mut self, stmt: &Statement) -> Result<(), CarpetError> {
        match stmt {
            Statement::Let { name, value, span } => {
                let (vreg, ty) = self.lower_expression(value)?;
                if self.variables.contains_key(name) {
                    return Err(CarpetError::new(
                        ErrorKind::UndefinedVariable,
                        format!("variable '{}' is already defined", name),
                        *span,
                    ));
                }
                self.instructions.push(Instruction::StoreVar {
                    name: name.clone(),
                    src: vreg,
                });
                self.variables.insert(name.clone(), (vreg, ty));
                Ok(())
            }
            Statement::Reassign { name, value, span } => {
                let old_type = match self.variables.get(name) {
                    Some((_, ty)) => *ty,
                    None => {
                        return Err(CarpetError::new(
                            ErrorKind::UndefinedVariable,
                            format!("variable '{}' is not defined", name),
                            *span,
                        ));
                    }
                };
                let (vreg, new_type) = self.lower_expression(value)?;
                if old_type != new_type {
                    return Err(CarpetError::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "cannot reassign variable '{}' from {:?} to {:?}",
                            name, old_type, new_type
                        ),
                        *span,
                    ));
                }
                self.instructions.push(Instruction::StoreVar {
                    name: name.clone(),
                    src: vreg,
                });
                self.variables.insert(name.clone(), (vreg, new_type));
                Ok(())
            }
            Statement::Say { value, .. } => {
                let (vreg, ty) = self.lower_expression(value)?;
                let func = match ty {
                    ValueType::Number => BuiltinFunc::SayNumber,
                    ValueType::String => BuiltinFunc::SayString,
                };
                self.instructions.push(Instruction::Call {
                    func,
                    args: vec![vreg],
                });
                Ok(())
            }
        }
    }

    fn lower_expression(&mut self, expr: &Expression) -> Result<(VReg, ValueType), CarpetError> {
        match expr {
            Expression::Number { value, .. } => {
                let vreg = self.alloc_vreg(ValueType::Number);
                self.instructions.push(Instruction::Const {
                    dest: vreg,
                    value: Constant::Number(*value),
                });
                Ok((vreg, ValueType::Number))
            }
            Expression::StringLit { value, .. } => {
                let sid = self.alloc_string(value);
                let vreg = self.alloc_vreg(ValueType::String);
                self.instructions.push(Instruction::Const {
                    dest: vreg,
                    value: Constant::String(sid),
                });
                Ok((vreg, ValueType::String))
            }
            Expression::Identifier { name, span } => {
                let (_, ty) = self.variables.get(name).copied().ok_or_else(|| {
                    CarpetError::new(
                        ErrorKind::UndefinedVariable,
                        format!("undefined variable '{}'", name),
                        *span,
                    )
                })?;
                let vreg = self.alloc_vreg(ty);
                self.instructions.push(Instruction::LoadVar {
                    dest: vreg,
                    name: name.clone(),
                });
                Ok((vreg, ty))
            }
            Expression::BinaryOp {
                op,
                left,
                right,
                span,
            } => {
                let (left_vreg, left_ty) = self.lower_expression(left)?;
                let (right_vreg, right_ty) = self.lower_expression(right)?;
                if left_ty != ValueType::Number || right_ty != ValueType::Number {
                    return Err(CarpetError::new(
                        ErrorKind::TypeMismatch,
                        "arithmetic operations require number operands".into(),
                        *span,
                    ));
                }
                let ir_op = match op {
                    BinOp::Add => IROp::Add,
                    BinOp::Sub => IROp::Sub,
                    BinOp::Mul => IROp::Mul,
                    BinOp::Div => IROp::Div,
                    BinOp::Mod => IROp::Mod,
                };
                let dest = self.alloc_vreg(ValueType::Number);
                self.instructions.push(Instruction::BinOp {
                    dest,
                    op: ir_op,
                    left: left_vreg,
                    right: right_vreg,
                });
                Ok((dest, ValueType::Number))
            }
            Expression::UnaryNeg { expr, span } => {
                let (src, ty) = self.lower_expression(expr)?;
                if ty != ValueType::Number {
                    return Err(CarpetError::new(
                        ErrorKind::TypeMismatch,
                        "negation requires a number operand".into(),
                        *span,
                    ));
                }
                let dest = self.alloc_vreg(ValueType::Number);
                self.instructions.push(Instruction::Neg { dest, src });
                Ok((dest, ValueType::Number))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use carpet::lexer::Lexer;
    use carpet::parser::Parser;

    fn lower_source(input: &str) -> Module {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let lowerer = Lowerer::new();
        lowerer.lower(&program).unwrap()
    }

    #[test]
    fn test_lower_let() {
        let module = lower_source("let x is 42");
        assert_eq!(module.functions.len(), 1);
        let block = &module.functions[0].blocks[0];
        assert_eq!(block.instructions.len(), 2);
        assert!(matches!(&block.instructions[0], Instruction::Const { .. }));
        assert!(
            matches!(&block.instructions[1], Instruction::StoreVar { name, .. } if name == "x")
        );
    }

    #[test]
    fn test_lower_say_number() {
        let module = lower_source("say(42)");
        let block = &module.functions[0].blocks[0];
        assert!(matches!(
            &block.instructions[1],
            Instruction::Call {
                func: BuiltinFunc::SayNumber,
                ..
            }
        ));
    }

    #[test]
    fn test_lower_say_string() {
        let module = lower_source("say(\"hello\")");
        let block = &module.functions[0].blocks[0];
        assert!(matches!(
            &block.instructions[1],
            Instruction::Call {
                func: BuiltinFunc::SayString,
                ..
            }
        ));
        assert_eq!(module.strings.len(), 1);
        assert_eq!(module.strings[0].value, "hello");
    }

    #[test]
    fn test_lower_arithmetic() {
        let module = lower_source("say(1 + 2 * 3)");
        let block = &module.functions[0].blocks[0];
        let binop_count = block
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::BinOp { .. }))
            .count();
        assert_eq!(binop_count, 2);
    }

    #[test]
    fn test_lower_reassign() {
        let module = lower_source("let x is 10\nx be 20");
        let block = &module.functions[0].blocks[0];
        let store_count = block
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::StoreVar { .. }))
            .count();
        assert_eq!(store_count, 2);
    }

    #[test]
    fn test_undefined_variable_error() {
        let mut lexer = Lexer::new("say(x)");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let lowerer = Lowerer::new();
        let result = lowerer.lower(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_type_mismatch_error() {
        let mut lexer = Lexer::new("let x is \"hello\"\nx be 42");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let lowerer = Lowerer::new();
        let result = lowerer.lower(&program);
        assert!(result.is_err());
    }
}
