use crate::{
    ApartOptions,
    basic_blocks::{
        Address, BasicBlocks, BlockIndex, BlockTerminator as BasicBlockTerminator, Instruction,
        Value,
    },
};

use std::{collections::HashSet, fmt};

pub fn convert(_: &ApartOptions, basic_blocks: &BasicBlocks) -> Ssa {
    let mut ssa = Ssa { blocks: vec![] };

    for basic_block in basic_blocks.blocks() {
        ssa.blocks.push(Block {
            call_argument_count: basic_block.call_argument_count(),
            max_register_count: 0,
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
        usize::try_from(u32::from(block_index))
            .ok()
            .and_then(|block_index| self.blocks.get(block_index))
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_block_mut(&mut self, block_index: BlockIndex) -> Option<&mut Block> {
        usize::try_from(u32::from(block_index))
            .ok()
            .and_then(|block_index| self.blocks.get_mut(block_index))
    }
}

pub struct Block {
    call_argument_count: u8,
    max_register_count: u16,
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
    pub const fn call_argument_count(&self) -> u8 {
        self.call_argument_count
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn call_argument_count_mut(&mut self) -> &mut u8 {
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
    pub const fn max_register_count(&self) -> u16 {
        self.max_register_count
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn max_register_count_mut(&mut self) -> &mut u16 {
        &mut self.max_register_count
    }
}

enum Successors {
    Pair(BlockIndex, BlockIndex),
    Some(BlockIndex),
    None,
}

impl Successors {
    const fn pop(&mut self) -> Option<BlockIndex> {
        match self {
            Self::Pair(first, second) => {
                let second = *second;

                *self = Self::Some(*first);

                Some(second)
            }
            Self::Some(first) => {
                let first = *first;

                *self = Self::None;

                Some(first)
            }
            Self::None => None,
        }
    }
}

#[derive(Debug)]
pub struct DefUse {
    definitions: HashSet<Address>,
    uses: HashSet<Address>,
}

impl DefUse {
    #[allow(dead_code)]
    #[must_use]
    pub const fn definitions(&self) -> &HashSet<Address> {
        &self.definitions
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn uses(&self) -> &HashSet<Address> {
        &self.uses
    }
}

#[derive(Debug)]
pub struct Liveness {
    live_in: HashSet<Address>,
    live_out: HashSet<Address>,
}

impl Liveness {
    #[allow(dead_code)]
    #[must_use]
    pub const fn live_in(&self) -> &HashSet<Address> {
        &self.live_in
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn live_out(&self) -> &HashSet<Address> {
        &self.live_out
    }
}

impl Ssa {
    /// # Attribution
    /// Derived from `rustc_middle`'s `Postorder::visit`, found [here](https://github.com/rust-lang/rust/blob/260f1acad9d7f70b5b56a636a9dc5a7f76150417/compiler/rustc_middle/src/mir/traversal.rs#L125-L134)
    fn visit(
        &self,
        block_index: BlockIndex,
        stack: &mut Vec<(BlockIndex, Successors)>,
        seen: &mut [bool],
    ) {
        let index = usize::try_from(u32::from(block_index))
            .unwrap_or_else(|_| panic!("number of blocks may not exceed {}", u32::MAX));

        if seen.get(index).is_some_and(|seen| *seen) {
            return;
        }

        seen[index] = true;

        match self.get_block(block_index).map(Block::terminator) {
            Some(BlockTerminator::Jump(destination)) => {
                stack.push((block_index, Successors::Some(*destination)));
            }
            Some(BlockTerminator::Branch {
                when_true,
                otherwise,
                ..
            }) => {
                stack.push((block_index, Successors::Pair(*when_true, *otherwise)));
            }
            None | Some(BlockTerminator::Return(_)) => {
                stack.push((block_index, Successors::None));
            }
        }
    }

    /// # Attribution
    /// Derived from `rustc_middle`'s `Postorder::traverse_successor`, found [here](https://github.com/rust-lang/rust/blob/260f1acad9d7f70b5b56a636a9dc5a7f76150417/compiler/rustc_middle/src/mir/traversal.rs#L185-L187)
    fn visit_successors(&self, stack: &mut Vec<(BlockIndex, Successors)>, seen: &mut [bool]) {
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
        let mut seen = vec![false; self.blocks().len()];

        let mut ordered = vec![];

        self.visit(block_index, &mut stack, &mut seen);
        self.visit_successors(&mut stack, &mut seen);

        while let Some((block_index, _)) = stack.pop() {
            self.visit_successors(&mut stack, &mut seen);

            ordered.push(block_index);
        }

        ordered
    }

    #[allow(dead_code, clippy::too_many_lines)]
    #[must_use]
    pub fn def_use_analysis(&self) -> Vec<DefUse> {
        let mut def_use = vec![];

        def_use.resize_with(self.blocks().len(), || DefUse {
            definitions: HashSet::new(),
            uses: HashSet::new(),
        });

        for (b, block) in self.blocks.iter().enumerate().rev() {
            for instruction in block.instructions() {
                match instruction {
                    Instruction::Panic | Instruction::FnLabel { .. } | Instruction::NoOp => {}
                    Instruction::Push(value) => {
                        if let Value::Address(address) = value
                            && address.block_index
                                != BlockIndex(
                                    u32::try_from(b).unwrap_or_else(|_| {
                                        panic!("block index was not a valid u32")
                                    }),
                                )
                            && let Some(def_use) = def_use.get_mut(b)
                            && !def_use.uses.contains(address)
                        {
                            def_use.uses.insert(*address);
                        }
                    }
                    Instruction::Unary {
                        operand: value,
                        temporary: to,
                        ..
                    }
                    | Instruction::Assign { value, to }
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
                    } => {
                        if let Value::Address(address) = to
                            && address.block_index
                                == BlockIndex(
                                    u32::try_from(b).unwrap_or_else(|_| {
                                        panic!("block index was not a valid u32")
                                    }),
                                )
                            && let Some(def_use) = def_use.get_mut(b)
                            && !def_use.definitions.contains(address)
                        {
                            def_use.definitions.insert(*address);
                        }

                        if let Value::Address(address) = value
                            && address.block_index
                                != BlockIndex(
                                    u32::try_from(b).unwrap_or_else(|_| {
                                        panic!("block index was not a valid u32")
                                    }),
                                )
                            && let Some(def_use) = def_use.get_mut(b)
                            && !def_use.uses.contains(address)
                        {
                            def_use.uses.insert(*address);
                        }
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        if let Value::Address(address) = to
                            && address.block_index
                                == BlockIndex(
                                    u32::try_from(b).unwrap_or_else(|_| {
                                        panic!("block index was not a valid u32")
                                    }),
                                )
                            && let Some(def_use) = def_use.get_mut(b)
                            && !def_use.definitions.contains(address)
                        {
                            def_use.definitions.insert(*address);
                        }

                        for value in [lhs, rhs] {
                            if let Value::Address(address) = value
                                && address.block_index
                                    != BlockIndex(u32::try_from(b).unwrap_or_else(|_| {
                                        panic!("block index was not a valid u32")
                                    }))
                                && let Some(def_use) = def_use.get_mut(b)
                                && !def_use.uses.contains(address)
                            {
                                def_use.uses.insert(*address);
                            }
                        }
                    }
                    Instruction::AccessAssign { of, value, .. } => {
                        for value in [of, value] {
                            if let Value::Address(address) = value
                                && address.block_index
                                    != BlockIndex(u32::try_from(b).unwrap_or_else(|_| {
                                        panic!("block index was not a valid u32")
                                    }))
                                && let Some(def_use) = def_use.get_mut(b)
                                && !def_use.uses.contains(address)
                            {
                                def_use.uses.insert(*address);
                            }
                        }
                    }
                }
            }

            match block.terminator() {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Branch {
                    condition: value, ..
                }
                | BlockTerminator::Return(value) => {
                    if let Value::Address(address) = value
                        && address.block_index
                            != BlockIndex(
                                u32::try_from(b)
                                    .unwrap_or_else(|_| panic!("block index was not a valid u32")),
                            )
                        && let Some(def_use) = def_use.get_mut(b)
                        && !def_use.uses.contains(address)
                    {
                        def_use.uses.insert(*address);
                    }
                }
            }
        }

        def_use
    }

    #[allow(dead_code, clippy::too_many_lines)]
    #[must_use]
    pub fn liveness_analysis(&self, def_use: &[DefUse]) -> Vec<Liveness> {
        let mut liveness = vec![];

        liveness.resize_with(self.blocks().len(), || Liveness {
            live_in: HashSet::new(),
            live_out: HashSet::new(),
        });

        let mut changed = true;

        while changed {
            changed = false;

            for (b, block) in self.blocks().iter().enumerate().rev() {
                match block.terminator() {
                    BlockTerminator::Return(_) => {}
                    BlockTerminator::Jump(destination) => {
                        let live =
                            liveness
                                .get(usize::try_from(u32::from(*destination)).unwrap_or_else(
                                    |_| panic!("block index was not a valid usize"),
                                ))
                                .into_iter()
                                .flat_map(|liveness| liveness.live_in.iter())
                                .filter(|live_in| {
                                    !matches!(liveness
                                        .get(b).map(|liveness| &liveness.live_out), Some(live_out) if live_out.contains(live_in))
                                })
                                .copied()
                                .collect::<Vec<_>>();

                        let Some(live_out) =
                            liveness.get_mut(b).map(|liveness| &mut liveness.live_out)
                        else {
                            panic!("there is a set for each block");
                        };

                        changed |= !live.is_empty();

                        live_out.extend(live);
                    }
                    BlockTerminator::Branch {
                        when_true,
                        otherwise,
                        ..
                    } => {
                        for destination in [when_true, otherwise] {
                            let live = liveness
                                .get(usize::try_from(u32::from(*destination)).unwrap_or_else(
                                    |_| panic!("block index was not a valid usize"),
                                ))
                                .into_iter()
                                .flat_map(|liveness| liveness.live_in.iter())
                                .filter(|live_in| {
                                    !matches!(liveness
                                        .get(b).map(|liveness| &liveness.live_out), Some(live_out) if live_out.contains(live_in))
                                })
                                .copied()
                                .collect::<Vec<_>>();

                            let Some(live_out) =
                                liveness.get_mut(b).map(|liveness| &mut liveness.live_out)
                            else {
                                panic!("there is a set for each block");
                            };

                            changed |= !live.is_empty();

                            live_out.extend(live);
                        }
                    }
                }

                let uses = def_use
                    .get(b)
                    .into_iter()
                    .flat_map(|def_use| def_use.uses().iter())
                    .filter(|used| {
                        !matches!(
                            liveness
                                .get(b)
                                .map(Liveness::live_in),
                            Some(live_in)
                                if live_in.contains(used)
                        )
                    })
                    .copied()
                    .collect::<Vec<_>>();

                let outs = liveness
                    .get(b)
                    .into_iter()
                    .flat_map(|liveness| liveness.live_out().iter())
                    .filter(|live_out| {
                        !matches!(
                            liveness.get(b)
                                .map(Liveness::live_in),
                            Some(live_in)
                                if live_in.contains(live_out)
                        ) && !matches!(
                            def_use.get(b)
                                .map(DefUse::definitions),
                            Some(definitions)
                                if definitions.contains(live_out)
                        ) && !uses.iter().any(|used| used != *live_out)
                    })
                    .copied()
                    .collect::<Vec<_>>();

                let Some(live_in) = liveness.get_mut(b).map(|liveness| &mut liveness.live_in)
                else {
                    panic!("there is a set for each block");
                };

                changed |= !uses.is_empty() || !outs.is_empty();

                live_in.extend(uses);
                live_in.extend(outs);
            }
        }

        liveness
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
                            Instruction::FnLabel { .. }
                            | Instruction::Panic
                            | Instruction::NoOp => {}
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
                            Instruction::FnLabel { .. }
                            | Instruction::Panic
                            | Instruction::NoOp => {}
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
                    Instruction::FnLabel { .. } | Instruction::Panic | Instruction::NoOp => {}
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
        for b in (0..(self.blocks.len()))
            .filter_map(|index| u32::try_from(index).ok())
            .map(BlockIndex)
        {
            if !blocks_used.contains(&b)
                && let Some(block) = self.get_block_mut(b)
            {
                block.instructions.clear();

                block.terminator = BlockTerminator::Return(Value::Never);
            }
        }
    }

    fn find_addresses(&mut self) -> Vec<(Address, Address)> {
        let mut addresses = vec![];

        self.for_live_blocks(|ssa, block_index| {
            if let Some(block) = ssa.get_block_mut(block_index) {
                for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                    match instruction {
                        Instruction::FnLabel { .. }
                        | Instruction::Panic
                        | Instruction::NoOp
                        | Instruction::Push(_)
                        | Instruction::AccessAssign { .. } => {}
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
                        Instruction::FnLabel { .. } | Instruction::Panic | Instruction::NoOp => {}
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
