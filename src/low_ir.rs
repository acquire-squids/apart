use crate::{
    Compiled,
    basic_blocks::{BlockIndex, Instruction as IrInstruction, Value as IrValue},
    parse::{BinaryOp as AstBinaryOp, UnaryOp as AstUnaryOp},
    ssa::{BlockTerminator, Ssa},
};

use std::fmt;

#[must_use]
pub fn lower(compiled: &Compiled<'_, Ssa>) -> Ir<Value> {
    let blocks = compiled
        .result()
        .blocks()
        .iter()
        .map(|block| {
            let mut instructions = block
                .instructions()
                .iter()
                .map(|instruction| lower_instruction(compiled, instruction))
                .collect::<Vec<_>>();

            instructions.push(lower_block_terminator(compiled, block.terminator()));

            Block { instructions }
        })
        .collect::<Vec<_>>();

    Ir {
        blocks,
        max_registers: compiled.result().max_registers(),
        function_count: compiled.result().function_count(),
    }
}

pub struct Ir<T> {
    blocks: Vec<Block<T>>,
    max_registers: usize,
    function_count: usize,
}

impl<T> Ir<T> {
    #[must_use]
    pub const fn new(blocks: Vec<Block<T>>, max_registers: usize, function_count: usize) -> Self {
        Self {
            blocks,
            max_registers,
            function_count,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_block(&self, block_index: BlockIndex) -> Option<&Block<T>> {
        self.blocks.get(usize::from(block_index))
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn blocks(&self) -> &[Block<T>] {
        self.blocks.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn max_registers(&self) -> usize {
        self.max_registers
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
}

pub struct Block<T> {
    instructions: Vec<Instruction<T>>,
}

impl<T> Block<T> {
    #[must_use]
    pub const fn new(instructions: Vec<Instruction<T>>) -> Self {
        Self { instructions }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn instructions(&self) -> &[Instruction<T>] {
        self.instructions.as_slice()
    }
}

impl<T> fmt::Display for Ir<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (b, block) in self.blocks().iter().enumerate() {
            writeln!(f, "{b}:")?;

            write!(f, "{block}")?;
        }

        Ok(())
    }
}

impl<T> fmt::Display for Block<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for instruction in self.instructions() {
            writeln!(f, "    {instruction:?}")?;
        }

        Ok(())
    }
}

crate::int_enum! {
    pub NativeFn as u16 ;
    PrintI64 => 0x00_00,
    PrintF64 => 0x00_01,
    PrintBool => 0x00_02,
    PrintUnit => 0x00_03,
    PrintU8 => 0x00_04,
    PrintI8 => 0x00_05,
    PrintU16 => 0x00_06,
    PrintI16 => 0x00_07,
    PrintU32 => 0x00_08,
    PrintI32 => 0x00_09,
    PrintU64 => 0x00_0A,
}

crate::int_enum! {
    pub UnaryOp as u8 ;
    Not => 0x00,
    Negate => 0x01,
}

crate::int_enum! {
    pub BinaryOp as u8 ;
    Multiply => 0x00,
    Divide => 0x01,
    Remainder => 0x02,
    Add => 0x03,
    Subtract => 0x04,
    Less => 0x05,
    Greater => 0x06,
    LessOrEqual => 0x07,
    GreaterOrEqual => 0x08,
    Equal => 0x09,
    NotEqual => 0x0A,
    And => 0x0B,
    Or => 0x0C,
}

#[derive(Debug)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F64(f64),
    Boolean(bool),
    Unit,
    Fn(BlockIndex),
    NativeFn(NativeFn),
    Compound(Vec<ValueOrLocation<Self>>),
    TaggedCompound {
        fields: Vec<ValueOrLocation<Self>>,
        tag: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueOrLocation<T> {
    Value(T),
    At(Location),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    StackOffset(StackOffset),
    Register(Register),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackOffset(pub usize);

impl From<StackOffset> for usize {
    fn from(value: StackOffset) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Register(pub usize);

impl From<Register> for usize {
    fn from(value: Register) -> Self {
        value.0
    }
}

#[derive(Debug)]
pub enum Instruction<T> {
    NoOp,
    Unary {
        op: UnaryOp,
        operand: ValueOrLocation<T>,
        to: Location,
    },
    Binary {
        op: BinaryOp,
        lhs: ValueOrLocation<T>,
        rhs: ValueOrLocation<T>,
        to: Location,
    },
    Assign {
        to: Location,
        value: ValueOrLocation<T>,
    },
    Push(ValueOrLocation<T>),
    Access {
        index: usize,
        of: ValueOrLocation<T>,
        to: Location,
    },
    AccessAssign {
        index: usize,
        of: ValueOrLocation<T>,
        value: ValueOrLocation<T>,
    },
    Call {
        callee: ValueOrLocation<T>,
        arity: usize,
        to: Location,
    },
    Jump(BlockIndex, Vec<(Location, ValueOrLocation<T>)>),
    Branch {
        condition: ValueOrLocation<T>,
        when_true: (BlockIndex, Vec<(Location, ValueOrLocation<T>)>),
        otherwise: (BlockIndex, Vec<(Location, ValueOrLocation<T>)>),
    },
    Return(ValueOrLocation<T>),
}

#[must_use]
fn lower_block_terminator(
    compiled: &Compiled<'_, Ssa>,
    block_terminator: &BlockTerminator,
) -> Instruction<Value> {
    match block_terminator {
        BlockTerminator::Return(value) => Instruction::Return(lower_value(compiled, value)),
        BlockTerminator::Jump(jump_to) => Instruction::Jump(
            jump_to.block(),
            jump_to
                .arguments()
                .iter()
                .zip(compiled.result().blocks()[usize::from(jump_to.block())].parameters())
                .map(|(argument, to)| {
                    (lower_value_to_location(to), lower_value(compiled, argument))
                })
                .collect::<Vec<_>>(),
        ),
        BlockTerminator::Branch {
            condition,
            when_true,
            otherwise,
        } => Instruction::Branch {
            condition: lower_value(compiled, condition),
            when_true: (
                when_true.block(),
                when_true
                    .arguments()
                    .iter()
                    .zip(compiled.result().blocks()[usize::from(when_true.block())].parameters())
                    .map(|(argument, to)| {
                        (lower_value_to_location(to), lower_value(compiled, argument))
                    })
                    .collect::<Vec<_>>(),
            ),
            otherwise: (
                otherwise.block(),
                otherwise
                    .arguments()
                    .iter()
                    .zip(compiled.result().blocks()[usize::from(otherwise.block())].parameters())
                    .map(|(argument, to)| {
                        (lower_value_to_location(to), lower_value(compiled, argument))
                    })
                    .collect::<Vec<_>>(),
            ),
        },
    }
}

#[must_use]
fn lower_instruction(
    compiled: &Compiled<'_, Ssa>,
    instruction: &IrInstruction,
) -> Instruction<Value> {
    match instruction {
        IrInstruction::NoOp => Instruction::NoOp,
        IrInstruction::Unary {
            op,
            operand,
            temporary,
        } => Instruction::Unary {
            op: match op {
                AstUnaryOp::Not => UnaryOp::Not,
                AstUnaryOp::Negate => UnaryOp::Negate,
            },
            operand: lower_value(compiled, operand),
            to: lower_value_to_location(temporary),
        },
        IrInstruction::Binary {
            op,
            lhs,
            rhs,
            temporary,
        } => Instruction::Binary {
            op: match op {
                AstBinaryOp::PathAccess | AstBinaryOp::Access | AstBinaryOp::Assign => {
                    unreachable!("these binary ops are eliminated by previous stages")
                }
                AstBinaryOp::Multiply => BinaryOp::Multiply,
                AstBinaryOp::Divide => BinaryOp::Divide,
                AstBinaryOp::Remainder => BinaryOp::Remainder,
                AstBinaryOp::Add => BinaryOp::Add,
                AstBinaryOp::Subtract => BinaryOp::Subtract,
                AstBinaryOp::Less => BinaryOp::Less,
                AstBinaryOp::Greater => BinaryOp::Greater,
                AstBinaryOp::LessOrEqual => BinaryOp::LessOrEqual,
                AstBinaryOp::GreaterOrEqual => BinaryOp::GreaterOrEqual,
                AstBinaryOp::Equal => BinaryOp::Equal,
                AstBinaryOp::NotEqual => BinaryOp::NotEqual,
                AstBinaryOp::And => BinaryOp::And,
                AstBinaryOp::Or => BinaryOp::Or,
            },
            lhs: lower_value(compiled, lhs),
            rhs: lower_value(compiled, rhs),
            to: lower_value_to_location(temporary),
        },
        IrInstruction::Assign { value, to } => Instruction::Assign {
            value: lower_value(compiled, value),
            to: lower_value_to_location(to),
        },
        IrInstruction::Push(value) => Instruction::Push(lower_value(compiled, value)),
        IrInstruction::Call {
            callee,
            arity,
            temporary,
        } => Instruction::Call {
            callee: lower_value(compiled, callee),
            arity: *arity,
            to: lower_value_to_location(temporary),
        },
        IrInstruction::Access {
            index,
            of,
            temporary,
        } => Instruction::Access {
            index: *index,
            of: lower_value(compiled, of),
            to: lower_value_to_location(temporary),
        },
        IrInstruction::AccessAssign { index, of, value } => Instruction::AccessAssign {
            index: *index,
            of: lower_value(compiled, of),
            value: lower_value(compiled, value),
        },
    }
}

#[must_use]
fn lower_value(compiled: &Compiled<'_, Ssa>, value: &IrValue) -> ValueOrLocation<Value> {
    match value {
        IrValue::BlockArgument(_) | IrValue::CallArgument(_) | IrValue::Address(_) => {
            unreachable!("these are eliminated by register allocation")
        }
        IrValue::U8(value) => ValueOrLocation::Value(Value::U8(*value)),
        IrValue::I8(value) => ValueOrLocation::Value(Value::I8(*value)),
        IrValue::U16(value) => ValueOrLocation::Value(Value::U16(*value)),
        IrValue::I16(value) => ValueOrLocation::Value(Value::I16(*value)),
        IrValue::U32(value) => ValueOrLocation::Value(Value::U32(*value)),
        IrValue::I32(value) => ValueOrLocation::Value(Value::I32(*value)),
        IrValue::U64(value) => ValueOrLocation::Value(Value::U64(*value)),
        IrValue::I64(value) => ValueOrLocation::Value(Value::I64(*value)),
        IrValue::F64(value) => ValueOrLocation::Value(Value::F64(*value)),
        IrValue::Boolean(value) => ValueOrLocation::Value(Value::Boolean(*value)),
        IrValue::Unit | IrValue::Runtime => ValueOrLocation::Value(Value::Unit),
        IrValue::Fn(block_index) => ValueOrLocation::Value(Value::Fn(*block_index)),
        IrValue::NativeFn(span) => {
            let source_index = compiled
                .sources()
                .iter()
                .position(|(id, _)| *id == span.source_id())
                .expect("the source must exist");

            ValueOrLocation::Value(Value::NativeFn(
                match compiled
                    .sources()
                    .get(source_index)
                    .and_then(|(_, source)| span.lexeme(source))
                {
                    Some("print_u8") => NativeFn::PrintU8,
                    Some("print_i8") => NativeFn::PrintI8,
                    Some("print_u16") => NativeFn::PrintU16,
                    Some("print_i16") => NativeFn::PrintI16,
                    Some("print_u32") => NativeFn::PrintU32,
                    Some("print_i32") => NativeFn::PrintI32,
                    Some("print_u64") => NativeFn::PrintU64,
                    Some("print_i64") => NativeFn::PrintI64,
                    Some("print_f64") => NativeFn::PrintF64,
                    Some("print_bool") => NativeFn::PrintBool,
                    Some("print_unit") => NativeFn::PrintUnit,
                    _ => unreachable!("unknown native function"),
                },
            ))
        }
        IrValue::StackOffset(_) | IrValue::Register(_) => {
            ValueOrLocation::At(lower_value_to_location(value))
        }
        IrValue::Compound(fields) => ValueOrLocation::Value(Value::Compound(
            fields
                .iter()
                .map(|field| lower_value(compiled, field))
                .collect::<Vec<_>>(),
        )),
        IrValue::TaggedCompound { tag, fields } => ValueOrLocation::Value(Value::TaggedCompound {
            tag: *tag,
            fields: fields
                .iter()
                .map(|field| lower_value(compiled, field))
                .collect::<Vec<_>>(),
        }),
    }
}

#[must_use]
fn lower_value_to_location(value: &IrValue) -> Location {
    match value {
        IrValue::StackOffset(offset) => Location::StackOffset(StackOffset(*offset)),
        IrValue::Register(index) => Location::Register(Register(*index)),
        _ => unreachable!("only stack offsets and registers can be locations"),
    }
}
