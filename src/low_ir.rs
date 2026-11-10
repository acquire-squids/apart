use crate::{
    ApartOptions, Compiled,
    basic_blocks::{
        BlockIndex, Instruction as IrInstruction, TypedValue as IrTypedValue, Value as IrValue,
    },
    parse::{BinaryOp as AstBinaryOp, UnaryOp as AstUnaryOp},
    ssa::{Block as IrBlock, BlockTerminator, Ssa},
    type_check::TypeIndex,
};

use std::fmt;

#[must_use]
pub fn lower(_: &ApartOptions, compiled: &Compiled<'_, Ssa>) -> Ir<TypedValue> {
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

            Block {
                instructions,
                call_argument_count: block.call_argument_count(),
                max_register_count: block.max_register_count(),
            }
        })
        .collect::<Vec<_>>();

    Ir { blocks }
}

pub struct Ir<T> {
    blocks: Vec<Block<T>>,
}

impl<T> Ir<T> {
    #[must_use]
    pub const fn new(blocks: Vec<Block<T>>) -> Self {
        Self { blocks }
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
}

pub struct Block<T> {
    instructions: Vec<Instruction<T>>,
    call_argument_count: u8,
    max_register_count: u16,
}

impl<T> Block<T> {
    #[must_use]
    pub const fn new(
        instructions: Vec<Instruction<T>>,
        call_argument_count: u8,
        max_register_count: u16,
    ) -> Self {
        Self {
            instructions,
            call_argument_count,
            max_register_count,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn instructions(&self) -> &[Instruction<T>] {
        self.instructions.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn max_register_count(&self) -> u16 {
        self.max_register_count
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn call_argument_count(&self) -> u8 {
        self.call_argument_count
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
    pub NativeFn as u32 ;
    PrintF64 => 0x00_00_00_01,
    PrintString => 0x00_00_00_0B,
    PrintF32 => 0x00_00_00_0C,
    ExtendString => 0x00_00_10_00,
    SliceString => 0x00_00_10_01,
    LengthString => 0x00_00_10_02,
    TruncateString => 0x00_00_10_03,
    FloorCharBoundaryString => 0x00_00_10_04,
    CeilCharBoundaryString => 0x00_00_10_05,
    Clock => 0x00_00_20_00,
    Sleep => 0x00_00_20_01,
    NewVec => 0x00_00_30_00,
    GetVec => 0x00_00_30_01,
    ExtendVec => 0x00_00_30_02,
    SliceVec => 0x00_00_30_03,
    LengthVec => 0x00_00_30_04,
    TruncateVec => 0x00_00_30_05,
    PushVec => 0x00_00_30_06,
    PopVec => 0x00_00_30_07,
    LogF64 => 0x00_00_40_01,
    FloorF64 => 0x00_00_40_02,
    // TODO: remove this with a trait implementation
    ToU64F64 => 0x00_00_40_03,
    FromU64F64 => 0x00_00_40_04,
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
    IntegerDivide => 0x0D,
}

#[derive(Debug)]
pub struct TypedValue {
    value: Value,
    type_index: TypeIndex,
}

impl TypedValue {
    #[allow(dead_code)]
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn type_index(&self) -> TypeIndex {
        self.type_index
    }
}

#[derive(Debug)]
pub enum Value {
    Never,
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    Boolean(bool),
    Unit,
    String(String),
    Fn {
        block_index: BlockIndex,
        max_register_count: u16,
    },
    NativeFn(NativeFn),
    Compound(Vec<ValueOrLocation<TypedValue>>),
    TaggedCompound {
        fields: Vec<ValueOrLocation<TypedValue>>,
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
    Register(Register),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Register(pub u16);

impl From<Register> for u16 {
    fn from(value: Register) -> Self {
        value.0
    }
}

#[derive(Debug)]
pub struct FnLabel {
    file_name: String,
    function_label: String,
}

impl FnLabel {
    pub const fn new(file_name: String, function_label: String) -> Self {
        Self {
            file_name,
            function_label,
        }
    }

    pub const fn file_name(&self) -> &str {
        self.file_name.as_str()
    }

    pub const fn function_label(&self) -> &str {
        self.function_label.as_str()
    }
}

#[derive(Debug)]
pub enum Instruction<T> {
    FnLabel(Box<FnLabel>),
    NoOp,
    Panic,
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
        index: u8,
        of: ValueOrLocation<T>,
        to: Location,
    },
    AccessAssign {
        index: u8,
        of: ValueOrLocation<T>,
        value: ValueOrLocation<T>,
    },
    Call {
        callee: ValueOrLocation<T>,
        arity: u8,
        to: Location,
    },
    GetTag {
        of: ValueOrLocation<T>,
        to: Location,
    },
    Jump(BlockIndex),
    Branch {
        condition: ValueOrLocation<T>,
        when_true: BlockIndex,
        otherwise: BlockIndex,
    },
    Return(ValueOrLocation<T>),
}

#[must_use]
fn lower_block_terminator(
    compiled: &Compiled<'_, Ssa>,
    block_terminator: &BlockTerminator,
) -> Instruction<TypedValue> {
    match block_terminator {
        BlockTerminator::Return(value) => Instruction::Return(lower_value(compiled, value)),
        BlockTerminator::Jump(jump_to) => Instruction::Jump(*jump_to),
        BlockTerminator::Branch {
            condition,
            when_true,
            otherwise,
        } => Instruction::Branch {
            condition: lower_value(compiled, condition),
            when_true: *when_true,
            otherwise: *otherwise,
        },
    }
}

#[must_use]
fn lower_instruction(
    compiled: &Compiled<'_, Ssa>,
    instruction: &IrInstruction,
) -> Instruction<TypedValue> {
    match instruction {
        IrInstruction::NoOp => Instruction::NoOp,
        IrInstruction::Panic => Instruction::Panic,
        IrInstruction::FnLabel {
            file_name,
            function_label,
        } => Instruction::FnLabel(Box::new(FnLabel {
            file_name: file_name.clone(),
            function_label: function_label.clone(),
        })),
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
                AstBinaryOp::IntegerDivide => BinaryOp::IntegerDivide,
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
        IrInstruction::GetTag { of, temporary } => Instruction::GetTag {
            of: lower_value(compiled, of),
            to: lower_value_to_location(temporary),
        },
    }
}

#[must_use]
fn lower_value(compiled: &Compiled<'_, Ssa>, value: &IrTypedValue) -> ValueOrLocation<TypedValue> {
    let type_index = value.type_index();

    ValueOrLocation::Value(TypedValue {
        value: match value.value() {
            IrValue::CallArgument(_) | IrValue::Address(_) => {
                unreachable!("these are eliminated by register allocation {value:?}")
            }
            IrValue::Never => Value::Never,
            IrValue::U8(value) => Value::U8(*value),
            IrValue::I8(value) => Value::I8(*value),
            IrValue::U16(value) => Value::U16(*value),
            IrValue::I16(value) => Value::I16(*value),
            IrValue::U32(value) => Value::U32(*value),
            IrValue::I32(value) => Value::I32(*value),
            IrValue::U64(value) => Value::U64(*value),
            IrValue::I64(value) => Value::I64(*value),
            IrValue::F32(value) => Value::F32(*value),
            IrValue::F64(value) => Value::F64(*value),
            IrValue::Boolean(value) => Value::Boolean(*value),
            IrValue::Unit => Value::Unit,
            IrValue::String(text) => Value::String(text.clone()),
            IrValue::Fn(block_index) => Value::Fn {
                block_index: *block_index,
                max_register_count: compiled
                    .result()
                    .get_block(*block_index)
                    .map_or(0, IrBlock::max_register_count),
            },
            IrValue::NativeFn { label, .. } => Value::NativeFn(match label.as_str() {
                "string~String~extend" => NativeFn::ExtendString,
                "string~String~slice" => NativeFn::SliceString,
                "string~String~length" => NativeFn::LengthString,
                "string~String~truncate" => NativeFn::TruncateString,
                "string~String~floor_char_boundary" => NativeFn::FloorCharBoundaryString,
                "string~String~ceil_char_boundary" => NativeFn::CeilCharBoundaryString,
                "vec~Vec[T]~new" => NativeFn::NewVec,
                "vec~Vec[T]~get" => NativeFn::GetVec,
                "vec~Vec[T]~extend" => NativeFn::ExtendVec,
                "vec~Vec[T]~slice" => NativeFn::SliceVec,
                "vec~Vec[T]~length" => NativeFn::LengthVec,
                "vec~Vec[T]~truncate" => NativeFn::TruncateVec,
                "vec~Vec[T]~push" => NativeFn::PushVec,
                "vec~Vec[T]~pop" => NativeFn::PopVec,
                "math~f64~log" => NativeFn::LogF64,
                "math~f64~floor" => NativeFn::FloorF64,
                "math~f64~to_u64" => NativeFn::ToU64F64,
                "math~f64~from_u64" => NativeFn::FromU64F64,
                "print_f32" => NativeFn::PrintF32,
                "print_f64" => NativeFn::PrintF64,
                "print_string" => NativeFn::PrintString,
                "clock" => NativeFn::Clock,
                "sleep" => NativeFn::Sleep,
                _ => unreachable!("unknown native function: {label}"),
            }),
            IrValue::Register(_) => {
                return ValueOrLocation::At(lower_value_to_location(value.value()));
            }
            IrValue::Compound(fields) => Value::Compound(
                fields
                    .iter()
                    .map(|field| lower_value(compiled, field))
                    .collect::<Vec<_>>(),
            ),
            IrValue::TaggedCompound { tag, fields } => Value::TaggedCompound {
                tag: *tag,
                fields: fields
                    .iter()
                    .map(|field| lower_value(compiled, field))
                    .collect::<Vec<_>>(),
            },
        },
        type_index,
    })
}

#[must_use]
fn lower_value_to_location(value: &IrValue) -> Location {
    match value {
        IrValue::Register(offset) => Location::Register(Register(
            u16::try_from(*offset)
                .expect("register allocation guarantees the registers don't exceed the maximum"),
        )),
        _ => unreachable!("only registers can be locations"),
    }
}
