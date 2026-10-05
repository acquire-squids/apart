use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashSet;

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let blocks_used = collect_used_blocks(ssa);

    let mut block_remap = vec![const { None }; ssa.blocks().len()];

    let mut block_count = 0;

    for (b, remap) in block_remap.iter_mut().enumerate() {
        if blocks_used.contains(&BlockIndex(b)) {
            *remap = Some(BlockIndex(block_count));

            block_count += 1;
        }
    }

    let addresses_used = collect_used_addresses(ssa);

    for (b, block) in ssa.blocks_mut().iter_mut().enumerate() {
        for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
            if !addresses_used.contains(&(BlockIndex(b), i)) {
                *instruction = Instruction::NoOp;

                changed = true;
            }
        }
    }

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block_mut(block_index) {
            match block.terminator_mut() {
                BlockTerminator::Return(_) => {}
                BlockTerminator::Jump(destination) => {
                    if let Some(destination_block) = block_remap
                        .get(usize::from(destination.block()))
                        .and_then(|remap| *remap)
                    {
                        *destination.block_mut() = destination_block;
                    }
                }
                BlockTerminator::Branch {
                    when_true,
                    otherwise,
                    ..
                } => {
                    if let Some(when_true_block) = block_remap
                        .get(usize::from(when_true.block()))
                        .and_then(|remap| *remap)
                    {
                        *when_true.block_mut() = when_true_block;
                    }

                    if let Some(otherwise_block) = block_remap
                        .get(usize::from(otherwise.block()))
                        .and_then(|remap| *remap)
                    {
                        *otherwise.block_mut() = otherwise_block;
                    }
                }
            }
        }

        ssa.for_value(block_index, |value| match value {
            Value::Fn(block_index) => {
                if let Some(fn_block) = block_remap
                    .get(usize::from(*block_index))
                    .and_then(|remap| *remap)
                {
                    *block_index = fn_block;
                }
            }
            Value::Address(Address { block_index, .. }) => {
                if let Some(address_block) = block_remap
                    .get(usize::from(*block_index))
                    .and_then(|remap| *remap)
                {
                    *block_index = address_block;
                }
            }
            _ => {}
        });
    });

    for b in (0..(ssa.blocks().len())).rev() {
        if block_remap.get(b).is_none_or(Option::is_none) {
            ssa.blocks_mut().remove(b);
        }
    }

    changed
}

fn collect_used_blocks(ssa: &mut Ssa) -> HashSet<BlockIndex> {
    let mut blocks_used = HashSet::new();

    ssa.for_live_blocks(|_, block_index| {
        blocks_used.insert(block_index);
    });

    blocks_used
}

fn value_uses_address(addresses_used: &mut HashSet<(BlockIndex, usize)>, value: &Value) {
    match value {
        Value::Address(address) => {
            addresses_used.insert((address.block_index, address.offset));
        }
        Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
            for value in values {
                value_uses_address(addresses_used, value);
            }
        }
        _ => {}
    }
}

fn collect_used_addresses(ssa: &Ssa) -> HashSet<(BlockIndex, usize)> {
    let mut addresses_used = HashSet::new();

    for (b, block) in ssa.blocks().iter().enumerate() {
        for (i, instruction) in block.instructions().iter().enumerate() {
            match instruction {
                Instruction::NoOp => {
                    addresses_used.insert((BlockIndex(b), i));
                }
                Instruction::Unary { operand: value, .. } | Instruction::Assign { value, .. } => {
                    value_uses_address(&mut addresses_used, value);
                }
                Instruction::Push(value)
                | Instruction::Call { callee: value, .. }
                | Instruction::Access { of: value, .. }
                | Instruction::GetTag { of: value, .. } => {
                    value_uses_address(&mut addresses_used, value);

                    addresses_used.insert((BlockIndex(b), i));
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    value_uses_address(&mut addresses_used, lhs);
                    value_uses_address(&mut addresses_used, rhs);
                }
                Instruction::AccessAssign { of, value, .. } => {
                    value_uses_address(&mut addresses_used, of);
                    value_uses_address(&mut addresses_used, value);

                    addresses_used.insert((BlockIndex(b), i));
                }
            }
        }

        match block.terminator() {
            BlockTerminator::Return(value) => {
                value_uses_address(&mut addresses_used, value);
            }
            BlockTerminator::Jump(jump_to) => {
                for argument in jump_to.arguments() {
                    value_uses_address(&mut addresses_used, argument);
                }
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                value_uses_address(&mut addresses_used, condition);

                for argument in when_true.arguments() {
                    value_uses_address(&mut addresses_used, argument);
                }

                for argument in otherwise.arguments() {
                    value_uses_address(&mut addresses_used, argument);
                }
            }
        }
    }

    addresses_used
}
