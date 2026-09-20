use crate::{
    Compiled,
    basic_blocks::{Instruction as IrInstruction, Value as IrValue},
    parse::{BinaryOp, UnaryOp},
    ssa::{BlockTerminator, Ssa},
};

/// # Panics
/// Panics if `usize` doesn't fit in a u64
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn compile(compiled: &Compiled<'_, Ssa>) -> Vec<u8> {
    let ssa = compiled.result();

    let mut bytecode = vec![];

    bytecode.extend_from_slice(
        u64::try_from(compiled.result().max_registers())
            .expect("128-bit usize not allowed!  sorry!")
            .to_le_bytes()
            .as_slice(),
    );

    bytecode.extend_from_slice(
        u64::try_from(compiled.result().blocks().len())
            .expect("128-bit usize not allowed!  sorry!")
            .to_le_bytes()
            .as_slice(),
    );

    for _ in ssa.blocks() {
        bytecode.extend_from_slice(
            u64::try_from(bytecode.len())
                .expect("128-bit usize not allowed!  sorry!")
                .to_le_bytes()
                .as_slice(),
        );
    }

    for (b, block) in ssa.blocks().iter().enumerate() {
        let ip = bytecode.len();

        bytecode[(16 + b * 8)..(16 + b * 8 + 8)].copy_from_slice(
            u64::try_from(ip)
                .expect("128-bit usize not allowed!  sorry!")
                .to_le_bytes()
                .as_slice(),
        );

        for instruction in block.instructions() {
            bytecode.append(&mut instruction.to_bytes(compiled));
        }

        match block.terminator() {
            BlockTerminator::Jump(jump_to) => {
                bytecode.push(0xC0);

                bytecode.extend_from_slice(
                    u64::try_from(16 + usize::from(jump_to.block()) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytecode.extend_from_slice(
                    u64::try_from(jump_to.arguments().len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for (argument, to) in jump_to
                    .arguments()
                    .iter()
                    .zip(compiled.result().blocks()[usize::from(jump_to.block())].parameters())
                {
                    bytecode.append(&mut to.to_bytes(compiled));
                    bytecode.append(&mut argument.to_bytes(compiled));
                }
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                bytecode.push(0xC1);

                bytecode.append(&mut condition.to_bytes(compiled));

                bytecode.extend_from_slice(
                    u64::try_from(16 + usize::from(when_true.block()) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytecode.extend_from_slice(
                    u64::try_from(16 + usize::from(otherwise.block()) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytecode.extend_from_slice(
                    u64::try_from(when_true.arguments().len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for (argument, to) in when_true
                    .arguments()
                    .iter()
                    .zip(compiled.result().blocks()[usize::from(when_true.block())].parameters())
                {
                    bytecode.append(&mut to.to_bytes(compiled));
                    bytecode.append(&mut argument.to_bytes(compiled));
                }

                bytecode.extend_from_slice(
                    u64::try_from(otherwise.arguments().len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for (argument, to) in otherwise
                    .arguments()
                    .iter()
                    .zip(compiled.result().blocks()[usize::from(otherwise.block())].parameters())
                {
                    bytecode.append(&mut to.to_bytes(compiled));
                    bytecode.append(&mut argument.to_bytes(compiled));
                }
            }
            BlockTerminator::Return(value) => {
                bytecode.push(0xC2);

                bytecode.append(&mut value.to_bytes(compiled));
            }
        }
    }

    bytecode
}

macro_rules! int_enum {
    (
        $v:vis $name:ident as $int:ty ;
        $($variant:ident => $value:literal,)+
    ) => {
        #[derive(Debug)]
        $v enum $name {
            $($variant,)+
        }

        impl From<$name> for $int {
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => $value,)+
                }
            }
        }

        impl TryFrom<$int> for $name {
            type Error = $int;

            fn try_from(value: $int) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(value),
                }
            }
        }
    };
}

int_enum! {
    pub OpCode as u8 ;
    Not => 0x20,
    Negate => 0x21,
    Multiply => 0x40,
    Divide => 0x41,
    Remainder => 0x42,
    Add => 0x43,
    Subtract => 0x44,
    Less => 0x45,
    Greater => 0x46,
    LessOrEqual => 0x47,
    GreaterOrEqual => 0x48,
    Equal => 0x49,
    NotEqual => 0x4A,
    And => 0x4B,
    Or => 0x4C,
    Assign => 0x60,
    Push => 0x80,
    Call => 0xA0,
    Access => 0xB0,
    AccessAssign => 0xB1,
    Jump => 0xC0,
    Branch => 0xC1,
    Return => 0xC2,
}

int_enum! {
    pub TypeId as u8 ;
    I64 => 0x00,
    F64 => 0x01,
    Boolean => 0x02,
    Unit => 0x03,
    Fn => 0x04,
    NativeFn => 0x05,
    StackOffset => 0x06,
    Register => 0x07,
    Compound => 0x08,
    TaggedCompound => 0x09,
}

int_enum! {
    pub NativeFn as u16 ;
    PrintI64 => 0x00_00,
    PrintF64 => 0x00_01,
    PrintBool => 0x00_02,
    PrintUnit => 0x00_03,
}

trait Assemble {
    fn to_bytes(&self, compiled: &Compiled<'_, Ssa>) -> Vec<u8>;
}

impl Assemble for IrInstruction {
    #[allow(clippy::too_many_lines)]
    fn to_bytes(&self, compiled: &Compiled<'_, Ssa>) -> Vec<u8> {
        match self {
            Self::NoOp => vec![],
            Self::Unary {
                op,
                operand,
                temporary,
            } => {
                let mut bytes = vec![
                    match op {
                        UnaryOp::Not => OpCode::Not,
                        UnaryOp::Negate => OpCode::Negate,
                    }
                    .into(),
                ];

                bytes.append(&mut temporary.to_bytes(compiled));

                bytes.append(&mut operand.to_bytes(compiled));

                bytes
            }
            Self::Binary {
                op,
                lhs,
                rhs,
                temporary,
            } => {
                let mut bytes = match op {
                    BinaryOp::PathAccess | BinaryOp::Access | BinaryOp::Assign => vec![],
                    _ => vec![u8::from(match op {
                        BinaryOp::Multiply => OpCode::Multiply,
                        BinaryOp::Divide => OpCode::Divide,
                        BinaryOp::Remainder => OpCode::Remainder,
                        BinaryOp::Add => OpCode::Add,
                        BinaryOp::Subtract => OpCode::Subtract,
                        BinaryOp::Less => OpCode::Less,
                        BinaryOp::Greater => OpCode::Greater,
                        BinaryOp::LessOrEqual => OpCode::LessOrEqual,
                        BinaryOp::GreaterOrEqual => OpCode::GreaterOrEqual,
                        BinaryOp::Equal => OpCode::Equal,
                        BinaryOp::NotEqual => OpCode::NotEqual,
                        BinaryOp::And => OpCode::And,
                        BinaryOp::Or => OpCode::Or,
                        _ => unreachable!("unhandled binary operator"),
                    })],
                };

                bytes.append(&mut temporary.to_bytes(compiled));

                bytes.append(&mut lhs.to_bytes(compiled));

                bytes.append(&mut rhs.to_bytes(compiled));

                bytes
            }
            Self::Assign { value, to } => {
                let mut bytes = vec![u8::from(OpCode::Assign)];

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::Push(value) => {
                let mut bytes = vec![u8::from(OpCode::Push)];

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::Call {
                callee,
                arity,
                temporary,
            } => {
                let mut bytes = vec![u8::from(OpCode::Call)];

                bytes.append(&mut callee.to_bytes(compiled));

                bytes.extend_from_slice(
                    u64::try_from(*arity)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.append(&mut temporary.to_bytes(compiled));

                bytes
            }
            Self::Access {
                index,
                of,
                temporary,
            } => {
                let mut bytes = vec![u8::from(OpCode::Access)];

                bytes.append(&mut temporary.to_bytes(compiled));

                bytes.extend_from_slice(
                    u64::try_from(*index)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.append(&mut of.to_bytes(compiled));

                bytes
            }
            Self::AccessAssign { index, of, value } => {
                let mut bytes = vec![u8::from(OpCode::AccessAssign)];

                bytes.extend_from_slice(
                    u64::try_from(*index)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.append(&mut of.to_bytes(compiled));

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
        }
    }
}

impl Assemble for IrValue {
    #[allow(clippy::too_many_lines)]
    fn to_bytes(&self, compiled: &Compiled<'_, Ssa>) -> Vec<u8> {
        match self {
            Self::BlockArgument(_) | Self::CallArgument(_) | Self::Address(_) => {
                unreachable!("these are eliminated by register allocation")
            }
            Self::Integer(value) => {
                let mut bytes = vec![u8::from(TypeId::I64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::Float(value) => {
                let mut bytes = vec![u8::from(TypeId::F64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::Boolean(value) => {
                vec![u8::from(TypeId::Boolean), u8::from(*value)]
            }
            Self::Unit | Self::Runtime => {
                vec![u8::from(TypeId::Unit)]
            }
            Self::Fn(block_index) => {
                let mut bytes = vec![u8::from(TypeId::Fn)];

                bytes.extend_from_slice(
                    u64::try_from(16 + usize::from(*block_index) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::NativeFn(span) => {
                let mut bytes = vec![u8::from(TypeId::NativeFn)];

                let source_index = compiled
                    .sources()
                    .iter()
                    .position(|(id, _)| *id == span.source_id())
                    .expect("the source must exist");

                bytes.extend_from_slice(
                    match compiled
                        .sources()
                        .get(source_index)
                        .and_then(|(_, source)| span.lexeme(source))
                    {
                        Some("print_i64") => u16::from(NativeFn::PrintI64).to_le_bytes(),
                        Some("print_f64") => u16::from(NativeFn::PrintF64).to_le_bytes(),
                        Some("print_bool") => u16::from(NativeFn::PrintBool).to_le_bytes(),
                        Some("print_unit") => u16::from(NativeFn::PrintUnit).to_le_bytes(),
                        _ => unreachable!("unknown native function"),
                    }
                    .as_slice(),
                );

                bytes
            }
            Self::StackOffset(offset) => {
                let mut bytes = vec![u8::from(TypeId::StackOffset)];

                bytes.extend_from_slice(
                    u64::try_from(*offset)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::Register(index) => {
                let mut bytes = vec![u8::from(TypeId::Register)];

                bytes.extend_from_slice(
                    u64::try_from(*index)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::Compound(values) => {
                let mut bytes = vec![u8::from(TypeId::Compound)];

                bytes.extend_from_slice(
                    u64::try_from(values.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for value in values {
                    bytes.extend_from_slice(value.to_bytes(compiled).as_slice());
                }

                bytes
            }
            Self::TaggedCompound { tag, fields } => {
                let mut bytes = vec![u8::from(TypeId::TaggedCompound)];

                bytes.extend_from_slice(tag.to_le_bytes().as_slice());

                bytes.extend_from_slice(
                    u64::try_from(fields.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                for value in fields {
                    bytes.extend_from_slice(value.to_bytes(compiled).as_slice());
                }

                bytes
            }
        }
    }
}
