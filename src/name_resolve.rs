use crate::{
    Reportable, Span, Spanned,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, TypeSignature},
};

use std::{collections::HashMap, error, fmt};

pub fn resolve_names(ast: &Ast) -> Result<HashMap<Span, Span>, Vec<Spanned<Error>>> {
    let mut resolver = NameResolver::new();

    // primitives should be resolved before anything else
    resolver.resolve_primitives(ast);

    resolver.resolve_native_functions(ast);

    // we need to resolve all functions before their bodies
    resolver.resolve_functions(ast);

    resolver.resolve(ast);

    if resolver.errors.is_empty() {
        Ok(resolver.names)
    } else {
        Err(resolver.errors)
    }
}

struct NameResolver {
    scopes: Vec<HashMap<String, (Span, Defined)>>,
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
            scopes: vec![HashMap::new()],
            errors: vec![],
            names: HashMap::new(),
        }
    }

    fn declare_name(&mut self, name: String, span: Span) {
        let Some(scope) = self.scopes.last_mut() else {
            unreachable!("there will always be at least one scope");
        };

        scope.insert(name, (span, Defined::No));
    }

    fn define_name(&mut self, name: &str) {
        if let Some((_, defined)) = self
            .scopes
            .iter_mut()
            .filter_map(|scope| scope.get_mut(name))
            .next_back()
        {
            *defined = Defined::Yes;
        }
    }

    fn resolve_name(&self, name: &str) -> Option<(Span, Defined)> {
        let mut found = None;

        for (span, defined) in self.scopes.iter().rev().filter_map(|scope| scope.get(name)) {
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
            .scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.values_mut().find(|(name, _)| *name == target))
        {
            // *name = span;

            return;
        }

        self.errors.push(Spanned::new(Error::NameNotDeclared, span));
    }

    fn resolve_and_insert_name(&mut self, name: &str, span: Span) {
        if let Some((name_span, defined)) = self.resolve_name(name) {
            if defined == Defined::No {
                self.errors
                    .push(Spanned::new(Error::NameUsedInItsDeclaration, span));
            } else {
                self.names.insert(span, name_span);
            }
        } else {
            self.errors.push(Spanned::new(Error::NameNotDeclared, span));
        }
    }

    fn resolve_type_signature(&mut self, ty: &Spanned<TypeSignature>) {
        match ty.kind() {
            TypeSignature::Name(name) => {
                self.resolve_and_insert_name(name, ty.span());
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
    fn resolve_primitives(&mut self, ast: &Ast) {
        for root in ast.roots() {
            match ast.get_item(*root).map(Spanned::kind) {
                None => unreachable!("the ast should be valid since we succeeded in parsing"),
                Some(Item::Primitive(name)) => {
                    if self.resolve_name(name.kind()).is_some() {
                        self.errors
                            .push(Spanned::new(Error::DuplicatePrimitiveName, name.span()));
                    } else {
                        self.declare_name(name.kind().clone(), name.span());

                        self.define_name(name.kind());
                    }
                }
                Some(Item::NativeFn { .. } | Item::Fn { .. }) => {}
            }
        }
    }

    fn resolve_native_functions(&mut self, ast: &Ast) {
        for root in ast.roots() {
            match ast.get_item(*root).map(Spanned::kind) {
                None => unreachable!("the ast should be valid since we succeeded in parsing"),
                Some(Item::NativeFn { name, signature }) => {
                    self.resolve_type_signature(signature);

                    if self.resolve_name(name.kind()).is_some() {
                        self.errors
                            .push(Spanned::new(Error::DuplicateNativeFnName, name.span()));
                    } else {
                        self.declare_name(name.kind().clone(), name.span());

                        self.define_name(name.kind());
                    }
                }
                Some(Item::Primitive(_) | Item::Fn { .. }) => {}
            }
        }
    }

    fn resolve_functions(&mut self, ast: &Ast) {
        for root in ast.roots() {
            match ast.get_item(*root).map(Spanned::kind) {
                None => unreachable!("the ast should be valid since we succeeded in parsing"),
                Some(Item::NativeFn { .. } | Item::Primitive(_)) => {}
                Some(Item::Fn {
                    name,
                    parameters,
                    return_type,
                    ..
                }) => {
                    for parameter in parameters {
                        self.resolve_type_signature(parameter.ty());
                    }

                    self.resolve_type_signature(return_type);

                    if self.resolve_name(name.kind()).is_some() {
                        self.errors
                            .push(Spanned::new(Error::DuplicateFnName, name.span()));
                    } else {
                        self.declare_name(name.kind().clone(), name.span());

                        self.define_name(name.kind());
                    }
                }
            }
        }
    }

    fn resolve(&mut self, ast: &Ast) {
        for root in ast.roots() {
            if let Some(Item::Fn {
                body, parameters, ..
            }) = ast.get_item(*root).map(Spanned::kind)
            {
                self.scopes.push(HashMap::new());

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

                self.resolve_names(ast, *body);

                self.scopes.pop();
            }
        }
    }
}

impl NameResolver {
    fn resolve_names(&mut self, ast: &Ast, expr: ExprIndex) {
        match ast.get_expr(expr).map(Spanned::kind) {
            Some(Expr::Let {
                name,
                type_signature,
                ..
            }) => {
                if let Some(type_signature) = type_signature {
                    self.resolve_type_signature(type_signature);
                }

                self.declare_name(
                    name.clone(),
                    ast.get_expr(expr)
                        .map(Spanned::span)
                        .expect("if the expression exists, the span does too"),
                );
            }
            Some(Expr::Block(_)) => {
                self.scopes.push(HashMap::new());
            }
            _ => {}
        }

        ast.for_children_exprs(expr, |ast, expr| self.resolve_names(ast, expr));

        match ast.get_expr(expr).map(Spanned::kind) {
            Some(Expr::Let { name, .. }) => {
                self.define_name(name);
            }
            Some(Expr::Block(_)) => {
                self.scopes.pop();
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
                    Some(_) => unreachable!("only names can be assigned to (for now)"),
                }
            }
            Some(Expr::Name(name)) => {
                let span = ast
                    .get_expr(expr)
                    .map(Spanned::span)
                    .expect("if the expression exists, the span does too");

                self.resolve_and_insert_name(name, span);
            }
            _ => {}
        }
    }
}
