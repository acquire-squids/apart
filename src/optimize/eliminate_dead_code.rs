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
                | Instruction::Call(value) => {
                    if let Value::FnBlock(block_index) = value {
                        blocks_used[usize::from(*block_index)] = true;
                    }
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    if let Value::FnBlock(block_index) = lhs {
                        blocks_used[usize::from(*block_index)] = true;
                    }

                    if let Value::FnBlock(block_index) = rhs {
                        blocks_used[usize::from(*block_index)] = true;
                    }
                }
            }
        }
    }

    for block in ssa.blocks_mut() {
        match block.terminator_mut() {
            BlockTerminator::Return(_) => {}
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
                    blocks_used[usize::from(when_true.block())] = true;
                    blocks_used[usize::from(otherwise.block())] = true;
                }
            },
        }
    }

    blocks_used
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
                    if let Value::Address(address) = value {
                        addresses_used.insert((address.block_index, address.offset));
                    }
                }
                Instruction::Push(value) | Instruction::Call(value) => {
                    if let Value::Address(address) = value {
                        addresses_used.insert((address.block_index, address.offset));
                    }

                    addresses_used.insert((BlockIndex(b), i));
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    if let Value::Address(address) = lhs {
                        addresses_used.insert((address.block_index, address.offset));
                    }

                    if let Value::Address(address) = rhs {
                        addresses_used.insert((address.block_index, address.offset));
                    }
                }
            }
        }

        match block.terminator() {
            BlockTerminator::Return(value) => {
                if let Value::Address(address) = value {
                    addresses_used.insert((address.block_index, address.offset));
                }
            }
            BlockTerminator::Jump(jump_to) => {
                for argument in jump_to.arguments() {
                    if let Argument::Address(address) = argument {
                        addresses_used.insert((address.block_index, address.offset));
                    }
                }
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => {
                if let Value::Address(address) = condition {
                    addresses_used.insert((address.block_index, address.offset));
                }

                for argument in when_true.arguments() {
                    if let Argument::Address(address) = argument {
                        addresses_used.insert((address.block_index, address.offset));
                    }
                }

                for argument in otherwise.arguments() {
                    if let Argument::Address(address) = argument {
                        addresses_used.insert((address.block_index, address.offset));
                    }
                }
            }
        }
    }

    addresses_used
}
