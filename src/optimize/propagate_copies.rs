use crate::{
    basic_blocks::{Instruction, Value},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashMap;

// TODO: this is not full copy propagation (it only gets rid of definitions that are copies)
pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut copies = HashMap::new();

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block(block_index) {
            for (i, instruction) in block.instructions().iter().enumerate() {
                if let Instruction::Assign {
                    value: Value::Address(value),
                    to: Value::Address(to),
                } = instruction
                    && to.block_index == block_index
                    && to.offset == i
                {
                    copies.insert(*to, *value);
                }
            }
        }
    });

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block_mut(block_index) {
            for instruction in block.instructions_mut() {
                match instruction {
                    Instruction::FnLabel { .. }
                    | Instruction::Panic
                    | Instruction::NoOp
                    | Instruction::ScopeStart
                    | Instruction::PopN(_) => {}
                    Instruction::Unary { operand: value, .. }
                    | Instruction::Assign { value, .. }
                    | Instruction::Push(value)
                    | Instruction::Call { callee: value, .. }
                    | Instruction::Access { of: value, .. }
                    | Instruction::GetTag { of: value, .. } => {
                        if let Value::Address(address) = value
                            && let Some(copy) = copies.get(address).copied()
                        {
                            *value = Value::Address(copy);

                            changed = true;
                        }
                    }
                    Instruction::Binary { lhs, rhs, .. }
                    | Instruction::AccessAssign {
                        of: lhs,
                        value: rhs,
                        ..
                    } => {
                        if let Value::Address(address) = lhs
                            && let Some(copy) = copies.get(address).copied()
                        {
                            *lhs = Value::Address(copy);

                            changed = true;
                        }

                        if let Value::Address(address) = rhs
                            && let Some(copy) = copies.get(address).copied()
                        {
                            *rhs = Value::Address(copy);

                            changed = true;
                        }
                    }
                }
            }

            match block.terminator_mut() {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Return(value)
                | BlockTerminator::Branch {
                    condition: value, ..
                } => {
                    if let Value::Address(address) = value
                        && let Some(copy) = copies.get(address).copied()
                    {
                        *value = Value::Address(copy);

                        changed = true;
                    }
                }
            }
        }
    });

    changed
}
