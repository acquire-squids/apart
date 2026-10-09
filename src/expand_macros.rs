use crate::{
    Reportable, Spanned, Target,
    parse::{Ast, BinaryOp, Expr, ExprIndex, Item, ItemIndex, UnaryOp},
};

use std::{error, fmt};

pub fn expand(ast: &mut Ast, target: Target) -> Result<(), Vec<Spanned<Error>>> {
    let mut errors = vec![];

    for r in 0..(ast.roots().len()) {
        let Some(item) = ast.roots().get(r) else {
            unreachable!("roots are never removed");
        };

        if let Err(error) = expand_item(ast, target, *item) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Boolean(bool),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    UnknownDirective,
    InvalidTarget,
    InvalidDirectiveBinaryOperand,
    InvalidDirectiveBinaryOp,
    InvalidDirectiveUnaryOperand,
    InvalidDirectiveUnaryOp,
    UnknownDirectiveVariable,
    InvalidDirectiveExpression,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDirective => write!(f, "this is not a known compiler directive"),
            Self::InvalidTarget => write!(
                f,
                "this is not a valid target and can't be used to conditionally compile"
            ),
            Self::InvalidDirectiveBinaryOperand | Self::InvalidDirectiveUnaryOperand => write!(
                f,
                "this compiler directive operand is not valid for its operator"
            ),
            Self::InvalidDirectiveBinaryOp | Self::InvalidDirectiveUnaryOp => {
                write!(f, "this compiler directive operand is not allowed")
            }
            Self::UnknownDirectiveVariable => {
                write!(f, "this is not a known compiler directive variable")
            }
            Self::InvalidDirectiveExpression => write!(
                f,
                "this type of expression is not allowed in a compiler directive"
            ),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {
    fn notes(&self) -> Vec<String> {
        match self {
            Self::UnknownDirective => vec!["only \"if\" is a valid compiler directive".to_string()],
            Self::InvalidTarget => {
                vec!["try passing the \"--list-targets\" flag to see all valid targets".to_string()]
            }
            Self::InvalidDirectiveBinaryOp => {
                vec!["you may only use \"==\", \"!=\", \"&&\", or \"||\"".to_string()]
            }
            Self::InvalidDirectiveUnaryOp => vec!["you may only use \"!\"".to_string()],
            Self::UnknownDirectiveVariable => {
                vec!["only \"target\" is a valid compiler directive variable".to_string()]
            }
            Self::InvalidDirectiveBinaryOperand
            | Self::InvalidDirectiveUnaryOperand
            | Self::InvalidDirectiveExpression => vec![],
        }
    }
}

fn expand_item(mut ast: &mut Ast, target: Target, item: ItemIndex) -> Result<(), Spanned<Error>> {
    match ast[item].kind() {
        Item::Directive {
            name,
            condition,
            item: directive_item,
        } => match name.kind().as_str() {
            "if" => {
                if matches!(interpret(ast, target, *condition)?, Value::Boolean(true)) {
                    ast[item] = ast[*directive_item].clone();
                }

                Ok(())
            }
            _ => Err(Spanned::new(Error::UnknownDirective, name.span())),
        },
        Item::Primitive(_)
        | Item::NativeFn { .. }
        | Item::Product { .. }
        | Item::Sum { .. }
        | Item::Mod { .. }
        | Item::Teach { .. }
        | Item::Fn { .. } => Ok(()),
    }
}

fn interpret(ast: &Ast, target: Target, expr: ExprIndex) -> Result<Value, Spanned<Error>> {
    match ast[expr].kind() {
        Expr::Binary { op, lhs, rhs } => {
            let lhs_value = interpret(ast, target, *lhs)?;
            let rhs_value = interpret(ast, target, *rhs)?;

            match (op, lhs_value, rhs_value) {
                (BinaryOp::Equal, lhs_value, rhs_value) => {
                    Ok(Value::Boolean(lhs_value == rhs_value))
                }
                (BinaryOp::NotEqual, lhs_value, rhs_value) => {
                    Ok(Value::Boolean(lhs_value != rhs_value))
                }
                (BinaryOp::And, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                    Ok(Value::Boolean(lhs && rhs))
                }
                (BinaryOp::Or, Value::Boolean(lhs), Value::Boolean(rhs)) => {
                    Ok(Value::Boolean(lhs || rhs))
                }
                (BinaryOp::And | BinaryOp::Or, Value::Boolean(_), _) => Err(Spanned::new(
                    Error::InvalidDirectiveBinaryOperand,
                    ast[*rhs].span(),
                )),
                (BinaryOp::And | BinaryOp::Or, _, Value::Boolean(_)) => Err(Spanned::new(
                    Error::InvalidDirectiveBinaryOperand,
                    ast[*lhs].span(),
                )),
                _ => Err(Spanned::new(
                    Error::InvalidDirectiveBinaryOp,
                    ast[expr].span(),
                )),
            }
        }
        Expr::Unary { op, expr } => {
            let operand = interpret(ast, target, *expr)?;

            match (op, operand) {
                (UnaryOp::Not, Value::Boolean(value)) => Ok(Value::Boolean(!value)),
                (UnaryOp::Not, _) => Err(Spanned::new(
                    Error::InvalidDirectiveUnaryOperand,
                    ast[*expr].span(),
                )),
                _ => Err(Spanned::new(
                    Error::InvalidDirectiveUnaryOp,
                    ast[*expr].span(),
                )),
            }
        }
        Expr::Boolean(value) => Ok(Value::Boolean(*value)),
        Expr::Group(expr) => interpret(ast, target, *expr),
        Expr::Name(name) => match name.as_str() {
            "target" => Ok(Value::String(match target {
                Target::Vm => "vm".to_string(),
            })),
            _ => Err(Spanned::new(
                Error::UnknownDirectiveVariable,
                ast[expr].span(),
            )),
        },
        Expr::String(value) => Ok(Value::String(value.clone())),
        Expr::Integer(_)
        | Expr::NegativeInteger(_)
        | Expr::Float(_)
        | Expr::Unit
        | Expr::BinaryNoLhs { .. }
        | Expr::Block(_)
        | Expr::Let { .. }
        | Expr::If { .. }
        | Expr::CallNoCallee(_)
        | Expr::MethodCall { .. }
        | Expr::Call { .. }
        | Expr::While { .. }
        | Expr::Return(_)
        | Expr::AsUnitNoValue
        | Expr::AsUnit(_)
        | Expr::Product { .. }
        | Expr::PathElement(_)
        | Expr::SelfType
        | Expr::ProductNoName(_)
        | Expr::Match { .. } => Err(Spanned::new(
            Error::InvalidDirectiveExpression,
            ast[expr].span(),
        )),
    }
}
