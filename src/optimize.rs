pub mod eliminate_dead_code;
pub mod fold_constants;
pub mod propagate_constants;
pub mod propagate_copies;
pub mod thread_jumps;

use crate::{ApartOptions, ssa::Ssa, type_check::TypeChecker};

pub fn optimize(_: &ApartOptions, types: &TypeChecker, ssa: &mut Ssa) {
    loop {
        let threaded_jumps = thread_jumps::optimize(types, ssa);
        let eliminated_dead_code = eliminate_dead_code::optimize(types, ssa, true);
        let propagated_constants = propagate_constants::optimize(types, ssa);
        let folded_constants = fold_constants::optimize(types, ssa);
        let propagated_copies = propagate_copies::optimize(types, ssa);

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
