use crate::{
    Reportable, Span, Spanned,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, TypeSignature, UnaryOp},
};

use std::{collections::HashMap, error, fmt};

pub fn check_types(
    ast: &Ast,
    names: &HashMap<Span, Span>,
) -> Result<TypeChecker, Vec<Spanned<Error>>> {
    let mut type_checker = TypeChecker::new();

    type_checker.check_primitives(ast);

    type_checker.check_native_functions(ast, names);

    type_checker.check_functions(ast, names);

    type_checker.type_check_functions(ast, names);

    type_checker.check_for_main(ast);

    if type_checker.errors.is_empty() {
        Ok(type_checker)
    } else {
        Err(type_checker.errors)
    }
}

pub struct TypeChecker {
    errors: Vec<Spanned<Error>>,
    type_map: HashMap<Span, TypeIndex>,
    types: Vec<Type>,
    fn_return_type: Option<TypeIndex>,
    last_in_fn: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    I64,
    F64,
    Boolean,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Primitive(Primitive),
    Unknown,
    Fn {
        parameters: Vec<TypeIndex>,
        return_type: TypeIndex,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeIndex(usize);

impl From<TypeIndex> for usize {
    fn from(value: TypeIndex) -> Self {
        value.0
    }
}

impl Type {
    fn to_string(&self, types: &[Self]) -> String {
        match self {
            Self::Primitive(p) => match p {
                Primitive::I64 => "i64",
                Primitive::F64 => "f64",
                Primitive::Boolean => "bool",
                Primitive::Unit => "unit",
            }
            .to_string(),
            Self::Unknown => "!!UNKNOWN TYPE!!".to_string(),
            Self::Fn {
                parameters,
                return_type,
            } => {
                let mut buffer = "Funky(".to_string();

                if let Some(parameter) = parameters
                    .first()
                    .and_then(|parameter| types.get(usize::from(*parameter)))
                {
                    buffer.push_str(parameter.to_string(types).as_str());
                }

                for parameter in parameters
                    .iter()
                    .skip(1)
                    .filter_map(|parameter| types.get(usize::from(*parameter)))
                {
                    buffer.push_str(parameter.to_string(types).as_str());
                }

                buffer.push(')');

                if let Some(return_type) = types.get(usize::from(*return_type))
                    && return_type != &Self::Primitive(Primitive::Unit)
                {
                    buffer.push_str(" = ");
                    buffer.push_str(return_type.to_string(types).as_str());
                }

                buffer
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Error {
    TypeMismatch { expected: String, got: String },
    UnknownType,
    ArithmeticImpossible,
    CannotCompare,
    ConditionNotBoolean,
    CallArgumentCountMismatch { expected: usize, got: usize },
    CalledUncallable,
    MainFnWithParameters,
    MainFnWithReturnType,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, but got {got}")
            }
            Self::UnknownType => {
                write!(f, "the type of this expression is unknown")
            }
            Self::ArithmeticImpossible => {
                write!(f, "arithmetic cannot be performed with this expression")
            }
            Self::CannotCompare => {
                write!(
                    f,
                    "only numbers of the same type can be compared in this way"
                )
            }
            Self::ConditionNotBoolean => {
                write!(f, "conditions can only be booleans")
            }
            Self::CallArgumentCountMismatch { expected, got } => {
                write!(f, "expected {expected} arguments, but got {got}")
            }
            Self::CalledUncallable => write!(f, "this expression cannot be called"),
            Self::MainFnWithParameters => {
                write!(f, "the \"main\" function should take no parameters")
            }
            Self::MainFnWithReturnType => write!(f, "the \"main\" function should return unit"),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

impl TypeChecker {
    fn new() -> Self {
        let mut me = Self {
            errors: vec![],
            type_map: HashMap::new(),
            types: vec![],
            fn_return_type: None,
            last_in_fn: false,
        };

        me.push_type(Type::Unknown);

        me
    }

    #[allow(dead_code)]
    pub fn get_type(&self, type_index: TypeIndex) -> Option<&Type> {
        self.types.get(usize::from(type_index))
    }

    #[allow(dead_code)]
    pub fn get_type_index(&self, span: Span) -> Option<TypeIndex> {
        self.type_map.get(&span).copied()
    }

    fn push_type(&mut self, ty: Type) -> TypeIndex {
        let type_index = TypeIndex(self.types.len());

        self.types.push(ty);

        type_index
    }

    fn get_type_index_or_error(&mut self, span: Span) -> TypeIndex {
        if !self.type_map.contains_key(&span) {
            self.errors.push(Spanned::new(Error::UnknownType, span));

            self.type_map.insert(span, self.type_unknown());
        }

        self.type_map
            .get(&span)
            .copied()
            .expect("expressions always receive a fallback type if they don't have a type when `get_type_index_or_error` is called")
    }

    fn expr_type(&mut self, ast: &Ast, expr: ExprIndex) -> &Type {
        let type_index = self
            .get_type_index_or_error(ast.get_expr(expr).map(Spanned::span).expect(
            "`expr_type` is only called by type checking functions with valid expression indices",
        ));

        self.types
            .get(usize::from(type_index))
            .expect("expressions always receive a fallback type if they don't have a type when `get_type_index_or_error` is called")
    }

    fn type_integer(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(Primitive::I64)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_float(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(Primitive::F64)))
                .expect("the f64 type should be initialized before all type checking"),
        )
    }

    fn type_boolean(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(Primitive::Boolean)))
                .expect("the bool type should be initialized before all type checking"),
        )
    }

    fn type_unit(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(Primitive::Unit)))
                .expect("the unit type should be initialized before all type checking"),
        )
    }

    fn type_unknown(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Unknown))
                .expect("The unknown type should be initialized before all type checking"),
        )
    }
}

impl TypeChecker {
    fn check_primitives(&mut self, ast: &Ast) {
        for root in ast.roots() {
            if let Some(Item::Primitive(name)) = ast.get_item(*root).map(Spanned::kind) {
                let type_index = match name.kind().as_str() {
                    "i64" => self.push_type(Type::Primitive(Primitive::I64)),
                    "f64" => self.push_type(Type::Primitive(Primitive::F64)),
                    "bool" => self.push_type(Type::Primitive(Primitive::Boolean)),
                    "unit" => self.push_type(Type::Primitive(Primitive::Unit)),
                    _ => {
                        unreachable!("unknown primitive declared in core");
                    }
                };

                self.type_map.insert(name.span(), type_index);
            }
        }
    }

    fn check_native_functions(&mut self, ast: &Ast, names: &HashMap<Span, Span>) {
        for root in ast.roots() {
            if let Some(Item::NativeFn { name, signature }) = ast.get_item(*root).map(Spanned::kind)
                && let TypeSignature::Fn {
                    parameters,
                    return_type,
                } = signature.kind()
            {
                let parameters = parameters
                    .iter()
                    .map(|parameter| {
                        self.check_type_signature(names, parameter);

                        self.get_type_index_or_error(parameter.span())
                    })
                    .collect::<Vec<_>>();

                let return_type = {
                    self.check_type_signature(names, return_type);

                    self.get_type_index_or_error(return_type.span())
                };

                let type_index = self.push_type(Type::Fn {
                    parameters,
                    return_type,
                });

                self.type_map.insert(name.span(), type_index);
            }
        }
    }

    fn check_type_signature(&mut self, names: &HashMap<Span, Span>, ty: &Spanned<TypeSignature>) {
        match ty.kind() {
            TypeSignature::Name(_) => {
                let span = ty.span();

                if let Some(name) = names.get(&span) {
                    let name_type = self.get_type_index_or_error(*name);

                    self.type_map.insert(span, name_type);
                } else {
                    self.type_map.insert(span, self.type_unknown());
                }
            }
            TypeSignature::Fn {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.check_type_signature(names, parameter);
                }

                self.check_type_signature(names, return_type);
            }
        }
    }

    fn check_functions(&mut self, ast: &Ast, names: &HashMap<Span, Span>) {
        for root in ast.roots() {
            if let Some(Item::Fn {
                name,
                parameters,
                return_type,
                ..
            }) = ast.get_item(*root).map(Spanned::kind)
            {
                let parameters = parameters
                    .iter()
                    .map(|parameter| {
                        self.check_type_signature(names, parameter.ty());

                        let type_index = self
                            .get_type_index(parameter.ty().span())
                            .expect("the type should have been set in the previous call");

                        self.type_map.insert(parameter.name().span(), type_index);

                        self.get_type_index_or_error(parameter.name().span())
                    })
                    .collect::<Vec<_>>();

                let return_type = {
                    self.check_type_signature(names, return_type);

                    self.get_type_index_or_error(return_type.span())
                };

                let type_index = self.push_type(Type::Fn {
                    parameters,
                    return_type,
                });

                self.type_map.insert(name.span(), type_index);
            }
        }
    }

    fn check_for_main(&mut self, ast: &Ast) {
        for root in ast.roots() {
            if let Some(Item::Fn {
                name, parameters, ..
            }) = ast.get_item(*root).map(Spanned::kind)
                && name.kind() == "main"
            {
                if let Some(parameter) = parameters.first() {
                    self.errors.push(Spanned::new(
                        Error::MainFnWithParameters,
                        parameter.name().span(),
                    ));
                }

                if let Some(Type::Fn { return_type, .. }) = self
                    .get_type_index(name.span())
                    .and_then(|type_index| self.get_type(type_index))
                    && !matches!(
                        self.get_type(*return_type),
                        Some(Type::Primitive(Primitive::Unit))
                    )
                {
                    self.errors
                        .push(Spanned::new(Error::MainFnWithReturnType, name.span()));
                }

                break;
            }
        }
    }
}

macro_rules! type_check_binary_op {
    (
        $type_checker:ident, $ast:ident, $expr:ident, ($lhs:ident, $rhs:ident) ;
        lhs_failure: $lhs_failure:expr ;
        $(
            ($allow_lhs:pat, $allow_rhs:pat) => {
                rhs_failure: $rhs_failure:expr,
                success: $success:expr,
            }
        )+
    ) => {{
        #[allow(unreachable_patterns)]
        let ty = match ($type_checker.expr_type($ast, $lhs).clone(), $type_checker.expr_type($ast, $rhs).clone()) {
            ($crate::type_check::Type::Unknown, _) => Ok($type_checker.type_unknown()),
            (_, $crate::type_check::Type::Unknown) => Ok($type_checker.type_unknown()),
            $(
                ($allow_lhs, $allow_rhs) => Ok($success),
            )+
            $(
                ($allow_lhs, rhs_type) => {
                    Err($crate::type_check::Error::TypeMismatch {
                        expected: $rhs_failure.clone().to_string($type_checker.types.as_slice()),
                        got: rhs_type.to_string($type_checker.types.as_slice()),
                    })
                }
                (_, _) => Err($lhs_failure),
            )+
        };

        match ty {
            Ok(ty) => {
                $type_checker.type_map.insert(
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("if the kind exists, the span does too"),
                    ty
                );
            }
            Err(error @ $crate::type_check::Error::TypeMismatch { .. }) => {
                $type_checker.type_map.insert(
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("if the kind exists, the span does too"),
                    $type_checker.type_unknown()
                );

                $type_checker.errors.push($crate::Spanned::new(
                    error,
                    $ast.get_expr($rhs)
                        .map($crate::Spanned::span)
                        .expect("this macro is only used with existing right operands"),
                ));
            }
            Err(error) => {
                $type_checker.type_map.insert(
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("if the kind exists, the span does too"),
                    $type_checker.type_unknown()
                );

                $type_checker.errors.push($crate::Spanned::new(
                    error,
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("this macro is only used with existing expressions"),
                ));
            }
        }
    }};
}

impl TypeChecker {
    fn type_check_functions(&mut self, ast: &Ast, names: &HashMap<Span, Span>) {
        for root in ast.roots() {
            if let Some(Item::Fn { name, body, .. }) = ast.get_item(*root).map(Spanned::kind) {
                let fn_return_type = self.fn_return_type.take();

                self.fn_return_type = self
                    .get_type_index(name.span())
                    .and_then(|type_index| self.get_type(type_index))
                    .and_then(|ty| {
                        if let Type::Fn { return_type, .. } = ty {
                            Some(*return_type)
                        } else {
                            None
                        }
                    });

                let last_in_fn = self.last_in_fn;

                self.last_in_fn = true;

                self.type_check_expr(ast, names, *body);

                self.last_in_fn = last_in_fn;

                self.fn_return_type = fn_return_type;
            }
        }
    }

    fn check_return_type_mismatch(&mut self, ast: &Ast, expr: ExprIndex) {
        if let Some(expr_type) = ast
            .get_expr(expr)
            .map(Spanned::span)
            .and_then(|span| self.get_type_index(span))
            .and_then(|type_index| self.get_type(type_index))
            && let Some(fn_return_type) = self
                .fn_return_type
                .and_then(|type_index| self.get_type(type_index))
            && expr_type != fn_return_type
            && !matches!(expr_type, Type::Unknown)
        {
            self.errors.push(Spanned::new(
                Error::TypeMismatch {
                    expected: fn_return_type.to_string(self.types.as_slice()),
                    got: expr_type.to_string(self.types.as_slice()),
                },
                ast.get_expr(expr)
                    .map(Spanned::span)
                    .expect("is the expression exists, so does the span"),
            ));
        }
    }

    #[allow(clippy::too_many_lines)]
    fn type_check_expr(&mut self, ast: &Ast, names: &HashMap<Span, Span>, expr: ExprIndex) {
        let span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, so does the span");

        match ast.get_expr(expr).map(Spanned::kind) {
            None => {
                unreachable!("All expressions should exist");
            }
            Some(Expr::BinaryNoLhs { .. }) => {
                unreachable!("there should never be a BinaryNoLhs after parsing finished");
            }
            Some(Expr::CallNoCallee(_)) => {
                unreachable!("there should never be a CallNoCallee after parsing finished");
            }
            Some(Expr::AsUnitNoValue) => {
                unreachable!("there should never be a ToUnitNoValue after parsing finished");
            }
            Some(Expr::Integer(_)) => {
                self.type_map.insert(span, self.type_integer());

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
            Some(Expr::Float(_)) => {
                self.type_map.insert(span, self.type_float());

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
            Some(Expr::Boolean(_)) => {
                self.type_map.insert(span, self.type_boolean());

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
            Some(Expr::Unit) => {
                self.type_map.insert(span, self.type_unit());

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
            Some(Expr::Group(inner_expr)) => {
                ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

                let inner_span = ast
                    .get_expr(*inner_expr)
                    .map(Spanned::span)
                    .expect("if the expression exists, so does the span");

                let inner_type = self.get_type_index_or_error(inner_span);

                self.type_map.insert(span, inner_type);
            }
            Some(Expr::Block(exprs)) => {
                self.type_check_block(ast, names, expr, exprs.as_slice());
            }
            Some(Expr::Name(_)) => {
                if let Some(name) = names.get(&span) {
                    let name_type = self.get_type_index_or_error(*name);

                    self.type_map.insert(span, name_type);

                    if self.last_in_fn {
                        self.check_return_type_mismatch(ast, expr);
                    }
                } else {
                    self.type_map.insert(span, self.type_unknown());
                }
            }
            Some(Expr::Let {
                name,
                type_signature,
                value,
            }) => {
                self.type_check_let(
                    ast,
                    names,
                    expr,
                    (name.span(), type_signature.as_ref(), *value),
                );
            }
            Some(Expr::Unary { op, expr: operand }) => {
                self.type_check_unary(ast, names, expr, (*op, *operand));
            }
            Some(Expr::Binary { op, lhs, rhs }) => {
                self.type_check_binary(ast, names, expr, (*op, *lhs, *rhs));
            }
            Some(Expr::If {
                condition,
                when_true,
                otherwise,
            }) => {
                self.type_check_if(ast, names, expr, (*condition, *when_true, *otherwise));
            }
            Some(Expr::Call { callee, arguments }) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

                self.type_check_call(ast, names, expr, (*callee, arguments.as_slice()));

                self.last_in_fn = last_in_fn;

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
            Some(Expr::While { condition, .. }) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

                self.type_check_while(ast, expr, *condition);

                self.last_in_fn = last_in_fn;

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
            Some(Expr::Return(value)) => {
                ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

                self.type_check_return(ast, expr, *value);
            }
            Some(Expr::AsUnit(_)) => {
                let last_in_fn = self.last_in_fn;

                self.last_in_fn = false;

                ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

                self.type_map.insert(span, self.type_unit());

                self.last_in_fn = last_in_fn;

                if self.last_in_fn {
                    self.check_return_type_mismatch(ast, expr);
                }
            }
        }
    }
}

macro_rules! type_check_unary_op {
    (
        $type_checker:ident, $ast:ident, $expr:ident, ($operand:ident) ;
        rhs_failure: $rhs_failure:expr ;
        $(
            $allow:pat => $success:expr,
        )+
    ) => {{
        let ty = match &*$type_checker.expr_type($ast, $operand) {
            $crate::type_check::Type::Unknown => Ok($type_checker.type_unknown()),
            $(
                $allow => Ok($success),
            )+
            operand_type => {
                let operand_type = operand_type.clone();

                Err($crate::type_check::Error::TypeMismatch {
                    expected: $rhs_failure.clone().to_string($type_checker.types.as_slice()),
                    got: operand_type.to_string($type_checker.types.as_slice()),
                })
            }
        };

        match ty {
            Ok(ty) => {
                $type_checker.type_map.insert(
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("if the kind exists, the span does too"),
                    ty,
                );
            }
            Err(error @ $crate::type_check::Error::TypeMismatch { .. }) => {
                $type_checker.type_map.insert(
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("if the kind exists, the span does too"),
                    $type_checker.type_unknown(),
                );

                $type_checker.errors.push($crate::Spanned::new(
                    error,
                    $ast.get_expr($operand)
                        .map($crate::Spanned::span)
                        .expect("this macro is only used with existing operands"),
                ));
            }
            Err(error) => {
                $type_checker.type_map.insert(
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("if the kind exists, the span does too"),
                    $type_checker.type_unknown(),
                );

                $type_checker.errors.push($crate::Spanned::new(
                    error,
                    $ast.get_expr($expr)
                        .map($crate::Spanned::span)
                        .expect("this macro is only used with existing expressions"),
                ));
            }
        }
    }};
}

impl TypeChecker {
    fn type_check_block(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        exprs: &[ExprIndex],
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        for expr in exprs.iter().rev().skip(1).rev() {
            self.type_check_expr(ast, names, *expr);
        }

        self.last_in_fn = last_in_fn;

        let span = ast
            .get_expr(expr)
            .expect("`type_check_block` is only called on existing blocks")
            .span();

        if let Some(last_expr) = exprs.last() {
            self.type_check_expr(ast, names, *last_expr);

            let last_span = ast
                .get_expr(*last_expr)
                .map(Spanned::span)
                .expect("if the expression exists, so does the span");

            let last_expr_type = self.get_type_index_or_error(last_span);

            self.type_map.insert(span, last_expr_type);
        } else {
            self.type_map.insert(span, self.type_unit());

            if self.last_in_fn {
                self.check_return_type_mismatch(ast, expr);
            }
        }
    }

    fn type_check_let(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (span, type_signature, value): (Span, Option<&Spanned<TypeSignature>>, ExprIndex),
    ) {
        ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

        let value_span = ast
            .get_expr(value)
            .map(Spanned::span)
            .expect("if the expression exists, so does the span");

        let value_type = self.get_type_index_or_error(value_span);

        if let Some(type_signature) = type_signature {
            self.check_type_signature(names, type_signature);

            let expected_type = self.get_type_index_or_error(type_signature.span());

            if let Some(value_type) = self.get_type(value_type)
                && let Some(expected_type) = self.get_type(expected_type)
                && value_type != expected_type
            {
                self.errors.push(Spanned::new(
                    Error::TypeMismatch {
                        expected: expected_type.to_string(self.types.as_slice()),
                        got: value_type.to_string(self.types.as_slice()),
                    },
                    value_span,
                ));
            }
        }

        self.type_map.insert(span, value_type);

        if self.last_in_fn {
            self.check_return_type_mismatch(ast, expr);
        }
    }

    fn type_check_unary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (op, operand): (UnaryOp, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.type_check_expr(ast, names, operand);

        self.last_in_fn = last_in_fn;

        match op {
            UnaryOp::Negate => type_check_unary_op!(
                self, ast, expr, (operand) ;
                rhs_failure: self.expr_type(ast, operand) ;
                Type::Primitive(Primitive::I64) => self.type_integer(),
                Type::Primitive(Primitive::F64) => self.type_float(),
            ),
            UnaryOp::Not => type_check_unary_op!(
                self, ast, expr, (operand) ;
                rhs_failure: self.expr_type(ast, operand) ;
                Type::Primitive(Primitive::Boolean) => self.type_boolean(),
            ),
        }

        if self.last_in_fn {
            self.check_return_type_mismatch(ast, expr);
        }
    }
}

impl TypeChecker {
    fn type_check_binary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (op, lhs, rhs): (BinaryOp, ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.type_check_expr(ast, names, lhs);
        self.type_check_expr(ast, names, rhs);

        self.last_in_fn = last_in_fn;

        match op {
            BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::Add
            | BinaryOp::Subtract => type_check_binary_op!(
                self, ast, expr, (lhs, rhs) ;
                lhs_failure: Error::ArithmeticImpossible ;
                (Type::Primitive(Primitive::I64), Type::Primitive(Primitive::I64)) => {
                    rhs_failure: Type::Primitive(Primitive::I64),
                    success: self.type_integer(),
                }
                (Type::Primitive(Primitive::F64), Type::Primitive(Primitive::F64)) => {
                    rhs_failure: Type::Primitive(Primitive::F64),
                    success: self.type_float(),
                }
            ),
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessOrEqual
            | BinaryOp::GreaterOrEqual => type_check_binary_op!(
                self, ast, expr, (lhs, rhs) ;
                lhs_failure: Error::CannotCompare ;
                (Type::Primitive(Primitive::I64), Type::Primitive(Primitive::I64)) => {
                    rhs_failure: Type::Primitive(Primitive::I64),
                    success: self.type_boolean(),
                }
                (Type::Primitive(Primitive::F64), Type::Primitive(Primitive::F64)) => {
                    rhs_failure: Type::Primitive(Primitive::F64),
                    success: self.type_boolean(),
                }
            ),
            BinaryOp::NotEqual | BinaryOp::Equal => type_check_binary_op!(
                self, ast, expr, (lhs, rhs) ;
                lhs_failure: Error::UnknownType ;
                (Type::Primitive(Primitive::I64), Type::Primitive(Primitive::I64)) => {
                    rhs_failure: Type::Primitive(Primitive::I64),
                    success: self.type_boolean(),
                }
                (Type::Primitive(Primitive::F64), Type::Primitive(Primitive::F64)) => {
                    rhs_failure: Type::Primitive(Primitive::F64),
                    success: self.type_boolean(),
                }
                (Type::Primitive(Primitive::Boolean), Type::Primitive(Primitive::Boolean)) => {
                    rhs_failure: Type::Primitive(Primitive::Boolean),
                    success: self.type_boolean(),
                }
                (Type::Primitive(Primitive::Unit), Type::Primitive(Primitive::Unit)) => {
                    rhs_failure: Type::Primitive(Primitive::Unit),
                    success: self.type_boolean(),
                }
            ),
            BinaryOp::And | BinaryOp::Or => type_check_binary_op!(
                self, ast, expr, (lhs, rhs) ;
                lhs_failure: Error::TypeMismatch {
                    expected: Type::Primitive(Primitive::Boolean).to_string(self.types.as_slice()),
                    got: self.expr_type(ast, lhs).clone().to_string(self.types.as_slice()),
                } ;
                (Type::Primitive(Primitive::Boolean), Type::Primitive(Primitive::Boolean)) => {
                    rhs_failure: Type::Primitive(Primitive::Boolean),
                    success: self.type_boolean(),
                }
            ),
            BinaryOp::Assign => type_check_binary_op!(
                self, ast, expr, (lhs, rhs) ;
                lhs_failure: Error::UnknownType ;
                (Type::Primitive(Primitive::I64), Type::Primitive(Primitive::I64)) => {
                    rhs_failure: Type::Primitive(Primitive::I64),
                    success: self.type_integer(),
                }
                (Type::Primitive(Primitive::F64), Type::Primitive(Primitive::F64)) => {
                    rhs_failure: Type::Primitive(Primitive::F64),
                    success: self.type_float(),
                }
                (Type::Primitive(Primitive::Boolean), Type::Primitive(Primitive::Boolean)) => {
                    rhs_failure: Type::Primitive(Primitive::Boolean),
                    success: self.type_boolean(),
                }
                (Type::Primitive(Primitive::Unit), Type::Primitive(Primitive::Unit)) => {
                    rhs_failure: Type::Primitive(Primitive::Unit),
                    success: self.type_unit(),
                }
            ),
        }

        if self.last_in_fn {
            self.check_return_type_mismatch(ast, expr);
        }
    }
}

impl TypeChecker {
    fn type_check_if(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (condition, when_true, otherwise): (ExprIndex, ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.type_check_expr(ast, names, condition);

        self.last_in_fn = last_in_fn;

        self.type_check_expr(ast, names, when_true);
        self.type_check_expr(ast, names, otherwise);

        if !matches!(
            self.expr_type(ast, condition),
            Type::Primitive(Primitive::Boolean) | Type::Unknown
        ) {
            self.errors.push(Spanned::new(
                Error::ConditionNotBoolean,
                ast.get_expr(condition)
                    .map(Spanned::span)
                    .expect("parsing ensures if expressions have conditions"),
            ));
        }

        type_check_binary_op!(
            self, ast, expr, (when_true, otherwise) ;
            lhs_failure: Error::UnknownType ;
            (_, Type::Primitive(Primitive::Unit)) => {
                rhs_failure: Type::Primitive(Primitive::Unit),
                success: self.type_unit(),
            }
            (Type::Primitive(Primitive::I64), Type::Primitive(Primitive::I64)) => {
                rhs_failure: Type::Primitive(Primitive::I64),
                success: self.type_integer(),
            }
            (Type::Primitive(Primitive::F64), Type::Primitive(Primitive::F64)) => {
                rhs_failure: Type::Primitive(Primitive::F64),
                success: self.type_float(),
            }
            (Type::Primitive(Primitive::Boolean), Type::Primitive(Primitive::Boolean)) => {
                rhs_failure: Type::Primitive(Primitive::Boolean),
                success: self.type_boolean(),
            }
        );
    }

    fn type_check_call(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (callee, arguments): (ExprIndex, &[ExprIndex]),
    ) {
        let span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, so does the span");

        let callee_span = ast
            .get_expr(callee)
            .map(Spanned::span)
            .expect("parsing ensures all calls have a callee");

        if let Some(callee) = names.get(&callee_span)
            && let Some(Type::Fn {
                parameters,
                return_type,
            }) = self
                .type_map
                .get(callee)
                .and_then(|type_index| self.types.get(usize::from(*type_index)))
        {
            if arguments.len() != parameters.len() {
                self.errors.push(Spanned::new(
                    Error::CallArgumentCountMismatch {
                        expected: parameters.len(),
                        got: arguments.len(),
                    },
                    ast.get_expr(expr)
                        .map(Spanned::span)
                        .expect("if the expression exists, so does the span"),
                ));
            }

            let errors = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (*parameter, *argument))
                .filter_map(|(parameter, argument)| {
                    if let Some(parameter_type) = self.types.get(usize::from(parameter))
                        && let Some(argument_span) = ast.get_expr(argument).map(Spanned::span)
                        && let Some(argument_type) = self
                            .type_map
                            .get(&argument_span)
                            .and_then(|type_index| self.types.get(usize::from(*type_index)))
                        && parameter_type != argument_type
                    {
                        Some(Spanned::new(
                            Error::TypeMismatch {
                                expected: parameter_type.to_string(self.types.as_slice()),
                                got: argument_type.to_string(self.types.as_slice()),
                            },
                            ast.get_expr(argument)
                                .map(Spanned::span)
                                .expect("the argument exists because we're iterating it right now"),
                        ))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            for error in errors {
                self.errors.push(error);
            }

            self.type_map.insert(span, *return_type);
        } else if !matches!(self.expr_type(ast, callee), Type::Unknown) {
            self.errors.push(Spanned::new(
                Error::CalledUncallable,
                ast.get_expr(callee)
                    .map(Spanned::span)
                    .expect("parsing ensures all calls have a callee"),
            ));

            self.type_map.insert(span, self.type_unknown());
        }
    }

    fn type_check_while(&mut self, ast: &Ast, expr: ExprIndex, condition: ExprIndex) {
        let span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        if !matches!(
            self.expr_type(ast, condition),
            Type::Primitive(Primitive::Boolean) | Type::Unknown
        ) {
            self.errors.push(Spanned::new(
                Error::ConditionNotBoolean,
                ast.get_expr(condition)
                    .map(Spanned::span)
                    .expect("parsing ensures all while loops have a condition"),
            ));
        }

        self.type_map.insert(span, self.type_unit());
    }

    fn type_check_return(&mut self, ast: &Ast, expr: ExprIndex, value: ExprIndex) {
        let value_span = ast
            .get_expr(value)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        let value_type = self.get_type_index_or_error(value_span);

        self.type_map.insert(
            ast.get_expr(expr)
                .map(Spanned::span)
                .expect("parsing ensures all returns have a value"),
            value_type,
        );

        self.check_return_type_mismatch(ast, value);
    }
}
