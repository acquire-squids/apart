use crate::{
    Span, Spanned,
    name_resolve::Names,
    parse::{
        Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, MatchCase, PathElement, Pattern, UnaryOp,
    },
    type_check::{Primitive, Type, TypeChecker},
};

use std::{collections::HashMap, fmt, iter};

pub fn translate(ast: &Ast, names: &Names, types: &TypeChecker) -> BasicBlocks {
    let mut translator = Translator::new();

    if let Some(root) = ast.roots().iter().find(|root| {
        if let Item::Fn { name, .. } = ast[**root].kind()
            && name.kind() == "main"
        {
            true
        } else {
            false
        }
    }) {
        translator.label_function(ast, *root);
    }

    translator.label_items(ast, ast.roots(), false);

    let function_count = translator.blocks.len();

    if let Some(root) = ast.roots().iter().find(|root| {
        if let Item::Fn { name, .. } = ast[**root].kind()
            && name.kind() == "main"
        {
            true
        } else {
            false
        }
    }) {
        translator.translate_function(ast, names, types, *root);
    }

    translator.translate_items(ast, names, types, ast.roots(), false);

    for (b, block) in translator.blocks.iter_mut().enumerate() {
        if block.terminator.is_none() {
            block.terminator = Some(BlockTerminator::Jump(BlockIndex(if b < function_count {
                b + function_count + 1
            } else {
                b + 1
            })));
        }
    }

    BasicBlocks {
        blocks: translator.blocks,
    }
}

pub struct BasicBlocks {
    blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockIndex(pub usize);

impl From<BlockIndex> for usize {
    fn from(value: BlockIndex) -> Self {
        value.0
    }
}

pub struct Block {
    call_argument_count: usize,
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
    pub const fn call_argument_count(&self) -> usize {
        self.call_argument_count
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
    CompoundField { index: usize, of: Address },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F64(f64),
    Boolean(bool),
    Unit,
    String(String),
    Fn(BlockIndex),
    Address(Address),
    NativeFn(Span),
    Runtime,
    CallArgument(usize),
    StackOffset(usize),
    Register(usize),
    Compound(Vec<Self>),
    TaggedCompound { fields: Vec<Self>, tag: u16 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
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
    Call {
        callee: Value,
        arity: usize,
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
    GetTag {
        of: Value,
        temporary: Value,
    },
    ScopeStart,
    PopN(usize),
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
        match ast[f].kind() {
            Item::Fn { name, .. } => {
                self.addresses.insert(
                    name.span(),
                    Addresslike::Block(BlockIndex(self.blocks.len())),
                );

                self.next_block();
            }
            Item::Primitive(_) | Item::Product { .. } | Item::Sum { .. } => {}
            Item::NativeFn { name, .. } => {
                self.addresses
                    .insert(name.span(), Addresslike::NativeFn(name.span()));
            }
            Item::Mod { contents, .. } => {
                for item in contents {
                    self.label_function(ast, *item);
                }
            }
            Item::Teach { body, .. } => {
                for item in body {
                    self.label_function(ast, *item);
                }
            }
        }
    }

    fn next_block(&mut self) -> BlockIndex {
        let block_index = BlockIndex(self.blocks.len());

        self.blocks.push(Block {
            call_argument_count: 0,
            instructions: vec![],
            terminator: None,
        });

        self.switch_to_block(block_index);

        block_index
    }

    fn next_address(&self) -> Address {
        Address {
            block_index: self
                .current_block
                .expect("match expressions only exist in blocks"),
            offset: self.instructions_len(),
            version: 0,
        }
    }

    fn assign_if_not_address(&mut self) -> Address {
        let value = self
            .values
            .pop()
            .expect("every expression produces a value");

        if let Value::Address(address) = value {
            address
        } else {
            let address = self.next_address();

            self.push_instruction(Instruction::Assign {
                value,
                to: Value::Address(address),
            });

            address
        }
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

    fn label_items(&mut self, ast: &Ast, items: &[ItemIndex], allow_main: bool) {
        for item in items {
            match ast[*item].kind() {
                Item::Mod { contents, .. } => {
                    self.label_items(ast, contents, true);
                }
                Item::Teach { body, .. } => {
                    self.label_items(ast, body, true);
                }
                Item::Fn { name, .. } | Item::NativeFn { name, .. }
                    if name.kind() != "main" || allow_main =>
                {
                    self.label_function(ast, *item);
                }
                Item::Primitive(_)
                | Item::Product { .. }
                | Item::Sum { .. }
                | Item::Fn { .. }
                | Item::NativeFn { .. } => {}
            }
        }
    }

    fn translate_items(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        items: &[ItemIndex],
        allow_main: bool,
    ) {
        for item in items {
            match ast[*item].kind() {
                Item::Primitive(_)
                | Item::NativeFn { .. }
                | Item::Product { .. }
                | Item::Sum { .. } => {}
                Item::Mod { contents, .. } => {
                    self.translate_items(ast, names, types, contents.as_slice(), true);
                }
                Item::Teach { body, .. } => {
                    self.translate_items(ast, names, types, body.as_slice(), true);
                }
                Item::Fn { name, .. } => {
                    if name.kind() != "main" || allow_main {
                        self.translate_function(ast, names, types, *item);
                    }
                }
            }
        }
    }

    fn translate_function(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        item: ItemIndex,
    ) {
        if let Item::Fn {
            name,
            parameters,
            body,
            ..
        } = ast[item].kind()
            && let Some(block_index) = self
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
            self.switch_to_block(block_index);

            for (p, parameter) in parameters.iter().enumerate() {
                self.addresses
                    .insert(parameter.name().span(), Addresslike::CallArgument(p));
            }

            let after_arguments = self.next_block();

            let Some(block) = self.blocks.get_mut(usize::from(block_index)) else {
                unreachable!("we're guaranteed to have a block by now");
            };

            block.call_argument_count = parameters.len();

            if block.terminator.is_none() {
                block.terminator = Some(BlockTerminator::Jump(after_arguments));
            }

            let block_index = after_arguments;

            self.switch_to_block(block_index);

            let last_in_fn = self.last_in_fn;

            self.last_in_fn = true;

            self.translate_expr(ast, names, types, *body);

            self.last_in_fn = last_in_fn;

            assert_eq!(self.values.as_slice(), &[]);
        }
    }
}

impl Translator {
    #[allow(clippy::too_many_lines)]
    fn translate_expr(&mut self, ast: &Ast, names: &Names, types: &TypeChecker, expr: ExprIndex) {
        match ast[expr].kind() {
            Expr::BinaryNoLhs { .. }
            | Expr::CallNoCallee(_)
            | Expr::AsUnitNoValue
            | Expr::ProductNoName(_) => {
                unreachable!("the ast should be valid since we succeeded in parsing");
            }
            Expr::PathElement(_) => {
                unreachable!(
                    "type checking guarantees these path elements aren't used as expressions"
                );
            }
            Expr::SelfType => {
                unreachable!("type checking guarantees a \"Self\" isn't used as an expression");
            }
            Expr::Integer(value) => {
                self.values.push(
                    if let Type::Primitive(primitive) = &types[types[ast[expr].span()]] {
                        match primitive.kind() {
                            Primitive::U8 => Value::U8(
                                u8::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I8 => Value::I8(
                                i8::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::U16 => Value::U16(
                                u16::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I16 => Value::I16(
                                i16::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::U32 => Value::U32(
                                u32::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I32 => Value::I32(
                                i32::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::U64 => Value::U64(*value),
                            Primitive::I64 => Value::I64(
                                i64::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            _ => unreachable!("type checking guarantees integers are integers"),
                        }
                    } else {
                        unreachable!("type checking guarantees integers are primitives")
                    },
                );

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Expr::NegativeInteger(value) => {
                self.values.push(
                    if let Type::Primitive(primitive) = &types[types[ast[expr].span()]] {
                        match primitive.kind() {
                            Primitive::I8 => Value::I8(
                                i8::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I16 => Value::I16(
                                i16::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I32 => Value::I32(
                                i32::try_from(*value)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I64 => Value::I64(*value),
                            _ => unreachable!("type checking guarantees integers are integers"),
                        }
                    } else {
                        unreachable!("type checking guarantees integers are primitives")
                    },
                );

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Expr::Float(value) => {
                self.values.push(Value::F64(*value));

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Expr::Boolean(value) => {
                self.values.push(Value::Boolean(*value));

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Expr::Unit => {
                self.values.push(Value::Unit);

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Expr::Name(_) => {
                self.translate_name(ast, names, expr);
            }
            Expr::Unary { op, .. } => {
                self.translate_unary(ast, names, types, expr, *op);
            }
            Expr::Block(exprs) if exprs.is_empty() => {
                self.values.push(Value::Unit);

                if self.last_in_fn {
                    self.emit_return();
                }
            }
            Expr::Block(exprs) => {
                self.translate_block(ast, names, types, exprs.as_slice());
            }
            Expr::Group(_) => {
                ast.for_children_exprs(expr, |ast, expr| {
                    self.translate_expr(ast, names, types, expr);
                });
            }
            Expr::Let { name, value, .. } => {
                self.translate_assign_name(ast, names, types, (name.span(), *value));
            }
            Expr::Binary {
                op: BinaryOp::Assign,
                lhs,
                rhs,
            } => {
                self.translate_assign(ast, names, types, (*lhs, *rhs));
            }
            Expr::Binary {
                op: BinaryOp::PathAccess,
                lhs,
                rhs,
            } => {
                self.translate_path_access(ast, names, types, (*lhs, *rhs));
            }
            Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            } => {
                self.translate_access(ast, names, types, (*lhs, *rhs));
            }
            Expr::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
            } => {
                self.translate_and(ast, names, types, (*lhs, *rhs));
            }
            Expr::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
            } => {
                self.translate_or(ast, names, types, (*lhs, *rhs));
            }
            Expr::Binary { op, .. } => {
                self.translate_binary(ast, names, types, expr, *op);
            }
            Expr::If {
                condition,
                when_true,
                otherwise,
            } => {
                self.translate_if(ast, names, types, (*condition, *when_true, *otherwise));
            }
            Expr::Call { callee, arguments } => {
                self.translate_call(ast, names, types, (*callee, arguments.as_slice()));
            }
            Expr::MethodCall {
                target,
                method,
                arguments,
            } => {
                self.translate_method_call(
                    ast,
                    names,
                    types,
                    (*target, *method, arguments.as_slice()),
                );
            }
            Expr::While {
                condition,
                when_true,
            } => {
                self.translate_while(ast, names, types, (*condition, *when_true));
            }
            Expr::Return(value) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                self.translate_expr(ast, names, types, *value);

                self.last_in_fn = last_in_fn;

                self.emit_return();

                self.values.push(Value::Unit);
            }
            Expr::AsUnit(value) => {
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
            Expr::Product { fields, .. } => {
                self.translate_product(ast, names, types, expr, fields.as_slice());
            }
            Expr::Match {
                expr,
                cases,
                fallback,
            } => {
                self.translate_match(ast, names, types, (*expr, cases.as_slice(), *fallback));
            }
            Expr::String(text) => {
                self.values.push(Value::String(text.clone()));

                if self.last_in_fn {
                    self.emit_return();
                }
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

    fn translate_name(&mut self, ast: &Ast, names: &Names, expr: ExprIndex) {
        let name = self
            .addresses
            .get(&names[ast[expr].span()])
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
            Addresslike::CompoundField { index, of } => {
                let (index, of) = (*index, *of);

                let address = self.next_address();

                self.push_instruction(Instruction::Access {
                    index,
                    of: Value::Address(of),
                    temporary: Value::Address(address),
                });

                self.values.push(Value::Address(address));
            }
        }

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_block(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        exprs: &[ExprIndex],
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.push_instruction(Instruction::ScopeStart);

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

        self.push_instruction(Instruction::PopN(0));
    }

    fn translate_unary(
        &mut self,
        ast: &Ast,
        names: &Names,
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

        let address = self.next_address();

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
        names: &Names,
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

        let address = self.next_address();

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
        names: &Names,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let value = rhs;

        match ast[lhs].kind() {
            Expr::Name(_) => {
                let lhs_span = ast[lhs].span();

                self.translate_assign_name(ast, names, types, (lhs_span, value));
            }
            Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            } => {
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
        names: &Names,
        types: &TypeChecker,
        (span, value): (Span, ExprIndex),
    ) {
        self.translate_expr(ast, names, types, value);

        let value = self
            .values
            .pop()
            .expect("assignments can only occur with a value");

        match names
            .get(span)
            .and_then(|span| self.addresses.get_mut(&span))
        {
            Some(Addresslike::Block(_) | Addresslike::NativeFn(_)) => {
                unreachable!("name resolution prevents assigning to these addresses");
            }
            Some(Addresslike::CompoundField { index, of }) => {
                let (index, of) = (*index, *of);

                self.push_instruction(Instruction::AccessAssign {
                    index,
                    of: Value::Address(of),
                    value,
                });
            }
            Some(Addresslike::Address(address)) => {
                address.version += 1;

                let address = *address;

                self.push_instruction(Instruction::Assign {
                    value,
                    to: Value::Address(address),
                });

                self.addresses.insert(span, Addresslike::Address(address));

                self.values.push(Value::Address(address));
            }
            Some(Addresslike::CallArgument(index)) => {
                let index = *index;

                self.push_instruction(Instruction::Assign {
                    value,
                    to: Value::CallArgument(index),
                });

                self.values.push(Value::CallArgument(index));
            }
            None => {
                let address = self.next_address();

                self.push_instruction(Instruction::Assign {
                    value,
                    to: Value::Address(address),
                });

                self.addresses.insert(span, Addresslike::Address(address));

                self.values.push(Value::Address(address));
            }
        }
    }

    fn translate_and(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, lhs);

        let address = self.assign_if_not_address();

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = self.next_block();

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition: Value::Address(address),
                when_true: when_true_block,
                otherwise: BlockIndex(usize::MAX),
            });
        }

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
            version: 0,
        }));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_or(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, lhs);

        let address = self.assign_if_not_address();

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let otherwise_block = self.next_block();

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition: Value::Address(address),
                when_true: BlockIndex(usize::MAX),
                otherwise: otherwise_block,
            });
        }

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
            version: 0,
        }));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_if(
        &mut self,
        ast: &Ast,
        names: &Names,
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

        let address = self.next_address();

        self.push_instruction(Instruction::Assign {
            value: Value::Runtime,
            to: Value::Address(address),
        });

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = self.next_block();

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition,
                when_true: when_true_block,
                otherwise: BlockIndex(usize::MAX),
            });
        }

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

        self.values.push(Value::Address(Address {
            block_index: address.block_index,
            offset: address.offset,
            version: 0,
        }));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_call(
        &mut self,
        ast: &Ast,
        names: &Names,
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

        let arity = arguments.len();

        for argument in arguments {
            self.push_instruction(Instruction::Push(argument));
        }

        let address = self.next_address();

        self.push_instruction(Instruction::Call {
            callee,
            arity,
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
        names: &Names,
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
            .expect("every expression produces a value");

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = self.next_block();

        if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
            block.terminator = Some(BlockTerminator::Branch {
                condition,
                when_true: when_true_block,
                otherwise: BlockIndex(usize::MAX),
            });
        }

        self.translate_expr(ast, names, types, when_true);

        let when_true_block = self
            .current_block
            .expect("while loops can only exist in a block");

        let otherwise_block = self.next_block();

        if let BlockTerminator::Branch { otherwise, .. } = self
            .blocks
            .get_mut(usize::from(current_block))
            .and_then(|block| block.terminator.as_mut())
            .expect("the destination to backpatch was set just before")
        {
            *otherwise = otherwise_block;
        }

        if let Some(block) = self.blocks.get_mut(usize::from(when_true_block))
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Jump(condition_block));
        }

        self.last_in_fn = last_in_fn;

        self.values.pop();

        self.values.push(Value::Unit);

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_product(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        expr: ExprIndex,
        fields: &[(Spanned<String>, ExprIndex)],
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let expr_span = ast[expr].span();

        let Type::Product {
            fields: type_fields,
            ..
        } = &types[types[expr_span]]
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
        names: &Names,
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

        let address = self.next_address();

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
        let lhs_span = ast[lhs].span();

        match &types[types[lhs_span]] {
            Type::Product {
                fields: type_fields,
                ..
            } => match ast[rhs].kind() {
                Expr::Name(name) => {
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
        names: &Names,
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

        let address = self.next_address();

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

    #[allow(clippy::too_many_lines)]
    fn translate_path_access(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let original_lhs = lhs;

        let lhs = match ast[lhs].kind() {
            Expr::Name(_) | Expr::SelfType => lhs,
            Expr::Binary {
                op: BinaryOp::PathAccess,
                rhs,
                ..
            } => match ast[*rhs].kind() {
                Expr::Name(_) | Expr::SelfType => *rhs,
                _ => {
                    unreachable!("name resolution verifies the path is correct");
                }
            },
            _ => {
                unreachable!("name resolution verifies the path is correct");
            }
        };

        let type_span = match names
            .get(ast[lhs].span())
            .and_then(|span| types.get_type(span))
        {
            Some(
                Type::Unknown
                | Type::Any
                | Type::Integer(_)
                | Type::NegativeInteger(_)
                | Type::Existential(_)
                | Type::Generic(_)
                | Type::Fn { .. }
                | Type::Associations(_)
                | Type::AnyOf { .. },
            )
            | None => None,
            Some(Type::Primitive(primitive)) => Some(primitive.span()),
            Some(Type::Product { name, .. } | Type::Sum { name, .. }) => Some(name.span()),
        };

        let type_span = type_span.map(|type_span| names.get(type_span).unwrap_or(type_span));

        match ast[rhs].kind() {
            Expr::Name(method_name) if let Some(type_span) = type_span => {
                if let Some(method_span) = ast[original_lhs]
                    .span()
                    .combine_with(ast[rhs].span())
                    .and_then(|span| types.resolve_association(type_span, method_name, span))
                {
                    let callee = match self
                        .addresses
                        .get(&method_span)
                        .expect("type checking guarantees the method exists")
                    {
                        Addresslike::Block(block_index) => Value::Fn(*block_index),
                        Addresslike::NativeFn(span) => Value::NativeFn(*span),
                        Addresslike::CallArgument(_)
                        | Addresslike::Address(_)
                        | Addresslike::CompoundField { .. } => {
                            unreachable!("methods are only ever functions or native functions");
                        }
                    };

                    self.values.push(callee);
                } else {
                    unreachable!("{type_span:?} {method_name:?} {:?}", ast[rhs].span());
                }
            }
            Expr::Product { name, .. }
                if let Some(type_span) = type_span
                    && let Some(Type::Sum {
                        variants: type_variants,
                        ..
                    }) = types.get_type(type_span) =>
            {
                let variant_index = match ast[rhs].kind() {
                    Expr::Product { name, .. } => {
                        let Some(variant_index) = type_variants
                            .iter()
                            .position(|variant| {
                                matches!(
                                    &types[*variant],
                                    Type::Product { name: variant_name, .. }
                                        if variant_name.kind() == name.kind()
                                )
                            })
                            .and_then(|index| u16::try_from(index).ok())
                        else {
                            unreachable!("type checking guarantees the variant exists on the type");
                        };

                        variant_index
                    }
                    _ => unreachable!("for now, only products can be variants"),
                };

                self.translate_expr(ast, names, types, rhs);

                let Some(Value::Compound(fields)) = self.values.pop() else {
                    unreachable!("type checking guarantees the right operand is a product literal");
                };

                self.values.push(Value::TaggedCompound {
                    fields,
                    tag: variant_index,
                });
            }
            _ => {
                self.translate_expr(ast, names, types, rhs);
            }
        }

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_method_call(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        (target, method, arguments): (ExprIndex, ExprIndex, &[ExprIndex]),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let type_span = match &types[types[ast[target].span()]] {
            Type::Unknown
            | Type::Any
            | Type::Integer(_)
            | Type::NegativeInteger(_)
            | Type::Existential(_)
            | Type::Generic(_)
            | Type::Fn { .. }
            | Type::Associations(_)
            | Type::AnyOf { .. } => {
                unreachable!("type checking guarantees these types are not used for method calls");
            }
            Type::Primitive(primitive) => primitive.span(),
            Type::Product { name, .. } | Type::Sum { name, .. } => name.span(),
        };

        let type_span = names.get(type_span).unwrap_or(type_span);

        let Expr::Name(method_name) = ast[method].kind() else {
            unreachable!("methods are only names");
        };

        let Some(method_span) =
            types.resolve_association(type_span, method_name, ast[method].span())
        else {
            unreachable!("type checking guarantees the method exists");
        };

        let callee = match self
            .addresses
            .get(&method_span)
            .expect("type checking guarantees the method exists")
        {
            Addresslike::Block(block_index) => Value::Fn(*block_index),
            Addresslike::NativeFn(span) => Value::NativeFn(*span),
            Addresslike::CallArgument(_)
            | Addresslike::Address(_)
            | Addresslike::CompoundField { .. } => {
                unreachable!("methods are only ever functions or native functions");
            }
        };

        let arguments = iter::once(target)
            .chain(arguments.iter().copied())
            .map(|argument| {
                self.translate_expr(ast, names, types, argument);

                self.values
                    .pop()
                    .expect("each method call argument should produce a value")
            })
            .collect::<Vec<_>>();

        let arity = arguments.len();

        for argument in arguments {
            self.push_instruction(Instruction::Push(argument));
        }

        let address = self.next_address();

        self.push_instruction(Instruction::Call {
            callee,
            arity,
            temporary: Value::Address(address),
        });

        self.values.push(Value::Address(address));

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_match(
        &mut self,
        ast: &Ast,
        names: &Names,
        types: &TypeChecker,
        (expr, cases, fallback): (ExprIndex, &[MatchCase], ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(ast, names, types, expr);

        let expr_address = self.assign_if_not_address();

        let address = self.next_address();

        self.push_instruction(Instruction::Assign {
            value: Value::Runtime,
            to: Value::Address(address),
        });

        let mut backpatch_successes = vec![];

        let mut address_version = address.version;

        for case in cases {
            let backpatch_these = self.translate_pattern(types, expr_address, case.pattern());

            self.push_instruction(Instruction::ScopeStart);

            self.translate_expr(ast, names, types, case.case());

            let case_value = self
                .values
                .pop()
                .expect("every expression produces a value");

            address_version += 1;

            self.push_instruction(Instruction::Assign {
                value: case_value,
                to: Value::Address(Address {
                    block_index: address.block_index,
                    offset: address.offset,
                    version: address_version,
                }),
            });

            self.push_instruction(Instruction::PopN(0));

            backpatch_successes.push(
                self.current_block
                    .expect("match expressions can only exist in blocks"),
            );

            let next_block = self.next_block();

            for backpatch in backpatch_these {
                if let Some(BlockTerminator::Branch { otherwise, .. }) = self
                    .blocks
                    .get_mut(usize::from(backpatch))
                    .map(Block::terminator_mut)
                {
                    *otherwise = next_block;
                }
            }
        }

        self.translate_expr(ast, names, types, fallback);

        let fallback_value = self
            .values
            .pop()
            .expect("every expression produces a value");

        address_version += 1;

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address_version,
        };

        self.push_instruction(Instruction::Assign {
            value: fallback_value,
            to: Value::Address(address),
        });

        self.next_block();

        for backpatch in backpatch_successes {
            if let Some(block) = self.blocks.get_mut(usize::from(backpatch))
                && block.terminator.is_none()
            {
                block.terminator = Some(BlockTerminator::Jump(
                    self.current_block
                        .expect("match expressions can only exist in blocks"),
                ));
            }
        }

        self.last_in_fn = last_in_fn;

        self.values.push(Value::Address(Address {
            block_index: address.block_index,
            offset: address.offset,
            version: 0,
        }));

        if self.last_in_fn {
            self.emit_return();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn translate_pattern(
        &mut self,
        types: &TypeChecker,
        match_against: Address,
        pattern: &Spanned<Pattern>,
    ) -> Vec<BlockIndex> {
        match &types[types[pattern.span()]] {
            Type::Integer(_)
            | Type::NegativeInteger(_)
            | Type::Unknown
            | Type::Any
            | Type::Fn { .. }
            | Type::Existential(_)
            | Type::Generic(_)
            | Type::Associations(_)
            | Type::AnyOf { .. } => unreachable!(
                "only integer primitives, booleans, unit, strings, and products can be patterns"
            ),
            Type::Primitive(primitive) => {
                let value = match (primitive.kind(), pattern.kind()) {
                    (Primitive::U8, Pattern::Integer(value)) => Value::U8(
                        u8::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I8, Pattern::Integer(value)) => Value::I8(
                        i8::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::U16, Pattern::Integer(value)) => Value::U16(
                        u16::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I16, Pattern::Integer(value)) => Value::I16(
                        i16::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::U32, Pattern::Integer(value)) => Value::U32(
                        u32::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I32, Pattern::Integer(value)) => Value::I32(
                        i32::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::U64, Pattern::Integer(value)) => Value::U64(*value),
                    (Primitive::I64, Pattern::Integer(value)) => Value::I64(
                        i64::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I8, Pattern::NegativeInteger(value)) => Value::I8(
                        i8::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I16, Pattern::NegativeInteger(value)) => Value::I16(
                        i16::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I32, Pattern::NegativeInteger(value)) => Value::I32(
                        i32::try_from(*value)
                            .expect("type checking guarantees the conversion is valid"),
                    ),
                    (Primitive::I64, Pattern::NegativeInteger(value)) => Value::I64(*value),
                    (Primitive::Boolean, Pattern::Boolean(value)) => Value::Boolean(*value),
                    (Primitive::Unit, Pattern::Unit) => Value::Unit,
                    (Primitive::String, Pattern::String(text)) => Value::String(text.clone()),
                    _ => {
                        unreachable!("type checking guarantees no other combinations make it here")
                    }
                };

                let condition_address = self.next_address();

                self.push_instruction(Instruction::Binary {
                    op: BinaryOp::Equal,
                    lhs: Value::Address(match_against),
                    rhs: value,
                    temporary: Value::Address(condition_address),
                });

                let current_block = self
                    .current_block
                    .expect("a block will exist if we're translating expressions");

                let when_true_block = self.next_block();

                if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
                    block.terminator = Some(BlockTerminator::Branch {
                        condition: Value::Address(condition_address),
                        when_true: when_true_block,
                        otherwise: BlockIndex(usize::MAX),
                    });
                }

                vec![current_block]
            }
            Type::Product {
                fields: field_types,
                ..
            } => match pattern.kind() {
                Pattern::Integer(_)
                | Pattern::NegativeInteger(_)
                | Pattern::Boolean(_)
                | Pattern::Unit
                | Pattern::String(_) => unreachable!(
                    "type checking guarantees only patterns that are products make it here"
                ),
                Pattern::Product { fields, .. } => {
                    let mut to_backpatch = vec![];

                    for (field_name, field) in fields {
                        let field_address = self.next_address();

                        let field_index = field_types.iter()
                            .position(|(field_type_name, _)| field_type_name == field_name.kind())
                            .expect("all fields in the pattern exist in the type since type checking succeeded");

                        self.push_instruction(Instruction::Access {
                            index: field_index,
                            of: Value::Address(match_against),
                            temporary: Value::Address(field_address),
                        });

                        if let Some(field) = field {
                            to_backpatch.append(&mut self.translate_pattern(
                                types,
                                field_address,
                                field,
                            ));
                        } else {
                            self.addresses.insert(
                                field_name.span(),
                                Addresslike::CompoundField {
                                    index: field_index,
                                    of: match_against,
                                },
                            );
                        }
                    }

                    to_backpatch
                }
            },
            Type::Sum {
                variants: type_variants,
                ..
            } => match pattern.kind() {
                Pattern::Integer(_)
                | Pattern::NegativeInteger(_)
                | Pattern::Boolean(_)
                | Pattern::Unit
                | Pattern::String(_) => unreachable!(
                    "type checking guarantees only patterns that are sum variants make it here"
                ),
                Pattern::Product { path, fields } => {
                    let Some(PathElement::Name(name)) = path.last().map(Spanned::kind) else {
                        unreachable!("name resolution guarantees paths in patterns end in a name");
                    };

                    let Some(variant_index) = type_variants
                        .iter()
                        .position(|variant| {
                            matches!(
                                &types[*variant],
                                Type::Product { name: variant_name, .. }
                                    if variant_name.kind() == name
                            )
                        })
                        .and_then(|index| u16::try_from(index).ok())
                    else {
                        unreachable!("type checking guarantees the variant exists on the type");
                    };

                    let Type::Product {
                        fields: field_types,
                        ..
                    } = &types[type_variants[usize::from(variant_index)]]
                    else {
                        unreachable!("only products are variants of sums");
                    };

                    let tag_address = self.next_address();

                    self.push_instruction(Instruction::GetTag {
                        of: Value::Address(match_against),
                        temporary: Value::Address(tag_address),
                    });

                    let condition_address = self.next_address();

                    self.push_instruction(Instruction::Binary {
                        op: BinaryOp::Equal,
                        lhs: Value::Address(tag_address),
                        rhs: Value::U16(variant_index),
                        temporary: Value::Address(condition_address),
                    });

                    let current_block = self
                        .current_block
                        .expect("a block will exist if we're translating expressions");

                    let when_true_block = self.next_block();

                    if let Some(block) = self.blocks.get_mut(usize::from(current_block)) {
                        block.terminator = Some(BlockTerminator::Branch {
                            condition: Value::Address(condition_address),
                            when_true: when_true_block,
                            otherwise: BlockIndex(usize::MAX),
                        });
                    }

                    let mut to_backpatch = vec![current_block];

                    for (field_name, field) in fields {
                        let field_address = self.next_address();

                        let field_index = field_types.iter()
                            .position(|(field_type_name, _)| field_type_name == field_name.kind())
                            .expect("all fields in the pattern exist in the type since type checking succeeded");

                        self.push_instruction(Instruction::Access {
                            index: field_index,
                            of: Value::Address(match_against),
                            temporary: Value::Address(field_address),
                        });

                        if let Some(field) = field {
                            to_backpatch.append(&mut self.translate_pattern(
                                types,
                                field_address,
                                field,
                            ));
                        } else {
                            self.addresses.insert(
                                field_name.span(),
                                Addresslike::CompoundField {
                                    index: field_index,
                                    of: match_against,
                                },
                            );
                        }
                    }

                    to_backpatch
                }
            },
        }
    }
}
