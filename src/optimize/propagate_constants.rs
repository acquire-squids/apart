use crate::{
    basic_blocks::{Address, Instruction, Value},
    ssa::{Block, BlockTerminator, Ssa},
};

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    for b in 0..(ssa.blocks().len()) {
        if let Some(block) = ssa.blocks().get(b) {
            for i in 0..(block.instructions().len()) {
                if let Some(instruction) = ssa
                    .blocks()
                    .get(b)
                    .and_then(|block| block.instructions().get(i))
                {
                    match instruction {
                        Instruction::NoOp | Instruction::Pop | Instruction::Binary { .. } => {}
                        Instruction::Unary { operand: value, .. }
                        | Instruction::Assign { value, .. }
                        | Instruction::Push(value)
                        | Instruction::Call { callee: value, .. } => {
                            if let Value::Address(_) = value
                                && let Some(propagated_value) = clone_constant(ssa, value)
                                && let Some(
                                    Instruction::Unary { operand: value, .. }
                                    | Instruction::Assign { value, .. }
                                    | Instruction::Push(value)
                                    | Instruction::Call { callee: value, .. },
                                ) = ssa
                                    .blocks_mut()
                                    .get_mut(b)
                                    .and_then(|block| block.instructions_mut().get_mut(i))
                            {
                                *value = propagated_value;

                                changed = true;
                            }
                        }
                    }
                }

                if let Some(Instruction::Binary { lhs, .. }) = ssa
                    .blocks()
                    .get(b)
                    .and_then(|block| block.instructions().get(i))
                    && let Value::Address(_) = lhs
                    && let Some(propagated_value) = clone_constant(ssa, lhs)
                    && let Some(Instruction::Binary { lhs, .. }) = ssa
                        .blocks_mut()
                        .get_mut(b)
                        .and_then(|block| block.instructions_mut().get_mut(i))
                {
                    *lhs = propagated_value;

                    changed = true;
                }

                if let Some(Instruction::Binary { rhs, .. }) = ssa
                    .blocks()
                    .get(b)
                    .and_then(|block| block.instructions().get(i))
                    && let Value::Address(_) = rhs
                    && let Some(propagated_value) = clone_constant(ssa, rhs)
                    && let Some(Instruction::Binary { rhs, .. }) = ssa
                        .blocks_mut()
                        .get_mut(b)
                        .and_then(|block| block.instructions_mut().get_mut(i))
                {
                    *rhs = propagated_value;

                    changed = true;
                }
            }
        }

        if let Some(block) = ssa.blocks().get(b) {
            match block.terminator() {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Return(value) => {
                    if let Value::Address(_) = value
                        && let Some(propagated_value) = clone_constant(ssa, value)
                        && let Some(BlockTerminator::Return(value)) =
                            ssa.blocks_mut().get_mut(b).map(Block::terminator_mut)
                    {
                        *value = propagated_value;

                        changed = true;
                    }
                }
                BlockTerminator::Branch { condition, .. } => {
                    if let Value::Address(_) = condition
                        && let Some(propagated_value) = clone_constant(ssa, condition)
                        && let Some(BlockTerminator::Branch { condition, .. }) =
                            ssa.blocks_mut().get_mut(b).map(Block::terminator_mut)
                    {
                        *condition = propagated_value;

                        changed = true;
                    }
                }
            }
        }
    }

    changed
}

fn clone_constant(ssa: &Ssa, value: &Value) -> Option<Value> {
    match value {
        Value::Integer(_)
        | Value::Float(_)
        | Value::Boolean(_)
        | Value::Unit
        | Value::Fn(_)
        | Value::NativeFn(_)
        | Value::Argument(_)
        | Value::Register(_) => Some(value.clone()),
        Value::Address(address) => clone_constant_from_address(ssa, address),
        Value::Runtime => None,
    }
}

fn clone_constant_from_address(ssa: &Ssa, address: &Address) -> Option<Value> {
    if let Some(Instruction::Assign { value, .. }) = ssa
        .get_block(address.block_index)
        .and_then(|block| block.instructions().get(address.offset))
    {
        clone_constant(ssa, value)
    } else {
        None
    }
}
