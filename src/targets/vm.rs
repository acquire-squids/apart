use crate::{
    Compiled,
    basic_blocks::BlockIndex,
    low_ir::{
        BinaryOp, Instruction as IrInstruction, Ir, Location, NativeFn, Register, UnaryOp,
        Value as IrValue, ValueOrLocation,
    },
};

use std::collections::HashSet;

/// # Panics
/// Panics if `usize` doesn't fit in a u64
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn compile(compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
    let ssa = compiled.result();

    let mut bytecode = vec![];

    bytecode.extend_from_slice(
        u64::try_from(compiled.result().main_max_register_count())
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
    }

    bytecode
}

crate::int_enum! {
    pub OpCode as u8 ;
    Unary => 0x20,
    Binary => 0x40,
    Assign => 0x60,
    Push => 0x80,
    PopN => 0x90,
    Call => 0xA0,
    Access => 0xB0,
    AccessAssign => 0xB1,
    GetTag => 0xB2,
    Jump => 0xC0,
    Branch => 0xC1,
    Return => 0xC2,
}

crate::int_enum! {
    pub ValueAt as u8 ;
    Value => 0x00,
    Register => 0x01,
}

crate::int_enum! {
    pub TypeId as u8 ;
    I64 => 0x00,
    F64 => 0x01,
    Boolean => 0x02,
    Unit => 0x03,
    Fn => 0x04,
    NativeFn => 0x05,
    Compound => 0x08,
    TaggedCompound => 0x09,
    U8 => 0x0A,
    I8 => 0x0B,
    U16 => 0x0C,
    I16 => 0x0D,
    U32 => 0x0E,
    I32 => 0x0F,
    U64 => 0x10,
    String => 0x11,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CopyableValue {
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
    Fn {
        address: usize,
        max_register_count: u32,
    },
    NativeFn(NativeFn),
    ValueIndex(ValueIndex),
}

#[derive(Debug)]
pub enum Value {
    MakeCompound(Vec<ValueOrLocation<CopyableValue>>),
    MakeTaggedCompound {
        fields: Vec<ValueOrLocation<CopyableValue>>,
        tag: u16,
    },
    Compound(Vec<CopyableValue>),
    TaggedCompound {
        fields: Vec<CopyableValue>,
        tag: u16,
    },
    MakeString(String),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueIndex(pub usize);

impl From<ValueIndex> for usize {
    fn from(value: ValueIndex) -> Self {
        value.0
    }
}

trait Assemble {
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8>;
}

impl Assemble for IrInstruction<IrValue> {
    #[allow(clippy::too_many_lines)]
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
        match self {
            Self::NoOp => vec![],
            Self::Unary { op, operand, to } => {
                let mut bytes = vec![u8::from(OpCode::Unary)];

                bytes.push(u8::from(*op));

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut operand.to_bytes(compiled));

                bytes
            }
            Self::Binary { op, lhs, rhs, to } => {
                let mut bytes = vec![u8::from(OpCode::Binary)];

                bytes.push(u8::from(*op));

                bytes.append(&mut to.to_bytes(compiled));

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
            Self::Call { callee, arity, to } => {
                let mut bytes = vec![u8::from(OpCode::Call)];

                bytes.append(&mut callee.to_bytes(compiled));

                bytes.extend_from_slice(
                    u64::try_from(*arity)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.append(&mut to.to_bytes(compiled));

                bytes
            }
            Self::Access { index, of, to } => {
                let mut bytes = vec![u8::from(OpCode::Access)];

                bytes.append(&mut to.to_bytes(compiled));

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
            Self::GetTag { of, to } => {
                let mut bytes = vec![u8::from(OpCode::GetTag)];

                bytes.append(&mut to.to_bytes(compiled));

                bytes.append(&mut of.to_bytes(compiled));

                bytes
            }
            Self::PopN(count) => {
                let mut bytes = vec![u8::from(OpCode::PopN)];

                bytes.extend_from_slice(
                    u64::try_from(*count)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::Jump(destination) => {
                let mut bytes = vec![u8::from(OpCode::Jump)];

                bytes.extend_from_slice(
                    u64::try_from(16 + usize::from(*destination) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                let mut bytes = vec![u8::from(OpCode::Branch)];

                bytes.append(&mut condition.to_bytes(compiled));

                bytes.extend_from_slice(
                    u64::try_from(16 + usize::from(*when_true) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.extend_from_slice(
                    u64::try_from(16 + usize::from(*otherwise) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::Return(value) => {
                let mut bytes = vec![u8::from(OpCode::Return)];

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
        }
    }
}

impl Assemble for ValueOrLocation<IrValue> {
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
        match self {
            Self::Value(value) => {
                let mut bytes = vec![u8::from(ValueAt::Value)];

                bytes.append(&mut value.to_bytes(compiled));

                bytes
            }
            Self::At(location) => location.to_bytes(compiled),
        }
    }
}

impl Assemble for Location {
    fn to_bytes(&self, _: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
        match self {
            Self::Register(index) => {
                let mut bytes = vec![u8::from(ValueAt::Register)];

                bytes.extend_from_slice(
                    u64::try_from(usize::from(*index))
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
        }
    }
}

impl Assemble for IrValue {
    #[allow(clippy::too_many_lines)]
    fn to_bytes(&self, compiled: &Compiled<'_, Ir<Self>>) -> Vec<u8> {
        match self {
            Self::U8(value) => {
                let mut bytes = vec![u8::from(TypeId::U8)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I8(value) => {
                let mut bytes = vec![u8::from(TypeId::I8)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U16(value) => {
                let mut bytes = vec![u8::from(TypeId::U16)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I16(value) => {
                let mut bytes = vec![u8::from(TypeId::I16)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U32(value) => {
                let mut bytes = vec![u8::from(TypeId::U32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I32(value) => {
                let mut bytes = vec![u8::from(TypeId::I32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U64(value) => {
                let mut bytes = vec![u8::from(TypeId::U64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I64(value) => {
                let mut bytes = vec![u8::from(TypeId::I64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::F64(value) => {
                let mut bytes = vec![u8::from(TypeId::F64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::Boolean(value) => {
                vec![u8::from(TypeId::Boolean), u8::from(*value)]
            }
            Self::Unit => {
                vec![u8::from(TypeId::Unit)]
            }
            Self::String(text) => {
                let mut bytes = vec![u8::from(TypeId::String)];

                bytes.extend_from_slice(
                    u64::try_from(text.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.extend_from_slice(text.as_bytes());

                bytes
            }
            Self::Fn {
                block_index,
                max_register_count,
            } => {
                let mut bytes = vec![u8::from(TypeId::Fn)];

                bytes.extend_from_slice(
                    u64::try_from(16 + usize::from(*block_index) * 8)
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes.extend_from_slice(
                    u32::try_from(*max_register_count)
                        .expect("a function's max register count was not a valid u32")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::NativeFn(native_fn) => {
                let mut bytes = vec![u8::from(TypeId::NativeFn)];

                bytes.extend_from_slice(u16::from(*native_fn).to_le_bytes().as_slice());

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

pub trait Disassemble<T>
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self;
}

pub trait Instructive: Sized {
    fn ip_mut(&mut self) -> &mut usize;

    fn instructions_mut(&mut self) -> &mut Vec<IrInstruction<CopyableValue>>;

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
                OpCode::Unary => {
                    let op = UnaryOp::from_bytes(bytes, self);

                    let to = Location::from_bytes(bytes, self);

                    let operand = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Unary { op, operand, to });
                }
                OpCode::Binary => {
                    let op = BinaryOp::from_bytes(bytes, self);

                    let to = Location::from_bytes(bytes, self);

                    let lhs = ValueOrLocation::from_bytes(bytes, self);

                    let rhs = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Binary { op, lhs, rhs, to });
                }
                OpCode::Assign => {
                    let to = Location::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Assign { to, value });
                }
                OpCode::Push => {
                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut().push(IrInstruction::Push(value));
                }
                OpCode::Access => {
                    let to = Location::from_bytes(bytes, self);

                    let index = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("access index is not a valid usize");

                    *self.ip_mut() += 8;

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Access { index, of, to });
                }
                OpCode::AccessAssign => {
                    let index = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|index| usize::try_from(index).ok())
                        .expect("access index is not a valid usize");

                    *self.ip_mut() += 8;

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::AccessAssign { index, of, value });
                }
                OpCode::GetTag => {
                    let to = Location::from_bytes(bytes, self);

                    let of = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::GetTag { of, to });
                }
                OpCode::PopN => {
                    let count = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|arity| usize::try_from(arity).ok())
                        .expect("call arity is not a valid usize");

                    *self.ip_mut() += 8;

                    self.instructions_mut().push(IrInstruction::PopN(count));
                }
                OpCode::Call => {
                    let callee = ValueOrLocation::from_bytes(bytes, self);

                    let arity = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|arity| usize::try_from(arity).ok())
                        .expect("call arity is not a valid usize");

                    *self.ip_mut() += 8;

                    let to = Location::from_bytes(bytes, self);

                    self.instructions_mut()
                        .push(IrInstruction::Call { callee, arity, to });
                }
                OpCode::Jump => {
                    let to = bytes[(*self.ip_mut())..(*self.ip_mut() + 8)]
                        .as_array::<8>()
                        .map(|array| u64::from_le_bytes(*array))
                        .and_then(|to| usize::try_from(to).ok())
                        .expect("jump address is not a valid usize");

                    *self.ip_mut() += 8;

                    self.instructions_mut()
                        .push(IrInstruction::Jump(BlockIndex(to)));
                }
                OpCode::Branch => {
                    let condition = ValueOrLocation::from_bytes(bytes, self);

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

                    let when_true = BlockIndex(when_true);

                    let otherwise = BlockIndex(otherwise);

                    self.instructions_mut().push(IrInstruction::Branch {
                        condition,
                        when_true,
                        otherwise,
                    });
                }
                OpCode::Return => {
                    let value = ValueOrLocation::from_bytes(bytes, self);

                    self.instructions_mut().push(IrInstruction::Return(value));
                }
            }
        }

        for instruction in self.instructions_mut() {
            match instruction {
                IrInstruction::NoOp | IrInstruction::PopN(_) => {}
                IrInstruction::Unary { operand: value, .. }
                | IrInstruction::Assign { value, .. }
                | IrInstruction::Push(value)
                | IrInstruction::Access { of: value, .. }
                | IrInstruction::GetTag { of: value, .. }
                | IrInstruction::Call { callee: value, .. }
                | IrInstruction::Return(value) => {
                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = value {
                        *address = block_addresses[(*address - 16) / 8];
                    }
                }
                IrInstruction::Binary { lhs, rhs, .. }
                | IrInstruction::AccessAssign {
                    of: lhs,
                    value: rhs,
                    ..
                } => {
                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = lhs {
                        *address = block_addresses[(*address - 16) / 8];
                    }

                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = rhs {
                        *address = block_addresses[(*address - 16) / 8];
                    }
                }
                IrInstruction::Jump(address) => {
                    *address = BlockIndex(block_addresses[(usize::from(*address) - 16) / 8]);
                }
                IrInstruction::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => {
                    if let ValueOrLocation::Value(CopyableValue::Fn { address, .. }) = condition {
                        *address = block_addresses[(*address - 16) / 8];
                    }

                    *when_true = BlockIndex(block_addresses[(usize::from(*when_true) - 16) / 8]);

                    *otherwise = BlockIndex(block_addresses[(usize::from(*otherwise) - 16) / 8]);
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

impl<T> Disassemble<T> for UnaryOp
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
            Err(error) => panic!("unknown unary op {error}"),
        }
    }
}

impl<T> Disassemble<T> for BinaryOp
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
            Err(error) => panic!("unknown binary op {error}"),
        }
    }
}

impl<T> Disassemble<T> for ValueOrLocation<CopyableValue>
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match ValueAt::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(ValueAt::Value) => Self::Value(CopyableValue::from_bytes(bytes, instructive)),
            Ok(ValueAt::Register) => {
                Self::At(Location::Register(Register::from_bytes(bytes, instructive)))
            }
            Err(error) => panic!("invalid value op {error}"),
        }
    }
}

impl<T> Disassemble<T> for Location
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        match ValueAt::try_from(
            bytes[{
                let ip = *instructive.ip_mut();
                *instructive.ip_mut() += 1;
                ip
            }],
        ) {
            Ok(ValueAt::Register) => Self::Register(Register::from_bytes(bytes, instructive)),
            Ok(ValueAt::Value) => panic!("got a value where a location was expected"),
            Err(error) => panic!("invalid value op {error}"),
        }
    }
}

impl<T> Disassemble<T> for Register
where
    T: Instructive,
{
    fn from_bytes(bytes: &[u8], instructive: &mut T) -> Self {
        Self(
            bytes[(*instructive.ip_mut())..{
                *instructive.ip_mut() += 8;
                *instructive.ip_mut()
            }]
                .as_array::<8>()
                .map(|array| u64::from_le_bytes(*array))
                .and_then(|index| usize::try_from(index).ok())
                .expect("128-bit usize not allowed!  sorry!"),
        )
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
            Ok(TypeId::U8) => Self::U8(u8::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 1;
                    *instructive.ip_mut()
                }]
                    .as_array::<1>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::I8) => Self::I8(i8::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 1;
                    *instructive.ip_mut()
                }]
                    .as_array::<1>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::U16) => Self::U16(u16::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 2;
                    *instructive.ip_mut()
                }]
                    .as_array::<2>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::I16) => Self::I16(i16::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 2;
                    *instructive.ip_mut()
                }]
                    .as_array::<2>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::U32) => Self::U32(u32::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 4;
                    *instructive.ip_mut()
                }]
                    .as_array::<4>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::I32) => Self::I32(i32::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 4;
                    *instructive.ip_mut()
                }]
                    .as_array::<4>()
                    .expect("the range is the same length as the expected array"),
            )),
            Ok(TypeId::U64) => Self::U64(u64::from_le_bytes(
                *bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .expect("the range is the same length as the expected array"),
            )),
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
            Ok(TypeId::String) => {
                let text = bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|length| usize::try_from(length).ok())
                    .and_then(|length| {
                        String::from_utf8(
                            bytes[(*instructive.ip_mut())..{
                                *instructive.ip_mut() += length;
                                *instructive.ip_mut()
                            }]
                                .to_vec(),
                        )
                        .ok()
                    })
                    .expect("a string wasn't valid utf-8");

                instructive.values_mut().push(Value::MakeString(text));

                Self::ValueIndex(ValueIndex(instructive.values_mut().len() - 1))
            }
            Ok(TypeId::Fn) => {
                let block_index = bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 8;
                    *instructive.ip_mut()
                }]
                    .as_array::<8>()
                    .map(|array| u64::from_le_bytes(*array))
                    .and_then(|address| usize::try_from(address).ok())
                    .expect("a function was not a valid usize");

                let max_register_count = bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 4;
                    *instructive.ip_mut()
                }]
                    .as_array::<4>()
                    .map(|array| u32::from_le_bytes(*array))
                    .expect("a function's max register count was not a valid u32");

                Self::Fn {
                    address: block_index,
                    max_register_count,
                }
            }
            Ok(TypeId::NativeFn) => Self::NativeFn(
                bytes[(*instructive.ip_mut())..{
                    *instructive.ip_mut() += 2;
                    *instructive.ip_mut()
                }]
                    .as_array::<2>()
                    .map(|array| u16::from_le_bytes(*array))
                    .and_then(|native_fn| NativeFn::try_from(native_fn).ok())
                    .expect("a native function was not valid"),
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
                    let field = ValueOrLocation::from_bytes(bytes, instructive);

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
                    let field = ValueOrLocation::from_bytes(bytes, instructive);

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
