pub mod bytecode_macro;

use crate::{
    ApartOptions, CompileSuccess,
    middle_ir::{
        BinaryOp, Block, FnLabel, Instruction as IrInstruction, Location, NativeFn, Typed, UnaryOp,
        Value as IrValue, ValueOrLocation,
    },
    type_check::TypeIndex,
};

use std::collections::HashMap;

struct Compiler {
    ip: usize,
    literals: Vec<ValueIndex>,
    arena: Vec<GarbageCollectedValue>,
}

impl Instructive for Compiler {
    fn ip_mut(&mut self) -> &mut usize {
        &mut self.ip
    }

    fn literals_mut(&mut self) -> &mut Vec<ValueIndex> {
        &mut self.literals
    }

    fn arena_mut(&mut self) -> &mut Vec<GarbageCollectedValue> {
        &mut self.arena
    }

    fn ip(&self) -> usize {
        self.ip
    }

    fn literals(&self) -> &[ValueIndex] {
        self.literals.as_slice()
    }

    fn arena(&self) -> &[GarbageCollectedValue] {
        self.arena.as_slice()
    }
}

/// # Panics
/// - panics if `bytecode.len()` > `u32::MAX`
/// - panics if the number of instructions in the output exceeds `u32::MAX`
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn compile(options: &ApartOptions, compiled: &CompileSuccess<'_>) -> Vec<u8> {
    let ir = compiled.result().ir();

    let mut instructive = Compiler {
        ip: 0,
        literals: vec![],
        arena: vec![],
    };

    let mut block_addresses = vec![0u32; ir.blocks().len()];
    let mut address_to_label = HashMap::new();

    let mut instructions = vec![];

    for (b, block) in ir.blocks().iter().enumerate() {
        block_addresses[b] =
            u32::try_from(instructions.len()).expect("block index was not a valid u32");

        address_to_label.insert(
            u32::try_from(b).expect("block index was not a valid u32"),
            u32::try_from(b).expect("block index was not a valid u32"),
        );

        instructions.push(Instruction::Label);

        for instruction in block.instructions() {
            let bytecode = (compiled, instruction).encode(&mut instructive);

            let mut total_size = 0;

            while let Some((instruction, instruction_size)) =
                Instruction::decode(&bytecode[total_size..], &mut instructive)
            {
                instructions.push(instruction);

                total_size += instruction_size;
            }
        }
    }

    let mut bytecode = vec![];

    bytecode.extend(
        ir.blocks()
            .first()
            .map_or(0u16, Block::max_register_count)
            .to_le_bytes(),
    );

    for instruction in instructions {
        if let Instruction::ApartFnLabel { .. } = instruction {
            if !options.strip_debug_info {
                bytecode.extend(instruction.encode(&mut instructive));
            }
        } else {
            bytecode.extend(instruction.encode(&mut instructive));
        }
    }

    loop {
        if !backpatch(
            &mut bytecode,
            &mut instructive,
            block_addresses.as_slice(),
            &address_to_label,
        ) {
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
pub fn decompile<T>(bytecode: &[u8], instructive: &mut T) -> Vec<Instruction>
where
    T: Instructive,
{
    *instructive.ip_mut() = 2;

    let mut instructions = vec![];

    let mut address_to_label = HashMap::new();

    let mut block_addresses = vec![];

    while instructive.ip() < bytecode.len() {
        address_to_label.insert(
            u32::try_from(instructive.ip()).expect("instruction pointer was not a valid u32"),
            u32::try_from(instructions.len()).expect("instruction count was not a valid u32"),
        );

        block_addresses.push(
            u32::try_from(instructions.len()).expect("instruction count was not a valid u32"),
        );

        let Some((instruction, instruction_size)) =
            Instruction::decode(&bytecode[(instructive.ip())..], instructive)
        else {
            panic!("failed to decode an instruction before reaching the bytecode's end");
        };

        *instructive.ip_mut() += instruction_size;

        instructions.push(instruction);
    }

    for instruction in &mut instructions {
        backpatch_instruction(
            block_addresses.as_slice(),
            &address_to_label,
            instructive,
            instruction,
        );
    }

    *instructive.ip_mut() = 0;

    instructions
}

fn backpatch<T>(
    bytecode: &mut Vec<u8>,
    instructive: &mut T,
    block_addresses: &[u32],
    address_to_label: &HashMap<u32, u32>,
) -> bool
where
    T: Instructive,
{
    *instructive.ip_mut() = 2;

    let mut changed = false;

    while instructive.ip() < bytecode.len() {
        let at = instructive.ip();

        let Some((mut instruction, instruction_size)) =
            Instruction::decode(&bytecode[(instructive.ip())..], instructive)
        else {
            panic!("failed to decode an instruction before reaching the bytecode's end");
        };

        *instructive.ip_mut() += instruction_size;

        if backpatch_instruction(
            block_addresses,
            address_to_label,
            instructive,
            &mut instruction,
        ) {
            replace_instruction(bytecode, instructive, &instruction, at);

            changed |= true;
        }
    }

    changed
}

fn replace_instruction<T>(
    bytecode: &mut Vec<u8>,
    instructive: &mut T,
    instruction: &Instruction,
    at: usize,
) where
    T: Instructive,
{
    let ip = *instructive.ip_mut();

    let assembled = instruction.encode(instructive);

    let changed_instruction_byte_count = assembled.len();

    bytecode.splice(at..ip, assembled);

    *instructive.ip_mut() = at + changed_instruction_byte_count;
}

#[allow(clippy::too_many_lines)]
fn backpatch_instruction<T>(
    block_addresses: &[u32],
    address_to_label: &HashMap<u32, u32>,
    instructive: &mut T,
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
        | Instruction::ApartFnLabel { .. }
        | Instruction::Not { .. }
        | Instruction::Negate { .. }
        | Instruction::Multiply { .. }
        | Instruction::Divide { .. }
        | Instruction::Remainder { .. }
        | Instruction::Add { .. }
        | Instruction::Subtract { .. }
        | Instruction::Less { .. }
        | Instruction::Greater { .. }
        | Instruction::LessOrEqual { .. }
        | Instruction::GreaterOrEqual { .. }
        | Instruction::Equal { .. }
        | Instruction::NotEqual { .. }
        | Instruction::And { .. }
        | Instruction::Or { .. }
        | Instruction::Assign { .. }
        | Instruction::Push { .. }
        | Instruction::Access { .. }
        | Instruction::AccessAssign { .. }
        | Instruction::Call { .. }
        | Instruction::GetTag { .. }
        | Instruction::Return { .. } => {}
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
                backpatch_value(value, block_addresses, address_to_label, instructive);
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
                backpatch_value(lhs, block_addresses, address_to_label, instructive);

            instruction_changed |=
                backpatch_value(rhs, block_addresses, address_to_label, instructive);
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
        Instruction::Branch {
            when_true,
            otherwise,
            ..
        } => {
            let label_index = address_to_label.get(when_true).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {when_true}");
            });

            let new_destination = block_addresses
                .get(usize::try_from(label_index).expect("label index was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {when_true}");
                });

            if new_destination != *when_true {
                *when_true = new_destination;

                instruction_changed |= true;
            }

            let label_index = address_to_label.get(otherwise).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {otherwise}");
            });

            let new_destination = block_addresses
                .get(usize::try_from(label_index).expect("label index was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {otherwise}");
                });

            if new_destination != *otherwise {
                *otherwise = new_destination;

                instruction_changed |= true;
            }
        }
        Instruction::BranchValue {
            condition,
            when_true,
            otherwise,
        } => {
            instruction_changed |=
                backpatch_value(condition, block_addresses, address_to_label, instructive);

            let label_index = address_to_label.get(when_true).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {when_true}");
            });

            let new_destination = block_addresses
                .get(usize::try_from(label_index).expect("label index was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {when_true}");
                });

            if new_destination != *when_true {
                *when_true = new_destination;

                instruction_changed |= true;
            }

            let label_index = address_to_label.get(otherwise).copied().unwrap_or_else(|| {
                println!("{address_to_label:?}");
                panic!("destination was not a label {otherwise}");
            });

            let new_destination = block_addresses
                .get(usize::try_from(label_index).expect("label index was not a valid usize"))
                .copied()
                .unwrap_or_else(|| {
                    panic!("block address not found for block {otherwise}");
                });

            if new_destination != *otherwise {
                *otherwise = new_destination;

                instruction_changed |= true;
            }
        }
    }

    instruction_changed
}

#[allow(clippy::too_many_lines)]
fn backpatch_value<T>(
    value: &mut Value,
    block_addresses: &[u32],
    address_to_label: &HashMap<u32, u32>,
    instructive: &mut T,
) -> bool
where
    T: Instructive,
{
    match value {
        Value::ApartFn { address, .. } => {
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
        Value::ValueIndex { index: value_index } => {
            match instructive.arena_mut().get_mut(usize::from(*value_index)) {
                Some(
                    GarbageCollectedValue::Compound { fields, .. }
                    | GarbageCollectedValue::TaggedCompound { fields, .. },
                ) => {
                    let mut taken_fields = std::mem::take(fields);

                    let mut changed = false;

                    for field in &mut taken_fields {
                        changed |=
                            backpatch_value(field, block_addresses, address_to_label, instructive);
                    }

                    match instructive.arena_mut().get_mut(usize::from(*value_index)) {
                        Some(
                            GarbageCollectedValue::Compound { fields, .. }
                            | GarbageCollectedValue::TaggedCompound { fields, .. },
                        ) => {
                            *fields = taken_fields;
                        }
                        _ => unreachable!(
                            "the outer match guarantees only these variants are matched"
                        ),
                    }

                    changed
                }
                Some(GarbageCollectedValue::ApartVec { values, .. }) => {
                    let mut taken_values = std::mem::take(values);

                    let mut changed = false;

                    for value in &mut taken_values {
                        changed |=
                            backpatch_value(value, block_addresses, address_to_label, instructive);
                    }

                    match instructive.arena_mut().get_mut(usize::from(*value_index)) {
                        Some(GarbageCollectedValue::ApartVec { values, .. }) => {
                            *values = taken_values;
                        }
                        _ => unreachable!(
                            "the outer match guarantees only these variants are matched"
                        ),
                    }

                    changed
                }
                Some(GarbageCollectedValue::ApartString { .. }) | None => false,
            }
        }
        Value::Never
        | Value::U8 { .. }
        | Value::I8 { .. }
        | Value::U16 { .. }
        | Value::I16 { .. }
        | Value::U32 { .. }
        | Value::I32 { .. }
        | Value::U64 { .. }
        | Value::I64 { .. }
        | Value::F32 { .. }
        | Value::F64 { .. }
        | Value::Boolean { .. }
        | Value::Unit
        | Value::ApartNativeFn { .. }
        | Value::ApartRegister { .. } => false,
    }
}

fn find_labels(
    bytecode: &mut Vec<u8>,
    block_addresses: &mut [u32],
    address_to_label: &mut HashMap<u32, u32>,
    remove_labels: bool,
) {
    address_to_label.clear();

    let mut instructive = Compiler {
        ip: 2,
        literals: vec![],
        arena: vec![],
    };

    let mut label_index = 0u32;

    while instructive.ip < bytecode.len() {
        let Some((instruction, instruction_size)) =
            Instruction::decode(&bytecode[(instructive.ip)..], &mut instructive)
        else {
            panic!("failed to decode an instruction before reaching the bytecode's end");
        };

        instructive.ip += instruction_size;

        if matches!(instruction, Instruction::Label) {
            if remove_labels {
                instructive.ip -= instruction_size;
                bytecode.splice((instructive.ip)..(instructive.ip + instruction_size), []);
            } else {
                address_to_label.insert(
                    block_addresses
                        [usize::try_from(label_index).expect("label index was not a valid usize")],
                    label_index,
                );

                block_addresses
                    [usize::try_from(label_index).expect("label index was not a valid usize")] =
                    u32::try_from(instructive.ip).expect("ip was not a valid u32")
                        - label_index
                        - 1;

                label_index += 1;
            }
        }
    }
}

impl<T> Encode<T> for (&CompileSuccess<'_>, &IrInstruction<IrValue>)
where
    T: Instructive,
{
    type Output = Vec<u8>;

    #[allow(clippy::too_many_lines)]
    fn encode(&self, instructive: &mut T) -> Self::Output {
        match self.1 {
            IrInstruction::NoOp => vec![],
            IrInstruction::Panic => Instruction::Panic.encode(instructive),
            IrInstruction::FnLabel(fn_label) => Instruction::ApartFnLabel {
                fn_label: Box::new(FnLabel::new(
                    fn_label.file_name().to_string(),
                    fn_label.function_label().to_string(),
                )),
            }
            .encode(instructive),
            IrInstruction::Unary {
                op,
                operand: ir_operand,
                to,
            } => match (to, op, ir_operand.kind()) {
                (
                    Location::Register(to),
                    UnaryOp::Not,
                    ValueOrLocation::At(Location::Register(operand)),
                ) => Instruction::Not {
                    to: u16::from(*to),
                    operand: u16::from(*operand),
                }
                .encode(instructive),
                (Location::Register(to), UnaryOp::Not, ValueOrLocation::Value(_)) => {
                    Instruction::NotValue {
                        to: u16::from(*to),
                        operand: Value::decode(
                            (self.0, ir_operand).encode(instructive).as_slice(),
                            instructive,
                        )
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    }
                    .encode(instructive)
                }
                (
                    Location::Register(to),
                    UnaryOp::Negate,
                    ValueOrLocation::At(Location::Register(operand)),
                ) => Instruction::Negate {
                    to: u16::from(*to),
                    operand: u16::from(*operand),
                }
                .encode(instructive),
                (Location::Register(to), UnaryOp::Negate, ValueOrLocation::Value(_)) => {
                    Instruction::NegateValue {
                        to: u16::from(*to),
                        operand: Value::decode(
                            (self.0, ir_operand).encode(instructive).as_slice(),
                            instructive,
                        )
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    }
                    .encode(instructive)
                }
            },
            IrInstruction::Binary { op, lhs, rhs, to } => match (to, op, lhs.kind(), rhs.kind()) {
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Multiply {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::MultiplyRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::MultiplyValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Multiply,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::MultiplyValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Divide {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::DivideRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::DivideValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Divide,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::DivideValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Remainder {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::RemainderRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::RemainderValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Remainder,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::RemainderValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Add {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::AddRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::AddValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Add,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::AddValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Subtract {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::SubtractRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::SubtractValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Subtract,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::SubtractValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Less {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::LessRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Less,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::LessValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Greater {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::GreaterRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Greater,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::GreaterValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessOrEqual {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::LessOrEqualRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::LessOrEqualValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::LessOrEqual,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::LessOrEqualValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterOrEqual {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::GreaterOrEqualRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::GreaterOrEqualValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::GreaterOrEqual,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::GreaterOrEqualValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Equal {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::EqualRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::EqualValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Equal,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::EqualValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::NotEqual {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::NotEqualRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::NotEqualValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::NotEqual,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::NotEqualValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::And {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::AndRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::AndValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::And,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::AndValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::Or {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::At(Location::Register(lhs)),
                    ValueOrLocation::Value(_),
                ) => Instruction::OrRegisterValue {
                    to: u16::from(*to),
                    lhs: u16::from(*lhs),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::At(Location::Register(rhs)),
                ) => Instruction::OrValueRegister {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: u16::from(*rhs),
                }
                .encode(instructive),
                (
                    Location::Register(to),
                    BinaryOp::Or,
                    ValueOrLocation::Value(_),
                    ValueOrLocation::Value(_),
                ) => Instruction::OrValueValue {
                    to: u16::from(*to),
                    lhs: Value::decode((self.0, lhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    rhs: Value::decode((self.0, rhs).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
            },
            IrInstruction::Assign { value, to } => match (to, value.kind()) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(operand))) => {
                    Instruction::Assign {
                        to: u16::from(*to),
                        operand: u16::from(*operand),
                    }
                    .encode(instructive)
                }
                (Location::Register(to), ValueOrLocation::Value(_)) => Instruction::AssignValue {
                    to: u16::from(*to),
                    operand: {
                        Value::decode((self.0, value).encode(instructive).as_slice(), instructive)
                            .map_or_else(
                                || unreachable!("all IR values are valid values"),
                                |(value, _)| value,
                            )
                    },
                }
                .encode(instructive),
            },
            IrInstruction::Push(value) => match value.kind() {
                ValueOrLocation::At(Location::Register(operand)) => Instruction::Push {
                    operand: u16::from(*operand),
                }
                .encode(instructive),
                ValueOrLocation::Value(_) => Instruction::PushValue {
                    operand: Value::decode(
                        (self.0, value).encode(instructive).as_slice(),
                        instructive,
                    )
                    .map_or_else(
                        || panic!("all IR values are valid values"),
                        |(value, _)| value,
                    ),
                }
                .encode(instructive),
            },
            IrInstruction::Call { callee, arity, to } => match (to, callee.kind()) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(callee))) => {
                    Instruction::Call {
                        to: u16::from(*to),
                        arity: *arity,
                        callee: u16::from(*callee),
                    }
                    .encode(instructive)
                }
                (Location::Register(to), ValueOrLocation::Value(_)) => Instruction::CallValue {
                    to: u16::from(*to),
                    arity: *arity,
                    callee: Value::decode(
                        (self.0, callee).encode(instructive).as_slice(),
                        instructive,
                    )
                    .map_or_else(
                        || panic!("all IR values are valid values"),
                        |(value, _)| value,
                    ),
                }
                .encode(instructive),
            },
            IrInstruction::Access { index, of, to }
            | IrInstruction::TaggedAccess { index, of, to, .. } => match (to, of.kind()) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(of))) => {
                    Instruction::Access {
                        to: u16::from(*to),
                        index: *index,
                        of: u16::from(*of),
                    }
                    .encode(instructive)
                }
                (Location::Register(to), ValueOrLocation::Value(_)) => Instruction::AccessValue {
                    to: u16::from(*to),
                    index: *index,
                    of: Value::decode((self.0, of).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
            },
            IrInstruction::AccessAssign { index, of, value }
            | IrInstruction::TaggedAccessAssign {
                index, of, value, ..
            } => match (of.kind(), value.kind()) {
                (
                    ValueOrLocation::At(Location::Register(to)),
                    ValueOrLocation::At(Location::Register(value)),
                ) => Instruction::AccessAssign {
                    index: *index,
                    to: u16::from(*to),
                    value: u16::from(*value),
                }
                .encode(instructive),
                (ValueOrLocation::At(Location::Register(to)), ValueOrLocation::Value(_)) => {
                    Instruction::AccessAssignRegisterValue {
                        index: *index,
                        to: u16::from(*to),
                        value: Value::decode(
                            (self.0, value).encode(instructive).as_slice(),
                            instructive,
                        )
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    }
                    .encode(instructive)
                }
                (ValueOrLocation::Value(_), ValueOrLocation::At(Location::Register(value))) => {
                    Instruction::AccessAssignValueRegister {
                        index: *index,
                        to: Value::decode((self.0, of).encode(instructive).as_slice(), instructive)
                            .map_or_else(
                                || panic!("all IR values are valid values"),
                                |(value, _)| value,
                            ),
                        value: u16::from(*value),
                    }
                    .encode(instructive)
                }
                (ValueOrLocation::Value(_), ValueOrLocation::Value(_)) => {
                    Instruction::AccessAssignValueValue {
                        index: *index,
                        to: Value::decode((self.0, of).encode(instructive).as_slice(), instructive)
                            .map_or_else(
                                || panic!("all IR values are valid values"),
                                |(value, _)| value,
                            ),
                        value: Value::decode(
                            (self.0, value).encode(instructive).as_slice(),
                            instructive,
                        )
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                    }
                    .encode(instructive)
                }
            },
            IrInstruction::GetTag { of, to } => match (to, of.kind()) {
                (Location::Register(to), ValueOrLocation::At(Location::Register(operand))) => {
                    Instruction::GetTag {
                        to: u16::from(*to),
                        of: u16::from(*operand),
                    }
                    .encode(instructive)
                }
                (Location::Register(to), ValueOrLocation::Value(_)) => Instruction::GetTagValue {
                    to: u16::from(*to),
                    of: Value::decode((self.0, of).encode(instructive).as_slice(), instructive)
                        .map_or_else(
                            || panic!("all IR values are valid values"),
                            |(value, _)| value,
                        ),
                }
                .encode(instructive),
            },
            IrInstruction::Jump(destination) => Instruction::Jump {
                destination: u32::from(*destination),
            }
            .encode(instructive),
            IrInstruction::Branch {
                condition,
                when_true,
                otherwise,
            } => match condition.kind() {
                ValueOrLocation::At(Location::Register(condition)) => Instruction::Branch {
                    condition: u16::from(*condition),
                    when_true: u32::from(*when_true),
                    otherwise: u32::from(*otherwise),
                }
                .encode(instructive),
                ValueOrLocation::Value(_) => Instruction::BranchValue {
                    condition: Value::decode(
                        (self.0, condition).encode(instructive).as_slice(),
                        instructive,
                    )
                    .map_or_else(
                        || panic!("all IR values are valid values"),
                        |(value, _)| value,
                    ),
                    when_true: u32::from(*when_true),
                    otherwise: u32::from(*otherwise),
                }
                .encode(instructive),
            },
            IrInstruction::Return(value) => match value.kind() {
                ValueOrLocation::At(Location::Register(value)) => Instruction::Return {
                    value: u16::from(*value),
                }
                .encode(instructive),
                ValueOrLocation::Value(_) => Instruction::ReturnValue {
                    value: Value::decode(
                        (self.0, value).encode(instructive).as_slice(),
                        instructive,
                    )
                    .map_or_else(
                        || panic!("all IR values are valid values"),
                        |(value, _)| value,
                    ),
                }
                .encode(instructive),
            },
        }
    }
}

impl<T> Encode<T> for (&CompileSuccess<'_>, &Typed<ValueOrLocation<IrValue>>)
where
    T: Instructive,
{
    type Output = Vec<u8>;

    fn encode(&self, instructive: &mut T) -> Self::Output {
        match self.1.kind() {
            ValueOrLocation::Value(value) => {
                (self.0, (value, self.1.type_index())).encode(instructive)
            }
            ValueOrLocation::At(location) => [
                value_constants::ApartRegister.encode(instructive),
                (self.0, (*location, self.1.type_index())).encode(instructive),
            ]
            .concat(),
        }
    }
}

impl<T> Encode<T> for (&CompileSuccess<'_>, (Location, TypeIndex))
where
    T: Instructive,
{
    type Output = Vec<u8>;

    fn encode(&self, instructive: &mut T) -> Self::Output {
        match self.1.0 {
            Location::Register(index) => u16::from(index).encode(instructive),
        }
    }
}

impl<T> Encode<T> for Box<FnLabel>
where
    T: Instructive,
{
    type Output = Vec<u8>;

    fn encode(&self, instructive: &mut T) -> Self::Output {
        [
            self.file_name().encode(instructive),
            self.function_label().encode(instructive),
        ]
        .concat()
    }
}

impl<T> Decode<T> for Box<FnLabel>
where
    T: Instructive,
{
    type Output = Self;

    fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
        let Some((file_name, file_name_size)) = String::decode(bytecode, instructive) else {
            panic!("function label was not valid");
        };

        let bytecode = &bytecode[file_name_size..];

        let Some((function_label, function_label_size)) = String::decode(bytecode, instructive)
        else {
            panic!("function label was not valid");
        };

        Some((
            Self::new(FnLabel::new(file_name, function_label)),
            file_name_size + function_label_size,
        ))
    }
}

impl<T> Encode<T> for NativeFn
where
    T: Instructive,
{
    type Output = Vec<u8>;

    fn encode(&self, instructive: &mut T) -> Self::Output {
        u32::from(*self).encode(instructive)
    }
}

impl<T> Decode<T> for NativeFn
where
    T: Instructive,
{
    type Output = Self;

    fn decode(bytecode: &[u8], instructive: &mut T) -> Option<(Self::Output, usize)> {
        let Some((native_fn, native_fn_size)) = bytecode
            .get(..std::mem::size_of::<u32>())
            .and_then(|bytes| bytes.as_array::<{ std::mem::size_of::<u32>() }>())
            .and_then(|bytes| u32::decode(bytes, instructive))
        else {
            panic!("native function was not a valid u32");
        };

        Some((
            Self::try_from(native_fn)
                .unwrap_or_else(|_| panic!("native function {native_fn:#010X} did not exist")),
            native_fn_size,
        ))
    }
}

impl<T> Encode<T> for (&CompileSuccess<'_>, (&IrValue, TypeIndex))
where
    T: Instructive,
{
    type Output = Vec<u8>;

    #[allow(clippy::too_many_lines)]
    fn encode(&self, instructive: &mut T) -> Self::Output {
        match self.1.0 {
            IrValue::Never => Value::Never.encode(instructive),
            IrValue::U8(value) => Value::U8 { value: *value }.encode(instructive),
            IrValue::I8(value) => Value::I8 { value: *value }.encode(instructive),
            IrValue::U16(value) => Value::U16 { value: *value }.encode(instructive),
            IrValue::I16(value) => Value::I16 { value: *value }.encode(instructive),
            IrValue::U32(value) => Value::U32 { value: *value }.encode(instructive),
            IrValue::I32(value) => Value::I32 { value: *value }.encode(instructive),
            IrValue::U64(value) => Value::U64 { value: *value }.encode(instructive),
            IrValue::I64(value) => Value::I64 { value: *value }.encode(instructive),
            IrValue::F32(value) => Value::F32 { value: *value }.encode(instructive),
            IrValue::F64(value) => Value::F64 { value: *value }.encode(instructive),
            IrValue::Boolean(value) => Value::Boolean { value: *value }.encode(instructive),
            IrValue::Unit => Value::Unit.encode(instructive),
            IrValue::String(text) => GarbageCollectedValue::ApartString {
                value: text.clone(),
            }
            .encode(instructive),
            IrValue::Fn {
                block_index,
                max_register_count,
            } => Value::ApartFn {
                address: u32::from(*block_index),
                max_register_count: *max_register_count,
            }
            .encode(instructive),
            IrValue::NativeFn(native_fn) => {
                Value::ApartNativeFn { value: *native_fn }.encode(instructive)
            }
            IrValue::Compound(fields) => GarbageCollectedValue::Compound {
                field_count: u8::try_from(fields.len()).unwrap_or_else(|_| {
                    panic!("type checking guarantees the number of fields is valid")
                }),
                fields: fields
                    .iter()
                    .map(|field| {
                        Value::decode((self.0, field).encode(instructive).as_slice(), instructive)
                            .unwrap_or_else(|| panic!("all IR values are valid"))
                            .0
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }
            .encode(instructive),
            IrValue::TaggedCompound { tag, fields } => GarbageCollectedValue::TaggedCompound {
                tag: *tag,
                field_count: u8::try_from(fields.len()).unwrap_or_else(|_| {
                    panic!("type checking guarantees the number of fields is valid")
                }),
                fields: fields
                    .iter()
                    .map(|field| {
                        Value::decode((self.0, field).encode(instructive).as_slice(), instructive)
                            .unwrap_or_else(|| panic!("all IR values are valid"))
                            .0
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }
            .encode(instructive),
        }
    }
}

bytecode_macro::bytecode! (
    pub Instruction as u8 ;

    opcode constants in opcode_constants ;

    prelude = prelude;

    instructive = Instructive;
    encode = Encode;
    decode = Decode;

    opcodes {
        NoOp,
        Label,
        ApartFnLabel {
            fn_label: Box<FnLabel>,
        },
        Not {
            to: u16,
            operand: u16,
        },
        NotValue {
            to: u16,
            operand: Value,
        },
        Negate {
            to: u16,
            operand: u16,
        },
        NegateValue {
            to: u16,
            operand: Value,
        },
        Multiply {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        MultiplyRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        MultiplyValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        MultiplyValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Divide {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        DivideRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        DivideValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        DivideValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Remainder {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        RemainderRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        RemainderValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        RemainderValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Add {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        AddRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        AddValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        AddValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Subtract {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        SubtractRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        SubtractValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        SubtractValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Less {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        LessRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        LessValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        LessValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Greater {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        GreaterRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        GreaterValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        GreaterValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        LessOrEqual {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        LessOrEqualRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        LessOrEqualValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        LessOrEqualValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        GreaterOrEqual {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        GreaterOrEqualRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        GreaterOrEqualValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        GreaterOrEqualValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Equal {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        EqualRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        EqualValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        EqualValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        NotEqual {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        NotEqualRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        NotEqualValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        NotEqualValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        And {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        AndRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        AndValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        AndValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Or {
            to: u16,
            lhs: u16,
            rhs: u16,
        },
        OrRegisterValue {
            to: u16,
            lhs: u16,
            rhs: Value,
        },
        OrValueRegister {
            to: u16,
            lhs: Value,
            rhs: u16,
        },
        OrValueValue {
            to: u16,
            lhs: Value,
            rhs: Value,
        },
        Assign {
            to: u16,
            operand: u16,
        },
        AssignValue {
            to: u16,
            operand: Value,
        },
        Push {
            operand: u16,
        },
        PushValue {
             operand: Value,
         },
        Access {
            to: u16,
            index: u8,
            of: u16,
        },
        AccessValue {
            to: u16,
            index: u8,
            of: Value,
        },
        AccessAssign {
            to: u16,
            index: u8,
            value: u16,
        },
        AccessAssignRegisterValue {
            to: u16,
            index: u8,
            value: Value,
        },
        AccessAssignValueRegister {
            to: Value,
            index: u8,
            value: u16,
        },
        AccessAssignValueValue {
            to: Value,
            index: u8,
            value: Value,
        },
        Call {
            to: u16,
            arity: u8,
            callee: u16,
        },
        CallValue {
            to: u16,
            arity: u8,
            callee: Value,
        },
        GetTag {
            to: u16,
            of: u16,
        },
        GetTagValue {
            to: u16,
            of: Value,
        },
        Jump {
            destination: u32,
        },
        Branch {
            condition: u16,
            when_true: u32,
            otherwise: u32,
        },
        BranchValue {
            condition: Value,
            when_true: u32,
            otherwise: u32,
        },
        Return {
            value: u16,
        },
        ReturnValue {
            value: Value,
        },
        Panic,
    }

    value = Value as u8;

    value constants in value_constants ;

    values {
        Never,
        U8 {
            value: u8,
        },
        I8 {
            value: i8,
        },
        U16 {
            value: u16,
        },
        I16 {
            value: i16,
        },
        U32 {
            value: u32,
        },
        I32 {
            value: i32,
        },
        U64 {
            value: u64,
        },
        I64 {
            value: i64,
        },
        F32 {
            value: f32,
        },
        F64 {
            value: f64,
        },
        Boolean {
            value: bool,
        },
        Unit,
        ApartFn {
            address: u32,
            max_register_count: u16,
        },
        ApartNativeFn {
            value: NativeFn,
        },
        ApartRegister {
            index: u16,
        },
    }

    garbage_collected = GarbageCollectedValue;

    arena_index = ValueIndex as u64;

    garbage_collected {
        Compound {
            field_count: u8,
            fields: Box<[Value]>,
        },
        TaggedCompound {
            tag: u16,
            field_count: u8,
            fields: Box<[Value]>,
        },
        ApartString {
            value: String,
        },
        ApartVec {
            length: u64,
            values: Vec<Value>,
        },
    }
);
