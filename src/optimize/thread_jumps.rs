use crate::{
    basic_blocks::{Address, BlockIndex, Value},
    ssa::{BlockTerminator, Ssa},
};

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut block_swaps = vec![const { None }; ssa.blocks().len()];

    for (b, block) in ssa.blocks().iter().enumerate() {
        if let BlockTerminator::Jump(destination) = block.terminator()
            && block.instructions().is_empty()
            && let Some(destination_block) = ssa.get_block(destination.block())
            && destination_block.parameters() == block.parameters()
        {
            block_swaps[usize::from(destination.block())] = Some(BlockIndex(b));
            changed = true;
        }
    }

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block_mut(block_index) {
            match block.terminator_mut() {
                BlockTerminator::Return(_) => {}
                BlockTerminator::Jump(destination) => {
                    if let Some(destination_block) = block_swaps
                        .get(usize::from(destination.block()))
                        .and_then(|swap| *swap)
                    {
                        *destination.block_mut() = destination_block;
                    }
                }
                BlockTerminator::Branch {
                    when_true,
                    otherwise,
                    ..
                } => {
                    if let Some(when_true_block) = block_swaps
                        .get(usize::from(when_true.block()))
                        .and_then(|swap| *swap)
                    {
                        *when_true.block_mut() = when_true_block;
                    }

                    if let Some(otherwise_block) = block_swaps
                        .get(usize::from(otherwise.block()))
                        .and_then(|swap| *swap)
                    {
                        *otherwise.block_mut() = otherwise_block;
                    }
                }
            }
        }

        ssa.for_value(block_index, |value| match value {
            Value::Fn(block_index) => {
                if let Some(fn_block) = block_swaps
                    .get(usize::from(*block_index))
                    .and_then(|swap| *swap)
                {
                    *block_index = fn_block;
                }
            }
            Value::Address(Address { block_index, .. }) => {
                if let Some(address_block) = block_swaps
                    .get(usize::from(*block_index))
                    .and_then(|swap| *swap)
                {
                    *block_index = address_block;
                }
            }
            _ => {}
        });
    });

    for b in (0..(ssa.blocks().len())).rev() {
        if let Some(swap_to) = block_swaps
            .get(b)
            .and_then(|swap_to| *swap_to)
            .map(usize::from)
        {
            ssa.blocks_mut().swap(b, swap_to);
        }
    }

    changed
}
