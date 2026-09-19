use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashMap;

pub fn allocate(ssa: &mut Ssa) {
    let mut seen = vec![false; ssa.blocks().len()];

    for b in 0..(ssa.function_count()) {
        let mut allocator = RegisterAllocator {
            index: 0,
            free: vec![],
            stack_size: 0,
            current_fn: BlockIndex(b),
            max_registers: ssa.max_registers(),
        };

        allocator.allocate_block(ssa, BlockIndex(b), &mut seen);
    }

    for block in ssa.blocks_mut() {
        block
            .instructions_mut()
            .retain(|instruction| !matches!(instruction, Instruction::NoOp));
    }
}

struct RegisterAllocator {
    index: usize,
    free: Vec<usize>,
    stack_size: usize,
    current_fn: BlockIndex,
    max_registers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Allocation {
    Register(usize),
    Stack(usize),
}

impl RegisterAllocator {
    fn allocate(&mut self) -> Allocation {
        if self.index < self.max_registers {
            self.index += 1;

            Allocation::Register(self.index - 1)
        } else if let Some(free) = self.free.pop() {
            Allocation::Register(free)
        } else {
            self.stack_size += 1;

            Allocation::Stack(self.stack_size - 1)
        }
    }

    fn free(&mut self, register: usize) {
        self.free.push(register);
    }

    #[allow(clippy::too_many_lines)]
    fn allocate_block(&mut self, ssa: &mut Ssa, block_index: BlockIndex, seen: &mut Vec<bool>) {
        if seen.get(usize::from(block_index)).is_some_and(|seen| !seen)
            && let Some(block) = ssa.get_block_mut(block_index)
        {
            seen[usize::from(block_index)] = true;

            let mut addresses = HashMap::new();

            let mut block_arguments = vec![];
            let mut call_arguments = vec![];

            for parameter in block.parameters_mut() {
                if let Value::Address(parameter_address) = parameter {
                    let allocated = self.allocate();

                    addresses.insert(*parameter_address, allocated);

                    block_arguments.push(allocated);

                    *parameter = self.allocation_to_value(allocated);
                }
            }

            for _ in 0..(block.call_argument_count()) {
                let allocated = self.allocate();

                call_arguments.push(allocated);

                block
                    .parameters_mut()
                    .push(self.allocation_to_value(allocated));
            }

            for instruction in block.instructions_mut() {
                match instruction {
                    Instruction::NoOp => {}
                    Instruction::Push(value) => {
                        self.value_to_allocation(
                            &addresses,
                            value,
                            block_arguments.as_slice(),
                            call_arguments.as_slice(),
                        );
                    }
                    Instruction::AccessAssign { value, of, .. } => {
                        self.value_to_allocation(
                            &addresses,
                            value,
                            block_arguments.as_slice(),
                            call_arguments.as_slice(),
                        );
                        self.value_to_allocation(
                            &addresses,
                            of,
                            block_arguments.as_slice(),
                            call_arguments.as_slice(),
                        );
                    }
                    Instruction::Unary {
                        operand: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Call {
                        callee: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Assign { value, to }
                    | Instruction::Access {
                        of: value,
                        temporary: to,
                        ..
                    } => {
                        self.value_to_allocation(
                            &addresses,
                            value,
                            block_arguments.as_slice(),
                            call_arguments.as_slice(),
                        );

                        let Value::Address(to_address) = to else {
                            unreachable!("destinations can only be addresses at this point");
                        };

                        addresses.insert(*to_address, self.allocate());

                        *to = match addresses
                            .get(to_address)
                            .expect("the address was just allocated")
                        {
                            Allocation::Register(index) => Value::Register(*index),
                            Allocation::Stack(offset) => Value::Address(Address {
                                block_index: self.current_fn,
                                offset: *offset,
                                version: 0,
                            }),
                        };
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        self.value_to_allocation(
                            &addresses,
                            lhs,
                            block_arguments.as_slice(),
                            call_arguments.as_slice(),
                        );

                        self.value_to_allocation(
                            &addresses,
                            rhs,
                            block_arguments.as_slice(),
                            call_arguments.as_slice(),
                        );

                        let Value::Address(to_address) = to else {
                            unreachable!("destinations can only be addresses at this point");
                        };

                        addresses.insert(*to_address, self.allocate());

                        *to = match addresses
                            .get(to_address)
                            .expect("the address was just allocated")
                        {
                            Allocation::Register(index) => Value::Register(*index),
                            Allocation::Stack(offset) => Value::Address(Address {
                                block_index: self.current_fn,
                                offset: *offset,
                                version: 0,
                            }),
                        };
                    }
                }
            }

            match block.terminator_mut() {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Return(value)
                | BlockTerminator::Branch {
                    condition: value, ..
                } => {
                    self.value_to_allocation(
                        &addresses,
                        value,
                        block_arguments.as_slice(),
                        call_arguments.as_slice(),
                    );
                }
            }

            match block.terminator_mut() {
                BlockTerminator::Return(_) => {}
                BlockTerminator::Jump(jump_to) => {
                    for (address, allocation) in &addresses {
                        if let Some(address) = jump_to.arguments_mut().iter_mut().find_map(|argument| {
                            if matches!(argument, Value::Address(argument_address) if argument_address == address)
                            {
                                Some(argument)
                            } else {
                                None
                            }
                        }) {
                            self.value_to_allocation(&addresses, address, block_arguments.as_slice(), call_arguments.as_slice());
                        } else if let Allocation::Register(index) = allocation {
                            self.free(*index);
                        }
                    }

                    for allocated in &call_arguments {
                        jump_to
                            .arguments_mut()
                            .push(self.allocation_to_value(*allocated));
                    }
                }
                BlockTerminator::Branch {
                    when_true,
                    otherwise,
                    ..
                } => {
                    let mut allocations_unused = vec![];

                    for (address, allocation) in &addresses {
                        if let Some(address) = when_true.arguments_mut().iter_mut().find_map(|argument| {
                            if matches!(argument, Value::Address(argument_address) if argument_address == address)
                            {
                                Some(argument)
                            } else {
                                None
                            }
                        }) {
                            self.value_to_allocation(&addresses, address, block_arguments.as_slice(), call_arguments.as_slice());
                        } else {
                            allocations_unused.push(allocation);
                        }
                    }

                    for (address, allocation) in &addresses {
                        if let Some(address) = otherwise.arguments_mut().iter_mut().find_map(|argument| {
                            if matches!(argument, Value::Address(argument_address) if argument_address == address)
                            {
                                Some(argument)
                            } else {
                                None
                            }
                        }) {
                            allocations_unused.retain(|unused_allocation| unused_allocation != &allocation);

                            self.value_to_allocation(&addresses, address, block_arguments.as_slice(), call_arguments.as_slice());
                        } else {
                            allocations_unused.push(allocation);
                        }
                    }

                    for allocation in allocations_unused {
                        if let Allocation::Register(index) = allocation {
                            self.free(*index);
                        }
                    }

                    for allocated in &call_arguments {
                        when_true
                            .arguments_mut()
                            .push(self.allocation_to_value(*allocated));
                        otherwise
                            .arguments_mut()
                            .push(self.allocation_to_value(*allocated));
                    }
                }
            }

            let mut children = vec![];

            ssa.for_children(block_index, |_, block_index| children.push(block_index));

            for child in children {
                *self = Self {
                    index: 0,
                    free: vec![],
                    stack_size: 0,
                    current_fn: self.current_fn,
                    max_registers: self.max_registers,
                };

                self.allocate_block(ssa, child, seen);
            }
        }
    }

    const fn allocation_to_value(&self, allocation: Allocation) -> Value {
        match allocation {
            Allocation::Register(index) => Value::Register(index),
            Allocation::Stack(offset) => Value::Address(Address {
                block_index: self.current_fn,
                offset,
                version: 0,
            }),
        }
    }

    fn value_to_allocation(
        &self,
        addresses: &HashMap<Address, Allocation>,
        value: &mut Value,
        block_arguments: &[Allocation],
        call_arguments: &[Allocation],
    ) {
        match value {
            Value::Address(address) => {
                *value = addresses.get(address).map_or_else(
                    || unreachable!("all addresses get allocated"),
                    |allocation| self.allocation_to_value(*allocation),
                );
            }
            Value::BlockArgument(index) => {
                *value = block_arguments.get(*index).map_or_else(
                    || unreachable!("all addresses get allocated"),
                    |allocation| self.allocation_to_value(*allocation),
                );
            }
            Value::CallArgument(index) => {
                *value = call_arguments.get(*index).map_or_else(
                    || unreachable!("all addresses get allocated"),
                    |allocation| self.allocation_to_value(*allocation),
                );
            }
            Value::Compound(values) => {
                for value in values {
                    self.value_to_allocation(addresses, value, block_arguments, call_arguments);
                }
            }
            _ => {}
        }
    }
}
