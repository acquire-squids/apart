use crate::{
    ApartOptions,
    basic_blocks::{Address, BlockIndex, Instruction, Value},
    ssa::{BlockTerminator, DefUse, Liveness, Ssa},
};

use std::{collections::HashMap, error, fmt};

pub fn allocate(_: &ApartOptions, ssa: &mut Ssa) -> Result<(), Vec<Error>> {
    let mut errors = vec![];

    let def_use = ssa.def_use_analysis();
    let def_use = def_use.as_slice();

    let liveness = ssa.liveness_analysis(def_use);
    let liveness = liveness.as_slice();

    let graph = interference_graph(ssa, def_use, liveness);

    ssa.for_functions(|ssa, blocks| {
        let mut addresses = HashMap::new();

        let mut allocator = RegisterAllocator {
            registers: 0,
            call_arguments: 0,
            max_registers: 0,
        };

        if let Some(block) = blocks
            .first()
            .and_then(|block_index| ssa.get_block(*block_index))
        {
            allocator.call_arguments = block.call_argument_count();

            for _ in 0..(block.call_argument_count()) {
                allocator.allocate();
            }
        }

        for block_index in blocks.iter().copied() {
            allocator.allocate_block(ssa, &graph, block_index, &mut addresses);

            if let Some(block) = ssa.get_block_mut(block_index) {
                block.instructions_mut().retain(|instruction| {
                    !matches!(
                        instruction,
                        Instruction::NoOp
                            | Instruction::Assign {
                                value: Value::Never,
                                ..
                            }
                    )
                });
            }
        }

        if let Some(block) = blocks
            .first()
            .and_then(|block_index| ssa.get_block_mut(*block_index))
        {
            if allocator.max_registers > usize::from(u16::MAX) {
                errors.push(Error::TooManyRegisters(allocator.max_registers));
            } else {
                *block.max_register_count_mut() = u16::try_from(allocator.max_registers)
                    .expect("verified fine by the if condition");
            }
        }
    });

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    TooManyRegisters(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyRegisters(got) => write!(
                f,
                "a function tried to use more than the maximum amount of {} registers (got {got})",
                u16::MAX
            ),
        }
    }
}

impl error::Error for Error {}

struct RegisterAllocator {
    call_arguments: u8,
    registers: usize,
    max_registers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Allocation {
    Register(usize),
}

#[derive(Debug)]
struct InterferenceGraph {
    nodes: Vec<Node>,
    address_mapping: HashMap<Address, NodeIndex>,
}

impl InterferenceGraph {
    #[allow(dead_code)]
    #[must_use]
    fn get_node(&self, node_index: NodeIndex) -> Option<&Node> {
        self.nodes.get(usize::from(node_index))
    }

    #[allow(dead_code)]
    #[must_use]
    fn get_node_from_address(&self, address: &Address) -> Option<&Node> {
        self.address_mapping
            .get(address)
            .copied()
            .and_then(|node_index| self.get_node(node_index))
    }
}

#[derive(Debug)]
struct Node {
    address: Address,
    conflicts: Vec<NodeIndex>,
}

impl Node {
    const fn conflicts(&self) -> &[NodeIndex] {
        self.conflicts.as_slice()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct NodeIndex(usize);

impl From<NodeIndex> for usize {
    fn from(value: NodeIndex) -> Self {
        value.0
    }
}

#[allow(dead_code)]
#[must_use]
fn interference_graph(ssa: &Ssa, def_use: &[DefUse], liveness: &[Liveness]) -> InterferenceGraph {
    let mut graph = InterferenceGraph {
        nodes: vec![],
        address_mapping: HashMap::new(),
    };

    for b in 0..(ssa.blocks().len()) {
        for a in def_use
            .get(b)
            .into_iter()
            .flat_map(|def_use| def_use.definitions().iter())
            .copied()
        {
            graph
                .address_mapping
                .insert(a, NodeIndex(graph.nodes.len()));

            graph.nodes.push(Node {
                address: a,
                conflicts: vec![],
            });
        }
    }

    for b in 0..(ssa.blocks().len()) {
        for a in def_use
            .get(b)
            .into_iter()
            .flat_map(|def_use| def_use.definitions().iter())
        {
            let conflicts = liveness
                .get(b)
                .into_iter()
                .flat_map(|liveness| liveness.live_out().iter())
                .chain(
                    def_use
                        .get(b)
                        .into_iter()
                        .flat_map(|def_use| def_use.definitions().iter()),
                )
                .chain(
                    def_use
                        .get(b)
                        .into_iter()
                        .flat_map(|def_use| def_use.uses().iter()),
                )
                .filter_map(|address| graph.address_mapping.get(address))
                .copied()
                .collect::<Vec<_>>();

            let Some(node) = graph
                .address_mapping
                .get(a)
                .copied()
                .and_then(|node_index| graph.nodes.get_mut(usize::from(node_index)))
            else {
                panic!("all variables exist");
            };

            node.conflicts = conflicts;
        }
    }

    graph
}

impl RegisterAllocator {
    fn allocate(&mut self) -> Allocation {
        self.registers += 1;

        self.max_registers = self.max_registers.max(self.registers);

        Allocation::Register(self.registers - 1)
    }

    fn assign(
        &mut self,
        graph: &InterferenceGraph,
        to: &mut Value,
        addresses: &mut HashMap<Address, Allocation>,
    ) {
        match to {
            Value::CallArgument(index) => {
                *to = Value::Register(usize::from(*index));
            }
            Value::Address(to_address) => {
                let to_address = *to_address;

                let conflicts = graph
                    .get_node_from_address(&to_address)
                    .into_iter()
                    .flat_map(|node| node.conflicts().iter())
                    .copied()
                    .filter_map(|node_index| graph.get_node(node_index))
                    .map(|node| node.address)
                    .filter_map(|address| addresses.get(&address))
                    .copied()
                    .collect::<Vec<_>>();

                for i in (usize::from(self.call_arguments))..(self.registers) {
                    if !conflicts.contains(&Allocation::Register(i)) {
                        addresses
                            .entry(to_address)
                            .or_insert_with(|| Allocation::Register(i));

                        break;
                    }
                }

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
        graph: &InterferenceGraph,
        block_index: BlockIndex,
        addresses: &mut HashMap<Address, Allocation>,
    ) {
        if let Some(block) = ssa.get_block_mut(block_index) {
            for (i, instruction) in block.instructions_mut().iter_mut().enumerate() {
                match instruction {
                    Instruction::NoOp | Instruction::FnLabel { .. } | Instruction::Panic => {}
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
                        self.registers -= usize::from(*arity);
                        self.max_registers -= usize::from(*arity);

                        self.value_to_allocation(value, (block_index, i), addresses);

                        self.assign(graph, to, addresses);
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

                        self.assign(graph, to, addresses);
                    }
                    Instruction::Binary {
                        lhs,
                        rhs,
                        temporary: to,
                        ..
                    } => {
                        self.value_to_allocation(lhs, (block_index, i), addresses);

                        self.value_to_allocation(rhs, (block_index, i), addresses);

                        self.assign(graph, to, addresses);
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

    #[allow(clippy::only_used_in_recursion, clippy::self_only_used_in_recursion)]
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
                *value = Value::Register(usize::from(*index));
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
