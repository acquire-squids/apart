use crate::{
    basic_blocks::{Instruction, TypedValue, Value},
    ssa::{BlockTerminator, Ssa},
    type_check::TypeChecker,
};

use std::collections::HashMap;

pub fn optimize(_: &TypeChecker, ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut copies = HashMap::new();

    let def_use = ssa.def_use_analysis();
    let def_use = def_use.as_slice();

    ssa.for_functions(|ssa, blocks| {
        for block_index in &blocks {
            if let Some(block) = ssa.get_block(*block_index) {
                let b = usize::from(*block_index);

                for (i, instruction) in block.instructions().iter().enumerate() {
                    if let Instruction::Assign {
                        value: TypedValue { value: Value::Address(value), .. },
                        to: Value::Address(to),
                    } = instruction
                    {
                        copies.insert(*to, ((b, i), *value));
                    }
                }
            }
        }

        for block_index in &blocks {
            if let Some(block) = ssa.get_block_mut(*block_index) {
                let b = usize::from(*block_index);
                let i = block.instructions().len();

                let propagate_copy = |i: usize, value: &mut Value| {
                    if let Value::Address(x) = value
                        && let Some(((x_b, x_i), y)) = copies.get(x)
                        // if there are no other definitions of x reaching this use of x
                        && !def_use
                            .iter()
                            .enumerate()
                            .take(b + 1)
                            .any(|(def_use_b, def_use)| def_use.definitions().iter().enumerate().any(|(def_use_i, def_use)| (def_use_b != b || def_use_i < i) && (def_use_b != *x_b || def_use_i != *x_i) && def_use.contains(x)))
                        // if there are no assignments to y between x's definitions and x's use
                        && !def_use
                            .iter()
                            .enumerate()
                            .take(b + 1)
                            .any(|(def_use_b, def_use)| def_use.definitions().iter().enumerate().any(|(def_use_i, def_use)| (def_use_b != b || def_use_i < i) && (def_use_b != *x_b || def_use_i > *x_i) && def_use.contains(y)))
                    {
                        *value = Value::Address(*y);

                        true
                    } else {
                        false
                    }
                };

                for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                    match instruction {
                        Instruction::FnLabel { .. } | Instruction::Panic | Instruction::NoOp => {}
                        Instruction::Unary { operand: value, .. }
                        | Instruction::Assign { value, .. }
                        | Instruction::Push(value)
                        | Instruction::Call { callee: value, .. }
                        | Instruction::Access { of: value, .. }
                        | Instruction::GetTag { of: value, .. } => {
                            changed |= propagate_copy(i, value.value_mut());
                        }
                        Instruction::Binary { lhs, rhs, .. }
                        | Instruction::AccessAssign {
                            of: lhs,
                            value: rhs,
                            ..
                        } => {
                            changed |= propagate_copy(i, lhs.value_mut());
                            changed |= propagate_copy(i, rhs.value_mut());
                        }
                    }
                }

                match block.terminator_mut() {
                    BlockTerminator::Jump(_) => {}
                    BlockTerminator::Return(value)
                    | BlockTerminator::Branch {
                        condition: value, ..
                    } => {
                        changed |= propagate_copy(i, value.value_mut());
                    }
                }
            }
        }
    });

    changed
}
