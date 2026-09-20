use crate::{
    Reportable, Span, Spanned,
    name_resolve::Names,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, TypeSignature, UnaryOp},
};

use std::{collections::HashMap, error, fmt, iter, ops::Index};

pub fn check_types(ast: &Ast, names: &Names) -> Result<TypeChecker, Vec<Spanned<Error>>> {
    let mut type_checker = TypeChecker::new();

    type_checker.check_primitives(ast);

    type_checker.check_types(ast, names, ast.roots());

    type_checker.check_items(ast, names, ast.roots());

    type_checker.type_check_functions(ast, names, ast.roots());

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
    associations: HashMap<Span, HashMap<String, Vec<AssociatedName>>>,
    associated_with: Option<Span>,
    resolved_associations: HashMap<Span, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F64,
    Boolean,
    Unit,
}

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(Spanned<Primitive>),
    Integer(u64),
    NegativeInteger(i64),
    Unknown,
    Fn {
        parameters: Vec<TypeIndex>,
        return_type: TypeIndex,
    },
    Existential(Spanned<String>),
    Generic(Spanned<String>),
    Product {
        name: Spanned<String>,
        fields: Vec<(String, TypeIndex)>,
        generics: Vec<TypeIndex>,
    },
    Sum {
        name: Spanned<String>,
        variants: Vec<TypeIndex>,
        generics: Vec<TypeIndex>,
    },
    AnyOf(Vec<(usize, TypeIndex)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeIndex(usize);

impl From<TypeIndex> for usize {
    fn from(value: TypeIndex) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssociatedName {
    span: Span,
    for_type: TypeIndex,
}

impl Type {
    #[allow(clippy::too_many_lines)]
    fn eq(&self, types: &[Self], other: &Self) -> bool {
        match (self, other) {
            (Self::Primitive(a), Self::Primitive(b)) => a.kind() == b.kind(),
            (Self::Unknown, Self::Unknown) => true,
            (Self::Generic(a), Self::Generic(b)) => a.kind() == b.kind(),
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
                    && a_parameters.len() == b_parameters.len()
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
                a_name.kind() == b_name.kind()
                    && a_fields.len() == b_fields.len()
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
                    && a_generics.len() == b_generics.len()
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
            (
                Self::Sum {
                    name: a_name,
                    variants: a_variants,
                    generics: a_generics,
                },
                Self::Sum {
                    name: b_name,
                    variants: b_variants,
                    generics: b_generics,
                },
            ) => {
                a_name.kind() == b_name.kind()
                    && a_variants.len() == b_variants.len()
                    && a_variants
                        .iter()
                        .zip(b_variants)
                        .all(|(a_variant_type, b_variant_type)| {
                            types
                                .get(usize::from(*a_variant_type))
                                .and_then(|a_variant_type| {
                                    Some((a_variant_type, types.get(usize::from(*b_variant_type))?))
                                })
                                .is_some_and(|(a_variant_type, b_variant_type)| {
                                    a_variant_type.eq(types, b_variant_type)
                                })
                        })
                    && a_generics.len() == b_generics.len()
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
            Self::Primitive(p) => match p.kind() {
                Primitive::U8 => "u8",
                Primitive::I8 => "i8",
                Primitive::U16 => "u16",
                Primitive::I16 => "i16",
                Primitive::U32 => "u32",
                Primitive::I32 => "i32",
                Primitive::U64 => "u64",
                Primitive::I64 => "i64",
                Primitive::F64 => "f64",
                Primitive::Boolean => "bool",
                Primitive::Unit => "unit",
            }
            .to_string(),
            Self::Integer(_) | Self::NegativeInteger(_) => "i64".to_string(),
            Self::Unknown => "!!UNKNOWN TYPE!!".to_string(),
            Self::Generic(name) | Self::Existential(name) => name.kind().clone(),
            Self::AnyOf(alternatives) => {
                let mut buffer = "ALTERNATIVES!(".to_string();

                if let Some(first_alternative) = alternatives
                    .first()
                    .and_then(|(_, type_index)| types.get(usize::from(*type_index)))
                {
                    buffer.push_str(first_alternative.to_string(types).as_str());

                    for alternative in alternatives
                        .iter()
                        .skip(1)
                        .filter_map(|(_, type_index)| types.get(usize::from(*type_index)))
                    {
                        buffer.push_str(" | ");

                        buffer.push_str(alternative.to_string(types).as_str());
                    }
                }

                buffer.push(')');

                buffer
            }
            Self::Product { name, generics, .. } | Self::Sum { name, generics, .. } => {
                let mut buffer = name.kind().clone();

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
                    // TODO: is it fine to use this span here?
                    && return_type.ne(types, &Self::Primitive(Spanned::new(Primitive::Unit, Span::new(0, 0, 0))))
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
    NotASum,
    InvalidSumVariant,
    NonExistentSumVariant,
    TooManyVariants,
    CannotHaveMethod,
    NonExistentMethod,
    MethodCalledUncallable,
    MethodCallArgumentCountMismatch { expected: usize, got: usize },
    FnFieldAsMethod,
    ModuleAsExpr,
    TypeAsExpr,
    AmbiguousFunction,
    TypeMustBeKnown,
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
            Self::CallArgumentCountMismatch { expected, got }
            | Self::MethodCallArgumentCountMismatch { expected, got } => {
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
            Self::NotASum => write!(f, "this type is not a sum"),
            Self::InvalidSumVariant => write!(f, "a sum variant must be a name and some fields"),
            Self::NonExistentSumVariant => write!(f, "this variant does not exist on the sum"),
            Self::TooManyVariants => {
                write!(f, "a sum may not have more than {} variants", u16::MAX)
            }
            Self::CannotHaveMethod => {
                write!(f, "this type cannot have a method")
            }
            Self::NonExistentMethod => {
                write!(f, "this method does not exist on this type")
            }
            Self::MethodCalledUncallable => {
                write!(f, "this associated name is not a method")
            }
            Self::FnFieldAsMethod => {
                write!(
                    f,
                    "you cannot directly use a function field like a method; try wrapping this in parentheses"
                )
            }
            Self::ModuleAsExpr => write!(f, "modules are not allowed to be used as expressions"),
            Self::TypeAsExpr => write!(f, "types are not allowed to be used as expressions"),
            Self::AmbiguousFunction => write!(
                f,
                "there are multiple functions with this name and matching parameters"
            ),
            Self::TypeMustBeKnown => write!(f, "the type must be known by this point"),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

impl Index<Span> for &TypeChecker {
    type Output = TypeIndex;

    fn index(&self, index: Span) -> &Self::Output {
        self.type_map.get(&index).unwrap_or_else(|| {
            panic!(
                "index out of bounds: the len is {} but the index is {index:?}",
                self.types.len(),
            );
        })
    }
}

impl Index<Span> for &mut TypeChecker {
    type Output = TypeIndex;

    fn index(&self, index: Span) -> &Self::Output {
        self.type_map.get(&index).unwrap_or_else(|| {
            panic!(
                "index out of bounds: the len is {} but the index is {index:?}",
                self.types.len(),
            );
        })
    }
}

impl Index<TypeIndex> for &TypeChecker {
    type Output = Type;

    fn index(&self, index: TypeIndex) -> &Self::Output {
        self.types.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "index out of bounds: the len is {} but the index is {index:?}",
                self.types.len(),
            );
        })
    }
}

impl Index<TypeIndex> for &mut TypeChecker {
    type Output = Type;

    fn index(&self, index: TypeIndex) -> &Self::Output {
        self.types.get(usize::from(index)).unwrap_or_else(|| {
            panic!(
                "index out of bounds: the len is {} but the index is {index:?}",
                self.types.len(),
            );
        })
    }
}

impl TypeChecker {
    fn new() -> Self {
        let mut me = Self {
            errors: vec![],
            type_map: HashMap::new(),
            types: vec![],
            fn_return_type: None,
            associations: HashMap::new(),
            associated_with: None,
            resolved_associations: HashMap::new(),
        };

        me.push_type(Type::Unknown);
        me.push_type(Type::Integer(0));

        me
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn resolve_association(&self, of: Span, name: &str, to_resolve: Span) -> Option<Span> {
        self.resolved_associations
            .get(&to_resolve)
            .and_then(|index| {
                self.associations
                    .get(&of)
                    .and_then(|names| names.get(name))
                    .and_then(|associations| {
                        associations
                            .get(*index)
                            .map(|associated_name| associated_name.span)
                    })
            })
    }

    fn push_type(&mut self, ty: Type) -> TypeIndex {
        let type_index = TypeIndex(self.types.len());

        self.types.push(ty);

        type_index
    }

    pub fn get_type(&self, span: Span) -> Option<&Type> {
        self.type_map
            .get(&span)
            .and_then(|type_index| self.types.get(usize::from(*type_index)))
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

    fn type_u8(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U8)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_i8(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I8)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_u16(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U16)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_i16(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I16)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_u32(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U32)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_i32(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I32)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_u64(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U64)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_i64(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I64)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    fn type_integer(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Integer(_)))
                .expect("the integer type should be initialized before all type checking"),
        )
    }

    fn type_float(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::F64)))
                .expect("the f64 type should be initialized before all type checking"),
        )
    }

    fn type_boolean(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::Boolean)))
                .expect("the bool type should be initialized before all type checking"),
        )
    }

    fn type_unit(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::Unit)))
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
            if let Item::Primitive(name) = ast[*root].kind() {
                let type_index =
                    match name.kind().as_str() {
                        "u8" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::U8, name.span()))),
                        "i8" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::I8, name.span()))),
                        "u16" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::U16, name.span()))),
                        "i16" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::I16, name.span()))),
                        "u32" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::U32, name.span()))),
                        "i32" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::I32, name.span()))),
                        "u64" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::U64, name.span()))),
                        "i64" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::I64, name.span()))),
                        "f64" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::F64, name.span()))),
                        "bool" => self.push_type(Type::Primitive(Spanned::new(
                            Primitive::Boolean,
                            name.span(),
                        ))),
                        "unit" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::Unit, name.span()))),
                        _ => {
                            unreachable!("unknown primitive declared in core");
                        }
                    };

                self.type_map.insert(name.span(), type_index);
            }
        }
    }

    fn check_types(&mut self, ast: &Ast, names: &Names, items: &[ItemIndex]) {
        for item in items {
            match ast[*item].kind() {
                Item::Mod {
                    contents, generics, ..
                } => {
                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);
                    }

                    self.check_types(ast, names, contents.as_slice());
                }
                Item::Teach {
                    student,
                    body,
                    generics,
                } => {
                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);
                    }

                    self.check_type_signature(names, student);

                    self.check_types(ast, names, body.as_slice());
                }
                Item::Product {
                    name,
                    fields,
                    generics,
                    ..
                } => {
                    let mut generic_type_indices = vec![];

                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(Spanned::new(
                            generic.kind().clone(),
                            generic.span(),
                        )));

                        self.type_map.insert(generic.span(), type_index);

                        generic_type_indices.push(type_index);
                    }

                    let fields = fields
                        .iter()
                        .map(|field| {
                            self.check_type_signature(names, field.ty());

                            let type_index = self[field.ty().span()];

                            self.type_map.insert(field.name().span(), type_index);

                            (
                                field.name().kind().clone(),
                                self.get_type_index_or_error(field.name().span()),
                            )
                        })
                        .collect::<Vec<_>>();

                    let type_index = self.push_type(Type::Product {
                        name: Spanned::new(name.kind().clone(), name.span()),
                        fields,
                        generics: generic_type_indices,
                    });

                    self.type_map.insert(name.span(), type_index);
                }
                Item::Sum {
                    name,
                    variants,
                    generics,
                    ..
                } => {
                    let mut generic_type_indices = vec![];

                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(Spanned::new(
                            generic.kind().clone(),
                            generic.span(),
                        )));

                        self.type_map.insert(generic.span(), type_index);

                        generic_type_indices.push(type_index);
                    }

                    self.check_types(ast, names, variants.as_slice());

                    let variants = variants
                        .iter()
                        .map(|variant| {
                            self[if let Item::Product { name, .. } = ast[*variant].kind() {
                                Some(name.span())
                            } else {
                                None
                            }
                            .expect("all items exist")]
                        })
                        .collect::<Vec<_>>();

                    if variants.len() > usize::from(u16::MAX) {
                        self.errors
                            .push(Spanned::new(Error::TooManyVariants, ast[*item].span()));
                    }

                    let type_index = self.push_type(Type::Sum {
                        name: Spanned::new(name.kind().clone(), name.span()),
                        variants,
                        generics: generic_type_indices,
                    });

                    self.type_map.insert(name.span(), type_index);
                }
                Item::Primitive(_) | Item::NativeFn { .. } | Item::Fn { .. } => {}
            }
        }
    }

    fn check_items(&mut self, ast: &Ast, names: &Names, items: &[ItemIndex]) {
        for item in items {
            let name = match ast[*item].kind() {
                Item::Primitive(name) | Item::Product { name, .. } | Item::Sum { name, .. } => {
                    Some(name)
                }
                Item::Mod { contents, .. } => {
                    self.check_items(ast, names, contents.as_slice());

                    None
                }
                Item::Teach { body, student, .. } => {
                    let associated_with = self.associated_with.take();

                    self.associated_with = Some(student.span());

                    self.check_items(ast, names, body.as_slice());

                    self.associated_with = associated_with;

                    None
                }
                Item::NativeFn { name, .. } => {
                    self.check_native_function(ast, names, *item);

                    Some(name)
                }
                Item::Fn { name, .. } => {
                    self.check_function(ast, names, *item);

                    Some(name)
                }
            };

            if let Some(name) = name
                && let Some(associated_with) = self.associated_with
            {
                let for_type = self[associated_with];

                self.associations
                    .entry(names[associated_with])
                    .and_modify(|names| {
                        names
                            .entry(name.kind().clone())
                            .and_modify(|associations| {
                                associations.push(AssociatedName {
                                    span: name.span(),
                                    for_type,
                                });
                            })
                            .or_insert_with(|| {
                                vec![AssociatedName {
                                    span: name.span(),
                                    for_type,
                                }]
                            });
                    })
                    .or_insert_with(|| {
                        let mut names = HashMap::new();

                        names.insert(
                            name.kind().clone(),
                            vec![AssociatedName {
                                span: name.span(),
                                for_type,
                            }],
                        );

                        names
                    });
            }
        }
    }

    fn check_native_function(&mut self, ast: &Ast, names: &Names, item: ItemIndex) {
        if let Item::NativeFn {
            name, signature, ..
        } = ast[item].kind()
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
        match &self[type_index] {
            Type::Primitive(_) | Type::Integer(_) | Type::NegativeInteger(_) | Type::Unknown | Type::Generic(_) => type_index,
            Type::Existential(name) => {
                originals.iter().position(|type_index| {
                    matches!(&self[*type_index], Type::Existential(original) if original.kind() == name.kind())
                }).map_or(type_index, |index| replacements[index])
            }
            Type::AnyOf(alternatives) => {
                let alternatives = alternatives.clone().into_iter().map(|(index, type_index)| (index, self.substitute_type(originals, replacements, type_index))).collect::<Vec<_>>();

                self.push_type(Type::AnyOf(alternatives))
            }
            Type::Fn {
                parameters,
                return_type,
            } => {
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
            Type::Product {
                name,
                fields,
                generics,
            } => {
                let name = Spanned::new(name.kind().clone(), name.span());

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
            Type::Sum {
                name,
                variants,
                generics,
            } => {
                let name = Spanned::new(name.kind().clone(), name.span());

                let generics = generics.clone();

                let variants = variants
                    .clone()
                    .into_iter()
                    .map(|type_index| {
                        self.substitute_type(originals, replacements, type_index)
                    })
                    .collect::<Vec<_>>();

                let generics = generics
                    .into_iter()
                    .map(|type_index| self.substitute_type(originals, replacements, type_index))
                    .collect::<Vec<_>>();

                self.push_type(Type::Sum {
                    name,
                    variants,
                    generics,
                })
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_type_signature(&mut self, names: &Names, ty: &Spanned<TypeSignature>) {
        match ty.kind() {
            TypeSignature::SelfTy => {
                self.type_map.insert(
                    ty.span(),
                    self[self.associated_with.expect(
                        "name resolution guarantees \"Self\" is only used in valid positions",
                    )],
                );
            }
            TypeSignature::Normal { name, generics }
            | TypeSignature::Path { name, generics, .. } => {
                let checked_generics = generics.iter().fold(vec![], |mut accum, generic| {
                    self.check_type_signature(names, generic);

                    accum.push(self[generic.span()]);

                    accum
                });

                let type_index = self[names[name.span()]];

                match &self[type_index] {
                    Type::Integer(_) | Type::NegativeInteger(_) => {
                        unreachable!("normal type signatures are never integer literals")
                    }
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
                    Type::AnyOf(_) => unreachable!("normal type signatures are never Type::AnyOf"),
                    Type::Product {
                        fields,
                        generics: generic_types,
                        ..
                    } => {
                        let expected_generics = fields
                            .iter()
                            .filter(|(_, type_index)| {
                                matches!(
                                    &self[*type_index],
                                    Type::Existential(name)
                                        if !generic_types
                                            .iter()
                                            .any(|generic_type| {
                                                matches!(
                                                    &self[*generic_type],
                                                    Type::Existential(generic_name)
                                                        if generic_name.kind() == name.kind()
                                                )
                                            })
                                )
                            })
                            .count()
                            + generic_types.len();

                        if expected_generics == 0 && !checked_generics.is_empty() {
                            self.errors
                                .push(Spanned::new(Error::ExpectedZeroGenerics, ty.span()));

                            self.type_map.insert(ty.span(), self.type_unknown());
                        } else if checked_generics.len() != expected_generics {
                            self.errors.push(Spanned::new(
                                Error::GenericCountMismatch {
                                    expected: expected_generics,
                                    got: checked_generics.len(),
                                },
                                ty.span(),
                            ));

                            self.type_map.insert(ty.span(), self.type_unknown());
                        } else {
                            let fields = fields.clone();
                            let generic_types = generic_types.clone();

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
                                name: Spanned::new(name.kind().clone(), name.span()),
                                fields,
                                generics: checked_generics,
                            });

                            self.type_map.insert(ty.span(), type_index);
                        }
                    }
                    Type::Sum {
                        variants,
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
                            let variants = variants.clone();
                            let generic_types = generic_types.clone();

                            let variants =
                                variants.iter().fold(vec![], |mut accum, variant_type| {
                                    let type_index = self.substitute_type(
                                        generic_types.as_slice(),
                                        checked_generics.as_slice(),
                                        *variant_type,
                                    );

                                    accum.push(type_index);

                                    accum
                                });

                            let type_index = self.push_type(Type::Sum {
                                name: name.clone(),
                                variants,
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

                    parameter_types.push(self[parameter.span()]);
                }

                self.check_type_signature(names, return_type);

                let function_ty = Type::Fn {
                    parameters: parameter_types,
                    return_type: self[return_type.span()],
                };

                let type_index = self.push_type(function_ty);

                self.type_map.insert(ty.span(), type_index);
            }
        }
    }

    fn check_function(&mut self, ast: &Ast, names: &Names, item: ItemIndex) {
        if let Item::Fn {
            name,
            parameters,
            return_type,
            generics,
            ..
        } = ast[item].kind()
        {
            for generic in generics {
                let type_index = self.push_type(Type::Existential(generic.clone()));

                self.type_map.insert(generic.span(), type_index);
            }

            let parameters = parameters
                .iter()
                .map(|parameter| {
                    self.check_type_signature(names, parameter.ty());

                    let type_index = self[parameter.ty().span()];

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
            if let Item::Fn {
                name, parameters, ..
            } = ast[*root].kind()
                && name.kind() == "main"
            {
                if let Some(parameter) = parameters.first() {
                    self.errors.push(Spanned::new(
                        Error::MainFnWithParameters,
                        parameter.name().span(),
                    ));
                }

                if let Type::Fn { return_type, .. } = &self[self[name.span()]]
                    && !matches!(&self[*return_type], Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::Unit))
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
    fn type_check_functions(&mut self, ast: &Ast, names: &Names, items: &[ItemIndex]) {
        for item in items {
            match ast[*item].kind() {
                Item::Primitive(_)
                | Item::NativeFn { .. }
                | Item::Product { .. }
                | Item::Sum { .. } => {}
                Item::Mod { contents, .. } => {
                    self.type_check_functions(ast, names, contents.as_slice());
                }
                Item::Teach { body, .. } => {
                    self.type_check_functions(ast, names, body.as_slice());
                }
                Item::Fn {
                    name,
                    body,
                    generics,
                    ..
                } => {
                    let mut generics = generics
                        .iter()
                        .map(|generic| (generic.kind().clone(), self[generic.span()]))
                        .collect::<Vec<_>>();

                    let expected_return_type =
                        if let Type::Fn { return_type, .. } = &self[self[name.span()]] {
                            Some(*return_type)
                        } else {
                            None
                        }
                        .map(|return_type| self.apply(return_type, &mut generics))
                        .expect("all functions have a return type");

                    let fn_return_type = self.fn_return_type;

                    self.fn_return_type = Some(expected_return_type);

                    let type_count = self.types.len();

                    if let Err(error) =
                        self.check(ast, names, *body, expected_return_type, &mut generics)
                    {
                        self.errors.push(Spanned::new(error, ast[*body].span()));
                    }

                    let type_i64 = self[self.type_i64()].clone();
                    let default_integer = self[self.type_u64()].clone();

                    for ty in &mut self.types[type_count..] {
                        match ty {
                            Type::Integer(value) => match i64::try_from(*value) {
                                Ok(_) => *ty = type_i64.clone(),
                                Err(_) => *ty = default_integer.clone(),
                            },
                            Type::NegativeInteger(_) => {
                                *ty = type_i64.clone();
                            }
                            _ => {}
                        }
                    }

                    self.fn_return_type = fn_return_type;
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infer(
        &mut self,
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        let span = ast[expr].span();

        let type_index = match ast[expr].kind() {
            Expr::BinaryNoLhs { .. }
            | Expr::CallNoCallee(_)
            | Expr::AsUnitNoValue
            | Expr::ProductNoName(_) => {
                unreachable!("these won't exist since parsing succeeded");
            }
            Expr::PathElement(_) => {
                self.errors.push(Spanned::new(Error::ModuleAsExpr, span));

                self.type_unknown()
            }
            Expr::SelfType => {
                self.errors.push(Spanned::new(Error::TypeAsExpr, span));

                self.type_unknown()
            }
            Expr::Integer(value) => self.push_type(Type::Integer(*value)),
            Expr::NegativeInteger(value) => self.push_type(Type::NegativeInteger(*value)),
            Expr::Float(_) => self.type_float(),
            Expr::Boolean(_) => self.type_boolean(),
            Expr::Unit => self.type_unit(),
            Expr::Name(_) => {
                let type_index = self[names[span]];

                if let Type::Existential(name) = &self[type_index] {
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
                    let condition_span = ast[*condition].span();

                    self.errors
                        .push(Spanned::new(Error::ConditionNotBoolean, condition_span));
                }

                let when_true_type = self.infer(ast, names, *when_true, context);

                if let Err(error) = self.check(ast, names, *otherwise, when_true_type, context) {
                    let otherwise_span = ast[*otherwise].span();

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
                    let condition_span = ast[*condition].span();

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

                    let annotation_ty = self[annotation.span()];

                    if let Err(error) = self.check(ast, names, *value, annotation_ty, context) {
                        let value_span = ast[*value].span();

                        self.errors.push(Spanned::new(error, value_span));

                        self.type_unknown()
                    } else {
                        self.type_map.insert(ast[*value].span(), annotation_ty);

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

                match self.infer_call(
                    ast,
                    names,
                    (ast[*callee].span(), callee_type_index, arguments.clone()),
                    context,
                ) {
                    Ok(type_index) => type_index,
                    Err(error) => {
                        if !matches!(self[self[ast[*callee].span()]], Type::Unknown) {
                            self.errors.push(error);
                        }

                        self.type_unknown()
                    }
                }
            }
            Expr::MethodCall {
                target,
                method,
                arguments,
            } => {
                let target_type_index = self.infer(ast, names, *target, context);

                let type_span = match self[target_type_index].clone() {
                    Type::Integer(_) | Type::NegativeInteger(_) => {
                        self.errors
                            .push(Spanned::new(Error::TypeMustBeKnown, ast[*target].span()));

                        return self.type_unknown();
                    }
                    Type::Unknown => return self.type_unknown(),
                    Type::Existential(_) | Type::Generic(_) | Type::Fn { .. } | Type::AnyOf(_) => {
                        None
                    }
                    Type::Primitive(primitive) => Some(primitive.span()),
                    Type::Product { name, .. } | Type::Sum { name, .. } => Some(name.span()),
                };

                let type_span =
                    type_span.map(|type_span| names.get(type_span).unwrap_or(type_span));

                if let Some(type_span) = type_span
                    && let Expr::Name(name) = ast[*method].kind()
                {
                    if names.get_association(type_span, name).is_some()
                        && let Some(associations) = self
                            .associations
                            .get(&type_span)
                            .and_then(|names| names.get(name))
                    {
                        let valid_associations = associations
                            .clone()
                            .into_iter()
                            .enumerate()
                            .filter_map(|(index, associated_name)| {
                                self.check_inferred(
                                    self[type_span],
                                    associated_name.for_type,
                                    context,
                                )
                                .ok()
                                .map(|_| (index, self[associated_name.span]))
                            })
                            .collect::<Vec<_>>();

                        let type_index = self.push_type(Type::AnyOf(valid_associations));

                        self.type_map.insert(ast[*method].span(), type_index);
                    } else if let Type::Product { fields, .. } = &self[target_type_index]
                        && let Some(Type::Fn { .. }) =
                            fields.iter().find_map(|(field_name, field_type)| {
                                if field_name == name {
                                    Some(&self[*field_type])
                                } else {
                                    None
                                }
                            })
                    {
                        self.errors.push(Spanned::new(
                            Error::FnFieldAsMethod,
                            ast[*target]
                                .span()
                                .combine_with(ast[*method].span())
                                .expect("these spans are from the same source"),
                        ));

                        return self.type_unknown();
                    } else {
                        self.errors
                            .push(Spanned::new(Error::NonExistentMethod, ast[*method].span()));

                        return self.type_unknown();
                    }

                    match self.infer_call(
                        ast,
                        names,
                        (
                            ast[*method].span(),
                            self[ast[*method].span()],
                            iter::once(*target)
                                .chain(arguments.iter().copied())
                                .collect::<Vec<_>>(),
                        ),
                        context,
                    ) {
                        Ok(type_index) => type_index,
                        Err(error) => {
                            if !matches!(self[self[ast[*method].span()]], Type::Unknown) {
                                self.errors.push(error.transmute(|error| match error {
                                    Error::CallArgumentCountMismatch { expected, got } => {
                                        Error::MethodCallArgumentCountMismatch { expected, got }
                                    }
                                    Error::CalledUncallable => Error::MethodCalledUncallable,
                                    _ => error,
                                }));
                            }

                            self.type_unknown()
                        }
                    }
                } else {
                    self.errors
                        .push(Spanned::new(Error::CannotHaveMethod, ast[*target].span()));

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
                if let Type::Product {
                    name: type_name,
                    fields: field_types,
                    generics,
                } = self[self[names[name.span()]]].clone()
                {
                    let error_count = self.errors.len();

                    let mut context = context.clone();

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
                                match self.check(ast, names, *field, *field_type, &mut context) {
                                    Ok(ty) => {
                                        checked_fields.push((field_name.kind().clone(), ty));
                                    }
                                    Err(error) => {
                                        self.errors.push(Spanned::new(error, ast[*field].span()));
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

                    checked_fields.sort_by(|(a_name, _), (b_name, _)| a_name.cmp(b_name));

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
                        let generics = generics
                            .into_iter()
                            .map(|generic| self.apply(generic, &mut context))
                            .collect::<Vec<_>>();

                        self.push_type(Type::Product {
                            name: type_name,
                            fields: checked_fields,
                            generics,
                        })
                    } else {
                        self.type_unknown()
                    }
                } else {
                    if !matches!(self[self[names[name.span()]]], Type::Unknown) {
                        self.errors
                            .push(Spanned::new(Error::UnknownProduct, name.span()));
                    }

                    self.type_unknown()
                }
            }
        };

        self.type_map.insert(span, type_index);

        type_index
    }

    fn infer_call(
        &mut self,
        ast: &Ast,
        names: &Names,
        (callee_span, callee_type_index, arguments): (Span, TypeIndex, Vec<ExprIndex>),
        context: &[(String, TypeIndex)],
    ) -> Result<TypeIndex, Spanned<Error>> {
        let functions = match &self[callee_type_index] {
            function @ Type::Fn { .. } => {
                vec![(0, function.clone())]
            }
            Type::AnyOf(alternatives) => alternatives
                .iter()
                .filter_map(|(index, type_index)| match &self[*type_index] {
                    Type::AnyOf(_) => {
                        unreachable!("Type::AnyOf should never be nested");
                    }
                    Type::Fn { .. } => Some((*index, self[*type_index].clone())),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => {
                return Err(Spanned::new(Error::CalledUncallable, callee_span));
            }
        };

        let functions = functions
            .into_iter()
            .map(|(index, function)| {
                if let Type::Fn {
                    parameters,
                    return_type,
                } = function
                {
                    if arguments.len() == parameters.len() {
                        let mut context = context.to_owned();

                        for (argument, parameter) in arguments.iter().zip(&parameters) {
                            if let Err(error) =
                                self.check(ast, names, *argument, *parameter, &mut context)
                            {
                                return Err(Spanned::new(error, ast[*argument].span()));
                            }
                        }

                        Ok((index, self.apply(return_type, &mut context)))
                    } else {
                        Err(Spanned::new(
                            Error::CallArgumentCountMismatch {
                                expected: parameters.len(),
                                got: arguments.len(),
                            },
                            callee_span,
                        ))
                    }
                } else {
                    unreachable!("types that aren't functions get filtered out before here");
                }
            })
            .collect::<Vec<_>>();

        let ok_functions = functions
            .iter()
            .filter(|function| function.is_ok())
            .collect::<Vec<_>>();

        if ok_functions.is_empty() {
            if functions.is_empty() {
                return Err(Spanned::new(Error::CalledUncallable, callee_span));
            }

            Err(functions
                .into_iter()
                .find_map(Result::err)
                .expect("if it's not empty and there's no ok, there's at least one error"))
        } else if ok_functions.len() > 1 {
            Err(Spanned::new(Error::AmbiguousFunction, callee_span))
        } else {
            let (index, type_index) = functions
                .into_iter()
                .find_map(|function| {
                    if function.is_ok() {
                        function.ok()
                    } else {
                        None
                    }
                })
                .expect("it's going to be okay");

            self.resolved_associations.insert(callee_span, index);

            Ok(type_index)
        }
    }

    fn infer_unary(
        &mut self,
        ast: &Ast,
        names: &Names,
        (op, operand): (UnaryOp, ExprIndex),
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match op {
            UnaryOp::Not => {
                self.infer(ast, names, operand, context);

                if let Err(error) = self.check(ast, names, operand, self.type_boolean(), context) {
                    let span = ast[operand].span();

                    self.errors.push(Spanned::new(error, span));

                    self.type_unknown()
                } else {
                    self.type_boolean()
                }
            }
            UnaryOp::Negate => {
                self.infer(ast, names, operand, context);

                if self
                    .check(ast, names, operand, self.type_integer(), context)
                    .or_else(|_| self.check(ast, names, operand, self.type_u8(), context))
                    .or_else(|_| self.check(ast, names, operand, self.type_i16(), context))
                    .or_else(|_| self.check(ast, names, operand, self.type_i32(), context))
                    .or_else(|_| self.check(ast, names, operand, self.type_i64(), context))
                    .or_else(|_| self.check(ast, names, operand, self.type_float(), context))
                    .is_err()
                {
                    let span = ast[operand].span();

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
        names: &Names,
        (op, lhs, rhs): (BinaryOp, ExprIndex, ExprIndex),
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match op {
            BinaryOp::PathAccess => {
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

                let lhs_span = ast[lhs].span();

                if let Expr::Name(name) = ast[rhs].kind()
                    && let Some(type_span) = names.get(lhs_span)
                    && names.get_association(lhs_span, name).is_some()
                    && let Some(associations) = self
                        .associations
                        .get(&type_span)
                        .and_then(|names| names.get(name))
                {
                    let valid_associations = associations
                        .clone()
                        .into_iter()
                        .enumerate()
                        .filter_map(|(index, associated_name)| {
                            self.check_inferred(self[type_span], associated_name.for_type, context)
                                .ok()
                                .map(|_| (index, self[associated_name.span]))
                        })
                        .collect::<Vec<_>>();

                    self.push_type(Type::AnyOf(valid_associations))
                } else if let Some(Type::Sum { variants, .. }) = self.get_type(names[lhs_span]) {
                    let rhs_span = ast[rhs].span();

                    if let Expr::Product { name, .. } = ast[rhs].kind() {
                        if let Some((variant_index, variant_type)) =
                            variants.iter().enumerate().find_map(|(i, variant)| {
                                if let Type::Product {
                                    name: variant_name, ..
                                } = &self[*variant]
                                    && name.kind() == variant_name.kind()
                                {
                                    Some((i, *variant))
                                } else {
                                    None
                                }
                            })
                        {
                            let mut context = context.clone();

                            match self.check(ast, names, rhs, variant_type, &mut context) {
                                Ok(type_index) => {
                                    let Type::Sum {
                                        name,
                                        variants,
                                        generics,
                                    } = self[self[names[lhs_span]]].clone()
                                    else {
                                        unreachable!(
                                            "the outer if condition guarantees this is true"
                                        );
                                    };

                                    let ty = Type::Sum {
                                        name,
                                        variants: variants
                                            .into_iter()
                                            .enumerate()
                                            .map(|(i, variant)| {
                                                if i == variant_index {
                                                    type_index
                                                } else {
                                                    variant
                                                }
                                            })
                                            .collect::<Vec<_>>(),
                                        generics: generics
                                            .into_iter()
                                            .map(|generic| self.apply(generic, &mut context))
                                            .collect::<Vec<_>>(),
                                    };

                                    self.push_type(ty)
                                }
                                Err(error) => {
                                    self.errors.push(Spanned::new(error, rhs_span));

                                    self.type_unknown()
                                }
                            }
                        } else {
                            self.errors
                                .push(Spanned::new(Error::NonExistentSumVariant, rhs_span));

                            self.type_unknown()
                        }
                    } else {
                        self.errors
                            .push(Spanned::new(Error::InvalidSumVariant, rhs_span));

                        self.type_unknown()
                    }
                } else {
                    self.infer(ast, names, rhs, context)
                }
            }
            BinaryOp::Access => {
                let lhs_type = self.infer(ast, names, lhs, context);

                let lhs_span = ast[lhs].span();

                if let Type::Product { fields, .. } = &self[lhs_type] {
                    if let Expr::Name(name) = ast[rhs].kind() {
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
                            self.errors
                                .push(Spanned::new(Error::NonExistentField, ast[rhs].span()));

                            self.type_unknown()
                        }
                    } else {
                        self.errors
                            .push(Spanned::new(Error::InvalidAccess, ast[rhs].span()));

                        self.type_unknown()
                    }
                } else {
                    if !matches!(self[lhs_type], Type::Unknown) {
                        self.errors
                            .push(Spanned::new(Error::CannotAccess, lhs_span));
                    }

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
                    .or_else(|_| self.check(ast, names, lhs, self.type_u8(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i8(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_u16(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i16(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_u32(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i32(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_u64(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i64(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_float(), context))
                {
                    Err(_) => {
                        let span = ast[lhs].span();

                        self.errors
                            .push(Spanned::new(Error::ArithmeticImpossible, span));

                        self.type_unknown()
                    }
                    Ok(lhs_type) => match self.check(ast, names, rhs, lhs_type, context) {
                        Err(error) => {
                            let span = ast[rhs].span();

                            self.errors.push(Spanned::new(error, span));

                            self.type_unknown()
                        }
                        Ok(rhs_type) => rhs_type,
                    },
                }
            }
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessOrEqual
            | BinaryOp::GreaterOrEqual => {
                match self
                    .check(ast, names, lhs, self.type_integer(), context)
                    .or_else(|_| self.check(ast, names, lhs, self.type_u8(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i8(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_u16(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i16(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_u32(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i32(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_u64(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_i64(), context))
                    .or_else(|_| self.check(ast, names, lhs, self.type_float(), context))
                {
                    Err(_) => {
                        let span = ast[lhs].span();

                        self.errors.push(Spanned::new(Error::CannotCompare, span));

                        self.type_unknown()
                    }
                    Ok(lhs_type) => match self.check(ast, names, rhs, lhs_type, context) {
                        Err(error) => {
                            let span = ast[rhs].span();

                            self.errors.push(Spanned::new(error, span));

                            self.type_unknown()
                        }
                        Ok(_) => self.type_boolean(),
                    },
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                match self.check(ast, names, lhs, self.type_boolean(), context) {
                    Err(_) => {
                        let span = ast[lhs].span();

                        self.errors
                            .push(Spanned::new(Error::ArithmeticImpossible, span));

                        self.type_unknown()
                    }
                    Ok(lhs_type) => match self.check(ast, names, rhs, lhs_type, context) {
                        Err(error) => {
                            let span = ast[rhs].span();

                            self.errors.push(Spanned::new(error, span));

                            self.type_unknown()
                        }
                        Ok(_) => self.type_boolean(),
                    },
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let lhs_type = self.infer(ast, names, lhs, context);

                match self.check(ast, names, rhs, lhs_type, context) {
                    Err(error) => {
                        let span = ast[rhs].span();

                        self.errors.push(Spanned::new(error, span));

                        self.type_unknown()
                    }
                    Ok(_) => self.type_boolean(),
                }
            }
            BinaryOp::Assign => {
                let lhs_type = self.infer(ast, names, lhs, context);

                match self.check(ast, names, rhs, lhs_type, context) {
                    Err(error) => {
                        let span = ast[rhs].span();

                        self.errors.push(Spanned::new(error, span));

                        self.type_unknown()
                    }
                    Ok(rhs_type) => {
                        let rhs_span = ast[rhs].span();

                        match ast[lhs].kind() {
                            Expr::Name(_) => {
                                let lhs_span = names[ast[lhs].span()];

                                self.type_map.insert(lhs_span, rhs_type);
                            }
                            Expr::Binary {
                                op: BinaryOp::Access,
                                lhs,
                                rhs,
                            } => {
                                if self
                                    .check_inferred(lhs_type, self.type_unknown(), context)
                                    .is_err()
                                {
                                    return self.type_unknown();
                                }

                                let lhs_span = ast[*lhs].span();

                                let Expr::Name(accessor) = ast[*rhs].kind() else {
                                    unreachable!("accessors are always names");
                                };

                                let mut context = context.clone();

                                let Type::Product {
                                    fields, generics, ..
                                } = &self[self[names[lhs_span]]]
                                else {
                                    unreachable!("accessees are always products");
                                };

                                let mut applied_generics = generics.clone();

                                let field_index = fields
                                    .iter()
                                    .position(|(field_name, _)| field_name == accessor)
                                    .expect("the field is guaranteed to exist");

                                match self.check_inferred(
                                    fields[field_index].1,
                                    rhs_type,
                                    &mut context,
                                ) {
                                    Err(type_index) => {
                                        let error = Spanned::new(
                                            self.type_mismatch_error(type_index, rhs_type),
                                            rhs_span,
                                        );

                                        self.errors.push(error);

                                        return self.type_unknown();
                                    }
                                    Ok(type_index) => {
                                        for generic in &mut applied_generics {
                                            *generic = self.apply(*generic, &mut context);
                                        }

                                        let product_type_index = self[names[lhs_span]];

                                        let Some(Type::Product {
                                            fields, generics, ..
                                        }) = self.types.get_mut(usize::from(product_type_index))
                                        else {
                                            unreachable!("accessees are always products");
                                        };

                                        fields[field_index].1 = type_index;

                                        *generics = applied_generics;
                                    }
                                }
                            }
                            _ => {}
                        }

                        rhs_type
                    }
                }
            }
        }
    }

    fn overwrite_with(
        &mut self,
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        should_be: TypeIndex,
    ) -> TypeIndex {
        self.type_map.insert(
            names
                .get(ast[expr].span())
                .unwrap_or_else(|| ast[expr].span()),
            should_be,
        );

        self[names
            .get(ast[expr].span())
            .unwrap_or_else(|| ast[expr].span())]
    }

    fn type_mismatch_error(&self, type_index: TypeIndex, should_be: TypeIndex) -> Error {
        Error::TypeMismatch {
            expected: self[should_be].to_string(self.types.as_slice()),
            got: self[type_index].to_string(self.types.as_slice()),
        }
    }

    fn check(
        &mut self,
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        should_be: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, Error> {
        let expr_type_index = self.infer(ast, names, expr, context);

        self.check_inferred(expr_type_index, should_be, context)
            .inspect(|type_index| {
                self.overwrite_with(ast, names, expr, *type_index);
            })
            .map_err(|type_index| {
                let should_be = self.apply(should_be, context);

                self.type_mismatch_error(type_index, should_be)
            })
    }

    #[allow(clippy::too_many_lines)]
    fn check_inferred(
        &mut self,
        inferred: TypeIndex,
        should_be: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, TypeIndex> {
        match (&self[inferred], &self[should_be]) {
            (Type::Unknown, _) | (_, Type::Unknown) => Ok(self.type_unknown()),
            (
                Type::Integer(_) | Type::NegativeInteger(_),
                Type::Integer(_) | Type::NegativeInteger(_),
            ) => Ok(inferred),
            (Type::Integer(value), Type::Primitive(b)) => match b.kind() {
                Primitive::U8 if u8::try_from(*value).is_ok() => Ok(should_be),
                Primitive::I8 if i8::try_from(*value).is_ok() => Ok(should_be),
                Primitive::U16 if u16::try_from(*value).is_ok() => Ok(should_be),
                Primitive::I16 if i16::try_from(*value).is_ok() => Ok(should_be),
                Primitive::U32 if u32::try_from(*value).is_ok() => Ok(should_be),
                Primitive::I32 if i32::try_from(*value).is_ok() => Ok(should_be),
                Primitive::U64 => Ok(should_be),
                Primitive::I64 if i64::try_from(*value).is_ok() => Ok(should_be),
                _ => Err(inferred),
            },
            (Type::Primitive(a), Type::Integer(value)) => match a.kind() {
                Primitive::U8 if u8::try_from(*value).is_ok() => Ok(inferred),
                Primitive::I8 if i8::try_from(*value).is_ok() => Ok(inferred),
                Primitive::U16 if u16::try_from(*value).is_ok() => Ok(inferred),
                Primitive::I16 if i16::try_from(*value).is_ok() => Ok(inferred),
                Primitive::U32 if u32::try_from(*value).is_ok() => Ok(inferred),
                Primitive::I32 if i32::try_from(*value).is_ok() => Ok(inferred),
                Primitive::U64 => Ok(inferred),
                Primitive::I64 if i64::try_from(*value).is_ok() => Ok(inferred),
                _ => Err(should_be),
            },
            (Type::NegativeInteger(value), Type::Primitive(b)) => match b.kind() {
                Primitive::I8 if i8::try_from(*value).is_ok() => Ok(should_be),
                Primitive::I16 if i16::try_from(*value).is_ok() => Ok(should_be),
                Primitive::I32 if i32::try_from(*value).is_ok() => Ok(should_be),
                Primitive::I64 => Ok(should_be),
                _ => Err(inferred),
            },
            (Type::Primitive(a), Type::NegativeInteger(value)) => match a.kind() {
                Primitive::I8 if i8::try_from(*value).is_ok() => Ok(inferred),
                Primitive::I16 if i16::try_from(*value).is_ok() => Ok(inferred),
                Primitive::I32 if i32::try_from(*value).is_ok() => Ok(inferred),
                Primitive::I64 => Ok(inferred),
                _ => Err(should_be),
            },
            (Type::Primitive(a), Type::Primitive(b)) => {
                if a.kind() == b.kind() {
                    Ok(should_be)
                } else {
                    Err(inferred)
                }
            }
            (Type::Generic(a), Type::Generic(b)) => {
                if a.kind() == b.kind() {
                    Ok(should_be)
                } else {
                    Err(inferred)
                }
            }
            (_, Type::Existential(name)) => {
                context.push((name.kind().clone(), inferred));

                Ok(inferred)
            }
            (Type::Existential(name), _) => {
                context.push((name.kind().clone(), should_be));

                Ok(should_be)
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

                let mut errored = false;

                for (inferred_parameter, parameter) in
                    inferred_parameters.into_iter().zip(&mut parameters)
                {
                    match self.check_inferred(inferred_parameter, *parameter, context) {
                        Ok(type_index) => {
                            *parameter = type_index;
                        }
                        Err(type_index) => {
                            *parameter = type_index;
                            errored = true;
                        }
                    }
                }

                match self.check_inferred(inferred_return_type, return_type, context) {
                    Ok(type_index) => {
                        return_type = type_index;
                    }
                    Err(type_index) => {
                        return_type = type_index;
                        errored = true;
                    }
                }

                if errored {
                    Err(self.push_type(Type::Fn {
                        parameters,
                        return_type,
                    }))
                } else {
                    Ok(self.push_type(Type::Fn {
                        parameters,
                        return_type,
                    }))
                }
            }
            (
                Type::Product {
                    name: inferred_name,
                    fields: inferred_fields,
                    generics: inferred_generics,
                },
                Type::Product {
                    name,
                    fields,
                    generics,
                },
            ) => {
                let mut errored = inferred_name.kind() != name.kind();

                let inferred_fields = inferred_fields.clone();

                let inferred_generics = inferred_generics.clone();

                let name = name.clone();

                let mut fields = fields.clone();

                let mut generics = generics.clone();

                for ((_, inferred_field), (_, field)) in
                    inferred_fields.into_iter().zip(&mut fields)
                {
                    match self.check_inferred(inferred_field, *field, context) {
                        Ok(type_index) => {
                            *field = type_index;
                        }
                        Err(type_index) => {
                            *field = type_index;
                            errored = true;
                        }
                    }
                }

                for (inferred_generic, generic) in inferred_generics.into_iter().zip(&mut generics)
                {
                    match self.check_inferred(inferred_generic, *generic, context) {
                        Ok(type_index) => {
                            *generic = type_index;
                        }
                        Err(type_index) => {
                            *generic = type_index;
                            errored = true;
                        }
                    }
                }

                if errored {
                    Err(self.push_type(Type::Product {
                        name,
                        fields,
                        generics,
                    }))
                } else {
                    Ok(self.push_type(Type::Product {
                        name,
                        fields,
                        generics,
                    }))
                }
            }
            (
                Type::Sum {
                    name: inferred_name,
                    variants: inferred_variants,
                    generics: inferred_generics,
                },
                Type::Sum {
                    name,
                    variants,
                    generics,
                },
            ) => {
                let mut errored = inferred_name.kind() != name.kind();

                let inferred_variants = inferred_variants.clone();

                let inferred_generics = inferred_generics.clone();

                let name = name.clone();

                let mut variants = variants.clone();

                let mut generics = generics.clone();

                for (inferred_variant, variant) in inferred_variants.into_iter().zip(&mut variants)
                {
                    match self.check_inferred(inferred_variant, *variant, context) {
                        Ok(type_index) => {
                            *variant = type_index;
                        }
                        Err(type_index) => {
                            *variant = type_index;
                            errored = true;
                        }
                    }
                }

                for (inferred_generic, generic) in inferred_generics.into_iter().zip(&mut generics)
                {
                    match self.check_inferred(inferred_generic, *generic, context) {
                        Ok(type_index) => {
                            *generic = type_index;
                        }
                        Err(type_index) => {
                            *generic = type_index;
                            errored = true;
                        }
                    }
                }

                if errored {
                    Err(self.push_type(Type::Sum {
                        name,
                        variants,
                        generics,
                    }))
                } else {
                    Ok(self.push_type(Type::Sum {
                        name,
                        variants,
                        generics,
                    }))
                }
            }
            (_, _) => Err(self.apply(inferred, context)),
        }
    }

    fn apply(
        &mut self,
        type_index: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match &self[type_index] {
            Type::Integer(_)
            | Type::NegativeInteger(_)
            | Type::Primitive(_)
            | Type::Generic(_)
            | Type::Unknown => type_index,
            Type::Existential(name) => context
                .iter()
                .rfind(|(context_name, _)| context_name == name.kind())
                .map_or(type_index, |(_, type_index)| *type_index),
            Type::AnyOf(alternatives) => {
                let alternatives = alternatives
                    .clone()
                    .into_iter()
                    .map(|(index, type_index)| (index, self.apply(type_index, context)))
                    .collect::<Vec<_>>();

                self.push_type(Type::AnyOf(alternatives))
            }
            Type::Fn {
                parameters,
                return_type,
            } => {
                let mut parameters = parameters.clone();

                let mut return_type = *return_type;

                for parameter in &mut parameters {
                    *parameter = self.apply(*parameter, context);
                }

                return_type = self.apply(return_type, context);

                self.push_type(Type::Fn {
                    parameters,
                    return_type,
                })
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
                    *field = self.apply(*field, context);
                }

                for generic in &mut generics {
                    *generic = self.apply(*generic, context);
                }

                self.push_type(Type::Product {
                    name,
                    fields,
                    generics,
                })
            }
            Type::Sum {
                name,
                variants,
                generics,
            } => {
                let name = name.clone();

                let mut variants = variants.clone();

                let mut generics = generics.clone();

                for variant in &mut variants {
                    *variant = self.apply(*variant, context);
                }

                for generic in &mut generics {
                    *generic = self.apply(*generic, context);
                }

                self.push_type(Type::Sum {
                    name,
                    variants,
                    generics,
                })
            }
        }
    }
}
