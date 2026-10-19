use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{Block, BlockTerminator, Ssa},
};

#[allow(clippy::too_many_lines)]
pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut block_swaps = vec![const { None }; ssa.blocks().len()];

    for (b, block) in ssa.blocks_mut().iter_mut().enumerate() {
        match block.terminator_mut() {
            BlockTerminator::Return(_) => {}
            BlockTerminator::Jump(destination) => {
                let destination = *destination;

                if block.instructions().is_empty()
                    || block.instructions().iter().all(|instruction| {
                        matches!(
                            instruction,
                            Instruction::NoOp
                                | Instruction::ScopeStart
                                | Instruction::PopN(0)
                                | Instruction::FnLabel { .. }
                        )
                    })
                {
                    block_swaps[usize::try_from(u32::from(destination))
                        .expect("this is verified fine by previous stages")] = Some(BlockIndex(
                        u32::try_from(b).expect("this is verified fine by previous stages"),
                    ));

                    changed = true;
                }
            }
            BlockTerminator::Branch {
                condition,
                when_true,
                otherwise,
            } => match condition {
                Value::Boolean(true) => {
                    let when_true = *when_true;

                    *block.terminator_mut() = BlockTerminator::Jump(when_true);

                    changed |= true;
                }
                Value::Boolean(false) => {
                    let otherwise = *otherwise;

                    *block.terminator_mut() = BlockTerminator::Jump(otherwise);

                    changed |= true;
                }
                _ => {}
            },
        }
    }

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block_mut(block_index) {
            match block.terminator_mut() {
                BlockTerminator::Return(_) => {}
                BlockTerminator::Jump(destination) => {
                    if let Some(destination_block) = usize::try_from(u32::from(*destination))
                        .ok()
                        .and_then(|destination| block_swaps.get(destination))
                        .and_then(|swap| *swap)
                    {
                        *destination = destination_block;
                    }
                }
                BlockTerminator::Branch {
                    when_true,
                    otherwise,
                    ..
                } => {
                    if let Some(when_true_block) = usize::try_from(u32::from(*when_true))
                        .ok()
                        .and_then(|destination| block_swaps.get(destination))
                        .and_then(|swap| *swap)
                    {
                        *when_true = when_true_block;
                    }

                    if let Some(otherwise_block) = usize::try_from(u32::from(*otherwise))
                        .ok()
                        .and_then(|destination| block_swaps.get(destination))
                        .and_then(|swap| *swap)
                    {
                        *otherwise = otherwise_block;
                    }
                }
            }
        }

        ssa.for_value(block_index, |value| match value {
            Value::Fn(block_index) => {
                if let Some(fn_block) = usize::try_from(u32::from(*block_index))
                    .ok()
                    .and_then(|destination| block_swaps.get(destination))
                    .and_then(|swap| *swap)
                {
                    *block_index = fn_block;
                }
            }
            Value::Address(Address { block_index, .. }) => {
                if let Some(address_block) = usize::try_from(u32::from(*block_index))
                    .ok()
                    .and_then(|destination| block_swaps.get(destination))
                    .and_then(|swap| *swap)
                {
                    *block_index = address_block;
                }
            }
            _ => {}
        });
    });

    for destination in (0..(ssa.blocks().len()))
        .filter_map(|index| u32::try_from(index).ok())
        .map(BlockIndex)
    {
        if let Some(b) = usize::try_from(u32::from(destination))
            .ok()
            .and_then(|destination| block_swaps.get(destination))
            .and_then(|swap_to| *swap_to)
            && let Some(call_argument_count) = ssa.get_block(b).map(Block::call_argument_count)
        {
            let fn_label = ssa
                .get_block(destination)
                .and_then(|block| block.instructions().first())
                .filter(|instruction| matches!(instruction, Instruction::FnLabel { .. }))
                .cloned();

            ssa.blocks_mut().swap(
                usize::try_from(u32::from(destination))
                    .expect("this is verified fine by previous stages"),
                usize::try_from(u32::from(b)).expect("this is verified fine by previous stages"),
            );

            if let Some(block) = ssa.get_block_mut(b) {
                *block.call_argument_count_mut() = call_argument_count;

                if let Some(fn_label) = fn_label {
                    block.instructions_mut().insert(0, fn_label);

                    ssa.for_value(destination, |value| {
                        if let Value::Address(address) = value
                            && address.block_index == b
                        {
                            address.offset += 1;
                        }
                    });
                }
            }
        }
    }

    changed
}
