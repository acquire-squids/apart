use crate::{
    Reportable, Span, Spanned,
    lex::{self, Lexer, Token},
};

use std::{error, fmt, mem};

pub fn parse(lexer: &mut Lexer, ast: &mut Ast) -> Result<(), Vec<Spanned<Error>>> {
    let mut parser = Parser::new(false);

    parser.parse(lexer, ast)
}

struct Parser {
    errors: Vec<Spanned<Error>>,
    is_core: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    ExpectedExpr,
    Lex(lex::Error),
    InvalidInteger,
    InvalidFloat,
    UnclosedGroup,
    UnclosedBlock,
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
    UnclosedFnParameters,
    FnWithoutBody,
    UnclosedFnBody,
    CallWithoutComma,
    UnclosedCall,
    UnclosedFnTypeParameters,
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
}

impl fmt::Display for Error {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedExpr => write!(f, "expected an expression"),
            Self::Lex(lex_error) => write!(f, "{lex_error}"),
            Self::InvalidInteger => write!(f, "this integer is not a valid i64"),
            Self::InvalidFloat => write!(f, "this integer is not a valid f64"),
            Self::UnclosedGroup => write!(f, "this group expression was never closed"),
            Self::UnclosedBlock => write!(f, "this block expression was never closed"),
            Self::BlockWithoutSemicolon => write!(
                f,
                "if there is another expression here, a semicolon should be between it and the previous expression"
            ),
            Self::InvalidName => write!(f, "this is not a valid name"),
            Self::NameIsKeyword => {
                write!(f, "this cannot be used as a name because it is a keyword")
            }
            Self::LetInWithoutEqual => {
                write!(f, "expected an equal sign between the name and its value")
            }
            Self::LetInWithoutIn => write!(f, "expected \"in\" before let expression body"),
            Self::LetInWithoutBlock => write!(f, "the body of a let expression must be a block"),
            Self::IfThenWithoutBlock => {
                write!(f, "the \"then\" branch of an if expression must be a block")
            }
            Self::IfElseWithoutBlock => {
                write!(f, "the \"else\" branch of an if expression must be a block")
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
            Self::UnclosedFnParameters | Self::UnclosedFnTypeParameters => {
                write!(f, "this list of function parameters was never closed")
            }
            Self::FnWithoutBody => {
                write!(
                    f,
                    "expected a block expression to serve as the function body"
                )
            }
            Self::UnclosedFnBody => {
                write!(f, "this function body was never closed")
            }
            Self::CallWithoutComma => write!(
                f,
                "if there is another expression here, a comma should be between it and the previous expression"
            ),
            Self::UnclosedCall => write!(f, "this call was never closed"),
            Self::FnParametersWithoutComma | Self::FnTypeParametersWithoutComma => write!(
                f,
                "if there is another parameter here,a  comma should be between it and the previous parameter"
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
                    "if there is another binding here, a comma should between its name and the previous value"
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
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

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

#[derive(Debug)]
pub enum Expr {
    Integer(i64),
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
    While {
        condition: ExprIndex,
        when_true: ExprIndex,
    },
    Return(ExprIndex),
    AsUnitNoValue,
    AsUnit(ExprIndex),
}

#[derive(Debug)]
pub enum Item {
    Primitive(Spanned<String>),
    NativeFn {
        name: Spanned<String>,
        signature: Spanned<TypeSignature>,
    },
    Fn {
        name: Spanned<String>,
        parameters: Vec<Parameter>,
        return_type: Spanned<TypeSignature>,
        body: ExprIndex,
    },
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
    Name(String),
    Fn {
        parameters: Vec<Spanned<Self>>,
        return_type: Box<Spanned<Self>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
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
    pub fn get_item(&self, index: ItemIndex) -> Option<&Spanned<Item>> {
        self.items.get(usize::from(index))
    }

    #[allow(dead_code)]
    pub fn get_mut_item(&mut self, index: ItemIndex) -> Option<&mut Spanned<Item>> {
        self.items.get_mut(usize::from(index))
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

    #[allow(dead_code)]
    pub fn get_expr(&self, index: ExprIndex) -> Option<&Spanned<Expr>> {
        self.exprs.get(usize::from(index))
    }

    #[allow(dead_code)]
    pub fn get_mut_expr(&mut self, index: ExprIndex) -> Option<&mut Spanned<Expr>> {
        self.exprs.get_mut(usize::from(index))
    }

    fn push_expr(&mut self, expr: Spanned<Expr>) -> ExprIndex {
        let index = ExprIndex(self.exprs.len());

        self.exprs.push(expr);

        index
    }

    fn parse_core(mut self, core_id: usize) -> Self {
        let mut lexer = Lexer::new(core_id);

        lexer.push_source(crate::CORE_SOURCE);

        let mut parser = Parser::new(true);

        if let Err(errors) = parser.parse(&mut lexer, &mut self) {
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
        let Some(expr) = self.get_expr(expr) else {
            return;
        };

        match expr.kind() {
            Expr::Integer(_)
            | Expr::Float(_)
            | Expr::Boolean(_)
            | Expr::Unit
            | Expr::Name(_)
            | Expr::AsUnitNoValue => {}
            Expr::Unary { expr, .. } | Expr::Group(expr) => {
                f(self, *expr);
            }
            Expr::BinaryNoLhs { rhs, .. } => {
                f(self, *rhs);
            }
            Expr::Binary { lhs, rhs, .. } => {
                let (lhs, rhs) = (*lhs, *rhs);

                f(self, lhs);
                f(self, rhs);
            }
            Expr::Block(exprs) => {
                let exprs = exprs.clone();

                for expr in exprs {
                    f(self, expr);
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
                let (condition, when_true, otherwise) = (*condition, *when_true, *otherwise);

                f(self, condition);
                f(self, when_true);
                f(self, otherwise);
            }
            Expr::CallNoCallee(arguments) => {
                let arguments = arguments.clone();

                for argument in arguments {
                    f(self, argument);
                }
            }
            Expr::Call { callee, arguments } => {
                let callee = *callee;
                let arguments = arguments.clone();

                f(self, callee);

                for argument in arguments {
                    f(self, argument);
                }
            }
            Expr::While {
                condition,
                when_true,
            } => {
                let (condition, when_true) = (*condition, *when_true);

                f(self, condition);
                f(self, when_true);
            }
        }
    }

    #[allow(dead_code)]
    pub fn for_children_exprs_mut<F>(&mut self, expr: ExprIndex, mut f: F)
    where
        F: FnMut(&mut Self, ExprIndex),
    {
        let Some(expr) = self.get_expr(expr) else {
            return;
        };

        match expr.kind() {
            Expr::Integer(_)
            | Expr::Float(_)
            | Expr::Boolean(_)
            | Expr::Unit
            | Expr::Name(_)
            | Expr::AsUnitNoValue => {}
            Expr::Unary { expr, .. } | Expr::Group(expr) => {
                f(self, *expr);
            }
            Expr::BinaryNoLhs { rhs, .. } => {
                f(self, *rhs);
            }
            Expr::Binary { lhs, rhs, .. } => {
                let (lhs, rhs) = (*lhs, *rhs);

                f(self, lhs);
                f(self, rhs);
            }
            Expr::Block(exprs) => {
                let exprs = exprs.clone();

                for expr in exprs {
                    f(self, expr);
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
                let (condition, when_true, otherwise) = (*condition, *when_true, *otherwise);

                f(self, condition);
                f(self, when_true);
                f(self, otherwise);
            }
            Expr::CallNoCallee(arguments) => {
                let arguments = arguments.clone();

                for argument in arguments {
                    f(self, argument);
                }
            }
            Expr::Call { callee, arguments } => {
                let callee = *callee;
                let arguments = arguments.clone();

                f(self, callee);

                for argument in arguments {
                    f(self, argument);
                }
            }
            Expr::While {
                condition,
                when_true,
            } => {
                let (condition, when_true) = (*condition, *when_true);

                f(self, condition);
                f(self, when_true);
            }
        }
    }
}

impl Parser {
    const fn new(is_core: bool) -> Self {
        Self {
            errors: vec![],
            is_core,
        }
    }

    fn keyword<'a>(&self, lexer: &'a Lexer, span: Span) -> Option<&'a str> {
        let lexeme = span.lexeme(lexer.source())?;

        match lexeme {
            "primitive" | "native" if self.is_core => Some(lexeme),
            "let" | "in" | "if" | "else" | "true" | "false" | "funky" | "while" | "return" => {
                Some(lexeme)
            }
            _ => None,
        }
    }

    fn advance(&mut self, lexer: &mut Lexer) -> Option<Spanned<Token>> {
        if self.peek(lexer).is_some() {
            lexer.next().and_then(std::result::Result::ok)
        } else {
            None
        }
    }

    fn peek<'a>(&mut self, lexer: &'a mut Lexer) -> Option<&'a Spanned<Token>> {
        while let Some(token_or_error) = lexer.peek() {
            if token_or_error.is_ok() {
                break;
            }

            let Err(error) = lexer.next()? else {
                unreachable!("A lex error was somehow not an error");
            };

            self.errors.push(error.transmute(Error::Lex));
        }

        lexer
            .peek()
            .and_then(|guaranteed_token| guaranteed_token.as_ref().ok())
    }

    fn check_next<'a>(&mut self, lexer: &'a mut Lexer, token: Token) -> Option<&'a Spanned<Token>> {
        self.peek(lexer)
            .filter(|next_token| next_token.kind() == &token)
    }

    fn match_next(&mut self, lexer: &mut Lexer, token: Token) -> Option<Spanned<Token>> {
        self.check_next(lexer, token)
            .map(|_| ())
            .and_then(|()| self.advance(lexer))
    }

    fn consume_next_with_span(
        &mut self,
        lexer: &mut Lexer,
        token: Token,
        error: Error,
        span: Span,
    ) -> Result<Spanned<Token>, Spanned<Error>> {
        self.match_next(lexer, token)
            .ok_or_else(|| Spanned::new(error, span))
    }

    fn consume_next(
        &mut self,
        lexer: &mut Lexer,
        token: Token,
        error: Error,
    ) -> Result<Spanned<Token>, Spanned<Error>> {
        let source_len = lexer.source().len();
        let source_id = lexer.source_id();

        let span = self.peek(lexer).map_or_else(
            || Span::new(source_id, source_len, source_len),
            Spanned::span,
        );

        self.consume_next_with_span(lexer, token, error, span)
    }

    fn check_keyword_next<'a>(
        &mut self,
        lexer: &'a mut Lexer,
        lexeme: &str,
    ) -> Option<&'a Spanned<Token>> {
        if let Some(next_token) = self.peek(lexer)
            && next_token.kind() == &Token::Identifier
            && let next_token_span = next_token.span()
        {
            self.keyword(lexer, next_token_span)
                .and_then(|text| if text == lexeme { Some(()) } else { None })
                .and_then(|()| self.peek(lexer))
        } else {
            None
        }
    }

    fn match_keyword_next(&mut self, lexer: &mut Lexer, lexeme: &str) -> Option<Spanned<Token>> {
        self.check_keyword_next(lexer, lexeme)
            .map(|_| ())
            .and_then(|()| self.advance(lexer))
    }

    fn consume_name(
        &mut self,
        lexer: &mut Lexer,
        error: Error,
    ) -> Result<Spanned<Token>, Spanned<Error>> {
        if let Some(next_token) = self.peek(lexer)
            && next_token.kind() == &Token::Identifier
            && let next_token_span = next_token.span()
        {
            self.keyword(lexer, next_token_span)
                .map_or_else(|| Some(()), |_| None)
        } else {
            None
        }
        .and_then(|()| self.advance(lexer))
        .ok_or_else(|| {
            let span = Span::new(
                lexer.source_id(),
                lexer.source().len(),
                lexer.source().len(),
            );

            Spanned::new(error, self.peek(lexer).map_or(span, Spanned::span))
        })
    }

    fn consume_keyword(
        &mut self,
        lexer: &mut Lexer,
        error: Error,
        lexeme: &str,
    ) -> Result<Spanned<Token>, Spanned<Error>> {
        self.check_keyword_next(lexer, lexeme)
            .map(|_| ())
            .and_then(|()| self.advance(lexer))
            .ok_or_else(|| {
                let span = Span::new(
                    lexer.source_id(),
                    lexer.source().len(),
                    lexer.source().len(),
                );

                Spanned::new(error, self.peek(lexer).map_or(span, Spanned::span))
            })
    }
}

impl Parser {
    fn parse(&mut self, lexer: &mut Lexer, ast: &mut Ast) -> Result<(), Vec<Spanned<Error>>> {
        let mut succeeded = false;

        while self.peek(lexer).is_some() {
            match self.parse_item(lexer, ast) {
                Err(error) => {
                    if succeeded {
                        self.errors.push(error);
                        succeeded = false;
                    }

                    self.advance(lexer);
                }
                Ok(item) => {
                    succeeded = true;
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

impl Parser {
    fn parse_item(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let token = self
            .peek(lexer)
            .expect("items are only parsed in the main loop");

        if token.kind() == &Token::Identifier {
            let span = token.span();

            match self.keyword(lexer, span) {
                Some("primitive") => {
                    self.advance(lexer);

                    self.parse_primitive(lexer, ast, span)
                }
                Some("native") => {
                    self.advance(lexer);

                    let token =
                        self.consume_next(lexer, Token::Identifier, Error::ExpectedNativeItem)?;

                    let native_span = span;
                    let span = token.span();

                    if self.keyword(lexer, span) == Some("funky") {
                        self.parse_native_function(lexer, ast, native_span)
                    } else {
                        Err(Spanned::new(Error::UnknownNativeItem, token.span()))
                    }
                }
                Some("funky") => {
                    self.advance(lexer);

                    self.parse_fn(lexer, ast, span)
                }
                _ => Err(Spanned::new(Error::ExpectedItem, span)),
            }
        } else {
            Err(Spanned::new(Error::ExpectedItem, token.span()))
        }
    }

    fn parse_primitive(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        span: Span,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .name_lexeme(lexer)
            .map_err(|error| error.transmute(|_| Error::PrimitiveWithoutName))
            .map(|(name, name_span)| Spanned::new(name, name_span))?;

        let semicolon_span = self
            .consume_next(lexer, Token::Semicolon, Error::PrimitiveWithoutSemicolon)?
            .span();

        let span = span
            .combine_with(semicolon_span)
            .expect("these spans are from the same source");

        Ok(ast.push_item(Spanned::new(Item::Primitive(name), span)))
    }

    fn parse_native_function(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        span: Span,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .name_lexeme(lexer)
            .map_err(|error| error.transmute(|_| Error::NativeFnWithoutName))
            .map(|(name, name_span)| Spanned::new(name, name_span))?;

        self.consume_next(lexer, Token::Equal, Error::NativeFnWithoutEqual)?;

        let signature = self
            .parse_type_signature(lexer)
            .map_err(|error| error.transmute(|_| Error::NativeFnWithoutType))?;

        let semicolon_span = self
            .consume_next(lexer, Token::Semicolon, Error::NativeFnWithoutSemicolon)?
            .span();

        let span = span
            .combine_with(semicolon_span)
            .expect("these spans are from the same source");

        Ok(ast.push_item(Spanned::new(Item::NativeFn { name, signature }, span)))
    }

    fn parse_fn(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        span: Span,
    ) -> Result<ItemIndex, Spanned<Error>> {
        let name = self
            .name_lexeme(lexer)
            .map_err(|error| error.transmute(|_| Error::FnWithoutName))
            .map(|(name, name_span)| Spanned::new(name, name_span))?;

        let parameters_span = self
            .consume_next(lexer, Token::OpenParenthesis, Error::FnWithoutParameters)?
            .span();

        let mut parameters = vec![];

        while self.peek(lexer).is_some()
            && self.check_next(lexer, Token::CloseParenthesis).is_none()
        {
            let parameter_name = self.consume_name(lexer, Error::FnParameterWithoutName)?;

            let less = self.consume_next(lexer, Token::Less, Error::FnParameterWithoutLeftArrow)?;

            let minus =
                self.consume_next(lexer, Token::Minus, Error::FnParameterWithoutLeftArrow)?;

            if minus.span().start() != less.span().end() {
                return Err(Spanned::new(
                    Error::FnParameterWithoutLeftArrow,
                    less.span()
                        .combine_with(minus.span())
                        .expect("these spans are from the same source"),
                ));
            }

            let type_signature = self.parse_type_signature(lexer)?;

            parameters.push(Parameter {
                name: Spanned::new(
                    parameter_name
                        .span()
                        .lexeme(lexer.source())
                        .map_or_else(String::new, std::string::ToString::to_string),
                    parameter_name.span(),
                ),
                type_signature,
            });

            if self.check_next(lexer, Token::CloseParenthesis).is_none() {
                self.consume_next(lexer, Token::Comma, Error::FnParametersWithoutComma)?;
            }
        }

        let parameters_end = self
            .consume_next_with_span(
                lexer,
                Token::CloseParenthesis,
                Error::UnclosedFnParameters,
                parameters_span,
            )?
            .span();

        let return_type = if let Some(minus) = self.match_next(lexer, Token::Minus) {
            if let Some(greater) = self.match_next(lexer, Token::Greater)
                && minus.span().end() == greater.span().start()
            {
                self.parse_type_signature(lexer)?
            } else {
                lexer.restore(minus.span());

                Spanned::new(TypeSignature::Name("unit".to_string()), parameters_end)
            }
        } else {
            Spanned::new(TypeSignature::Name("unit".to_string()), parameters_end)
        };

        let body_span = self
            .consume_next(lexer, Token::OpenBracket, Error::FnWithoutBody)?
            .span();

        let mut exprs = vec![];

        self.block_exprs(lexer, ast, &mut exprs)?;

        let fn_end = self
            .consume_next(lexer, Token::CloseBracket, Error::UnclosedFnBody)?
            .span();

        let body = ast.push_expr(Spanned::new(
            Expr::Block(exprs),
            body_span.combine_with(fn_end).unwrap_or(body_span),
        ));

        let funky = ast.push_item(Spanned::new(
            Item::Fn {
                name,
                parameters,
                return_type,
                body,
            },
            span.combine_with(fn_end).unwrap_or(span),
        ));

        Ok(funky)
    }

    fn parse_type_signature(
        &mut self,
        lexer: &mut Lexer,
    ) -> Result<Spanned<TypeSignature>, Spanned<Error>> {
        let token = self.peek(lexer);

        match token.map(Spanned::kind) {
            Some(Token::Identifier) => {
                let span = token
                    .expect("if the token has a kind, it has a span")
                    .span();

                match self.keyword(lexer, span) {
                    Some("funky") => {
                        self.advance(lexer);

                        let open_span = self
                            .consume_next(
                                lexer,
                                Token::OpenParenthesis,
                                Error::FnTypeWithoutParameters,
                            )?
                            .span();

                        let mut parameters = vec![];

                        while self.peek(lexer).is_some()
                            && self.check_next(lexer, Token::CloseParenthesis).is_none()
                        {
                            parameters.push(self.parse_type_signature(lexer)?);

                            if self.check_next(lexer, Token::CloseParenthesis).is_none() {
                                self.consume_next(
                                    lexer,
                                    Token::Comma,
                                    Error::FnTypeParametersWithoutComma,
                                )?;
                            }
                        }

                        let close_span = self
                            .consume_next_with_span(
                                lexer,
                                Token::CloseParenthesis,
                                Error::UnclosedFnTypeParameters,
                                open_span,
                            )?
                            .span();

                        let span = span
                            .combine_with(close_span)
                            .expect("these spans are from the same source");

                        let return_type = if let Some(minus) = self.match_next(lexer, Token::Minus)
                        {
                            if let Some(greater) = self.match_next(lexer, Token::Greater)
                                && minus.span().end() == greater.span().start()
                            {
                                self.parse_type_signature(lexer)?
                            } else {
                                lexer.restore(minus.span());

                                Spanned::new(TypeSignature::Name("unit".to_string()), close_span)
                            }
                        } else {
                            Spanned::new(TypeSignature::Name("unit".to_string()), close_span)
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
                    Some(_) => Err(Spanned::new(Error::NameIsKeyword, span)),
                    None => {
                        let (name, span) = self.name_lexeme(lexer)?;

                        Ok(Spanned::new(TypeSignature::Name(name), span))
                    }
                }
            }
            Some(_) => Err(Spanned::new(
                Error::ExpectedType,
                token
                    .map(Spanned::span)
                    .expect("if the token has a kind, it has a span"),
            )),
            None => Err(Spanned::new(
                Error::ExpectedType,
                Span::new(
                    lexer.source_id(),
                    lexer.source().len(),
                    lexer.source().len(),
                ),
            )),
        }
    }
}

mod precedence {
    use super::{Ast, Error, ExprIndex, Lexer, Parser, Span, Spanned};

    pub type ParseFn =
        fn(&mut Parser, &mut Lexer, &mut Ast, u16, Span) -> Result<ExprIndex, Spanned<Error>>;

    pub const PRIMARY: u16 = 0xEE00;

    pub const CALL: u16 = 0xBB00;

    pub const NOT: u16 = 0xAA00;

    pub const NEGATE: u16 = 0xAA00;

    pub const LEFT_MULTIPLY: u16 = 0x9950;
    pub const RIGHT_MULTIPLY: u16 = 0x9900;

    pub const LEFT_DIVIDE: u16 = 0x9950;
    pub const RIGHT_DIVIDE: u16 = 0x9900;

    pub const LEFT_REMAINDER: u16 = 0x9950;
    pub const RIGHT_REMAINDER: u16 = 0x9900;

    pub const LEFT_ADD: u16 = 0x8850;
    pub const RIGHT_ADD: u16 = 0x8800;

    pub const LEFT_SUBTRACT: u16 = 0x8850;
    pub const RIGHT_SUBTRACT: u16 = 0x8800;

    pub const LEFT_LESS: u16 = 0x7750;
    pub const RIGHT_LESS: u16 = 0x7700;

    pub const LEFT_GREATER: u16 = 0x7750;
    pub const RIGHT_GREATER: u16 = 0x7700;

    pub const LEFT_LESS_OR_EQUAL: u16 = 0x7750;
    pub const RIGHT_LESS_OR_EQUAL: u16 = 0x7700;

    pub const LEFT_GREATER_OR_EQUAL: u16 = 0x7750;
    pub const RIGHT_GREATER_OR_EQUAL: u16 = 0x7700;

    pub const LEFT_EQUAL: u16 = 0x6650;
    pub const RIGHT_EQUAL: u16 = 0x6600;

    pub const LEFT_NOT_EQUAL: u16 = 0x6650;
    pub const RIGHT_NOT_EQUAL: u16 = 0x6600;

    pub const LEFT_AND: u16 = 0x5550;
    pub const RIGHT_AND: u16 = 0x5500;

    pub const LEFT_OR: u16 = 0x4450;
    pub const RIGHT_OR: u16 = 0x4400;

    pub const RETURN: u16 = 0x3800;

    pub const LEFT_ASSIGN: u16 = 0x3300;
    pub const RIGHT_ASSIGN: u16 = 0x3350;

    pub const AS_UNIT: u16 = 0x1100;
}

macro_rules! infix_op_precedence {
    (
        $parser:ident, $lexer:ident ;
        $(2nd token $dual_kind:ident => $dual_parse_fn:ident ($dual_left_precedence:ident, $dual_right_precedence:ident), $($is_dual:lifetime)?)?
        $parse_fn:ident ($left_precedence:ident, $right_precedence:ident) $(,)?
    ) => {{
        let token = $parser.advance($lexer)?;

        $($($is_dual)? if let Some(next_token) = $parser.peek($lexer)
            && matches!(next_token.kind(), $crate::lex::Token::$dual_kind)
            && token.span().end() == next_token.span().start()
        {
            let next_token = $parser.advance($lexer)?;

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
            lexer: &mut $crate::lex::Lexer,
            ast: &mut $crate::parse::Ast,
            precedence: u16,
            span: $crate::Span,
        ) -> Result<$crate::parse::ExprIndex, $crate::Spanned<$crate::parse::Error>> {
            let expr = self.parse_expression(lexer, ast, precedence)?;

            Ok(ast.push_expr($crate::Spanned::new(
                $crate::parse::Expr::Unary {
                    op: $crate::parse::UnaryOp::$op,
                    expr,
                },
                span.combine_with(ast.get_expr(expr).map_or(span, $crate::Spanned::span))
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
            lexer: &mut $crate::lex::Lexer,
            ast: &mut $crate::parse::Ast,
            precedence: u16,
            span: $crate::Span,
        ) -> Result<$crate::parse::ExprIndex, $crate::Spanned<$crate::parse::Error>> {
            let rhs = self.parse_expression(lexer, ast, precedence)?;

            Ok(ast.push_expr($crate::Spanned::new(
                $crate::parse::Expr::BinaryNoLhs {
                    op: $crate::parse::BinaryOp::$op,
                    rhs,
                },
                span.combine_with(ast.get_expr(rhs).map_or(span, $crate::Spanned::span))
                    .expect("These spans are from the same source"),
            )))
        }
    };
}

impl Parser {
    fn parse_expression(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        min_precedence: u16,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let Some((precedence, (prefix_fn, prefix_span))) = self.prefix_precedence(lexer) else {
            let source_len = lexer.source().len();
            let source_id = lexer.source_id();

            return Err(Spanned::new(
                Error::ExpectedExpr,
                self.peek(lexer).map_or_else(
                    || Span::new(source_id, source_len, source_len),
                    Spanned::span,
                ),
            ));
        };

        let mut lhs = prefix_fn(self, lexer, ast, precedence, prefix_span)?;

        loop {
            if let Some((left_precedence, (postfix_fn, postfix_span))) =
                self.postfix_precedence(lexer)
            {
                if left_precedence < min_precedence {
                    lexer.restore(postfix_span);
                    break;
                }

                let unfinished_postfix =
                    postfix_fn(self, lexer, ast, left_precedence, postfix_span)?;

                if let Some(unfinished_postfix) = ast.get_mut_expr(unfinished_postfix) {
                    match unfinished_postfix.kind() {
                        Expr::CallNoCallee(arguments) => {
                            let arguments = arguments.clone();

                            *unfinished_postfix = Spanned::new(
                                Expr::Call {
                                    callee: lhs,
                                    arguments,
                                },
                                unfinished_postfix
                                    .span()
                                    .combine_with(prefix_span)
                                    .expect("these spans are from the same source"),
                            );
                        }
                        Expr::AsUnitNoValue => {
                            *unfinished_postfix = Spanned::new(
                                Expr::AsUnit(lhs),
                                unfinished_postfix
                                    .span()
                                    .combine_with(prefix_span)
                                    .expect("these spans are from the same source"),
                            );
                        }
                        _ => {
                            unreachable!("a postfix expression was unaccounted for")
                        }
                    }
                }

                lhs = unfinished_postfix;

                continue;
            }

            if let Some(((left_precedence, right_precedence), (infix_fn, infix_span))) =
                self.infix_precedence(lexer)
            {
                if left_precedence < min_precedence {
                    lexer.restore(infix_span);
                    break;
                }

                let unfinished_infix = infix_fn(self, lexer, ast, right_precedence, infix_span)?;

                if let Some(unfinished_infix) = ast.get_mut_expr(unfinished_infix) {
                    match unfinished_infix.kind() {
                        Expr::BinaryNoLhs { op, rhs } => {
                            let (op, rhs) = (*op, *rhs);

                            *unfinished_infix = Spanned::new(
                                Expr::Binary { lhs, op, rhs },
                                unfinished_infix
                                    .span()
                                    .combine_with(prefix_span)
                                    .expect("these spans are from the same source"),
                            );
                        }
                        _ => {
                            unreachable!("an infix expression was unaccounted for");
                        }
                    }
                }

                lhs = unfinished_infix;

                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn prefix_precedence(
        &mut self,
        lexer: &mut Lexer,
    ) -> Option<(u16, (precedence::ParseFn, Span))> {
        let token = self.peek(lexer)?;

        match token.kind() {
            Token::Minus => Some((precedence::NEGATE, (Self::negate, token.span()))),
            Token::Bang => Some((precedence::NOT, (Self::not, token.span()))),
            Token::Integer(_) => Some((precedence::PRIMARY, (Self::integer, token.span()))),
            Token::Float(_) => Some((precedence::PRIMARY, (Self::float, token.span()))),
            Token::OpenParenthesis => {
                let token = self.advance(lexer)?;

                Some((precedence::PRIMARY, (Self::group, token.span())))
            }
            Token::OpenBracket => {
                let token = self.advance(lexer)?;

                Some((precedence::PRIMARY, (Self::block, token.span())))
            }
            Token::Identifier => {
                let span = token.span();

                match self.keyword(lexer, span) {
                    Some("let") => {
                        self.advance(lexer)?;

                        Some((precedence::PRIMARY, (Self::let_in, span)))
                    }
                    Some("if") => {
                        self.advance(lexer)?;

                        Some((precedence::PRIMARY, (Self::if_expr, span)))
                    }
                    Some("while") => {
                        self.advance(lexer)?;

                        Some((precedence::PRIMARY, (Self::while_expr, span)))
                    }
                    Some("return") => {
                        self.advance(lexer)?;

                        Some((precedence::RETURN, (Self::return_expr, span)))
                    }
                    Some("true") => Some((precedence::PRIMARY, (Self::so_true, span))),
                    Some("false") => Some((precedence::PRIMARY, (Self::so_false, span))),
                    Some(_) => None,
                    None => Some((precedence::PRIMARY, (Self::name, span))),
                }
            }
            _ => None,
        }
    }

    fn postfix_precedence(
        &mut self,
        lexer: &mut Lexer,
    ) -> Option<(u16, (precedence::ParseFn, Span))> {
        let token = self.peek(lexer)?;

        match token.kind() {
            Token::OpenParenthesis => {
                let token = self.advance(lexer)?;

                Some((precedence::CALL, (Self::call, token.span())))
            }
            Token::Semicolon => {
                let token = self.advance(lexer)?;

                Some((precedence::AS_UNIT, (Self::as_unit, token.span())))
            }
            _ => None,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infix_precedence(
        &mut self,
        lexer: &mut Lexer,
    ) -> Option<((u16, u16), (precedence::ParseFn, Span))> {
        let token = self.peek(lexer)?;

        match token.kind() {
            Token::Star => infix_op_precedence!(
                self, lexer;
                multiply (LEFT_MULTIPLY, RIGHT_MULTIPLY),
            ),
            Token::Slash => infix_op_precedence!(
                self, lexer;
                divide (LEFT_DIVIDE, RIGHT_DIVIDE),
            ),
            Token::Percent => infix_op_precedence!(
                self, lexer;
                remainder (LEFT_REMAINDER, RIGHT_REMAINDER),
            ),
            Token::Plus => infix_op_precedence!(
                self, lexer;
                add (LEFT_ADD, RIGHT_ADD),
            ),
            Token::Minus => infix_op_precedence!(
                self, lexer;
                subtract (LEFT_SUBTRACT, RIGHT_SUBTRACT),
            ),
            Token::Less => infix_op_precedence!(
                self, lexer;
                2nd token Equal => less_or_equal (LEFT_LESS_OR_EQUAL, RIGHT_LESS_OR_EQUAL),
                less (LEFT_LESS, RIGHT_LESS),
            ),
            Token::Greater => infix_op_precedence!(
                self, lexer;
                2nd token Equal => greater_or_equal (LEFT_GREATER_OR_EQUAL, RIGHT_GREATER_OR_EQUAL),
                greater (LEFT_GREATER, RIGHT_GREATER),
            ),
            Token::Equal => infix_op_precedence!(
                self, lexer;
                2nd token Equal => equal (LEFT_EQUAL, RIGHT_EQUAL),
                assign (LEFT_ASSIGN, RIGHT_ASSIGN),
            ),
            Token::Bang => {
                let token = self.advance(lexer)?;

                if let Some(next_token) = self.peek(lexer)
                    && matches!(next_token.kind(), Token::Equal)
                    && token.span().end() == next_token.span().start()
                {
                    let next_token = self.advance(lexer)?;

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
                    lexer.restore(token.span());

                    None
                }
            }
            Token::Ampersand => {
                let token = self.advance(lexer)?;

                if let Some(next_token) = self.peek(lexer)
                    && matches!(next_token.kind(), Token::Ampersand)
                    && token.span().end() == next_token.span().start()
                {
                    let next_token = self.advance(lexer)?;

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
                    lexer.restore(token.span());

                    None
                }
            }
            Token::Pipe => {
                let token = self.advance(lexer)?;

                if let Some(next_token) = self.peek(lexer)
                    && matches!(next_token.kind(), Token::Pipe)
                    && token.span().end() == next_token.span().start()
                {
                    let next_token = self.advance(lexer)?;

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
                    lexer.restore(token.span());

                    None
                }
            }
            _ => None,
        }
    }

    fn integer(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        _: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let token = self
            .advance(lexer)
            .expect("`integer` is only called when the next token is already checked");

        let Token::Integer(radix) = token.kind() else {
            unreachable!("non-integer passed to integer parse function");
        };

        token
            .span()
            .lexeme(lexer.source())
            .and_then(|lexeme| i64::from_str_radix(lexeme, *radix).ok())
            .map_or_else(
                || Err(Spanned::new(Error::InvalidInteger, token.span())),
                |num| Ok(ast.push_expr(Spanned::new(Expr::Integer(num), token.span()))),
            )
    }

    fn float(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        _: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let token = self
            .advance(lexer)
            .expect("`float` is only called when the next token is already checked");

        let Token::Float(radix) = token.kind() else {
            unreachable!("non-float passed to float parse function");
        };

        match token
            .span()
            .lexeme(lexer.source())
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

    #[allow(clippy::unnecessary_wraps)]
    fn so_true(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        _: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let span = self
            .match_keyword_next(lexer, "true")
            .as_ref()
            .map(Spanned::span)
            .expect("`so_true` is only called when the next token is already checked");

        Ok(ast.push_expr(Spanned::new(Expr::Boolean(true), span)))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn so_false(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        _: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let span = self
            .match_keyword_next(lexer, "false")
            .as_ref()
            .map(Spanned::span)
            .expect("`so_false` is only called when the next token is already checked");

        Ok(ast.push_expr(Spanned::new(Expr::Boolean(false), span)))
    }

    unary_op!(negate, Negate);
    unary_op!(not, Not);

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

    fn group(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let expr = self.parse_expression(lexer, ast, 0)?;

        let span = span
            .combine_with(
                self.consume_next_with_span(
                    lexer,
                    Token::CloseParenthesis,
                    Error::UnclosedGroup,
                    span,
                )?
                .span(),
            )
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(Expr::Group(expr), span)))
    }

    fn block_exprs(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        exprs: &mut Vec<ExprIndex>,
    ) -> Result<(), Spanned<Error>> {
        while self.peek(lexer).is_some() && self.check_next(lexer, Token::CloseBracket).is_none() {
            let expr = self.parse_expression(lexer, ast, 0)?;

            exprs.push(expr);

            if !matches!(
                ast.get_expr(expr).map(Spanned::kind),
                Some(Expr::Block(_) | Expr::If { .. } | Expr::While { .. } | Expr::AsUnit(_))
            ) && self.check_next(lexer, Token::CloseBracket).is_none()
            {
                self.consume_next(lexer, Token::Semicolon, Error::BlockWithoutSemicolon)?;
            }
        }

        Ok(())
    }

    fn block(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let mut exprs = vec![];

        self.block_exprs(lexer, ast, &mut exprs)?;

        let span = span
            .combine_with(
                self.consume_next_with_span(
                    lexer,
                    Token::CloseBracket,
                    Error::UnclosedBlock,
                    span,
                )?
                .span(),
            )
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(Expr::Block(exprs), span)))
    }

    fn name_lexeme(&mut self, lexer: &mut Lexer) -> Result<(String, Span), Spanned<Error>> {
        let name_span = self.consume_name(lexer, Error::InvalidName)?.span();

        Ok((
            name_span
                .lexeme(lexer.source())
                .ok_or_else(|| {
                    Spanned::new(
                        Error::InvalidName,
                        Span::new(
                            lexer.source_id(),
                            lexer.source().len(),
                            lexer.source().len(),
                        ),
                    )
                })?
                .to_string(),
            name_span,
        ))
    }

    fn name(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        _: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let (name, name_span) = self.name_lexeme(lexer)?;

        Ok(ast.push_expr(Spanned::new(Expr::Name(name), name_span)))
    }

    fn let_in(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let mut exprs = vec![];

        while self.peek(lexer).is_some() && self.check_keyword_next(lexer, "in").is_none() {
            let (name, name_span) = self.name_lexeme(lexer)?;

            let type_signature = if let Some(less) = self.match_next(lexer, Token::Less) {
                if let Some(minus) = self.match_next(lexer, Token::Minus)
                    && minus.span().start() == less.span().end()
                {
                    Some(self.parse_type_signature(lexer)?)
                } else {
                    lexer.restore(less.span());

                    None
                }
            } else {
                None
            };

            let equal = self.consume_next(lexer, Token::Equal, Error::LetInWithoutEqual)?;

            let value = self.parse_expression(lexer, ast, 0)?;

            exprs.push(
                ast.push_expr(Spanned::new(
                    Expr::Let {
                        name: Spanned::new(name, name_span),
                        type_signature,
                        value,
                    },
                    name_span
                        .combine_with(
                            ast.get_expr(value)
                                .map_or_else(|| equal.span(), Spanned::span),
                        )
                        .expect("these spans are from the same source"),
                )),
            );

            if self.check_keyword_next(lexer, "in").is_none() {
                self.consume_next(lexer, Token::Comma, Error::LetInWithoutComma)?;
            }
        }

        self.consume_keyword(lexer, Error::LetInWithoutIn, "in")?
            .span();

        self.consume_next(lexer, Token::OpenBracket, Error::LetInWithoutBlock)?;

        self.block_exprs(lexer, ast, &mut exprs)?;

        let span = span
            .combine_with(
                self.consume_next_with_span(
                    lexer,
                    Token::CloseBracket,
                    Error::UnclosedBlock,
                    span,
                )?
                .span(),
            )
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(Expr::Block(exprs), span)))
    }

    fn if_expr(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let condition = self
            .parse_expression(lexer, ast, 0)
            .map_err(|error| error.transmute(|_| Error::IfWithoutCondition))?;

        if self.check_next(lexer, Token::OpenBracket).is_none() {
            let source_len = lexer.source().len();
            let source_id = lexer.source_id();

            return Err(Spanned::new(
                Error::IfThenWithoutBlock,
                self.peek(lexer).map_or_else(
                    || Span::new(source_id, source_len, source_len),
                    Spanned::span,
                ),
            ));
        }

        let when_true_span = self
            .consume_next(lexer, Token::OpenBracket, Error::IfThenWithoutBlock)?
            .span();

        let when_true = self.block(lexer, ast, 0, when_true_span)?;

        let otherwise = if let Some(next_token) = self.peek(lexer)
            && next_token.kind() == &Token::Identifier
            && self.match_keyword_next(lexer, "else").is_some()
        {
            if let Some(next_token) = self.peek(lexer)
                && next_token.kind() == &Token::Identifier
                && let next_token_span = next_token.span()
                && self.match_keyword_next(lexer, "if").is_some()
            {
                self.if_expr(lexer, ast, 0, next_token_span)?
            } else {
                let otherwise_span = self
                    .consume_next(lexer, Token::OpenBracket, Error::IfElseWithoutBlock)?
                    .span();

                self.block(lexer, ast, 0, otherwise_span)?
            }
        } else {
            ast.push_expr(Spanned::new(
                Expr::Unit,
                span.combine_with(ast.get_expr(when_true).map_or(span, Spanned::span))
                    .expect("these spans are from the same source"),
            ))
        };

        let span = span
            .combine_with(ast.get_expr(otherwise).map_or(span, Spanned::span))
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
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let condition = self
            .parse_expression(lexer, ast, 0)
            .map_err(|error| error.transmute(|_| Error::WhileWithoutCondition))?;

        if self.check_next(lexer, Token::OpenBracket).is_none() {
            let source_len = lexer.source().len();
            let source_id = lexer.source_id();

            return Err(Spanned::new(
                Error::WhileWithoutBlock,
                self.peek(lexer).map_or_else(
                    || Span::new(source_id, source_len, source_len),
                    Spanned::span,
                ),
            ));
        }

        let when_true_span = self
            .consume_next(lexer, Token::OpenBracket, Error::WhileWithoutBlock)?
            .span();

        let when_true = self.block(lexer, ast, 0, when_true_span)?;

        let span = span
            .combine_with(ast.get_expr(when_true).map_or(span, Spanned::span))
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(
            Expr::While {
                condition,
                when_true,
            },
            span,
        )))
    }

    fn call_exprs(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        exprs: &mut Vec<ExprIndex>,
    ) -> Result<(), Spanned<Error>> {
        while self.peek(lexer).is_some()
            && self.check_next(lexer, Token::CloseParenthesis).is_none()
        {
            let expr = self.parse_expression(lexer, ast, 0)?;

            exprs.push(expr);

            if self.check_next(lexer, Token::CloseParenthesis).is_none() {
                self.consume_next(lexer, Token::Comma, Error::CallWithoutComma)?;
            }
        }

        Ok(())
    }

    fn call(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let mut arguments = vec![];

        self.call_exprs(lexer, ast, &mut arguments)?;

        let call_end = self
            .consume_next_with_span(lexer, Token::CloseParenthesis, Error::UnclosedCall, span)?
            .span();

        Ok(ast.push_expr(Spanned::new(
            Expr::CallNoCallee(arguments),
            span.combine_with(call_end).unwrap_or(span),
        )))
    }

    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    fn as_unit(
        &mut self,
        _: &mut Lexer,
        ast: &mut Ast,
        _: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        Ok(ast.push_expr(Spanned::new(Expr::AsUnitNoValue, span)))
    }

    fn return_expr(
        &mut self,
        lexer: &mut Lexer,
        ast: &mut Ast,
        precedence: u16,
        span: Span,
    ) -> Result<ExprIndex, Spanned<Error>> {
        let value = self
            .parse_expression(lexer, ast, precedence)
            .map_err(|error| error.transmute(|_| Error::ReturnWithoutValue))?;

        let span = span
            .combine_with(
                ast.get_expr(value)
                    .expect("an expression is guaranteed since we made it to here")
                    .span(),
            )
            .expect("these spans are from the same source");

        Ok(ast.push_expr(Spanned::new(Expr::Return(value), span)))
    }
}
