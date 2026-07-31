mod eliminate_dead_code;
mod fold_constants;
mod propagate_constants;

use crate::ssa::Ssa;

pub fn optimize(ssa: &mut Ssa) {
    loop {
        let eliminated = eliminate_dead_code::optimize(ssa);
        let propagated = propagate_constants::optimize(ssa);
        let folded = fold_constants::optimize(ssa);

        if eliminated || propagated || folded {
            continue;
        }

        break;
    }
}
