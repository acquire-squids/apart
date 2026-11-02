use crate::{
    Span, Spanned,
    name_resolve::{Associations, Definitions, Names},
    parse::{
        Ast, Expr as AstExpr, ExprIndex as AstExprIndex, Item as AstItem,
        ItemIndex as AstItemIndex, Pattern as AstPattern, TypeSignature as AstTypeSignature,
    },
};

pub use crate::{
    name_resolve::{Definition, ModuleId, ROOT_MODULE},
    parse::{BinaryOp, PathElement, UnaryOp, Visibility},
};

use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

pub struct Ir {
    exprs: Vec<Resolved<Expr>>,
    items: Vec<Spanned<Item>>,
    roots: Vec<ItemIndex>,
    associations: Associations,
    definitions: Definitions,
}

#[derive(Debug, Clone)]
pub struct Resolved<T> {
    kind: Spanned<T>,
    resolved: Span,
}

impl<T> Resolved<T> {
    #[allow(dead_code)]
    #[must_use]
    pub const fn new(kind: Spanned<T>, resolved: Span) -> Self {
        Self { kind, resolved }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn kind(&self) -> &T {
        self.kind.kind()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn span(&self) -> Span {
        self.kind.span()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn resolved(&self) -> Span {
        self.resolved
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    name: Spanned<String>,
    type_signature: Resolved<TypeSignature>,
    visibility: Visibility,
}

impl Field {
    #[allow(dead_code)]
    #[must_use]
    const fn new(
        name: Spanned<String>,
        type_signature: Resolved<TypeSignature>,
        visibility: Visibility,
    ) -> Self {
        Self {
            name,
            type_signature,
            visibility,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn name(&self) -> &Spanned<String> {
        &self.name
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn ty(&self) -> &Resolved<TypeSignature> {
        &self.type_signature
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn visibility(&self) -> Visibility {
        self.visibility
    }
}

#[derive(Debug, Clone)]
pub struct Parameter {
    name: Resolved<String>,
    type_signature: Resolved<TypeSignature>,
}

impl Parameter {
    #[allow(dead_code)]
    #[must_use]
    const fn new(name: Resolved<String>, type_signature: Resolved<TypeSignature>) -> Self {
        Self {
            name,
            type_signature,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn name(&self) -> &Resolved<String> {
        &self.name
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn ty(&self) -> &Resolved<TypeSignature> {
        &self.type_signature
    }
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Integer(u32),
    NegativeInteger(u32),
    Boolean(bool),
    Unit,
    Product {
        path: Vec<Resolved<PathElement>>,
        fields: Vec<(Resolved<String>, Option<Resolved<Self>>)>,
    },
    String(String),
}

#[derive(Debug, Clone)]
pub enum TypeSignature {
    Path {
        path: Vec<Resolved<PathElement>>,
        generics: Vec<Resolved<Self>>,
    },
    SelfTy,
    Fn {
        parameters: Vec<Resolved<Self>>,
        return_type: Box<Resolved<Self>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemIndex(usize);

impl From<ItemIndex> for usize {
    fn from(value: ItemIndex) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExprIndex(usize);

impl From<ExprIndex> for usize {
    fn from(value: ExprIndex) -> Self {
        value.0
    }
}

impl Index<ExprIndex> for &Ir {
    type Output = Resolved<Expr>;

    fn index(&self, index: ExprIndex) -> &Self::Output {
        self.exprs.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "expression index out of bounds: the length is {} but the index is {index:?}",
                self.exprs.len(),
            );
        })
    }
}

impl Index<ItemIndex> for &Ir {
    type Output = Spanned<Item>;

    fn index(&self, index: ItemIndex) -> &Self::Output {
        self.items.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "item index out of bounds: the length is {} but the index is {index:?}",
                self.items.len(),
            );
        })
    }
}

impl Index<ExprIndex> for &mut Ir {
    type Output = Resolved<Expr>;

    fn index(&self, index: ExprIndex) -> &Self::Output {
        self.exprs.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "expression index out of bounds: the length is {} but the index is {index:?}",
                self.exprs.len(),
            );
        })
    }
}

impl Index<ItemIndex> for &mut Ir {
    type Output = Spanned<Item>;

    fn index(&self, index: ItemIndex) -> &Self::Output {
        self.items.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "item index out of bounds: the length is {} but the index is {index:?}",
                self.items.len(),
            );
        })
    }
}

impl IndexMut<ExprIndex> for &mut Ir {
    fn index_mut(&mut self, index: ExprIndex) -> &mut <Self as Index<ExprIndex>>::Output {
        let len = self.exprs.len();

        self.exprs.get_mut(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "expression index out of bounds: the length is {len} but the index is {index:?}"
            );
        })
    }
}

impl IndexMut<ItemIndex> for &mut Ir {
    fn index_mut(&mut self, index: ItemIndex) -> &mut <Self as Index<ItemIndex>>::Output {
        let len = self.items.len();

        self.items.get_mut(usize::from(index)).unwrap_or_else(|| {
            panic!("item index out of bounds: the length is {len} but the index is {index:?}");
        })
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Never,
    Integer(u32),
    NegativeInteger(u32),
    Float,
    Boolean(bool),
    Unit,
    Unary {
        op: UnaryOp,
        operand: ExprIndex,
    },
    Binary {
        op: BinaryOp,
        lhs: ExprIndex,
        rhs: ExprIndex,
    },
    Block(Vec<ExprIndex>),
    Name(String),
    Let {
        name: Spanned<String>,
        type_signature: Option<Resolved<TypeSignature>>,
        value: ExprIndex,
    },
    If {
        condition: ExprIndex,
        when_true: ExprIndex,
        otherwise: ExprIndex,
    },
    Call {
        callee: ExprIndex,
        arguments: Vec<ExprIndex>,
    },
    MethodCall {
        target: ExprIndex,
        method: ExprIndex,
        arguments: Vec<ExprIndex>,
    },
    While {
        condition: ExprIndex,
        when_true: ExprIndex,
    },
    Return(ExprIndex),
    AsUnit(ExprIndex),
    Product {
        name: Resolved<String>,
        fields: Vec<(Spanned<String>, ExprIndex)>,
    },
    PathElement(PathElement),
    Match {
        expr: ExprIndex,
        cases: Vec<MatchCase>,
        fallback: ExprIndex,
    },
    String(String),
}

#[derive(Debug, Clone)]
pub struct MatchCase {
    pattern: Resolved<Pattern>,
    case: ExprIndex,
}

impl MatchCase {
    pub const fn pattern(&self) -> &Resolved<Pattern> {
        &self.pattern
    }

    pub const fn case(&self) -> ExprIndex {
        self.case
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Primitive {
        name: Spanned<String>,
        generics: Vec<Spanned<String>>,
    },
    NativeFn {
        name: Spanned<String>,
        signature: Resolved<TypeSignature>,
    },
    Fn {
        name: Spanned<String>,
        parameters: Vec<Parameter>,
        return_type: Resolved<TypeSignature>,
        generics: Vec<Spanned<String>>,
        body: ExprIndex,
    },
    Product {
        name: Spanned<String>,
        fields: Vec<Field>,
        generics: Vec<Spanned<String>>,
    },
    Sum {
        name: Spanned<String>,
        variants: Vec<ItemIndex>,
        generics: Vec<Spanned<String>>,
    },
    Mod {
        name: Spanned<String>,
        generics: Vec<Spanned<String>>,
        contents: Vec<ItemIndex>,
    },
    Teach {
        student: Resolved<TypeSignature>,
        body: Vec<ItemIndex>,
        generics: Vec<Spanned<String>>,
    },
    Alias {
        alias: Spanned<String>,
        generics: Vec<Spanned<String>>,
        target: Resolved<TypeSignature>,
    },
}

pub fn lower(ast: &Ast, names: Names) -> Ir {
    let mut hir = Ir {
        exprs: vec![],
        items: vec![],
        roots: vec![],
        associations: HashMap::new(),
        definitions: HashMap::new(),
    };

    for root in ast.roots() {
        if let Some(item) = lower_item(ast, &names, &mut hir, *root) {
            hir.roots.push(item);
        }
    }

    (hir.associations, hir.definitions) = names.take();

    hir
}

fn resolved<T>(names: &Names, spanned: &Spanned<T>) -> Resolved<T>
where
    T: Clone,
{
    Resolved::new(
        Spanned::new(spanned.kind().clone(), spanned.span()),
        names.get(spanned.span()).unwrap_or_else(|| spanned.span()),
    )
}

fn resolved_type_signature(
    names: &Names,
    ty: &Spanned<AstTypeSignature>,
) -> Resolved<TypeSignature> {
    Resolved::new(
        match ty.kind() {
            AstTypeSignature::Path { path, generics } => Spanned::new(
                TypeSignature::Path {
                    path: path
                        .iter()
                        .map(|path_element| resolved(names, path_element))
                        .collect::<Vec<_>>(),
                    generics: generics
                        .iter()
                        .map(|generic| resolved_type_signature(names, generic))
                        .collect::<Vec<_>>(),
                },
                ty.span(),
            ),
            AstTypeSignature::SelfTy => Spanned::new(TypeSignature::SelfTy, ty.span()),
            AstTypeSignature::Fn {
                parameters,
                return_type,
            } => Spanned::new(
                TypeSignature::Fn {
                    parameters: parameters
                        .iter()
                        .map(|parameter| resolved_type_signature(names, parameter))
                        .collect::<Vec<_>>(),
                    return_type: Box::new(resolved_type_signature(names, return_type)),
                },
                ty.span(),
            ),
        },
        names.get(ty.span()).unwrap_or_else(|| ty.span()),
    )
}

fn resolved_pattern(names: &Names, pattern: &Spanned<AstPattern>) -> Resolved<Pattern> {
    Resolved::new(
        match pattern.kind() {
            AstPattern::Integer(radix) => Spanned::new(Pattern::Integer(*radix), pattern.span()),
            AstPattern::NegativeInteger(radix) => {
                Spanned::new(Pattern::NegativeInteger(*radix), pattern.span())
            }
            AstPattern::Boolean(value) => Spanned::new(Pattern::Boolean(*value), pattern.span()),
            AstPattern::Unit => Spanned::new(Pattern::Unit, pattern.span()),
            AstPattern::String(text) => Spanned::new(Pattern::String(text.clone()), pattern.span()),
            AstPattern::Product { path, fields } => Spanned::new(
                Pattern::Product {
                    path: path
                        .iter()
                        .map(|path_element| resolved(names, path_element))
                        .collect::<Vec<_>>(),
                    fields: fields
                        .iter()
                        .map(|(field_name, pattern)| {
                            (
                                resolved(names, field_name),
                                pattern
                                    .as_ref()
                                    .map(|pattern| resolved_pattern(names, pattern)),
                            )
                        })
                        .collect::<Vec<_>>(),
                },
                pattern.span(),
            ),
        },
        names.get(pattern.span()).unwrap_or_else(|| pattern.span()),
    )
}

#[allow(clippy::too_many_lines)]
fn lower_item(ast: &Ast, names: &Names, hir: &mut Ir, item: AstItemIndex) -> Option<ItemIndex> {
    match ast[item].kind() {
        AstItem::Directive { .. } => None,
        AstItem::Primitive { name, generics } => Some(hir.push_item(Spanned::new(
            Item::Primitive {
                name: name.clone(),
                generics: generics.clone(),
            },
            ast[item].span(),
        ))),
        AstItem::NativeFn {
            name, signature, ..
        } => Some(hir.push_item(Spanned::new(
            Item::NativeFn {
                name: name.clone(),
                signature: resolved_type_signature(names, signature),
            },
            ast[item].span(),
        ))),
        AstItem::Fn {
            name,
            parameters,
            return_type,
            generics,
            body,
            ..
        } => {
            let body = lower_expr(ast, names, hir, *body);

            Some(
                hir.push_item(Spanned::new(
                    Item::Fn {
                        name: name.clone(),
                        parameters: parameters
                            .iter()
                            .map(|parameter| {
                                Parameter::new(
                                    resolved(names, parameter.name()),
                                    resolved_type_signature(names, parameter.ty()),
                                )
                            })
                            .collect::<Vec<_>>(),
                        return_type: resolved_type_signature(names, return_type),
                        generics: generics.clone(),
                        body,
                    },
                    ast[item].span(),
                )),
            )
        }
        AstItem::Product {
            name,
            fields,
            generics,
            ..
        } => Some(
            hir.push_item(Spanned::new(
                Item::Product {
                    name: name.clone(),
                    fields: fields
                        .iter()
                        .map(|field| {
                            Field::new(
                                field.name().clone(),
                                resolved_type_signature(names, field.ty()),
                                field.visibility(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    generics: generics.clone(),
                },
                ast[item].span(),
            )),
        ),
        AstItem::Sum {
            name,
            generics,
            variants,
            ..
        } => {
            let variants = variants
                .iter()
                .filter_map(|variant| lower_item(ast, names, hir, *variant))
                .collect::<Vec<_>>();

            Some(hir.push_item(Spanned::new(
                Item::Sum {
                    name: name.clone(),
                    generics: generics.clone(),
                    variants,
                },
                ast[item].span(),
            )))
        }
        AstItem::Mod {
            name,
            generics,
            contents,
            ..
        } => {
            let contents = contents
                .iter()
                .filter_map(|content| lower_item(ast, names, hir, *content))
                .collect::<Vec<_>>();

            Some(hir.push_item(Spanned::new(
                Item::Mod {
                    name: name.clone(),
                    generics: generics.clone(),
                    contents,
                },
                ast[item].span(),
            )))
        }
        AstItem::Teach {
            student,
            body,
            generics,
        } => {
            let body = body
                .iter()
                .filter_map(|body| lower_item(ast, names, hir, *body))
                .collect::<Vec<_>>();

            Some(hir.push_item(Spanned::new(
                Item::Teach {
                    student: resolved_type_signature(names, student),
                    body,
                    generics: generics.clone(),
                },
                ast[item].span(),
            )))
        }
        AstItem::Alias {
            alias,
            generics,
            target,
            ..
        } => Some(hir.push_item(Spanned::new(
            Item::Alias {
                alias: alias.clone(),
                generics: generics.clone(),
                target: resolved_type_signature(names, target),
            },
            ast[item].span(),
        ))),
    }
}

#[allow(clippy::too_many_lines)]
fn lower_expr(ast: &Ast, names: &Names, hir: &mut Ir, expr: AstExprIndex) -> ExprIndex {
    let spanned = Spanned::new(
        match ast[expr].kind() {
            AstExpr::BinaryNoLhs { .. }
            | AstExpr::CallNoCallee { .. }
            | AstExpr::AsUnitNoValue
            | AstExpr::ProductNoName { .. } => {
                unreachable!("these variants don't exist after parsing successfully finishes")
            }
            AstExpr::Never => Expr::Never,
            AstExpr::Integer(radix) => Expr::Integer(*radix),
            AstExpr::NegativeInteger(radix) => Expr::NegativeInteger(*radix),
            AstExpr::Float => Expr::Float,
            AstExpr::Boolean(value) => Expr::Boolean(*value),
            AstExpr::Unit => Expr::Unit,
            AstExpr::Unary { op, expr } => Expr::Unary {
                op: *op,
                operand: lower_expr(ast, names, hir, *expr),
            },
            AstExpr::Binary { op, lhs, rhs } => Expr::Binary {
                op: *op,
                lhs: lower_expr(ast, names, hir, *lhs),
                rhs: lower_expr(ast, names, hir, *rhs),
            },
            AstExpr::Group(expr) => return lower_expr(ast, names, hir, *expr),
            AstExpr::Block(exprs) if exprs.is_empty() => Expr::Unit,
            AstExpr::Block(exprs) if exprs.len() == 1 => {
                return lower_expr(
                    ast,
                    names,
                    hir,
                    exprs
                        .first()
                        .copied()
                        .expect("the if guard guarantees the expression exists"),
                );
            }
            AstExpr::Block(exprs) => Expr::Block(
                exprs
                    .iter()
                    .map(|expr| lower_expr(ast, names, hir, *expr))
                    .collect::<Vec<_>>(),
            ),
            AstExpr::Name(name) => {
                return hir.push_expr(Resolved::new(
                    Spanned::new(Expr::Name(name.clone()), ast[expr].span()),
                    names
                        .get(ast[expr].span())
                        .unwrap_or_else(|| ast[expr].span()),
                ));
            }
            AstExpr::Let {
                name,
                type_signature,
                value,
            } => Expr::Let {
                name: name.clone(),
                type_signature: type_signature
                    .as_ref()
                    .map(|type_signature| resolved_type_signature(names, type_signature)),
                value: lower_expr(ast, names, hir, *value),
            },
            AstExpr::If {
                condition,
                when_true,
                otherwise,
            } => Expr::If {
                condition: lower_expr(ast, names, hir, *condition),
                when_true: lower_expr(ast, names, hir, *when_true),
                otherwise: lower_expr(ast, names, hir, *otherwise),
            },
            AstExpr::Call { callee, arguments } => Expr::Call {
                callee: lower_expr(ast, names, hir, *callee),
                arguments: arguments
                    .iter()
                    .map(|expr| lower_expr(ast, names, hir, *expr))
                    .collect::<Vec<_>>(),
            },
            AstExpr::MethodCall {
                target,
                method,
                arguments,
            } => Expr::MethodCall {
                target: lower_expr(ast, names, hir, *target),
                method: lower_expr(ast, names, hir, *method),
                arguments: arguments
                    .iter()
                    .map(|expr| lower_expr(ast, names, hir, *expr))
                    .collect::<Vec<_>>(),
            },
            AstExpr::While {
                condition,
                when_true,
            } => Expr::While {
                condition: lower_expr(ast, names, hir, *condition),
                when_true: lower_expr(ast, names, hir, *when_true),
            },
            AstExpr::Return(expr) => Expr::Return(lower_expr(ast, names, hir, *expr)),
            AstExpr::AsUnit(expr) => Expr::AsUnit(lower_expr(ast, names, hir, *expr)),
            AstExpr::Product { name, fields } => Expr::Product {
                name: resolved(names, name),
                fields: fields
                    .iter()
                    .map(|(field_name, field)| {
                        (field_name.clone(), lower_expr(ast, names, hir, *field))
                    })
                    .collect::<Vec<_>>(),
            },
            AstExpr::PathElement(path_element) => Expr::PathElement(path_element.clone()),
            AstExpr::Match {
                expr,
                cases,
                fallback,
            } => Expr::Match {
                expr: lower_expr(ast, names, hir, *expr),
                cases: cases
                    .iter()
                    .map(|case| MatchCase {
                        pattern: resolved_pattern(names, case.pattern()),
                        case: lower_expr(ast, names, hir, case.case()),
                    })
                    .collect::<Vec<_>>(),
                fallback: lower_expr(ast, names, hir, *fallback),
            },
            AstExpr::String(text) => Expr::String(text.clone()),
        },
        ast[expr].span(),
    );

    hir.push_expr(Resolved::new(
        spanned,
        names
            .get(ast[expr].span())
            .unwrap_or_else(|| ast[expr].span()),
    ))
}

impl Ir {
    pub fn new() -> Self {
        Self {
            exprs: vec![],
            items: vec![],
            roots: vec![],
            associations: HashMap::new(),
            definitions: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn roots(&self) -> &[ItemIndex] {
        self.roots.as_slice()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_association(&self, of: Span, associated_name: &str) -> Option<&[Span]> {
        self.associations
            .get(&of)
            .and_then(|associations| associations.get(associated_name))
            .map(Vec::as_slice)
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_definition(&self, span: Span) -> Option<&Definition> {
        self.definitions.get(&span)
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn take(self) -> (Associations, Definitions) {
        (self.associations, self.definitions)
    }

    fn push_item(&mut self, item: Spanned<Item>) -> ItemIndex {
        let index = ItemIndex(self.items.len());

        self.items.push(item);

        index
    }

    fn push_expr(&mut self, expr: Resolved<Expr>) -> ExprIndex {
        let index = ExprIndex(self.exprs.len());

        self.exprs.push(expr);

        index
    }

    #[allow(dead_code)]
    pub fn for_children_exprs<F>(&self, expr: ExprIndex, mut f: F)
    where
        F: FnMut(&Self, ExprIndex),
    {
        match self[expr].kind() {
            Expr::Integer(_)
            | Expr::NegativeInteger(_)
            | Expr::Float
            | Expr::Boolean(_)
            | Expr::Unit
            | Expr::Name(_)
            | Expr::PathElement(_)
            | Expr::String(_)
            | Expr::Never => {}
            Expr::Unary { operand, .. } => {
                f(self, *operand);
            }
            Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                ..
            } => {
                f(self, *lhs);
            }
            Expr::Binary { lhs, rhs, .. } => {
                f(self, *lhs);
                f(self, *rhs);
            }
            Expr::Block(exprs) => {
                for expr in exprs {
                    f(self, *expr);
                }
            }
            Expr::Let { value, .. } | Expr::Return(value) | Expr::AsUnit(value) => {
                f(self, *value);
            }
            Expr::If {
                condition,
                when_true,
                otherwise,
            } => {
                f(self, *condition);
                f(self, *when_true);
                f(self, *otherwise);
            }
            Expr::Call { callee, arguments } => {
                f(self, *callee);

                for argument in arguments {
                    f(self, *argument);
                }
            }
            Expr::MethodCall {
                target, arguments, ..
            } => {
                f(self, *target);

                for argument in arguments {
                    f(self, *argument);
                }
            }
            Expr::While {
                condition,
                when_true,
            } => {
                f(self, *condition);
                f(self, *when_true);
            }
            Expr::Product { fields, .. } => {
                for (_, value) in fields {
                    f(self, *value);
                }
            }
            Expr::Match {
                expr,
                cases,
                fallback,
            } => {
                f(self, *expr);

                for case in cases {
                    f(self, case.case);
                }

                f(self, *fallback);
            }
        }
    }
}
