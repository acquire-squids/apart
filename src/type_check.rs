use crate::{
    Reportable, Span, Spanned,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, TypeSignature, UnaryOp},
};

use std::{collections::HashMap, error, fmt};

pub fn check_types(
    ast: &Ast,
    names: &HashMap<Span, Span>,
) -> Result<TypeChecker, Vec<Spanned<Error>>> {
    let mut type_checker = TypeChecker::new();

    type_checker.check_primitives(ast);

    type_checker.check_types(ast, names);

    for root in ast.roots() {
        type_checker.check_item(ast, names, *root);
    }

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

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(Primitive),
    Unknown,
    Fn {
        parameters: Vec<TypeIndex>,
        return_type: TypeIndex,
    },
    Generic(String),
    Product {
        name: String,
        fields: Vec<(String, TypeIndex)>,
        generics: Vec<TypeIndex>,
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
    fn eq(&self, types: &[Self], other: &Self) -> bool {
        match (self, other) {
            (Self::Primitive(a), Self::Primitive(b)) => a == b,
            (Self::Unknown, Self::Unknown) => true,
            (Self::Generic(a), Self::Generic(b)) => a == b,
            (
                Self::Fn {
                    parameters: a_parameters,
                    return_type: a_return_type,
                },
                Self::Fn {
                    parameters: b_parameters,
                    return_type: b_return_type,
                },
            ) => {
                types
                    .get(usize::from(*a_return_type))
                    .and_then(|a_return_type| {
                        Some((a_return_type, types.get(usize::from(*b_return_type))?))
                    })
                    .is_some_and(|(a_return_type, b_return_type)| {
                        a_return_type.eq(types, b_return_type)
                    })
                    && a_parameters.iter().zip(b_parameters).all(
                        |(a_parameter_type, b_parameter_type)| {
                            types
                                .get(usize::from(*a_parameter_type))
                                .and_then(|a_parameter_type| {
                                    Some((
                                        a_parameter_type,
                                        types.get(usize::from(*b_parameter_type))?,
                                    ))
                                })
                                .is_some_and(|(a_parameter_type, b_parameter_type)| {
                                    a_parameter_type.eq(types, b_parameter_type)
                                })
                        },
                    )
            }
            (
                Self::Product {
                    name: a_name,
                    fields: a_fields,
                    generics: a_generics,
                },
                Self::Product {
                    name: b_name,
                    fields: b_fields,
                    generics: b_generics,
                },
            ) => {
                a_name == b_name
                    && a_fields.iter().zip(b_fields).all(
                        |((a_field_name, a_field_type), (b_field_name, b_field_type))| {
                            a_field_name == b_field_name
                                && types
                                    .get(usize::from(*a_field_type))
                                    .and_then(|a_field_type| {
                                        Some((a_field_type, types.get(usize::from(*b_field_type))?))
                                    })
                                    .is_some_and(|(a_field_type, b_field_type)| {
                                        a_field_type.eq(types, b_field_type)
                                    })
                        },
                    )
                    && a_generics
                        .iter()
                        .zip(b_generics)
                        .all(|(a_generic_type, b_generic_type)| {
                            types
                                .get(usize::from(*a_generic_type))
                                .and_then(|a_generic_type| {
                                    Some((a_generic_type, types.get(usize::from(*b_generic_type))?))
                                })
                                .is_some_and(|(a_generic_type, b_generic_type)| {
                                    a_generic_type.eq(types, b_generic_type)
                                })
                        })
            }
            (_, _) => false,
        }
    }

    fn ne(&self, types: &[Self], other: &Self) -> bool {
        !self.eq(types, other)
    }

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
            Self::Generic(name) => name.clone(),
            Self::Product { name, generics, .. } => {
                let mut buffer = name.clone();

                if !generics.is_empty() {
                    buffer.push('[');

                    if let Some(first_generic) = generics
                        .first()
                        .and_then(|type_index| types.get(usize::from(*type_index)))
                    {
                        buffer.push_str(first_generic.to_string(types).as_str());
                    }

                    for generic in generics
                        .iter()
                        .skip(1)
                        .filter_map(|generic| types.get(usize::from(*generic)))
                    {
                        buffer.push_str(", ");

                        buffer.push_str(generic.to_string(types).as_str());
                    }

                    buffer.push(']');
                }

                buffer
            }
            Self::Fn {
                parameters,
                return_type,
            } => {
                let mut buffer = "funky(".to_string();

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
                    buffer.push_str(", ");

                    buffer.push_str(parameter.to_string(types).as_str());
                }

                buffer.push(')');

                if let Some(return_type) = types.get(usize::from(*return_type))
                    && return_type.ne(types, &Self::Primitive(Primitive::Unit))
                {
                    buffer.push_str(" -> ");

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
    ProductMissingField(String),
    DuplicateField,
    UnknownProduct,
    CannotAccess,
    InvalidAccess,
    NonExistentField,
    ExpectedZeroGenerics,
    GenericCountMismatch { expected: usize, got: usize },
    GenericsOnPrimitive,
    GenericsOnGeneric,
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
            Self::ProductMissingField(name) => {
                write!(f, "this product is missing its required field \"{name}\"")
            }
            Self::DuplicateField => write!(f, "fields cannot exist twice on a product"),
            Self::UnknownProduct => {
                write!(f, "a product of this name does not exist in this scope")
            }
            Self::CannotAccess => write!(f, "this expression has nothing to access"),
            Self::InvalidAccess => write!(f, "only names can be used to access"),
            Self::NonExistentField => write!(f, "this field does not exist on the accessed type"),
            Self::ExpectedZeroGenerics => write!(f, "this type doesn't require any generics"),
            Self::GenericCountMismatch { expected, got } => {
                write!(f, "expected {expected} generics, but got {got} generics")
            }
            Self::GenericsOnPrimitive => write!(f, "primitive types cannot have generics"),
            Self::GenericsOnGeneric => write!(f, "generics cannot have generics"),
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

    #[allow(dead_code)]
    pub const fn types(&self) -> &[Type] {
        self.types.as_slice()
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

    fn check_types(&mut self, ast: &Ast, names: &HashMap<Span, Span>) {
        for root in ast.roots() {
            match ast.get_item(*root).map(Spanned::kind) {
                Some(Item::Product {
                    name,
                    fields,
                    generics,
                }) => {
                    let mut generic_type_indices = vec![];

                    for generic in generics {
                        let type_index = self.push_type(Type::Generic(generic.kind().clone()));

                        self.type_map.insert(generic.span(), type_index);

                        generic_type_indices.push(type_index);
                    }

                    let fields = fields
                        .iter()
                        .map(|field| {
                            self.check_type_signature(names, field.ty());

                            let type_index = self
                                .get_type_index(field.ty().span())
                                .expect("the type should have been set in the previous call");

                            self.type_map.insert(field.name().span(), type_index);

                            (
                                field.name().kind().clone(),
                                self.get_type_index_or_error(field.name().span()),
                            )
                        })
                        .collect::<Vec<_>>();

                    let type_index = self.push_type(Type::Product {
                        name: name.kind().clone(),
                        fields,
                        generics: generic_type_indices,
                    });

                    self.type_map.insert(name.span(), type_index);
                }
                None | Some(Item::Primitive(_) | Item::NativeFn { .. } | Item::Fn { .. }) => {}
            }
        }
    }

    fn check_item(&mut self, ast: &Ast, names: &HashMap<Span, Span>, item: ItemIndex) {
        match ast.get_item(item).map(Spanned::kind) {
            None | Some(Item::Primitive(_) | Item::Product { .. }) => {}
            Some(Item::NativeFn { .. }) => {
                self.check_native_function(ast, names, item);
            }
            Some(Item::Fn { .. }) => {
                self.check_function(ast, names, item);
            }
        }
    }

    fn check_native_function(&mut self, ast: &Ast, names: &HashMap<Span, Span>, item: ItemIndex) {
        if let Some(Item::NativeFn { name, signature }) = ast.get_item(item).map(Spanned::kind)
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

    fn check_type_signature(&mut self, names: &HashMap<Span, Span>, ty: &Spanned<TypeSignature>) {
        match ty.kind() {
            TypeSignature::Normal { name, generics } => {
                let mut generic_types = vec![];

                for generic in generics {
                    self.check_type_signature(names, generic);

                    generic_types.push(
                        self.get_type_index(generic.span())
                            .expect("the generic was already checked"),
                    );
                }

                let name_span = name.span();

                if let Some(name) = names.get(&name_span)
                    && let Some(type_index) = self.get_type_index(*name)
                {
                    if generic_types.is_empty() {
                        self.type_map.insert(ty.span(), type_index);

                        return;
                    }

                    match self.get_type(type_index) {
                        Some(Type::Product {
                            name,
                            fields,
                            generics: original_generics,
                        }) => {
                            if original_generics.is_empty() && !generic_types.is_empty() {
                                self.errors
                                    .push(Spanned::new(Error::ExpectedZeroGenerics, name_span));
                            } else if generic_types.len() != original_generics.len() {
                                self.errors.push(Spanned::new(
                                    Error::GenericCountMismatch {
                                        expected: original_generics.len(),
                                        got: generic_types.len(),
                                    },
                                    ty.span(),
                                ));
                            } else {
                                let mut new_fields = fields.clone();

                                for (generic_index, original_generic) in
                                    original_generics.iter().enumerate()
                                {
                                    for (field_index, _) in fields
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, (_, field_ty))| field_ty == original_generic)
                                    {
                                        new_fields[field_index].1 = generic_types[generic_index];
                                    }
                                }

                                let type_index = self.push_type(Type::Product {
                                    name: name.clone(),
                                    fields: new_fields,
                                    generics: generic_types,
                                });

                                self.type_map.insert(ty.span(), type_index);

                                return;
                            }
                        }
                        Some(Type::Primitive(_)) => {
                            self.errors
                                .push(Spanned::new(Error::GenericsOnPrimitive, name_span));
                        }
                        Some(Type::Generic(_)) => {
                            self.errors
                                .push(Spanned::new(Error::GenericsOnGeneric, name_span));
                        }
                        None | Some(Type::Unknown | Type::Fn { .. }) => {
                            unreachable!("all names resolve, and a name cannot be a function type")
                        }
                    }
                }

                self.type_map.insert(ty.span(), self.type_unknown());
            }
            TypeSignature::Fn {
                parameters,
                return_type,
            } => {
                let mut parameter_types = vec![];

                for parameter in parameters {
                    self.check_type_signature(names, parameter);

                    parameter_types.push(
                        self.get_type_index(parameter.span())
                            .expect("the parameter was already checked"),
                    );
                }

                self.check_type_signature(names, return_type);

                let function_ty = Type::Fn {
                    parameters: parameter_types,
                    return_type: self
                        .get_type_index(return_type.span())
                        .expect("the return type was already checked"),
                };

                let type_index = self.push_type(function_ty);

                self.type_map.insert(ty.span(), type_index);
            }
        }
    }

    fn check_function(&mut self, ast: &Ast, names: &HashMap<Span, Span>, item: ItemIndex) {
        if let Some(Item::Fn {
            name,
            parameters,
            return_type,
            generics,
            ..
        }) = ast.get_item(item).map(Spanned::kind)
        {
            for generic in generics {
                let type_index = self.push_type(Type::Generic(generic.kind().clone()));

                self.type_map.insert(generic.span(), type_index);
            }

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
        #[allow(unreachable_patterns, unused_variables)]
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
            && expr_type.ne(self.types.as_slice(), fn_return_type)
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
            Some(Expr::Binary {
                op: BinaryOp::Access,
                lhs,
                rhs,
            }) => {
                self.type_check_access(ast, names, expr, (*lhs, *rhs));
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
                self.type_check_call(ast, names, expr, (*callee, arguments.as_slice()));
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
            Some(Expr::Product { name, fields }) => {
                self.type_check_product(ast, names, expr, (name, fields.as_slice()));
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
        #[allow(unreachable_patterns, unused_variables)]
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
                && value_type.ne(self.types.as_slice(), expected_type)
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

        let expr_span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        self.type_map.insert(expr_span, value_type);

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
    fn check_type_equality(
        &self,
        (lhs_type, rhs_type): (&Type, &Type),
        rhs_span: Span,
    ) -> Result<TypeIndex, Spanned<Error>> {
        if lhs_type.eq(self.types.as_slice(), rhs_type) {
            Ok(self
                .get_type_index(rhs_span)
                .expect("rhs has a type since we succeeded the equality check"))
        } else {
            Err(Spanned::new(
                Error::TypeMismatch {
                    expected: lhs_type.to_string(self.types.as_slice()),
                    got: rhs_type.to_string(self.types.as_slice()),
                },
                rhs_span,
            ))
        }
    }

    #[allow(clippy::too_many_lines)]
    fn type_check_binary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (op, lhs, rhs): (BinaryOp, ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("the ast should be valid since we succeeded in parsing");

        self.type_check_expr(ast, names, lhs);
        self.type_check_expr(ast, names, rhs);

        self.last_in_fn = last_in_fn;

        let lhs_span = ast
            .get_expr(lhs)
            .map(Spanned::span)
            .expect("the ast should be valid since we succeeded in parsing");

        let rhs_span = ast
            .get_expr(rhs)
            .map(Spanned::span)
            .expect("the ast should be valid since we succeeded in parsing");

        let lhs_type_index = self
            .get_type_index(lhs_span)
            .expect("all expressions will have a type, even if unknown, after being checked");

        let rhs_type_index = self
            .get_type_index(rhs_span)
            .expect("all expressions will have a type, even if unknown, after being checked");

        match op {
            BinaryOp::Access => {
                unreachable!("accesses shouldn't be checked in `type_check_binary`")
            }
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
            BinaryOp::NotEqual | BinaryOp::Equal => {
                match self.check_type_equality(
                    (
                        self.get_type(lhs_type_index)
                            .expect("lhs was checked before the match arm"),
                        self.get_type(rhs_type_index)
                            .expect("rhs was checked before the match arm"),
                    ),
                    rhs_span,
                ) {
                    Ok(_) => {
                        self.type_map.insert(span, self.type_boolean());
                    }
                    Err(error) => {
                        self.errors.push(error);

                        self.type_map.insert(span, self.type_unknown());
                    }
                }
            }
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
            BinaryOp::Assign => {
                match self.check_type_equality(
                    (
                        self.get_type(lhs_type_index)
                            .expect("lhs was checked before the match arm"),
                        self.get_type(rhs_type_index)
                            .expect("rhs was checked before the match arm"),
                    ),
                    rhs_span,
                ) {
                    Ok(type_index) => {
                        self.type_map.insert(span, type_index);
                    }
                    Err(error) => {
                        self.errors.push(error);

                        self.type_map.insert(span, self.type_unknown());
                    }
                }
            }
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

    #[allow(clippy::too_many_lines)]
    fn substitute_type(
        &mut self,
        originals: &[TypeIndex],
        replacements: &[TypeIndex],
        to_substitute: TypeIndex,
    ) -> Option<TypeIndex> {
        match self.get_type(to_substitute) {
            None => None,
            Some(Type::Primitive(_) | Type::Unknown) => Some(to_substitute),
            Some(Type::Generic(name)) => {
                let name = name.clone();

                for (i, original) in originals.iter().enumerate() {
                    match self.get_type(*original) {
                        Some(Type::Generic(other_name)) if other_name == name.as_str() => {
                            return Some(replacements[i]);
                        }
                        Some(Type::Generic(_) | Type::Primitive(_) | Type::Unknown) | None => {}
                        Some(Type::Fn {
                            parameters: original_parameters,
                            return_type: original_return_type,
                        }) => {
                            if let Some(Type::Fn {
                                parameters,
                                return_type,
                            }) = self.get_type(replacements[i])
                            {
                                let (original_parameters, parameters) =
                                    (original_parameters.clone(), parameters.clone());

                                let (original_return_type, return_type) =
                                    (*original_return_type, *return_type);

                                if let Some(type_index) = self.substitute_type(
                                    original_parameters.as_slice(),
                                    parameters.as_slice(),
                                    to_substitute,
                                ) {
                                    return Some(type_index);
                                } else if let Some(type_index) = self.substitute_type(
                                    [original_return_type].as_slice(),
                                    [return_type].as_slice(),
                                    to_substitute,
                                ) {
                                    return Some(type_index);
                                }
                            }
                        }
                        Some(Type::Product {
                            generics: original_generics,
                            ..
                        }) => {
                            if let Some(Type::Product {
                                generics: replacement_generics,
                                ..
                            }) = self.get_type(replacements[i])
                            {
                                let (original_generics, replacement_generics) =
                                    (original_generics.clone(), replacement_generics.clone());

                                if let Some(type_index) = self.substitute_type(
                                    original_generics.as_slice(),
                                    replacement_generics.as_slice(),
                                    to_substitute,
                                ) {
                                    return Some(type_index);
                                }
                            }
                        }
                    }
                }

                None
            }
            Some(Type::Fn {
                parameters,
                return_type,
            }) => {
                let parameters = parameters.clone();
                let return_type = *return_type;

                let mut substituted_parameters = vec![];

                for parameter in parameters {
                    substituted_parameters.push(self.substitute_type(
                        originals,
                        replacements,
                        parameter,
                    )?);
                }

                let parameters = substituted_parameters;

                let return_type = self.substitute_type(originals, replacements, return_type)?;

                Some(self.push_type(Type::Fn {
                    parameters,
                    return_type,
                }))
            }
            Some(Type::Product {
                name,
                fields,
                generics,
            }) => {
                let name = name.clone();
                let fields = fields.clone();
                let generics = generics.clone();

                let mut substituted_fields = vec![];

                for (field_name, field) in fields {
                    substituted_fields.push((
                        field_name,
                        self.substitute_type(originals, replacements, field)?,
                    ));
                }

                let fields = substituted_fields;

                let mut substituted_generics = vec![];

                for generic in generics {
                    substituted_generics.push(self.substitute_type(
                        originals,
                        replacements,
                        generic,
                    )?);
                }

                let generics = substituted_generics;

                Some(self.push_type(Type::Product {
                    name,
                    fields,
                    generics,
                }))
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn type_check_call(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (callee, arguments): (ExprIndex, &[ExprIndex]),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        ast.for_children_exprs(expr, |ast, expr| self.type_check_expr(ast, names, expr));

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

            let parameters = parameters.clone();
            let return_type = *return_type;

            let arguments = arguments
                .iter()
                .filter_map(|argument| {
                    ast.get_expr(*argument)
                        .map(Spanned::span)
                        .and_then(|argument_span| {
                            self.get_type_index(argument_span)
                                .map(|type_index| (argument_span, type_index))
                        })
                })
                .collect::<Vec<_>>();

            let known_argument_types = arguments
                .iter()
                .map(|(_, parameter_type_index)| *parameter_type_index)
                .collect::<Vec<_>>();

            let mut checked_arguments = vec![];

            for (original_parameter_type_index, (argument_type_span, argument_type_index)) in
                parameters.iter().zip(arguments)
            {
                if let Some(type_index) = self.substitute_type(
                    parameters.as_slice(),
                    known_argument_types.as_slice(),
                    *original_parameter_type_index,
                ) {
                    checked_arguments.push(type_index);
                } else {
                    let expected = self
                        .get_type(*original_parameter_type_index)
                        .expect("all expressions have a span, even if unknown")
                        .to_string(self.types.as_slice());

                    let got = self
                        .get_type(argument_type_index)
                        .expect("all expressions have a span, even if unknown")
                        .to_string(self.types.as_slice());

                    self.errors.push(Spanned::new(
                        Error::TypeMismatch { expected, got },
                        argument_type_span,
                    ));
                }
            }

            if checked_arguments.len() == parameters.len() {
                if let Some(return_type) = self.substitute_type(
                    parameters.as_slice(),
                    checked_arguments.as_slice(),
                    return_type,
                ) {
                    self.type_map.insert(span, return_type);
                } else {
                    self.type_map.insert(span, return_type);
                }
            } else {
                self.type_map.insert(span, self.type_unknown());
            }
        } else if !matches!(self.expr_type(ast, callee), Type::Unknown) {
            self.errors.push(Spanned::new(
                Error::CalledUncallable,
                ast.get_expr(callee)
                    .map(Spanned::span)
                    .expect("parsing ensures all calls have a callee"),
            ));

            self.type_map.insert(span, self.type_unknown());
        }

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.check_return_type_mismatch(ast, expr);
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

    #[allow(clippy::too_many_lines)]
    fn type_check_product(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (name, fields): (&Spanned<String>, &[(Spanned<String>, ExprIndex)]),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        let span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        for (_, value) in fields {
            self.type_check_expr(ast, names, *value);
        }

        let mut field_types = vec![];

        for (field, value) in fields {
            field_types.push((
                field,
                ast.get_expr(*value)
                    .map(Spanned::span)
                    .and_then(|span| {
                        self.get_type_index(span)
                            .map(|type_index| (span, type_index))
                    })
                    .expect("we found the types in the loop above"),
            ));
        }

        if let Some(Type::Product {
            fields, generics, ..
        }) = names
            .get(&name.span())
            .and_then(|span| self.get_type_index(*span))
            .and_then(|type_index| self.get_type(type_index))
        {
            let generics = generics.clone();
            let mut errors = vec![];

            for (field_name, _) in fields.iter().filter_map(|(field_name, field_ty)| {
                self.get_type(*field_ty).map(|ty| (field_name, ty))
            }) {
                if !field_types
                    .iter()
                    .any(|(name, (_, _))| field_name == name.kind())
                {
                    errors.push(Spanned::new(
                        Error::ProductMissingField(field_name.clone()),
                        span,
                    ));
                } else if let Some((name, (_, _))) = field_types
                    .iter()
                    .filter(|(name, (_, _))| name.kind() == field_name)
                    .nth(1)
                {
                    errors.push(Spanned::new(Error::DuplicateField, name.span()));
                }
            }

            if errors.is_empty() {
                let fields = fields
                    .iter()
                    .map(|(_, field_type_index)| *field_type_index)
                    .collect::<Vec<_>>();

                let known_field_types = field_types
                    .iter()
                    .map(|(_, (_, field_type_index))| *field_type_index)
                    .collect::<Vec<_>>();

                let mut checked_fields = vec![];

                for (
                    original_field_type_index,
                    (field_name, (field_type_span, field_type_index)),
                ) in fields.iter().zip(field_types)
                {
                    if let Some(type_index) = self.substitute_type(
                        fields.as_slice(),
                        known_field_types.as_slice(),
                        *original_field_type_index,
                    ) {
                        checked_fields.push((field_name.kind().clone(), type_index));
                    } else {
                        let expected = self
                            .get_type(*original_field_type_index)
                            .expect("all expressions have a span, even if unknown")
                            .to_string(self.types.as_slice());

                        let got = self
                            .get_type(field_type_index)
                            .expect("all expressions have a span, even if unknown")
                            .to_string(self.types.as_slice());

                        self.errors.push(Spanned::new(
                            Error::TypeMismatch { expected, got },
                            field_type_span,
                        ));
                    }
                }

                if checked_fields.len() == fields.len() {
                    let substituted_generics = generics
                        .iter()
                        .filter_map(|generic| {
                            self.substitute_type(
                                fields.as_slice(),
                                known_field_types.as_slice(),
                                *generic,
                            )
                        })
                        .collect::<Vec<_>>();

                    if substituted_generics.len() == generics.len() {
                        let type_index = self.push_type(Type::Product {
                            name: name.kind().clone(),
                            fields: checked_fields,
                            generics: substituted_generics,
                        });

                        self.type_map.insert(span, type_index);
                    } else {
                        self.type_map.insert(span, self.type_unknown());
                    }
                } else {
                    self.type_map.insert(span, self.type_unknown());
                }
            } else {
                self.errors.append(&mut errors);

                self.type_map.insert(span, self.type_unknown());
            }
        } else {
            self.errors
                .push(Spanned::new(Error::UnknownProduct, name.span()));

            self.type_map.insert(span, self.type_unknown());
        }

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.check_return_type_mismatch(ast, expr);
        }
    }

    fn type_check_access(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        (lhs, rhs): (ExprIndex, ExprIndex),
    ) {
        let last_in_fn = self.last_in_fn;

        self.last_in_fn = false;

        self.type_check_expr(ast, names, lhs);

        let expr_span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, the span does too");

        if let Some(Type::Product { fields, .. }) = ast
            .get_expr(lhs)
            .map(Spanned::span)
            .and_then(|span| self.get_type_index(span))
            .and_then(|type_index| self.get_type(type_index))
        {
            if let Some((name_span, Expr::Name(name))) =
                ast.get_expr(rhs).map(|expr| (expr.span(), expr.kind()))
            {
                if let Some((_, ty)) = fields.iter().find(|(field_name, _)| field_name == name) {
                    self.type_map.insert(expr_span, *ty);
                } else {
                    self.errors
                        .push(Spanned::new(Error::NonExistentField, name_span));

                    self.type_map.insert(expr_span, self.type_unknown());
                }
            } else {
                self.errors.push(Spanned::new(
                    Error::InvalidAccess,
                    ast.get_expr(rhs)
                        .map(Spanned::span)
                        .expect("the ast should be valid since we succeeded in parsing"),
                ));

                self.type_map.insert(expr_span, self.type_unknown());
            }
        } else {
            self.errors.push(Spanned::new(
                Error::CannotAccess,
                ast.get_expr(lhs)
                    .map(Spanned::span)
                    .expect("the ast should be valid since we succeeded in parsing"),
            ));

            self.type_map.insert(expr_span, self.type_unknown());
        }

        self.last_in_fn = last_in_fn;

        if self.last_in_fn {
            self.check_return_type_mismatch(ast, expr);
        }
    }
}
