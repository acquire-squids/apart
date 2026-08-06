use crate::{
    Reportable, Span, Spanned,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, TypeSignature},
};

use std::{collections::HashMap, error, fmt};

pub fn resolve_names(ast: &Ast) -> Result<HashMap<Span, Span>, Vec<Spanned<Error>>> {
    let mut resolver = NameResolver::new();

    // primitives should be resolved before anything else
    resolver.resolve_primitives(ast);

    // now that all the types are resolved, resolve their uses
    for root in ast.roots() {
        resolver.resolve_types(ast, *root, None);
    }

    resolver.resolve_native_functions(ast);

    resolver.resolve(ast);

    if resolver.errors.is_empty() {
        Ok(resolver.names)
    } else {
        Err(resolver.errors)
    }
}

struct NameResolver {
    variable_scopes: Vec<HashMap<String, (Span, Defined)>>,
    persistent_scopes: HashMap<Span, HashMap<String, Span>>,
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
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Defined {
    Yes,
    No,
}

impl NameResolver {
    fn new() -> Self {
        Self {
            variable_scopes: vec![HashMap::new()],
            persistent_scopes: HashMap::new(),
            errors: vec![],
            names: HashMap::new(),
        }
    }

    fn associate_name(&mut self, of: Span, name: String, span: Span) {
        self.persistent_scopes
            .entry(of)
            .and_modify(|associated_with| {
                associated_with.insert(name.clone(), span);
            })
            .or_insert_with(|| {
                let mut hash_map = HashMap::new();

                hash_map.insert(name, span);

                hash_map
            });
    }

    fn resolve_associated_name(&self, of: Span, name: &str) -> Option<Span> {
        self.persistent_scopes
            .get(&self.names.get(&of).map_or(of, |span| *span))
            .and_then(|associated_with| associated_with.get(name))
            .copied()
    }

    fn declare_name(&mut self, name: String, span: Span) {
        let Some(scope) = self.variable_scopes.last_mut() else {
            unreachable!("there will always be at least one scope");
        };

        scope.insert(name, (span, Defined::No));
    }

    fn define_name(&mut self, name: &str) {
        if let Some((_, defined)) = self
            .variable_scopes
            .iter_mut()
            .filter_map(|scope| scope.get_mut(name))
            .next_back()
        {
            *defined = Defined::Yes;
        }
    }

    fn resolve_name(&self, name: &str) -> Option<(Span, Defined)> {
        let mut found = None;

        for (span, defined) in self
            .variable_scopes
            .iter()
            .rev()
            .filter_map(|scope| scope.get(name))
        {
            if found.is_none() {
                found = Some(span);
            }

            if *defined == Defined::Yes {
                return Some((*span, *defined));
            }
        }

        found.map(|span| (*span, Defined::No))
    }

    fn assign_name(&mut self, target: Span, span: Span) {
        // if let Some((name, _)) = self
        if let Some((_, _)) = self
            .variable_scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.values_mut().find(|(name, _)| *name == target))
        {
            // *name = span;

            return;
        }

        self.errors.push(Spanned::new(Error::NameNotDeclared, span));
    }

    fn resolve_and_insert_name(&mut self, associated_with: Option<Span>, name: &str, span: Span) {
        if let Some((name_span, defined)) = self.resolve_name(name) {
            if defined == Defined::No {
                self.errors
                    .push(Spanned::new(Error::NameUsedInItsDeclaration, span));
            } else {
                self.names.insert(span, name_span);
            }
        } else if let Some(name_span) = associated_with
            .and_then(|associated_with| self.resolve_associated_name(associated_with, name))
        {
            self.names.insert(span, name_span);
        } else {
            self.errors.push(Spanned::new(Error::NameNotDeclared, span));
        }
    }

    fn resolve_type_signature(
        &mut self,
        (associated_with_outer, associated_with_inner): (Option<Span>, Option<Span>),
        ty: &Spanned<TypeSignature>,
    ) {
        match ty.kind() {
            TypeSignature::Normal { name, generics } => {
                for generic in generics {
                    self.resolve_type_signature(
                        (associated_with_outer, associated_with_inner),
                        generic,
                    );
                }

                self.resolve_and_insert_name(
                    associated_with_inner.map_or(associated_with_outer, |associated_with| {
                        Some(associated_with)
                    }),
                    name.kind(),
                    ty.span(),
                );
            }
            TypeSignature::Fn {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.resolve_type_signature(
                        (associated_with_outer, associated_with_inner),
                        parameter,
                    );
                }

                self.resolve_type_signature(
                    (associated_with_outer, associated_with_inner),
                    return_type,
                );
            }
        }
    }
}

impl NameResolver {
    fn resolve_primitives(&mut self, ast: &Ast) {
        for root in ast.roots() {
            if let Some(Item::Primitive(name)) = ast.get_item(*root).map(Spanned::kind) {
                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicatePrimitiveName, name.span()));
                } else {
                    self.declare_name(name.kind().clone(), name.span());

                    self.define_name(name.kind());
                }
            }
        }
    }

    fn resolve_native_functions(&mut self, ast: &Ast) {
        for root in ast.roots() {
            if let Some(Item::NativeFn { name, signature }) = ast.get_item(*root).map(Spanned::kind)
            {
                self.resolve_type_signature((None, None), signature);

                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicateNativeFnName, name.span()));
                } else {
                    self.declare_name(name.kind().clone(), name.span());

                    self.define_name(name.kind());
                }
            }
        }
    }

    fn resolve_types(&mut self, ast: &Ast, item: ItemIndex, associated_with: Option<Span>) {
        match ast.get_item(item).map(Spanned::kind) {
            None | Some(Item::Primitive(_) | Item::NativeFn { .. }) => {}
            Some(Item::Fn {
                name,
                parameters,
                return_type,
                generics,
                ..
            }) => {
                for generic in generics {
                    self.associate_name(name.span(), generic.kind().clone(), generic.span());
                }
                for parameter in parameters {
                    self.resolve_type_signature(
                        (associated_with, Some(name.span())),
                        parameter.ty(),
                    );
                }

                self.resolve_type_signature((associated_with, Some(name.span())), return_type);

                self.resolve_function_name(ast, item, associated_with);
            }
            Some(Item::Product {
                name,
                fields,
                generics,
            }) => {
                for generic in generics {
                    self.associate_name(name.span(), generic.kind().clone(), generic.span());
                }

                for field in fields {
                    self.resolve_type_signature((Some(name.span()), None), field.ty());
                }

                if self.resolve_name(name.kind()).is_some() {
                    self.errors
                        .push(Spanned::new(Error::DuplicateProductName, name.span()));
                } else {
                    self.declare_name(name.kind().clone(), name.span());

                    self.define_name(name.kind());
                }
            }
        }
    }

    fn resolve_function_name(&mut self, ast: &Ast, item: ItemIndex, associated_with: Option<Span>) {
        if let Some(Item::Fn { name, .. }) = ast.get_item(item).map(Spanned::kind) {
            if let Some(associated_with) = associated_with {
                if self
                    .resolve_associated_name(associated_with, name.kind())
                    .is_some()
                {
                    self.errors
                        .push(Spanned::new(Error::DuplicateFnName, name.span()));
                } else {
                    self.associate_name(associated_with, name.kind().clone(), name.span());
                }
            } else if self.resolve_name(name.kind()).is_some() {
                self.errors
                    .push(Spanned::new(Error::DuplicateFnName, name.span()));
            } else {
                self.declare_name(name.kind().clone(), name.span());

                self.define_name(name.kind());
            }
        }
    }

    fn resolve(&mut self, ast: &Ast) {
        for root in ast.roots() {
            if let Some(Item::Fn {
                name,
                body,
                parameters,
                ..
            }) = ast.get_item(*root).map(Spanned::kind)
            {
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

                self.resolve_names(ast, *body, Some(name.span()));

                self.variable_scopes.pop();
            }
        }
    }
}

impl NameResolver {
    fn resolve_names(&mut self, ast: &Ast, expr: ExprIndex, associated_with: Option<Span>) {
        match ast.get_expr(expr).map(Spanned::kind) {
            Some(Expr::Let {
                name,
                type_signature,
                ..
            }) => {
                if let Some(type_signature) = type_signature {
                    self.resolve_type_signature((associated_with, None), type_signature);
                }

                self.declare_name(name.kind().clone(), name.span());
            }
            Some(Expr::Block(_)) => {
                self.variable_scopes.push(HashMap::new());
            }
            _ => {}
        }

        ast.for_children_exprs(expr, |ast, expr| {
            self.resolve_names(ast, expr, associated_with);
        });

        match ast.get_expr(expr).map(Spanned::kind) {
            Some(Expr::Let { name, .. }) => {
                self.define_name(name.kind());
            }
            Some(Expr::Block(_)) => {
                self.variable_scopes.pop();
            }
            Some(Expr::Binary {
                op: BinaryOp::Assign,
                lhs,
                ..
            }) => {
                let lhs = *lhs;

                match ast.get_expr(lhs).map(Spanned::kind) {
                    None => unreachable!("parsing ensures all binary left operands exist"),
                    Some(Expr::Name(name)) => {
                        if let Some((name_span, _)) = self.resolve_name(name) {
                            let span = ast
                                .get_expr(expr)
                                .map(Spanned::span)
                                .expect("parsing ensures all binary left operands exist");

                            self.assign_name(name_span, span);
                        }
                    }
                    Some(Expr::Binary {
                        op: BinaryOp::Access,
                        rhs,
                        ..
                    }) if matches!(ast.get_expr(*rhs).map(Spanned::kind), Some(Expr::Name(_))) => {}
                    Some(_) => {
                        self.errors.push(Spanned::new(
                            Error::InvalidAssignTarget,
                            ast.get_expr(lhs)
                                .map(Spanned::span)
                                .expect("if the expression exists, the span does too"),
                        ));
                    }
                }
            }
            Some(Expr::Name(name)) => {
                let span = ast
                    .get_expr(expr)
                    .map(Spanned::span)
                    .expect("if the expression exists, the span does too");

                self.resolve_and_insert_name(associated_with, name, span);
            }
            Some(Expr::Product { name, .. }) => {
                self.resolve_and_insert_name(None, name.kind(), name.span());
            }
            _ => {}
        }
    }
}
