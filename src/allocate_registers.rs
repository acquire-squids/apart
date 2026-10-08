use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashMap;

pub fn allocate(ssa: &mut Ssa) {
    ssa.for_functions(|ssa, blocks| {
        let mut addresses = HashMap::new();

        let mut allocator = RegisterAllocator {
            register_count: 0,
            scope_register_counts: vec![],
            max_register_count: 0,
        };

        if let Some(block) = blocks
            .first()
            .and_then(|block_index| ssa.get_block(*block_index))
        {
            for _ in 0..(block.call_argument_count()) {
                allocator.allocate();
            }
        }

        for block_index in blocks.iter().copied() {
            allocator.allocate_block(ssa, block_index, &mut addresses);

            if let Some(block) = ssa.get_block_mut(block_index) {
                block.instructions_mut().retain(|instruction| {
                    !matches!(
                        instruction,
                        Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(0)
                    )
                });
            }
        }

        if let Some(block) = blocks
            .first()
            .and_then(|block_index| ssa.get_block_mut(*block_index))
        {
            *block.max_register_count_mut() = allocator.max_register_count;
        }
    });
}

struct RegisterAllocator {
    register_count: usize,
    scope_register_counts: Vec<usize>,
    max_register_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Allocation {
    Register(usize),
}

impl RegisterAllocator {
    const fn allocate(&mut self) -> Allocation {
        self.register_count += 1;

        Allocation::Register(self.register_count - 1)
    }

    fn assign(&mut self, to: &mut Value, addresses: &mut HashMap<Address, Allocation>) {
        match to {
            Value::CallArgument(index) => {
                *to = Value::Register(*index);
            }
            Value::Address(to_address) => {
                let to_address = *to_address;

                addresses
                    .entry(to_address)
                    .or_insert_with(|| self.allocate());

                *to = Self::allocation_to_value(
                    *addresses
                        .get(&to_address)
                        .expect("the address was just allocated"),
                );
            }
            _ => {
                unreachable!(
                    "only call arguments and addresses are assignment destinations at this point"
                );
            }
        }
    }

    fn allocate_block(
        &mut self,
        ssa: &mut Ssa,
        block_index: BlockIndex,
        addresses: &mut HashMap<Address, Allocation>,
    ) {
        if let Some(block) = ssa.get_block_mut(block_index) {
            for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                match instruction {
                    Instruction::NoOp => {}
                    Instruction::ScopeStart => {
                        self.scope_register_counts.push(self.register_count);
                    }
                    Instruction::PopN(count) => {
                        let register_count = self.register_count;

                        *count = register_count
                            - self
                                .scope_register_counts
                                .last()
                                .map_or(0, |register_count| *register_count);

                        self.register_count = self
                            .scope_register_counts
                            .last()
                            .map_or(0, |register_count| *register_count);

                        self.max_register_count = self.max_register_count.max(register_count);
                    }
                    Instruction::Push(value) => {
                        self.allocate();

                        self.value_to_allocation(value, (block_index, i), addresses);
                    }
                    Instruction::AccessAssign { value, of, .. } => {
                        self.value_to_allocation(value, (block_index, i), addresses);

                        self.value_to_allocation(of, (block_index, i), addresses);
                    }
                    Instruction::Call {
                        callee: value,
                        temporary: to,
                        arity,
                    } => {
                        self.value_to_allocation(value, (block_index, i), addresses);

                        self.max_register_count = self.max_register_count.max(self.register_count);

                        self.register_count -= *arity;

                        self.assign(to, addresses);
                    }
                    Instruction::Unary {
                        operand: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Assign { value, to }
                    | Instruction::Access {
                        of: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::GetTag {
                        of: value,
                        temporary: to,
                    } => {
                        self.value_to_allocation(value, (block_index, i), addresses);

                        self.assign(to, addresses);
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        self.value_to_allocation(lhs, (block_index, i), addresses);

                        self.value_to_allocation(rhs, (block_index, i), addresses);

                        self.assign(to, addresses);
                    }
                }
            }

            let i = block.instructions().len();

            match block.terminator_mut() {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Return(value)
                | BlockTerminator::Branch {
                    condition: value, ..
                } => {
                    self.value_to_allocation(value, (block_index, i), addresses);
                }
            }
        }
    }

    const fn allocation_to_value(allocation: Allocation) -> Value {
        match allocation {
            Allocation::Register(offset) => Value::Register(offset),
        }
    }

    #[allow(
        clippy::unused_self,
        clippy::only_used_in_recursion,
        clippy::self_only_used_in_recursion
    )]
    fn value_to_allocation(
        &mut self,
        value: &mut Value,
        (block_index, i): (BlockIndex, usize),
        addresses: &HashMap<Address, Allocation>,
    ) {
        match value {
            Value::Address(address) => {
                let address = *address;

                *value = addresses.get(&address).map_or_else(
                    || {
                        unreachable!(
                            "all addresses get allocated: ({block_index:?}, {i}) {value:?}"
                        )
                    },
                    |allocation| Self::allocation_to_value(*allocation),
                );
            }
            Value::CallArgument(index) => {
                *value = Value::Register(*index);
            }
            Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
                for value in values {
                    self.value_to_allocation(value, (block_index, i), addresses);
                }
            }
            _ => {}
        }
    }
}
