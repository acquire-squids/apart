use crate::{
    basic_blocks::{BlockIndex, Instruction, Value},
    ssa::{Argument, BlockTerminator, Ssa},
};

use std::collections::HashSet;

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let blocks_used = collect_used_blocks(ssa);

    for (b, block) in ssa.blocks_mut().iter_mut().enumerate() {
        if !block.instructions().is_empty() && blocks_used.get(b).copied().is_none_or(|used| !used)
        {
            block.instructions_mut().clear();

            changed = true;
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

    changed
}

fn value_uses_block(blocks_used: &mut [bool], value: &Value) {
    match value {
        Value::Fn(block_index) => {
            blocks_used[usize::from(*block_index)] = true;
        }
        Value::Compound(values) => {
            for value in values {
                value_uses_block(blocks_used, value);
            }
        }
        _ => {}
    }
}

fn collect_used_blocks(ssa: &mut Ssa) -> Vec<bool> {
    let mut blocks_used = vec![false; ssa.blocks().len() + 1];

    blocks_used[0] = true;

    for block in ssa.blocks() {
        for instruction in block.instructions() {
            match instruction {
                Instruction::NoOp | Instruction::Pop => {}
                Instruction::Unary { operand: value, .. }
                | Instruction::Assign { value, .. }
                | Instruction::Push(value)
                | Instruction::Call { callee: value, .. }
                | Instruction::Access { of: value, .. } => {
                    value_uses_block(blocks_used.as_mut_slice(), value);
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    value_uses_block(blocks_used.as_mut_slice(), lhs);
                    value_uses_block(blocks_used.as_mut_slice(), rhs);
                }
                Instruction::AccessAssign { of, value, .. } => {
                    value_uses_block(blocks_used.as_mut_slice(), of);
                    value_uses_block(blocks_used.as_mut_slice(), value);
                }
            }
        }
    }

    for block in ssa.blocks_mut() {
        match block.terminator_mut() {
            BlockTerminator::Return(value) => {
                value_uses_block(blocks_used.as_mut_slice(), value);
            }
            BlockTerminator::Jump(jump_to) => {
                blocks_used[usize::from(jump_to.block())] = true;
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => match condition {
                Value::Boolean(true) => {
                    blocks_used[usize::from(when_true.block())] = true;

                    *block.terminator_mut() = BlockTerminator::Jump(when_true.clone());
                }
                Value::Boolean(false) => {
                    blocks_used[usize::from(otherwise.block())] = true;

                    *block.terminator_mut() = BlockTerminator::Jump(otherwise.clone());
                }
                _ => {
                    value_uses_block(blocks_used.as_mut_slice(), condition);

                    blocks_used[usize::from(when_true.block())] = true;
                    blocks_used[usize::from(otherwise.block())] = true;
                }
            },
        }
    }

    blocks_used
}

fn value_uses_address(addresses_used: &mut HashSet<(BlockIndex, usize)>, value: &Value) {
    match value {
        Value::Address(address) => {
            addresses_used.insert((address.block_index, address.offset));
        }
        Value::Compound(values) => {
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
                Instruction::NoOp | Instruction::Pop => {
                    addresses_used.insert((BlockIndex(b), i));
                }
                Instruction::Unary { operand: value, .. } | Instruction::Assign { value, .. } => {
                    value_uses_address(&mut addresses_used, value);
                }
                Instruction::Push(value)
                | Instruction::Call { callee: value, .. }
                | Instruction::Access { of: value, .. } => {
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
                    if let Argument::Address(value) = argument {
                        value_uses_address(&mut addresses_used, value);
                    }
                }
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                value_uses_address(&mut addresses_used, condition);

                for argument in when_true.arguments() {
                    if let Argument::Address(value) = argument {
                        value_uses_address(&mut addresses_used, value);
                    }
                }

                for argument in otherwise.arguments() {
                    if let Argument::Address(value) = argument {
                        value_uses_address(&mut addresses_used, value);
                    }
                }
            }
        }
    }

    addresses_used
}
