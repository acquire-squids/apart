mod eliminate_dead_code;
mod fold_constants;
mod propagate_constants;
mod propagate_copies;
mod thread_jumps;

use crate::{ApartOptions, ssa::Ssa};

pub fn optimize(_: &ApartOptions, ssa: &mut Ssa) {
    loop {
        let threaded_jumps = thread_jumps::optimize(ssa);
        let eliminated_dead_code = eliminate_dead_code::optimize(ssa);
        let propagated_constants = propagate_constants::optimize(ssa);
        let folded_constants = fold_constants::optimize(ssa);
        let propagated_copies = propagate_copies::optimize(ssa);

        if threaded_jumps
            || eliminated_dead_code
            || propagated_constants
            || folded_constants
            || propagated_copies
        {
            continue;
        }

        break;
    }
}
