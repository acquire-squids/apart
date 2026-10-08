use crate::basic_blocks::{
    Address, BasicBlocks, BlockIndex, BlockTerminator as BasicBlockTerminator, Instruction, Value,
};

use std::{collections::HashSet, fmt};

pub fn convert(basic_blocks: &BasicBlocks, max_registers: usize) -> Ssa {
    let mut ssa = Ssa {
        blocks: vec![],
        max_registers,
    };

    for basic_block in basic_blocks.blocks() {
        ssa.blocks.push(Block {
            call_argument_count: basic_block.call_argument_count(),
            max_stack_size: 0,
            instructions: basic_block.instructions().to_vec(),
            terminator: match basic_block.terminator() {
                BasicBlockTerminator::Jump(block_index) => BlockTerminator::Jump(*block_index),
                BasicBlockTerminator::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => BlockTerminator::Branch {
                    condition: condition.clone(),
                    when_true: *when_true,
                    otherwise: *otherwise,
                },
                BasicBlockTerminator::Return(value) => BlockTerminator::Return(value.clone()),
            },
        });
    }

    ssa.liveliness();

    ssa
}

pub struct Ssa {
    blocks: Vec<Block>,
    max_registers: usize,
}

impl fmt::Display for Ssa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (b, block) in self.blocks().iter().enumerate() {
            writeln!(f, "{b}:")?;

            write!(f, "{block}")?;
        }

        Ok(())
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for instruction in self.instructions() {
            writeln!(f, "    {instruction:?}")?;
        }

        writeln!(f, "    {:?}", self.terminator())
    }
}

impl Ssa {
    #[allow(dead_code)]
    #[must_use]
    pub const fn max_registers(&self) -> usize {
        self.max_registers
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn blocks(&self) -> &[Block] {
        self.blocks.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn blocks_mut(&mut self) -> &mut Vec<Block> {
        &mut self.blocks
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_block(&self, block_index: BlockIndex) -> Option<&Block> {
        self.blocks.get(usize::from(block_index))
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_block_mut(&mut self, block_index: BlockIndex) -> Option<&mut Block> {
        self.blocks.get_mut(usize::from(block_index))
    }

    #[allow(dead_code)]
    pub fn for_children<F>(&self, block_index: BlockIndex, mut f: F)
    where
        F: FnMut(&Self, BlockIndex),
    {
        let Some(block) = self.get_block(block_index) else {
            return;
        };

        match block.terminator() {
            BlockTerminator::Jump(b) => {
                f(self, *b);
            }
            BlockTerminator::Branch {
                when_true,
                otherwise,
                ..
            } => {
                let (when_true, otherwise) = (*when_true, *otherwise);

                f(self, when_true);
                f(self, otherwise);
            }
            BlockTerminator::Return(_) => {}
        }
    }
}

pub struct Block {
    call_argument_count: usize,
    max_stack_size: usize,
    instructions: Vec<Instruction>,
    terminator: BlockTerminator,
}

#[derive(Debug)]
pub enum BlockTerminator {
    Jump(BlockIndex),
    Branch {
        condition: Value,
        when_true: BlockIndex,
        otherwise: BlockIndex,
    },
    Return(Value),
}

impl Block {
    #[allow(dead_code)]
    #[must_use]
    pub const fn instructions(&self) -> &[Instruction] {
        self.instructions.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn instructions_mut(&mut self) -> &mut Vec<Instruction> {
        &mut self.instructions
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn call_argument_count(&self) -> usize {
        self.call_argument_count
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn call_argument_count_mut(&mut self) -> &mut usize {
        &mut self.call_argument_count
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn terminator(&self) -> &BlockTerminator {
        &self.terminator
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn terminator_mut(&mut self) -> &mut BlockTerminator {
        &mut self.terminator
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn max_stack_size(&self) -> usize {
        self.max_stack_size
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn max_stack_size_mut(&mut self) -> &mut usize {
        &mut self.max_stack_size
    }
}

impl Ssa {
    /// # Attribution
    /// Derived from `rustc_middle`'s `Postorder::visit`, found [here](https://github.com/rust-lang/rust/blob/260f1acad9d7f70b5b56a636a9dc5a7f76150417/compiler/rustc_middle/src/mir/traversal.rs#L125-L134)
    fn visit(
        &self,
        block_index: BlockIndex,
        stack: &mut Vec<(BlockIndex, Vec<BlockIndex>)>,
        seen: &mut HashSet<BlockIndex>,
    ) {
        if !seen.insert(block_index) {
            return;
        }

        let mut children = vec![];

        self.for_children(block_index, |_, child_block_index| {
            children.push(child_block_index);
        });

        stack.push((block_index, children));
    }

    /// # Attribution
    /// Derived from `rustc_middle`'s `Postorder::traverse_successor`, found [here](https://github.com/rust-lang/rust/blob/260f1acad9d7f70b5b56a636a9dc5a7f76150417/compiler/rustc_middle/src/mir/traversal.rs#L185-L187)
    fn visit_successors(
        &self,
        stack: &mut Vec<(BlockIndex, Vec<BlockIndex>)>,
        seen: &mut HashSet<BlockIndex>,
    ) {
        while let Some(block_index) = stack
            .last_mut()
            .and_then(|(_, successors)| successors.pop())
        {
            self.visit(block_index, stack, seen);
        }
    }

    /// # Attribution
    /// Derived from `rustc_middle`'s `<Postorder as Iterator>::next`, found [here](https://github.com/rust-lang/rust/blob/260f1acad9d7f70b5b56a636a9dc5a7f76150417/compiler/rustc_middle/src/mir/traversal.rs#L195-L198)
    fn postorder(&self, block_index: BlockIndex) -> Vec<BlockIndex> {
        let mut stack = vec![];
        let mut seen = HashSet::new();

        let mut ordered = vec![];

        self.visit(block_index, &mut stack, &mut seen);
        self.visit_successors(&mut stack, &mut seen);

        while let Some((block_index, _)) = stack.pop() {
            self.visit_successors(&mut stack, &mut seen);

            ordered.push(block_index);
        }

        ordered
    }

    #[allow(dead_code)]
    pub fn for_functions<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self, Vec<BlockIndex>),
    {
        let mut seen = HashSet::new();

        let mut functions = vec![];

        let mut frontier = vec![BlockIndex(0)];

        while let Some(block_to_explore) = frontier.pop() {
            let ordered = self.postorder(block_to_explore);

            let mut function = vec![];

            for block_index in ordered {
                if !seen.insert(block_index) {
                    continue;
                }

                function.push(block_index);

                if let Some(block) = self.get_block(block_index) {
                    for instruction in block.instructions() {
                        match instruction {
                            Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(_) => {}
                            Instruction::Push(value) => {
                                Self::value_uses_block(value, &mut frontier);
                            }
                            Instruction::Unary {
                                operand: value,
                                temporary: to,
                                ..
                            }
                            | Instruction::Assign { value, to, .. }
                            | Instruction::Call {
                                callee: value,
                                temporary: to,
                                ..
                            }
                            | Instruction::Access {
                                of: value,
                                temporary: to,
                                ..
                            }
                            | Instruction::GetTag {
                                of: value,
                                temporary: to,
                                ..
                            } => {
                                Self::value_uses_block(value, &mut frontier);

                                Self::value_uses_block(to, &mut frontier);
                            }
                            Instruction::Binary {
                                lhs,
                                rhs,
                                temporary: to,
                                ..
                            } => {
                                Self::value_uses_block(lhs, &mut frontier);
                                Self::value_uses_block(rhs, &mut frontier);

                                Self::value_uses_block(to, &mut frontier);
                            }
                            Instruction::AccessAssign { of, value, .. } => {
                                Self::value_uses_block(of, &mut frontier);
                                Self::value_uses_block(value, &mut frontier);
                            }
                        }
                    }

                    match block.terminator() {
                        BlockTerminator::Branch {
                            condition: value, ..
                        }
                        | BlockTerminator::Return(value) => {
                            Self::value_uses_block(value, &mut frontier);
                        }
                        BlockTerminator::Jump(_) => {}
                    }
                }
            }

            if !function.is_empty() {
                function.reverse();

                functions.push(function);
            }
        }

        for function in functions {
            f(self, function);
        }
    }

    #[allow(dead_code)]
    pub fn for_live_blocks<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self, BlockIndex),
    {
        let mut seen = HashSet::new();

        let mut frontier = vec![BlockIndex(0)];

        while let Some(block_to_explore) = frontier.pop() {
            let ordered = self.postorder(block_to_explore);

            for b in ordered {
                if !seen.insert(b) {
                    continue;
                }

                if let Some(block) = self.get_block(b) {
                    for instruction in block.instructions() {
                        match instruction {
                            Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(_) => {}
                            Instruction::Push(value) => {
                                Self::value_uses_block(value, &mut frontier);
                            }
                            Instruction::Unary {
                                operand: value,
                                temporary: to,
                                ..
                            }
                            | Instruction::Assign { value, to, .. }
                            | Instruction::Call {
                                callee: value,
                                temporary: to,
                                ..
                            }
                            | Instruction::Access {
                                of: value,
                                temporary: to,
                                ..
                            }
                            | Instruction::GetTag {
                                of: value,
                                temporary: to,
                                ..
                            } => {
                                Self::value_uses_block(value, &mut frontier);

                                Self::value_uses_block(to, &mut frontier);
                            }
                            Instruction::Binary {
                                lhs,
                                rhs,
                                temporary: to,
                                ..
                            } => {
                                Self::value_uses_block(lhs, &mut frontier);
                                Self::value_uses_block(rhs, &mut frontier);

                                Self::value_uses_block(to, &mut frontier);
                            }
                            Instruction::AccessAssign { of, value, .. } => {
                                Self::value_uses_block(of, &mut frontier);
                                Self::value_uses_block(value, &mut frontier);
                            }
                        }
                    }

                    match block.terminator() {
                        BlockTerminator::Branch {
                            condition: value, ..
                        }
                        | BlockTerminator::Return(value) => {
                            Self::value_uses_block(value, &mut frontier);
                        }
                        BlockTerminator::Jump(_) => {}
                    }
                }

                f(self, b);
            }
        }
    }

    fn value_uses_block(value: &Value, frontier: &mut Vec<BlockIndex>) {
        match value {
            Value::Fn(block_index) | Value::Address(Address { block_index, .. }) => {
                if !frontier.contains(block_index) {
                    frontier.push(*block_index);
                }
            }
            Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
                for value in values {
                    Self::value_uses_block(value, frontier);
                }
            }
            _ => {}
        }
    }

    fn nested_values<F>(value: &mut Value, f: &mut F)
    where
        F: FnMut(&mut Value),
    {
        match value {
            Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
                for value in values {
                    f(value);
                }
            }
            _ => {
                f(value);
            }
        }
    }

    #[allow(dead_code)]
    pub fn for_value<F>(&mut self, block_index: BlockIndex, mut f: F)
    where
        F: FnMut(&mut Value),
    {
        if let Some(block) = self.get_block_mut(block_index) {
            for instruction in block.instructions_mut() {
                match instruction {
                    Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(_) => {}
                    Instruction::Push(value) => {
                        Self::nested_values(value, &mut f);
                    }
                    Instruction::Unary {
                        operand: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Assign { value, to, .. }
                    | Instruction::Call {
                        callee: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Access {
                        of: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::GetTag {
                        of: value,
                        temporary: to,
                        ..
                    } => {
                        Self::nested_values(value, &mut f);

                        Self::nested_values(to, &mut f);
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        Self::nested_values(lhs, &mut f);
                        Self::nested_values(rhs, &mut f);

                        Self::nested_values(to, &mut f);
                    }
                    Instruction::AccessAssign { of, value, .. } => {
                        Self::nested_values(of, &mut f);
                        Self::nested_values(value, &mut f);
                    }
                }
            }

            match block.terminator_mut() {
                BlockTerminator::Branch {
                    condition: value, ..
                }
                | BlockTerminator::Return(value) => {
                    Self::nested_values(value, &mut f);
                }
                BlockTerminator::Jump(_) => {}
            }
        }
    }

    fn liveliness(&mut self) {
        let addresses = self.find_addresses();

        self.map_addresses(addresses.as_slice());

        let mut blocks_used = HashSet::new();

        self.for_live_blocks(|_, b| {
            blocks_used.insert(b);
        });

        self.clean_dead_blocks(&blocks_used);
    }

    fn clean_dead_blocks(&mut self, blocks_used: &HashSet<BlockIndex>) {
        for b in (0..(self.blocks.len())).map(BlockIndex) {
            if !blocks_used.contains(&b)
                && let Some(block) = self.get_block_mut(b)
            {
                block.instructions.clear();

                block.terminator = BlockTerminator::Return(Value::Runtime);
            }
        }
    }

    fn find_addresses(&mut self) -> Vec<(Address, Address)> {
        let mut addresses = vec![];

        self.for_live_blocks(|ssa, block_index| {
            if let Some(block) = ssa.get_block_mut(block_index) {
                for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                    match instruction {
                        Instruction::NoOp
                        | Instruction::Push(_)
                        | Instruction::AccessAssign { .. }
                        | Instruction::ScopeStart
                        | Instruction::PopN(_) => {}
                        Instruction::Unary { temporary: to, .. }
                        | Instruction::Call { temporary: to, .. }
                        | Instruction::Access { temporary: to, .. }
                        | Instruction::GetTag { temporary: to, .. }
                        | Instruction::Binary { temporary: to, .. }
                        | Instruction::Assign { to, .. } => {
                            if let Value::Address(to) = to {
                                let new_address = if to.block_index == block_index {
                                    Address {
                                        block_index,
                                        offset: i,
                                        version: 0,
                                    }
                                } else {
                                    Address {
                                        block_index: to.block_index,
                                        offset: to.offset,
                                        version: 0,
                                    }
                                };

                                addresses.push((*to, new_address));

                                *to = new_address;
                            }
                        }
                    }
                }
            }
        });

        addresses
    }

    fn map_addresses(&mut self, addresses: &[(Address, Address)]) {
        self.for_live_blocks(|ssa, block_index| {
            if let Some(block) = ssa.get_block_mut(block_index) {
                for instruction in block.instructions_mut() {
                    match instruction {
                        Instruction::NoOp | Instruction::ScopeStart | Instruction::PopN(_) => {}
                        Instruction::Push(value)
                        | Instruction::Unary { operand: value, .. }
                        | Instruction::Call { callee: value, .. }
                        | Instruction::Access { of: value, .. }
                        | Instruction::GetTag { of: value, .. }
                        | Instruction::Assign { value, .. } => {
                            Self::map_address(value, addresses);
                        }
                        Instruction::Binary { lhs, rhs, .. } => {
                            Self::map_address(lhs, addresses);
                            Self::map_address(rhs, addresses);
                        }
                        Instruction::AccessAssign { of, value, .. } => {
                            Self::map_address(of, addresses);
                            Self::map_address(value, addresses);
                        }
                    }
                }

                match &mut block.terminator {
                    BlockTerminator::Jump(_) => {}
                    BlockTerminator::Branch {
                        condition: value, ..
                    }
                    | BlockTerminator::Return(value) => {
                        Self::map_address(value, addresses);
                    }
                }
            }
        });
    }

    fn map_address(value: &mut Value, addresses: &[(Address, Address)]) {
        match value {
            Value::Address(address) => {
                if let Some((_, new_address)) = addresses
                    .iter()
                    .find(|(old_address, _)| old_address == address)
                    .copied()
                {
                    *address = new_address;
                }
            }
            Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
                for value in values {
                    Self::map_address(value, addresses);
                }
            }
            _ => {}
        }
    }
}
