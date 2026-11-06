use crate::{
    basic_blocks::{Instruction, Value},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashMap;

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut copies = HashMap::new();

    let def_use = ssa.def_use_analysis();
    let def_use = def_use.as_slice();

    ssa.for_functions(|ssa, blocks| {
        let first_b = blocks
            .first()
            .map_or(0, |block_index| usize::from(*block_index));

        for block_index in &blocks {
            if let Some(block) = ssa.get_block(*block_index) {
                for (i, instruction) in block.instructions().iter().enumerate() {
                    if let Instruction::Assign {
                        value: Value::Address(value),
                        to: Value::Address(to),
                    } = instruction
                        && to.block_index == *block_index
                        && to.offset == i
                    {
                        copies.insert(*to, *value);
                    }
                }
            }
        }

        for block_index in &blocks {
            if let Some(block) = ssa.get_block_mut(*block_index) {
                let b = usize::from(*block_index);

                let propagate_copy = |value: &mut Value| {
                    if let Value::Address(x) = value
                        && let Some(y) = copies.get(x)
                        // if there is only one definition of x reaching this copy
                        && def_use
                            .iter()
                            .skip(first_b)
                            .take(b - first_b)
                            .filter(|def_use| def_use.definitions().contains(x))
                            .count()
                            <= 1
                        // if there are no assignments to y reaching this copy
                        && !def_use
                            .iter()
                            .skip(first_b)
                            .take(b - first_b)
                            .any(|def_use| def_use.definitions().contains(y))
                    {
                        *value = Value::Address(*y);

                        true
                    } else {
                        false
                    }
                };

                for instruction in block.instructions_mut() {
                    match instruction {
                        Instruction::FnLabel { .. } | Instruction::Panic | Instruction::NoOp => {}
                        Instruction::Unary { operand: value, .. }
                        | Instruction::Assign { value, .. }
                        | Instruction::Push(value)
                        | Instruction::Call { callee: value, .. }
                        | Instruction::Access { of: value, .. }
                        | Instruction::GetTag { of: value, .. } => {
                            changed |= propagate_copy(value);
                        }
                        Instruction::Binary { lhs, rhs, .. }
                        | Instruction::AccessAssign {
                            of: lhs,
                            value: rhs,
                            ..
                        } => {
                            changed |= propagate_copy(lhs);
                            changed |= propagate_copy(rhs);
                        }
                    }
                }

                match block.terminator_mut() {
                    BlockTerminator::Jump(_) => {}
                    BlockTerminator::Return(value)
                    | BlockTerminator::Branch {
                        condition: value, ..
                    } => {
                        changed |= propagate_copy(value);
                    }
                }
            }
        }
    });

    changed
}
