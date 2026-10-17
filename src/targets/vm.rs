use crate::{
    ApartOptions, Compiled,
    low_ir::{
        BinaryOp, FnLabel, Instruction as IrInstruction, Ir, Location, NativeFn, Register, UnaryOp,
        Value as IrValue, ValueOrLocation,
    },
};

use std::collections::HashMap;

struct Compiler {
    ip: usize,
    constants: Vec<ValueIndex>,
    values: Vec<Value>,
}

impl Instructive for Compiler {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn constants_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.constants
    }

    fn values_mut(&mut self) -> &mut Vec<Value> {
        &mut self.values
    }
}

/// # Panics
/// Panics if `usize` doesn't fit in a u64
#[must_use]
pub fn compile(options: &ApartOptions, compiled: &Compiled<'_, Ir<IrValue>>) -> Vec<u8> {
    let ssa = compiled.result();

    let mut assembler = Compiler {
        ip: 0,
        constants: vec![],
        values: vec![],
    };

    let mut bytecode = vec![];

    bytecode.extend_from_slice(
        compiled
            .result()
            .main_max_register_count()
            .to_le_bytes()
            .as_slice(),
    );

    let mut block_addresses = vec![0u32; ssa.blocks().len()];
    let mut address_to_label = HashMap::new();

    for (b, block) in ssa.blocks().iter().enumerate() {
        block_addresses[b] =
            u32::try_from(bytecode.len()).expect("block index was not a valid u32");

        address_to_label.insert(
            u32::try_from(b).expect("block index was not a valid u32"),
            u32::try_from(b).expect("block index was not a valid u32"),
        );

        bytecode.extend(Instruction::Label.assemble(&mut assembler));

        for instruction in block.instructions() {
            if !options.strip_debug_info || !matches!(instruction, IrInstruction::FnLabel(_)) {
                bytecode.append(&mut instruction.assemble(&mut assembler));
            }
        }
    }

    loop {
        if !backpatch(&mut bytecode, block_addresses.as_slice(), &address_to_label) {
            break;
        }

        find_labels(
            &mut bytecode,
            &mut block_addresses,
            &mut address_to_label,
            false,
        );
    }

    find_labels(
        &mut bytecode,
        &mut block_addresses,
        &mut address_to_label,
        true,
    );

    bytecode
}

/// # Panics
/// - panics if `bytecode.len()` > `u32::MAX`
/// - panics if the number of instructions in the output exceeds `u32::MAX`
#[must_use]
pub fn decompile<T>(bytecode: &[u8], decompiler: &mut T) -> Vec<Instruction>
where
    T: Instructive,
{
    *decompiler.ip_mut() = 2;

    let mut instructions = vec![];

    let mut address_to_label = HashMap::new();

    let mut block_addresses = vec![];

    while *decompiler.ip_mut() < bytecode.len() {
        address_to_label.insert(
            u32::try_from(*decompiler.ip_mut()).expect("instruction pointer was not a valid u32"),
            u32::try_from(instructions.len()).expect("instruction count was not a valid u32"),
        );

        block_addresses.push(
            u32::try_from(instructions.len()).expect("instruction count was not a valid u32"),
        );

        instructions.push(Instruction::disassemble(bytecode, decompiler));
    }

    for instruction in &mut instructions {
        backpatch_instruction(
            block_addresses.as_slice(),
            &address_to_label,
            decompiler,
            instruction,
        );
    }

    *decompiler.ip_mut() = 0;

    instructions
}

fn backpatch(
    bytecode: &mut Vec<u8>,
    block_addresses: &[u32],
    address_to_label: &HashMap<u32, u32>,
) -> bool {
    let mut changed = false;

    let mut disassembler = Compiler {
        ip: 2,
        constants: vec![],
        values: vec![],
    };

    while disassembler.ip < bytecode.len() {
        let at = disassembler.ip;

        let mut instruction = Instruction::disassemble(bytecode, &mut disassembler);

        if backpatch_instruction(
            block_addresses,
            address_to_label,
            &mut disassembler,
            &mut instruction,
        ) {
            replace_instruction(bytecode, &mut disassembler, &instruction, at);

            changed |= true;
        }
    }

    changed
}

fn replace_instruction<T>(
    bytecode: &mut Vec<u8>,
    disassembler: &mut T,
    instruction: &Instruction,
    at: usize,
) where
    T: Instructive,
{
    let ip = *disassembler.ip_mut();

    let assembled = instruction.assemble(disassembler);

    let changed_instruction_byte_count = assembled.len();

    bytecode.splice(at..ip, assembled);

    *disassembler.ip_mut() = at + changed_instruction_byte_count;
}

#[allow(clippy::too_many_lines)]
fn backpatch_instruction<T>(
    block_addresses: &[u32],
    address_to_label: &HashMap<u32, u32>,
    disassembler: &mut T,
    instruction: &mut Instruction,
) -> bool
where
    T: Instructive,
{
    let mut instruction_changed = false;

    match instruction {
        Instruction::NoOp
        | Instruction::Panic
        | Instruction::Label
        | Instruction::FnLabel { .. }
        | Instruction::NotRegister { .. }
        | Instruction::NegateRegister { .. }
        | Instruction::MultiplyRegisterRegister { .. }
        | Instruction::DivideRegisterRegister { .. }
        | Instruction::RemainderRegisterRegister { .. }
        | Instruction::AddRegisterRegister { .. }
        | Instruction::SubtractRegisterRegister { .. }
        | Instruction::LessRegisterRegister { .. }
        | Instruction::GreaterRegisterRegister { .. }
        | Instruction::LessOrEqualRegisterRegister { .. }
        | Instruction::GreaterOrEqualRegisterRegister { .. }
        | Instruction::EqualRegisterRegister { .. }
        | Instruction::NotEqualRegisterRegister { .. }
        | Instruction::AndRegisterRegister { .. }
        | Instruction::OrRegisterRegister { .. }
        | Instruction::AssignRegister { .. }
        | Instruction::PushRegister { .. }
        | Instruction::AccessRegister { .. }
        | Instruction::AccessAssignRegisterRegister { .. }
        | Instruction::CallRegister { .. }
        | Instruction::GetTagRegister { .. }
        | Instruction::PopN { .. }
        | Instruction::ReturnRegister { .. }
        | Instruction::I64
        | Instruction::F64
        | Instruction::Boolean
        | Instruction::Unit
        | Instruction::Fn
        | Instruction::NativeFn
        | Instruction::Compound
        | Instruction::TaggedCompound
        | Instruction::U8
        | Instruction::I8
        | Instruction::U16
        | Instruction::I16
        | Instruction::U32
        | Instruction::I32
        | Instruction::U64
        | Instruction::String
        | Instruction::F32
        | Instruction::Never
        | Instruction::Register => {}
        Instruction::NotValue { operand: value, .. }
        | Instruction::NegateValue { operand: value, .. }
        | Instruction::MultiplyRegisterValue { rhs: value, .. }
        | Instruction::MultiplyValueRegister { lhs: value, .. }
        | Instruction::DivideRegisterValue { rhs: value, .. }
        | Instruction::DivideValueRegister { lhs: value, .. }
        | Instruction::RemainderRegisterValue { rhs: value, .. }
        | Instruction::RemainderValueRegister { lhs: value, .. }
        | Instruction::AddRegisterValue { rhs: value, .. }
        | Instruction::AddValueRegister { lhs: value, .. }
        | Instruction::SubtractRegisterValue { rhs: value, .. }
        | Instruction::SubtractValueRegister { lhs: value, .. }
        | Instruction::LessRegisterValue { rhs: value, .. }
        | Instruction::LessValueRegister { lhs: value, .. }
        | Instruction::GreaterRegisterValue { rhs: value, .. }
        | Instruction::GreaterValueRegister { lhs: value, .. }
        | Instruction::LessOrEqualRegisterValue { rhs: value, .. }
        | Instruction::LessOrEqualValueRegister { lhs: value, .. }
        | Instruction::GreaterOrEqualRegisterValue { rhs: value, .. }
        | Instruction::GreaterOrEqualValueRegister { lhs: value, .. }
        | Instruction::EqualRegisterValue { rhs: value, .. }
        | Instruction::EqualValueRegister { lhs: value, .. }
        | Instruction::NotEqualRegisterValue { rhs: value, .. }
        | Instruction::NotEqualValueRegister { lhs: value, .. }
        | Instruction::AndRegisterValue { rhs: value, .. }
        | Instruction::AndValueRegister { lhs: value, .. }
        | Instruction::OrRegisterValue { rhs: value, .. }
        | Instruction::OrValueRegister { lhs: value, .. }
        | Instruction::AssignValue { operand: value, .. }
        | Instruction::PushValue { operand: value }
        | Instruction::AccessValue { of: value, .. }
        | Instruction::AccessAssignRegisterValue { value, .. }
        | Instruction::AccessAssignValueRegister { to: value, .. }
        | Instruction::CallValue { callee: value, .. }
        | Instruction::GetTagValue { of: value, .. }
        | Instruction::ReturnValue { value } => {
            instruction_changed |=
                backpatch_value(value, block_addresses, address_to_label, disassembler);
        }
        Instruction::MultiplyValueValue { lhs, rhs, .. }
        | Instruction::DivideValueValue { lhs, rhs, .. }
        | Instruction::RemainderValueValue { lhs, rhs, .. }
        | Instruction::AddValueValue { lhs, rhs, .. }
        | Instruction::SubtractValueValue { lhs, rhs, .. }
        | Instruction::LessValueValue { lhs, rhs, .. }
        | Instruction::GreaterValueValue { lhs, rhs, .. }
        | Instruction::LessOrEqualValueValue { lhs, rhs, .. }
        | Instruction::GreaterOrEqualValueValue { lhs, rhs, .. }
        | Instruction::EqualValueValue { lhs, rhs, .. }
        | Instruction::NotEqualValueValue { lhs, rhs, .. }
        | Instruction::AndValueValue { lhs, rhs, .. }
        | Instruction::OrValueValue { lhs, rhs, .. }
        | Instruction::AccessAssignValueValue {
            to: lhs,
            value: rhs,
            ..
        } => {
            instruction_changed |=
                backpatch_value(lhs, block_addresses, address_to_label, disassembler);

            instruction_changed |=
                backpatch_value(rhs, block_addresses, address_to_label, disassembler);
        }
        Instruction::Jump { destination } => {
            let label_index = address_to_label
                .get(destination)
                .copied()
                .unwrap_or_else(|| {
                    println!("{address_to_label:?}");
                    panic!("destination was not a label {destination}");
                });

            let new_destination = block_addresses
                .get(usize::try_from(label_index).expect("label index was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {destination}");
                });

            if new_destination != *destination {
                *destination = new_destination;

                instruction_changed |= true;
            }
        }
        Instruction::BranchRegister {
            when_true,
            otherwise,
            ..
        } => {
            let label_index = address_to_label.get(when_true).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {when_true}");
            });

            let new_when_true = block_addresses
                .get(usize::try_from(label_index).expect("destination was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {when_true}");
                });

            if new_when_true != *when_true {
                *when_true = new_when_true;

                instruction_changed |= true;
            }

            let label_index = address_to_label.get(otherwise).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {otherwise}");
            });

            let new_otherwise = block_addresses
                .get(usize::try_from(label_index).expect("destination was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {otherwise}");
                });

            if new_otherwise != *otherwise {
                *otherwise = new_otherwise;

                instruction_changed |= true;
            }
        }
        Instruction::BranchValue {
            condition,
            when_true,
            otherwise,
        } => {
            instruction_changed |=
                backpatch_value(condition, block_addresses, address_to_label, disassembler);

            let label_index = address_to_label.get(when_true).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {when_true}");
            });

            let new_when_true = block_addresses
                .get(usize::try_from(label_index).expect("destination was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {when_true}");
                });

            if new_when_true != *when_true {
                *when_true = new_when_true;

                instruction_changed |= true;
            }

            let label_index = address_to_label.get(otherwise).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {otherwise}");
            });

            let new_otherwise = block_addresses
                .get(usize::try_from(label_index).expect("destination was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {otherwise}");
                });

            if new_otherwise != *otherwise {
                *otherwise = new_otherwise;

                instruction_changed |= true;
            }
        }
    }

    instruction_changed
}

fn backpatch_value<T>(
    value: &mut CopyableValue,
    block_addresses: &[u32],
    address_to_label: &HashMap<u32, u32>,
    disassembler: &mut T,
) -> bool
where
    T: Instructive,
{
    match value {
        CopyableValue::Fn { address, .. } => {
            let label_index = address_to_label.get(address).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {address}");
            });

            let new_destination = block_addresses
                .get(usize::try_from(label_index).expect("destination was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {address}");
                });

            if new_destination == *address {
                false
            } else {
                *address = new_destination;

                true
            }
        }
        CopyableValue::ValueIndex(value_index) => {
            let value_index = *value_index;

            match disassembler.values_mut().get_mut(usize::from(value_index)) {
                Some(Value::Compound(values) | Value::TaggedCompound { fields: values, .. }) => {
                    let mut taken_values = std::mem::take(values);

                    let mut changed = false;

                    for value in &mut taken_values {
                        changed |=
                            backpatch_value(value, block_addresses, address_to_label, disassembler);
                    }

                    match disassembler.values_mut().get_mut(usize::from(value_index)) {
                        Some(
                            Value::Compound(values) | Value::TaggedCompound { fields: values, .. },
                        ) => {
                            *values = taken_values;
                        }
                        _ => unreachable!(
                            "the outer match guarantees only these variants are matched"
                        ),
                    }

                    changed
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn find_labels(
    bytecode: &mut Vec<u8>,
    block_addresses: &mut [u32],
    address_to_label: &mut HashMap<u32, u32>,
    remove_labels: bool,
) {
    address_to_label.clear();

    let mut disassembler = Compiler {
        ip: 2,
        constants: vec![],
        values: vec![],
    };

    let mut label_index = 0u32;

    while disassembler.ip < bytecode.len() {
        if matches!(
            Instruction::disassemble(bytecode, &mut disassembler),
            Instruction::Label
        ) {
            if remove_labels {
                disassembler.ip -= 1;
                bytecode.splice((disassembler.ip)..=(disassembler.ip), []);
            } else {
                address_to_label.insert(
                    block_addresses
                        [usize::try_from(label_index).expect("label index was not a valid usize")],
                    label_index,
                );

                block_addresses
                    [usize::try_from(label_index).expect("label index was not a valid usize")] =
                    u32::try_from(disassembler.ip).expect("ip was not a valid u32")
                        - label_index
                        - 1;

                label_index += 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CopyableValue {
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
    Fn {
        address: u32,
        max_register_count: u16,
    },
    NativeFn(NativeFn),
    ValueIndex(ValueIndex),
    Register(u16),
}

#[derive(Debug, Clone)]
pub enum Value {
    Compound(Vec<CopyableValue>),
    TaggedCompound {
        fields: Vec<CopyableValue>,
        tag: u16,
    },
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueIndex(pub usize);

impl From<ValueIndex> for usize {
    fn from(value: ValueIndex) -> Self {
        value.0
    }
}

pub trait Assemble<T>
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8>;
}

impl<T> Assemble<T> for IrInstruction<IrValue>
where
    T: Instructive,
{
    #[allow(clippy::too_many_lines)]
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        match self {
            Self::NoOp => vec![],
            Self::Panic => Instruction::Panic.assemble(assembler),
            Self::FnLabel(fn_label) => Instruction::FnLabel {
                fn_label: Box::new(FnLabel::new(
                    fn_label.file_name().to_string(),
                    fn_label.function_label().to_string(),
                )),
            }
            .assemble(assembler),
            Self::Unary { op, operand, to } => match (to, op, operand) {
                (
                    Location::Register(to),
                    UnaryOp::Not,
                    ValueOrLocation::At(Location::Register(operand)),
                ) => Instruction::NotRegister {
                    to: *to,
                    operand: *operand,
                }
                .assemble(assembler),
                (Location::Register(to), UnaryOp::Not, ValueOrLocation::Value(value)) => {
                    Instruction::NotValue {
                        to: *to,
                        operand: CopyableValue::disassemble(
                            value.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
                (
                    Location::Register(to),
                    UnaryOp::Negate,
                    ValueOrLocation::At(Location::Register(operand)),
                ) => Instruction::NegateRegister {
                    to: *to,
                    operand: *operand,
                }
                .assemble(assembler),
                (Location::Register(to), UnaryOp::Negate, ValueOrLocation::Value(value)) => {
                    Instruction::NegateValue {
                        to: *to,
                        operand: CopyableValue::disassemble(
                            value.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
            },
            Self::Binary { op, lhs, rhs, to } => match (to, op, lhs, rhs) {
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::MultiplyRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::MultiplyRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::MultiplyValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::MultiplyValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::DivideRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::DivideRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::DivideValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::DivideValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::RemainderRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::RemainderRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::RemainderValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::RemainderValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::AddRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::AddRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::AddValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::AddValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::SubtractRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::SubtractRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::SubtractValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::SubtractValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::LessRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::LessValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::GreaterRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::GreaterValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessOrEqualRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::LessOrEqualRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessOrEqualValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::LessOrEqualValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterOrEqualRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::GreaterOrEqualRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterOrEqualValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::GreaterOrEqualValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::EqualRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::EqualRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::EqualValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::EqualValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::NotEqualRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::NotEqualRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::NotEqualValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::NotEqualValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::AndRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::AndRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::AndValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::AndValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::OrRegisterRegister {
                    to: *to,
                    lhs: *lhs,
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::OrRegisterValue {
                    to: *to,
                    lhs: *lhs,
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::OrValueRegister {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: *rhs,
                }
                .assemble(assembler),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::Value(lhs),
                    ValueOrLocation::Value(rhs),
                ) => Instruction::OrValueValue {
                    to: *to,
                    lhs: CopyableValue::disassemble(lhs.assemble(assembler).as_slice(), assembler),
                    rhs: CopyableValue::disassemble(rhs.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
            },
            Self::Assign { value, to } => match (to, value) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(operand))) => {
                    Instruction::AssignRegister {
                        to: *to,
                        operand: *operand,
                    }
                    .assemble(assembler)
                }
                (Location::Register(to), ValueOrLocation::Value(value)) => {
                    Instruction::AssignValue {
                        to: *to,
                        operand: CopyableValue::disassemble(
                            value.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
            },
            Self::Push(value) => match value {
                ValueOrLocation::At(Location::Register(operand)) => {
                    Instruction::PushRegister { operand: *operand }.assemble(assembler)
                }
                ValueOrLocation::Value(value) => Instruction::PushValue {
                    operand: CopyableValue::disassemble(
                        value.assemble(assembler).as_slice(),
                        assembler,
                    ),
                }
                .assemble(assembler),
            },
            Self::Call { callee, arity, to } => match (to, callee) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(callee))) => {
                    Instruction::CallRegister {
                        to: *to,
                        arity: *arity,
                        callee: *callee,
                    }
                    .assemble(assembler)
                }
                (Location::Register(to), ValueOrLocation::Value(callee)) => {
                    Instruction::CallValue {
                        to: *to,
                        arity: *arity,
                        callee: CopyableValue::disassemble(
                            callee.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
            },
            Self::Access { index, of, to } => match (to, of) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(of))) => {
                    Instruction::AccessRegister {
                        to: *to,
                        index: *index,
                        of: *of,
                    }
                    .assemble(assembler)
                }
                (Location::Register(to), ValueOrLocation::Value(of)) => Instruction::AccessValue {
                    to: *to,
                    index: *index,
                    of: CopyableValue::disassemble(of.assemble(assembler).as_slice(), assembler),
                }
                .assemble(assembler),
            },
            Self::AccessAssign { index, of, value } => match (of, value) {
                (
                    ValueOrLocation::At(Location::Register(to)),
                    ValueOrLocation::At(Location::Register(value)),
                ) => Instruction::AccessAssignRegisterRegister {
                    index: *index,
                    to: *to,
                    value: *value,
                }
                .assemble(assembler),
                (ValueOrLocation::At(Location::Register(to)), ValueOrLocation::Value(value)) => {
                    Instruction::AccessAssignRegisterValue {
                        index: *index,
                        to: *to,
                        value: CopyableValue::disassemble(
                            value.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
                (ValueOrLocation::Value(to), ValueOrLocation::At(Location::Register(value))) => {
                    Instruction::AccessAssignValueRegister {
                        index: *index,
                        to: CopyableValue::disassemble(
                            to.assemble(assembler).as_slice(),
                            assembler,
                        ),
                        value: *value,
                    }
                    .assemble(assembler)
                }
                (ValueOrLocation::Value(to), ValueOrLocation::Value(value)) => {
                    Instruction::AccessAssignValueValue {
                        index: *index,
                        to: CopyableValue::disassemble(
                            to.assemble(assembler).as_slice(),
                            assembler,
                        ),
                        value: CopyableValue::disassemble(
                            value.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
            },
            Self::GetTag { of, to } => match (to, of) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(operand))) => {
                    Instruction::GetTagRegister {
                        to: *to,
                        of: *operand,
                    }
                    .assemble(assembler)
                }
                (Location::Register(to), ValueOrLocation::Value(value)) => {
                    Instruction::GetTagValue {
                        to: *to,
                        of: CopyableValue::disassemble(
                            value.assemble(assembler).as_slice(),
                            assembler,
                        ),
                    }
                    .assemble(assembler)
                }
            },
            Self::PopN(count) => Instruction::PopN {
                count: u16::try_from(*count).expect("max register count must fit within a u16"),
            }
            .assemble(assembler),
            Self::Jump(destination) => Instruction::Jump {
                destination: u32::from(*destination),
            }
            .assemble(assembler),
            Self::Branch {
                condition,
                when_true,
                otherwise,
            } => match condition {
                ValueOrLocation::At(Location::Register(condition)) => Instruction::BranchRegister {
                    condition: *condition,
                    when_true: u32::from(*when_true),
                    otherwise: u32::from(*otherwise),
                }
                .assemble(assembler),
                ValueOrLocation::Value(condition) => Instruction::BranchValue {
                    condition: CopyableValue::disassemble(
                        condition.assemble(assembler).as_slice(),
                        assembler,
                    ),
                    when_true: u32::from(*when_true),
                    otherwise: u32::from(*otherwise),
                }
                .assemble(assembler),
            },
            Self::Return(value) => match value {
                ValueOrLocation::At(Location::Register(value)) => {
                    Instruction::ReturnRegister { value: *value }.assemble(assembler)
                }
                ValueOrLocation::Value(value) => Instruction::ReturnValue {
                    value: CopyableValue::disassemble(
                        value.assemble(assembler).as_slice(),
                        assembler,
                    ),
                }
                .assemble(assembler),
            },
        }
    }
}

impl<T> Assemble<T> for ValueOrLocation<IrValue>
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        match self {
            Self::Value(value) => value.assemble(assembler),
            Self::At(location) => location.assemble(assembler),
        }
    }
}

impl<T> Assemble<T> for Location
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        match self {
            Self::Register(index) => index.assemble(assembler),
        }
    }
}

impl<T> Assemble<T> for IrValue
where
    T: Instructive,
{
    #[allow(clippy::too_many_lines)]
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        match self {
            Self::Never => {
                vec![u8::from(Instruction::Never)]
            }
            Self::U8(value) => {
                let mut bytes = vec![u8::from(Instruction::U8)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I8(value) => {
                let mut bytes = vec![u8::from(Instruction::I8)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U16(value) => {
                let mut bytes = vec![u8::from(Instruction::U16)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I16(value) => {
                let mut bytes = vec![u8::from(Instruction::I16)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U32(value) => {
                let mut bytes = vec![u8::from(Instruction::U32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I32(value) => {
                let mut bytes = vec![u8::from(Instruction::I32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::U64(value) => {
                let mut bytes = vec![u8::from(Instruction::U64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::I64(value) => {
                let mut bytes = vec![u8::from(Instruction::I64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::F32(value) => {
                let mut bytes = vec![u8::from(Instruction::F32)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::F64(value) => {
                let mut bytes = vec![u8::from(Instruction::F64)];

                bytes.extend_from_slice(value.to_le_bytes().as_slice());

                bytes
            }
            Self::Boolean(value) => {
                vec![u8::from(Instruction::Boolean), u8::from(*value)]
            }
            Self::Unit => {
                vec![u8::from(Instruction::Unit)]
            }
            Self::String(text) => {
                let mut bytes = vec![u8::from(Instruction::String)];

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
                let mut bytes = vec![u8::from(Instruction::Fn)];

                bytes.extend_from_slice(u32::from(*block_index).to_le_bytes().as_slice());

                bytes.extend_from_slice(
                    u16::try_from(*max_register_count)
                        .expect("a function's max register count was not a valid u32")
                        .to_le_bytes()
                        .as_slice(),
                );

                bytes
            }
            Self::NativeFn(native_fn) => {
                let mut bytes = vec![u8::from(Instruction::NativeFn)];

                bytes.extend_from_slice(u16::from(*native_fn).to_le_bytes().as_slice());

                bytes
            }
            Self::Compound(values) => {
                let mut bytes = vec![u8::from(Instruction::Compound)];

                bytes.extend_from_slice(
                    u16::try_from(values.len())
                        .expect("type checking guarantees the number of fields fits within a u16")
                        .to_le_bytes()
                        .as_slice(),
                );

                for value in values {
                    bytes.extend_from_slice(value.assemble(assembler).as_slice());
                }

                bytes
            }
            Self::TaggedCompound { tag, fields } => {
                let mut bytes = vec![u8::from(Instruction::TaggedCompound)];

                bytes.extend_from_slice(tag.to_le_bytes().as_slice());

                bytes.extend_from_slice(
                    u16::try_from(fields.len())
                        .expect("type checking guarantees the number of fields fits within a u16")
                        .to_le_bytes()
                        .as_slice(),
                );

                for value in fields {
                    bytes.extend_from_slice(value.assemble(assembler).as_slice());
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
    fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self;
}

pub trait Instructive: Sized {
    fn ip_mut(&mut self) -> &mut usize;

    fn constants_mut(&mut self) -> &mut Vec<ValueIndex>;

    fn values_mut(&mut self) -> &mut Vec<Value>;
}

macro_rules! we_make_numbers_here {
    (
        $number_ty:ty, $number_type_id:ident $(,)?
    ) => {
        impl<T> $crate::targets::vm::Assemble<T> for $number_ty
        where
            T: $crate::targets::vm::Instructive,
        {
            fn assemble(&self, assembler: &mut T) -> Vec<u8> {
                *assembler.ip_mut() = 0;

                self.to_le_bytes().to_vec()
            }
        }

        impl<T> $crate::targets::vm::Disassemble<T> for $number_ty
        where
            T: $crate::targets::vm::Instructive,
        {
            fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
                const SIZE: usize = std::mem::size_of::<$number_ty>();

                bytecode
                    .get(
                        (*disassembler.ip_mut())..{
                            *disassembler.ip_mut() += SIZE;
                            *disassembler.ip_mut()
                        },
                    )
                    .and_then(|bytes| bytes.as_array::<SIZE>())
                    .map(|bytes| Self::from_le_bytes(*bytes))
                    .unwrap_or_else(|| {
                        panic!("{} was expected", std::any::type_name::<Self>());
                    })
            }
        }
    };
}

we_make_numbers_here!(u8, U8);
we_make_numbers_here!(i8, I8);

we_make_numbers_here!(u16, U16);
we_make_numbers_here!(i16, I16);

we_make_numbers_here!(u32, U32);
we_make_numbers_here!(i32, I32);

we_make_numbers_here!(u64, U64);
we_make_numbers_here!(i64, I64);

we_make_numbers_here!(f32, F32);
we_make_numbers_here!(f64, F64);

impl<T> Assemble<T> for bool
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        u8::from(*self).to_le_bytes().to_vec()
    }
}

impl<T> Disassemble<T> for bool
where
    T: Instructive,
{
    fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
        const SIZE: usize = std::mem::size_of::<u8>();

        bytecode
            .get(
                (*disassembler.ip_mut())..{
                    *disassembler.ip_mut() += SIZE;
                    *disassembler.ip_mut()
                },
            )
            .and_then(|bytes| bytes.as_array::<SIZE>())
            .map(|bytes| u8::from_le_bytes(*bytes))
            .and_then(|byte| Self::try_from(byte).ok())
            .unwrap_or_else(|| {
                panic!("{} was expected", std::any::type_name::<Self>());
            })
    }
}

impl<T> Assemble<T> for String
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        let mut bytes = u64::try_from(self.len())
            .expect("128-bit usize not allowed!  sorry!")
            .to_le_bytes()
            .to_vec();

        bytes.extend_from_slice(self.as_bytes());

        bytes
    }
}

impl<T> Disassemble<T> for String
where
    T: Instructive,
{
    fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
        let length = u64::disassemble(bytecode, disassembler);

        usize::try_from(length).ok().map_or_else(
            || {
                panic!("{} was expected", std::any::type_name::<Self>());
            },
            |length| {
                bytecode
                    .get(
                        (*disassembler.ip_mut())..{
                            *disassembler.ip_mut() += length;
                            *disassembler.ip_mut()
                        },
                    )
                    .and_then(|bytes| Self::from_utf8(bytes.to_vec()).ok())
                    .expect("out of instructions!")
            },
        )
    }
}

impl<T> Assemble<T> for Register
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        let register = u16::from(*self);

        let register = register.to_le_bytes();

        [Instruction::Register.assemble(assembler), register.to_vec()].concat()
    }
}

impl<T> Disassemble<T> for Register
where
    T: Instructive,
{
    fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
        match Instruction::disassemble(bytecode, disassembler) {
            Instruction::Register => Self(u16::disassemble(bytecode, disassembler)),
            instruction => panic!("expected a register, got {instruction:?}"),
        }
    }
}

impl<T> Assemble<T> for Box<FnLabel>
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        [
            self.file_name().to_string().assemble(assembler),
            self.function_label().to_string().assemble(assembler),
        ]
        .concat()
    }
}

impl<T> Disassemble<T> for Box<FnLabel>
where
    T: Instructive,
{
    fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
        Self::new(FnLabel::new(
            String::disassemble(bytecode, disassembler),
            String::disassemble(bytecode, disassembler),
        ))
    }
}

impl<T> Assemble<T> for CopyableValue
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        match self {
            Self::Never => Instruction::Never.assemble(assembler),
            Self::U8(value) => [
                Instruction::U8.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::I8(value) => [
                Instruction::I8.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::U16(value) => [
                Instruction::U16.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::I16(value) => [
                Instruction::I16.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::U32(value) => [
                Instruction::U32.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::I32(value) => [
                Instruction::I32.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::U64(value) => [
                Instruction::U64.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::I64(value) => [
                Instruction::I64.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::F32(value) => [
                Instruction::F32.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::F64(value) => [
                Instruction::F64.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::Boolean(value) => [
                Instruction::Boolean.assemble(assembler),
                value.assemble(assembler),
            ]
            .concat(),
            Self::Unit => Instruction::Unit.assemble(assembler),
            Self::Fn {
                address,
                max_register_count,
            } => [
                Instruction::Fn.assemble(assembler),
                address.assemble(assembler),
                max_register_count.assemble(assembler),
            ]
            .concat(),
            Self::NativeFn(native_fn) => [
                Instruction::NativeFn.assemble(assembler),
                u16::from(*native_fn).assemble(assembler),
            ]
            .concat(),
            Self::ValueIndex(value_index) => assembler
                .values_mut()
                .get(usize::from(*value_index))
                .cloned()
                .unwrap_or_else(|| {
                    panic!("tried to assemble a value that doesn't exist {value_index:?}")
                })
                .assemble(assembler),
            Self::Register(index) => [
                Instruction::Register.assemble(assembler),
                index.assemble(assembler),
            ]
            .concat(),
        }
    }
}

impl<T> Disassemble<T> for CopyableValue
where
    T: Instructive,
{
    fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
        match Instruction::disassemble(bytecode, disassembler) {
            Instruction::U8 => Self::U8(u8::disassemble(bytecode, disassembler)),
            Instruction::I8 => Self::I8(i8::disassemble(bytecode, disassembler)),
            Instruction::U16 => Self::U16(u16::disassemble(bytecode, disassembler)),
            Instruction::I16 => Self::I16(i16::disassemble(bytecode, disassembler)),
            Instruction::U32 => Self::U32(u32::disassemble(bytecode, disassembler)),
            Instruction::I32 => Self::I32(i32::disassemble(bytecode, disassembler)),
            Instruction::U64 => Self::U64(u64::disassemble(bytecode, disassembler)),
            Instruction::I64 => Self::I64(i64::disassemble(bytecode, disassembler)),
            Instruction::F32 => Self::F32(f32::disassemble(bytecode, disassembler)),
            Instruction::F64 => Self::F64(f64::disassemble(bytecode, disassembler)),
            Instruction::Boolean => Self::Boolean(bool::disassemble(bytecode, disassembler)),
            Instruction::Unit => Self::Unit,
            Instruction::Fn => {
                let address = u32::disassemble(bytecode, disassembler);

                let max_register_count = u16::disassemble(bytecode, disassembler);

                Self::Fn {
                    address,
                    max_register_count,
                }
            }
            Instruction::NativeFn => {
                let native_fn = NativeFn::try_from(u16::disassemble(bytecode, disassembler))
                    .unwrap_or_else(|error| {
                        panic!("{error} is not a valid native function");
                    });

                Self::NativeFn(native_fn)
            }
            Instruction::Compound => {
                let field_count = u16::disassemble(bytecode, disassembler);

                let mut fields = vec![];

                for _ in 0..field_count {
                    fields.push(Self::disassemble(bytecode, disassembler));
                }

                disassembler.values_mut().push(Value::Compound(fields));

                let value_index = ValueIndex(disassembler.values_mut().len() - 1);

                disassembler.constants_mut().push(value_index);

                Self::ValueIndex(value_index)
            }
            Instruction::TaggedCompound => {
                let tag = u16::disassemble(bytecode, disassembler);

                let field_count = u16::disassemble(bytecode, disassembler);

                let mut fields = vec![];

                for _ in 0..field_count {
                    fields.push(Self::disassemble(bytecode, disassembler));
                }

                disassembler
                    .values_mut()
                    .push(Value::TaggedCompound { tag, fields });

                let value_index = ValueIndex(disassembler.values_mut().len() - 1);

                disassembler.constants_mut().push(value_index);

                Self::ValueIndex(value_index)
            }
            Instruction::String => {
                let value = String::disassemble(bytecode, disassembler);

                disassembler.values_mut().push(Value::String(value));

                let value_index = ValueIndex(disassembler.values_mut().len() - 1);

                disassembler.constants_mut().push(value_index);

                Self::ValueIndex(value_index)
            }
            Instruction::Never => Self::Never,
            Instruction::Register => Self::Register(u16::disassemble(bytecode, disassembler)),
            instruction => {
                panic!("expected a value, but got {instruction:?}");
            }
        }
    }
}

impl<T> Assemble<T> for Value
where
    T: Instructive,
{
    fn assemble(&self, assembler: &mut T) -> Vec<u8> {
        *assembler.ip_mut() = 0;

        match self {
            Self::Compound(values) => {
                let mut bytes = Instruction::Compound.assemble(assembler);

                bytes.extend(
                    u16::try_from(values.len())
                        .expect("type checking guarantees the number of fields fits within a u16")
                        .to_le_bytes(),
                );

                for value in values {
                    bytes.extend(value.assemble(assembler));
                }

                bytes
            }
            Self::TaggedCompound {
                tag,
                fields: values,
            } => {
                let mut bytes = Instruction::TaggedCompound.assemble(assembler);

                bytes.extend(tag.to_le_bytes());

                bytes.extend(
                    u16::try_from(values.len())
                        .expect("type checking guarantees the number of fields fits within a u16")
                        .to_le_bytes(),
                );

                for value in values {
                    bytes.extend(value.assemble(assembler));
                }

                bytes
            }
            Self::String(value) => {
                let mut bytes = Instruction::String.assemble(assembler);

                bytes.extend(
                    u64::try_from(value.len())
                        .expect("128-bit usize not allowed!  sorry!")
                        .to_le_bytes(),
                );

                bytes.extend(value.as_bytes());

                bytes
            }
        }
    }
}

macro_rules! instruction {
    (
        $v:vis $instructions:ident ;
        $($lit:literal $name:ident $(=> { $operand:ident : $operand_ty:ty $(, $operands:ident: $operands_ty:ty)* $(,)? })?,)+
    ) => {
        #[derive(Debug)]
        $v enum $instructions {
            $($name $({ $operand: $operand_ty $(, $operands: $operands_ty)* , })?,)+
        }

        impl<T> $crate::targets::vm::Assemble<T> for $instructions
        where
            T: $crate::targets::vm::Instructive,
        {
            fn assemble(&self, assembler: &mut T) -> Vec<u8> {
                *assembler.ip_mut() = 0;

                match self {
                    $(
                        Self::$name $({ $operand $(, $operands)* })? => {
                            $(
                                let $operand = <$operand_ty as $crate::targets::vm::Assemble<T>>::assemble($operand, assembler);

                                $(
                                    let $operands = <$operands_ty as $crate::targets::vm::Assemble<T>>::assemble($operands, assembler);
                                )*
                            )?

                            [vec![$lit] $(, $operand $(, $operands)*)?].concat()
                        }
                    )+
                }
            }
        }

        impl<T> $crate::targets::vm::Disassemble<T> for $instructions
        where
            T: $crate::targets::vm::Instructive,
        {
            fn disassemble(bytecode: &[u8], disassembler: &mut T) -> Self {
                match bytecode.get({
                    let previous_ip = *disassembler.ip_mut();
                    *disassembler.ip_mut() += 1;
                    previous_ip
                }) {
                    None => panic!("out of instructions!"),
                    $(
                        Some($lit) => {
                            $(
                                let $operand = <$operand_ty as $crate::targets::vm::Disassemble<T>>::disassemble(bytecode, disassembler);

                                $(
                                    let $operands = <$operands_ty as $crate::targets::vm::Disassemble<T>>::disassemble(bytecode, disassembler);
                                )*
                            )?

                            Self::$name $({
                                $operand,
                                $(
                                    $operands,
                                )*
                            })?
                        }
                    )+
                    Some(op_code) => {
                        panic!("unknown op code {op_code:#X}");
                    }
                }
            }
        }

        impl From<$instructions> for u8 {
            fn from(value: $instructions) -> Self {
                match value {
                    $(
                        $instructions::$name $({ $operand: _ $(, $operands: _)* })? => $lit,
                    )+
                }
            }
        }
    };
}

instruction! {
    pub Instruction ;
    0x00 NoOp,
    0x01 Label,
    0x02 FnLabel => { fn_label: Box<FnLabel>, },
    0x10 NotRegister => { to: Register, operand: Register },
    0x11 NotValue => { to: Register, operand: CopyableValue },
    0x12 NegateRegister => { to: Register, operand: Register },
    0x13 NegateValue => { to: Register, operand: CopyableValue },
    0x20 MultiplyRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x21 MultiplyRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x22 MultiplyValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x23 MultiplyValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x24 DivideRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x25 DivideRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x26 DivideValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x27 DivideValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x28 RemainderRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x29 RemainderRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x2A RemainderValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x2B RemainderValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x2C AddRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x2D AddRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x2E AddValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x2F AddValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x30 SubtractRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x31 SubtractRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x32 SubtractValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x33 SubtractValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x34 LessRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x35 LessRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x36 LessValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x37 LessValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x38 GreaterRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x39 GreaterRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x3A GreaterValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x3B GreaterValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x3C LessOrEqualRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x3D LessOrEqualRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x3E LessOrEqualValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x3F LessOrEqualValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x40 GreaterOrEqualRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x41 GreaterOrEqualRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x42 GreaterOrEqualValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x43 GreaterOrEqualValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x44 EqualRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x45 EqualRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x46 EqualValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x47 EqualValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x48 NotEqualRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x49 NotEqualRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x4A NotEqualValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x4B NotEqualValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x4C AndRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x4D AndRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x4E AndValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x4F AndValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x50 OrRegisterRegister => { to: Register, lhs: Register, rhs: Register },
    0x51 OrRegisterValue => { to: Register, lhs: Register, rhs: CopyableValue },
    0x52 OrValueRegister => { to: Register, lhs: CopyableValue, rhs: Register },
    0x53 OrValueValue => { to: Register, lhs: CopyableValue, rhs: CopyableValue },
    0x54 AssignRegister => { to: Register, operand: Register },
    0x55 AssignValue => { to: Register, operand: CopyableValue },
    0x56 PushRegister => { operand: Register },
    0x57 PushValue => {  operand: CopyableValue },
    0x58 AccessRegister => { to: Register, index: u16, of: Register },
    0x59 AccessValue => { to: Register, index: u16, of: CopyableValue },
    0x5A AccessAssignRegisterRegister => { to: Register, index: u16, value: Register },
    0x5B AccessAssignRegisterValue => { to: Register, index: u16, value: CopyableValue },
    0x5C AccessAssignValueRegister => { to: CopyableValue, index: u16, value: Register },
    0x5D AccessAssignValueValue => { to: CopyableValue, index: u16, value: CopyableValue },
    0x5E CallRegister => { to: Register, arity: u16, callee: Register },
    0x5F CallValue => { to: Register, arity: u16, callee: CopyableValue },
    0x60 GetTagRegister => { to: Register, of: Register },
    0x61 GetTagValue => { to: Register, of: CopyableValue },
    0x62 PopN => { count: u16 },
    0x63 Jump => { destination: u32 },
    0x64 BranchRegister => { condition: Register, when_true: u32, otherwise: u32 },
    0x65 BranchValue => { condition: CopyableValue, when_true: u32, otherwise: u32 },
    0x66 ReturnRegister => { value: Register },
    0x67 ReturnValue => { value: CopyableValue },
    0x68 Panic,
    0xA0 I64,
    0xA1 F64,
    0xA2 Boolean,
    0xA3 Unit,
    0xA6 Fn,
    0xA8 NativeFn,
    0xA9 Compound,
    0xAA TaggedCompound,
    0xAB U8,
    0xAC I8,
    0xAD U16,
    0xAE I16,
    0xAF U32,
    0xB0 I32,
    0xB1 U64,
    0xB2 String,
    0xB3 F32,
    0xB4 Never,
    0xE0 Register,
}
