use crate::{
    basic_blocks::{Address, BlockIndex, Value},
    ssa::{Block, BlockTerminator, Ssa},
};

pub fn optimize(ssa: &mut Ssa) -> bool {
    let mut changed = false;

    let mut block_swaps = vec![const { None }; ssa.blocks().len()];

    for (b, block) in ssa.blocks().iter().enumerate() {
        if let BlockTerminator::Jump(destination) = block.terminator()
            && block.instructions().is_empty()
        {
            block_swaps[usize::from(*destination)] = Some((
                BlockIndex(b),
                ssa.get_block(BlockIndex(b))
                    .map_or(0, Block::call_argument_count),
            ));

            changed = true;
        }
    }

    ssa.for_live_blocks(|ssa, block_index| {
        if let Some(block) = ssa.get_block_mut(block_index) {
            match block.terminator_mut() {
                BlockTerminator::Return(_) => {}
                BlockTerminator::Jump(destination) => {
                    if let Some((destination_block, _)) = block_swaps
                        .get(usize::from(*destination))
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
                    if let Some((when_true_block, _)) = block_swaps
                        .get(usize::from(*when_true))
                        .and_then(|swap| *swap)
                    {
                        *when_true = when_true_block;
                    }

                    if let Some((otherwise_block, _)) = block_swaps
                        .get(usize::from(*otherwise))
                        .and_then(|swap| *swap)
                    {
                        *otherwise = otherwise_block;
                    }
                }
            }
        }

        ssa.for_value(block_index, |value| match value {
            Value::Fn(block_index) => {
                if let Some((fn_block, _)) = block_swaps
                    .get(usize::from(*block_index))
                    .and_then(|swap| *swap)
                {
                    *block_index = fn_block;
                }
            }
            Value::Address(Address { block_index, .. }) => {
                if let Some((address_block, _)) = block_swaps
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
        if let Some((swap_to, call_argument_count)) = block_swaps
            .get(b)
            .and_then(|swap_to| *swap_to)
            .map(|(block_index, call_argument_count)| {
                (usize::from(block_index), call_argument_count)
            })
        {
            ssa.blocks_mut().swap(b, swap_to);

            if let Some(block) = ssa.get_block_mut(BlockIndex(swap_to)) {
                *block.call_argument_count_mut() = call_argument_count;
            }
        }
    }

    changed
}
