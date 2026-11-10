use crate::{
    ApartOptions, Span, Spanned,
    high_ir::{
        BinaryOp, Expr, ExprIndex, Ir, Item, ItemIndex, MatchCase, PathElement, Pattern, Resolved,
        UnaryOp,
    },
    type_check::{Primitive, Type, TypeChecker, TypeIndex},
};

use std::{collections::HashMap, error, fmt, iter};

pub fn translate(
    options: &ApartOptions,
    sources: &[(usize, (&str, &str))],
    (ir, types): (&Ir, &TypeChecker),
) -> Result<BasicBlocks, Error> {
    let mut translator = Translator::new();

    if let Some(root) = ir.roots().iter().find(|root| {
        if let Item::Fn { name, .. } = ir[**root].kind()
            && name.kind() == "main"
        {
            true
        } else {
            false
        }
    }) {
        translator.label_items(sources, (ir, types), [*root].as_slice(), true)?;
    }

    translator.label_items(sources, (ir, types), ir.roots(), false)?;

    let function_count = translator.blocks.len();

    translator.translate_items(options, sources, (ir, types), ir.roots())?;

    for (b, block) in translator.blocks.iter_mut().enumerate() {
        if block.terminator.is_none() {
            block.terminator = Some(BlockTerminator::Jump(BlockIndex(
                u32::try_from(if b < function_count {
                    b + function_count + 1
                } else {
                    b + 1
                })
                .map_err(|_| Error::TooManyBlocks)?,
            )));
        }
    }

    Ok(BasicBlocks {
        blocks: translator.blocks,
    })
}

pub struct BasicBlocks {
    blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockIndex(pub u32);

impl From<BlockIndex> for u32 {
    fn from(value: BlockIndex) -> Self {
        value.0
    }
}

impl From<BlockIndex> for usize {
    fn from(value: BlockIndex) -> Self {
        Self::try_from(value.0).expect("this is guaranteed to be fine since `basic_blocks::translate` would have errored and ended the compilation process early if it wasn't")
    }
}

#[derive(Debug)]
pub enum Error {
    TooManyBlocks,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyBlocks => write!(
                f,
                "the number of basic blocks is not allowed to exceed u32::MAX"
            ),
        }
    }
}

impl error::Error for Error {}

pub struct Block {
    call_argument_count: u8,
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
    pub const fn call_argument_count(&self) -> u8 {
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

struct Translator<'a> {
    blocks: Vec<Block>,
    current_block: Option<BlockIndex>,
    values: Vec<TypedValue>,
    addresses: HashMap<Span, TypedAddresslike>,
    last_in_fn: bool,
    label: Vec<&'a str>,
}

impl Translator<'_> {
    fn new() -> Self {
        Self {
            blocks: vec![],
            current_block: None,
            values: vec![],
            addresses: HashMap::new(),
            last_in_fn: false,
            label: vec![],
        }
    }

    const fn switch_to_block(&mut self, block_index: BlockIndex) {
        self.current_block = Some(block_index);
    }

    fn get_block(&self, block_index: BlockIndex) -> Option<&Block> {
        self.blocks.get(usize::from(block_index))
    }

    fn get_block_mut(&mut self, block_index: BlockIndex) -> Option<&mut Block> {
        self.blocks.get_mut(usize::from(block_index))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedAddresslike {
    addresslike: Addresslike,
    type_index: TypeIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Addresslike {
    Address(Address),
    Fn(BlockIndex),
    CallArgument(u8),
    NativeFn {
        label: String,
    },
    CompoundField {
        index: u8,
        of: Address,
        of_type_index: TypeIndex,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedValue {
    pub value: Value,
    pub type_index: TypeIndex,
}

impl TypedValue {
    #[allow(dead_code)]
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn type_index(&self) -> TypeIndex {
        self.type_index
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Never,
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    Boolean(bool),
    Unit,
    String(String),
    Fn(BlockIndex),
    Address(Address),
    NativeFn { label: String },
    CallArgument(u8),
    Register(usize),
    Compound(Vec<TypedValue>),
    TaggedCompound { fields: Vec<TypedValue>, tag: u16 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Panic,
    FnLabel {
        file_name: String,
        function_label: String,
    },
    NoOp,
    Unary {
        op: UnaryOp,
        operand: TypedValue,
        temporary: Value,
    },
    Binary {
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        temporary: Value,
    },
    Assign {
        value: TypedValue,
        to: Value,
    },
    Push(TypedValue),
    Call {
        callee: TypedValue,
        arity: u8,
        temporary: Value,
    },
    Access {
        index: u8,
        of: TypedValue,
        temporary: Value,
    },
    AccessAssign {
        index: u8,
        of: TypedValue,
        value: TypedValue,
    },
    GetTag {
        of: TypedValue,
        temporary: Value,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockTerminator {
    Jump(BlockIndex),
    Branch {
        condition: TypedValue,
        when_true: BlockIndex,
        otherwise: BlockIndex,
    },
    Return(TypedValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    pub block_index: BlockIndex,
    pub offset: usize,
    pub version: u64,
}

impl<'a> Translator<'a> {
    fn next_block(&mut self) -> Result<BlockIndex, Error> {
        let block_index =
            BlockIndex(u32::try_from(self.blocks.len()).map_err(|_| Error::TooManyBlocks)?);

        self.blocks.push(Block {
            call_argument_count: 0,
            instructions: vec![],
            terminator: None,
        });

        self.switch_to_block(block_index);

        Ok(block_index)
    }

    fn next_address(&self) -> Address {
        Address {
            block_index: self
                .current_block
                .expect("addresses are only used in blocks"),
            offset: self.instructions_len(),
            version: 0,
        }
    }

    fn push_instruction(&mut self, instruction: Instruction) {
        let block = self
            .current_block
            .and_then(|block_index| self.get_block_mut(block_index))
            .expect("instructions only get pushed within blocks");

        block.instructions.push(instruction);
    }

    fn instructions_len(&self) -> usize {
        self.current_block
            .and_then(|block_index| self.get_block(block_index))
            .map_or(0, |block| block.instructions().len())
    }

    fn label_items(
        &mut self,
        sources: &[(usize, (&str, &'a str))],
        (ir, types): (&Ir, &TypeChecker),
        items: &[ItemIndex],
        allow_main: bool,
    ) -> Result<(), Error> {
        for item in items {
            match ir[*item].kind() {
                Item::Mod { name, contents, .. } => {
                    let lexeme = sources
                        .iter()
                        .find(|(source_id, _)| *source_id == name.span().source_id())
                        .and_then(|(_, (_, source))| name.span().lexeme(source))
                        .expect("the span is from this source");

                    self.label.push(lexeme);

                    self.label_items(sources, (ir, types), contents, true)?;

                    self.label.pop();
                }
                Item::Teach { student, body, .. } => {
                    let lexeme = sources
                        .iter()
                        .find(|(source_id, _)| *source_id == student.span().source_id())
                        .and_then(|(_, (_, source))| student.span().lexeme(source))
                        .expect("the span is from this source");

                    self.label.push(lexeme);

                    self.label_items(sources, (ir, types), body, true)?;

                    self.label.pop();
                }
                Item::NativeFn { name, .. } if name.kind() != "main" || allow_main => {
                    let lexeme = sources
                        .iter()
                        .find(|(source_id, _)| *source_id == name.span().source_id())
                        .and_then(|(_, (_, source))| name.span().lexeme(source))
                        .expect("the span is from this source");

                    self.label.push(lexeme);

                    self.addresses.insert(
                        name.span(),
                        TypedAddresslike {
                            addresslike: Addresslike::NativeFn {
                                label: self.concat_label(),
                            },
                            type_index: types[name.span()],
                        },
                    );

                    self.label.pop();
                }
                Item::Fn { name, .. } if name.kind() != "main" || allow_main => {
                    self.addresses.insert(
                        name.span(),
                        TypedAddresslike {
                            addresslike: Addresslike::Fn(BlockIndex(
                                u32::try_from(self.blocks.len())
                                    .map_err(|_| Error::TooManyBlocks)?,
                            )),
                            type_index: types[name.span()],
                        },
                    );

                    let _ = self.next_block();

                    let lexeme = sources
                        .iter()
                        .find(|(source_id, _)| *source_id == name.span().source_id())
                        .and_then(|(_, (_, source))| name.span().lexeme(source))
                        .expect("the span is from this source");

                    self.label.push(lexeme);

                    let file_name = sources
                        .iter()
                        .find_map(|(source_id, (file_name, _))| {
                            if *source_id == name.span().source_id() {
                                Some(file_name.to_string())
                            } else {
                                None
                            }
                        })
                        .expect("the span is from this source");

                    let function_label = self.concat_label();

                    self.push_instruction(Instruction::FnLabel {
                        file_name,
                        function_label,
                    });

                    self.label.pop();
                }
                Item::Primitive { .. }
                | Item::Product { .. }
                | Item::Sum { .. }
                | Item::Fn { .. }
                | Item::NativeFn { .. }
                | Item::Alias { .. } => {}
            }
        }

        Ok(())
    }

    fn translate_items(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &'a str))],
        (ir, types): (&Ir, &TypeChecker),
        items: &[ItemIndex],
    ) -> Result<(), Error> {
        for item in items {
            match ir[*item].kind() {
                Item::Primitive { .. }
                | Item::NativeFn { .. }
                | Item::Product { .. }
                | Item::Sum { .. }
                | Item::Alias { .. } => {}
                Item::Mod { contents, .. } => {
                    self.translate_items(options, sources, (ir, types), contents.as_slice())?;
                }
                Item::Teach { body, .. } => {
                    self.translate_items(options, sources, (ir, types), body.as_slice())?;
                }
                Item::Fn { .. } => {
                    self.translate_function(options, sources, (ir, types), *item)?;
                }
            }
        }

        Ok(())
    }

    fn concat_label(&self) -> String {
        self.label.join("~")
    }

    fn translate_function(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        item: ItemIndex,
    ) -> Result<(), Error> {
        if let Item::Fn {
            name,
            parameters,
            body,
            ..
        } = ir[item].kind()
            && let Some(block_index) = self.addresses.get(&name.span()).and_then(|address| {
                if let Addresslike::Fn(block_index) = address.addresslike {
                    Some(block_index)
                } else {
                    None
                }
            })
        {
            self.switch_to_block(block_index);

            let mut parameter_count = 0u8;

            for parameter in parameters {
                self.addresses.insert(
                    parameter.name().span(),
                    TypedAddresslike {
                        addresslike: Addresslike::CallArgument(parameter_count),
                        type_index: types[parameter.name().resolved()],
                    },
                );

                parameter_count += 1;
            }

            let after_arguments = self.next_block()?;

            let Some(block) = self.get_block_mut(block_index) else {
                unreachable!("we're guaranteed to have a block by now");
            };

            block.call_argument_count = parameter_count;

            if block.terminator.is_none() {
                block.terminator = Some(BlockTerminator::Jump(after_arguments));
            }

            let block_index = after_arguments;

            self.switch_to_block(block_index);

            let last_in_fn = self.last_in_fn;

            self.last_in_fn = true;

            self.translate_expr(options, sources, (ir, types), *body)?;

            self.last_in_fn = last_in_fn;

            assert_eq!(self.values.as_slice(), &[]);
        }

        Ok(())
    }
}

impl Translator<'_> {
    fn assign_value(&mut self, value: TypedValue) -> Address {
        let address = self.next_address();

        self.push_instruction(Instruction::Assign {
            value,
            to: Value::Address(address),
        });

        address
    }

    fn push_value(&mut self, value: TypedValue) {
        self.values.push(value);
    }

    fn pop_value(&mut self) -> TypedValue {
        self.values
            .pop()
            .expect("values are only popped after expressions, and all expressions produce a value")
    }

    #[allow(clippy::too_many_lines)]
    fn translate_expr(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        expr: ExprIndex,
    ) -> Result<(), Error> {
        match ir[expr].kind() {
            Expr::PathElement(_) => {
                unreachable!(
                    "type checking guarantees these path elements aren't used as expressions"
                );
            }
            Expr::Integer(radix) => {
                self.push_value(
                    if let Type::Primitive { primitive, .. } = &types[types[ir[expr].span()]] {
                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == ir[expr].span().source_id())
                            .and_then(|(_, (_, source))| ir[expr].span().lexeme(source))
                            .expect("the span is from this source")
                            .replace('_', "");

                        let lexeme = lexeme.as_str();

                        match primitive.kind() {
                            Primitive::U8 => TypedValue {
                                value: Value::U8(
                                    u8::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I8 => TypedValue {
                                value: Value::I8(
                                    i8::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::U16 => TypedValue {
                                value: Value::U16(
                                    u16::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I16 => TypedValue {
                                value: Value::I16(
                                    i16::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::U32 => TypedValue {
                                value: Value::U32(
                                    u32::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I32 => TypedValue {
                                value: Value::I32(
                                    i32::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::U64 => TypedValue {
                                value: Value::U64(
                                    u64::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I64 => TypedValue {
                                value: Value::I64(
                                    i64::from_str_radix(lexeme, *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            primitive => unreachable!(
                                "type checking guarantees integers are integers {primitive:?}"
                            ),
                        }
                    } else {
                        unreachable!(
                            "type checking guarantees integers are primitives {:?}",
                            types.type_to_string(types[ir[expr].span()])
                        );
                    },
                );

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::NegativeInteger(radix) => {
                self.push_value(
                    if let Type::Primitive { primitive, .. } = &types[types[ir[expr].span()]] {
                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == ir[expr].span().source_id())
                            .and_then(|(_, (_, source))| ir[expr].span().lexeme(source))
                            .expect("the span is from this source")
                            .replace('_', "");

                        let lexeme = lexeme.as_str();

                        match primitive.kind() {
                            Primitive::I8 => TypedValue {
                                value: Value::I8(
                                    i8::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I16 => TypedValue {
                                value: Value::I16(
                                    i16::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I32 => TypedValue {
                                value: Value::I32(
                                    i32::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            Primitive::I64 => TypedValue {
                                value: Value::I64(
                                    i64::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                        .expect("type checking guarantees the conversion is valid"),
                                ),
                                type_index: types[ir[expr].resolved()],
                            },
                            primitive => unreachable!(
                                "type checking guarantees integers are integers {primitive:?}"
                            ),
                        }
                    } else {
                        unreachable!(
                            "type checking guarantees integers are primitives {:?}",
                            types.type_to_string(types[ir[expr].span()])
                        );
                    },
                );

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::Float => {
                self.push_value(
                    if let Type::Primitive { primitive, .. } = &types[types[ir[expr].span()]] {
                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == ir[expr].span().source_id())
                            .and_then(|(_, (_, source))| ir[expr].span().lexeme(source))
                            .expect("the span is from this source")
                            .replace('_', "");

                        let lexeme = lexeme.as_str();

                        match primitive.kind() {
                            Primitive::F32 => {
                                TypedValue {
                                    value: Value::F32(lexeme.parse::<f32>().expect(
                                        "type checking guarantees the conversion is valid",
                                    )),
                                    type_index: types[ir[expr].resolved()],
                                }
                            }
                            Primitive::F64 => {
                                TypedValue {
                                    value: Value::F64(lexeme.parse::<f64>().expect(
                                        "type checking guarantees the conversion is valid",
                                    )),
                                    type_index: types[ir[expr].resolved()],
                                }
                            }
                            primitive => unreachable!(
                                "type checking guarantees floats are floats {primitive:?}"
                            ),
                        }
                    } else {
                        unreachable!(
                            "type checking guarantees floats are primitives {:?}",
                            types.type_to_string(types[ir[expr].span()])
                        );
                    },
                );

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::Boolean(value) => {
                self.push_value(TypedValue {
                    value: Value::Boolean(*value),
                    type_index: types[ir[expr].resolved()],
                });

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::Unit => {
                self.push_value(TypedValue {
                    value: Value::Unit,
                    type_index: types[ir[expr].resolved()],
                });

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::Never => {
                self.push_instruction(Instruction::Panic);

                self.push_value(TypedValue {
                    value: Value::Never,
                    type_index: types[ir[expr].resolved()],
                });

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::Name(_) => {
                self.translate_name((ir, types), expr);

                Ok(())
            }
            Expr::Unary { op, .. } => {
                self.translate_unary(options, sources, (ir, types), expr, *op)
            }
            Expr::Block(exprs) => {
                self.translate_block(options, sources, (ir, types), exprs.as_slice())
            }
            Expr::Let { name, value, .. } => self.translate_assign_name(
                options,
                sources,
                (ir, types),
                (expr, name.span(), *value),
            ),
            Expr::Binary {
                op: BinaryOp::Assign,
                lhs,
                rhs,
            } => self.translate_assign(options, sources, (ir, types), (expr, *lhs, *rhs)),
            Expr::Binary {
                op: BinaryOp::PathAccess,
                lhs,
                rhs,
            } => self.translate_path_access(options, sources, (ir, types), (*lhs, *rhs)),
            Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            } => self.translate_access(options, sources, (ir, types), (expr, *lhs, *rhs)),
            Expr::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
            } => self.translate_and(options, sources, (ir, types), (expr, *lhs, *rhs)),
            Expr::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
            } => self.translate_or(options, sources, (ir, types), (expr, *lhs, *rhs)),
            Expr::Binary { op, .. } => {
                self.translate_binary(options, sources, (ir, types), expr, *op)
            }
            Expr::If {
                condition,
                when_true,
                otherwise,
            } => self.translate_if(
                options,
                sources,
                (ir, types),
                (expr, *condition, *when_true, *otherwise),
            ),
            Expr::Call { callee, arguments } => self.translate_call(
                options,
                sources,
                (ir, types),
                (expr, *callee, arguments.as_slice()),
            ),
            Expr::MethodCall {
                target,
                method,
                arguments,
            } => self.translate_method_call(
                options,
                sources,
                (ir, types),
                (expr, *target, *method, arguments.as_slice()),
            ),
            Expr::While {
                condition,
                when_true,
            } => self.translate_while(
                options,
                sources,
                (ir, types),
                (expr, *condition, *when_true),
            ),
            Expr::Return(value) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                self.translate_expr(options, sources, (ir, types), *value)?;

                self.last_in_fn = last_in_fn;

                self.emit_return();

                self.push_value(TypedValue {
                    value: Value::Never,
                    type_index: types[ir[expr].resolved()],
                });

                Ok(())
            }
            Expr::AsUnit(value) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                self.translate_expr(options, sources, (ir, types), *value)?;

                self.pop_value();

                self.last_in_fn = last_in_fn;

                self.push_value(TypedValue {
                    value: Value::Unit,
                    type_index: types[ir[expr].resolved()],
                });

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
            Expr::Product { fields, .. } => {
                self.translate_product(options, sources, (ir, types), expr, fields.as_slice())
            }
            Expr::Match {
                expr: match_against,
                cases,
                fallback,
            } => self.translate_match(
                options,
                sources,
                (ir, types),
                (expr, *match_against, cases.as_slice(), *fallback),
            ),
            Expr::String(text) => {
                let mut characters = text.chars().peekable();

                let mut text = String::new();

                while let Some(ch) = characters.next() {
                    match ch {
                        '\\' => {
                            let mut buffer = String::new();

                            match characters.next() {
                                Some('\\') => buffer.push('\\'),
                                Some('t') => buffer.push('\t'),
                                Some('n') => buffer.push('\n'),
                                Some('r') => buffer.push('\r'),
                                Some('u') => {
                                    if matches!(characters.by_ref().peek(), Some('{')) {
                                        characters.next();

                                        while let Some(n) = characters.peek()
                                            && n.is_ascii_hexdigit()
                                        {
                                            buffer.push(*n);

                                            characters.next();
                                        }

                                        if matches!(characters.by_ref().peek(), Some('}')) {
                                            characters.next();

                                            if let Some(ch) =
                                                u32::from_str_radix(buffer.as_str(), 16)
                                                    .ok()
                                                    .and_then(char::from_u32)
                                            {
                                                buffer.clear();
                                                buffer.push(ch);
                                            } else {
                                                buffer = format!("\\u{{{buffer}}}");
                                            }
                                        } else {
                                            buffer = format!("\\u{{{buffer}");
                                        }
                                    } else {
                                        buffer = "\\u{".to_string();
                                    }
                                }
                                Some(n) => {
                                    buffer.push(ch);
                                    buffer.push(n);
                                }
                                None => {
                                    buffer.push(ch);
                                }
                            }

                            text.push_str(buffer.as_str());
                        }
                        _ => {
                            text.push(ch);
                        }
                    }
                }

                self.push_value(TypedValue {
                    value: Value::String(text),
                    type_index: types[ir[expr].resolved()],
                });

                if self.last_in_fn {
                    self.emit_return();
                }

                Ok(())
            }
        }
    }

    fn emit_return(&mut self) {
        let value = self.pop_value();

        let current_block = self
            .current_block
            .expect("there will be a block by this point");

        if let Some(block) = self.get_block_mut(current_block)
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Return(value));
        }
    }

    fn translate_name(&mut self, (ir, types): (&Ir, &TypeChecker), expr: ExprIndex) {
        let name = self
            .addresses
            .get(&ir[expr].resolved())
            .expect("the name should be valid since we succeeded in name resolution");

        match name.addresslike {
            Addresslike::Fn(block_index) => {
                self.push_value(TypedValue {
                    value: Value::Fn(block_index),
                    type_index: types[ir[expr].resolved()],
                });
            }
            Addresslike::Address(address) => {
                self.push_value(TypedValue {
                    value: Value::Address(address),
                    type_index: types[ir[expr].resolved()],
                });
            }
            Addresslike::CallArgument(offset) => {
                self.push_value(TypedValue {
                    value: Value::CallArgument(offset),
                    type_index: types[ir[expr].resolved()],
                });
            }
            Addresslike::NativeFn { ref label } => {
                self.push_value(TypedValue {
                    value: Value::NativeFn {
                        label: label.clone(),
                    },
                    type_index: types[ir[expr].resolved()],
                });
            }
            Addresslike::CompoundField {
                index,
                of,
                of_type_index,
            } => {
                let (index, of) = (index, of);

                let address = self.next_address();

                self.push_instruction(Instruction::Access {
                    index,
                    of: TypedValue {
                        value: Value::Address(of),
                        type_index: of_type_index,
                    },
                    temporary: Value::Address(address),
                });

                self.push_value(TypedValue {
                    value: Value::Address(address),
                    type_index: types[ir[expr].resolved()],
                });
            }
        }

        if self.last_in_fn {
            self.emit_return();
        }
    }

    fn translate_block(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        exprs: &[ExprIndex],
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        for expr in exprs.iter().rev().skip(1).rev() {
            self.translate_expr(options, sources, (ir, types), *expr)?;

            self.pop_value();
        }

        self.last_in_fn = last_in_fn;

        self.translate_expr(
            options,
            sources,
            (ir, types),
            *exprs
                .last()
                .expect("`translate_block` in only called on blocks that aren't empty"),
        )?;

        Ok(())
    }

    fn translate_unary(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        expr: ExprIndex,
        op: UnaryOp,
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let mut error = Ok(());

        ir.for_children_exprs(expr, |ir, expr| {
            if let Err(real_error) = self.translate_expr(options, sources, (ir, types), expr) {
                error = Err(real_error);
            }
        });

        let operand = self.pop_value();

        let address = self.next_address();

        self.push_instruction(Instruction::Unary {
            op,
            operand,
            temporary: Value::Address(address),
        });

        self.push_value(TypedValue {
            value: Value::Address(address),
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        error
    }

    fn translate_binary(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        expr: ExprIndex,
        op: BinaryOp,
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let mut error = Ok(());

        ir.for_children_exprs(expr, |ir, expr| {
            if let Err(real_error) = self.translate_expr(options, sources, (ir, types), expr) {
                error = Err(real_error);
            }
        });

        let rhs = self.pop_value();

        let lhs = self.pop_value();

        let address = self.next_address();

        self.push_instruction(Instruction::Binary {
            // TODO: this injection of integer division feels hacky.  improve it later
            op: if
                matches!(op, BinaryOp::Divide)
                && (
                    matches!(types[types[ir[expr].span()]], Type::Integer(_) | Type::NegativeInteger(_))
                    || matches!(&types[types[ir[expr].span()]], Type::Primitive { primitive, .. } if matches!(primitive.kind(), Primitive::U8 | Primitive::I8 | Primitive::U16 | Primitive::I16 | Primitive::U32 | Primitive::I32 | Primitive::U64 | Primitive::I64))
                )
            {
                BinaryOp::IntegerDivide
            } else {
                op
            },
            lhs,
            rhs,
            temporary: Value::Address(address),
        });

        self.push_value(TypedValue {
            value: Value::Address(address),
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        error
    }

    fn translate_assign(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, lhs, rhs): (ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let value = rhs;

        match ir[lhs].kind() {
            Expr::Name(_) => {
                self.translate_assign_name(
                    options,
                    sources,
                    (ir, types),
                    (expr, ir[lhs].resolved(), value),
                )?;

                Ok(())
            }
            Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            } => self.translate_assign_access(
                options,
                sources,
                (ir, types),
                (expr, *lhs, *rhs, value),
            ),
            _ => {
                unreachable!("for now, only names and accesses can be assigned to");
            }
        }
    }

    fn translate_assign_name(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, span, value): (ExprIndex, Span, ExprIndex),
    ) -> Result<(), Error> {
        self.translate_expr(options, sources, (ir, types), value)?;

        let value = self.pop_value();

        match self
            .addresses
            .get_mut(&span)
            .map(|addresslike| (&mut addresslike.addresslike, addresslike.type_index))
        {
            Some((Addresslike::Fn(_) | Addresslike::NativeFn { .. }, _)) => {
                unreachable!("name resolution prevents assigning to these addresses");
            }
            Some((Addresslike::CompoundField { index, of, .. }, type_index)) => {
                let index = *index;
                let of = *of;

                self.push_instruction(Instruction::AccessAssign {
                    index,
                    of: TypedValue {
                        value: Value::Address(of),
                        type_index,
                    },
                    value,
                });
            }
            Some((Addresslike::Address(address), type_index)) => {
                address.version += 1;

                let address = *address;

                self.push_instruction(Instruction::Assign {
                    value,
                    to: Value::Address(address),
                });

                self.addresses.insert(
                    span,
                    TypedAddresslike {
                        addresslike: Addresslike::Address(address),
                        type_index,
                    },
                );
            }
            Some((Addresslike::CallArgument(index), _)) => {
                let index = *index;

                self.push_instruction(Instruction::Assign {
                    value,
                    to: Value::CallArgument(index),
                });
            }
            None => {
                let type_index = value.type_index();

                let address = self.assign_value(value);

                self.addresses.insert(
                    span,
                    TypedAddresslike {
                        addresslike: Addresslike::Address(address),
                        type_index,
                    },
                );
            }
        }

        self.push_value(TypedValue {
            value: Value::Unit,
            type_index: types[ir[expr].resolved()],
        });

        Ok(())
    }

    fn translate_and(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, lhs, rhs): (ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(options, sources, (ir, types), lhs)?;

        let value = self.pop_value();

        let value_type_index = value.type_index();

        let address = self.assign_value(value);

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = self.next_block()?;

        if let Some(block) = self.get_block_mut(current_block) {
            block.terminator = Some(BlockTerminator::Branch {
                condition: TypedValue {
                    value: Value::Address(address),
                    type_index: value_type_index,
                },
                when_true: when_true_block,
                otherwise: BlockIndex(u32::MAX),
            });
        }

        self.translate_expr(options, sources, (ir, types), rhs)?;

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let rhs = self.pop_value();

        self.push_instruction(Instruction::Assign {
            value: rhs,
            to: Value::Address(address),
        });

        let after_all =
            BlockIndex(u32::try_from(self.blocks.len()).map_err(|_| Error::TooManyBlocks)?);

        if let Some(block) = self
            .current_block
            .and_then(|block_index| self.get_block_mut(block_index))
        {
            block.terminator = Some(BlockTerminator::Jump(after_all));
        }

        if let Some(BlockTerminator::Branch { otherwise, .. }) = self
            .get_block_mut(current_block)
            .and_then(|block| block.terminator.as_mut())
        {
            *otherwise = after_all;
        }

        self.last_in_fn = last_in_fn;

        self.next_block()?;

        self.push_value(TypedValue {
            value: Value::Address(Address {
                block_index: address.block_index,
                offset: address.offset,
                version: 0,
            }),
            type_index: types[ir[expr].resolved()],
        });

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn translate_or(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, lhs, rhs): (ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(options, sources, (ir, types), lhs)?;

        let value = self.pop_value();

        let value_type_index = value.type_index();

        let address = self.assign_value(value);

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let otherwise_block = self.next_block()?;

        if let Some(block) = self.get_block_mut(current_block) {
            block.terminator = Some(BlockTerminator::Branch {
                condition: TypedValue {
                    value: Value::Address(address),
                    type_index: value_type_index,
                },
                when_true: BlockIndex(u32::MAX),
                otherwise: otherwise_block,
            });
        }

        self.translate_expr(options, sources, (ir, types), rhs)?;

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let rhs = self.pop_value();

        self.push_instruction(Instruction::Assign {
            value: rhs,
            to: Value::Address(address),
        });

        let after_all =
            BlockIndex(u32::try_from(self.blocks.len()).map_err(|_| Error::TooManyBlocks)?);

        if let Some(block) = self
            .current_block
            .and_then(|block_index| self.get_block_mut(block_index))
        {
            block.terminator = Some(BlockTerminator::Jump(after_all));
        }

        if let Some(BlockTerminator::Branch { when_true, .. }) = self
            .get_block_mut(current_block)
            .and_then(|block| block.terminator.as_mut())
        {
            *when_true = after_all;
        }

        self.last_in_fn = last_in_fn;

        self.next_block()?;

        self.push_value(TypedValue {
            value: Value::Address(Address {
                block_index: address.block_index,
                offset: address.offset,
                version: 0,
            }),
            type_index: types[ir[expr].resolved()],
        });

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn translate_if(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, condition, when_true, otherwise): (ExprIndex, ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(options, sources, (ir, types), condition)?;

        let condition = self.pop_value();

        let address = self.assign_value(TypedValue {
            value: Value::Never,
            type_index: types[ir[expr].resolved()],
        });

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = self.next_block()?;

        if let Some(block) = self.get_block_mut(current_block) {
            block.terminator = Some(BlockTerminator::Branch {
                condition,
                when_true: when_true_block,
                otherwise: BlockIndex(u32::MAX),
            });
        }

        self.translate_expr(options, sources, (ir, types), when_true)?;

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let value = self.pop_value();

        self.push_instruction(Instruction::Assign {
            value,
            to: Value::Address(address),
        });

        let when_true_block = self
            .current_block
            .expect("if expressions can only exist in a block");

        let otherwise_block = self.next_block()?;

        if let BlockTerminator::Branch { otherwise, .. } = self
            .get_block_mut(current_block)
            .and_then(|block| block.terminator.as_mut())
            .expect("the destination to backpatch was set just before")
        {
            *otherwise = otherwise_block;
        }

        self.translate_expr(options, sources, (ir, types), otherwise)?;

        let address = Address {
            block_index: address.block_index,
            offset: address.offset,
            version: address.version + 1,
        };

        let value = self.pop_value();

        self.push_instruction(Instruction::Assign {
            value,
            to: Value::Address(address),
        });

        let after_all = self.next_block()?;

        if let Some(block) = self.get_block_mut(when_true_block)
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Jump(after_all));
        }

        self.last_in_fn = last_in_fn;

        self.push_value(TypedValue {
            value: Value::Address(Address {
                block_index: address.block_index,
                offset: address.offset,
                version: 0,
            }),
            type_index: types[ir[expr].resolved()],
        });

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn translate_call(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, callee, arguments): (ExprIndex, ExprIndex, &[ExprIndex]),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(options, sources, (ir, types), callee)?;

        let callee = self.pop_value();

        let mut error = Ok(());

        let arguments = arguments
            .iter()
            .copied()
            .map(|argument| {
                if let Err(real_error) =
                    self.translate_expr(options, sources, (ir, types), argument)
                {
                    error = Err(real_error);
                }

                self.pop_value()
            })
            .collect::<Vec<_>>();

        let mut arity = 0u8;

        for argument in arguments {
            self.push_instruction(Instruction::Push(argument));

            arity += 1;
        }

        let address = self.next_address();

        self.push_instruction(Instruction::Call {
            callee,
            arity,
            temporary: Value::Address(address),
        });

        self.push_value(TypedValue {
            value: Value::Address(address),
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        error
    }

    fn translate_while(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, condition, when_true): (ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let condition_block = self.next_block()?;

        self.translate_expr(options, sources, (ir, types), condition)?;

        let condition = self.pop_value();

        let current_block = self
            .current_block
            .expect("a block will exist if we're translating expressions");

        let when_true_block = self.next_block()?;

        if let Some(block) = self.get_block_mut(current_block) {
            block.terminator = Some(BlockTerminator::Branch {
                condition,
                when_true: when_true_block,
                otherwise: BlockIndex(u32::MAX),
            });
        }

        self.translate_expr(options, sources, (ir, types), when_true)?;

        let when_true_block = self
            .current_block
            .expect("while loops can only exist in a block");

        let otherwise_block = self.next_block()?;

        if let BlockTerminator::Branch { otherwise, .. } = self
            .get_block_mut(current_block)
            .and_then(|block| block.terminator.as_mut())
            .expect("the destination to backpatch was set just before")
        {
            *otherwise = otherwise_block;
        }

        if let Some(block) = self.get_block_mut(when_true_block)
            && block.terminator.is_none()
        {
            block.terminator = Some(BlockTerminator::Jump(condition_block));
        }

        self.last_in_fn = last_in_fn;

        self.pop_value();

        self.push_value(TypedValue {
            value: Value::Unit,
            type_index: types[ir[expr].resolved()],
        });

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn translate_product(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        expr: ExprIndex,
        fields: &[(Spanned<String>, ExprIndex)],
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let expr_span = ir[expr].span();

        let Type::Product {
            fields: type_fields,
            ..
        } = &types[types[expr_span]]
        else {
            unreachable!("type checking guarantees a product is a product");
        };

        let mut values = vec![];

        for field in type_fields {
            let Some((_, value)) = fields.iter().find(|(name, _)| name.kind() == field.name())
            else {
                unreachable!("type checking guarantees all fields are present and not duplicated");
            };

            self.translate_expr(options, sources, (ir, types), *value)?;

            values.push(self.pop_value());
        }

        self.push_value(TypedValue {
            value: Value::Compound(values),
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn translate_access(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, lhs, rhs): (ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let field_index = Self::find_field_index((ir, types), (lhs, rhs));

        self.translate_expr(options, sources, (ir, types), lhs)?;

        let lhs = self.pop_value();

        let address = self.next_address();

        self.push_instruction(Instruction::Access {
            index: field_index,
            of: lhs,
            temporary: Value::Address(address),
        });

        self.push_value(TypedValue {
            value: Value::Address(address),
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn find_field_index(
        (ir, types): (&Ir, &TypeChecker),
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) -> u8 {
        let lhs_span = ir[lhs].span();

        match &types[types[lhs_span]] {
            Type::Product {
                fields: type_fields,
                ..
            } => match ir[rhs].kind() {
                Expr::Name(name) => {
                    let Some(field_index) = type_fields
                        .iter()
                        .position(|field| field.name() == name)
                        .and_then(|index| u8::try_from(index).ok())
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
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, lhs, rhs, value): (ExprIndex, ExprIndex, ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let field_index = Self::find_field_index((ir, types), (lhs, rhs));

        self.translate_expr(options, sources, (ir, types), lhs)?;

        let lhs = self.pop_value();

        self.translate_expr(options, sources, (ir, types), value)?;

        let value = self.pop_value();

        self.push_instruction(Instruction::AccessAssign {
            index: field_index,
            of: lhs,
            value,
        });

        self.push_value(TypedValue {
            value: Value::Unit,
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn translate_path_access(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let original_lhs = lhs;

        let lhs = match ir[lhs].kind() {
            Expr::Name(_) | Expr::PathElement(_) => lhs,
            Expr::Binary {
                op: BinaryOp::PathAccess,
                rhs,
                ..
            } => match ir[*rhs].kind() {
                Expr::Name(_) | Expr::PathElement(_) => *rhs,
                _ => {
                    unreachable!(
                        "name resolution verifies the path is correct {:?}",
                        ir[*rhs].kind()
                    );
                }
            },
            _ => {
                unreachable!(
                    "name resolution verifies the path is correct {:?}",
                    ir[lhs].kind()
                );
            }
        };

        let type_span = match types.get_type(ir[lhs].resolved()) {
            Some(
                Type::Error
                | Type::Integer(_)
                | Type::NegativeInteger(_)
                | Type::Float(_)
                | Type::Existential(_)
                | Type::Generic(_)
                | Type::Fn { .. }
                | Type::Associations(_),
            )
            | None => None,
            Some(Type::Primitive { primitive, .. }) => Some(primitive.span()),
            Some(Type::Product { name, .. } | Type::Sum { name, .. }) => Some(name.span()),
        };

        match ir[rhs].kind() {
            Expr::Name(method_name) if let Some(type_span) = type_span => {
                if let Some(method_span) = ir[original_lhs]
                    .span()
                    .combine_with(ir[rhs].span())
                    .and_then(|span| types.resolve_association(type_span, method_name, span))
                {
                    let callee = match self
                        .addresses
                        .get(&method_span)
                        .expect("type checking guarantees the method exists")
                        .addresslike
                    {
                        Addresslike::Fn(block_index) => Value::Fn(block_index),
                        Addresslike::NativeFn { ref label } => Value::NativeFn {
                            label: label.clone(),
                        },
                        Addresslike::CallArgument(_)
                        | Addresslike::Address(_)
                        | Addresslike::CompoundField { .. } => {
                            unreachable!("methods are only ever functions or native functions");
                        }
                    };

                    self.push_value(TypedValue {
                        value: callee,
                        type_index: types[ir[rhs].resolved()],
                    });
                } else {
                    unreachable!("{type_span:?} {method_name:?} {:?}", ir[rhs].span());
                }
            }
            Expr::Product { name, .. }
                if let Some(type_span) = type_span
                    && let Some(Type::Sum {
                        variants: type_variants,
                        ..
                    }) = types.get_type(type_span) =>
            {
                let variant_index = match ir[rhs].kind() {
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

                self.translate_expr(options, sources, (ir, types), rhs)?;

                let TypedValue {
                    value: Value::Compound(fields),
                    ..
                } = self.pop_value()
                else {
                    unreachable!("type checking guarantees the right operand is a product literal");
                };

                self.push_value(TypedValue {
                    value: Value::TaggedCompound {
                        fields,
                        tag: variant_index,
                    },
                    type_index: types[type_span],
                });
            }
            _ => {
                self.translate_expr(options, sources, (ir, types), rhs)?;
            }
        }

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    fn translate_method_call(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, target, method, arguments): (ExprIndex, ExprIndex, ExprIndex, &[ExprIndex]),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let type_span = match &types[types[ir[target].span()]] {
            Type::Error
            | Type::Integer(_)
            | Type::NegativeInteger(_)
            | Type::Float(_)
            | Type::Existential(_)
            | Type::Generic(_)
            | Type::Fn { .. }
            | Type::Associations(_) => {
                unreachable!("type checking guarantees these types are not used for method calls");
            }
            Type::Primitive { primitive, .. } => primitive.span(),
            Type::Product { name, .. } | Type::Sum { name, .. } => name.span(),
        };

        let Expr::Name(method_name) = ir[method].kind() else {
            unreachable!("methods are only names");
        };

        let Some(method_span) =
            types.resolve_association(type_span, method_name, ir[method].span())
        else {
            unreachable!("type checking guarantees the method exists");
        };

        let callee = match self
            .addresses
            .get(&method_span)
            .expect("type checking guarantees the method exists")
            .addresslike
        {
            Addresslike::Fn(block_index) => Value::Fn(block_index),
            Addresslike::NativeFn { ref label } => Value::NativeFn {
                label: label.clone(),
            },
            Addresslike::CallArgument(_)
            | Addresslike::Address(_)
            | Addresslike::CompoundField { .. } => {
                unreachable!("methods are only ever functions or native functions");
            }
        };

        self.push_value(TypedValue {
            value: callee,
            type_index: types[ir[method].resolved()],
        });

        let mut error = Ok(());

        let arguments = iter::once(target)
            .chain(arguments.iter().copied())
            .map(|argument| {
                if let Err(real_error) =
                    self.translate_expr(options, sources, (ir, types), argument)
                {
                    error = Err(real_error);
                }

                self.pop_value()
            })
            .collect::<Vec<_>>();

        let mut arity = 0u8;

        for argument in arguments {
            self.push_instruction(Instruction::Push(argument));

            arity += 1;
        }

        let callee = self.pop_value();

        let address = self.next_address();

        self.push_instruction(Instruction::Call {
            callee,
            arity,
            temporary: Value::Address(address),
        });

        self.push_value(TypedValue {
            value: Value::Address(address),
            type_index: types[ir[expr].resolved()],
        });

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.emit_return();
        }

        error
    }

    fn translate_match(
        &mut self,
        options: &ApartOptions,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        (expr, match_against, cases, fallback): (ExprIndex, ExprIndex, &[MatchCase], ExprIndex),
    ) -> Result<(), Error> {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.translate_expr(options, sources, (ir, types), match_against)?;

        let value = self.pop_value();

        let expr_address = self.assign_value(value);

        let address = self.assign_value(TypedValue {
            value: Value::Never,
            type_index: types[ir[expr].resolved()],
        });

        let mut backpatch_successes = vec![];

        let mut address_version = address.version;

        for case in cases {
            let backpatch_these =
                self.translate_pattern(sources, (ir, types), expr_address, case.pattern())?;

            self.translate_expr(options, sources, (ir, types), case.case())?;

            let case_value = self.pop_value();

            address_version += 1;

            self.push_instruction(Instruction::Assign {
                value: case_value,
                to: Value::Address(Address {
                    block_index: address.block_index,
                    offset: address.offset,
                    version: address_version,
                }),
            });

            backpatch_successes.push(
                self.current_block
                    .expect("match expressions can only exist in blocks"),
            );

            let next_block = self.next_block()?;

            for backpatch in backpatch_these {
                if let Some(BlockTerminator::Branch { otherwise, .. }) =
                    self.get_block_mut(backpatch).map(Block::terminator_mut)
                {
                    *otherwise = next_block;
                }
            }
        }

        self.translate_expr(options, sources, (ir, types), fallback)?;

        let fallback_value = self.pop_value();

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

        self.next_block()?;

        let block_index = self
            .current_block
            .expect("match expressions can only exist in blocks");

        for backpatch in backpatch_successes {
            if let Some(block) = self.get_block_mut(backpatch)
                && block.terminator.is_none()
            {
                block.terminator = Some(BlockTerminator::Jump(block_index));
            }
        }

        self.last_in_fn = last_in_fn;

        self.push_value(TypedValue {
            value: Value::Address(Address {
                block_index: address.block_index,
                offset: address.offset,
                version: 0,
            }),
            type_index: types[ir[expr].resolved()],
        });

        if self.last_in_fn {
            self.emit_return();
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn translate_pattern(
        &mut self,
        sources: &[(usize, (&str, &str))],
        (ir, types): (&Ir, &TypeChecker),
        match_against: Address,
        pattern: &Resolved<Pattern>,
    ) -> Result<Vec<BlockIndex>, Error> {
        match &types[types[pattern.span()]] {
            Type::Integer(_)
            | Type::NegativeInteger(_)
            | Type::Float(_)
            | Type::Error
            | Type::Fn { .. }
            | Type::Existential(_)
            | Type::Generic(_)
            | Type::Associations(_) => unreachable!(
                "only integer primitives, booleans, unit, strings, and products can be patterns"
            ),
            Type::Primitive { primitive, .. } => {
                let value = match (primitive.kind(), pattern.kind()) {
                    (_, Pattern::Integer(radix)) => {
                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == pattern.span().source_id())
                            .and_then(|(_, (_, source))| pattern.span().lexeme(source))
                            .expect("the span is from this source")
                            .replace('_', "");

                        let lexeme = lexeme.as_str();

                        match primitive.kind() {
                            Primitive::U8 => Value::U8(
                                u8::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I8 => Value::I8(
                                i8::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::U16 => Value::U16(
                                u16::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I16 => Value::I16(
                                i16::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::U32 => Value::U32(
                                u32::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I32 => Value::I32(
                                i32::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::U64 => Value::U64(
                                u64::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I64 => Value::I64(
                                i64::from_str_radix(lexeme, *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            _ => unreachable!("type checking guarantees integers are integers"),
                        }
                    }
                    (_, Pattern::NegativeInteger(radix)) => {
                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == pattern.span().source_id())
                            .and_then(|(_, (_, source))| pattern.span().lexeme(source))
                            .expect("the span is from this source")
                            .replace('_', "");

                        let lexeme = lexeme.as_str();

                        match primitive.kind() {
                            Primitive::I8 => Value::I8(
                                i8::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I16 => Value::I16(
                                i16::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I32 => Value::I32(
                                i32::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            Primitive::I64 => Value::I64(
                                i64::from_str_radix(format!("-{lexeme}").as_str(), *radix)
                                    .expect("type checking guarantees the conversion is valid"),
                            ),
                            _ => unreachable!("type checking guarantees integers are integers"),
                        }
                    }
                    (Primitive::Boolean, Pattern::Boolean(value)) => Value::Boolean(*value),
                    (Primitive::Unit, Pattern::Unit) => Value::Unit,
                    (Primitive::String, Pattern::String(text)) => Value::String(text.clone()),
                    _ => {
                        unreachable!("type checking guarantees no other combinations make it here")
                    }
                };

                self.push_value(TypedValue {
                    value,
                    type_index: types[pattern.resolved()],
                });

                let value = self.pop_value();

                let condition_address = self.next_address();

                self.push_instruction(Instruction::Binary {
                    op: BinaryOp::Equal,
                    lhs: TypedValue {
                        value: Value::Address(match_against),
                        type_index: types[pattern.resolved()],
                    },
                    rhs: value,
                    temporary: Value::Address(condition_address),
                });

                let current_block = self
                    .current_block
                    .expect("a block will exist if we're translating expressions");

                let when_true_block = self.next_block()?;

                if let Some(block) = self.get_block_mut(current_block) {
                    block.terminator = Some(BlockTerminator::Branch {
                        condition: TypedValue {
                            value: Value::Address(condition_address),
                            type_index: types.type_boolean(),
                        },
                        when_true: when_true_block,
                        otherwise: BlockIndex(u32::MAX),
                    });
                }

                Ok(vec![current_block])
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
                            .position(|field_type| field_type.name() == field_name.kind())
                            .and_then(|index| u8::try_from(index).ok())
                            .expect("all fields in the pattern exist in the type since type checking succeeded");

                        self.push_instruction(Instruction::Access {
                            index: field_index,
                            of: TypedValue {
                                value: Value::Address(match_against),
                                type_index: types[field_name.resolved()],
                            },
                            temporary: Value::Address(field_address),
                        });

                        if let Some(field) = field {
                            to_backpatch.append(&mut self.translate_pattern(
                                sources,
                                (ir, types),
                                field_address,
                                field,
                            )?);
                        } else {
                            self.addresses.insert(
                                field_name.span(),
                                TypedAddresslike {
                                    addresslike: Addresslike::CompoundField {
                                        index: field_index,
                                        of: match_against,
                                        of_type_index: types[pattern.resolved()],
                                    },
                                    type_index: types[field_name.resolved()],
                                },
                            );
                        }
                    }

                    Ok(to_backpatch)
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
                    let Some(PathElement::Name(name)) = path.last().map(Resolved::kind) else {
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

                    self.push_value(TypedValue {
                        value: Value::U16(variant_index),
                        type_index: types.type_u16(),
                    });

                    let variant_index = self.pop_value();

                    let tag_address = self.next_address();

                    self.push_instruction(Instruction::GetTag {
                        of: TypedValue {
                            value: Value::Address(match_against),
                            type_index: types[pattern.resolved()],
                        },
                        temporary: Value::Address(tag_address),
                    });

                    let condition_address = self.next_address();

                    self.push_instruction(Instruction::Binary {
                        op: BinaryOp::Equal,
                        lhs: TypedValue {
                            value: Value::Address(tag_address),
                            type_index: types.type_u16(),
                        },
                        rhs: variant_index,
                        temporary: Value::Address(condition_address),
                    });

                    let current_block = self
                        .current_block
                        .expect("a block will exist if we're translating expressions");

                    let when_true_block = self.next_block()?;

                    if let Some(block) = self.get_block_mut(current_block) {
                        block.terminator = Some(BlockTerminator::Branch {
                            condition: TypedValue {
                                value: Value::Address(condition_address),
                                type_index: types.type_boolean(),
                            },
                            when_true: when_true_block,
                            otherwise: BlockIndex(u32::MAX),
                        });
                    }

                    let mut to_backpatch = vec![current_block];

                    for (field_name, field) in fields {
                        let field_address = self.next_address();

                        let field_index = field_types.iter()
                            .position(|field_type| field_type.name() == field_name.kind())
                            .and_then(|index| u8::try_from(index).ok())
                            .expect("all fields in the pattern exist in the type since type checking succeeded");

                        self.push_instruction(Instruction::Access {
                            index: field_index,
                            of: TypedValue {
                                value: Value::Address(match_against),
                                type_index: types[pattern.resolved()],
                            },
                            temporary: Value::Address(field_address),
                        });

                        if let Some(field) = field {
                            to_backpatch.append(&mut self.translate_pattern(
                                sources,
                                (ir, types),
                                field_address,
                                field,
                            )?);
                        } else {
                            self.addresses.insert(
                                field_name.span(),
                                TypedAddresslike {
                                    addresslike: Addresslike::CompoundField {
                                        index: field_index,
                                        of: match_against,
                                        of_type_index: types[pattern.resolved()],
                                    },
                                    type_index: types[field_name.resolved()],
                                },
                            );
                        }
                    }

                    Ok(to_backpatch)
                }
            },
        }
    }
}
