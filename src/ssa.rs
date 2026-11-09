use crate::{
    ApartOptions,
    basic_blocks::{
        Address, BasicBlocks, BlockIndex, BlockTerminator as BasicBlockTerminator, Instruction,
        Value,
    },
    optimize::eliminate_dead_code,
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

    let mut changed = true;

    while changed {
        changed = eliminate_dead_code::optimize(&mut ssa, false);
    }

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
        self.blocks.get(usize::from(block_index))
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_block_mut(&mut self, block_index: BlockIndex) -> Option<&mut Block> {
        self.blocks.get_mut(usize::from(block_index))
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
    definitions: Vec<HashSet<Address>>,
    uses: Vec<HashSet<Address>>,
}

impl DefUse {
    #[allow(dead_code)]
    #[must_use]
    pub const fn definitions(&self) -> &[HashSet<Address>] {
        self.definitions.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn uses(&self) -> &[HashSet<Address>] {
        self.uses.as_slice()
    }
}

#[derive(Debug)]
pub struct Liveness {
    live_in: Vec<HashSet<Address>>,
    live_out: Vec<HashSet<Address>>,
}

impl Liveness {
    #[allow(dead_code)]
    #[must_use]
    pub const fn live_in(&self) -> &[HashSet<Address>] {
        self.live_in.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn live_out(&self) -> &[HashSet<Address>] {
        self.live_out.as_slice()
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
        let index = usize::from(block_index);

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

    fn use_addresses(def_use: &mut [DefUse], (b, i): (usize, usize), value: &Value) {
        match value {
            Value::Address(address) => {
                if let Some(def_use) = def_use
                    .get_mut(b)
                    .and_then(|def_use| def_use.uses.get_mut(i))
                    && !def_use.contains(address)
                {
                    def_use.insert(*address);
                }
            }
            Value::Compound(values) | Value::TaggedCompound { fields: values, .. } => {
                for value in values {
                    Self::use_addresses(def_use, (b, i), value);
                }
            }
            Value::Never
            | Value::U8(_)
            | Value::I8(_)
            | Value::U16(_)
            | Value::I16(_)
            | Value::U32(_)
            | Value::I32(_)
            | Value::U64(_)
            | Value::I64(_)
            | Value::F32(_)
            | Value::F64(_)
            | Value::Boolean(_)
            | Value::Unit
            | Value::String(_)
            | Value::Fn(_)
            | Value::NativeFn { .. }
            | Value::CallArgument(_)
            | Value::Register(_) => {}
        }
    }

    #[allow(dead_code, clippy::too_many_lines)]
    #[must_use]
    pub fn def_use_analysis(&self) -> Vec<DefUse> {
        let mut def_use = vec![];

        def_use.resize_with(self.blocks().len(), || DefUse {
            definitions: vec![],
            uses: vec![],
        });

        for (b, block) in self.blocks.iter().enumerate().rev() {
            def_use[b]
                .definitions
                .resize_with(block.instructions.len() + 1, HashSet::new);

            def_use[b]
                .uses
                .resize_with(block.instructions.len() + 1, HashSet::new);

            for (i, instruction) in block.instructions().iter().enumerate() {
                match instruction {
                    Instruction::Panic | Instruction::FnLabel { .. } | Instruction::NoOp => {}
                    Instruction::Push(value) => {
                        Self::use_addresses(def_use.as_mut_slice(), (b, i), value);
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
                            && usize::from(address.block_index) == b
                            && let Some(def_use) = def_use
                                .get_mut(b)
                                .and_then(|def_use| def_use.definitions.get_mut(i))
                            && !def_use.contains(address)
                        {
                            def_use.insert(*address);
                        }

                        Self::use_addresses(def_use.as_mut_slice(), (b, i), value);
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        if let Value::Address(address) = to
                            && usize::from(address.block_index) == b
                            && let Some(def_use) = def_use
                                .get_mut(b)
                                .and_then(|def_use| def_use.definitions.get_mut(i))
                            && !def_use.contains(address)
                        {
                            def_use.insert(*address);
                        }

                        for value in [lhs, rhs] {
                            Self::use_addresses(def_use.as_mut_slice(), (b, i), value);
                        }
                    }
                    Instruction::AccessAssign { of, value, .. } => {
                        for value in [of, value] {
                            Self::use_addresses(def_use.as_mut_slice(), (b, i), value);
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
                    Self::use_addresses(
                        def_use.as_mut_slice(),
                        (b, block.instructions().len()),
                        value,
                    );
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
            live_in: vec![],
            live_out: vec![],
        });

        let mut changed = true;

        while changed {
            changed = false;

            for (b, block) in self.blocks().iter().enumerate().rev() {
                liveness[b]
                    .live_in
                    .resize_with(block.instructions.len() + 1, HashSet::new);

                liveness[b]
                    .live_out
                    .resize_with(block.instructions.len() + 1, HashSet::new);

                match block.terminator() {
                    BlockTerminator::Return(_) => {}
                    BlockTerminator::Jump(destination) => {
                        let live =
                            liveness
                                .get(usize::from(*destination))
                                .and_then(|liveness| liveness.live_in.first())
                                .into_iter()
                                .flatten()
                                .filter(|live_in| {
                                    !matches!(liveness
                                        .get(b).and_then(|liveness| liveness.live_out.get(block.instructions.len())), Some(live_out) if live_out.contains(live_in))
                                })
                                .copied()
                                .collect::<Vec<_>>();

                        let Some(live_out) = liveness.get_mut(b).and_then(|liveness| {
                            liveness.live_out.get_mut(block.instructions.len())
                        }) else {
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
                            let live =
                            liveness
                                .get(usize::from(*destination))
                                .and_then(|liveness| liveness.live_in.first())
                                .into_iter()
                                .flatten()
                                .filter(|live_in| {
                                    !matches!(liveness
                                        .get(b).and_then(|liveness| liveness.live_out.get(block.instructions.len())), Some(live_out) if live_out.contains(live_in))
                                })
                                .copied()
                                .collect::<Vec<_>>();

                            let Some(live_out) = liveness.get_mut(b).and_then(|liveness| {
                                liveness.live_out.get_mut(block.instructions.len())
                            }) else {
                                panic!("there is a set for each block");
                            };

                            changed |= !live.is_empty();

                            live_out.extend(live);
                        }
                    }
                }

                for i in (0..=(block.instructions.len())).rev() {
                    let live =
                        liveness
                            .get(b)
                            .and_then(|liveness| liveness.live_in.get(i + 1))
                            .into_iter()
                            .flatten()
                            .filter(|live_in| {
                                !matches!(liveness
                                    .get(b).and_then(|liveness| liveness.live_out.get(i)), Some(live_out) if live_out.contains(live_in))
                            })
                            .copied()
                            .collect::<Vec<_>>();

                    let Some(live_out) = liveness
                        .get_mut(b)
                        .and_then(|liveness| liveness.live_out.get_mut(i))
                    else {
                        panic!("there is a set for each block");
                    };

                    changed |= !live.is_empty();

                    live_out.extend(live);

                    let uses = def_use
                        .get(b)
                        .and_then(|def_use| def_use.uses().get(i))
                        .into_iter()
                        .flatten()
                        .filter(|used| {
                            !matches!(
                                liveness
                                    .get(b)
                                    .and_then(|liveness| liveness.live_in.get(i)),
                                Some(live_in)
                                    if live_in.contains(used)
                            )
                        })
                        .copied()
                        .collect::<Vec<_>>();

                    let outs = liveness
                        .get(b)
                        .and_then(|liveness| liveness.live_out().get(i))
                        .into_iter()
                        .flatten()
                        .filter(|live_out| {
                            !matches!(
                                liveness.get(b)
                                    .and_then(|liveness| liveness.live_in.get(i)),
                                Some(live_in)
                                    if live_in.contains(live_out)
                            ) && !matches!(
                                def_use.get(b)
                                    .and_then(|def_use| def_use.definitions.get(i)),
                                Some(definitions)
                                    if definitions.contains(live_out)
                            )
                        })
                        .copied()
                        .collect::<Vec<_>>();

                    let Some(live_in) = liveness
                        .get_mut(b)
                        .and_then(|liveness| liveness.live_in.get_mut(i))
                    else {
                        panic!("there is a set for each block");
                    };

                    changed |= !uses.is_empty() || !outs.is_empty();

                    live_in.extend(uses);
                    live_in.extend(outs);
                }
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
