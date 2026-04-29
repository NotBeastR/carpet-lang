pub type VReg = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Number,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Number(f64),
    String(StringId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IROp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Const {
        dest: VReg,
        value: Constant,
    },
    BinOp {
        dest: VReg,
        op: IROp,
        left: VReg,
        right: VReg,
    },
    Neg {
        dest: VReg,
        src: VReg,
    },
    Call {
        func: BuiltinFunc,
        args: Vec<VReg>,
    },
    StoreVar {
        name: String,
        src: VReg,
    },
    LoadVar {
        dest: VReg,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinFunc {
    SayNumber,
    SayString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Return,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone)]
pub struct StringConstant {
    pub id: StringId,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
    pub strings: Vec<StringConstant>,
    pub vreg_types: Vec<ValueType>,
}

impl Module {
    pub fn vreg_type(&self, vreg: VReg) -> ValueType {
        self.vreg_types[vreg as usize]
    }
}
