use crate::{
    Reportable, Span, Spanned,
    lex::{Lexer, Token},
    token_tree::{self, Forest, TokenTree, TokenTreeIndex, TreeKind},
};

use std::{
    error, fmt, mem,
    ops::{Index, IndexMut},
};

pub fn parse(
    forest: &Forest,
    (source, source_id): (&str, usize),
    ast: &mut Ast,
) -> Result<(), Vec<Spanned<Error>>> {
    let mut parser = Parser::new(forest, (source, source_id), false);

    parser.parse(ast)
}

struct Parser<'s, 'p> {
    errors: Vec<Spanned<Error>>,
    forest: &'p Forest,
    current_tree: TokenTreeIndex,
    at: usize,
    source: &'s str,
    source_id: usize,
    is_core: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    ExpectedExpr,
    InvalidInteger,
    InvalidFloat,
    BlockWithoutSemicolon,
    InvalidName,
    NameIsKeyword,
    LetInWithoutEqual,
    LetInWithoutIn,
    LetInWithoutBlock,
    IfThenWithoutBlock,
    IfElseWithoutBlock,
    ExpectedItem,
    FnWithoutName,
    FnWithoutParameters,
    FnParameterWithoutName,
    FnParameterWithoutLeftArrow,
    FnWithoutBody,
    CallWithoutComma,
    FnParametersWithoutComma,
    FnTypeParametersWithoutComma,
    FnTypeWithoutParameters,
    LetInWithoutComma,
    ExpectedType,
    PrimitiveWithoutName,
    PrimitiveWithoutSemicolon,
    NativeFnWithoutName,
    NativeFnWithoutEqual,
    NativeFnWithoutType,
    ExpectedNativeItem,
    UnknownNativeItem,
    NativeFnWithoutSemicolon,
    WhileWithoutBlock,
    IfWithoutCondition,
    WhileWithoutCondition,
    ReturnWithoutValue,
    GenericWithoutName,
    GenericsWithoutComma,
    ProductWithoutName,
    ProductWithoutFields,
    ProductFieldWithoutName,
    ProductFieldWithoutLeftArrow,
    ProductFieldsWithoutComma,
    SumWithoutName,
    SumWithoutVariants,
    SumVariantWithoutName,
    SumVariantWithoutFields,
    SumVariantFieldWithoutName,
    SumVariantFieldWithoutLeftArrow,
    SumVariantFieldsWithoutComma,
    SumVariantsWithoutComma,
    ModWithoutName,
    ModWithoutBody,
    TeachWithoutBody,
    InvalidAssociatedItem,
    ExpectedBlock,
    ExpectedCallArguments,
}

impl fmt::Display for Error {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedExpr => write!(f, "expected an expression"),
            Self::InvalidInteger => write!(
                f,
                "this integer is invalid because it doesn't fit within 64 bits unsigned"
            ),
            Self::InvalidFloat => write!(f, "this integer is not a valid f64"),
            Self::BlockWithoutSemicolon => write!(
                f,
                "if there is another expression here, a semicolon should be between it and the previous expression"
            ),
            Self::InvalidName => write!(f, "expected a name"),
            Self::NameIsKeyword => {
                write!(f, "this cannot be used as a name because it is a keyword")
            }
            Self::LetInWithoutEqual => {
                write!(
                    f,
                    "expected an equal sign between the variable name and its value"
                )
            }
            Self::LetInWithoutIn => write!(f, "expected \"in\" before the let expression's body"),
            Self::LetInWithoutBlock => write!(f, "the body of a let expression must be a block"),
            Self::IfThenWithoutBlock => {
                write!(f, "the \"then\" branch of an if expression must be a block")
            }
            Self::IfElseWithoutBlock => {
                write!(
                    f,
                    "the \"else\" branch of an if expression must be a block or another if expression"
                )
            }
            Self::ExpectedItem => {
                write!(f, "expected an item")
            }
            Self::FnWithoutName => {
                write!(f, "expected a function name")
            }
            Self::FnWithoutParameters => {
                write!(
                    f,
                    "a list of zero or more function parameters must be provided within parentheses"
                )
            }
            Self::FnParameterWithoutName => {
                write!(f, "expected a function parameter name")
            }
            Self::FnParameterWithoutLeftArrow => {
                write!(
                    f,
                    "expected a \"<-\" after the function parameter name, and then a type signature"
                )
            }
            Self::FnWithoutBody => {
                write!(
                    f,
                    "expected a block expression to serve as the function body"
                )
            }
            Self::CallWithoutComma => write!(
                f,
                "if there is another expression here, a comma should be between it and the previous expression"
            ),
            Self::FnParametersWithoutComma | Self::FnTypeParametersWithoutComma => write!(
                f,
                "if there is another parameter here, a comma should be between it and the previous parameter"
            ),
            Self::FnTypeWithoutParameters => {
                write!(
                    f,
                    "a list of zero or more function parameters types must be provided within parentheses"
                )
            }
            Self::LetInWithoutComma => {
                write!(
                    f,
                    "if there is another variable name here, a comma should between its name and the previous value"
                )
            }
            Self::ExpectedType => write!(f, "expected a type signature"),
            Self::PrimitiveWithoutName => write!(f, "expected a primitive name"),
            Self::PrimitiveWithoutSemicolon => {
                write!(f, "there should be a semicolon after the primitive name")
            }
            Self::NativeFnWithoutName => {
                write!(f, "expected a native function name")
            }
            Self::NativeFnWithoutEqual => {
                write!(
                    f,
                    "there should be an equal sign after the native function name"
                )
            }
            Self::NativeFnWithoutType => {
                write!(f, "expected the type signature of the native function")
            }
            Self::ExpectedNativeItem => {
                write!(f, "expected a native item")
            }
            Self::UnknownNativeItem => {
                write!(f, "native items are only functions")
            }
            Self::NativeFnWithoutSemicolon => {
                write!(
                    f,
                    "there should be a semicolon after the native function signature"
                )
            }
            Self::IfWithoutCondition => {
                write!(f, "expected a condition for the if expression")
            }
            Self::WhileWithoutCondition => {
                write!(f, "expected a condition for the while loop")
            }
            Self::WhileWithoutBlock => {
                write!(f, "the body of a while loop must be a block")
            }
            Self::ReturnWithoutValue => {
                write!(f, "there must be a value to return")
            }
            Self::GenericWithoutName => {
                write!(f, "expected the name of a generic")
            }
            Self::GenericsWithoutComma => {
                write!(
                    f,
                    "if there is another generic here, a comma should be between it and the previous generic"
                )
            }
            Self::ProductWithoutName => {
                write!(f, "expected a product name")
            }
            Self::ProductWithoutFields => {
                write!(
                    f,
                    "the product's fields should be here, within curly brackets"
                )
            }
            Self::ProductFieldWithoutName => {
                write!(f, "expected a product field name")
            }
            Self::ProductFieldWithoutLeftArrow => {
                write!(
                    f,
                    "expected a \"<-\" after the product field name, and then a type signature"
                )
            }
            Self::ProductFieldsWithoutComma => {
                write!(
                    f,
                    "if there is another product field here, a comma should be between it and the previous field"
                )
            }
            Self::SumWithoutName => {
                write!(f, "expected a sum name")
            }
            Self::SumWithoutVariants => {
                write!(
                    f,
                    "the sum's variants should be here, within curly brackets"
                )
            }
            Self::SumVariantWithoutName => {
                write!(f, "expected a name for the sum variant")
            }
            Self::SumVariantWithoutFields => {
                write!(
                    f,
                    "the sum's variant's fields should be here, within curly brackets"
                )
            }
            Self::SumVariantFieldWithoutName => {
                write!(f, "expected a sum variant field name")
            }
            Self::SumVariantFieldWithoutLeftArrow => {
                write!(
                    f,
                    "expected a \"<-\" after the sum variant field name, and then a type signature"
                )
            }
            Self::SumVariantFieldsWithoutComma => {
                write!(
                    f,
                    "if there is another sum variant field here, a comma should be between it and the previous field"
                )
            }
            Self::SumVariantsWithoutComma => {
                write!(
                    f,
                    "if there is another sum variant here, a comma should be between it and the previous variant"
                )
            }
            Self::ModWithoutName => {
                write!(f, "expected a module name")
            }
            Self::ModWithoutBody => {
                write!(
                    f,
                    "the module's items should be here, within curly brackets"
                )
            }
            Self::TeachWithoutBody => {
                write!(
                    f,
                    "the associated items should be here, within curly brackets"
                )
            }
            Self::InvalidAssociatedItem => {
                write!(f, "only functions can be associated with a type")
            }
            Self::ExpectedBlock => write!(f, "expected zero or more expressions within brackets"),
            Self::ExpectedCallArguments => write!(
                f,
                "expected zero or more comma-separated expressions within parentheses"
            ),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {
    fn notes(&self) -> Vec<String> {
        match self {
            Self::InvalidName => vec!["a name can start with \"_\" or any ASCII letter, and be followed by zero or more of \"_\" or any ASCII letters or any ASCII digits".to_string()],
            Self::InvalidInteger => vec!["only u8, i8, u16, i16, u32, i32, u64, and i64 are supported".to_string()],
            Self::ExpectedExpr => vec![
                "an expression can start with \"(\", \"{\", \"if\", \"while\", \"let\", \"return\", \"!\", \"-\", \"true\", \"false\", \"root\", \"super\", \"Self\", a name, or a number".to_string()
            ],
            Self::ExpectedItem => vec!["items can start with \"funky\", \"product\", \"sum\", \"mod\", or \"teach\"".to_string()],
            Self::ExpectedType => vec!["type signatures can start with \"funky\" or a name".to_string()],
            Self::InvalidFloat
            | Self::BlockWithoutSemicolon
            | Self::NameIsKeyword
            | Self::LetInWithoutEqual
            | Self::LetInWithoutIn
            | Self::LetInWithoutBlock
            | Self::IfThenWithoutBlock
            | Self::IfElseWithoutBlock
            | Self::FnWithoutName
            | Self::FnWithoutParameters
            | Self::FnParameterWithoutName
            | Self::FnParameterWithoutLeftArrow
            | Self::FnWithoutBody
            | Self::CallWithoutComma
            | Self::FnParametersWithoutComma
            | Self::FnTypeParametersWithoutComma
            | Self::FnTypeWithoutParameters
            | Self::LetInWithoutComma
            | Self::PrimitiveWithoutName
            | Self::PrimitiveWithoutSemicolon
            | Self::NativeFnWithoutName
            | Self::NativeFnWithoutEqual
            | Self::NativeFnWithoutType
            | Self::ExpectedNativeItem
            | Self::UnknownNativeItem
            | Self::NativeFnWithoutSemicolon
            | Self::WhileWithoutBlock
            | Self::IfWithoutCondition
            | Self::WhileWithoutCondition
            | Self::ReturnWithoutValue
            | Self::GenericWithoutName
            | Self::GenericsWithoutComma
            | Self::ProductWithoutName
            | Self::ProductWithoutFields
            | Self::ProductFieldWithoutName
            | Self::ProductFieldWithoutLeftArrow
            | Self::ProductFieldsWithoutComma
            | Self::SumWithoutName
            | Self::SumWithoutVariants
            | Self::SumVariantWithoutName
            | Self::SumVariantWithoutFields
            | Self::SumVariantFieldWithoutName
            | Self::SumVariantFieldWithoutLeftArrow
            | Self::SumVariantFieldsWithoutComma
            | Self::SumVariantsWithoutComma
            | Self::ModWithoutName
            | Self::ModWithoutBody
            | Self::TeachWithoutBody
            | Self::InvalidAssociatedItem
            | Self::ExpectedBlock
            | Self::ExpectedCallArguments => vec![],
        }
    }
}

pub struct Ast {
    exprs: Vec<Spanned<Expr>>,
    items: Vec<Spanned<Item>>,
    roots: Vec<ItemIndex>,
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

impl Index<ExprIndex> for &Ast {
    type Output = Spanned<Expr>;

    fn index(&self, index: ExprIndex) -> &Self::Output {
        self.exprs.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "expression index out of bounds: the length is {} but the index is {index:?}",
                self.exprs.len(),
            );
        })
    }
}

impl Index<ItemIndex> for &Ast {
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

impl Index<ExprIndex> for &mut Ast {
    type Output = Spanned<Expr>;

    fn index(&self, index: ExprIndex) -> &Self::Output {
        self.exprs.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "expression index out of bounds: the length is {} but the index is {index:?}",
                self.exprs.len(),
            );
        })
    }
}

impl Index<ItemIndex> for &mut Ast {
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

impl IndexMut<ExprIndex> for &mut Ast {
    fn index_mut(&mut self, index: ExprIndex) -> &mut <Self as Index<ExprIndex>>::Output {
        let len = self.exprs.len();

        self.exprs.get_mut(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "expression index out of bounds: the length is {len} but the index is {index:?}"
            );
        })
    }
}

impl IndexMut<ItemIndex> for &mut Ast {
    fn index_mut(&mut self, index: ItemIndex) -> &mut <Self as Index<ItemIndex>>::Output {
        let len = self.items.len();

        self.items.get_mut(usize::from(index)).unwrap_or_else(|| {
            panic!("item index out of bounds: the length is {len} but the index is {index:?}");
        })
    }
}

#[derive(Debug)]
pub enum Expr {
    Integer(u64),
    NegativeInteger(i64),
    Float(f64),
    Boolean(bool),
    Unit,
    Unary {
        op: UnaryOp,
        expr: ExprIndex,
    },
    BinaryNoLhs {
        op: BinaryOp,
        rhs: ExprIndex,
    },
    Binary {
        op: BinaryOp,
        lhs: ExprIndex,
        rhs: ExprIndex,
    },
    Group(ExprIndex),
    Block(Vec<ExprIndex>),
    Name(String),
    Let {
        name: Spanned<String>,
        type_signature: Option<Spanned<TypeSignature>>,
        value: ExprIndex,
    },
    If {
        condition: ExprIndex,
        when_true: ExprIndex,
        otherwise: ExprIndex,
    },
    CallNoCallee(Vec<ExprIndex>),
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
    AsUnitNoValue,
    AsUnit(ExprIndex),
    Product {
        name: Spanned<String>,
        fields: Vec<(Spanned<String>, ExprIndex)>,
    },
    PathElement(PathElement),
    SelfType,
    ProductNoName(Vec<(Spanned<String>, ExprIndex)>),
}

#[derive(Debug)]
pub enum Item {
    Primitive(Spanned<String>),
    NativeFn {
        visibility: Visibility,
        name: Spanned<String>,
        signature: Spanned<TypeSignature>,
    },
    Fn {
        visibility: Visibility,
        name: Spanned<String>,
        parameters: Vec<Parameter>,
        return_type: Spanned<TypeSignature>,
        generics: Vec<Spanned<String>>,
        body: ExprIndex,
    },
    Product {
        visibility: Visibility,
        name: Spanned<String>,
        fields: Vec<Parameter>,
        generics: Vec<Spanned<String>>,
    },
    Sum {
        visibility: Visibility,
        name: Spanned<String>,
        variants: Vec<ItemIndex>,
        generics: Vec<Spanned<String>>,
    },
    Mod {
        visibility: Visibility,
        name: Spanned<String>,
        generics: Vec<Spanned<String>>,
        contents: Vec<ItemIndex>,
    },
    Teach {
        student: Spanned<TypeSignature>,
        body: Vec<ItemIndex>,
        generics: Vec<Spanned<String>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug)]
pub struct Parameter {
    name: Spanned<String>,
    type_signature: Spanned<TypeSignature>,
}

impl Parameter {
    pub const fn name(&self) -> &Spanned<String> {
        &self.name
    }

    pub const fn ty(&self) -> &Spanned<TypeSignature> {
        &self.type_signature
    }
}

#[derive(Debug)]
pub enum TypeSignature {
    Path {
        path: Vec<Spanned<PathElement>>,
        name: Spanned<String>,
        generics: Vec<Spanned<Self>>,
    },
    Normal {
        name: Spanned<String>,
        generics: Vec<Spanned<Self>>,
    },
    SelfTy,
    Fn {
        parameters: Vec<Spanned<Self>>,
        return_type: Box<Spanned<Self>>,
    },
}

#[derive(Debug)]
pub enum PathElement {
    Root,
    Super,
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    PathAccess,
    Access,
    Multiply,
    Divide,
    Remainder,
    Add,
    Subtract,
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
    Equal,
    NotEqual,
    And,
    Or,
    Assign,
}

impl Ast {
    pub fn new(core_id: usize) -> Self {
        Self {
            exprs: vec![],
            items: vec![],
            roots: vec![],
        }
        .parse_core(core_id)
    }

    #[allow(dead_code)]
    pub const fn roots(&self) -> &[ItemIndex] {
        self.roots.as_slice()
    }

    fn push_item(&mut self, item: Spanned<Item>) -> ItemIndex {
        let index = ItemIndex(self.items.len());

        self.items.push(item);

        index
    }

    fn push_expr(&mut self, expr: Spanned<Expr>) -> ExprIndex {
        let index = ExprIndex(self.exprs.len());

        self.exprs.push(expr);

        index
    }

    fn parse_core(mut self, core_id: usize) -> Self {
        let mut lexer = Lexer::new(core_id);

        lexer.push_source(crate::CORE_SOURCE);

        let report_data = reporting::ReportData::new(
            crate::CORE_SOURCE,
            "CORE ERROR",
            crate::CORE_PATH,
            "...",
            reporting::ReportColors::new(),
        );

        let forest = match token_tree::tokens_to_token_trees(&mut lexer) {
            Err(errors) => {
                for error in errors {
                    let _ = report_data.report(&error, &mut std::io::stderr().lock());
                }

                panic!("core failed to parse");
            }
            Ok(forest) => forest,
        };

        let mut parser = Parser::new(&forest, (crate::CORE_SOURCE, core_id), true);

        if let Err(errors) = parser.parse(&mut self) {
            let report_data = reporting::ReportData::new(
                crate::CORE_SOURCE,
                "CORE ERROR",
                crate::CORE_PATH,
                "...",
                reporting::ReportColors::new(),
            );

            for error in errors {
                let _ = report_data.report(&error, &mut std::io::stderr().lock());
            }

            panic!("core failed to parse");
        }

        self
    }

    #[allow(dead_code)]
    pub fn for_children_exprs<F>(&self, expr: ExprIndex, mut f: F)
    where
        F: FnMut(&Self, ExprIndex),
    {
        match self[expr].kind() {
            Expr::Integer(_)
            | Expr::NegativeInteger(_)
            | Expr::Float(_)
            | Expr::Boolean(_)
            | Expr::Unit
            | Expr::Name(_)
            | Expr::AsUnitNoValue
            | Expr::BinaryNoLhs {
                op: BinaryOp::Access | BinaryOp::PathAccess,
                ..
            }
            | Expr::PathElement(_)
            | Expr::SelfType
            | Expr::ProductNoName(_)
            | Expr::CallNoCallee(_) => {}
            Expr::Unary { expr, .. } | Expr::Group(expr) => {
                f(self, *expr);
            }
            Expr::BinaryNoLhs { rhs, .. } => {
                f(self, *rhs);
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
        }
    }
}

impl<'s, 'p> Parser<'s, 'p> {
    const fn new(forest: &'p Forest, (source, source_id): (&'s str, usize), is_core: bool) -> Self {
        Self {
            errors: vec![],
            forest,
            current_tree: forest.root(),
            at: 0,
            source,
            source_id,
            is_core,
        }
    }

    fn keyword(&self, span: Span) -> Option<&'s str> {
        let lexeme = span.lexeme(self.source)?;

        match lexeme {
            "primitive" | "native" if self.is_core => Some(lexeme),
            "let" | "in" | "if" | "else" | "true" | "false" | "funky" | "while" | "return"
            | "product" | "sum" | "pub" | "mod" | "teach" | "Self" | "root" | "super" => {
                Some(lexeme)
            }
            _ => None,
        }
    }

    fn in_token_tree(&self, token_tree_index: TokenTreeIndex) -> bool {
        self.current_tree == token_tree_index
            || matches!(
                self.forest.get_tree(self.current_tree),
                Some(TokenTree::Tree { outer: Some(outer), .. })
                    if *outer == token_tree_index && self.at == 0
            )
    }

    fn leave_finished_token_trees(&mut self) {
        while let Some(TokenTree::Tree { tokens, outer, .. }) =
            self.forest.get_tree(self.current_tree)
            && self.at >= tokens.len()
            && let Some(outer) = outer
            && let Some(TokenTree::Tree { tokens, .. }) = self.forest.get_tree(*outer)
        {
            self.at = tokens
                .iter()
                .position(|other_tree| *other_tree == self.current_tree)
                .map_or(tokens.len(), |at| at + 1);

            self.current_tree = *outer;
        }
    }

    fn advance_token_tree(&mut self) -> Option<&'p TokenTree> {
        self.leave_finished_token_trees();

        while let Some(TokenTree::Tree { tokens, .. }) = self.forest.get_tree(self.current_tree) {
            let next_token_tree = if let Some(token_tree_index) = tokens.get(self.at) {
                match self.forest.get_tree(*token_tree_index) {
                    Some(
                        tree @ TokenTree::Tree {
                            tokens: inner_tokens,
                            ..
                        },
                    ) => {
                        if inner_tokens.is_empty() {
                            self.at += 1;
                        } else {
                            self.current_tree = *token_tree_index;
                            self.at = 0;
                        }

                        Some(tree)
                    }
                    Some(token @ TokenTree::Token(_)) => {
                        self.at += 1;

                        Some(token)
                    }
                    None => unreachable!("all token trees should exist"),
                }
            } else {
                None
            };

            self.leave_finished_token_trees();

            if next_token_tree.is_some() {
                return next_token_tree;
            }
        }

        None
    }

    fn peek_token_tree(&self) -> Option<&'p TokenTree> {
        self.forest
            .get_tree(self.current_tree)
            .and_then(|token_tree| match token_tree {
                TokenTree::Tree { tokens, .. } => tokens
                    .get(self.at)
                    .and_then(|token_tree_index| self.forest.get_tree(*token_tree_index)),
                TokenTree::Token(_) => unreachable!("the current tree is never a single token"),
            })
    }

    fn check_next_token_tree(&self, tree_kind: TreeKind) -> Option<&'p TokenTree> {
        self.peek_token_tree().filter(|next_token_tree| matches!(next_token_tree, TokenTree::Tree { kind, .. } if *kind == tree_kind))
    }

    fn match_next_token_tree(&mut self, tree_kind: TreeKind) -> Option<&'p TokenTree> {
        self.check_next_token_tree(tree_kind)
            .and_then(|_| self.advance_token_tree())
    }

    fn consume_next_token_tree_with_span(
        &mut self,
        tree_kind: TreeKind,
        error: Error,
        span: Span,
    ) -> Result<&'p TokenTree, Spanned<Error>> {
        self.match_next_token_tree(tree_kind)
            .ok_or_else(|| Spanned::new(error, span))
    }

    fn consume_next_token_tree(
        &mut self,
        tree_kind: TreeKind,
        error: Error,
    ) -> Result<&'p TokenTree, Spanned<Error>> {
        let span = self.span_or_end();

        self.consume_next_token_tree_with_span(tree_kind, error, span)
    }

    fn check_next(&self, token_kind: Token) -> Option<&'p Spanned<Token>> {
        self.peek_token_tree()
            .and_then(|next_token_tree| match next_token_tree {
                TokenTree::Token(token) if *token.kind() == token_kind => Some(token),
                TokenTree::Token(_) | TokenTree::Tree { .. } => None,
            })
    }

    fn match_next(&mut self, token: Token) -> Option<&'p Spanned<Token>> {
        self.check_next(token).inspect(|_| {
            self.advance_token_tree();
        })
    }

    fn span_or_end(&self) -> Span {
        let source_len = self.source.len();
        let source_id = self.source_id;

        self.peek_token_tree().map_or_else(
            || Span::new(source_id, source_len, source_len),
            |token_tree| match token_tree {
                TokenTree::Token(token) => token.span(),
                TokenTree::Tree { span, .. } => *span,
            },
        )
    }

    fn consume_next_with_span(
        &mut self,
        token: Token,
        error: Error,
        span: Span,
    ) -> Result<&'p Spanned<Token>, Spanned<Error>> {
        self.match_next(token)
            .ok_or_else(|| Spanned::new(error, span))
    }

    fn consume_next(
        &mut self,
        token: Token,
        error: Error,
    ) -> Result<&'p Spanned<Token>, Spanned<Error>> {
        let span = self.span_or_end();

        self.consume_next_with_span(token, error, span)
    }

    fn check_keyword_next(&self, lexeme: &str) -> Option<&'p Spanned<Token>> {
        if self.peek_token_tree().is_some_and(|next_token_tree| {
            matches!(next_token_tree, TokenTree::Token(next_token) if *next_token.kind() == Token::Identifier
                && self
                    .keyword(next_token.span())
                    .is_some_and(|text| text == lexeme))
        }) {
            self.check_next(Token::Identifier)
        } else {
            None
        }
    }

    fn match_keyword_next(&mut self, lexeme: &str) -> Option<&'p Spanned<Token>> {
        self.check_keyword_next(lexeme).inspect(|_| {
            self.advance_token_tree();
        })
    }

    fn consume_name(&mut self, error: Error) -> Result<&'p Spanned<Token>, Spanned<Error>> {
        if self.peek_token_tree().is_some_and(|next_token_tree| {
            matches!(next_token_tree, TokenTree::Token(next_token) if *next_token.kind() == Token::Identifier
                && self
                    .keyword(next_token.span())
                .is_none()
            )
        }) {
            self.consume_next(Token::Identifier, error)
        } else {
            Err(Spanned::new(error, self.span_or_end()))
        }
    }

    fn consume_keyword(
        &mut self,
        error: Error,
        lexeme: &str,
    ) -> Result<&'p Spanned<Token>, Spanned<Error>> {
        self.match_keyword_next(lexeme)
            .ok_or_else(|| Spanned::new(error, self.span_or_end()))
    }

    fn spanned_name(&mut self) -> Result<Spanned<String>, Spanned<Error>> {
        let name_span = self.consume_name(Error::InvalidName)?.span();

        Ok(Spanned::new(
            name_span
                .lexeme(self.source)
                .ok_or_else(|| Spanned::new(Error::InvalidName, self.span_or_end()))?
                .to_string(),
            name_span,
        ))
    }
}

impl Parser<'_, '_> {
    fn parse(&mut self, ast: &mut Ast) -> Result<(), Vec<Spanned<Error>>> {
        let mut succeeded = None;

        while self.peek_token_tree().is_some() {
            match self.parse_item(ast) {
                Err(error) => {
                    if succeeded.is_none_or(|succeeded| succeeded) {
                        self.errors.push(error);
                        succeeded = Some(false);
                    }

                    self.advance_token_tree();
                }
                Ok(item) => {
                    succeeded = Some(true);
                    ast.roots.push(item);
                }
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(mem::take(&mut self.errors))
        }
    }
}

impl Parser<'_, '_> {
    fn parse_item(&mut self, ast: &mut Ast) -> Result<ItemIndex, Spanned<Error>> {
        let visibility = if self.match_keyword_next("pub").is_some() {
            Visibility::Public
        } else {
            Visibility::Private
        };

        if let Some(token) = self.match_next(Token::Identifier) {
            if let Some(keyword) = self.keyword(token.span()) {
                let span = token.span();

                match keyword {
                    "primitive" => self.parse_primitive(ast, span),
                    "native" => {
                        let token =
                            self.consume_next(Token::Identifier, Error::ExpectedNativeItem)?;

                        let native_span = span;
                        let span = token.span();

                        if self.keyword(span) == Some("funky") {
                            self.parse_native_function(ast, native_span, visibility)
                        } else {
                            Err(Spanned::new(Error::UnknownNativeItem, token.span()))
                        }
                    }
                    "funky" => self.parse_fn(ast, span, visibility),
                    "product" => self.parse_product(ast, span, visibility, true),
                    "sum" => self.parse_sum(ast, span, visibility),
                    "mod" => self.parse_mod(ast, span, visibility),
                    "teach" => self.parse_teach(ast, span),
                    _ => Err(Spanned::new(Error::ExpectedItem, span)),
                }
            } else {
                Err(Spanned::new(Error::ExpectedItem, token.span()))
            }
        } else {
            Err(Spanned::new(Error::ExpectedItem, self.span_or_end()))
        }
    }

    fn parse_primitive(&mut self, ast: &mut Ast, span: Span) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .spanned_name()
            .map_err(|error| error.transmute(|_| Error::PrimitiveWithoutName))?;

        let semicolon_span = self
            .consume_next(Token::Semicolon, Error::PrimitiveWithoutSemicolon)?
            .span();

        let span = span
            .combine_with(semicolon_span)
            .expect("these spans are from the same source");

        Ok(ast.push_item(Spanned::new(Item::Primitive(name), span)))
    }

    fn parse_native_function(
        &mut self,
        ast: &mut Ast,
        span: Span,
        visibility: Visibility,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .spanned_name()
            .map_err(|error| error.transmute(|_| Error::NativeFnWithoutName))?;

        self.consume_next(Token::Equal, Error::NativeFnWithoutEqual)?;

        let signature = self.parse_type_signature()?;

        let semicolon_span = self
            .consume_next(Token::Semicolon, Error::NativeFnWithoutSemicolon)?
            .span();

        let span = span
            .combine_with(semicolon_span)
            .expect("these spans are from the same source");

        Ok(ast.push_item(Spanned::new(
            Item::NativeFn {
                visibility,
                name,
                signature,
            },
            span,
        )))
    }

    #[allow(clippy::too_many_lines)]
    fn parse_fn(
        &mut self,
        ast: &mut Ast,
        span: Span,
        visibility: Visibility,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .spanned_name()
            .map_err(|error| error.transmute(|_| Error::FnWithoutName))?;

        let generics = self.parse_generics()?;

        let parameters_span = self.span_or_end();

        let parameters = if let TokenTree::Tree { tokens, .. } = self
            .consume_next_token_tree_with_span(
                TreeKind::Parentheses,
                Error::FnWithoutParameters,
                parameters_span,
            )?
            && !tokens.is_empty()
        {
            let mut parameters = vec![];

            let current_tree = self.current_tree;

            while self.in_token_tree(current_tree) {
                let parameter_name = self
                    .spanned_name()
                    .map_err(|error| error.transmute(|_| Error::FnParameterWithoutName))?;

                let less = self.consume_next(Token::Less, Error::FnParameterWithoutLeftArrow)?;

                let minus = self.consume_next(Token::Minus, Error::FnParameterWithoutLeftArrow)?;

                if minus.span().start() != less.span().end() {
                    return Err(Spanned::new(
                        Error::FnParameterWithoutLeftArrow,
                        less.span()
                            .combine_with(minus.span())
                            .expect("these spans are from the same source"),
                    ));
                }

                let type_signature = self.parse_type_signature()?;

                parameters.push(Parameter {
                    name: parameter_name,
                    type_signature,
                });

                if self.in_token_tree(current_tree) {
                    self.consume_next(Token::Comma, Error::FnParametersWithoutComma)?;
                }
            }

            parameters
        } else {
            vec![]
        };

        let current_tree = self.current_tree;
        let at = self.at;

        let return_type = if let Some(minus) = self.match_next(Token::Minus) {
            if self
                .match_next(Token::Greater)
                .is_some_and(|greater| minus.span().end() == greater.span().start())
            {
                self.parse_type_signature()?
            } else {
                self.current_tree = current_tree;
                self.at = at;

                Spanned::new(
                    TypeSignature::Normal {
                        name: Spanned::new("unit".to_string(), parameters_span),
                        generics: vec![],
                    },
                    parameters_span,
                )
            }
        } else {
            Spanned::new(
                TypeSignature::Normal {
                    name: Spanned::new("unit".to_string(), parameters_span),
                    generics: vec![],
                },
                parameters_span,
            )
        };

        let body_span = self.span_or_end();

        let exprs = self.block_exprs(ast)?;

        let body = ast.push_expr(Spanned::new(Expr::Block(exprs), body_span));

        let funky = ast.push_item(Spanned::new(
            Item::Fn {
                visibility,
                name,
                parameters,
                return_type,
                generics,
                body,
            },
            span.combine_with(body_span).unwrap_or(span),
        ));

        Ok(funky)
    }

    fn parse_generics(&mut self) -> Result<Vec<Spanned<String>>, Spanned<Error>> {
        Ok(
            if let Some(TokenTree::Tree { tokens, .. }) =
                self.match_next_token_tree(TreeKind::SquareBrackets)
                && !tokens.is_empty()
            {
                let mut generics = vec![];

                let current_tree = self.current_tree;

                while self.in_token_tree(current_tree) {
                    let generic_name = self
                        .spanned_name()
                        .map_err(|error| error.transmute(|_| Error::GenericWithoutName))?;

                    generics.push(generic_name);

                    if self.in_token_tree(current_tree) {
                        self.consume_next(Token::Comma, Error::GenericsWithoutComma)?;
                    }
                }

                generics
            } else {
                vec![]
            },
        )
    }

    #[allow(clippy::too_many_lines)]
    fn parse_product(
        &mut self,
        ast: &mut Ast,
        span: Span,
        visibility: Visibility,
        allow_generics: bool,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .spanned_name()
            .map_err(|error| error.transmute(|_| Error::ProductWithoutName))?;

        let generics = if allow_generics {
            self.parse_generics()?
        } else {
            vec![]
        };

        let fields_span = self.span_or_end();

        let fields = if let TokenTree::Tree { tokens, .. } = self
            .consume_next_token_tree_with_span(
                TreeKind::Brackets,
                Error::ProductWithoutFields,
                fields_span,
            )?
            && !tokens.is_empty()
        {
            let mut fields = vec![];

            let current_tree = self.current_tree;

            while self.in_token_tree(current_tree) {
                let name = self
                    .spanned_name()
                    .map_err(|error| error.transmute(|_| Error::ProductFieldWithoutName))?;

                let less = self.consume_next(Token::Less, Error::ProductFieldWithoutLeftArrow)?;

                let minus = self.consume_next(Token::Minus, Error::ProductFieldWithoutLeftArrow)?;

                if minus.span().start() != less.span().end() {
                    return Err(Spanned::new(
                        Error::ProductFieldWithoutLeftArrow,
                        less.span()
                            .combine_with(minus.span())
                            .expect("these spans are from the same source"),
                    ));
                }

                let type_signature = self.parse_type_signature()?;

                fields.push(Parameter {
                    name,
                    type_signature,
                });

                if self.in_token_tree(current_tree) {
                    self.consume_next(Token::Comma, Error::ProductFieldsWithoutComma)?;
                }
            }

            // sort fields to keep the order consistent in other passes
            fields.sort_by(|a, b| a.name().kind().cmp(b.name().kind()));

            fields
        } else {
            vec![]
        };

        Ok(ast.push_item(Spanned::new(
            Item::Product {
                visibility,
                name,
                generics,
                fields,
            },
            span.combine_with(fields_span).unwrap_or(span),
        )))
    }

    fn parse_sum(
        &mut self,
        ast: &mut Ast,
        span: Span,
        visibility: Visibility,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .spanned_name()
            .map_err(|error| error.transmute(|_| Error::ProductWithoutName))?;

        let generics = self.parse_generics()?;

        let variants_span = self.span_or_end();

        let variants = if let TokenTree::Tree { tokens, .. } = self
            .consume_next_token_tree_with_span(
                TreeKind::Brackets,
                Error::SumWithoutVariants,
                variants_span,
            )?
            && !tokens.is_empty()
        {
            let mut variants = vec![];

            let current_tree = self.current_tree;

            while self.in_token_tree(current_tree) {
                let span = self.span_or_end();

                variants.push(self.parse_product(ast, span, visibility, false).map_err(
                    |error| {
                        error.transmute(|error| match error {
                            Error::ProductWithoutName => Error::SumVariantWithoutName,
                            Error::ProductWithoutFields => Error::SumVariantWithoutFields,
                            Error::ProductFieldWithoutName => Error::SumVariantFieldWithoutName,
                            Error::ProductFieldWithoutLeftArrow => {
                                Error::SumVariantFieldWithoutLeftArrow
                            }
                            Error::ProductFieldsWithoutComma => Error::SumVariantFieldsWithoutComma,
                            _ => unreachable!("no other errors are produced by `parse_product`"),
                        })
                    },
                )?);

                if self.in_token_tree(current_tree) {
                    self.consume_next(Token::Comma, Error::SumVariantsWithoutComma)?;
                }
            }

            // sort variants to keep the order consistent in other passes
            variants.sort_by_key(|a| {
                let Item::Product { name, .. } = ast[*a].kind() else {
                    unreachable!("only products are sum variants");
                };

                name.kind()
            });

            variants
        } else {
            vec![]
        };

        Ok(ast.push_item(Spanned::new(
            Item::Sum {
                visibility,
                name,
                generics,
                variants,
            },
            span.combine_with(variants_span).unwrap_or(span),
        )))
    }

    fn parse_mod(
        &mut self,
        ast: &mut Ast,
        span: Span,
        visibility: Visibility,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .spanned_name()
            .map_err(|error| error.transmute(|_| Error::ModWithoutName))?;

        let generics = self.parse_generics()?;

        let contents_span = self.span_or_end();

        let contents =
            if let TokenTree::Tree { tokens, .. } = self.consume_next_token_tree_with_span(
                TreeKind::Brackets,
                Error::ModWithoutBody,
                contents_span,
            )? && !tokens.is_empty()
            {
                let mut contents = vec![];

                let current_tree = self.current_tree;

                while self.in_token_tree(current_tree) {
                    contents.push(self.parse_item(ast)?);
                }

                contents
            } else {
                vec![]
            };

        Ok(ast.push_item(Spanned::new(
            Item::Mod {
                visibility,
                name,
                generics,
                contents,
            },
            span.combine_with(contents_span)
                .expect("these spans are from the same source"),
        )))
    }

    fn parse_teach(&mut self, ast: &mut Ast, span: Span) -> Result<ItemIndex, Spanned<Error>> {
        let generics = self.parse_generics()?;

        let student = self.parse_type_signature()?;

        let contents_span = self.span_or_end();

        let body = if let TokenTree::Tree { tokens, .. } = self.consume_next_token_tree_with_span(
            TreeKind::Brackets,
            Error::TeachWithoutBody,
            contents_span,
        )? && !tokens.is_empty()
        {
            let mut body = vec![];

            let current_tree = self.current_tree;

            while self.in_token_tree(current_tree) {
                let item = self.parse_item(ast)?;

                match &ast[item].kind() {
                    Item::Primitive(_)
                    | Item::Product { .. }
                    | Item::Sum { .. }
                    | Item::Mod { .. }
                    | Item::Teach { .. } => {
                        self.errors
                            .push(Spanned::new(Error::InvalidAssociatedItem, ast[item].span()));
                    }
                    Item::NativeFn { .. } | Item::Fn { .. } => {}
                }

                body.push(item);
            }

            body
        } else {
            vec![]
        };

        Ok(ast.push_item(Spanned::new(
            Item::Teach {
                student,
                body,
                generics,
            },
            span.combine_with(contents_span)
                .expect("these spans are from the same source"),
        )))
    }

    fn path_element(&mut self) -> Result<Spanned<PathElement>, Spanned<Error>> {
        let span = self.span_or_end();

        match self.keyword(span) {
            Some("root") => {
                self.advance_token_tree();

                Ok(Spanned::new(PathElement::Root, span))
            }
            Some("super") => {
                self.advance_token_tree();

                Ok(Spanned::new(PathElement::Super, span))
            }
            _ => {
                let name = self.spanned_name()?;

                Ok(name.transmute(PathElement::Name))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse_type_signature(&mut self) -> Result<Spanned<TypeSignature>, Spanned<Error>> {
        if let Some(token) = self.check_next(Token::Identifier) {
            let span = token.span();

            match self.keyword(span) {
                Some("funky") => {
                    self.advance_token_tree();

                    let parameters_span = self.span_or_end();

                    let parameters = if let TokenTree::Tree { tokens, .. } = self
                        .consume_next_token_tree_with_span(
                            TreeKind::Parentheses,
                            Error::FnTypeWithoutParameters,
                            parameters_span,
                        )?
                        && !tokens.is_empty()
                    {
                        let mut parameters = vec![];

                        let current_tree = self.current_tree;

                        while self.in_token_tree(current_tree) {
                            parameters.push(self.parse_type_signature()?);

                            if self.in_token_tree(current_tree) {
                                self.consume_next(
                                    Token::Comma,
                                    Error::FnTypeParametersWithoutComma,
                                )?;
                            }
                        }

                        parameters
                    } else {
                        vec![]
                    };

                    let current_tree = self.current_tree;
                    let at = self.at;

                    let return_type = if let Some(minus) = self.match_next(Token::Minus) {
                        if let Some(greater) = self.match_next(Token::Greater)
                            && minus.span().end() == greater.span().start()
                        {
                            self.parse_type_signature()?
                        } else {
                            self.current_tree = current_tree;
                            self.at = at;

                            Spanned::new(
                                TypeSignature::Normal {
                                    name: Spanned::new("unit".to_string(), parameters_span),
                                    generics: vec![],
                                },
                                parameters_span,
                            )
                        }
                    } else {
                        Spanned::new(
                            TypeSignature::Normal {
                                name: Spanned::new("unit".to_string(), parameters_span),
                                generics: vec![],
                            },
                            parameters_span,
                        )
                    };

                    let return_span = return_type.span();

                    Ok(Spanned::new(
                        TypeSignature::Fn {
                            parameters,
                            return_type: Box::new(return_type),
                        },
                        span.combine_with(return_span)
                            .expect("these spans are from the same source"),
                    ))
                }
                Some("Self") => {
                    self.advance_token_tree();

                    Ok(Spanned::new(TypeSignature::SelfTy, span))
                }
                None | Some("root" | "super") => {
                    let mut path = vec![self.path_element()?];

                    while self.peek_token_tree().is_some()
                        && self.match_next(Token::Tilde).is_some()
                    {
                        let path_element = self.path_element()?;

                        path.push(path_element);
                    }

                    let mut generics = vec![];

                    let span = if let Some(TokenTree::Tree { tokens, span, .. }) =
                        self.match_next_token_tree(TreeKind::SquareBrackets)
                        && !tokens.is_empty()
                    {
                        let current_tree = self.current_tree;

                        while self.in_token_tree(current_tree) {
                            generics.push(self.parse_type_signature()?);

                            if self.in_token_tree(current_tree) {
                                self.consume_next(Token::Comma, Error::GenericsWithoutComma)?;
                            }
                        }

                        *span
                    } else {
                        path[0].span()
                    };

                    let name = path
                        .pop()
                        .expect("the path is guaranteed to have at least one name");

                    let name = if let PathElement::Name(end_of_path) = name.kind() {
                        Ok(Spanned::new(end_of_path.clone(), name.span()))
                    } else {
                        Err(Spanned::new(Error::InvalidName, name.span()))
                    }?;

                    let span = name
                        .span()
                        .combine_with(span)
                        .expect("these spans are from the same source");

                    Ok(Spanned::new(
                        if path.is_empty() {
                            TypeSignature::Normal { name, generics }
                        } else {
                            TypeSignature::Path {
                                path,
                                name,
                                generics,
                            }
                        },
                        span,
                    ))
                }
                Some(_) => Err(Spanned::new(Error::NameIsKeyword, span)),
            }
        } else {
            Err(Spanned::new(Error::ExpectedType, self.span_or_end()))
        }
    }
}

mod precedence {
    use super::{Ast, Error, ExprIndex, Parser, Span, Spanned};

    pub type ParseFn<'s, 'p> =
        fn(&mut Parser<'s, 'p>, &mut Ast, u16, Span) -> Result<ExprIndex, Spanned<Error>>;

    pub const PRIMARY: u16 = 0xEE00;

    pub const AS_PRODUCT: u16 = 0xEE00;

    pub const LEFT_PATH_ACCESS: u16 = 0xCC00;
    pub const RIGHT_PATH_ACCESS: u16 = 0xCC50;

    pub const CALL: u16 = 0xBB00;

    pub const LEFT_ACCESS: u16 = 0xBB00;
    pub const RIGHT_ACCESS: u16 = 0xBB50;

    pub const NOT: u16 = 0xAA00;

    pub const NEGATE: u16 = 0xAA00;

    pub const LEFT_MULTIPLY: u16 = 0x9900;
    pub const RIGHT_MULTIPLY: u16 = 0x9950;

    pub const LEFT_DIVIDE: u16 = 0x9900;
    pub const RIGHT_DIVIDE: u16 = 0x9950;

    pub const LEFT_REMAINDER: u16 = 0x9900;
    pub const RIGHT_REMAINDER: u16 = 0x9950;

    pub const LEFT_ADD: u16 = 0x8800;
    pub const RIGHT_ADD: u16 = 0x8850;

    pub const LEFT_SUBTRACT: u16 = 0x8800;
    pub const RIGHT_SUBTRACT: u16 = 0x8850;

    pub const LEFT_LESS: u16 = 0x7700;
    pub const RIGHT_LESS: u16 = 0x7750;

    pub const LEFT_GREATER: u16 = 0x7700;
    pub const RIGHT_GREATER: u16 = 0x7750;

    pub const LEFT_LESS_OR_EQUAL: u16 = 0x7700;
    pub const RIGHT_LESS_OR_EQUAL: u16 = 0x7750;

    pub const LEFT_GREATER_OR_EQUAL: u16 = 0x7700;
    pub const RIGHT_GREATER_OR_EQUAL: u16 = 0x7750;

    pub const LEFT_EQUAL: u16 = 0x6600;
    pub const RIGHT_EQUAL: u16 = 0x6650;

    pub const LEFT_NOT_EQUAL: u16 = 0x6600;
    pub const RIGHT_NOT_EQUAL: u16 = 0x6650;

    pub const LEFT_AND: u16 = 0x5500;
    pub const RIGHT_AND: u16 = 0x5550;

    pub const LEFT_OR: u16 = 0x4400;
    pub const RIGHT_OR: u16 = 0x4450;

    pub const RETURN: u16 = 0x3800;

    pub const LEFT_ASSIGN: u16 = 0x3350;
    pub const RIGHT_ASSIGN: u16 = 0x3300;

    pub const AS_UNIT: u16 = 0x1100;
}

macro_rules! infix_op_precedence {
    (
        $parser:ident, $first_kind:ident ;
        $(2nd token $dual_kind:ident => $dual_parse_fn:ident ($dual_left_precedence:ident, $dual_right_precedence:ident), $($is_dual:lifetime)?)?
        $parse_fn:ident ($left_precedence:ident, $right_precedence:ident) $(,)?
    ) => {{
        let token = $parser.match_next($crate::lex::Token::$first_kind)?;

        $($($is_dual)? if
            $parser.check_next($crate::lex::Token::$dual_kind)
            .is_some_and(|next_token| {
                token.span().end() == next_token.span().start()
            }) && let Some(next_token) = $parser.match_next($crate::lex::Token::$dual_kind)
        {
            Some((
                (
                    $crate::parse::precedence::$dual_left_precedence,
                    $crate::parse::precedence::$dual_right_precedence,
                ),
                (
                    Self::$dual_parse_fn,
                    token
                        .span()
                        .combine_with(next_token.span())
                        .expect("these spans are from the same source"),
                ),
            ))
        } else)? {
            Some((
                (
                    $crate::parse::precedence::$left_precedence,
                    $crate::parse::precedence::$right_precedence,
                ),
                (Self::$parse_fn, token.span()),
            ))
        }
    }};
}

macro_rules! unary_op {
    (
        $name:ident, $op:ident $(,)?
    ) => {
        fn $name(
            &mut self,
            ast: &mut $crate::parse::Ast,
            precedence: u16,
            span: $crate::Span,
        ) -> Result<$crate::parse::ExprIndex, $crate::Spanned<$crate::parse::Error>> {
            let expr = self.parse_expression(ast, precedence)?;

            Ok(ast.push_expr($crate::Spanned::new(
                $crate::parse::Expr::Unary {
                    op: $crate::parse::UnaryOp::$op,
                    expr,
                },
                span.combine_with(ast[expr].span())
                    .expect("these spans are from the same source"),
            )))
        }
    };
}

macro_rules! binary_op {
    (
        $name:ident, $op:ident $(,)?
    ) => {
        fn $name(
            &mut self,
            ast: &mut $crate::parse::Ast,
            precedence: u16,
            span: $crate::Span,
        ) -> Result<$crate::parse::ExprIndex, $crate::Spanned<$crate::parse::Error>> {
            let rhs = self.parse_expression(ast, precedence)?;

            Ok(ast.push_expr($crate::Spanned::new(
                $crate::parse::Expr::BinaryNoLhs {
                    op: $crate::parse::BinaryOp::$op,
                    rhs,
                },
                span.combine_with(ast[rhs].span())
                    .expect("These spans are from the same source"),
            )))
        }
    };
}

impl<'s, 'p> Parser<'s, 'p> {
    #[allow(clippy::too_many_lines)]
    fn parse_expression(
        &mut self,
        mut ast: &mut Ast,
        min_precedence: u16,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let within_tree = self.current_tree;

        let Some((precedence, (prefix_fn, prefix_span))) = self.prefix_precedence() else {
            return Err(Spanned::new(Error::ExpectedExpr, self.span_or_end()));
        };

        let mut lhs = prefix_fn(self, ast, precedence, prefix_span)?;

        loop {
            let current_tree = self.current_tree;
            let at = self.at;

            if self.in_token_tree(within_tree)
                && let Some((left_precedence, (postfix_fn, postfix_span))) =
                    self.postfix_precedence()
            {
                if left_precedence < min_precedence {
                    self.current_tree = current_tree;
                    self.at = at;

                    break;
                }

                let unfinished_postfix = postfix_fn(self, ast, left_precedence, postfix_span);

                let unfinished_postfix = match unfinished_postfix.as_ref().map_err(Spanned::kind) {
                    Ok(expr_index) => *expr_index,
                    Err(
                        Error::ProductFieldWithoutName
                        | Error::ProductFieldWithoutLeftArrow
                        | Error::ProductFieldsWithoutComma,
                    ) => {
                        self.current_tree = current_tree;
                        self.at = at;

                        break;
                    }
                    Err(_) => return unfinished_postfix,
                };

                match ast[unfinished_postfix].kind() {
                    Expr::CallNoCallee(arguments) => {
                        let arguments = arguments.clone();

                        if let Expr::Binary {
                            op: BinaryOp::Access,
                            lhs: target,
                            rhs: method,
                        } = ast[lhs].kind()
                        {
                            let target = *target;
                            let method = *method;

                            ast[unfinished_postfix] = Spanned::new(
                                Expr::MethodCall {
                                    target,
                                    method,
                                    arguments,
                                },
                                ast[unfinished_postfix]
                                    .span()
                                    .combine_with(prefix_span)
                                    .expect("these spans are from the same source"),
                            );
                        } else {
                            ast[unfinished_postfix] = Spanned::new(
                                Expr::Call {
                                    callee: lhs,
                                    arguments,
                                },
                                ast[unfinished_postfix]
                                    .span()
                                    .combine_with(prefix_span)
                                    .expect("these spans are from the same source"),
                            );
                        }
                    }
                    Expr::AsUnitNoValue => {
                        ast[unfinished_postfix] = Spanned::new(
                            Expr::AsUnit(lhs),
                            ast[unfinished_postfix]
                                .span()
                                .combine_with(prefix_span)
                                .expect("these spans are from the same source"),
                        );
                    }
                    Expr::ProductNoName(fields) => {
                        let fields = fields.clone();

                        ast[lhs] = Spanned::new(
                            Expr::Product {
                                name: match ast[lhs].kind() {
                                    Expr::Name(name) => Spanned::new(name.clone(), ast[lhs].span()),
                                    Expr::SelfType => {
                                        Spanned::new("Self".to_string(), ast[lhs].span())
                                    }
                                    Expr::Binary {
                                        op: BinaryOp::PathAccess,
                                        rhs,
                                        ..
                                    } => match ast[*rhs].kind() {
                                        Expr::Name(name) => {
                                            Spanned::new(name.clone(), ast[lhs].span())
                                        }
                                        Expr::SelfType => {
                                            Spanned::new("Self".to_string(), ast[lhs].span())
                                        }
                                        _ => {
                                            self.current_tree = current_tree;
                                            self.at = at;

                                            break;
                                        }
                                    },
                                    _ => {
                                        self.current_tree = current_tree;
                                        self.at = at;

                                        break;
                                    }
                                },
                                fields,
                            },
                            prefix_span
                                .combine_with(ast[unfinished_postfix].span())
                                .expect("these spans are from the same source"),
                        );

                        continue;
                    }
                    _ => {
                        unreachable!("a postfix expression was unaccounted for")
                    }
                }

                lhs = unfinished_postfix;

                continue;
            }

            let current_tree = self.current_tree;
            let at = self.at;

            if self.in_token_tree(within_tree)
                && let Some(((left_precedence, right_precedence), (infix_fn, infix_span))) =
                    self.infix_precedence()
            {
                if left_precedence < min_precedence {
                    self.current_tree = current_tree;
                    self.at = at;

                    break;
                }

                let unfinished_infix = infix_fn(self, ast, right_precedence, infix_span)?;

                match ast[unfinished_infix].kind() {
                    Expr::BinaryNoLhs { op, rhs } => {
                        let (op, rhs) = (*op, *rhs);

                        ast[unfinished_infix] = Spanned::new(
                            Expr::Binary { lhs, op, rhs },
                            ast[unfinished_infix]
                                .span()
                                .combine_with(prefix_span)
                                .expect("these spans are from the same source"),
                        );
                    }
                    _ => {
                        unreachable!("an infix expression was unaccounted for");
                    }
                }

                lhs = unfinished_infix;

                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn prefix_precedence(&mut self) -> Option<(u16, (precedence::ParseFn<'s, 'p>, Span))> {
        match self.peek_token_tree()? {
            TokenTree::Tree {
                span,
                kind: TreeKind::Parentheses,
                ..
            } => Some((precedence::PRIMARY, (Self::group, *span))),
            TokenTree::Tree {
                span,
                kind: TreeKind::Brackets,
                ..
            } => Some((precedence::PRIMARY, (Self::block, *span))),
            TokenTree::Token(token) => match token.kind() {
                Token::Minus => {
                    self.advance_token_tree()?;

                    Some((precedence::NEGATE, (Self::negate, token.span())))
                }
                Token::Bang => {
                    self.advance_token_tree()?;

                    Some((precedence::NOT, (Self::not, token.span())))
                }
                Token::Integer(_) => Some((precedence::PRIMARY, (Self::integer, token.span()))),
                Token::Float(_) => Some((precedence::PRIMARY, (Self::float, token.span()))),
                Token::Identifier => {
                    let span = token.span();

                    match self.keyword(span) {
                        Some("root" | "super") => {
                            Some((precedence::PRIMARY, (Self::path_element_expr, span)))
                        }
                        Some("Self") => {
                            self.advance_token_tree()?;

                            Some((precedence::PRIMARY, (Self::self_type_expr, span)))
                        }
                        Some("let") => {
                            self.advance_token_tree()?;

                            Some((precedence::PRIMARY, (Self::let_in, span)))
                        }
                        Some("if") => {
                            self.advance_token_tree()?;

                            Some((precedence::PRIMARY, (Self::if_expr, span)))
                        }
                        Some("while") => {
                            self.advance_token_tree()?;

                            Some((precedence::PRIMARY, (Self::while_expr, span)))
                        }
                        Some("return") => {
                            self.advance_token_tree()?;

                            Some((precedence::RETURN, (Self::return_expr, span)))
                        }
                        Some("true") => {
                            self.advance_token_tree()?;

                            Some((precedence::PRIMARY, (Self::so_true, span)))
                        }
                        Some("false") => {
                            self.advance_token_tree()?;

                            Some((precedence::PRIMARY, (Self::so_false, span)))
                        }
                        Some(_) => None,
                        None => Some((precedence::PRIMARY, (Self::name, span))),
                    }
                }
                _ => None,
            },
            TokenTree::Tree { .. } => None,
        }
    }

    fn postfix_precedence(&mut self) -> Option<(u16, (precedence::ParseFn<'s, 'p>, Span))> {
        match self.peek_token_tree()? {
            TokenTree::Tree {
                kind: TreeKind::Parentheses,
                span,
                ..
            } => Some((precedence::CALL, (Self::call, *span))),
            TokenTree::Tree {
                kind: TreeKind::Brackets,
                span,
                ..
            } => Some((precedence::AS_PRODUCT, (Self::product_expr, *span))),
            TokenTree::Token(token) if matches!(token.kind(), Token::Semicolon) => {
                self.advance_token_tree()?;

                Some((precedence::AS_UNIT, (Self::as_unit, token.span())))
            }
            TokenTree::Tree { .. } | TokenTree::Token(_) => None,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infix_precedence(&mut self) -> Option<((u16, u16), (precedence::ParseFn<'s, 'p>, Span))> {
        if let TokenTree::Token(token) = self.peek_token_tree()? {
            match token.kind() {
                Token::Tilde => infix_op_precedence!(
                    self, Tilde ;
                    path_access (LEFT_PATH_ACCESS, RIGHT_PATH_ACCESS),
                ),
                Token::Dot => infix_op_precedence!(
                    self, Dot ;
                    access (LEFT_ACCESS, RIGHT_ACCESS),
                ),
                Token::Star => infix_op_precedence!(
                    self, Star ;
                    multiply (LEFT_MULTIPLY, RIGHT_MULTIPLY),
                ),
                Token::Slash => infix_op_precedence!(
                    self, Slash ;
                    divide (LEFT_DIVIDE, RIGHT_DIVIDE),
                ),
                Token::Percent => infix_op_precedence!(
                    self, Percent ;
                    remainder (LEFT_REMAINDER, RIGHT_REMAINDER),
                ),
                Token::Plus => infix_op_precedence!(
                    self, Plus ;
                    add (LEFT_ADD, RIGHT_ADD),
                ),
                Token::Minus => infix_op_precedence!(
                    self, Minus ;
                    subtract (LEFT_SUBTRACT, RIGHT_SUBTRACT),
                ),
                Token::Less => infix_op_precedence!(
                    self, Less ;
                    2nd token Equal => less_or_equal (LEFT_LESS_OR_EQUAL, RIGHT_LESS_OR_EQUAL),
                    less (LEFT_LESS, RIGHT_LESS),
                ),
                Token::Greater => infix_op_precedence!(
                    self, Greater ;
                    2nd token Equal => greater_or_equal (LEFT_GREATER_OR_EQUAL, RIGHT_GREATER_OR_EQUAL),
                    greater (LEFT_GREATER, RIGHT_GREATER),
                ),
                Token::Equal => infix_op_precedence!(
                    self, Equal ;
                    2nd token Equal => equal (LEFT_EQUAL, RIGHT_EQUAL),
                    assign (LEFT_ASSIGN, RIGHT_ASSIGN),
                ),
                Token::Bang => {
                    let current_tree = self.current_tree;
                    let at = self.at;

                    let token = self.match_next(Token::Bang)?;

                    if self.peek_token_tree().is_some_and(|next_token_tree| {
                        matches!(next_token_tree, TokenTree::Token(next_token) if *next_token.kind() == Token::Equal
                            && token.span().end() == next_token.span().start()
                        )
                    }) && let Some(next_token) = self.match_next(Token::Equal)
                    {
                        Some((
                            (precedence::LEFT_NOT_EQUAL, precedence::RIGHT_NOT_EQUAL),
                            (
                                Self::not_equal,
                                token
                                    .span()
                                    .combine_with(next_token.span())
                                    .expect("these spans are from the same source"),
                            ),
                        ))
                    } else {
                        self.current_tree = current_tree;
                        self.at = at;

                        None
                    }
                }
                Token::Ampersand => {
                    let current_tree = self.current_tree;
                    let at = self.at;

                    let token = self.match_next(Token::Ampersand)?;

                    if self.peek_token_tree().is_some_and(|next_token_tree| {
                        matches!(next_token_tree, TokenTree::Token(next_token) if *next_token.kind() == Token::Ampersand
                            && token.span().end() == next_token.span().start()
                        )
                    }) && let Some(next_token) = self.match_next(Token::Ampersand)
                    {
                        Some((
                            (precedence::LEFT_AND, precedence::RIGHT_AND),
                            (
                                Self::and,
                                token
                                    .span()
                                    .combine_with(next_token.span())
                                    .expect("these spans are from the same source"),
                            ),
                        ))
                    } else {
                        self.current_tree = current_tree;
                        self.at = at;

                        None
                    }
                }
                Token::Pipe => {
                    let current_tree = self.current_tree;
                    let at = self.at;

                    let token = self.match_next(Token::Pipe)?;

                    if self.peek_token_tree().is_some_and(|next_token_tree| {
                        matches!(next_token_tree, TokenTree::Token(next_token) if *next_token.kind() == Token::Pipe
                            && token.span().end() == next_token.span().start()
                        )
                    }) && let Some(next_token) = self.match_next(Token::Pipe)
                    {
                        Some((
                            (precedence::LEFT_OR, precedence::RIGHT_OR),
                            (
                                Self::or,
                                token
                                    .span()
                                    .combine_with(next_token.span())
                                    .expect("these spans are from the same source"),
                            ),
                        ))
                    } else {
                        self.current_tree = current_tree;
                        self.at = at;

                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn integer(&mut self, ast: &mut Ast, _: u16, _: Span) -> Result<ExprIndex, Spanned<Error>> {
        let token = self
            .advance_token_tree()
            .and_then(|token_tree| match token_tree {
                TokenTree::Token(token) => Some(token),
                TokenTree::Tree { .. } => None,
            })
            .expect("`integer` is only called when the next token is already checked");

        let Token::Integer(radix) = token.kind() else {
            unreachable!("non-integer passed to integer parse function");
        };

        token
            .span()
            .lexeme(self.source)
            .and_then(|lexeme| u64::from_str_radix(lexeme, *radix).ok())
            .map_or_else(
                || Err(Spanned::new(Error::InvalidInteger, token.span())),
                |num| Ok(ast.push_expr(Spanned::new(Expr::Integer(num), token.span()))),
            )
    }

    fn float(&mut self, ast: &mut Ast, _: u16, _: Span) -> Result<ExprIndex, Spanned<Error>> {
        let token = self
            .advance_token_tree()
            .and_then(|token_tree| match token_tree {
                TokenTree::Token(token) => Some(token),
                TokenTree::Tree { .. } => None,
            })
            .expect("`float` is only called when the next token is already checked");

        let Token::Float(radix) = token.kind() else {
            unreachable!("non-float passed to float parse function");
        };

        match token
            .span()
            .lexeme(self.source)
            .and_then(|lexeme| lexeme.split_once('.'))
            .and_then(|(whole, fraction)| {
                Some((
                    i64::from_str_radix(whole, *radix).ok()?,
                    i64::from_str_radix(fraction, *radix).ok()?,
                ))
            }) {
            Some((whole, fraction)) => {
                #[allow(clippy::cast_precision_loss)]
                let power = 10.0f64.powf((fraction as f64).log10().floor() + 1.0);

                #[allow(clippy::cast_precision_loss)]
                let num =
                    (whole as f64) + ((fraction as f64) / if power == 0.0 { 1.0 } else { power });

                Ok(ast.push_expr(Spanned::new(Expr::Float(num), token.span())))
            }
            None => Err(Spanned::new(Error::InvalidFloat, token.span())),
        }
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn so_true(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        Ok(ast.push_expr(Spanned::new(Expr::Boolean(true), span)))
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn so_false(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        Ok(ast.push_expr(Spanned::new(Expr::Boolean(false), span)))
    }

    unary_op!(not, Not);

    fn negate(
        &mut self,
        ast: &mut Ast,
        precedence: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let expr = self.parse_expression(ast, precedence)?;

        if let Expr::Integer(value) = ast[expr].kind() {
            if let Ok(value) = i64::try_from(*value) {
                Ok(ast.push_expr(Spanned::new(
                    Expr::NegativeInteger(-value),
                    span.combine_with(ast[expr].span())
                        .expect("these spans are from the same source"),
                )))
            } else if *value
                == 1 + u64::try_from(i64::MAX).expect("unsigned can always fit signed max")
            {
                Ok(ast.push_expr(Spanned::new(
                    Expr::NegativeInteger(i64::MIN),
                    span.combine_with(ast[expr].span())
                        .expect("these spans are from the same source"),
                )))
            } else {
                Err(Spanned::new(
                    Error::InvalidInteger,
                    span.combine_with(ast[expr].span())
                        .expect("these spans are from the same source"),
                ))
            }
        } else {
            Ok(ast.push_expr(Spanned::new(
                Expr::Unary {
                    op: UnaryOp::Negate,
                    expr,
                },
                span.combine_with(ast[expr].span())
                    .expect("these spans are from the same source"),
            )))
        }
    }

    binary_op!(path_access, PathAccess);
    binary_op!(access, Access);
    binary_op!(multiply, Multiply);
    binary_op!(divide, Divide);
    binary_op!(remainder, Remainder);
    binary_op!(add, Add);
    binary_op!(subtract, Subtract);
    binary_op!(less, Less);
    binary_op!(greater, Greater);
    binary_op!(less_or_equal, LessOrEqual);
    binary_op!(greater_or_equal, GreaterOrEqual);
    binary_op!(not_equal, NotEqual);
    binary_op!(equal, Equal);
    binary_op!(and, And);
    binary_op!(or, Or);
    binary_op!(assign, Assign);

    fn group(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        self.advance_token_tree();

        let expr = self.parse_expression(ast, 0)?;

        Ok(ast.push_expr(Spanned::new(Expr::Group(expr), span)))
    }

    fn block_exprs(&mut self, ast: &mut Ast) -> Result<Vec<ExprIndex>, Spanned<Error>> {
        Ok(
            if let TokenTree::Tree { tokens, .. } =
                self.consume_next_token_tree(TreeKind::Brackets, Error::ExpectedBlock)?
                && !tokens.is_empty()
            {
                let mut exprs = vec![];

                let current_tree = self.current_tree;

                while self.in_token_tree(current_tree) {
                    let expr = self.parse_expression(ast, 0)?;

                    exprs.push(expr);

                    if !matches!(
                        ast[expr].kind(),
                        Expr::Block(_) | Expr::If { .. } | Expr::While { .. } | Expr::AsUnit(_)
                    ) && self.in_token_tree(current_tree)
                    {
                        self.consume_next(Token::Semicolon, Error::BlockWithoutSemicolon)?;
                    }
                }

                exprs
            } else {
                vec![]
            },
        )
    }

    fn block(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        let exprs = self.block_exprs(ast)?;

        Ok(ast.push_expr(Spanned::new(Expr::Block(exprs), span)))
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn self_type_expr(
        &mut self,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        Ok(ast.push_expr(Spanned::new(Expr::SelfType, span)))
    }

    fn path_element_expr(
        &mut self,
        ast: &mut Ast,
        _: u16,
        _: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let path_element = self.path_element()?;

        Ok(ast.push_expr(path_element.transmute(Expr::PathElement)))
    }

    fn name(&mut self, ast: &mut Ast, _: u16, _: Span) -> Result<ExprIndex, Spanned<Error>> {
        let name = self.spanned_name()?;

        Ok(ast.push_expr(name.transmute(Expr::Name)))
    }

    fn let_in(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        let mut exprs = vec![];

        while self.peek_token_tree().is_some() && self.check_keyword_next("in").is_none() {
            let name = self.spanned_name()?;

            let current_tree = self.current_tree;
            let at = self.at;

            let type_signature = if let Some(less) = self.match_next(Token::Less) {
                if let Some(minus) = self.match_next(Token::Minus)
                    && minus.span().start() == less.span().end()
                {
                    Some(self.parse_type_signature()?)
                } else {
                    self.current_tree = current_tree;
                    self.at = at;

                    None
                }
            } else {
                None
            };

            self.consume_next(Token::Equal, Error::LetInWithoutEqual)?;

            let value = self.parse_expression(ast, 0)?;

            let span = name
                .span()
                .combine_with(ast[value].span())
                .expect("these spans are from the same source");

            exprs.push(ast.push_expr(Spanned::new(
                Expr::Let {
                    name,
                    type_signature,
                    value,
                },
                span,
            )));

            if self.check_keyword_next("in").is_none() {
                self.consume_next(Token::Comma, Error::LetInWithoutComma)?;
            }
        }

        self.consume_keyword(Error::LetInWithoutIn, "in")?.span();

        let block_span = self.span_or_end();

        exprs.append(&mut self.block_exprs(ast).map_err(|error| {
            error.transmute(|error| match error {
                Error::ExpectedBlock => Error::LetInWithoutBlock,
                error => error,
            })
        })?);

        let span = span
            .combine_with(block_span)
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(Expr::Block(exprs), span)))
    }

    fn if_expr(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        let condition = self
            .parse_expression(ast, 0)
            .map_err(|error| error.transmute(|_| Error::IfWithoutCondition))?;

        if self.check_next_token_tree(TreeKind::Brackets).is_none() {
            return Err(Spanned::new(Error::IfThenWithoutBlock, self.span_or_end()));
        }

        let when_true_span = self.span_or_end();

        let when_true = self.block(ast, 0, when_true_span).map_err(|error| {
            error.transmute(|error| match error {
                Error::ExpectedBlock => Error::IfThenWithoutBlock,
                error => error,
            })
        })?;

        let otherwise = if self.match_keyword_next("else").is_some() {
            if let Some(next_token) = self.match_keyword_next("if") {
                self.if_expr(ast, 0, next_token.span())?
            } else {
                let otherwise_span = self.span_or_end();

                self.block(ast, 0, otherwise_span).map_err(|error| {
                    error.transmute(|error| match error {
                        Error::ExpectedBlock => Error::IfElseWithoutBlock,
                        error => error,
                    })
                })?
            }
        } else {
            ast.push_expr(Spanned::new(
                Expr::Unit,
                span.combine_with(ast[when_true].span())
                    .expect("these spans are from the same source"),
            ))
        };

        let span = span
            .combine_with(ast[otherwise].span())
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(
            Expr::If {
                condition,
                when_true,
                otherwise,
            },
            span,
        )))
    }

    fn while_expr(
        &mut self,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let condition = self
            .parse_expression(ast, 0)
            .map_err(|error| error.transmute(|_| Error::WhileWithoutCondition))?;

        if self.check_next_token_tree(TreeKind::Brackets).is_none() {
            return Err(Spanned::new(Error::WhileWithoutBlock, self.span_or_end()));
        }

        let when_true_span = self.span_or_end();

        let when_true = self.block(ast, 0, when_true_span).map_err(|error| {
            error.transmute(|error| match error {
                Error::ExpectedBlock => Error::WhileWithoutBlock,
                error => error,
            })
        })?;

        let span = span
            .combine_with(ast[when_true].span())
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(
            Expr::While {
                condition,
                when_true,
            },
            span,
        )))
    }

    fn call_exprs(&mut self, ast: &mut Ast) -> Result<Vec<ExprIndex>, Spanned<Error>> {
        Ok(
            if let TokenTree::Tree { tokens, .. } =
                self.consume_next_token_tree(TreeKind::Parentheses, Error::ExpectedCallArguments)?
                && !tokens.is_empty()
            {
                let mut exprs = vec![];

                let current_tree = self.current_tree;

                while self.in_token_tree(current_tree) {
                    let expr = self.parse_expression(ast, 0)?;

                    exprs.push(expr);

                    if self.in_token_tree(current_tree) {
                        self.consume_next(Token::Comma, Error::CallWithoutComma)?;
                    }
                }

                exprs
            } else {
                vec![]
            },
        )
    }

    fn call(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        let arguments = self.call_exprs(ast)?;

        Ok(ast.push_expr(Spanned::new(Expr::CallNoCallee(arguments), span)))
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn as_unit(&mut self, ast: &mut Ast, _: u16, span: Span) -> Result<ExprIndex, Spanned<Error>> {
        Ok(ast.push_expr(Spanned::new(Expr::AsUnitNoValue, span)))
    }

    fn return_expr(
        &mut self,
        ast: &mut Ast,
        precedence: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let value = self
            .parse_expression(ast, precedence)
            .map_err(|error| error.transmute(|_| Error::ReturnWithoutValue))?;

        let span = span
            .combine_with(ast[value].span())
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(Expr::Return(value), span)))
    }

    fn product_expr(
        &mut self,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let fields = if let TokenTree::Tree { tokens, .. } =
            self.consume_next_token_tree(TreeKind::Brackets, Error::ProductWithoutFields)?
            && !tokens.is_empty()
        {
            let mut fields = vec![];

            let current_tree = self.current_tree;

            while self.in_token_tree(current_tree) {
                let name = self
                    .spanned_name()
                    .map_err(|error| error.transmute(|_| Error::ProductFieldWithoutName))?;

                let less = self.consume_next(Token::Less, Error::ProductFieldWithoutLeftArrow)?;

                let minus = self.consume_next(Token::Minus, Error::ProductFieldWithoutLeftArrow)?;

                if minus.span().start() != less.span().end() {
                    return Err(Spanned::new(
                        Error::ProductFieldWithoutLeftArrow,
                        less.span()
                            .combine_with(minus.span())
                            .expect("these spans are from the same source"),
                    ));
                }

                let value = self.parse_expression(ast, 0)?;

                fields.push((name, value));

                if self.in_token_tree(current_tree) {
                    self.consume_next(Token::Comma, Error::ProductFieldsWithoutComma)?;
                }
            }

            fields
        } else {
            vec![]
        };

        Ok(ast.push_expr(Spanned::new(Expr::ProductNoName(fields), span)))
    }
}
