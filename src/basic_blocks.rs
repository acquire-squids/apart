use crate::{
    Span, Spanned,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, UnaryOp},
    type_check::{Type, TypeChecker},
};

use std::{collections::HashMap, fmt};

pub fn translate(ast: &Ast, names: &HashMap<Span, Span>, types: &TypeChecker) -> BasicBlocks {
    let mut translator = Translator::new();

    if let Some(root) = ast.roots().iter().find(|root| {
        if let Some(Item::Fn { name, .. }) = ast.get_item(**root).map(Spanned::kind)
            && name.kind() == "main"
        {
            true
        } else {
            false
        }
    }) {
        translator.label_function(ast, *root);
    }

    for root in ast.roots() {
        if let Some(Item::Fn { name, .. } | Item::NativeFn { name, .. }) =
            ast.get_item(*root).map(Spanned::kind)
            && name.kind() != "main"
        {
            translator.label_function(ast, *root);
        }
    }

    let function_count = translator.blocks.len();

    for root in ast.roots() {
        if let Some(Item::Fn {
            name,
            parameters,
            body,
            ..
        }) = ast.get_item(*root).map(Spanned::kind)
            && let Some(block_index) = translator
                .addresses
                .get(&name.span())
                .and_then(|address| {
                    if let Addresslike::Block(block_index) = address {
                        Some(block_index)
                    } else {
                        None
                    }
                })
                .copied()
        {
            translator.switch_to_block(block_index);

            for (p, parameter) in parameters.iter().enumerate() {
                translator.push_instruction(Instruction::Pop);

                translator
                    .addresses
                    .insert(parameter.name().span(), Addresslike::CallArgument(p));
            }

            let last_in_fn = translator.last_in_fn;

            translator.last_in_fn = true;

            translator.translate_expr(ast, names, types, *body);

            translator.last_in_fn = last_in_fn;

            assert_eq!(translator.values.as_slice(), &[]);
        }
    }

    for (b, block) in translator.blocks.iter_mut().enumerate() {
        if block.terminator.is_none() {
            block.terminator = Some(BlockTerminator::Jump(BlockIndex(b + 1)));
        }
    }

    BasicBlocks {
        blocks: translator.blocks,
        function_count,
    }
}

pub struct BasicBlocks {
    blocks: Vec<Block>,
    function_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockIndex(pub usize);

impl From<BlockIndex> for usize {
    fn from(value: BlockIndex) -> Self {
        value.0
    }
}

pub struct Block {
    instructions: Vec<Instruction>,
    terminator: Option<BlockTerminator>,
}

impl fmt::Display for BasicBlocks {
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
    pub const fn terminator(&self) -> &BlockTerminator {
        self.terminator
            .as_ref()
            .expect("block terminators should exist if the blocks do")
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn terminator_mut(&mut self) -> &mut BlockTerminator {
        self.terminator
            .as_mut()
            .expect("block terminators should exist if the blocks do")
    }
}

impl BasicBlocks {
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
            BlockTerminator::Jump(b) => {
                f(self, *b);
            }
            BlockTerminator::Branch {
                when_true,
                otherwise,
                ..
            } => {
                f(self, *when_true);
                f(self, *otherwise);
            }
            BlockTerminator::Return(_) => {}
        }
    }
}

struct Translator {
    blocks: Vec<Block>,
    current_block: Option<BlockIndex>,
    values: Vec<Value>,
    addresses: HashMap<Span, Addresslike>,
    last_in_fn: bool,
}

impl Translator {
    fn new() -> Self {
        Self {
            blocks: vec![],
            current_block: None,
            values: vec![],
            addresses: HashMap::new(),
            last_in_fn: false,
        }
    }

    const fn switch_to_block(&mut self, block_index: BlockIndex) {
        self.current_block = Some(block_index);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Addresslike {
    Address(Address),
    Block(BlockIndex),
    CallArgument(usize),
    NativeFn(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Unit,
    Fn(BlockIndex),
    Address(Address),
    NativeFn(Span),
    Runtime,
    BlockArgument(usize),
    CallArgument(usize),
    Register(usize),
    Compound(Vec<Self>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    #[allow(dead_code)]
    NoOp,
    Unary {
        op: UnaryOp,
        operand: Value,
        temporary: Value,
    },
    Binary {
        op: BinaryOp,
        lhs: Value,
        rhs: Value,
        temporary: Value,
    },
    Assign {
        value: Value,
        to: Value,
    },
    Push(Value),
    Pop,
    Call {
        callee: Value,
        temporary: Value,
    },
    Access {
        index: usize,
        of: Value,
        temporary: Value,
    },
    AccessAssign {
        index: usize,
        of: Value,
        value: Value,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockTerminator {
    Jump(BlockIndex),
    Branch {
        condition: Value,
        when_true: BlockIndex,
        otherwise: BlockIndex,
    },
    Return(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    pub block_index: BlockIndex,
    pub offset: usize,
    pub version: u64,
}

impl Translator {
    fn label_function(&mut self, ast: &Ast, f: ItemIndex) {
        match ast.get_item(f).map(Spanned::kind) {
            None => {
                unreachable!("all roots should exist");
            }
            Some(Item::Fn { name, .. }) => {
                self.addresses.insert(
                    name.span(),
                    Addresslike::Block(BlockIndex(self.blocks.len())),
                );

                self.next_block();
            }
            Some(Item::Primitive(_) | Item::Product { .. }) => {}
            Some(Item::NativeFn { name, .. }) => {
                self.addresses
                    .insert(name.span(), Addresslike::NativeFn(name.span()));
            }
        }
    }

    fn next_block(&mut self) -> BlockIndex {
        let block_index = BlockIndex(self.blocks.len());

        self.blocks.push(Block {
            instructions: vec![],
            terminator: None,
        });

        self.switch_to_block(block_index);

        block_index
    }

    fn push_instruction(&mut self, instruction: Instruction) {
        let block = self
            .current_block
            .and_then(|block_index| self.blocks.get_mut(usize::from(block_index)))
            .expect("instructions only get pushed within blocks");

        block.instructions.push(instruction);
    }

    fn instructions_len(&self) -> usize {
        self.current_block
            .and_then(|block_index| self.blocks.get(usize::from(block_index)))
            .expect("instructions only get checked within blocks")
            .instructions
            .len()
    }
}

impl Translator {
    #[allow(clippy::too_many_lines)]
    fn translate_expr(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        expr: ExprIndex,
    ) {
        match ast.get_expr(expr).map(Spanned::kind) {
            None | Some(Expr::BinaryNoLhs { .. } | Expr::CallNoCallee(_) | Expr::AsUnitNoValue) => {
                unreachable!("the ast should be valid since we succeeded in parsing");
            }
            Some(Expr::Integer(value)) => {
                self.values.push(Value::Integer(*value));

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Some(Expr::Float(value)) => {
                self.values.push(Value::Float(*value));

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Some(Expr::Boolean(value)) => {
                self.values.push(Value::Boolean(*value));

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Some(Expr::Unit) => {
                self.values.push(Value::Unit);

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Some(Expr::Name(_)) => {
                self.translate_name(ast, names, expr);
            }
            Some(Expr::Unary { op, .. }) => {
                self.translate_unary(ast, names, types, expr, *op);
            }
            Some(Expr::Block(exprs)) if exprs.is_empty() => {
                self.values.push(Value::Unit);

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Some(Expr::Block(exprs)) => {
                self.translate_block(ast, names, types, exprs.as_slice());
            }
            Some(Expr::Group(_)) => {
                ast.for_children_exprs(expr, |ast, expr| {
                    self.translate_expr(ast, names, types, expr);
                });
            }
            Some(Expr::Let { name, value, .. }) => {
                self.translate_assign_name(ast, names, types, (name.span(), *value));
            }
            Some(Expr::Binary {
                op: BinaryOp::Assign,
                lhs,
                rhs,
            }) => {
                self.translate_assign(ast, names, types, (*lhs, *rhs));
            }
            Some(Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            }) => {
                self.translate_access(ast, names, types, (*lhs, *rhs));
            }
            Some(Expr::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
            }) => {
                self.translate_and(ast, names, types, (*lhs, *rhs));
            }
            Some(Expr::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
            }) => {
                self.translate_or(ast, names, types, (*lhs, *rhs));
            }
            Some(Expr::Binary { op, .. }) => {
                self.translate_binary(ast, names, types, expr, *op);
            }
            Some(Expr::If {
                condition,
                when_true,
                otherwise,
            }) => {
                self.translate_if(ast, names, types, (*condition, *when_true, *otherwise));
            }
            Some(Expr::Call { callee, arguments }) => {
                self.translate_call(ast, names, types, (*callee, arguments.as_slice()));
            }
            Some(Expr::While {
                condition,
                when_true,
            }) => {
                self.translate_while(ast, names, types, (*condition, *when_true));
            }
            Some(Expr::Return(value)) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                self.translate_expr(ast, names, types, *value);

                self.last_in_fn = last_in_fn;

                self.emit_return();

                self.values.push(Value::Unit);
            }
            Some(Expr::AsUnit(value)) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                self.translate_expr(ast, names, types, *value);

                self.values.pop();

                self.last_in_fn = last_in_fn;

                self.values.push(Value::Unit);

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Some(Expr::Product { fields, .. }) => {
                self.translate_product(ast, names, types, expr, fields.as_slice());
            }
        }
    }

    fn emit_return(&mut self) {
        let value = self
            .values
            .pop()
            .expect("there should always be a return value if parsing succeeded");

        let current_block = self
            .current_block
            .expect("there will be a block by this point");

        if let Some(block) = self.blocks.get_mut(usize::from(current_block))
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Return(value));
        }
    }

    fn translate_name(&mut self, ast: &Ast, names: &HashMap<Span, Span>, expr: ExprIndex) {
        let name = names
            .get(
                &ast.get_expr(expr)
                    .map(Spanned::span)
                    .expect("`translate_name` is only called from `translate`, which only uses existing expressions"),
            )
            .and_then(|name| self.addresses.get(name))
            .expect("the name should be valid since we succeeded in name resolution");

        match name {
            Addresslike::Block(block_index) => {
                self.values.push(Value::Fn(*block_index));
            }
            Addresslike::Address(address) => {
                self.values.push(Value::Address(*address));
            }
            Addresslike::CallArgument(offset) => {
                self.values.push(Value::CallArgument(*offset));
            }
            Addresslike::NativeFn(span) => {
                self.values.push(Value::NativeFn(*span));
            }
        }

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_block(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        exprs: &[ExprIndex],
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        for expr in exprs.iter().rev().skip(1).rev() {
            self.translate_expr(ast, names, types, *expr);

            self.values
                .pop()
                .expect("every expression produces a value");
        }

        self.last_in_fn = last_in_fn;

        self.translate_expr(
            ast,
            names,
            types,
            *exprs
                .last()
                .expect("`translate_block` in only called on blocks that aren't empty"),
        );
    }

    fn translate_unary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        expr: ExprIndex,
        op: UnaryOp,
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        ast.for_children_exprs(expr, |ast, expr| {
            self.translate_expr(ast, names, types, expr);
        });

        let operand = self
            .values
            .pop()
            .expect("all unary operands should produce a value");

        let address = Address {
            block_index: self
                .current_block
                .expect("unary expressions only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Unary {
            op,
            operand,
            temporary: Value::Address(address),
        });

        self.values.push(Value::Address(address));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_binary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        expr: ExprIndex,
        op: BinaryOp,
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        ast.for_children_exprs(expr, |ast, expr| {
            self.translate_expr(ast, names, types, expr);
        });

        let rhs = self
            .values
            .pop()
            .expect("all binary right operands should produce a value");

        let lhs = self
            .values
            .pop()
            .expect("all binary left operands should produce a value");

        let address = Address {
            block_index: self
                .current_block
                .expect("binary expressions only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Binary {
            op,
            lhs,
            rhs,
            temporary: Value::Address(address),
        });

        self.values.push(Value::Address(address));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_assign(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let value = rhs;

        match ast.get_expr(lhs).map(Spanned::kind) {
            Some(Expr::Name(_)) => {
                let lhs_span = ast
                    .get_expr(lhs)
                    .expect("`translate_assign` is only called on existing assignments from `translate`")
                    .span();

                self.translate_assign_name(ast, names, types, (lhs_span, value));
            }
            Some(Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            }) => {
                self.translate_assign_access(ast, names, types, (*lhs, *rhs, value));
            }
            _ => {
                unreachable!("for now, only names and accesses can be assigned to");
            }
        }
    }

    fn translate_assign_name(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (span, value): (Span, ExprIndex),
    ) {
        self.translate_expr(ast, names, types, value);

        let value = self
            .values
            .pop()
            .expect("assignments can only occur with a value");

        let address = Address {
            block_index: self
                .current_block
                .expect("assignments only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        let address = names
            .get(&span)
            .and_then(|name_span| self.addresses.get_mut(name_span))
            .and_then(|address| {
                if let Addresslike::Address(address) = address {
                    address.version += 1;

                    Some(*address)
                } else {
                    None
                }
            })
            .unwrap_or(address);

        self.push_instruction(Instruction::Assign {
            value,
            to: Value::Address(address),
        });

        self.addresses.insert(span, Addresslike::Address(address));

        self.values.push(Value::Address(address));
    }

    fn translate_and(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, lhs);

        let lhs = self
            .values
            .pop()
            .expect("every expression produces a value");

        let address = Address {
            block_index: self
                .current_block
                .expect("binary expressions only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Assign {
            value: lhs,
            to: Value::Address(address),
        });

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = BlockIndex(self.blocks.len());

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition: Value::Address(address),
                when_true: when_true_block,
                otherwise: BlockIndex(usize::MAX),
            });
        }

        self.next_block();

        self.translate_expr(ast, names, types, rhs);

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let rhs = self
            .values
            .pop()
            .expect("every expression produces a value");

        self.push_instruction(Instruction::Assign {
            value: rhs,
            to: Value::Address(address),
        });

        let after_all = BlockIndex(self.blocks.len());

        if let Some(block) = self
            .current_block
            .and_then(|block_index| self.blocks.get_mut(usize::from(block_index)))
        {
            block.terminator = Some(BlockTerminator::Jump(after_all));
        }

        if let Some(BlockTerminator::Branch { otherwise, .. }) = self
            .blocks
            .get_mut(usize::from(current_block))
            .and_then(|block| block.terminator.as_mut())
        {
            *otherwise = after_all;
        }

        self.last_in_fn = last_in_fn;

        self.next_block();

        self.values.push(Value::Address(Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        }));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_or(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, lhs);

        let lhs = self
            .values
            .pop()
            .expect("every expression produces a value");

        let address = Address {
            block_index: self
                .current_block
                .expect("binary expressions only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Assign {
            value: lhs,
            to: Value::Address(address),
        });

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let otherwise_block = BlockIndex(self.blocks.len());

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition: Value::Address(address),
                when_true: BlockIndex(usize::MAX),
                otherwise: otherwise_block,
            });
        }

        self.next_block();

        self.translate_expr(ast, names, types, rhs);

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let rhs = self
            .values
            .pop()
            .expect("every expression produces a value");

        self.push_instruction(Instruction::Assign {
            value: rhs,
            to: Value::Address(address),
        });

        let after_all = BlockIndex(self.blocks.len());

        if let Some(block) = self
            .current_block
            .and_then(|block_index| self.blocks.get_mut(usize::from(block_index)))
        {
            block.terminator = Some(BlockTerminator::Jump(after_all));
        }

        if let Some(BlockTerminator::Branch { when_true, .. }) = self
            .blocks
            .get_mut(usize::from(current_block))
            .and_then(|block| block.terminator.as_mut())
        {
            *when_true = after_all;
        }

        self.last_in_fn = last_in_fn;

        self.next_block();

        self.values.push(Value::Address(Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        }));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_if(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (condition, when_true, otherwise): (ExprIndex, ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, condition);

        let condition = self
            .values
            .pop()
            .expect("every expression produces a value");

        let address = Address {
            block_index: self
                .current_block
                .expect("binary expressions only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Assign {
            value: Value::Runtime,
            to: Value::Address(address),
        });

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = BlockIndex(self.blocks.len());

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition,
                when_true: when_true_block,
                otherwise: BlockIndex(usize::MAX),
            });
        }

        self.next_block();

        self.translate_expr(ast, names, types, when_true);

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let value = self.values.pop().expect("all expressions produce a value");

        self.push_instruction(Instruction::Assign {
            value,
            to: Value::Address(address),
        });

        let when_true_block = self
            .current_block
            .expect("if expressions can only exist in a block");

        let otherwise_block = self.next_block();

        if let BlockTerminator::Branch { otherwise, .. } = self
            .blocks
            .get_mut(usize::from(current_block))
            .and_then(|block| block.terminator.as_mut())
            .expect("the destination to backpatch was set just before")
        {
            *otherwise = otherwise_block;
        }

        self.translate_expr(ast, names, types, otherwise);

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let value = self.values.pop().expect("all expressions produce a value");

        self.push_instruction(Instruction::Assign {
            value,
            to: Value::Address(address),
        });

        let after_all = self.next_block();

        if let Some(block) = self.blocks.get_mut(usize::from(when_true_block))
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Jump(after_all));
        }

        self.last_in_fn = last_in_fn;

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        self.values.push(Value::Address(address));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_call(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (callee, arguments): (ExprIndex, &[ExprIndex]),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, callee);

        let callee = self
            .values
            .pop()
            .expect("parsing ensures a call expression has a callee");

        let arguments = arguments
            .iter()
            .copied()
            .map(|argument| {
                self.translate_expr(ast, names, types, argument);

                self.values
                    .pop()
                    .expect("each call argument should produce a value")
            })
            .collect::<Vec<_>>();

        for argument in arguments.into_iter().rev() {
            self.push_instruction(Instruction::Push(argument));
        }

        let address = Address {
            block_index: self.current_block.expect("calls only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Call {
            callee,
            temporary: Value::Address(address),
        });

        self.values.push(Value::Address(address));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_while(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (condition, when_true): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let condition_block = self.next_block();

        self.translate_expr(ast, names, types, condition);

        let condition = self
            .values
            .pop()
            .expect("if expressions are enforced to have conditions by an earlier stage");

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let then_branch = BlockIndex(self.blocks.len());

        if let Some(block) = self.blocks.get_mut(usize::from(current_block))
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Branch {
                condition,
                when_true: then_branch,
                otherwise: BlockIndex(usize::MAX),
            });
        }

        self.next_block();

        self.translate_expr(ast, names, types, when_true);

        if let Some(block) = self.blocks.get_mut(usize::from(then_branch))
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Jump(condition_block));
        }

        let after_all = self.next_block();

        if let BlockTerminator::Branch { otherwise, .. } = self
            .blocks
            .get_mut(usize::from(current_block))
            .and_then(|block| block.terminator.as_mut())
            .expect("the destination to backpatch was set just before")
        {
            *otherwise = after_all;
        }

        self.values.pop();

        self.values.push(Value::Unit);

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_product(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        expr: ExprIndex,
        fields: &[(Spanned<String>, ExprIndex)],
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let expr_span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        let Some(Type::Product {
            fields: type_fields,
            ..
        }) = types
            .get_type_index(expr_span)
            .and_then(|type_index| types.get_type(type_index))
        else {
            unreachable!("type checking guarantees a product is a product");
        };

        let mut values = vec![];

        for (field_name, _) in type_fields {
            let Some((_, value)) = fields.iter().find(|(name, _)| name.kind() == field_name) else {
                unreachable!("type checking guarantees all fields are present and not duplicated");
            };

            self.translate_expr(ast, names, types, *value);

            values.push(self.values.pop().expect("all expressions produce a value"));
        }

        self.values.push(Value::Compound(values));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_access(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let field_index = Self::find_field_index(ast, types, (lhs, rhs));

        self.translate_expr(ast, names, types, lhs);

        let lhs = self
            .values
            .pop()
            .expect("every expression produces a value");

        let address = Address {
            block_index: self.current_block.expect("calls only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::Access {
            index: field_index,
            of: lhs,
            temporary: Value::Address(address),
        });

        self.values.push(Value::Address(address));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn find_field_index(
        ast: &Ast,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) -> usize {
        let lhs_span = ast
            .get_expr(lhs)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        match types
            .get_type_index(lhs_span)
            .and_then(|type_index| types.get_type(type_index))
        {
            Some(Type::Product {
                fields: type_fields,
                ..
            }) => match ast.get_expr(rhs).map(Spanned::kind) {
                Some(Expr::Name(name)) => {
                    let Some(field_index) = type_fields
                        .iter()
                        .position(|(field_name, _)| field_name == name)
                    else {
                        unreachable!("type checking guarantees the field exists on the type");
                    };

                    field_index
                }
                _ => unreachable!("for now, only names can be accessors"),
            },
            _ => unreachable!("for now, only products can be accessees"),
        }
    }

    fn translate_assign_access(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        types: &TypeChecker,
        (lhs, rhs, value): (ExprIndex, ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let field_index = Self::find_field_index(ast, types, (lhs, rhs));

        self.translate_expr(ast, names, types, lhs);

        let lhs = self
            .values
            .pop()
            .expect("every expression produces a value");

        self.translate_expr(ast, names, types, value);

        let value = self
            .values
            .pop()
            .expect("every expression produces a value");

        let address = Address {
            block_index: self.current_block.expect("calls only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        };

        self.push_instruction(Instruction::AccessAssign {
            index: field_index,
            of: lhs,
            value,
        });

        self.values.push(Value::Address(address));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }
}
