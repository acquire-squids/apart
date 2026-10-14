use crate::{
    ApartOptions, Reportable, Span, Spanned,
    name_resolve::{Definition, ModuleId, Names, ROOT_MODULE},
    parse::{
        Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, PathElement, Pattern, TypeSignature,
        UnaryOp, Visibility,
    },
};

use std::{collections::HashMap, error, fmt, ops::Index};

pub fn check_types(
    _: &ApartOptions,
    sources: &[(usize, (&str, &str))],
    ast: &Ast,
    names: &Names,
) -> Result<TypeChecker, Vec<Spanned<Error>>> {
    let mut type_checker = TypeChecker::new();

    type_checker.initialize_primitives(ast);

    type_checker.check_type_signatures(ast, names, ast.roots());

    type_checker.check_aliases(ast, names, ast.roots());

    type_checker.associate_items(ast, names, ast.roots());

    type_checker.type_check_functions(sources, ast, names, ast.roots(), [].as_slice());

    type_checker.replace_default_numeric_types(sources);

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
    module_generics: Vec<Vec<TypeIndex>>,
    current_module: Vec<ModuleId>,
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
    F32,
    F64,
    Boolean,
    Unit,
    String,
    Never,
}

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(Spanned<Primitive>),
    Integer(Spanned<u32>),
    NegativeInteger(Spanned<u32>),
    Float(Span),
    AnyOf {
        alternatives: Vec<TypeIndex>,
        span: Span,
    },
    Error,
    Fn {
        parameters: Vec<TypeIndex>,
        return_type: TypeIndex,
    },
    Existential(Spanned<String>),
    Generic(Spanned<String>),
    Product {
        name: Spanned<String>,
        fields: Vec<Field>,
        generics: Vec<TypeIndex>,
    },
    Sum {
        name: Spanned<String>,
        variants: Vec<TypeIndex>,
        generics: Vec<TypeIndex>,
    },
    Associations(Vec<Association>),
}

#[derive(Debug, Clone)]
pub struct Association {
    resolved_index: usize,
    type_index: TypeIndex,
    span: Span,
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
    for_type: TypeIndex,
    definition: Definition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    name: String,
    ty: TypeIndex,
    visibility: Visibility,
}

impl Field {
    #[allow(dead_code)]
    #[must_use]
    pub const fn name(&self) -> &str {
        self.name.as_str()
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn type_index(&self) -> TypeIndex {
        self.ty
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn visibility(&self) -> Visibility {
        self.visibility
    }
}

impl Type {
    #[allow(clippy::too_many_lines)]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Primitive(a), Self::Primitive(b)) => a.kind() == b.kind(),
            (Self::Integer(a), Self::Integer(b)) => a.kind() == b.kind() && a.span() == b.span(),
            (Self::NegativeInteger(a), Self::NegativeInteger(b)) => {
                a.kind() == b.kind() && a.span() == b.span()
            }
            (Self::Error, Self::Error) => true,
            (Self::Existential(a), Self::Existential(b)) => {
                a.kind() == b.kind() && a.span() == b.span()
            }
            (Self::Generic(a), Self::Generic(b)) => a.kind() == b.kind() && a.span() == b.span(),
            (Self::Associations(a), Self::Associations(b)) => {
                a.iter().zip(b).all(|(a_association, b_association)| {
                    a_association.type_index == b_association.type_index
                })
            }
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
                a_return_type == b_return_type
                    && a_parameters.len() == b_parameters.len()
                    && a_parameters.iter().zip(b_parameters).all(
                        |(a_parameter_type, b_parameter_type)| a_parameter_type == b_parameter_type,
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
                    && a_fields.iter().zip(b_fields).all(|(a_field, b_field)| {
                        a_field.name() == b_field.name()
                            && a_field.type_index() == b_field.type_index()
                    })
                    && a_generics.len() == b_generics.len()
                    && a_generics
                        .iter()
                        .zip(b_generics)
                        .all(|(a_generic_type, b_generic_type)| a_generic_type == b_generic_type)
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
                        .all(|(a_variant_type, b_variant_type)| a_variant_type == b_variant_type)
                    && a_generics.len() == b_generics.len()
                    && a_generics
                        .iter()
                        .zip(b_generics)
                        .all(|(a_generic_type, b_generic_type)| a_generic_type == b_generic_type)
            }
            (_, _) => false,
        }
    }

    #[allow(clippy::too_many_lines)]
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
                Primitive::F32 => "f32",
                Primitive::F64 => "f64",
                Primitive::Boolean => "bool",
                Primitive::Unit => "unit",
                Primitive::String => "String",
                Primitive::Never => "never",
            }
            .to_string(),
            Self::Integer(_) | Self::NegativeInteger(_) => "i64".to_string(),
            Self::Float(_) => "f64".to_string(),
            Self::Error => "UNKNOWN TYPE!".to_string(),
            Self::Generic(name) | Self::Existential(name) => name.kind().clone(),
            Self::AnyOf { alternatives, .. } => {
                let mut buffer = "ALTERNATIVE!(".to_string();

                if let Some(first_alternative) = alternatives
                    .first()
                    .and_then(|type_index| types.get(usize::from(*type_index)))
                {
                    buffer.push_str(first_alternative.to_string(types).as_str());

                    for alternative in alternatives
                        .iter()
                        .skip(1)
                        .filter_map(|type_index| types.get(usize::from(*type_index)))
                    {
                        buffer.push_str(" | ");

                        buffer.push_str(alternative.to_string(types).as_str());
                    }
                }

                buffer.push(')');

                buffer
            }
            Self::Associations(alternatives) => {
                let mut buffer = "ASSOCIATIONS!(".to_string();

                if let Some(first_alternative) = alternatives
                    .first()
                    .and_then(|association| types.get(usize::from(association.type_index)))
                {
                    buffer.push_str(first_alternative.to_string(types).as_str());

                    for alternative in alternatives
                        .iter()
                        .skip(1)
                        .filter_map(|association| types.get(usize::from(association.type_index)))
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
                    && !matches!(return_type,
                        Self::Primitive(primitive)
                            if matches!(primitive.kind(), Primitive::Unit)
                    )
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
    ArithmeticImpossible(String),
    CannotCompare(String),
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
    ProductPatternMissingField(String),
    CannotNegate(String),
    PrivateField,
    PrivateFunction,
    PrivateMethod,
    UnknownInteger,
    UnknownFloat,
    InvalidInteger,
    InvalidFloat,
    TooManyArguments(usize),
    TooManyParameters(usize),
}

impl fmt::Display for Error {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, but got {got}")
            }
            Self::UnknownType => {
                write!(f, "the type of this expression is unknown")
            }
            Self::ArithmeticImpossible(ty) => {
                write!(f, "arithmetic cannot be performed with {ty}")
            }
            Self::CannotCompare(ty) => {
                write!(f, "{ty} cannot be compared in this way")
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
            Self::ProductPatternMissingField(name) => {
                write!(f, "this pattern is missing its required field \"{name}\"")
            }
            Self::CannotNegate(ty) => write!(f, "{ty} cannot be negated"),
            Self::PrivateField => write!(
                f,
                "this field is private and can't be accessed outside of the module that defined it"
            ),
            Self::PrivateFunction => write!(
                f,
                "this function is private and can't be called outside of the module that defined it"
            ),
            Self::PrivateMethod => write!(
                f,
                "this method is private and can't be called outside of the module that defined it"
            ),
            Self::UnknownInteger => write!(f, "the type of this integer is unknown"),
            Self::UnknownFloat => write!(f, "the type of this float is unknown"),
            Self::InvalidInteger => write!(
                f,
                "this integer does not fit into an i64 or a u64 and is thus invalid"
            ),
            Self::InvalidFloat => write!(
                f,
                "this float does not parse into an f64 or an f32 and is thus invalid"
            ),
            Self::TooManyArguments(got) => write!(
                f,
                "a function call may not exceed {} arguments (got {got})",
                u16::MAX
            ),
            Self::TooManyParameters(got) => write!(
                f,
                "a function definition may not exceed {} parameters (got {got})",
                u16::MAX
            ),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {
    fn notes(&self) -> Vec<String> {
        match self {
            Self::UnknownInteger => {
                vec!["try using the integer somewhere that has an integer annotation".to_string()]
            }
            Self::UnknownFloat => {
                vec!["try using the float somewhere that has a float annotation".to_string()]
            }
            _ => vec![],
        }
    }
}

impl Index<Span> for &TypeChecker {
    type Output = TypeIndex;

    fn index(&self, span: Span) -> &Self::Output {
        self.type_map.get(&span).unwrap_or_else(|| {
            panic!("tried to get the type of a span that was never inferred: {span:?}");
        })
    }
}

impl Index<Span> for &mut TypeChecker {
    type Output = TypeIndex;

    fn index(&self, span: Span) -> &Self::Output {
        self.type_map.get(&span).unwrap_or_else(|| {
            panic!("tried to get the type of a span that was never inferred: {span:?}");
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
            module_generics: vec![],
            current_module: vec![ROOT_MODULE.is()],
        };

        me.push_type(Type::Error);

        // TODO: is it okay to use this span?
        me.push_type(Type::Integer(Spanned::new(0, Span::new(0, 0, 0))));
        me.push_type(Type::NegativeInteger(Spanned::new(0, Span::new(0, 0, 0))));
        me.push_type(Type::Float(Span::new(0, 0, 0)));

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
                            .map(|associated_name| associated_name.definition.span())
                    })
            })
    }

    fn push_type(&mut self, ty: Type) -> TypeIndex {
        // TODO: this is probably slow?  fix it if it becomes a problem
        if let Some(type_index) = self
            .types
            .iter()
            .rposition(|existing_type| existing_type.eq(&ty))
            .map(TypeIndex)
        {
            type_index
        } else {
            let type_index = TypeIndex(self.types.len());

            self.types.push(ty);

            type_index
        }
    }

    pub fn get_type(&self, span: Span) -> Option<&Type> {
        self.type_map
            .get(&span)
            .and_then(|type_index| self.types.get(usize::from(*type_index)))
    }

    fn get_type_index_or_error(&mut self, span: Span) -> TypeIndex {
        if !self.type_map.contains_key(&span) {
            self.errors.push(Spanned::new(Error::UnknownType, span));

            self.type_map.insert(span, self.type_error());
        }

        self.type_map
            .get(&span)
            .copied()
            .expect("expressions always receive a fallback type if they don't have a type when `get_type_index_or_error` is called")
    }

    #[allow(dead_code)]
    fn type_u8(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U8)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_i8(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I8)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_u16(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U16)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_i16(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I16)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_u32(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U32)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_i32(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::I32)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_u64(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::U64)))
                .expect("the i64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
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

    fn type_negative_integer(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::NegativeInteger(_)))
                .expect("the negative integer type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_float(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Float(_)))
                .expect("the float type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_f32(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::F32)))
                .expect("the f32 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_f64(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::F64)))
                .expect("the f64 type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_boolean(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::Boolean)))
                .expect("the bool type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_unit(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::Unit)))
                .expect("the unit type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_string(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::String)))
                .expect("the string type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_never(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Primitive(primitive) if matches!(primitive.kind(), Primitive::Never)))
                .expect("the never type should be initialized before all type checking"),
        )
    }

    #[allow(dead_code)]
    fn type_error(&self) -> TypeIndex {
        TypeIndex(
            self.types
                .iter()
                .position(|ty| matches!(ty, Type::Error))
                .expect("the unknown type should be initialized before all type checking"),
        )
    }
}

impl TypeChecker {
    fn initialize_primitives(&mut self, ast: &Ast) {
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
                        "f32" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::F32, name.span()))),
                        "f64" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::F64, name.span()))),
                        "bool" => self.push_type(Type::Primitive(Spanned::new(
                            Primitive::Boolean,
                            name.span(),
                        ))),
                        "unit" => self
                            .push_type(Type::Primitive(Spanned::new(Primitive::Unit, name.span()))),
                        "String" => self.push_type(Type::Primitive(Spanned::new(
                            Primitive::String,
                            name.span(),
                        ))),
                        "never" => self.push_type(Type::Primitive(Spanned::new(
                            Primitive::Never,
                            name.span(),
                        ))),
                        _ => {
                            unreachable!("unknown primitive declared in core");
                        }
                    };

                self.type_map.insert(name.span(), type_index);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_type_signatures(&mut self, ast: &Ast, names: &Names, items: &[ItemIndex]) {
        for item in items {
            match ast[*item].kind() {
                Item::Mod {
                    contents, generics, ..
                } => {
                    let mut module_generics = vec![];

                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);

                        module_generics.push(type_index);
                    }

                    self.module_generics.push(module_generics);

                    self.check_type_signatures(ast, names, contents.as_slice());

                    self.module_generics.pop();
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

                    self.check_type_signatures(ast, names, body.as_slice());
                }
                Item::Product {
                    name,
                    fields,
                    generics,
                    ..
                } => {
                    let mut generic_type_indices = vec![];

                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);

                        generic_type_indices.push(type_index);
                    }

                    let fields = fields
                        .iter()
                        .map(|field| {
                            self.check_type_signature(names, field.ty());

                            let type_index = self[field.ty().span()];

                            self.type_map.insert(field.name().span(), type_index);

                            Field {
                                name: field.name().kind().clone(),
                                ty: self.get_type_index_or_error(field.name().span()),
                                visibility: field.visibility(),
                            }
                        })
                        .collect::<Vec<_>>();

                    for module_generic_type_index in self.module_generics.iter().flatten() {
                        if let Type::Existential(module_generic) = &self[*module_generic_type_index]
                            && fields.iter().any(|field| {
                                if let Type::Existential(generic) = &self[field.type_index()]
                                    && module_generic.span() == generic.span()
                                {
                                    true
                                } else {
                                    false
                                }
                            })
                        {
                            generic_type_indices.push(*module_generic_type_index);
                        }
                    }

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
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);

                        generic_type_indices.push(type_index);
                    }

                    self.check_type_signatures(ast, names, variants.as_slice());

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

                    for variant in &variants {
                        let Type::Product { generics, .. } = &self[*variant] else {
                            unreachable!("only products are sum variants");
                        };

                        generic_type_indices.extend(generics.iter().copied());
                    }

                    let type_index = self.push_type(Type::Sum {
                        name: Spanned::new(name.kind().clone(), name.span()),
                        variants,
                        generics: generic_type_indices,
                    });

                    self.type_map.insert(name.span(), type_index);
                }
                Item::Primitive(_)
                | Item::NativeFn { .. }
                | Item::Fn { .. }
                | Item::Directive { .. }
                | Item::Alias { .. } => {}
            }
        }
    }

    fn check_aliases(&mut self, ast: &Ast, names: &Names, items: &[ItemIndex]) {
        for item in items {
            match ast[*item].kind() {
                Item::Alias {
                    generics, target, ..
                } => {
                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);
                    }

                    self.check_type_signature(names, target);
                }
                Item::Mod {
                    contents, generics, ..
                } => {
                    let mut module_generics = vec![];

                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);

                        module_generics.push(type_index);
                    }

                    self.module_generics.push(module_generics);

                    self.check_aliases(ast, names, contents.as_slice());

                    self.module_generics.pop();
                }
                Item::Teach { body, generics, .. } => {
                    for generic in generics {
                        let type_index = self.push_type(Type::Existential(generic.clone()));

                        self.type_map.insert(generic.span(), type_index);
                    }

                    self.check_aliases(ast, names, body.as_slice());
                }
                Item::Primitive(_)
                | Item::NativeFn { .. }
                | Item::Fn { .. }
                | Item::Product { .. }
                | Item::Sum { .. }
                | Item::Directive { .. } => {}
            }
        }
    }

    fn associate_items(&mut self, ast: &Ast, names: &Names, items: &[ItemIndex]) {
        for item in items {
            let name = match ast[*item].kind() {
                Item::Directive { .. } | Item::Alias { .. } => None,
                Item::Primitive(name) | Item::Product { name, .. } | Item::Sum { name, .. } => {
                    Some(name)
                }
                Item::Mod { contents, .. } => {
                    self.associate_items(ast, names, contents.as_slice());

                    None
                }
                Item::Teach { body, student, .. } => {
                    let associated_with = self.associated_with.take();

                    self.associated_with = Some(student.span());

                    self.associate_items(ast, names, body.as_slice());

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
                    .and_modify(|associations| {
                        associations
                            .entry(name.kind().clone())
                            .and_modify(|associations| {
                                associations.push(AssociatedName {
                                    for_type,
                                    definition: names
                                        .get_definition(name.span())
                                        .cloned()
                                        .expect("this association should have been found by name resolution"),
                                });
                            })
                            .or_insert_with(|| {
                                vec![AssociatedName {
                                    for_type,
                                    definition: names
                                        .get_definition(name.span())
                                        .cloned()
                                        .expect("this association should have been found by name resolution"),
                                }]
                            });
                    })
                    .or_insert_with(|| {
                        let mut associations = HashMap::new();

                        associations.insert(
                            name.kind().clone(),
                            vec![AssociatedName {
                                for_type,
                                definition: names
                                    .get_definition(name.span())
                                    .cloned()
                                    .expect("this association should have been found by name resolution"),
                            }],
                        );

                        associations
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
            Type::Primitive(_) | Type::Integer(_) | Type::NegativeInteger(_) | Type::Float(_) | Type::Error | Type::Generic(_) => type_index,
            Type::Existential(name) => {
                originals.iter().rposition(|type_index| {
                    matches!(&self[*type_index], Type::Existential(original) if original.kind() == name.kind())
                }).map_or(type_index, |index| replacements[index])
            }
            Type::AnyOf { alternatives, span } => {
                let span = *span;

                let alternatives = alternatives.clone().into_iter().map(|type_index| self.substitute_type(originals, replacements, type_index)).collect::<Vec<_>>();

                self.push_type(Type::AnyOf { alternatives, span })
            }
            Type::Associations(alternatives) => {
                let alternatives = alternatives
                    .clone()
                    .into_iter()
                    .map(|association| Association {
                        resolved_index: association.resolved_index,
                        type_index: self.substitute_type(originals, replacements, association.type_index),
                        span: association.span,
                    }).collect::<Vec<_>>();

                self.push_type(Type::Associations(alternatives))
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
                    .map(|field| {
                        Field {
                            name: field.name().to_string(),
                            ty: self.substitute_type(originals, replacements, field.type_index()),
                            visibility: field.visibility(),
                        }
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
            TypeSignature::Path { path, generics } => {
                self.check_type_signature_path(names, ty, path, generics);
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

    #[allow(clippy::too_many_lines)]
    fn check_type_signature_path(
        &mut self,
        names: &Names,
        ty: &Spanned<TypeSignature>,
        path: &[Spanned<PathElement>],
        generics: &[Spanned<TypeSignature>],
    ) {
        let checked_generics = generics.iter().fold(vec![], |mut accum, generic| {
            self.check_type_signature(names, generic);

            accum.push(self[generic.span()]);

            accum
        });

        let name_span = path
            .last()
            .map(Spanned::span)
            .expect("paths are never empty");

        let type_index = self[names[name_span]];

        match &self[type_index] {
            Type::Integer(_) | Type::NegativeInteger(_) => {
                unreachable!("normal type signatures are never integer literals")
            }
            Type::Float(_) => {
                unreachable!("normal type signatures are never float literals")
            }
            Type::Primitive(_) if !checked_generics.is_empty() => {
                self.errors
                    .push(Spanned::new(Error::GenericsOnPrimitive, name_span));

                self.type_map.insert(ty.span(), self.type_error());
            }
            Type::Generic(_) | Type::Existential(_) if !checked_generics.is_empty() => {
                self.errors
                    .push(Spanned::new(Error::GenericsOnGeneric, name_span));

                self.type_map.insert(ty.span(), self.type_error());
            }
            Type::Error => unreachable!("normal type signatures are never Type::Unknown"),
            Type::Primitive(_) | Type::Generic(_) | Type::Existential(_) => {
                self.type_map.insert(ty.span(), type_index);
            }
            Type::Fn { .. } => unreachable!("normal type signatures are never functions"),
            Type::AnyOf { .. } => unreachable!("normal type signatures are never Type::AnyOf"),
            Type::Associations(_) => {
                unreachable!("normal type signatures are never Type::Associations")
            }
            Type::Product {
                name,
                fields,
                generics: generic_types,
                ..
            } => {
                if generic_types.is_empty() && !checked_generics.is_empty() {
                    self.errors
                        .push(Spanned::new(Error::ExpectedZeroGenerics, ty.span()));

                    self.type_map.insert(ty.span(), self.type_error());
                } else if checked_generics.len() != generic_types.len() {
                    self.errors.push(Spanned::new(
                        Error::GenericCountMismatch {
                            expected: generic_types.len(),
                            got: checked_generics.len(),
                        },
                        ty.span(),
                    ));

                    self.type_map.insert(ty.span(), self.type_error());
                } else {
                    let name = name.clone();

                    let fields = fields.clone();
                    let generic_types = generic_types.clone();

                    let fields = fields.iter().fold(vec![], |mut accum, field| {
                        let type_index = self.substitute_type(
                            generic_types.as_slice(),
                            checked_generics.as_slice(),
                            field.type_index(),
                        );

                        accum.push(Field {
                            name: field.name().to_string(),
                            ty: type_index,
                            visibility: field.visibility,
                        });

                        accum
                    });

                    let type_index = self.push_type(Type::Product {
                        name,
                        fields,
                        generics: checked_generics,
                    });

                    self.type_map.insert(ty.span(), type_index);
                }
            }
            Type::Sum {
                name,
                variants,
                generics: generic_types,
                ..
            } => {
                if generic_types.is_empty() && !checked_generics.is_empty() {
                    self.errors
                        .push(Spanned::new(Error::ExpectedZeroGenerics, ty.span()));

                    self.type_map.insert(ty.span(), self.type_error());
                } else if checked_generics.len() != generic_types.len() {
                    self.errors.push(Spanned::new(
                        Error::GenericCountMismatch {
                            expected: generic_types.len(),
                            got: checked_generics.len(),
                        },
                        ty.span(),
                    ));

                    self.type_map.insert(ty.span(), self.type_error());
                } else {
                    let name = name.clone();

                    let variants = variants.clone();
                    let generic_types = generic_types.clone();

                    let variants = variants.iter().fold(vec![], |mut accum, variant_type| {
                        let type_index = self.substitute_type(
                            generic_types.as_slice(),
                            checked_generics.as_slice(),
                            *variant_type,
                        );

                        accum.push(type_index);

                        accum
                    });

                    let type_index = self.push_type(Type::Sum {
                        name,
                        variants,
                        generics: checked_generics,
                    });

                    self.type_map.insert(ty.span(), type_index);
                }
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
            if parameters.len() > usize::from(u16::MAX) {
                self.errors.push(Spanned::new(
                    Error::TooManyParameters(parameters.len()),
                    name.span(),
                ));
            }

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
    fn type_check_functions(
        &mut self,
        sources: &[(usize, (&str, &str))],
        ast: &Ast,
        names: &Names,
        items: &[ItemIndex],
        context: &[(String, TypeIndex)],
    ) {
        for item in items {
            match ast[*item].kind() {
                Item::Primitive(_)
                | Item::NativeFn { .. }
                | Item::Product { .. }
                | Item::Sum { .. }
                | Item::Directive { .. }
                | Item::Alias { .. } => {}
                Item::Mod {
                    name,
                    contents,
                    generics,
                    ..
                } => {
                    let mut context = context.to_vec();

                    let context = &mut context;

                    for (generic_name, generic) in generics.iter().map(|generic| {
                        (
                            generic.kind().clone(),
                            self.push_type(Type::Generic(generic.clone())),
                        )
                    }) {
                        context.push((generic_name, generic));
                    }

                    self.current_module.push(
                        names
                            .get_definition(name.span())
                            .expect("all modules are found by name resolution")
                            .module()
                            .is(),
                    );

                    self.type_check_functions(sources, ast, names, contents.as_slice(), context);

                    self.current_module.pop();
                }
                Item::Teach { body, generics, .. } => {
                    let mut context = context.to_vec();

                    let context = &mut context;

                    for (generic_name, generic) in generics.iter().map(|generic| {
                        (
                            generic.kind().clone(),
                            self.push_type(Type::Generic(generic.clone())),
                        )
                    }) {
                        context.push((generic_name, generic));
                    }

                    self.type_check_functions(sources, ast, names, body.as_slice(), context);
                }
                Item::Fn {
                    name,
                    body,
                    generics,
                    ..
                } => {
                    let mut context = context.to_vec();

                    let context = &mut context;

                    for (generic_name, generic) in generics.iter().map(|generic| {
                        (
                            generic.kind().clone(),
                            self.push_type(Type::Generic(generic.clone())),
                        )
                    }) {
                        context.push((generic_name, generic));
                    }

                    let expected_return_type =
                        if let Type::Fn { return_type, .. } = &self[self[name.span()]] {
                            Some(*return_type)
                        } else {
                            None
                        }
                        .map(|return_type| self.apply(return_type, context))
                        .expect("all functions have a return type");

                    let fn_return_type = self.fn_return_type;

                    self.fn_return_type = Some(expected_return_type);

                    if let Err(error) =
                        self.check(sources, ast, names, *body, expected_return_type, context)
                    {
                        self.errors
                            .push(Spanned::new(error, Self::expr_span(ast, *body)));
                    } else {
                        self.propagate(ast, names, *body, Some(self[ast[*body].span()]));
                    }

                    self.fn_return_type = fn_return_type;
                }
            }
        }
    }

    fn type_is_unknown(&self, span: Span) -> bool {
        matches!(
            self.get_type(span),
            None | Some(
                Type::Integer(_) | Type::NegativeInteger(_) | Type::Float(_) | Type::AnyOf { .. }
            )
        )
    }

    fn should_propagate(&self, ast: &Ast, names: &Names, expr: ExprIndex) -> bool {
        self.type_is_unknown(
            names
                .get(ast[expr].span())
                .unwrap_or_else(|| ast[expr].span()),
        )
    }

    fn propagate_conditionally_overwrite(
        &mut self,
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        type_index: Option<TypeIndex>,
    ) {
        if self.should_propagate(ast, names, expr)
            && let Some(type_index) = type_index
            && let Type::Primitive(primitive) = &self[type_index]
        {
            match primitive.kind() {
                Primitive::U8
                | Primitive::I8
                | Primitive::U16
                | Primitive::I16
                | Primitive::U32
                | Primitive::I32
                | Primitive::U64
                | Primitive::I64
                    if matches!(
                        self.get_type(ast[expr].span()),
                        Some(Type::Integer(_) | Type::NegativeInteger(_))
                    ) =>
                {
                    self.overwrite_with(ast, names, expr, type_index);
                }
                Primitive::F32 | Primitive::F64
                    if matches!(self.get_type(ast[expr].span()), Some(Type::Float(_))) =>
                {
                    self.overwrite_with(ast, names, expr, type_index);
                }
                Primitive::Boolean
                | Primitive::Unit
                | Primitive::String
                | Primitive::Never
                | Primitive::U8
                | Primitive::I8
                | Primitive::U16
                | Primitive::I16
                | Primitive::U32
                | Primitive::I32
                | Primitive::U64
                | Primitive::I64
                | Primitive::F32
                | Primitive::F64 => {}
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn propagate(
        &mut self,
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        type_index: Option<TypeIndex>,
    ) {
        let span = ast[expr].span();

        if self
            .get_type(span)
            .is_some_and(|ty| !matches!(ty, Type::Error))
        {
            match ast[expr].kind() {
                Expr::BinaryNoLhs { .. }
                | Expr::CallNoCallee(_)
                | Expr::AsUnitNoValue
                | Expr::ProductNoName(_) => {
                    unreachable!("these won't exist since parsing succeeded");
                }
                Expr::PathElement(_)
                | Expr::Boolean(_)
                | Expr::Unit
                | Expr::Return(_)
                | Expr::String(_)
                | Expr::Never => {}
                Expr::Name(_) | Expr::Integer(_) | Expr::NegativeInteger(_) | Expr::Float => {
                    self.propagate_conditionally_overwrite(ast, names, expr, type_index);
                }
                Expr::Group(expr) => {
                    self.propagate(ast, names, *expr, type_index);
                }
                Expr::Unary { op, expr: operand } => {
                    if matches!(op, UnaryOp::Negate) {
                        self.propagate_conditionally_overwrite(ast, names, expr, type_index);
                    }

                    self.propagate(ast, names, *operand, type_index);
                }
                Expr::Binary { op, lhs, rhs } => match op {
                    BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Remainder
                    | BinaryOp::Add
                    | BinaryOp::Subtract => {
                        self.propagate_conditionally_overwrite(ast, names, expr, type_index);

                        self.propagate(ast, names, *lhs, Some(self[span]));
                        self.propagate(ast, names, *rhs, Some(self[span]));
                    }
                    BinaryOp::PathAccess
                    | BinaryOp::Access
                    | BinaryOp::Less
                    | BinaryOp::Greater
                    | BinaryOp::LessOrEqual
                    | BinaryOp::GreaterOrEqual
                    | BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Assign => {
                        if self.get_type(ast[*lhs].span()).is_some() {
                            self.propagate(ast, names, *lhs, Some(self[ast[*lhs].span()]));
                        }

                        if self.get_type(ast[*rhs].span()).is_some() {
                            self.propagate(ast, names, *rhs, Some(self[ast[*rhs].span()]));
                        }
                    }
                },
                Expr::Block(exprs) if exprs.is_empty() => {}
                Expr::Block(exprs) => {
                    for expr in exprs.iter().rev().skip(1).rev() {
                        self.propagate(ast, names, *expr, None);
                    }

                    self.propagate(
                        ast,
                        names,
                        exprs
                            .last()
                            .copied()
                            .expect("the above match arm guarantees this block isn't empty"),
                        type_index,
                    );
                }
                Expr::If {
                    condition,
                    when_true,
                    otherwise,
                } => {
                    self.propagate(ast, names, *condition, Some(self.type_boolean()));
                    self.propagate(ast, names, *when_true, type_index);
                    self.propagate(ast, names, *otherwise, type_index);
                }
                Expr::While {
                    condition,
                    when_true,
                } => {
                    self.propagate(ast, names, *condition, Some(self.type_boolean()));
                    self.propagate(ast, names, *when_true, None);
                }
                Expr::MethodCall {
                    target,
                    method,
                    arguments,
                } => {
                    self.propagate(ast, names, *target, None);
                    self.propagate(ast, names, *method, None);

                    for argument in arguments {
                        self.propagate(ast, names, *argument, None);
                    }
                }
                Expr::AsUnit(expr) => {
                    self.propagate(ast, names, *expr, None);
                }
                Expr::Call { callee, arguments } => {
                    self.propagate(ast, names, *callee, None);

                    for argument in arguments {
                        self.propagate(ast, names, *argument, None);
                    }
                }
                Expr::Product { fields, .. } => {
                    for (_, field) in fields {
                        self.propagate(ast, names, *field, None);
                    }
                }
                Expr::Match {
                    expr,
                    cases,
                    fallback,
                } => {
                    self.propagate(ast, names, *expr, None);

                    for case in cases {
                        self.propagate(ast, names, case.case(), type_index);
                    }

                    self.propagate(ast, names, *fallback, type_index);
                }
                Expr::Let { name, value, .. } => {
                    self.propagate(ast, names, *value, Some(self[name.span()]));
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn replace_default_numeric_types(&mut self, sources: &[(usize, (&str, &str))]) {
        let type_integer = self.type_integer();
        let type_negative_integer = self.type_negative_integer();
        let type_float = self.type_float();

        let default_integer = self[self.type_i64()].clone();
        let default_integer_unsigned = self[self.type_i64()].clone();
        let default_float = self[self.type_f64()].clone();

        for (i, ty) in self.types.iter_mut().enumerate() {
            if TypeIndex(i) != type_integer
                && TypeIndex(i) != type_negative_integer
                && TypeIndex(i) != type_float
            {
                match ty {
                    Type::Integer(radix) => {
                        let (radix_span, radix) = (radix.span(), *radix.kind());

                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == radix_span.source_id())
                            .and_then(|(_, (_, source))| radix_span.lexeme(source))
                            .expect("the span is from this source");

                        match i64::from_str_radix(lexeme, radix) {
                            Ok(_) => {
                                *ty = default_integer.clone();
                            }
                            Err(_) => match u64::from_str_radix(lexeme, radix) {
                                Ok(_) => {
                                    *ty = default_integer_unsigned.clone();
                                }
                                Err(_) => {
                                    self.errors
                                        .push(Spanned::new(Error::InvalidInteger, radix_span));
                                }
                            },
                        }
                    }
                    Type::NegativeInteger(radix) => {
                        let (radix_span, radix) = (radix.span(), *radix.kind());

                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == radix_span.source_id())
                            .and_then(|(_, (_, source))| radix_span.lexeme(source))
                            .expect("the span is from this source");

                        match i64::from_str_radix(lexeme, radix) {
                            Ok(_) => {
                                *ty = default_integer.clone();
                            }
                            Err(_) => {
                                self.errors
                                    .push(Spanned::new(Error::InvalidInteger, radix_span));
                            }
                        }
                    }
                    Type::Float(span) => {
                        let lexeme = sources
                            .iter()
                            .find(|(source_id, _)| *source_id == span.source_id())
                            .and_then(|(_, (_, source))| span.lexeme(source))
                            .expect("the span is from this source");

                        match lexeme.parse::<f64>() {
                            Ok(_) => {
                                *ty = default_float.clone();
                            }
                            Err(_) => match lexeme.parse::<f32>() {
                                Ok(_) => {
                                    *ty = default_float.clone();
                                }
                                Err(_) => {
                                    self.errors.push(Spanned::new(Error::InvalidFloat, *span));
                                }
                            },
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infer(
        &mut self,
        sources: &[(usize, (&str, &str))],
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        context: &[(String, TypeIndex)],
    ) -> TypeIndex {
        let span = ast[expr].span();

        let type_index = match ast[expr].kind() {
            Expr::BinaryNoLhs { .. }
            | Expr::CallNoCallee(_)
            | Expr::AsUnitNoValue
            | Expr::ProductNoName(_) => {
                unreachable!("these won't exist since parsing succeeded");
            }
            Expr::PathElement(PathElement::SelfType) => {
                self.errors.push(Spanned::new(Error::TypeAsExpr, span));

                self.type_error()
            }
            Expr::PathElement(_) => {
                self.errors.push(Spanned::new(Error::ModuleAsExpr, span));

                self.type_error()
            }
            Expr::Integer(radix) => {
                if self.get_type(span).is_some() {
                    self[span]
                } else {
                    self.push_type(Type::Integer(Spanned::new(*radix, span)))
                }
            }
            Expr::NegativeInteger(radix) => {
                if self.get_type(span).is_some() {
                    self[span]
                } else {
                    self.push_type(Type::NegativeInteger(Spanned::new(*radix, span)))
                }
            }
            Expr::Float => {
                if self.get_type(span).is_some() {
                    self[span]
                } else {
                    self.push_type(Type::Float(span))
                }
            }
            Expr::Boolean(_) => self.type_boolean(),
            Expr::Unit => self.type_unit(),
            Expr::Never => self.type_never(),
            Expr::Name(_) => {
                let mut context = context.to_vec();

                let context = &mut context;

                let type_index = self.apply(
                    // it should be okay to fall back to an error type
                    // name resolution would have failed if the name wasn't real
                    if self.get_type(names[span]).is_some() {
                        self[names[span]]
                    } else {
                        self.type_error()
                    },
                    context,
                );

                self.overwrite_with(ast, names, expr, type_index);

                type_index
            }
            Expr::Unary { op, expr: operand } => {
                let mut context = context.to_vec();

                let context = &mut context;

                self.infer_unary(sources, ast, names, (*op, *operand), context)
            }
            Expr::Binary { op, lhs, rhs } => {
                let mut context = context.to_vec();

                let context = &mut context;

                self.infer_binary(sources, ast, names, (*op, *lhs, *rhs), context)
            }
            Expr::Group(expr) => self.infer(sources, ast, names, *expr, context),
            Expr::Block(exprs) if exprs.is_empty() => self.type_unit(),
            Expr::Block(exprs) => {
                for expr in exprs.iter().take(exprs.len() - 1) {
                    self.infer(sources, ast, names, *expr, context);
                }

                self.infer(
                    sources,
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
                let mut context = context.to_vec();

                let context = &mut context;

                if self
                    .check(
                        sources,
                        ast,
                        names,
                        *condition,
                        self.type_boolean(),
                        context,
                    )
                    .is_err()
                {
                    self.errors.push(Spanned::new(
                        Error::ConditionNotBoolean,
                        Self::expr_span(ast, *condition),
                    ));
                }

                let otherwise_type = self.infer(sources, ast, names, *otherwise, context);

                match self.check(sources, ast, names, *when_true, otherwise_type, context) {
                    Ok(when_true_type) => self.push_type(Type::AnyOf {
                        alternatives: vec![when_true_type, otherwise_type],
                        span,
                    }),
                    Err(error) => {
                        self.errors
                            .push(Spanned::new(error, Self::expr_span(ast, *otherwise)));

                        self.type_error()
                    }
                }
            }
            Expr::While {
                condition,
                when_true,
            } => {
                let mut context = context.to_vec();

                let context = &mut context;

                if self
                    .check(
                        sources,
                        ast,
                        names,
                        *condition,
                        self.type_boolean(),
                        context,
                    )
                    .is_err()
                {
                    self.errors.push(Spanned::new(
                        Error::ConditionNotBoolean,
                        Self::expr_span(ast, *condition),
                    ));
                }

                self.infer(sources, ast, names, *when_true, context);

                self.type_unit()
            }
            Expr::Let {
                name,
                type_signature,
                value,
            } => {
                let type_index = if let Some(annotation) = type_signature {
                    self.check_type_signature(names, annotation);

                    let mut context = context.to_vec();

                    let context = &mut context;

                    let annotation_ty = self[annotation.span()];

                    let inferred = self.infer(sources, ast, names, *value, context);

                    match self.check_inferred(sources, inferred, annotation_ty, context) {
                        Err(errored) => {
                            self.errors.push(Spanned::new(
                                self.type_mismatch_error(errored, annotation_ty),
                                Self::expr_span(ast, *value),
                            ));

                            self.type_error()
                        }
                        Ok(checked) => {
                            self.type_map.insert(ast[*value].span(), annotation_ty);

                            if matches!(
                                self[inferred],
                                Type::Integer(_) | Type::NegativeInteger(_) | Type::Float(_)
                            ) {
                                checked
                            } else {
                                inferred
                            }
                        }
                    }
                } else {
                    self.infer(sources, ast, names, *value, context)
                };

                self.type_map.insert(name.span(), type_index);

                self.type_unit()
            }
            Expr::Call { callee, arguments } => {
                let callee_type_index = self.infer(sources, ast, names, *callee, context);

                match self.infer_call(
                    sources,
                    ast,
                    names,
                    (ast[*callee].span(), callee_type_index, arguments.as_slice()),
                    context,
                ) {
                    Ok(type_index) => type_index,
                    Err(error) => {
                        if !matches!(self[self[ast[*callee].span()]], Type::Error) {
                            self.errors.push(error);
                        }

                        self.type_error()
                    }
                }
            }
            Expr::MethodCall {
                target,
                method,
                arguments,
            } => {
                let target_type_index = self.infer(sources, ast, names, *target, context);

                let type_span = self.get_type_span_or_error(target_type_index);

                if let Some(Err(error)) = type_span {
                    return error;
                }

                let type_span = type_span
                    .and_then(Result::ok)
                    .map(|type_span| names.get(type_span).unwrap_or(type_span));

                if let Some(type_span) = type_span
                    && let Expr::Name(name) = ast[*method].kind()
                {
                    let mut context = context.to_vec();

                    let context = &mut context;

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
                                    sources,
                                    self[type_span],
                                    associated_name.for_type,
                                    context,
                                )
                                .ok()
                                .map(|_| Association {
                                    resolved_index: index,
                                    type_index: self[associated_name.definition.span()],
                                    span: associated_name.definition.span(),
                                })
                            })
                            .collect::<Vec<_>>();

                        let type_index = self.push_type(Type::Associations(valid_associations));

                        self.type_map.insert(ast[*method].span(), type_index);
                    } else if let Type::Product { fields, .. } = &self[target_type_index]
                        && let Some(Type::Fn { .. }) = fields.iter().find_map(|field| {
                            if field.name() == name {
                                Some(&self[field.type_index()])
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

                        return self.type_error();
                    } else {
                        self.errors
                            .push(Spanned::new(Error::NonExistentMethod, ast[*method].span()));

                        return self.type_error();
                    }

                    let mut arguments_with_target = vec![*target];

                    arguments_with_target.extend_from_slice(arguments.as_slice());

                    match self.infer_call(
                        sources,
                        ast,
                        names,
                        (
                            ast[*method].span(),
                            self[ast[*method].span()],
                            arguments_with_target.as_slice(),
                        ),
                        context,
                    ) {
                        Ok(type_index) => type_index,
                        Err(error) => {
                            if !matches!(self[self[ast[*method].span()]], Type::Error) {
                                self.errors.push(error.transmute(|error| match error {
                                    Error::CallArgumentCountMismatch { expected, got } => {
                                        Error::MethodCallArgumentCountMismatch { expected, got }
                                    }
                                    Error::CalledUncallable => Error::MethodCalledUncallable,
                                    Error::PrivateFunction => Error::PrivateMethod,
                                    _ => error,
                                }));
                            }

                            self.type_error()
                        }
                    }
                } else {
                    self.errors.push(Spanned::new(
                        Error::CannotHaveMethod,
                        Self::expr_span(ast, *target),
                    ));

                    self.type_error()
                }
            }
            Expr::Return(expr) => {
                let mut context = context.to_vec();

                let context = &mut context;

                if let Err(error) = self.check(
                    sources,
                    ast,
                    names,
                    *expr,
                    self.fn_return_type
                        .expect("returns can only happen in functions"),
                    context,
                ) {
                    self.errors
                        .push(Spanned::new(error, Self::expr_span(ast, *expr)));
                }

                self.type_never()
            }
            Expr::AsUnit(expr) => {
                self.infer(sources, ast, names, *expr, context);

                if let Expr::Return(_) = ast[*expr].kind() {
                    self.type_never()
                } else {
                    self.type_unit()
                }
            }
            Expr::Product { name, fields } => {
                if let Type::Product {
                    name: type_name,
                    fields: field_types,
                    generics,
                } = self[self[names[name.span()]]].clone()
                {
                    let error_count = self.errors.len();

                    let mut context = context.to_vec();

                    let context = &mut context;

                    let mut checked_fields = vec![];

                    for (field_name, field) in fields {
                        match self.field_type(
                            names,
                            &type_name,
                            field_types.as_slice(),
                            (field_name.kind(), field_name.span()),
                        ) {
                            Err(error) => {
                                self.errors.push(error);
                            }
                            Ok(field_type) => {
                                match self.check(sources, ast, names, *field, field_type, context) {
                                    Ok(ty) => {
                                        checked_fields.push(Field {
                                            name: field_name.kind().clone(),
                                            ty,
                                            visibility: field_types.iter().find_map(|field| {
                                                if field.name() == field_name.kind() {
                                                    Some(field.visibility())
                                                } else {
                                                    None
                                                }
                                            }).expect("the field is guaranteed to exist because of the outer match"),
                                        });
                                    }
                                    Err(error) => {
                                        self.errors.push(Spanned::new(
                                            error,
                                            Self::expr_span(ast, *field),
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    checked_fields.sort_by(|a_field, b_field| a_field.name().cmp(b_field.name()));

                    for field in field_types {
                        if !fields.iter().any(|(name, _)| name.kind() == field.name()) {
                            self.errors.push(Spanned::new(
                                Error::ProductMissingField(field.name().to_string()),
                                span,
                            ));
                        }
                    }

                    if self.errors.len() == error_count {
                        let generics = generics
                            .into_iter()
                            .map(|generic| self.apply(generic, context))
                            .collect::<Vec<_>>();

                        self.push_type(Type::Product {
                            name: type_name,
                            fields: checked_fields,
                            generics,
                        })
                    } else {
                        self.type_error()
                    }
                } else {
                    if !matches!(self[self[names[name.span()]]], Type::Error) {
                        self.errors
                            .push(Spanned::new(Error::UnknownProduct, name.span()));
                    }

                    self.type_error()
                }
            }
            Expr::Match {
                cases,
                expr,
                fallback,
            } => {
                let expr_type_index = self.infer(sources, ast, names, *expr, context);

                let mut match_type_index = vec![];

                for case in cases {
                    {
                        let mut context = context.to_vec();

                        let context = &mut context;

                        if let Err(type_index) = self.check_pattern(
                            sources,
                            names,
                            case.pattern(),
                            expr_type_index,
                            context,
                        ) && !matches!(self[type_index], Type::Error)
                        {
                            let error = self.type_mismatch_error(type_index, expr_type_index);

                            self.errors.push(Spanned::new(error, case.pattern().span()));
                        }
                    }

                    {
                        let mut context = context.to_vec();

                        let context = &mut context;

                        let case_type_index = self.infer(sources, ast, names, case.case(), context);

                        for other_case_type_index in match_type_index.clone() {
                            match self.check_inferred(
                                sources,
                                case_type_index,
                                other_case_type_index,
                                context,
                            ) {
                                Ok(type_index) => {
                                    match_type_index.push(type_index);
                                }
                                Err(type_index) => {
                                    if !matches!(self[type_index], Type::Error) {
                                        let error = Spanned::new(
                                            self.type_mismatch_error(
                                                type_index,
                                                other_case_type_index,
                                            ),
                                            Self::expr_span(ast, case.case()),
                                        );

                                        self.errors.push(error);

                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                let mut context = context.to_vec();

                let context = &mut context;

                let fallback_type_index = self.infer(sources, ast, names, *fallback, context);

                for other_case_type_index in match_type_index.clone() {
                    match self.check_inferred(
                        sources,
                        fallback_type_index,
                        other_case_type_index,
                        context,
                    ) {
                        Ok(type_index) => {
                            match_type_index.push(type_index);
                        }
                        Err(type_index) => {
                            if !matches!(self[type_index], Type::Error) {
                                let error = Spanned::new(
                                    self.type_mismatch_error(type_index, other_case_type_index),
                                    Self::expr_span(ast, *fallback),
                                );

                                self.errors.push(error);

                                break;
                            }
                        }
                    }
                }

                self.push_type(Type::AnyOf {
                    alternatives: match_type_index,
                    span,
                })
            }
            Expr::String(_) => self.type_string(),
        };

        self.type_map.insert(span, type_index);

        type_index
    }

    fn get_type_span_or_error(&mut self, type_index: TypeIndex) -> Option<Result<Span, TypeIndex>> {
        match &self[type_index] {
            Type::Integer(_) | Type::NegativeInteger(_) | Type::Float(_) => {
                Some(Err(self.type_error()))
            }
            Type::Error => Some(Err(self.type_error())),
            Type::Existential(_) | Type::Generic(_) | Type::Fn { .. } | Type::Associations(_) => {
                None
            }
            Type::AnyOf { alternatives, .. } => self
                .get_type_span_or_error(*alternatives.first().expect("Type::AnyOf is never empty")),
            Type::Primitive(primitive) => Some(Ok(primitive.span())),
            Type::Product { name, .. } | Type::Sum { name, .. } => Some(Ok(name.span())),
        }
    }

    fn infer_call(
        &mut self,
        sources: &[(usize, (&str, &str))],
        ast: &Ast,
        names: &Names,
        (callee_span, callee_type_index, arguments): (Span, TypeIndex, &[ExprIndex]),
        context: &[(String, TypeIndex)],
    ) -> Result<TypeIndex, Spanned<Error>> {
        if arguments.len() > usize::from(u16::MAX) {
            return Err(Spanned::new(
                Error::TooManyArguments(arguments.len()),
                callee_span,
            ));
        }

        let functions = match &self[callee_type_index] {
            function @ Type::Fn { .. } => {
                vec![(0, function.clone(), callee_span)]
            }
            Type::Associations(alternatives) => alternatives
                .iter()
                .filter_map(|association| match &self[association.type_index] {
                    Type::Associations(_) => {
                        unreachable!("Type::Associations should never be nested");
                    }
                    Type::Fn { .. } => Some((
                        association.resolved_index,
                        self[association.type_index].clone(),
                        association.span,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => {
                return Err(Spanned::new(Error::CalledUncallable, callee_span));
            }
        };

        let functions = functions
            .into_iter()
            .map(|(index, function, span)| {
                if let Type::Fn {
                    parameters,
                    return_type,
                } = function
                {
                    if arguments.len() == parameters.len() {
                        let mut context = context.to_vec();

                        let context = &mut context;

                        for (argument, parameter) in arguments.iter().zip(&parameters) {
                            if let Err(error) =
                                self.check(sources, ast, names, *argument, *parameter, context)
                            {
                                return Err(Spanned::new(error, Self::expr_span(ast, *argument)));
                            }
                        }

                        Ok((index, self.apply(return_type, context), span))
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
            let (index, type_index, span) = functions
                .into_iter()
                .find_map(|function| {
                    if function.is_ok() {
                        function.ok()
                    } else {
                        None
                    }
                })
                .expect("it's going to be okay");

            // TODO: better unify name resolution associations and type checking associations
            // type checking currently expects anything in `associated_with` to have a type
            // this means modules can't go in there, and a callee can be a path access of a module function
            if names.get_definition(span).is_none_or(|definition| {
                definition.module().is()
                    == self
                        .current_module
                        .last()
                        .map_or(ROOT_MODULE.is(), |module_id| *module_id)
                    || matches!(definition.visibility(), Visibility::Public)
            }) {
                self.resolved_associations.insert(callee_span, index);

                Ok(type_index)
            } else {
                Err(Spanned::new(Error::PrivateFunction, callee_span))
            }
        }
    }

    fn infer_unary(
        &mut self,
        sources: &[(usize, (&str, &str))],
        ast: &Ast,
        names: &Names,
        (op, operand): (UnaryOp, ExprIndex),
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match op {
            UnaryOp::Not => {
                self.infer(sources, ast, names, operand, context);

                if let Err(error) =
                    self.check(sources, ast, names, operand, self.type_boolean(), context)
                {
                    self.errors
                        .push(Spanned::new(error, Self::expr_span(ast, operand)));

                    self.type_error()
                } else {
                    self.type_boolean()
                }
            }
            UnaryOp::Negate => {
                self.infer(sources, ast, names, operand, context);

                match self
                    .check(sources, ast, names, operand, self.type_float(), context)
                    .or_else(|_| {
                        self.check(
                            sources,
                            ast,
                            names,
                            operand,
                            self.type_negative_integer(),
                            context,
                        )
                    }) {
                    Err(_) => {
                        self.errors.push(Spanned::new(
                            Error::CannotNegate(
                                self[self[ast[operand].span()]].to_string(self.types.as_slice()),
                            ),
                            Self::expr_span(ast, operand),
                        ));

                        self.type_error()
                    }
                    Ok(type_index) => type_index,
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn infer_binary(
        &mut self,
        sources: &[(usize, (&str, &str))],
        ast: &Ast,
        names: &Names,
        (op, lhs, rhs): (BinaryOp, ExprIndex, ExprIndex),
        context: &mut Vec<(String, TypeIndex)>,
    ) -> TypeIndex {
        match op {
            BinaryOp::PathAccess => {
                let lhs = match ast[lhs].kind() {
                    Expr::Name(_) | Expr::PathElement(_) => lhs,
                    Expr::Binary {
                        op: BinaryOp::PathAccess,
                        rhs,
                        ..
                    } => match ast[*rhs].kind() {
                        Expr::Name(_) | Expr::PathElement(_) => *rhs,
                        _ => {
                            unreachable!(
                                "name resolution verifies the path is correct {:?}",
                                ast[*rhs].kind()
                            );
                        }
                    },
                    _ => {
                        unreachable!(
                            "name resolution verifies the path is correct {:?}",
                            ast[lhs].kind()
                        );
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
                            self.check_inferred(
                                sources,
                                self[type_span],
                                associated_name.for_type,
                                context,
                            )
                            .ok()
                            .map(|_| Association {
                                resolved_index: index,
                                type_index: self[associated_name.definition.span()],
                                span: associated_name.definition.span(),
                            })
                        })
                        .collect::<Vec<_>>();

                    self.push_type(Type::Associations(valid_associations))
                } else if let Some(Type::Sum { variants, .. }) = self.get_type(names[lhs_span]) {
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

                            let context = &mut context;

                            match self.check(sources, ast, names, rhs, variant_type, context) {
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
                                            .map(|generic| self.apply(generic, context))
                                            .collect::<Vec<_>>(),
                                    };

                                    self.push_type(ty)
                                }
                                Err(error) => {
                                    self.errors
                                        .push(Spanned::new(error, Self::expr_span(ast, rhs)));

                                    self.type_error()
                                }
                            }
                        } else {
                            self.errors.push(Spanned::new(
                                Error::NonExistentSumVariant,
                                Self::expr_span(ast, rhs),
                            ));

                            self.type_error()
                        }
                    } else {
                        self.errors.push(Spanned::new(
                            Error::InvalidSumVariant,
                            Self::expr_span(ast, rhs),
                        ));

                        self.type_error()
                    }
                } else {
                    self.infer(sources, ast, names, rhs, context)
                }
            }
            BinaryOp::Access => {
                let lhs_type = self.infer(sources, ast, names, lhs, context);

                if let Type::Product {
                    fields,
                    name: type_name,
                    ..
                } = &self[lhs_type]
                {
                    if let Expr::Name(name) = ast[rhs].kind() {
                        match self.field_type(
                            names,
                            type_name,
                            fields.as_slice(),
                            (name, ast[rhs].span()),
                        ) {
                            Ok(type_index) => type_index,
                            Err(error) => {
                                self.errors.push(error);

                                self.type_error()
                            }
                        }
                    } else {
                        self.errors.push(Spanned::new(
                            Error::InvalidAccess,
                            Self::expr_span(ast, rhs),
                        ));

                        self.type_error()
                    }
                } else {
                    if !matches!(self[lhs_type], Type::Error) {
                        self.errors
                            .push(Spanned::new(Error::CannotAccess, Self::expr_span(ast, lhs)));
                    }

                    self.type_error()
                }
            }
            BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::Add
            | BinaryOp::Subtract => {
                match self
                    .check(sources, ast, names, lhs, self.type_float(), context)
                    .or_else(|_| self.check(sources, ast, names, lhs, self.type_integer(), context))
                {
                    Err(_) => {
                        self.errors.push(Spanned::new(
                            Error::ArithmeticImpossible(
                                self[self[ast[lhs].span()]].to_string(self.types.as_slice()),
                            ),
                            Self::expr_span(ast, lhs),
                        ));

                        self.type_error()
                    }
                    Ok(lhs_type) => match self.check(sources, ast, names, rhs, lhs_type, context) {
                        Err(error) => {
                            self.errors
                                .push(Spanned::new(error, Self::expr_span(ast, rhs)));

                            self.type_error()
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
                    .check(sources, ast, names, lhs, self.type_float(), context)
                    .or_else(|_| self.check(sources, ast, names, lhs, self.type_integer(), context))
                {
                    Err(_) => {
                        self.errors.push(Spanned::new(
                            Error::CannotCompare(
                                self[self[ast[lhs].span()]].to_string(self.types.as_slice()),
                            ),
                            Self::expr_span(ast, lhs),
                        ));

                        self.type_error()
                    }
                    Ok(lhs_type) => match self.check(sources, ast, names, rhs, lhs_type, context) {
                        Err(error) => {
                            self.errors
                                .push(Spanned::new(error, Self::expr_span(ast, rhs)));

                            self.type_error()
                        }
                        Ok(_) => self.type_boolean(),
                    },
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                match self.check(sources, ast, names, lhs, self.type_boolean(), context) {
                    Err(error) => {
                        self.errors
                            .push(Spanned::new(error, Self::expr_span(ast, lhs)));

                        self.type_error()
                    }
                    Ok(lhs_type) => match self.check(sources, ast, names, rhs, lhs_type, context) {
                        Err(error) => {
                            self.errors
                                .push(Spanned::new(error, Self::expr_span(ast, rhs)));

                            self.type_error()
                        }
                        Ok(_) => self.type_boolean(),
                    },
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let lhs_type = self.infer(sources, ast, names, lhs, context);

                match self.check(sources, ast, names, rhs, lhs_type, context) {
                    Err(error) => {
                        self.errors
                            .push(Spanned::new(error, Self::expr_span(ast, rhs)));

                        self.type_error()
                    }
                    Ok(_) => self.type_boolean(),
                }
            }
            BinaryOp::Assign => {
                let lhs_type = self.infer(sources, ast, names, lhs, context);

                match self.check(sources, ast, names, rhs, lhs_type, context) {
                    Err(error) => {
                        let span = ast[rhs].span();

                        self.errors.push(Spanned::new(error, span));

                        self.type_error()
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
                                    .check_inferred(sources, lhs_type, self.type_error(), context)
                                    .is_err()
                                {
                                    return self.type_error();
                                }

                                let lhs_span = ast[*lhs].span();

                                let Expr::Name(name) = ast[*rhs].kind() else {
                                    unreachable!("accessors are always names");
                                };

                                let mut context = context.clone();

                                let context = &mut context;

                                let Type::Product {
                                    fields,
                                    generics,
                                    name: type_name,
                                    ..
                                } = &self[self[names[lhs_span]]]
                                else {
                                    unreachable!("accessees are always products");
                                };

                                let mut applied_generics = generics.clone();

                                let field_type_index = match self.field_type(
                                    names,
                                    type_name,
                                    fields.as_slice(),
                                    (name, ast[*rhs].span()),
                                ) {
                                    Ok(type_index) => type_index,
                                    Err(error) => {
                                        self.errors.push(error);

                                        return self.type_error();
                                    }
                                };

                                match self.check_inferred(
                                    sources,
                                    field_type_index,
                                    rhs_type,
                                    context,
                                ) {
                                    Err(type_index) => {
                                        let error = Spanned::new(
                                            self.type_mismatch_error(type_index, rhs_type),
                                            rhs_span,
                                        );

                                        self.errors.push(error);

                                        return self.type_error();
                                    }
                                    Ok(type_index) => {
                                        for generic in &mut applied_generics {
                                            *generic = self.apply(*generic, context);
                                        }

                                        let product_type_index = self[names[lhs_span]];

                                        let Some(Type::Product {
                                            fields, generics, ..
                                        }) = self.types.get_mut(usize::from(product_type_index))
                                        else {
                                            unreachable!("accessees are always products");
                                        };

                                        let field_index = fields
                                            .iter()
                                            .position(|field| field.name() == name)
                                            .expect("if the field doesn't exist, we would have returned early");

                                        fields[field_index].ty = type_index;

                                        *generics = applied_generics;
                                    }
                                }
                            }
                            _ => {}
                        }

                        self.type_unit()
                    }
                }
            }
        }
    }

    fn field_type(
        &self,
        names: &Names,
        type_name: &Spanned<String>,
        fields: &[Field],
        (name, span): (&str, Span),
    ) -> Result<TypeIndex, Spanned<Error>> {
        if let Some((field_type, visibility)) = fields.iter().find_map(|field| {
            if field.name() == name {
                Some((field.type_index(), field.visibility()))
            } else {
                None
            }
        }) {
            if let Some(definition) = names.get_definition(type_name.span())
                && let Some(current_module_id) = self.current_module.last()
                && definition.module().is() == *current_module_id
            {
                Ok(field_type)
            } else if matches!(visibility, Visibility::Public) {
                Ok(field_type)
            } else {
                Err(Spanned::new(Error::PrivateField, span))
            }
        } else {
            Err(Spanned::new(Error::NonExistentField, span))
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

    fn expr_span(ast: &Ast, mut expr: ExprIndex) -> Span {
        while let Expr::Block(exprs) = ast[expr].kind()
            && !exprs.is_empty()
        {
            expr = exprs
                .last()
                .copied()
                .expect("the while condition guarantees there is at least one expression");
        }

        ast[expr].span()
    }

    fn check(
        &mut self,
        sources: &[(usize, (&str, &str))],
        ast: &Ast,
        names: &Names,
        expr: ExprIndex,
        should_be: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, Error> {
        let expr_type_index = self.infer(sources, ast, names, expr, context);

        self.check_inferred(sources, expr_type_index, should_be, context)
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
        sources: &[(usize, (&str, &str))],
        inferred: TypeIndex,
        should_be: TypeIndex,
        context: &mut Vec<(String, TypeIndex)>,
    ) -> Result<TypeIndex, TypeIndex> {
        match (&self[inferred], &self[should_be]) {
            (Type::Primitive(a), _) if matches!(a.kind(), Primitive::Never) => Ok(should_be),
            (Type::Error, _) | (_, Type::Error) => Ok(self.type_error()),
            (Type::Integer(a), Type::NegativeInteger(_)) => {
                let (a_radix, a_span) = (*a.kind(), a.span());

                let type_index =
                    self.push_type(Type::NegativeInteger(Spanned::new(a_radix, a_span)));

                self.type_map.insert(a_span, type_index);

                Ok(type_index)
            }
            (Type::Float(_), Type::Float(_))
            | (
                Type::Integer(_) | Type::NegativeInteger(_),
                Type::Integer(_) | Type::NegativeInteger(_),
            ) => Ok(inferred),
            (Type::Integer(radix), Type::Primitive(b)) => {
                let (radix_span, radix) = (radix.span(), *radix.kind());

                let lexeme = sources
                    .iter()
                    .find(|(source_id, _)| *source_id == radix_span.source_id())
                    .and_then(|(_, (_, source))| radix_span.lexeme(source))
                    .expect("the span is from this source");

                match b.kind() {
                    Primitive::U8 if u8::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I8 if i8::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::U16 if u16::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I16 if i16::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::U32 if u32::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I32 if i32::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::U64 if u64::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I64 if i64::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    _ => Err(inferred),
                }
                .inspect(|type_index| {
                    self.type_map.insert(radix_span, *type_index);
                })
            }
            (Type::Primitive(a), Type::Integer(_)) => match a.kind() {
                Primitive::U8
                | Primitive::I8
                | Primitive::U16
                | Primitive::I16
                | Primitive::U32
                | Primitive::I32
                | Primitive::U64
                | Primitive::I64 => Ok(inferred),
                _ => Err(inferred),
            },
            (Type::NegativeInteger(radix), Type::Primitive(b)) => {
                let (radix_span, radix) = (radix.span(), *radix.kind());

                let lexeme = sources
                    .iter()
                    .find(|(source_id, _)| *source_id == radix_span.source_id())
                    .and_then(|(_, (_, source))| radix_span.lexeme(source))
                    .expect("the span is from this source");

                match b.kind() {
                    Primitive::I8
                        if i8::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() =>
                    {
                        Ok(should_be)
                    }
                    Primitive::I16
                        if i16::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() =>
                    {
                        Ok(should_be)
                    }
                    Primitive::I32
                        if i32::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() =>
                    {
                        Ok(should_be)
                    }
                    Primitive::I64
                        if i64::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() =>
                    {
                        Ok(should_be)
                    }
                    _ => Err(inferred),
                }
                .inspect(|type_index| {
                    self.type_map.insert(radix_span, *type_index);
                })
            }
            (Type::Primitive(a), Type::NegativeInteger(_)) => match a.kind() {
                Primitive::I8 | Primitive::I16 | Primitive::I32 | Primitive::I64 => Ok(inferred),
                _ => Err(inferred),
            },
            (Type::Float(span), Type::Primitive(b)) => {
                let span = *span;

                let lexeme = sources
                    .iter()
                    .find(|(source_id, _)| *source_id == span.source_id())
                    .and_then(|(_, (_, source))| span.lexeme(source))
                    .expect("the span is from this source");

                match b.kind() {
                    Primitive::F32 if lexeme.parse::<f32>().is_ok() => Ok(should_be),
                    Primitive::F64 if lexeme.parse::<f64>().is_ok() => Ok(should_be),
                    _ => Err(inferred),
                }
                .inspect(|type_index| {
                    self.type_map.insert(span, *type_index);
                })
            }
            (Type::Primitive(a), Type::Float(_)) => match a.kind() {
                Primitive::F32 | Primitive::F64 => Ok(inferred),
                _ => Err(inferred),
            },
            (_, Type::AnyOf { alternatives, span }) => {
                let alternatives = alternatives.clone();
                let span = *span;

                for alternative in alternatives {
                    self.check_inferred(sources, alternative, inferred, context)?;
                }

                self.type_map.insert(span, inferred);

                Ok(inferred)
            }
            (Type::AnyOf { alternatives, span }, _) => {
                let alternatives = alternatives.clone();
                let span = *span;

                for alternative in alternatives {
                    self.check_inferred(sources, alternative, should_be, context)?;
                }

                self.type_map.insert(span, should_be);

                Ok(should_be)
            }
            (Type::Primitive(a), Type::Primitive(b)) => {
                if a.kind() == b.kind() {
                    Ok(should_be)
                } else {
                    Err(inferred)
                }
            }
            (Type::Generic(a), Type::Generic(b)) => {
                if a.kind() == b.kind() && a.span() == b.span() {
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
                    match self.check_inferred(sources, inferred_parameter, *parameter, context) {
                        Ok(type_index) => {
                            *parameter = type_index;
                        }
                        Err(type_index) => {
                            *parameter = type_index;
                            errored = true;
                        }
                    }
                }

                match self.check_inferred(sources, inferred_return_type, return_type, context) {
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

                for (inferred_field, field) in inferred_fields.into_iter().zip(&mut fields) {
                    match self.check_inferred(
                        sources,
                        inferred_field.type_index(),
                        field.type_index(),
                        context,
                    ) {
                        Ok(type_index) => {
                            field.ty = type_index;
                        }
                        Err(type_index) => {
                            field.ty = type_index;
                            errored = true;
                        }
                    }
                }

                for (inferred_generic, generic) in inferred_generics.into_iter().zip(&mut generics)
                {
                    match self.check_inferred(sources, inferred_generic, *generic, context) {
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
                    match self.check_inferred(sources, inferred_variant, *variant, context) {
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
                    match self.check_inferred(sources, inferred_generic, *generic, context) {
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

    fn apply(&mut self, type_index: TypeIndex, context: &[(String, TypeIndex)]) -> TypeIndex {
        match &self[type_index] {
            Type::Integer(_)
            | Type::NegativeInteger(_)
            | Type::Float(_)
            | Type::Primitive(_)
            | Type::Generic(_)
            | Type::Error => type_index,
            Type::Existential(name) => context
                .iter()
                .rfind(|(context_name, _)| context_name == name.kind())
                .map_or(type_index, |(_, type_index)| *type_index),
            Type::AnyOf { alternatives, span } => {
                let span = *span;

                let alternatives = alternatives
                    .clone()
                    .into_iter()
                    .map(|type_index| self.apply(type_index, context))
                    .collect::<Vec<_>>();

                self.push_type(Type::AnyOf { alternatives, span })
            }
            Type::Associations(alternatives) => {
                let alternatives = alternatives
                    .clone()
                    .into_iter()
                    .map(|association| Association {
                        resolved_index: association.resolved_index,
                        type_index: self.apply(type_index, context),
                        span: association.span,
                    })
                    .collect::<Vec<_>>();

                self.push_type(Type::Associations(alternatives))
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

                for field in &mut fields {
                    field.ty = self.apply(field.type_index(), context);
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

    #[allow(clippy::too_many_lines)]
    fn check_pattern(
        &mut self,
        sources: &[(usize, (&str, &str))],
        names: &Names,
        pattern: &Spanned<Pattern>,
        should_be: TypeIndex,
        context: &[(String, TypeIndex)],
    ) -> Result<TypeIndex, TypeIndex> {
        match (pattern.kind(), &self[should_be]) {
            (Pattern::Integer(value), Type::Integer(_) | Type::NegativeInteger(_)) => {
                Ok(self.push_type(Type::Integer(Spanned::new(*value, pattern.span()))))
            }
            (Pattern::NegativeInteger(value), Type::Integer(_) | Type::NegativeInteger(_)) => {
                Ok(self.push_type(Type::NegativeInteger(Spanned::new(*value, pattern.span()))))
            }
            (Pattern::Integer(radix), Type::Primitive(b)) => {
                let radix = *radix;

                let lexeme = sources
                    .iter()
                    .find(|(source_id, _)| *source_id == pattern.span().source_id())
                    .and_then(|(_, (_, source))| pattern.span().lexeme(source))
                    .expect("the span is from this source");

                match b.kind() {
                    Primitive::U8 if u8::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I8 if i8::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::U16 if u16::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I16 if i16::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::U32 if u32::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I32 if i32::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::U64 if u64::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    Primitive::I64 if i64::from_str_radix(lexeme, radix).is_ok() => Ok(should_be),
                    _ => Err(should_be),
                }
                .inspect(|type_index| {
                    self.type_map.insert(pattern.span(), *type_index);
                })
            },
            (Pattern::NegativeInteger(radix), Type::Primitive(b)) => {
                let radix = *radix;

                let lexeme = sources
                    .iter()
                    .find(|(source_id, _)| *source_id == pattern.span().source_id())
                    .and_then(|(_, (_, source))| pattern.span().lexeme(source))
                    .expect("the span is from this source");

                match b.kind() {
                    Primitive::I8 if i8::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() => Ok(should_be),
                    Primitive::I16 if i16::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() => Ok(should_be),
                    Primitive::I32 if i32::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() => Ok(should_be),
                    Primitive::I64 if i64::from_str_radix(format!("-{lexeme}").as_str(), radix).is_ok() => Ok(should_be),
                    _ => Err(should_be),
                }
                .inspect(|type_index| {
                    self.type_map.insert(pattern.span(), *type_index);
                })
            },
            (Pattern::String(text), Type::Primitive(primitive)) if matches!(primitive.kind(), Primitive::String) => {
                Ok(self.type_string())
            }
            (Pattern::Boolean(_), Type::Primitive(primitive))
                if matches!(primitive.kind(), Primitive::Boolean) =>
            {
                Ok(self.type_boolean())
            }
            (Pattern::Unit, Type::Primitive(primitive))
                if matches!(primitive.kind(), Primitive::Unit) =>
            {
                Ok(self.type_unit())
            }
            (
                Pattern::Product { path, fields },
                Type::Product {
                    name: should_be_name,
                    fields: should_be_fields,
                    generics: should_be_generics,
                },
            ) => {
                let mut context = context.to_vec();

                let context = &mut context;

                let name_span = path
                    .last()
                    .map(Spanned::span)
                    .expect("paths are never empty");

                let name = match path.last().map(Spanned::kind) {
                    Some(PathElement::Name(name)) => name.clone(),
                    Some(PathElement::SelfType) => match path
                        .last()
                        .map(Spanned::span)
                        .and_then(|associated_with| self.get_type(names[associated_with]))
                    {
                        Some(Type::Product { name, .. })
                            if name.kind() != should_be_name.kind() =>
                        {
                            return Err(self.apply(should_be, context));
                        }
                        Some(Type::Product { name, .. }) => name.kind().clone(),
                        _ => return Err(self.apply(should_be, context)),
                    },
                    _ => return Err(self.apply(should_be, context)),
                };

                let should_be_name = should_be_name.clone();
                let should_be_fields = should_be_fields.clone();
                let should_be_generics = should_be_generics.clone();

                let error_count = self.errors.len();

                let mut checked_fields = vec![];

                for should_be_field in &should_be_fields {
                    if let Some((field_name, field)) =
                        fields.iter().find_map(|(field_name, field)| {
                            if should_be_field.name() == field_name.kind() {
                                Some((field_name, field))
                            } else {
                                None
                            }
                        })
                    {
                        match self.field_type(names, &should_be_name, should_be_fields.as_slice(), (field_name.kind(), field_name.span())) {
                            Err(error) => {
                                self.errors.push(error);

                                self.type_map.insert(field_name.span(), self.type_error());
                            }
                            Ok(should_be_type_index) => {
                                let visibility = should_be_field.visibility();

                                if let Some(field) = field {
                                    match self.check_pattern(sources, names, field, should_be_type_index, context) {
                                        Ok(type_index) => {
                                            checked_fields
                                                .push(Field {
                                                    name: field_name.kind().clone(),
                                                    ty: type_index,
                                                    visibility,
                                                });

                                            self.type_map.insert(field_name.span(), type_index);
                                        }
                                        Err(type_index) => {
                                            let should_be = self.apply(should_be_type_index, context);

                                            self.errors.push(Spanned::new(
                                                self.type_mismatch_error(type_index, should_be),
                                                field_name.span(),
                                            ));
                                        }
                                    }
                                } else {
                                    let type_index = self.apply(should_be_type_index, context);

                                    checked_fields.push(Field {
                                        name: field_name.kind().clone(),
                                        ty: type_index,
                                        visibility,
                                    });

                                    self.type_map.insert(field_name.span(), type_index);
                                }
                            }
                        }
                    }
                }

                checked_fields.sort_by(|a_field, b_field| a_field.name().cmp(b_field.name()));

                for (field_name, _) in fields {
                    if !should_be_fields.iter().any(|should_be_field| {
                        should_be_field.name() == field_name.kind()
                    }) {
                        self.errors
                            .push(Spanned::new(Error::NonExistentField, field_name.span()));
                    }
                }

                if self.errors.len() == error_count {
                    Ok(self.push_type(Type::Product {
                        name: Spanned::new(name, name_span),
                        fields: checked_fields,
                        generics: should_be_generics,
                    }))
                } else {
                    Err(self.type_error())
                }
            }
            (
                Pattern::Product { path, .. },
                Type::Sum {
                    name: should_be_name,
                    variants: should_be_variants,
                    ..
                },
            ) => {
                let mut context = context.to_vec();

                let context = &mut context;

                let name_span = path
                    .last()
                    .map(Spanned::span)
                    .expect("paths are never empty");

                let name = match path.last().map(Spanned::kind) {
                    Some(PathElement::Name(name)) => name.clone(),
                    _ => return Err(self.apply(should_be, context)),
                };

                let (variant_index, variant_type) = if matches!(
                    path.iter()
                        .nth_back(1)
                        .map(Spanned::kind),
                    Some(PathElement::Name(sum_name))
                        if sum_name == should_be_name.kind()
                ) || matches!(
                        path.iter()
                            .nth_back(1)
                            .map(Spanned::kind),
                        Some(PathElement::SelfType)
                            if matches!(
                                path
                                    .iter()
                                    .nth_back(1)
                                    .map(Spanned::span)
                                    .and_then(|associated_with| self.get_type(names[associated_with])),
                                Some(Type::Sum { name: sum_name, .. })
                                    if sum_name.kind() == should_be_name.kind())
                ) {
                    if let Some((variant_index, variant_type)) = should_be_variants
                        .iter()
                        .enumerate()
                        .find_map(|(i, variant)| {
                            if let Type::Product {
                                name: variant_name, ..
                            } = &self[*variant]
                                && name.as_str() == variant_name.kind()
                            {
                                Some((i, *variant))
                            } else {
                                None
                            }
                        })
                    {
                        (variant_index, variant_type)
                    } else {
                        if !matches!(self.get_type(names[name_span]), Some(Type::Error)) {
                            self.errors
                                .push(Spanned::new(Error::NonExistentSumVariant, name_span));
                        }

                        return Err(self.type_error());
                    }
                } else {
                    if !matches!(self.get_type(names[name_span]), Some(Type::Error)) {
                        self.errors.push(Spanned::new(Error::NotASum, name_span));
                    }

                    return Err(self.type_error());
                };

                let type_index = self.check_pattern(sources, names, pattern, variant_type, context)?;

                let Type::Sum {
                    name,
                    variants,
                    generics,
                } = self[should_be].clone()
                else {
                    unreachable!("the outer if condition guarantees this is true");
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
                        .map(|generic| self.apply(generic, context))
                        .collect::<Vec<_>>(),
                };

                Ok(self.push_type(ty))
            }
            (_, _) => Err(self.type_error()),
        }
        .inspect(|type_index| {
            self.type_map.insert(pattern.span(), *type_index);
        })
    }
}
