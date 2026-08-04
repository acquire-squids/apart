use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{Argument, BlockTerminator, Ssa},
};

use std::collections::HashMap;

pub fn allocate<const MAX_REGISTERS: usize>(ssa: &mut Ssa) {
    let mut allocator = RegisterAllocator::<MAX_REGISTERS> {
        index: 0,
        free: vec![],
        stack_size: 0,
        current_fn: BlockIndex(0),
    };

    let mut seen = vec![false; ssa.blocks().len()];

    for b in 0..(ssa.function_count()) {
        allocator.current_fn = BlockIndex(b);

        allocator.stack_size = 0;

        allocator.allocate_block(ssa, BlockIndex(b), &mut seen);
    }
}

struct RegisterAllocator<const MAX_REGISTERS: usize> {
    index: usize,
    free: Vec<usize>,
    stack_size: usize,
    current_fn: BlockIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Allocation {
    Register(usize),
    Stack(usize),
}

impl<const MAX_REGISTERS: usize> RegisterAllocator<MAX_REGISTERS> {
    fn allocate(&mut self) -> Allocation {
        if self.index < MAX_REGISTERS {
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

            for instruction in block.instructions_mut() {
                match instruction {
                    Instruction::NoOp | Instruction::Pop => {}
                    Instruction::Push(value) => {
                        if let Value::Address(address) = value {
                            *value = match addresses.get(address) {
                                None => unreachable!("all addresses get allocated"),
                                Some(Allocation::Register(index)) => Value::Register(*index),
                                Some(Allocation::Stack(offset)) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        }
                    }
                    Instruction::Unary {
                        operand: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Call {
                        callee: value,
                        temporary: to,
                    }
                    | Instruction::Assign { value, to } => {
                        if let Value::Address(address) = value {
                            *value = match addresses.get(address) {
                                None => unreachable!("all addresses get allocated"),
                                Some(Allocation::Register(index)) => Value::Register(*index),
                                Some(Allocation::Stack(offset)) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        }

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
                        if let Value::Address(address) = lhs {
                            *lhs = match addresses.get(address) {
                                None => unreachable!("all addresses get allocated"),
                                Some(Allocation::Register(index)) => Value::Register(*index),
                                Some(Allocation::Stack(offset)) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        }

                        if let Value::Address(address) = rhs {
                            *rhs = match addresses.get(address) {
                                None => unreachable!("all addresses get allocated"),
                                Some(Allocation::Register(index)) => Value::Register(*index),
                                Some(Allocation::Stack(offset)) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        }

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
                    if let Value::Address(address) = value {
                        *value = match addresses.get(address) {
                            None => unreachable!("all addresses get allocated"),
                            Some(Allocation::Register(index)) => Value::Register(*index),
                            Some(Allocation::Stack(offset)) => Value::Address(Address {
                                block_index: self.current_fn,
                                offset: *offset,
                                version: 0,
                            }),
                        };
                    }
                }
            }

            match block.terminator_mut() {
                BlockTerminator::Return(_) => {}
                BlockTerminator::Jump(jump_to) => {
                    for (address, allocation) in &addresses {
                        if let Some(address) = jump_to.arguments_mut().iter_mut().find_map(|argument| {
                            if let Argument::Address(argument) = argument
                                && matches!(argument, Value::Address(argument_address) if argument_address == address)
                            {
                                Some(argument)
                            } else {
                                None
                            }
                        }) {
                            *address = match allocation {
                                Allocation::Register(index) => Value::Register(*index),
                                Allocation::Stack(offset) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        } else if let Allocation::Register(index) = allocation {
                            self.free(*index);
                        }
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
                            if let Argument::Address(argument) = argument
                                && matches!(argument, Value::Address(argument_address) if argument_address == address)
                            {
                                Some(argument)
                            } else {
                                None
                            }
                        }) {
                            *address = match allocation {
                                Allocation::Register(index) => Value::Register(*index),
                                Allocation::Stack(offset) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        } else {
                            allocations_unused.push(allocation);
                        }
                    }

                    for (address, allocation) in &addresses {
                        if let Some(address) = otherwise.arguments_mut().iter_mut().find_map(|argument| {
                            if let Argument::Address(argument) = argument
                                && matches!(argument, Value::Address(argument_address) if argument_address == address)
                            {
                                Some(argument)
                            } else {
                                None
                            }
                        }) {
                            allocations_unused.retain(|unused_allocation| unused_allocation != &allocation);

                            *address = match allocation {
                                Allocation::Register(index) => Value::Register(*index),
                                Allocation::Stack(offset) => Value::Address(Address {
                                    block_index: self.current_fn,
                                    offset: *offset,
                                    version: 0,
                                }),
                            };
                        } else {
                            allocations_unused.push(allocation);
                        }
                    }

                    for allocation in allocations_unused {
                        if let Allocation::Register(index) = allocation {
                            self.free(*index);
                        }
                    }
                }
            }

            let mut children = vec![];

            ssa.for_children(block_index, |_, block_index| children.push(block_index));

            for child in children {
                self.stack_size = 0;
                self.allocate_block(ssa, child, seen);
            }
        }
    }
}
