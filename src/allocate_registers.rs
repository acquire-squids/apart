use crate::{
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{BlockTerminator, Ssa},
};

use std::collections::HashMap;

pub fn allocate(ssa: &mut Ssa) {
    ssa.for_functions(|ssa, blocks| {
        let mut lifespans = HashMap::new();

        for block_index in &blocks {
            RegisterAllocator::find_lifespans(ssa, *block_index, &mut lifespans);
        }

        let mut addresses = HashMap::new();

        let mut allocator = RegisterAllocator {
            index: 0,
            free: vec![],
            stack_size: 0,
            max_registers: ssa.max_registers(),
            scope_stack_sizes: vec![],
            max_stack_size: 0,
        };

        if let Some(block) = blocks
            .first()
            .and_then(|block_index| ssa.get_block(*block_index))
        {
            for _ in 0..(block.call_argument_count()) {
                allocator.allocate(false);
            }
        }

        for block_index in blocks.iter().copied() {
            allocator.allocate_block(ssa, block_index, &lifespans, &mut addresses);

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
            *block.max_stack_size_mut() = allocator.max_stack_size;
        }
    });
}

struct RegisterAllocator {
    index: usize,
    free: Vec<usize>,
    stack_size: usize,
    max_registers: usize,
    scope_stack_sizes: Vec<usize>,
    max_stack_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Allocation {
    Register(usize),
    Stack(usize),
}

type LifeEnds = HashMap<Address, (BlockIndex, usize)>;

impl RegisterAllocator {
    fn allocate(&mut self, allow_register: bool) -> Allocation {
        if self.index < self.max_registers && allow_register {
            self.index += 1;

            Allocation::Register(self.index - 1)
        } else if let Some(free) = self.free.pop()
            && allow_register
        {
            Allocation::Register(free)
        } else {
            self.stack_size += 1;

            Allocation::Stack(self.stack_size - 1)
        }
    }

    fn free(&mut self, register: usize) {
        self.free.push(register);
    }

    fn find_lifespans(ssa: &Ssa, block_index: BlockIndex, lifespans: &mut LifeEnds) {
        if let Some(block) = ssa.get_block(block_index) {
            for (i, instruction) in block.instructions().iter().enumerate() {
                match instruction {
                    Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(_) => {}
                    Instruction::Push(value) => {
                        Self::adjust_lifespan(value, (block_index, i), lifespans);
                    }
                    Instruction::AccessAssign { value, of, .. } => {
                        Self::adjust_lifespan(value, (block_index, i), lifespans);

                        Self::adjust_lifespan(of, (block_index, i), lifespans);
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
                    }
                    | Instruction::GetTag {
                        of: value,
                        temporary: to,
                    } => {
                        Self::adjust_lifespan(value, (block_index, i), lifespans);

                        Self::adjust_lifespan(to, (block_index, i), lifespans);
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        Self::adjust_lifespan(lhs, (block_index, i), lifespans);

                        Self::adjust_lifespan(rhs, (block_index, i), lifespans);

                        Self::adjust_lifespan(to, (block_index, i), lifespans);
                    }
                }
            }

            let i = block.instructions().len();

            match block.terminator() {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Return(value)
                | BlockTerminator::Branch {
                    condition: value, ..
                } => {
                    Self::adjust_lifespan(value, (block_index, i), lifespans);
                }
            }
        }
    }

    fn adjust_lifespan(
        value: &Value,
        (block_index, i): (BlockIndex, usize),
        lifespans: &mut LifeEnds,
    ) {
        match value {
            Value::Address(address) => {
                lifespans
                    .entry(*address)
                    .and_modify(|(end, e)| {
                        if block_index >= *end {
                            *end = block_index;
                            *e = i;
                        }
                    })
                    .or_insert((block_index, i));
            }
            Value::Compound(fields) | Value::TaggedCompound { fields, .. } => {
                for field in fields {
                    Self::adjust_lifespan(field, (block_index, i), lifespans);
                }
            }
            _ => {}
        }
    }

    fn assign(
        &mut self,
        to: &mut Value,
        (block_index, i): (BlockIndex, usize),
        lifespans: &LifeEnds,
        addresses: &mut HashMap<Address, Allocation>,
    ) {
        match to {
            Value::CallArgument(index) => {
                *to = Value::StackOffset(*index);
            }
            Value::Address(to_address) => {
                let to_address = *to_address;

                addresses
                    .entry(to_address)
                    .or_insert_with(|| self.allocate(true));

                *to = Self::allocation_to_value(
                    *addresses
                        .get(&to_address)
                        .expect("the address was just allocated"),
                );

                if let Value::Register(index) = to
                    && let Some((end, e)) = lifespans.get(&to_address)
                    && (*end < block_index || (*end == block_index && *e <= i))
                {
                    self.free(*index);
                }
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
        lifespans: &LifeEnds,
        addresses: &mut HashMap<Address, Allocation>,
    ) {
        if let Some(block) = ssa.get_block_mut(block_index) {
            for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                match instruction {
                    Instruction::NoOp => {}
                    Instruction::ScopeStart => {
                        self.scope_stack_sizes.push(self.stack_size);
                    }
                    Instruction::PopN(count) => {
                        let stack_size = self.stack_size;

                        *count = stack_size
                            - self
                                .scope_stack_sizes
                                .last()
                                .map_or(0, |stack_size| *stack_size);

                        self.stack_size = self
                            .scope_stack_sizes
                            .last()
                            .map_or(0, |stack_size| *stack_size);

                        self.max_stack_size = self.max_stack_size.max(stack_size);
                    }
                    Instruction::Push(value) => {
                        self.allocate(false);

                        self.value_to_allocation(value, (block_index, i), lifespans, addresses);
                    }
                    Instruction::AccessAssign { value, of, .. } => {
                        self.value_to_allocation(value, (block_index, i), lifespans, addresses);

                        self.value_to_allocation(of, (block_index, i), lifespans, addresses);
                    }
                    Instruction::Call {
                        callee: value,
                        temporary: to,
                        arity,
                    } => {
                        self.value_to_allocation(value, (block_index, i), lifespans, addresses);

                        self.max_stack_size = self.max_stack_size.max(self.stack_size);

                        self.stack_size -= *arity;

                        self.assign(to, (block_index, i), lifespans, addresses);
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
                        self.value_to_allocation(value, (block_index, i), lifespans, addresses);

                        self.assign(to, (block_index, i), lifespans, addresses);
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        self.value_to_allocation(lhs, (block_index, i), lifespans, addresses);

                        self.value_to_allocation(rhs, (block_index, i), lifespans, addresses);

                        self.assign(to, (block_index, i), lifespans, addresses);
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
                    self.value_to_allocation(value, (block_index, i), lifespans, addresses);
                }
            }
        }
    }

    const fn allocation_to_value(allocation: Allocation) -> Value {
        match allocation {
            Allocation::Register(index) => Value::Register(index),
            Allocation::Stack(offset) => Value::StackOffset(offset),
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
        lifespans: &LifeEnds,
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

                if let Value::Register(index) = value
                    && let Some((end, e)) = lifespans.get(&address)
                    && (*end < block_index || (*end == block_index && *e <= i))
                {
                    self.free(*index);
                }
            }
            Value::CallArgument(index) => {
                *value = Value::StackOffset(*index);
            }
            Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
                for value in values {
                    self.value_to_allocation(value, (block_index, i), lifespans, addresses);
                }
            }
            _ => {}
        }
    }
}
