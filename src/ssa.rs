use crate::basic_blocks::{
    Address, BasicBlocks, BlockIndex, BlockTerminator as BasicBlockTerminator, Instruction, Value,
};

use std::{collections::HashSet, fmt};

pub fn convert(basic_blocks: &BasicBlocks) -> Ssa {
    let mut ssa = Ssa { blocks: vec![] };

    for basic_block in basic_blocks.blocks() {
        ssa.blocks.push(Block {
            parameters: vec![],
            instructions: basic_block.instructions().to_vec(),
            terminator: match basic_block.terminator() {
                BasicBlockTerminator::Jump(block_index) => BlockTerminator::Jump(JumpTo {
                    block_index: *block_index,
                    arguments: vec![],
                }),
                BasicBlockTerminator::Branch {
                    condition,
                    when_true,
                    otherwise,
                } => BlockTerminator::Branch {
                    condition: condition.clone(),
                    when_true: JumpTo {
                        block_index: *when_true,
                        arguments: vec![],
                    },
                    otherwise: JumpTo {
                        block_index: *otherwise,
                        arguments: vec![],
                    },
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
            writeln!(f, "{b} (arguments: {:?}):", block.parameters().len())?;

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
    pub const fn blocks_mut(&mut self) -> &mut [Block] {
        self.blocks.as_mut_slice()
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
    pub fn for_children_mut<F>(&mut self, block_index: BlockIndex, mut f: F)
    where
        F: FnMut(&mut Self, BlockIndex),
    {
        let Some(block) = self.get_block(block_index) else {
            return;
        };

        match block.terminator() {
            BlockTerminator::Jump(JumpTo { block_index: b, .. }) => {
                f(self, *b);
            }
            BlockTerminator::Branch {
                when_true:
                    JumpTo {
                        block_index: when_true,
                        ..
                    },
                otherwise:
                    JumpTo {
                        block_index: otherwise,
                        ..
                    },
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
    parameters: Vec<Address>,
    instructions: Vec<Instruction>,
    terminator: BlockTerminator,
}

#[derive(Debug)]
pub enum BlockTerminator {
    Jump(JumpTo),
    Branch {
        condition: Value,
        when_true: JumpTo,
        otherwise: JumpTo,
    },
    Return(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpTo {
    block_index: BlockIndex,
    arguments: Vec<Argument>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Argument {
    Address(Address),
    Passthrough(usize),
}

impl JumpTo {
    #[allow(dead_code)]
    #[must_use]
    pub const fn block(&self) -> BlockIndex {
        self.block_index
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn block_mut(&mut self) -> &mut BlockIndex {
        &mut self.block_index
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn arguments(&self) -> &[Argument] {
        self.arguments.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn arguments_mut(&mut self) -> &mut Vec<Argument> {
        &mut self.arguments
    }
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
    pub const fn parameters(&self) -> &[Address] {
        self.parameters.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn parameters_mut(&mut self) -> &mut Vec<Address> {
        &mut self.parameters
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
}

impl Ssa {
    fn liveliness(&mut self) {
        let mut seen = HashSet::new();
        let mut living = vec![];

        let mut addresses = vec![];

        for b in (0..(self.blocks.len())).rev() {
            addresses.append(&mut self.liveliness_recursive(BlockIndex(b), &mut living, &mut seen));
        }

        for b in 0..(self.blocks.len()) {
            if let Some(block) = self.blocks.get(b)
                && let BlockTerminator::Jump(jump_to) = &block.terminator
            {
                let arguments = self.collect_arguments(b, addresses.as_slice(), jump_to);

                if let Some(block) = self.blocks.get_mut(b)
                    && let BlockTerminator::Jump(jump_to) = &mut block.terminator
                {
                    jump_to.arguments = arguments;
                }
            }

            if let Some(block) = self.blocks.get(b)
                && let BlockTerminator::Branch { when_true, .. } = &block.terminator
            {
                let arguments = self.collect_arguments(b, addresses.as_slice(), when_true);

                if let Some(block) = self.blocks.get_mut(b)
                    && let BlockTerminator::Branch { when_true, .. } = &mut block.terminator
                {
                    when_true.arguments = arguments;
                }
            }

            if let Some(block) = self.blocks.get(b)
                && let BlockTerminator::Branch { otherwise, .. } = &block.terminator
            {
                let arguments = self.collect_arguments(b, addresses.as_slice(), otherwise);

                if let Some(block) = self.blocks.get_mut(b)
                    && let BlockTerminator::Branch { otherwise, .. } = &mut block.terminator
                {
                    otherwise.arguments = arguments;
                }
            }
        }
    }

    fn collect_arguments(
        &self,
        b: usize,
        addresses: &[(Address, Address)],
        jump_to: &JumpTo,
    ) -> Vec<Argument> {
        let mut arguments = vec![];

        if let Some(block) = self.blocks.get(b)
            && let Some(successor) = self.get_block(jump_to.block_index)
        {
            for successor_parameter in &successor.parameters {
                if let Some(address) = addresses.iter().find_map(|(old_address, new_address)| {
                    if successor_parameter.block_index == old_address.block_index
                        && successor_parameter.offset == old_address.offset
                        && usize::from(new_address.block_index) == b
                    {
                        Some(new_address)
                    } else {
                        None
                    }
                }) {
                    arguments.push(Argument::Address(*address));
                } else if let Some(i) = block.parameters.iter().position(|parameter| {
                    parameter.block_index == successor_parameter.block_index
                        && parameter.offset == successor_parameter.offset
                }) {
                    arguments.push(Argument::Passthrough(i));
                }
            }
        }

        arguments
    }

    fn liveliness_recursive(
        &mut self,
        block_index: BlockIndex,
        living: &mut Vec<Address>,
        seen: &mut HashSet<BlockIndex>,
    ) -> Vec<(Address, Address)> {
        if seen.insert(block_index) {
            let mut addresses = vec![];

            if let Some(block) = self.get_block_mut(block_index) {
                for (i, instruction) in block.instructions_mut().iter_mut().enumerate().rev() {
                    match instruction {
                        Instruction::NoOp | Instruction::Pop => {}
                        Instruction::Unary { operand: value, .. }
                        | Instruction::Push(value)
                        | Instruction::Call(value) => {
                            Self::accumulate_live_value(block_index, living, value);
                        }
                        Instruction::Binary { lhs, rhs, .. } => {
                            Self::accumulate_live_value(block_index, living, lhs);
                            Self::accumulate_live_value(block_index, living, rhs);
                        }
                        Instruction::Assign { value, to } => {
                            Self::accumulate_live_value(block_index, living, value);

                            let old_address = *to;

                            let new_address = Address {
                                block_index,
                                offset: i,
                                version: 0,
                            };

                            addresses.push((*to, new_address));

                            *to = new_address;

                            if old_address.block_index == block_index {
                                living.retain(|address| {
                                    address.block_index != old_address.block_index
                                        || address.offset != old_address.offset
                                });
                            }
                        }
                    }
                }

                match &mut block.terminator {
                    BlockTerminator::Jump(_) => {}
                    BlockTerminator::Branch {
                        condition: value, ..
                    }
                    | BlockTerminator::Return(value) => {
                        Self::accumulate_live_value(block_index, living, value);
                    }
                }

                for live_from_elsewhere in &*living {
                    let live_from_elsewhere = *live_from_elsewhere;

                    if !block.parameters.iter().any(|parameter| {
                        parameter.block_index == live_from_elsewhere.block_index
                            && parameter.offset == live_from_elsewhere.offset
                    }) && live_from_elsewhere.block_index != block_index
                    {
                        block.parameters.push(live_from_elsewhere);
                    }
                }

                for instruction in block.instructions_mut().iter_mut().rev() {
                    match instruction {
                        Instruction::NoOp | Instruction::Pop => {}
                        Instruction::Unary { operand: value, .. }
                        | Instruction::Push(value)
                        | Instruction::Call(value)
                        | Instruction::Assign { value, .. } => {
                            Self::value_to_argument(living.as_slice(), addresses.as_slice(), value);
                        }
                        Instruction::Binary { lhs, rhs, .. } => {
                            Self::value_to_argument(living.as_slice(), addresses.as_slice(), lhs);
                            Self::value_to_argument(living.as_slice(), addresses.as_slice(), rhs);
                        }
                    }
                }

                match &mut block.terminator {
                    BlockTerminator::Jump(_) => {}
                    BlockTerminator::Branch {
                        condition: value, ..
                    }
                    | BlockTerminator::Return(value) => {
                        Self::value_to_argument(living.as_slice(), addresses.as_slice(), value);
                    }
                }
            }

            addresses
        } else {
            vec![]
        }
    }

    fn accumulate_live_value(block_index: BlockIndex, living: &mut Vec<Address>, value: &Value) {
        match value {
            Value::Address(address)
                if address.block_index != block_index
                    && !living.iter().any(|live_address| {
                        live_address.block_index == address.block_index
                            && live_address.offset == address.offset
                    }) =>
            {
                living.push(*address);
            }
            _ => {}
        }
    }

    fn value_to_argument(living: &[Address], addresses: &[(Address, Address)], value: &mut Value) {
        match value {
            Value::Address(address)
                if let Some(i) = living.iter().position(|live_from_elsewhere| {
                    address.block_index == live_from_elsewhere.block_index
                        && address.offset == live_from_elsewhere.offset
                }) =>
            {
                *value = Value::Argument(i);
            }
            Value::Address(address)
                if let Some(new_address) =
                    addresses.iter().find_map(|(old_address, new_address)| {
                        if address == old_address {
                            Some(*new_address)
                        } else {
                            None
                        }
                    }) =>
            {
                *address = new_address;
            }
            Value::Argument(i) if *i < living.len() => {
                *i += living.len();
            }
            _ => {}
        }
    }
}
