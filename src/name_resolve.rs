use crate::{
    Reportable, Span, Spanned,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, TypeSignature, Visibility},
};

use std::{collections::HashMap, error, fmt, ops::Index};

pub fn resolve_names(ast: &Ast) -> Result<Names, Vec<Spanned<Error>>> {
    let mut resolver = NameResolver::new();

    for root in ast.roots() {
        resolver.associate_types(ast, *root);
    }

    for root in ast.roots() {
        resolver.resolve_types(ast, *root);
    }

    resolver.resolve_items(ast, ast.roots());

    if resolver.errors.is_empty() {
        Ok(Names(resolver.names))
    } else {
        Err(resolver.errors)
    }
}

// TODO: is it okay to use this span?
const ROOT_SPAN: Span = Span::new(0, 0, 0);

struct NameResolver {
    variable_scopes: Vec<HashMap<String, Definition>>,
    persistent_scopes: HashMap<Span, HashMap<String, Definition>>,
    associated_with: Option<Span>,
    current_mod: (usize, Span),
    errors: Vec<Spanned<Error>>,
    names: HashMap<Span, Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    NameUsedInItsDeclaration,
    NameNotDeclared,
    DuplicateFnParameterName,
    DuplicatePrimitiveName,
    DuplicateNativeFnName,
    DuplicateFnName,
    DuplicateProductName,
    InvalidAssignTarget,
    DuplicateSumName,
    DuplicateSumVariant,
    InvalidPath,
    DuplicateModName,
    ExpectedType,
    NameIsType,
    UnassociatedName,
    PathDoesNotExist,
    PathIsValue,
    NameIsMod,
    PathCannotAssociate,
    AssignmentTargetIsMod,
    AssignmentTargetIsType,
    AssignmentTargetIsFunction,
    PathIsPrivate,
    RootDeeperThanPathStart,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameUsedInItsDeclaration => {
                write!(f, "cannot use a name within its own declaration")
            }
            Self::NameNotDeclared => write!(f, "this name has not yet been declared"),
            Self::DuplicateFnParameterName => write!(
                f,
                "there is already a parameter for this function with this name"
            ),
            Self::DuplicatePrimitiveName => {
                write!(f, "there is already a primitive with this name")
            }
            Self::DuplicateNativeFnName => write!(
                f,
                "this native function name is already in use in this scope"
            ),
            Self::DuplicateFnName => {
                write!(f, "this function name is already in use in this scope")
            }
            Self::DuplicateProductName => {
                write!(f, "this product name is already in use in this scope")
            }
            Self::InvalidAssignTarget => {
                write!(f, "only names and fields may be assigned to")
            }
            Self::DuplicateSumName => {
                write!(f, "this sum name is already in use in this scope")
            }
            Self::DuplicateSumVariant => {
                write!(f, "this sum variant name is already in use by its sum")
            }
            Self::InvalidPath => {
                write!(f, "paths can only be made up of names")
            }
            Self::DuplicateModName => {
                write!(f, "this mod name is already in use in this scope")
            }
            Self::ExpectedType => {
                write!(f, "expected a type, but got a variable")
            }
            Self::NameIsType => {
                write!(f, "this is a type, but it should be a variable")
            }
            Self::UnassociatedName => {
                write!(f, "this name is not associated with this scope")
            }
            Self::PathDoesNotExist => {
                write!(f, "this path does not exist at this depth")
            }
            Self::PathIsValue => {
                write!(f, "this path must be a mod or a type, but it is a variable")
            }
            Self::NameIsMod => {
                write!(f, "this name is a module, but it should be a variable")
            }
            Self::PathCannotAssociate => {
                write!(
                    f,
                    "only modules and types can be the leading part of a path"
                )
            }
            Self::AssignmentTargetIsMod => {
                write!(f, "you cannot assign to a module")
            }
            Self::AssignmentTargetIsType => {
                write!(f, "you cannot assign to a type")
            }
            Self::AssignmentTargetIsFunction => {
                write!(f, "you cannot assign to a function")
            }
            Self::PathIsPrivate => {
                write!(f, "this is private and cannot be accessed from here")
            }
            Self::RootDeeperThanPathStart => {
                write!(
                    f,
                    "\"root\" can only be used in a path if it is the very beginning"
                )
            }
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DefinitionKind {
    Type,
    Mod,
    Function,
    Product,
    Sum,
    SumVariant,
    DefinedName,
    DeclaredOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Definition {
    kind: DefinitionKind,
    visibility: Visibility,
    span: Span,
}

pub struct Names(HashMap<Span, Span>);

impl Index<Span> for Names {
    type Output = Span;

    fn index(&self, index: Span) -> &Self::Output {
        self.0.get(&index).unwrap_or_else(|| {
            panic!("unresolved span: {index:?}");
        })
    }
}

impl Names {
    pub fn get(&self, span: Span) -> Option<Span> {
        self.0.get(&span).copied()
    }
}

impl NameResolver {
    fn new() -> Self {
        Self {
            variable_scopes: vec![HashMap::new()],
            persistent_scopes: HashMap::new(),
            associated_with: None,
            current_mod: (0, ROOT_SPAN),
            errors: vec![],
            names: HashMap::new(),
        }
    }

    fn associate_name(&mut self, of: Span, name: String, definition: Definition) {
        self.persistent_scopes
            .entry(of)
            .and_modify(|associated_with| {
                associated_with.insert(name.clone(), definition);
            })
            .or_insert_with(|| {
                let mut hash_map = HashMap::new();

                hash_map.insert(name, definition);

                hash_map
            });
    }

    fn resolve_associated_name(&self, of: Span, name: &str) -> Option<Definition> {
        self.persistent_scopes
            .get(&self.names.get(&of).map_or(of, |span| *span))
            .and_then(|associated_with| associated_with.get(name))
            .copied()
    }

    fn declare_name(&mut self, name: String, span: Span) {
        let Some(scope) = self.variable_scopes.last_mut() else {
            unreachable!("there will always be at least one scope");
        };

        scope.insert(
            name,
            Definition {
                kind: DefinitionKind::DeclaredOnly,
                visibility: Visibility::Public,
                span,
            },
        );
    }

    fn define_name(&mut self, name: &str) {
        if let Some(definition) = self
            .variable_scopes
            .iter_mut()
            .filter_map(|scope| scope.get_mut(name))
            .next_back()
        {
            definition.kind = DefinitionKind::DefinedName;
        }
    }

    fn declare_type(&mut self, name: String, span: Span) {
        let Some(scope) = self.variable_scopes.last_mut() else {
            unreachable!("there will always be at least one scope");
        };

        scope.insert(
            name,
            Definition {
                kind: DefinitionKind::Type,
                visibility: Visibility::Public,
                span,
            },
        );
    }

    fn undeclare(&mut self, name: &str) {
        let Some(scope) = self.variable_scopes.last_mut() else {
            unreachable!("there will always be at least one scope");
        };

        scope.remove(name);
    }

    fn resolve_name(&self, name: &str) -> Option<Definition> {
        let mut found = None;

        for definition in self
            .variable_scopes
            .iter()
            .rev()
            .filter_map(|scope| scope.get(name))
        {
            if found.is_none() {
                found = Some(definition.span);
            }

            if definition.kind != DefinitionKind::DeclaredOnly {
                return Some(*definition);
            }
        }

        found.map_or_else(
            || {
                self.associated_with
                    .and_then(|associated_with| self.resolve_associated_name(associated_with, name))
                    .map_or_else(
                        || self.resolve_associated_name(self.current_mod.1, name),
                        Some,
                    )
            },
            |span| {
                Some(Definition {
                    kind: DefinitionKind::DeclaredOnly,
                    visibility: Visibility::Public,
                    span,
                })
            },
        )
    }

    fn resolve_and_insert_name(
        &mut self,
        name: &str,
        span: Span,
    ) -> Result<Definition, Spanned<Error>> {
        if let Some(definition) = self.resolve_name(name) {
            if definition.kind == DefinitionKind::DeclaredOnly {
                Err(Spanned::new(Error::NameUsedInItsDeclaration, span))
            } else {
                self.names.insert(span, definition.span);
                Ok(definition)
            }
        } else {
            Err(Spanned::new(Error::NameNotDeclared, span))
        }
    }

    fn resolve_type_signature(&mut self, ty: &Spanned<TypeSignature>) {
        match ty.kind() {
            TypeSignature::Path {
                path,
                name,
                generics,
            } => {
                let associated_with = self.associated_with.take();

                let mut found_root = None;

                for (i, element) in path.iter().enumerate() {
                    let deeper_than_here = i > 0
                        && found_root
                            .is_none_or(|found_root| i - (found_root + 1) > self.current_mod.0);

                    let error_count = self.errors.len();

                    self.resolve_path_element(element.kind(), element.span(), deeper_than_here);

                    if self.errors.len() > error_count {
                        break;
                    } else if element.kind() == "root" {
                        if deeper_than_here {
                            self.errors
                                .push(Spanned::new(Error::RootDeeperThanPathStart, element.span()));

                            break;
                        }

                        found_root = Some(i);

                        self.associated_with = Some(ROOT_SPAN);

                        continue;
                    }

                    if let Some(span) = self.names.get(&element.span()) {
                        self.associated_with = Some(*span);
                    } else {
                        break;
                    }
                }

                match self.resolve_and_insert_name(name.kind(), name.span()) {
                    Err(error) => {
                        self.errors.push(error);
                    }
                    Ok(definition)
                        if matches!(
                            definition.kind,
                            DefinitionKind::DefinedName
                                | DefinitionKind::Function
                                | DefinitionKind::Mod
                        ) =>
                    {
                        self.errors
                            .push(Spanned::new(Error::ExpectedType, ty.span()));
                    }
                    Ok(_) => {}
                }

                self.associated_with = associated_with;

                for generic in generics {
                    self.resolve_type_signature(generic);
                }
            }
            TypeSignature::Normal { name, generics } => {
                for generic in generics {
                    self.resolve_type_signature(generic);
                }

                match self.resolve_and_insert_name(name.kind(), name.span()) {
                    Err(error) => {
                        self.errors.push(error);
                    }
                    Ok(definition)
                        if matches!(
                            definition.kind,
                            DefinitionKind::DefinedName
                                | DefinitionKind::Function
                                | DefinitionKind::Mod
                        ) =>
                    {
                        self.errors
                            .push(Spanned::new(Error::ExpectedType, ty.span()));
                    }
                    Ok(_) => {}
                }
            }
            TypeSignature::Fn {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.resolve_type_signature(parameter);
                }

                self.resolve_type_signature(return_type);
            }
        }
    }
}

impl NameResolver {
    #[allow(clippy::too_many_lines)]
    fn associate_types(&mut self, ast: &Ast, item: ItemIndex) {
        match ast[item].kind() {
            Item::NativeFn { .. } => {}
            Item::Primitive(name) => {
                let associated_with = self.associated_with.take();

                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicatePrimitiveName, name.span()));
                }

                self.declare_type(name.kind().clone(), name.span());

                self.associated_with = associated_with;
            }
            Item::Fn { name, generics, .. } => {
                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                for generic in generics {
                    self.associate_name(
                        name.span(),
                        generic.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Type,
                            visibility: Visibility::Private,
                            span: generic.span(),
                        },
                    );
                }

                self.associated_with = associated_with;
            }
            Item::Mod {
                name,
                generics,
                contents,
                visibility,
            } => {
                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicateModName, name.span()));
                } else {
                    self.associate_name(
                        self.current_mod.1,
                        name.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Mod,
                            visibility: *visibility,
                            span: name.span(),
                        },
                    );
                }

                let associated_with = self.associated_with.take();
                let current_mod = self.current_mod;

                self.current_mod = (current_mod.0 + 1, name.span());

                for generic in generics {
                    self.associate_name(
                        name.span(),
                        generic.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Type,
                            visibility: Visibility::Private,
                            span: generic.span(),
                        },
                    );
                }

                for item in contents {
                    self.associate_types(ast, *item);
                }

                self.current_mod = current_mod;
                self.associated_with = associated_with;
            }
            Item::Product {
                name,
                generics,
                visibility,
                ..
            } => {
                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicateProductName, name.span()));
                } else {
                    self.associate_name(
                        self.current_mod.1,
                        name.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Product,
                            visibility: *visibility,
                            span: name.span(),
                        },
                    );
                }

                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                for generic in generics {
                    self.associate_name(
                        name.span(),
                        generic.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Type,
                            visibility: Visibility::Private,
                            span: generic.span(),
                        },
                    );
                }

                self.associated_with = associated_with;
            }
            Item::Sum {
                name,
                variants,
                generics,
                visibility,
                ..
            } => {
                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicateSumName, name.span()));
                } else {
                    self.associate_name(
                        self.current_mod.1,
                        name.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Sum,
                            visibility: *visibility,
                            span: name.span(),
                        },
                    );
                }

                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                for generic in generics {
                    self.associate_name(
                        name.span(),
                        generic.kind().clone(),
                        Definition {
                            kind: DefinitionKind::Type,
                            visibility: Visibility::Private,
                            span: generic.span(),
                        },
                    );
                }

                for variant in variants {
                    let Item::Product {
                        name: variant_name, ..
                    } = ast[*variant].kind()
                    else {
                        unreachable!("variants are only products");
                    };

                    if self
                        .resolve_associated_name(name.span(), variant_name.kind())
                        .is_some()
                    {
                        self.errors
                            .push(Spanned::new(Error::DuplicateSumVariant, name.span()));
                    } else {
                        self.associate_name(
                            name.span(),
                            variant_name.kind().clone(),
                            Definition {
                                kind: DefinitionKind::SumVariant,
                                visibility: *visibility,
                                span: variant_name.span(),
                            },
                        );
                    }
                }

                self.associated_with = associated_with;
            }
        }
    }

    fn resolve_types(&mut self, ast: &Ast, item: ItemIndex) {
        match ast[item].kind() {
            Item::Primitive(_) => {}
            Item::NativeFn {
                name,
                signature,
                visibility,
            } => {
                let associated_with = self.associated_with.take();

                self.resolve_type_signature(signature);

                self.resolve_function_name(name, *visibility, Error::DuplicateNativeFnName);

                self.declare_name(name.kind().clone(), name.span());
                self.define_name(name.kind());

                self.associated_with = associated_with;
            }
            Item::Mod {
                name,
                contents,
                generics,
                ..
            } => {
                let current_mod = self.current_mod;

                self.current_mod = (current_mod.0 + 1, name.span());

                for generic in generics {
                    self.declare_type(generic.kind().clone(), generic.span());
                }

                for item in contents {
                    self.resolve_types(ast, *item);
                }

                for generic in generics {
                    self.undeclare(generic.kind());
                }

                self.current_mod = current_mod;
            }
            Item::Fn {
                name,
                parameters,
                return_type,
                visibility,
                ..
            } => {
                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                for parameter in parameters {
                    self.resolve_type_signature(parameter.ty());
                }

                self.resolve_type_signature(return_type);

                self.associated_with = associated_with;

                self.resolve_function_name(name, *visibility, Error::DuplicateFnName);
            }
            Item::Product { name, fields, .. } => {
                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                for field in fields {
                    self.resolve_type_signature(field.ty());
                }

                self.associated_with = associated_with;
            }
            Item::Sum { name, variants, .. } => {
                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                for variant in variants {
                    let Item::Product { fields, .. } = ast[*variant].kind() else {
                        unreachable!("variants are only products");
                    };

                    for field in fields {
                        self.resolve_type_signature(field.ty());
                    }
                }

                self.associated_with = associated_with;
            }
        }
    }

    fn resolve_function_name(
        &mut self,
        name: &Spanned<String>,
        visibility: Visibility,
        error: Error,
    ) {
        if let Some(associated_with) = self.associated_with {
            if self
                .resolve_associated_name(associated_with, name.kind())
                .is_some()
            {
                self.errors.push(Spanned::new(error, name.span()));
            } else {
                self.associate_name(
                    associated_with,
                    name.kind().clone(),
                    Definition {
                        kind: DefinitionKind::Function,
                        visibility,
                        span: name.span(),
                    },
                );
            }
        } else if self.resolve_name(name.kind()).is_some() {
            self.errors.push(Spanned::new(error, name.span()));
        } else {
            self.associate_name(
                self.current_mod.1,
                name.kind().clone(),
                Definition {
                    kind: DefinitionKind::Function,
                    visibility,
                    span: name.span(),
                },
            );
        }
    }

    fn resolve_items(&mut self, ast: &Ast, items: &[ItemIndex]) {
        for item in items {
            self.resolve_item(ast, *item);
        }
    }

    fn resolve_item(&mut self, ast: &Ast, item: ItemIndex) {
        match ast[item].kind() {
            Item::Primitive(_)
            | Item::NativeFn { .. }
            | Item::Product { .. }
            | Item::Sum { .. } => {}
            Item::Mod {
                name,
                contents,
                generics,
                ..
            } => {
                let current_mod = self.current_mod;

                self.current_mod = (current_mod.0 + 1, name.span());

                for generic in generics {
                    self.declare_type(generic.kind().clone(), generic.span());
                }

                self.resolve_items(ast, contents);

                for generic in generics {
                    self.undeclare(generic.kind());
                }

                self.current_mod = current_mod;
            }
            Item::Fn {
                name,
                parameters,
                body,
                ..
            } => {
                let associated_with = self.associated_with.take();

                self.associated_with = Some(name.span());

                self.variable_scopes.push(HashMap::new());

                for (p, parameter) in parameters.iter().enumerate() {
                    if parameters
                        .iter()
                        .take(p)
                        .chain(parameters.iter().skip(p + 1))
                        .all(|other_parameter| {
                            other_parameter.name().kind() != parameter.name().kind()
                        })
                    {
                        self.declare_name(parameter.name().kind().clone(), parameter.name().span());

                        self.define_name(parameter.name().kind());
                    } else {
                        self.errors.push(Spanned::new(
                            Error::DuplicateFnParameterName,
                            parameter.name().span(),
                        ));
                    }
                }

                self.resolve_expr(ast, *body, false);

                self.variable_scopes.pop();

                self.associated_with = associated_with;
            }
        }
    }
}

impl NameResolver {
    #[allow(clippy::too_many_lines)]
    fn resolve_expr(&mut self, ast: &Ast, expr: ExprIndex, check_visibility: bool) {
        match ast[expr].kind() {
            Expr::Let {
                name,
                type_signature,
                ..
            } => {
                if let Some(type_signature) = type_signature {
                    self.resolve_type_signature(type_signature);
                }

                self.declare_name(name.kind().clone(), name.span());
            }
            Expr::Block(_) => {
                self.variable_scopes.push(HashMap::new());
            }
            _ => {}
        }

        if let Expr::Binary {
            op: BinaryOp::PathAccess,
            ..
        } = ast[expr].kind()
        {
            self.resolve_path(ast, expr);
        } else {
            ast.for_children_exprs(expr, |ast, expr| {
                self.resolve_expr(ast, expr, false);
            });
        }

        match ast[expr].kind() {
            Expr::Let { name, .. } => {
                self.define_name(name.kind());
            }
            Expr::Block(_) => {
                self.variable_scopes.pop();
            }
            Expr::Binary {
                op: BinaryOp::Assign,
                lhs,
                ..
            } => {
                let lhs = *lhs;

                match ast[lhs].kind() {
                    Expr::Name(name) => match self.resolve_and_insert_name(name, ast[lhs].span()) {
                        Err(error) => {
                            self.errors.push(error);
                        }
                        Ok(definition)
                            if matches!(
                                definition.kind,
                                DefinitionKind::Type
                                    | DefinitionKind::Product
                                    | DefinitionKind::Sum
                                    | DefinitionKind::SumVariant
                            ) =>
                        {
                            self.errors
                                .push(Spanned::new(Error::AssignmentTargetIsType, ast[lhs].span()));
                        }
                        Ok(definition) if matches!(definition.kind, DefinitionKind::Mod) => {
                            self.errors
                                .push(Spanned::new(Error::AssignmentTargetIsMod, ast[lhs].span()));
                        }
                        Ok(definition) if matches!(definition.kind, DefinitionKind::Function) => {
                            self.errors.push(Spanned::new(
                                Error::AssignmentTargetIsFunction,
                                ast[lhs].span(),
                            ));
                        }
                        Ok(_) => {}
                    },
                    Expr::Binary {
                        op: BinaryOp::Access,
                        rhs,
                        ..
                    } if matches!(ast[*rhs].kind(), Expr::Name(_)) => {}
                    _ => {
                        self.errors
                            .push(Spanned::new(Error::InvalidAssignTarget, ast[lhs].span()));
                    }
                }
            }
            Expr::Name(name) => {
                let span = ast[expr].span();

                match self.resolve_and_insert_name(name, ast[expr].span()) {
                    Err(error) => {
                        self.errors.push(error);
                    }
                    Ok(definition)
                        if matches!(
                            definition.kind,
                            DefinitionKind::Type
                                | DefinitionKind::Product
                                | DefinitionKind::Sum
                                | DefinitionKind::SumVariant
                        ) =>
                    {
                        self.errors.push(Spanned::new(Error::NameIsType, span));
                    }
                    Ok(definition) if matches!(definition.kind, DefinitionKind::Mod) => {
                        self.errors.push(Spanned::new(Error::NameIsMod, span));
                    }
                    Ok(definition)
                        if check_visibility
                            && matches!(definition.visibility, Visibility::Private) =>
                    {
                        self.errors.push(Spanned::new(Error::PathIsPrivate, span));
                    }
                    Ok(_) => {}
                }
            }
            Expr::Product { name, .. } => {
                match self.resolve_and_insert_name(name.kind(), name.span()) {
                    Err(error) => {
                        self.errors.push(error);
                    }
                    Ok(definition)
                        if !matches!(
                            definition.kind,
                            DefinitionKind::Product | DefinitionKind::SumVariant
                        ) =>
                    {
                        self.errors
                            .push(Spanned::new(Error::ExpectedType, name.span()));
                    }
                    Ok(definition)
                        if check_visibility
                            && matches!(definition.visibility, Visibility::Private)
                            && matches!(definition.kind, DefinitionKind::Product) =>
                    {
                        self.errors
                            .push(Spanned::new(Error::PathIsPrivate, name.span()));
                    }
                    Ok(_) => {}
                }
            }
            _ => {}
        }
    }

    fn resolve_path(&mut self, ast: &Ast, expr: ExprIndex) {
        let mut path = vec![];

        let mut lhs = expr;

        let error_count = self.errors.len();

        while let Expr::Binary {
            op: BinaryOp::PathAccess,
            lhs: lhs_of_lhs,
            rhs: rhs_of_lhs,
        } = ast[lhs].kind()
        {
            match ast[*lhs_of_lhs].kind() {
                Expr::Name(_)
                | Expr::Binary {
                    op: BinaryOp::PathAccess,
                    ..
                } => match ast[*rhs_of_lhs].kind() {
                    Expr::Name(_) | Expr::Product { .. } => {
                        path.push(*rhs_of_lhs);
                    }
                    _ => {
                        self.errors
                            .push(Spanned::new(Error::InvalidPath, ast[*rhs_of_lhs].span()));
                    }
                },
                _ => {
                    self.errors
                        .push(Spanned::new(Error::InvalidPath, ast[*lhs_of_lhs].span()));
                }
            }

            lhs = *lhs_of_lhs;
        }

        if let Expr::Name(_) = ast[lhs].kind() {
            path.push(lhs);
        } else {
            self.errors
                .push(Spanned::new(Error::InvalidPath, ast[lhs].span()));
        }

        if self.errors.len() == error_count {
            let associated_with = self.associated_with.take();

            let path_len = path.len();

            let mut found_root = None;

            while path.len() > 1
                && let Some(p) = path.pop()
            {
                if let Expr::Name(name) = ast[p].kind() {
                    let deeper_than_here = path.len() < path_len - 1
                        && found_root.is_none_or(|found_root| {
                            (path_len - path.len()) - (found_root + 1) > self.current_mod.0
                        });

                    let error_count = self.errors.len();

                    self.resolve_path_element(name, ast[p].span(), deeper_than_here);

                    if self.errors.len() > error_count {
                        break;
                    } else if name == "root" {
                        if deeper_than_here {
                            self.errors
                                .push(Spanned::new(Error::RootDeeperThanPathStart, ast[p].span()));

                            break;
                        }

                        found_root = Some(path_len - path.len());

                        self.associated_with = Some(ROOT_SPAN);

                        continue;
                    }
                }

                if let Some(span) = self.names.get(&ast[p].span()) {
                    self.associated_with = Some(*span);
                } else {
                    break;
                }
            }

            if self.errors.len() == error_count
                && path.len() == 1
                && let Some(rhs) = path.pop()
            {
                self.resolve_expr(ast, rhs, true);
            }

            self.associated_with = associated_with;
        }
    }

    fn resolve_path_element(&mut self, name: &str, span: Span, deeper_than_here: bool) {
        if name == "root" {
            return;
        }

        match self.resolve_and_insert_name(name, span) {
            Err(_) => {
                self.errors
                    .push(Spanned::new(Error::PathDoesNotExist, span));
            }
            Ok(definition) if matches!(definition.kind, DefinitionKind::DefinedName) => {
                self.errors.push(Spanned::new(Error::PathIsValue, span));
            }
            Ok(definition) if matches!(definition.kind, DefinitionKind::Function) => {
                self.errors
                    .push(Spanned::new(Error::PathCannotAssociate, span));
            }
            Ok(definition)
                if deeper_than_here && matches!(definition.visibility, Visibility::Private) =>
            {
                self.errors.push(Spanned::new(Error::PathIsPrivate, span));
            }
            Ok(_) => {}
        }
    }
}
