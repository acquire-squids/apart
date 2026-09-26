use crate::{
    Reportable, Span, Spanned,
    lex::{Error as LexError, Lexer, Token},
};

use std::{error, fmt};

pub struct Forest {
    trees: Vec<TokenTree>,
    root: TokenTreeIndex,
}

impl Forest {
    #[allow(dead_code)]
    #[must_use]
    pub const fn root(&self) -> TokenTreeIndex {
        self.root
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_tree(&self, token_tree_index: TokenTreeIndex) -> Option<&TokenTree> {
        self.trees.get(usize::from(token_tree_index))
    }
}

#[derive(Debug)]
pub enum TokenTree {
    Token(Spanned<Token>),
    Tree {
        span: Span,
        kind: TreeKind,
        outer: Option<TokenTreeIndex>,
        tokens: Vec<TokenTreeIndex>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenTreeIndex(usize);

impl From<TokenTreeIndex> for usize {
    fn from(value: TokenTreeIndex) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeKind {
    Parentheses,
    Brackets,
    SquareBrackets,
    Invisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    Lex(LexError),
    Mismatch { expected: TreeKind, got: TreeKind },
    Missing(TreeKind),
    Orphaned(TreeKind),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lex(error) => write!(f, "{error}"),
            Self::Mismatch { expected, got } => write!(
                f,
                "mismatched closing delimiter: expected a \"{}\", but got a \"{}\"",
                match expected {
                    TreeKind::Parentheses => ")",
                    TreeKind::Brackets => "}",
                    TreeKind::SquareBrackets => "]",
                    TreeKind::Invisible =>
                        unreachable!("invisibile delimiters are always balanced"),
                },
                match got {
                    TreeKind::Parentheses => ")",
                    TreeKind::Brackets => "}",
                    TreeKind::SquareBrackets => "]",
                    TreeKind::Invisible =>
                        unreachable!("invisibile delimiters are always balanced"),
                }
            ),
            Self::Missing(expected) => write!(
                f,
                "missing closing delimiter: expected \"{}\"",
                match expected {
                    TreeKind::Parentheses => ")",
                    TreeKind::Brackets => "}",
                    TreeKind::SquareBrackets => "]",
                    TreeKind::Invisible =>
                        unreachable!("invisibile delimiters are always balanced"),
                }
            ),
            Self::Orphaned(kind) => write!(
                f,
                "orphaned delimiter: there is not a matching \"{}\"",
                match kind {
                    TreeKind::Parentheses => "(",
                    TreeKind::Brackets => "{",
                    TreeKind::SquareBrackets => "[",
                    TreeKind::Invisible =>
                        unreachable!("invisibile delimiters are always balanced"),
                }
            ),
        }
    }
}

impl error::Error for Error {}

impl Reportable for Error {}

#[allow(clippy::too_many_lines)]
pub fn tokens_to_token_trees(lexer: &mut Lexer) -> Result<Forest, Vec<Spanned<Error>>> {
    let mut trees = vec![TokenTree::Tree {
        // TODO: is it okay to use this span?
        span: Span::new(lexer.source_id(), 0, 0),
        kind: TreeKind::Invisible,
        outer: None,
        tokens: vec![],
    }];

    let root = TokenTreeIndex(0);

    let mut stack = vec![root];

    let mut errors = vec![];

    for maybe_token in lexer {
        match maybe_token {
            Err(error) => {
                errors.push(error.transmute(Error::Lex));
            }
            Ok(token) => match token.kind() {
                Token::OpenParenthesis => {
                    trees.push(TokenTree::Tree {
                        span: token.span(),
                        kind: TreeKind::Parentheses,
                        outer: stack.last().copied(),
                        tokens: vec![],
                    });

                    stack.push(TokenTreeIndex(trees.len() - 1));
                }
                Token::OpenBracket => {
                    trees.push(TokenTree::Tree {
                        span: token.span(),
                        kind: TreeKind::Brackets,
                        outer: stack.last().copied(),
                        tokens: vec![],
                    });

                    stack.push(TokenTreeIndex(trees.len() - 1));
                }
                Token::OpenSquareBracket => {
                    trees.push(TokenTree::Tree {
                        span: token.span(),
                        kind: TreeKind::SquareBrackets,
                        outer: stack.last().copied(),
                        tokens: vec![],
                    });

                    stack.push(TokenTreeIndex(trees.len() - 1));
                }
                Token::CloseParenthesis => {
                    if let Some((token_tree_index, TokenTree::Tree { span, kind, .. })) =
                        stack.pop().and_then(|token_tree_index| {
                            trees
                                .get_mut(usize::from(token_tree_index))
                                .map(|tree| (token_tree_index, tree))
                        })
                    {
                        if matches!(kind, TreeKind::Parentheses) {
                            *span = span
                                .combine_with(token.span())
                                .expect("these spans are from the same source");

                            if let Some(TokenTree::Tree {
                                tokens: outer_tokens,
                                ..
                            }) = stack.last().and_then(|token_tree_index| {
                                trees.get_mut(usize::from(*token_tree_index))
                            }) {
                                outer_tokens.push(token_tree_index);
                            } else {
                                stack.push(token_tree_index);
                            }
                        } else {
                            errors.push(Spanned::new(
                                Error::Mismatch {
                                    expected: *kind,
                                    got: TreeKind::Parentheses,
                                },
                                token.span(),
                            ));
                        }
                    } else {
                        errors.push(Spanned::new(
                            Error::Orphaned(TreeKind::Parentheses),
                            token.span(),
                        ));
                    }
                }
                Token::CloseBracket => {
                    if let Some((token_tree_index, TokenTree::Tree { span, kind, .. })) =
                        stack.pop().and_then(|token_tree_index| {
                            trees
                                .get_mut(usize::from(token_tree_index))
                                .map(|tree| (token_tree_index, tree))
                        })
                    {
                        if matches!(kind, TreeKind::Brackets) {
                            *span = span
                                .combine_with(token.span())
                                .expect("these spans are from the same source");

                            if let Some(TokenTree::Tree {
                                tokens: outer_tokens,
                                ..
                            }) = stack.last().and_then(|token_tree_index| {
                                trees.get_mut(usize::from(*token_tree_index))
                            }) {
                                outer_tokens.push(token_tree_index);
                            } else {
                                stack.push(token_tree_index);
                            }
                        } else {
                            errors.push(Spanned::new(
                                Error::Mismatch {
                                    expected: *kind,
                                    got: TreeKind::Brackets,
                                },
                                token.span(),
                            ));
                        }
                    } else {
                        errors.push(Spanned::new(
                            Error::Orphaned(TreeKind::Brackets),
                            token.span(),
                        ));
                    }
                }
                Token::CloseSquareBracket => {
                    if let Some((token_tree_index, TokenTree::Tree { span, kind, .. })) =
                        stack.pop().and_then(|token_tree_index| {
                            trees
                                .get_mut(usize::from(token_tree_index))
                                .map(|tree| (token_tree_index, tree))
                        })
                    {
                        if matches!(kind, TreeKind::SquareBrackets) {
                            *span = span
                                .combine_with(token.span())
                                .expect("these spans are from the same source");

                            if let Some(TokenTree::Tree {
                                tokens: outer_tokens,
                                ..
                            }) = stack.last().and_then(|token_tree_index| {
                                trees.get_mut(usize::from(*token_tree_index))
                            }) {
                                outer_tokens.push(token_tree_index);
                            } else {
                                stack.push(token_tree_index);
                            }
                        } else {
                            errors.push(Spanned::new(
                                Error::Mismatch {
                                    expected: *kind,
                                    got: TreeKind::SquareBrackets,
                                },
                                token.span(),
                            ));
                        }
                    } else {
                        errors.push(Spanned::new(
                            Error::Orphaned(TreeKind::SquareBrackets),
                            token.span(),
                        ));
                    }
                }
                _ => {
                    trees.push(TokenTree::Token(token));

                    let token_tree_index = TokenTreeIndex(trees.len() - 1);

                    if let Some(TokenTree::Tree {
                        tokens: outer_tokens,
                        ..
                    }) = stack
                        .last()
                        .and_then(|token_tree_index| trees.get_mut(usize::from(*token_tree_index)))
                    {
                        outer_tokens.push(token_tree_index);
                    } else {
                        stack.push(token_tree_index);
                    }
                }
            },
        }
    }

    while stack.len() > 1
        && let Some(tree) = stack
            .pop()
            .and_then(|token_tree_index| trees.get(usize::from(token_tree_index)))
    {
        if let TokenTree::Tree { span, kind, .. } = tree {
            errors.push(Spanned::new(Error::Missing(*kind), *span));
        }
    }

    if errors.is_empty() {
        Ok(Forest { trees, root })
    } else {
        Err(errors)
    }
}
