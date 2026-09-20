use crate::{
    Compiled,
    basic_blocks::{Instruction as IrInstruction, Value as IrValue},
    parse::{BinaryOp, UnaryOp},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashSet;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CopyableValue {
    I64(i64),
    F64(f64),
    Boolean(bool),
    Unit,
    Fn(usize),
    NativeFn(u16),
    Register(usize),
    StackOffset(usize),
    ValueIndex(ValueIndex),
}

#[derive(Debug)]
pub enum Value {
    MakeCompound(Vec<CopyableValue>),
    MakeTaggedCompound {
        fields: Vec<CopyableValue>,
        tag: u16,
    },
    Compound(Vec<CopyableValue>),
    TaggedCompound {
        fields: Vec<CopyableValue>,
        tag: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueIndex(pub usize);

impl From<ValueIndex> for usize {
    fn from(value: ValueIndex) -> Self {
        value.0
    }
}

#[derive(Debug)]
pub enum Instruction {
    Unary {
        op_code: OpCode,
        operand: CopyableValue,
        to: CopyableValue,
    },
    Binary {
        op_code: OpCode,
        lhs: CopyableValue,
        rhs: CopyableValue,
        to: CopyableValue,
    },
    Assign {
        to: CopyableValue,
        value: CopyableValue,
    },
    Push(CopyableValue),
    Access {
        index: usize,
        of: CopyableValue,
        to: CopyableValue,
    },
    AccessAssign {
        index: usize,
        of: CopyableValue,
        value: CopyableValue,
    },
    Call {
        callee: CopyableValue,
        arity: usize,
        to: CopyableValue,
    },
    Jump(usize, Vec<(CopyableValue, CopyableValue)>),
    Branch {
        condition: CopyableValue,
        when_true: (usize, Vec<(CopyableValue, CopyableValue)>),
        otherwise: (usize, Vec<(CopyableValue, CopyableValue)>),
    },
    Return(CopyableValue),
}

pub trait Disassemble<T>
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self;
}

pub trait Instructive: Sized {
    fn ip_mut(&mut self) -> &mut usize;

    fn instructions_mut(&mut self) -> &mut Vec<Instruction>;

    fn values_mut(&mut self) -> &mut Vec<Value>;

    #[allow(clippy::too_many_lines)]
    fn read(&mut self, bytes: &[u8]) {
        *self.ip_mut() = 16;

        let block_count = bytes[8..16]
            .as_array::<8>()
            .map(|array| u64::from_le_bytes(*array))
            .and_then(|index| usize::try_from(index).ok())
            .expect("block count is not a valid usize");

        if block_count == 0 {
            return;
        }

        let entrypoint = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
            .as_array::<8>()
            .map(|array| u64::from_le_bytes(*array))
            .and_then(|index| usize::try_from(index).ok())
            .expect("block address is not a valid usize");

        let mut block_byte_offsets = HashSet::new();

        for _ in 0..block_count {
            let block_byte_offset = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                .as_array::<8>()
                .map(|array| u64::from_le_bytes(*array))
                .and_then(|index| usize::try_from(index).ok())
                .expect("block address is not a valid usize");

            *self.ip_mut() += 8;

            block_byte_offsets.insert(block_byte_offset);
        }

        let mut block_addresses = vec![];

        *self.ip_mut() = entrypoint;

        while *self.ip_mut() < bytes.len() {
            if block_byte_offsets.contains(&*self.ip_mut()) {
                block_addresses.push(self.instructions_mut().len());
            }

            let op_code = OpCode::from_bytes(bytes, self);

            match op_code {
                OpCode::Not | OpCode::Negate => {
                    let to = CopyableValue::from_bytes(bytes, self);

                    let operand = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut().push(Instruction::Unary {
                        op_code,
                        operand,
                        to,
                    });
                }
                OpCode::Multiply
                | OpCode::Divide
                | OpCode::Remainder
                | OpCode::Add
                | OpCode::Subtract
                | OpCode::Less
                | OpCode::Greater
                | OpCode::LessOrEqual
                | OpCode::GreaterOrEqual
                | OpCode::Equal
                | OpCode::NotEqual
                | OpCode::And
                | OpCode::Or => {
                    let to = CopyableValue::from_bytes(bytes, self);

                    let lhs = CopyableValue::from_bytes(bytes, self);

                    let rhs = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut().push(Instruction::Binary {
                        op_code,
                        lhs,
                        rhs,
                        to,
                    });
                }
                OpCode::Assign => {
                    let to = CopyableValue::from_bytes(bytes, self);

                    let value = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(Instruction::Assign { to, value });
                }
                OpCode::Push => {
                    let value = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut().push(Instruction::Push(value));
                }
                OpCode::Access => {
                    let to = CopyableValue::from_bytes(bytes, self);

                    let index = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("access index is not a valid usize");

                    *self.ip_mut() += 8;

                    let of = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(Instruction::Access { index, of, to });
                }
                OpCode::AccessAssign => {
                    let index = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("access index is not a valid usize");

                    *self.ip_mut() += 8;

                    let of = CopyableValue::from_bytes(bytes, self);

                    let value = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(Instruction::AccessAssign { index, of, value });
                }
                OpCode::Call => {
                    let callee = CopyableValue::from_bytes(bytes, self);

                    let arity = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|arity| usize::try_from(arity).ok())
                        .expect("call arity is not a valid usize");

                    *self.ip_mut() += 8;

                    let to = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(Instruction::Call { callee, arity, to });
                }
                OpCode::Jump => {
                    let to = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    let argument_count = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|argument_count| usize::try_from(argument_count).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    let mut arguments = vec![];

                    for _ in 0..argument_count {
                        let to = CopyableValue::from_bytes(bytes, self);

                        let value = CopyableValue::from_bytes(bytes, self);

                        arguments.push((to, value));
                    }

                    self.instructions_mut()
                        .push(Instruction::Jump(to, arguments));
                }
                OpCode::Branch => {
                    let condition = CopyableValue::from_bytes(bytes, self);

                    let when_true = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    let otherwise = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    let argument_count = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|argument_count| usize::try_from(argument_count).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    let mut arguments = vec![];

                    for _ in 0..argument_count {
                        let to = CopyableValue::from_bytes(bytes, self);

                        let value = CopyableValue::from_bytes(bytes, self);

                        arguments.push((to, value));
                    }

                    let when_true = (when_true, arguments);

                    let argument_count = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|argument_count| usize::try_from(argument_count).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    let mut arguments = vec![];

                    for _ in 0..argument_count {
                        let to = CopyableValue::from_bytes(bytes, self);

                        let value = CopyableValue::from_bytes(bytes, self);

                        arguments.push((to, value));
                    }

                    let otherwise = (otherwise, arguments);

                    self.instructions_mut().push(Instruction::Branch {
                        condition,
                        when_true,
                        otherwise,
                    });
                }
                OpCode::Return => {
                    let value = CopyableValue::from_bytes(bytes, self);

                    self.instructions_mut().push(Instruction::Return(value));
                }
            }
        }

        for instruction in self.instructions_mut() {
            match instruction {
                Instruction::Unary { operand: value, .. }
                | Instruction::Assign { value, .. }
                | Instruction::Push(value)
                | Instruction::Access { of: value, .. }
                | Instruction::Call { callee: value, .. }
                | Instruction::Return(value) => {
                    if let CopyableValue::Fn(address) = value {
                        *address = block_addresses[(*address - 16) / 8];
                    }
                }
                Instruction::Binary { lhs, rhs, .. }
                | Instruction::AccessAssign {
                    of: lhs,
                    value: rhs,
                    ..
                } => {
                    if let CopyableValue::Fn(address) = lhs {
                        *address = block_addresses[(*address - 16) / 8];
                    }

                    if let CopyableValue::Fn(address) = rhs {
                        *address = block_addresses[(*address - 16) / 8];
                    }
                }
                Instruction::Jump(address, arguments) => {
                    *address = block_addresses[(*address - 16) / 8];

                    for (_, argument) in arguments {
                        if let CopyableValue::Fn(address) = argument {
                            *address = block_addresses[(*address - 16) / 8];
                        }
                    }
                }
                Instruction::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => {
                    if let CopyableValue::Fn(address) = condition {
                        *address = block_addresses[(*address - 16) / 8];
                    }

                    when_true.0 = block_addresses[(when_true.0 - 16) / 8];

                    otherwise.0 = block_addresses[(otherwise.0 - 16) / 8];

                    for (_, argument) in &mut when_true.1 {
                        if let CopyableValue::Fn(address) = argument {
                            *address = block_addresses[(*address - 16) / 8];
                        }
                    }

                    for (_, argument) in &mut otherwise.1 {
                        if let CopyableValue::Fn(address) = argument {
                            *address = block_addresses[(*address - 16) / 8];
                        }
                    }
                }
            }
        }

        *self.ip_mut() = 0;
    }
}

impl<T> Disassemble<T> for OpCode
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match Self::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(op_code) => op_code,
            Err(error) => panic!("unknown opcode {error}"),
        }
    }
}

impl<T> Disassemble<T> for CopyableValue
where
    T: Instructive,
{
    #[allow(clippy::too_many_lines)]
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match TypeId::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(TypeId::I64) => Self::I64(i64::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::F64) => Self::F64(f64::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::Boolean) => Self::Boolean(
                bool::try_from(
                    bytes[{
                        let ip = *instructive.ip_mut();
                        *instructive.ip_mut() += 1;
                        ip
                    }],
                )
                .expect("a boolean could not be created from its byte"),
            ),
            Ok(TypeId::Unit) => Self::Unit,
            Ok(TypeId::Fn) => Self::Fn(
                bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a function was not a valid usize"),
            ),
            Ok(TypeId::NativeFn) => Self::NativeFn(
                bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 2;
                    *instructive.ip_mut()
                }]
                    .as_array::<2>()
                    .map(|array| u16::from_le_bytes(*array))
                    .expect("a native function was not valid"),
            ),
            Ok(TypeId::StackOffset) => Self::StackOffset(
                bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a stack offset was not a valid usize"),
            ),
            Ok(TypeId::Register) => Self::Register(
                bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a register was not a valid usize"),
            ),
            Ok(TypeId::Compound) => {
                let field_count = bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|field_count| usize::try_from(field_count).ok())
                    .expect("a compound's field count was not a valid usize");

                let mut fields = vec![];

                for _ in 0..field_count {
                    let field = Self::from_bytes(bytes, instructive);

                    fields.push(field);
                }

                instructive.values_mut().push(Value::MakeCompound(fields));

                Self::ValueIndex(ValueIndex(instructive.values_mut().len() - 1))
            }
            Ok(TypeId::TaggedCompound) => {
                let tag = bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 2;
                    *instructive.ip_mut()
                }]
                    .as_array::<2>()
                    .map(|array| u16::from_le_bytes(*array))
                    .expect("a tagged compound's tag was not a valid u16");

                let field_count = bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|field_count| usize::try_from(field_count).ok())
                    .expect("a tagged compound's field count was not a valid usize");

                let mut fields = vec![];

                for _ in 0..field_count {
                    let field = Self::from_bytes(bytes, instructive);

                    fields.push(field);
                }

                instructive
                    .values_mut()
                    .push(Value::MakeTaggedCompound { tag, fields });

                Self::ValueIndex(ValueIndex(instructive.values_mut().len() - 1))
            }
            Err(error) => panic!("unknown type id {error}"),
        }
    }
}
