use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{Block, BlockTerminator, Ssa},
};

use std::collections::HashMap;

#[allow(clippy::too_many_lines)]
pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut address_to_ip = HashMap::new();

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block(block_index) {
            for (i, instruction) in block.instructions().iter().enumerate() {
                if let Instruction::Assign { to, .. } = instruction
                    && let Value::Address(address) = to
                {
                    address_to_ip
                        .entry(*address)
                        .and_modify(|ip| {
                            *ip = None;
                        })
                        .or_insert_with(|| Some((block_index, i)));
                }
            }
        }
    });

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block(block_index) {
            for i in 0..(block.instructions().len()) {
                if let Some(instruction) = ssa
                    .get_block(block_index)
                    .and_then(|block| block.instructions().get(i))
                {
                    match instruction {
                        Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(_) => {}
                        Instruction::Unary { operand: value, .. }
                        | Instruction::Assign { value, .. }
                        | Instruction::Push(value)
                        | Instruction::Call { callee: value, .. }
                        | Instruction::Access { of: value, .. }
                        | Instruction::Binary { lhs: value, .. }
                        | Instruction::AccessAssign { of: value, .. }
                        | Instruction::GetTag { of: value, .. } => {
                            if let Some(propagated_value) =
                                clone_constant(ssa, value, &address_to_ip)
                                && let Some(
                                    Instruction::Unary { operand: value, .. }
                                    | Instruction::Assign { value, .. }
                                    | Instruction::Push(value)
                                    | Instruction::Call { callee: value, .. }
                                    | Instruction::Access { of: value, .. }
                                    | Instruction::Binary { lhs: value, .. }
                                    | Instruction::AccessAssign { of: value, .. }
                                    | Instruction::GetTag { of: value, .. },
                                ) = ssa
                                    .get_block_mut(block_index)
                                    .and_then(|block| block.instructions_mut().get_mut(i))
                                && propagated_value != *value
                            {
                                *value = propagated_value;

                                changed = true;
                            }
                        }
                    }
                }

                if let Some(Instruction::Binary { rhs: value, .. }) = ssa
                    .get_block(block_index)
                    .and_then(|block| block.instructions().get(i))
                    && let Some(propagated_value) = clone_constant(ssa, value, &address_to_ip)
                    && let Some(Instruction::Binary { rhs: value, .. }) = ssa
                        .get_block_mut(block_index)
                        .and_then(|block| block.instructions_mut().get_mut(i))
                    && propagated_value != *value
                {
                    *value = propagated_value;

                    changed = true;
                }

                if let Some(Instruction::AccessAssign { value, .. }) = ssa
                    .get_block(block_index)
                    .and_then(|block| block.instructions().get(i))
                    && let Some(propagated_value) = clone_constant(ssa, value, &address_to_ip)
                    && let Some(Instruction::AccessAssign { value, .. }) = ssa
                        .get_block_mut(block_index)
                        .and_then(|block| block.instructions_mut().get_mut(i))
                    && propagated_value != *value
                {
                    *value = propagated_value;

                    changed = true;
                }
            }

            if let Some(block) = ssa.get_block(block_index) {
                match block.terminator() {
                    BlockTerminator::Jump(_) => {}
                    BlockTerminator::Return(value) => {
                        if let Some(propagated_value) = clone_constant(ssa, value, &address_to_ip)
                            && let Some(BlockTerminator::Return(value)) =
                                ssa.get_block_mut(block_index).map(Block::terminator_mut)
                            && propagated_value != *value
                        {
                            *value = propagated_value;

                            changed = true;
                        }
                    }
                    BlockTerminator::Branch {
                        condition: value, ..
                    } => {
                        if let Some(propagated_value) = clone_constant(ssa, value, &address_to_ip)
                            && let Some(BlockTerminator::Branch {
                                condition: value, ..
                            }) = ssa.get_block_mut(block_index).map(Block::terminator_mut)
                            && propagated_value != *value
                        {
                            *value = propagated_value;

                            changed = true;
                        }
                    }
                }
            }
        }
    });

    changed
}

fn clone_constant(
    ssa: &Ssa,
    value: &Value,
    address_to_ip: &HashMap<Address, Option<(BlockIndex, usize)>>,
) -> Option<Value> {
    match value {
        Value::U8(_)
        | Value::I8(_)
        | Value::U16(_)
        | Value::I16(_)
        | Value::U32(_)
        | Value::I32(_)
        | Value::U64(_)
        | Value::I64(_)
        | Value::F64(_)
        | Value::Boolean(_)
        | Value::Unit
        | Value::Fn(_)
        | Value::NativeFn(_)
        | Value::CallArgument(_) => Some(value.clone()),
        Value::Address(address) => clone_constant_from_address(ssa, address, address_to_ip),
        Value::Runtime
        | Value::Register(_)
        | Value::StackOffset(_)
        | Value::Compound(_)
        | Value::TaggedCompound { .. }
        | Value::String(_) => None,
    }
}

fn clone_constant_from_address(
    ssa: &Ssa,
    address: &Address,
    address_to_ip: &HashMap<Address, Option<(BlockIndex, usize)>>,
) -> Option<Value> {
    if let Some((block_index, i)) = address_to_ip.get(address).and_then(|ip| *ip)
        && let Some(Instruction::Assign { value, .. }) = ssa
            .get_block(block_index)
            .and_then(|block| block.instructions().get(i))
    {
        clone_constant(ssa, value, address_to_ip)
    } else {
        None
    }
}
