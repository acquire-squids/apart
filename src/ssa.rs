use crate::basic_blocks::{
    Address, BasicBlocks, BlockIndex, BlockTerminator as BasicBlockTerminator, Instruction, Value,
};

use std::{collections::HashSet, fmt};

pub fn convert(basic_blocks: &BasicBlocks) -> Ssa {
    let mut ssa = Ssa {
        blocks: vec![],
        function_count: basic_blocks.function_count(),
    };

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
    function_count: usize,
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
    pub const fn function_count(&self) -> usize {
        self.function_count
    }

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
    pub fn for_children<F>(&self, block_index: BlockIndex, mut f: F)
    where
        F: FnMut(&Self, BlockIndex),
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

#[derive(Debug, Clone, PartialEq)]
pub struct JumpTo {
    block_index: BlockIndex,
    arguments: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    Address(Value),
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
        let mut living = vec![];

        let mut addresses = vec![];

        for b in (0..(self.blocks.len())).rev() {
            addresses.append(&mut self.liveliness_recursive(BlockIndex(b), &mut living));
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
                    arguments.push(Argument::Address(Value::Address(*address)));
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
    ) -> Vec<(Address, Address)> {
        let mut addresses = vec![];

        if let Some(block) = self.get_block_mut(block_index) {
            Self::accumulate_live_values(block_index, block, living, &mut addresses);

            let mut parameters: Vec<Address> = vec![];

            for live_from_elsewhere in &*living {
                let live_from_elsewhere = *live_from_elsewhere;

                if !parameters.iter().any(|parameter| {
                    parameter.block_index == live_from_elsewhere.block_index
                        && parameter.offset == live_from_elsewhere.offset
                }) && live_from_elsewhere.block_index != block_index
                    && self.parameter_in_use(block_index, &live_from_elsewhere, &mut HashSet::new())
                {
                    parameters.push(live_from_elsewhere);
                }
            }

            if let Some(block) = self.get_block_mut(block_index) {
                let parameter_count = parameters.len();

                block.parameters = parameters;

                Self::values_to_arguments(
                    parameter_count,
                    block,
                    living.as_slice(),
                    addresses.as_slice(),
                );
            }
        }

        addresses
    }

    fn accumulate_live_values(
        block_index: BlockIndex,
        block: &mut Block,
        living: &mut Vec<Address>,
        addresses: &mut Vec<(Address, Address)>,
    ) {
        for (i, instruction) in block.instructions_mut().iter_mut().enumerate().rev() {
            match instruction {
                Instruction::NoOp | Instruction::Pop => {}
                Instruction::Unary { operand: value, .. }
                | Instruction::Push(value)
                | Instruction::Call { callee: value, .. } => {
                    Self::accumulate_live_value(block_index, living, value);
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    Self::accumulate_live_value(block_index, living, lhs);
                    Self::accumulate_live_value(block_index, living, rhs);
                }
                Instruction::Assign { value, to } => {
                    Self::accumulate_live_value(block_index, living, value);

                    let Value::Address(to) = to else {
                        unreachable!("assignments are only to addresses");
                    };

                    let new_address = Address {
                        block_index,
                        offset: i,
                        version: 0,
                    };

                    addresses.push((*to, new_address));

                    *to = new_address;

                    if to.block_index == block_index {
                        living.retain(|address| {
                            address.block_index != to.block_index || address.offset != to.offset
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

    fn parameter_in_use(
        &self,
        block_index: BlockIndex,
        parameter: &Address,
        seen: &mut HashSet<BlockIndex>,
    ) -> bool {
        if seen.insert(block_index)
            && let Some(block) = self.get_block(block_index)
        {
            match &block.terminator {
                BlockTerminator::Jump(_) => {}
                BlockTerminator::Branch {
                    condition: value, ..
                }
                | BlockTerminator::Return(value) => {
                    if let Value::Address(address) = value
                        && address.block_index == parameter.block_index
                        && address.offset == parameter.offset
                    {
                        return true;
                    }
                }
            }

            if block.parameters.iter().any(|block_parameter| {
                block_parameter.block_index == parameter.block_index
                    && block_parameter.offset == parameter.offset
            }) {
                return true;
            }

            for instruction in &block.instructions {
                match instruction {
                    Instruction::NoOp | Instruction::Pop => {}
                    Instruction::Unary { operand: value, .. }
                    | Instruction::Assign { value, .. }
                    | Instruction::Push(value)
                    | Instruction::Call { callee: value, .. } => {
                        if let Value::Address(address) = value
                            && address.block_index == parameter.block_index
                            && address.offset == parameter.offset
                        {
                            return true;
                        }
                    }
                    Instruction::Binary { lhs, rhs, .. } => {
                        if let Value::Address(address) = lhs
                            && address.block_index == parameter.block_index
                            && address.offset == parameter.offset
                        {
                            return true;
                        }

                        if let Value::Address(address) = rhs
                            && address.block_index == parameter.block_index
                            && address.offset == parameter.offset
                        {
                            return true;
                        }
                    }
                }
            }

            let mut in_use = false;

            self.for_children(block_index, |ssa, block_index| {
                if !in_use {
                    in_use = ssa.parameter_in_use(block_index, parameter, seen);
                }
            });

            in_use
        } else {
            false
        }
    }

    fn values_to_arguments(
        parameter_count: usize,
        block: &mut Block,
        living: &[Address],
        addresses: &[(Address, Address)],
    ) {
        for instruction in block.instructions_mut().iter_mut().rev() {
            match instruction {
                Instruction::NoOp | Instruction::Pop => {}
                Instruction::Unary { operand: value, .. }
                | Instruction::Push(value)
                | Instruction::Call { callee: value, .. }
                | Instruction::Assign { value, .. } => {
                    Self::value_to_argument(parameter_count, living, addresses, value);
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    Self::value_to_argument(parameter_count, living, addresses, lhs);
                    Self::value_to_argument(parameter_count, living, addresses, rhs);
                }
            }
        }

        match &mut block.terminator {
            BlockTerminator::Jump(_) => {}
            BlockTerminator::Branch {
                condition: value, ..
            }
            | BlockTerminator::Return(value) => {
                Self::value_to_argument(parameter_count, living, addresses, value);
            }
        }
    }

    fn value_to_argument(
        parameter_count: usize,
        living: &[Address],
        addresses: &[(Address, Address)],
        value: &mut Value,
    ) {
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
                *i += parameter_count;
            }
            _ => {}
        }
    }
}
