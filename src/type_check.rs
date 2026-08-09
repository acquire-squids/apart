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
    Existential(String),
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
            Self::Generic(name) | Self::Existential(name) => name.clone(),
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
    pub fn get_type_from_span(&self, span: Span) -> Option<&Type> {
        self.get_type_index(span)
            .and_then(|type_index| self.types.get(usize::from(type_index)))
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
                        let type_index = self.push_type(Type::Existential(generic.kind().clone()));

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

    fn substitute_type(
        &mut self,
        originals: &[TypeIndex],
        replacements: &[TypeIndex],
        type_index: TypeIndex,
    ) -> TypeIndex {
        match self.get_type(type_index) {
            None => self.type_unknown(),
            Some(Type::Primitive(_) | Type::Unknown | Type::Generic(_)) => type_index,
            Some(Type::Existential(name)) => {
                originals.iter().position(|type_index| {
                    matches!(self.get_type(*type_index), Some(Type::Existential(original)) if original == name)
                }).map_or(type_index, |index| replacements[index])
            }
            Some(Type::Fn {
                parameters,
                return_type,
            }) => {
                let return_type = *return_type;

                let parameters = parameters
                    .clone()
                    .into_iter()
                    .map(|type_index| self.substitute_type(originals, replacements, type_index))
                    .collect::<Vec<_>>();

                let return_type = self.substitute_type(originals, replacements, return_type);

                self.push_type(Type::Fn {
                    parameters,
                    return_type,
                })
            }
            Some(Type::Product {
                name,
                fields,
                generics,
            }) => {
                let name = name.clone();

                let generics = generics.clone();

                let fields = fields
                    .clone()
                    .into_iter()
                    .map(|(field_name, type_index)| {
                        (
                            field_name,
                            self.substitute_type(originals, replacements, type_index),
                        )
                    })
                    .collect::<Vec<_>>();

                let generics = generics
                    .into_iter()
                    .map(|type_index| self.substitute_type(originals, replacements, type_index))
                    .collect::<Vec<_>>();

                self.push_type(Type::Product {
                    name,
                    fields,
                    generics,
                })
            }
        }
    }

    fn check_type_signature(&mut self, names: &HashMap<Span, Span>, ty: &Spanned<TypeSignature>) {
        match ty.kind() {
            TypeSignature::Normal { name, generics } => {
                let checked_generics = generics.iter().fold(vec![], |mut accum, generic| {
                    self.check_type_signature(names, generic);

                    accum.push(
                        self.get_type_index(generic.span())
                            .expect("the type is set by the above call"),
                    );

                    accum
                });

                let type_index = names
                    .get(&name.span())
                    .and_then(|span| self.get_type_index(*span))
                    .expect("all type signatures exist");

                match self
                    .get_type(type_index)
                    .cloned()
                    .expect("all type signatures exist")
                {
                    Type::Primitive(_) if !checked_generics.is_empty() => {
                        self.errors
                            .push(Spanned::new(Error::GenericsOnPrimitive, name.span()));

                        self.type_map.insert(ty.span(), self.type_unknown());
                    }
                    Type::Generic(_) | Type::Existential(_) if !checked_generics.is_empty() => {
                        self.errors
                            .push(Spanned::new(Error::GenericsOnGeneric, name.span()));

                        self.type_map.insert(ty.span(), self.type_unknown());
                    }
                    Type::Unknown => {}
                    Type::Primitive(_) | Type::Generic(_) | Type::Existential(_) => {
                        self.type_map.insert(ty.span(), type_index);
                    }
                    Type::Fn { .. } => unreachable!("normal type signatures are never functions"),
                    Type::Product {
                        fields,
                        generics: generic_types,
                        ..
                    } => {
                        if generic_types.is_empty() && !checked_generics.is_empty() {
                            self.errors
                                .push(Spanned::new(Error::ExpectedZeroGenerics, ty.span()));

                            self.type_map.insert(ty.span(), self.type_unknown());
                        } else if checked_generics.len() != checked_generics.len() {
                            self.errors.push(Spanned::new(
                                Error::GenericCountMismatch {
                                    expected: generic_types.len(),
                                    got: checked_generics.len(),
                                },
                                ty.span(),
                            ));

                            self.type_map.insert(ty.span(), self.type_unknown());
                        } else {
                            let fields = fields.iter().fold(
                                vec![],
                                |mut accum, (field_name, field_type)| {
                                    let type_index = self.substitute_type(
                                        generic_types.as_slice(),
                                        checked_generics.as_slice(),
                                        *field_type,
                                    );

                                    accum.push((field_name.clone(), type_index));

                                    accum
                                },
                            );

                            let type_index = self.push_type(Type::Product {
                                name: name.kind().clone(),
                                fields,
                                generics: checked_generics,
                            });

                            self.type_map.insert(ty.span(), type_index);
                        }
                    }
                }
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
                let type_index = self.push_type(Type::Existential(generic.kind().clone()));

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

                if let Some(Type::Fn { return_type, .. }) = self.get_type_from_span(name.span())
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

impl TypeChecker {
    fn type_check_functions(&mut self, ast: &Ast, names: &HashMap<Span, Span>) {
        for root in ast.roots() {
            if let Some(Item::Fn {
                name,
                body,
                generics,
                ..
            }) = ast.get_item(*root).map(Spanned::kind)
            {
                let mut generics = generics
                    .iter()
                    .map(|generic| {
                        (
                            generic.kind().clone(),
                            self.get_type_index(generic.span())
                                .expect("all generics are a type"),
                        )
                    })
                    .collect::<Vec<_>>();

                let expected_return_type = self
                    .get_type_from_span(name.span())
                    .and_then(|ty| {
                        if let Type::Fn { return_type, .. } = ty {
                            Some(*return_type)
                        } else {
                            None
                        }
                    })
                    .map(|return_type| match self.apply(return_type, &mut generics) {
                        Ok(return_type) => return_type,
                        Err(error) => {
                            self.errors.push(Spanned::new(
                                error,
                                ast.get_expr(*body)
                                    .map(Spanned::span)
                                    .expect("every expression has a span"),
                            ));

                            self.type_unknown()
                        }
                    })
                    .expect("all functions have a return type");

                let fn_return_type = self.fn_return_type;

                self.fn_return_type = Some(expected_return_type);

                if let Err(error) =
                    self.check(ast, names, *body, expected_return_type, &mut generics)
                {
                    self.errors.push(Spanned::new(
                        error,
                        ast.get_expr(*body)
                            .map(Spanned::span)
                            .expect("every expression has a span"),
                    ));
                }

                self.fn_return_type = fn_return_type;
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infer(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        let span = ast
            .get_expr(expr)
            .map(Spanned::span)
            .expect("if the expression exists, so does the span");

        let type_index = match ast
            .get_expr(expr)
            .map(Spanned::kind)
            .expect("all expressions exists")
        {
            Expr::BinaryNoLhs { .. } | Expr::CallNoCallee(_) | Expr::AsUnitNoValue => {
                unreachable!("these won't exist since parsing succeeded");
            }
            Expr::Integer(_) => self.type_integer(),
            Expr::Float(_) => self.type_float(),
            Expr::Boolean(_) => self.type_boolean(),
            Expr::Unit => self.type_unit(),
            Expr::Name(_) => {
                let Some(type_index) = names.get(&span).and_then(|span| self.get_type_index(*span))
                else {
                    unreachable!("name resolution ensures all names are defined");
                };

                if let Some(Type::Existential(name)) = self.get_type(type_index) {
                    self.push_type(Type::Generic(name.clone()))
                } else {
                    type_index
                }
            }
            Expr::Unary { op, expr: operand } => {
                self.infer_unary(ast, names, (*op, *operand), context)
            }
            Expr::Binary { op, lhs, rhs } => {
                self.infer_binary(ast, names, (*op, *lhs, *rhs), context)
            }
            Expr::Group(expr) => self.infer(ast, names, *expr, context),
            Expr::Block(exprs) if exprs.is_empty() => self.type_unit(),
            Expr::Block(exprs) => {
                for expr in exprs.iter().take(exprs.len() - 1) {
                    self.infer(ast, names, *expr, context);
                }

                self.infer(
                    ast,
                    names,
                    exprs
                        .last()
                        .copied()
                        .expect("the block won't be empty because of the earlier match arm"),
                    context,
                )
            }
            Expr::If {
                condition,
                when_true,
                otherwise,
            } => {
                if self
                    .check(ast, names, *condition, self.type_boolean(), context)
                    .is_err()
                {
                    let condition_span = ast
                        .get_expr(*condition)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors
                        .push(Spanned::new(Error::ConditionNotBoolean, condition_span));
                }

                let when_true_type = self.infer(ast, names, *when_true, context);

                if let Err(error) = self.check(ast, names, *otherwise, when_true_type, context) {
                    let otherwise_span = ast
                        .get_expr(*otherwise)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors.push(Spanned::new(error, otherwise_span));

                    self.type_unknown()
                } else {
                    when_true_type
                }
            }
            Expr::While {
                condition,
                when_true,
            } => {
                if self
                    .check(ast, names, *condition, self.type_boolean(), context)
                    .is_err()
                {
                    let condition_span = ast
                        .get_expr(*condition)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors
                        .push(Spanned::new(Error::ConditionNotBoolean, condition_span));
                }

                self.infer(ast, names, *when_true, context);

                self.type_unit()
            }
            Expr::Let {
                name,
                type_signature,
                value,
            } => {
                let type_index = if let Some(annotation) = type_signature {
                    self.check_type_signature(names, annotation);

                    let annotation_ty = self.get_type_index(annotation.span()).expect(
                        "the annotation should be resolved by the `check_type_signature` call",
                    );

                    if let Err(error) = self.check(ast, names, *value, annotation_ty, context) {
                        let value_span = ast
                            .get_expr(*value)
                            .map(Spanned::span)
                            .expect("all expressions exist");

                        self.errors.push(Spanned::new(error, value_span));

                        self.type_unknown()
                    } else {
                        annotation_ty
                    }
                } else {
                    self.infer(ast, names, *value, context)
                };

                self.type_map.insert(name.span(), type_index);

                type_index
            }
            Expr::Call { callee, arguments } => {
                let callee_type_index = self.infer(ast, names, *callee, context);

                if let Type::Fn {
                    parameters,
                    return_type,
                } = self
                    .get_type(callee_type_index)
                    .cloned()
                    .expect("all expressions have types")
                {
                    if arguments.len() == parameters.len() {
                        let mut context = context.clone();

                        for (argument, parameter) in arguments.iter().zip(&parameters) {
                            if let Err(error) =
                                self.check(ast, names, *argument, *parameter, &mut context)
                            {
                                self.errors.push(Spanned::new(
                                    error,
                                    ast.get_expr(*argument)
                                        .map(Spanned::span)
                                        .expect("if the expression exists, the span does too"),
                                ));
                            }
                        }

                        match self.apply(return_type, &mut context) {
                            Ok(type_index) => type_index,
                            Err(error) => {
                                self.errors.push(Spanned::new(error, span));

                                self.type_unknown()
                            }
                        }
                    } else {
                        self.errors.push(Spanned::new(
                            Error::CallArgumentCountMismatch {
                                expected: parameters.len(),
                                got: arguments.len(),
                            },
                            span,
                        ));

                        self.type_unknown()
                    }
                } else {
                    let callee_span = ast
                        .get_expr(*callee)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors
                        .push(Spanned::new(Error::CalledUncallable, callee_span));

                    self.type_unknown()
                }
            }
            Expr::Return(expr) => {
                if let Err(error) = self.check(
                    ast,
                    names,
                    *expr,
                    self.fn_return_type
                        .expect("returns can only happen in functions"),
                    context,
                ) {
                    self.errors.push(Spanned::new(error, span));
                }

                self.type_unit()
            }
            Expr::AsUnit(expr) => {
                self.infer(ast, names, *expr, context);

                self.type_unit()
            }
            Expr::Product { name, fields } => {
                if let Some(Type::Product {
                    fields: field_types,
                    ..
                }) = names
                    .get(&name.span())
                    .and_then(|span| self.get_type_from_span(*span))
                    .cloned()
                {
                    let error_count = self.errors.len();

                    let mut checked_fields = vec![];

                    for (field_name, field) in fields {
                        if let Some(field_type) =
                            field_types
                                .iter()
                                .find_map(|(field_type_name, field_type)| {
                                    if field_type_name == field_name.kind() {
                                        Some(field_type)
                                    } else {
                                        None
                                    }
                                })
                        {
                            if field_types
                                .iter()
                                .filter(|(field_type_name, _)| field_type_name == field_name.kind())
                                .nth(1)
                                .is_none()
                            {
                                match self.check(ast, names, *field, *field_type, context) {
                                    Ok(ty) => {
                                        checked_fields.push((field_name.kind().clone(), ty));
                                    }
                                    Err(error) => {
                                        self.errors.push(Spanned::new(
                                            error,
                                            ast.get_expr(*field)
                                                .map(Spanned::span)
                                                .expect("all expressions exist"),
                                        ));
                                    }
                                }
                            } else {
                                self.errors
                                    .push(Spanned::new(Error::DuplicateField, field_name.span()));
                            }
                        } else {
                            self.errors
                                .push(Spanned::new(Error::NonExistentField, name.span()));
                        }
                    }

                    for (field_name, _) in field_types {
                        if !fields
                            .iter()
                            .any(|(name, _)| name.kind() == field_name.as_str())
                        {
                            self.errors.push(Spanned::new(
                                Error::ProductMissingField(field_name.clone()),
                                span,
                            ));
                        }
                    }

                    if self.errors.len() == error_count {
                        self.push_type(Type::Product {
                            name: name.kind().clone(),
                            fields: checked_fields,
                            generics: vec![],
                        })
                    } else {
                        self.type_unknown()
                    }
                } else {
                    self.errors
                        .push(Spanned::new(Error::UnknownProduct, name.span()));

                    self.type_unknown()
                }
            }
        };

        self.type_map.insert(span, type_index);

        type_index
    }

    fn infer_unary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        (op, operand): (UnaryOp, ExprIndex),
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match op {
            UnaryOp::Not => {
                self.infer(ast, names, operand, context);

                if let Err(error) = self.check(ast, names, operand, self.type_boolean(), context) {
                    let span = ast
                        .get_expr(operand)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors.push(Spanned::new(error, span));

                    self.type_unknown()
                } else {
                    self.type_boolean()
                }
            }
            UnaryOp::Negate => {
                self.infer(ast, names, operand, context);

                if let Err(_) = self.check(ast, names, operand, self.type_integer(), context)
                    && let Err(_) = self.check(ast, names, operand, self.type_float(), context)
                {
                    let span = ast
                        .get_expr(operand)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors
                        .push(Spanned::new(Error::ArithmeticImpossible, span));

                    self.type_unknown()
                } else {
                    self.type_boolean()
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infer_binary(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        (op, lhs, rhs): (BinaryOp, ExprIndex, ExprIndex),
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match op {
            BinaryOp::Access => {
                let lhs_type = self.infer(ast, names, lhs, context);

                let lhs_span = ast
                    .get_expr(lhs)
                    .map(Spanned::span)
                    .expect("all expressions exist");

                if let Type::Product { fields, .. } =
                    self.get_type(lhs_type).expect("all expressions have types")
                {
                    if let Expr::Name(name) = ast
                        .get_expr(rhs)
                        .map(Spanned::kind)
                        .expect("all expressions exist")
                    {
                        if let Some(field_type) =
                            fields.iter().find_map(|(field_name, field_type)| {
                                if field_name == name {
                                    Some(field_type)
                                } else {
                                    None
                                }
                            })
                        {
                            *field_type
                        } else {
                            self.errors.push(Spanned::new(
                                Error::NonExistentField,
                                ast.get_expr(rhs)
                                    .map(Spanned::span)
                                    .expect("all expressions exist"),
                            ));

                            self.type_unknown()
                        }
                    } else {
                        self.errors.push(Spanned::new(
                            Error::InvalidAccess,
                            ast.get_expr(rhs)
                                .map(Spanned::span)
                                .expect("all expressions exist"),
                        ));

                        self.type_unknown()
                    }
                } else {
                    self.errors
                        .push(Spanned::new(Error::CannotAccess, lhs_span));

                    self.type_unknown()
                }
            }
            BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::Add
            | BinaryOp::Subtract => {
                match self
                    .check(ast, names, lhs, self.type_integer(), context)
                    .or_else(|_| self.check(ast, names, lhs, self.type_float(), context))
                {
                    Err(_) => {
                        let span = ast
                            .get_expr(lhs)
                            .map(Spanned::span)
                            .expect("all expressions exist");

                        self.errors
                            .push(Spanned::new(Error::ArithmeticImpossible, span));

                        self.type_unknown()
                    }
                    Ok(lhs_type) => {
                        if let Err(error) = self.check(ast, names, rhs, lhs_type, context) {
                            let span = ast
                                .get_expr(rhs)
                                .map(Spanned::span)
                                .expect("all expressions exist");

                            self.errors.push(Spanned::new(error, span));

                            self.type_unknown()
                        } else {
                            lhs_type
                        }
                    }
                }
            }
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessOrEqual
            | BinaryOp::GreaterOrEqual => {
                match self
                    .check(ast, names, lhs, self.type_integer(), context)
                    .or_else(|_| self.check(ast, names, lhs, self.type_float(), context))
                {
                    Err(_) => {
                        let span = ast
                            .get_expr(lhs)
                            .map(Spanned::span)
                            .expect("all expressions exist");

                        self.errors.push(Spanned::new(Error::CannotCompare, span));

                        self.type_unknown()
                    }
                    Ok(lhs_type) => {
                        if let Err(error) = self.check(ast, names, rhs, lhs_type, context) {
                            let span = ast
                                .get_expr(rhs)
                                .map(Spanned::span)
                                .expect("all expressions exist");

                            self.errors.push(Spanned::new(error, span));

                            self.type_unknown()
                        } else {
                            self.type_boolean()
                        }
                    }
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                match self.check(ast, names, lhs, self.type_boolean(), context) {
                    Err(_) => {
                        let span = ast
                            .get_expr(lhs)
                            .map(Spanned::span)
                            .expect("all expressions exist");

                        self.errors
                            .push(Spanned::new(Error::ArithmeticImpossible, span));

                        self.type_unknown()
                    }
                    Ok(lhs_type) => {
                        if let Err(error) = self.check(ast, names, rhs, lhs_type, context) {
                            let span = ast
                                .get_expr(rhs)
                                .map(Spanned::span)
                                .expect("all expressions exist");

                            self.errors.push(Spanned::new(error, span));

                            self.type_unknown()
                        } else {
                            self.type_boolean()
                        }
                    }
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let lhs_type = self.infer(ast, names, lhs, context);

                if let Err(error) = self.check(ast, names, rhs, lhs_type, context) {
                    let span = ast
                        .get_expr(rhs)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors.push(Spanned::new(error, span));

                    self.type_unknown()
                } else {
                    self.type_boolean()
                }
            }
            BinaryOp::Assign => {
                let lhs_type = self.infer(ast, names, lhs, context);

                if let Err(error) = self.check(ast, names, rhs, lhs_type, context) {
                    let span = ast
                        .get_expr(rhs)
                        .map(Spanned::span)
                        .expect("all expressions exist");

                    self.errors.push(Spanned::new(error, span));

                    self.type_unknown()
                } else {
                    lhs_type
                }
            }
        }
    }

    fn check(
        &mut self,
        ast: &Ast,
        names: &HashMap<Span, Span>,
        expr: ExprIndex,
        should_be: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, Error> {
        let expr_type_index = self.infer(ast, names, expr, context);

        self.check_inferred(expr_type_index, should_be, context)
    }

    fn check_inferred(
        &mut self,
        inferred: TypeIndex,
        should_be: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, Error> {
        match (
            self.get_type(inferred).expect("all types exist"),
            self.get_type(should_be).expect("all types exist"),
        ) {
            (Type::Unknown, _) | (_, Type::Unknown) => Ok(self.type_unknown()),
            (Type::Primitive(a), Type::Primitive(b)) => {
                if a == b {
                    Ok(should_be)
                } else {
                    Err(Error::TypeMismatch {
                        expected: Type::Primitive(*a).to_string(self.types.as_slice()),
                        got: Type::Primitive(*a).to_string(self.types.as_slice()),
                    })
                }
            }
            (Type::Generic(a), Type::Generic(b)) => {
                if a == b {
                    Ok(should_be)
                } else {
                    Err(Error::TypeMismatch {
                        expected: Type::Generic(a.clone()).to_string(self.types.as_slice()),
                        got: Type::Generic(b.clone()).to_string(self.types.as_slice()),
                    })
                }
            }
            (_, Type::Existential(name)) => {
                context.push((name.clone(), inferred));

                Ok(inferred)
            }
            (
                Type::Fn {
                    parameters: inferred_parameters,
                    return_type: inferred_return_type,
                },
                Type::Fn {
                    parameters,
                    return_type,
                },
            ) => {
                let inferred_parameters = inferred_parameters.clone();

                let inferred_return_type = *inferred_return_type;

                let mut parameters = parameters.clone();

                let mut return_type = *return_type;

                for (inferred_parameter, parameter) in
                    inferred_parameters.into_iter().zip(&mut parameters)
                {
                    *parameter = self.check_inferred(inferred_parameter, *parameter, context)?;
                }

                return_type = self.check_inferred(inferred_return_type, return_type, context)?;

                Ok(self.push_type(Type::Fn {
                    parameters,
                    return_type,
                }))
            }
            (
                a @ Type::Product {
                    name: inferred_name,
                    fields: inferred_fields,
                    generics: inferred_generics,
                },
                b @ Type::Product {
                    name,
                    fields,
                    generics,
                },
            ) => {
                if inferred_name != name {
                    return Err(Error::TypeMismatch {
                        expected: a.to_string(self.types.as_slice()),
                        got: b.to_string(self.types.as_slice()),
                    });
                }

                let inferred_fields = inferred_fields.clone();

                let inferred_generics = inferred_generics.clone();

                let name = name.clone();

                let mut fields = fields.clone();

                let mut generics = generics.clone();

                for ((_, inferred_field), (_, field)) in
                    inferred_fields.into_iter().zip(&mut fields)
                {
                    *field = self.check_inferred(inferred_field, *field, context)?;
                }

                for (inferred_generic, generic) in inferred_generics.into_iter().zip(&mut generics)
                {
                    *generic = self.check_inferred(inferred_generic, *generic, context)?;
                }

                Ok(self.push_type(Type::Product {
                    name,
                    fields,
                    generics,
                }))
            }
            (a, b) => Err(Error::TypeMismatch {
                expected: a.to_string(self.types.as_slice()),
                got: b.to_string(self.types.as_slice()),
            }),
        }
    }

    fn apply(
        &mut self,
        type_index: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, Error> {
        match self.get_type(type_index).expect("all types exist") {
            Type::Primitive(_) | Type::Generic(_) | Type::Unknown => Ok(type_index),
            Type::Existential(name) => Ok(context
                .iter()
                .rfind(|(context_name, _)| context_name == name)
                .map_or(type_index, |(_, type_index)| *type_index)),
            Type::Fn {
                parameters,
                return_type,
            } => {
                let mut parameters = parameters.clone();

                let mut return_type = *return_type;

                for parameter in &mut parameters {
                    *parameter = self.apply(*parameter, context)?;
                }

                return_type = self.apply(return_type, context)?;

                Ok(self.push_type(Type::Fn {
                    parameters,
                    return_type,
                }))
            }
            Type::Product {
                name,
                fields,
                generics,
            } => {
                let name = name.clone();

                let mut fields = fields.clone();

                let mut generics = generics.clone();

                for (_, field) in &mut fields {
                    *field = self.apply(*field, context)?;
                }

                for generic in &mut generics {
                    *generic = self.apply(*generic, context)?;
                }

                Ok(self.push_type(Type::Product {
                    name,
                    fields,
                    generics,
                }))
            }
        }
    }
}
