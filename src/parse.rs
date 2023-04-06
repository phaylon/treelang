use smol_str::SmolStr;
use src_ctx::{Input, SourceError, Offset};

use crate::{
    Tree, Item, Node, NodeKind, ItemKind, Statement, Directive,
};

use self::input::InputExt;


mod input;

mod token {
    use super::{ItemKind, Group};

    pub const COMMENT: char = ';';
    pub const DIRECTIVE: char = ':';
    pub const PARENTHESIS_OPEN: char = '(';
    pub const PARENTHESIS_CLOSE: char = ')';
    pub const BRACKET_OPEN: char = '[';
    pub const BRACKET_CLOSE: char = ']';
    pub const BRACE_OPEN: char = '{';
    pub const BRACE_CLOSE: char = '}';

    pub const PAIRS: &[Group] = &[
        (PARENTHESIS_OPEN, PARENTHESIS_CLOSE, ItemKind::Parentheses),
        (BRACKET_OPEN, BRACKET_CLOSE, ItemKind::Brackets),
        (BRACE_OPEN, BRACE_CLOSE, ItemKind::Braces),
    ];

    pub const ALL: &[char] = &[
        COMMENT, DIRECTIVE, PARENTHESIS_OPEN, PARENTHESIS_CLOSE, BRACKET_OPEN, BRACKET_CLOSE,
        BRACE_OPEN, BRACE_CLOSE,
    ];
}

/// Type alias for [`Result`] with [`ParseError`].
pub type ParseResult<T = ()> = Result<T, SourceError<ParseError>>;

type GroupWrapper = fn(Vec<Item>) -> ItemKind;
type Group = (char, char, GroupWrapper);

/// Errors encountered during [`Tree::parse`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid indentation characters")]
    IndentChars,
    #[error("Invalid indentation depth")]
    IndentDepth,
    #[error("Statement has an unexpected child node")]
    StatementWithChild,
    #[error("Unexpected character `{unexpected}`")]
    UnexpectedChar { unexpected: char },
    #[error("Missing closing `{missing}` character")]
    UnclosedGroup { missing: char },
    #[error("Invalid integer format `{value}`")]
    InvalidInt { value: SmolStr },
    #[error("Invalid floating point format `{value}`")]
    InvalidFloat { value: SmolStr },
    #[error("Empty directive signature")]
    EmptyDirectiveSignature,
}

pub(crate) fn parse_input(input: Input<'_>, indent: Indent) -> ParseResult<Tree> {
    let mut stack = DepthStack::default();
    let mut input = Some(input);
    while let Some(current) = input.take() {
        let (line, rest) = current.split_line();
        input = rest;

        if line.skip_whitespace_and_comments().is_empty() {
            continue;
        }

        let (depth, line) = indent.extract(line)?;
        let node = parse_node(line)?;
        stack.insert(depth, node)?;
    }
    stack.into_tree()
}

fn parse_node(mut input: Input<'_>) -> ParseResult<Node> {
    let node_offset = input.offset();
    let mut items = Vec::new();
    'items: loop {
        input = input.skip_whitespace_and_comments();
        return {
            if input.is_empty() {
                Ok(Node {
                    location: node_offset,
                    kind: NodeKind::Statement(Statement {
                        signature: items,
                    }),
                })
            } else if let Some(rest) = input.skip_char(':') {
                if items.is_empty() {
                    Err(SourceError::new(
                        ParseError::EmptyDirectiveSignature,
                        node_offset,
                        "empty directive",
                    ))
                } else {
                    let arguments = parse_all_items(rest)?;
                    Ok(Node {
                        location: node_offset,
                        kind: NodeKind::Directive(Directive {
                            signature: items,
                            arguments,
                            children: Vec::new(),
                        }),
                    })
                }
            } else {
                let (item, rest) = parse_item(input)?;
                items.push(item);
                input = rest;
                continue 'items;
            }
        }
    }
}

fn parse_items_until(
    mut input: Input<'_>,
    end: char,
    open_offset: Offset,
) -> ParseResult<(Vec<Item>, Input<'_>)> {
    let mut items = Vec::new();
    'items: loop {
        input = input.skip_whitespace_and_comments();
        return {
            if input.is_empty() {
                Err(SourceError::new(
                    ParseError::UnclosedGroup { missing: end },
                    open_offset,
                    "opened here",
                ))
            } else if let Some(rest) = input.skip_char(end) {
                Ok((items, rest))
            } else {
                let (item, rest) = parse_item(input)?;
                items.push(item);
                input = rest;
                continue 'items;
            }
        };
    }
}

fn parse_all_items(mut input: Input<'_>) -> ParseResult<Vec<Item>> {
    let mut items = Vec::new();
    'items: loop {
        input = input.skip_whitespace_and_comments();
        return {
            if input.is_empty() {
                Ok(items)
            } else {
                let (item, rest) = parse_item(input)?;
                items.push(item);
                input = rest;
                continue 'items;
            }
        };
    }
}

fn parse_item(input: Input<'_>) -> ParseResult<(Item, Input<'_>)> {
    if let Some((rest, (_, close, wrap_kind))) = try_skip_group_open(&input) {
        let (items, rest) = parse_items_until(rest, close, input.offset())?;
        let location = input.offset().span(input.skip(1).offset());
        Ok((Item { location, kind: wrap_kind(items) }, rest))
    } else if let Some((value, span, rest)) = input.try_take_chars(|c| !is_structure_char(c)) {
        if value.starts_with(|c: char| c.is_ascii_digit()) || value.starts_with('-') {
            if value.contains('.') {
                if let Some(value) = value.parse().ok() {
                    Ok((Item { location: span, kind: ItemKind::Float(value) }, rest))
                } else {
                    Err(SourceError::new(
                        ParseError::InvalidFloat { value: value.into() },
                        span.start(),
                        "expected valid float",
                    ))
                }
            } else {
                if let Some(value) = value.parse().ok() {
                    Ok((Item { location: span, kind: ItemKind::Int(value) }, rest))
                } else {
                    Err(SourceError::new(
                        ParseError::InvalidInt { value: value.into() },
                        span.start(),
                        "expected valid int",
                    ))
                }
            }
        } else {
            Ok((Item { location: span, kind: ItemKind::Word(value.into()) }, rest))
        }
    } else {
        Err(SourceError::new(
            ParseError::UnexpectedChar { unexpected: input.char().unwrap() },
            input.offset(),
            "parse error",
        ))
    }
}

fn try_skip_group_open<'a>(input: &Input<'a>) -> Option<(Input<'a>, Group)> {
    for &group in token::PAIRS {
        let (open, ..) = group;
        if let Some(rest) = input.skip_char(open) {
            return Some((rest, group));
        }
    }
    None
}

fn is_structure_char(c: char) -> bool {
    c.is_whitespace() || token::ALL.contains(&c)
}

#[derive(Default)]
struct DepthStack {
    tree: Tree,
    levels: Vec<Node>,
}

impl DepthStack {
    fn into_tree(mut self) -> ParseResult<Tree> {
        self.vacate_level(0)?;
        Ok(self.tree)
    }

    fn insert(&mut self, depth: usize, node: Node) -> ParseResult {
        self.vacate_level(depth)?;
        if depth != self.levels.len() {
            let mut error = SourceError::new(
                ParseError::IndentDepth,
                node.location,
                "invalid indentation",
            );
            if let Some(nearest) = self.levels.last() {
                error = error.with_context(nearest.location);
            }
            return Err(error);
        }
        self.levels.push(node);
        Ok(())
    }

    fn vacate_level(&mut self, depth: usize) -> ParseResult {
        while self.levels.len() > depth {
            let node = self.levels.pop().unwrap();
            if let Some(parent) = self.levels.last_mut() {
                match &mut parent.kind {
                    NodeKind::Directive(Directive { children, .. }) => {
                        children.push(node);
                    },
                    NodeKind::Statement(_) => {
                        let error = SourceError::new(
                            ParseError::StatementWithChild,
                            node.location,
                            "child node",
                        );
                        let error = error.with_context(parent.location);
                        return Err(error);
                    },
                }
            } else {
                self.tree.roots.push(node);
            }
        }
        Ok(())
    }
}

/// Indentation setting for [`Tree::parse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indent {
    width: IndentWidth,
}

impl Indent {
    /// Indentation by a single tab character.
    pub const fn tabs() -> Self {
        Self { width: IndentWidth::Tabs }
    }

    /// Indentation by `count` spaces.
    ///
    /// Returns `None` if the number of spaces is 0.
    pub const fn try_spaces(count: u8) -> Option<Self> {
        if count > 0 {
            Some(Self { width: IndentWidth::Spaces(count) })
        } else {
            None
        }
    }

    /// Indentation by `count` spaces.
    ///
    /// Panics if the number of spaces is 0.
    pub const fn spaces(count: u8) -> Self {
        if count == 0 {
            panic!("zero-width indentation specified");
        }
        Self { width: IndentWidth::Spaces(count) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndentWidth {
    Tabs,
    Spaces(u8),
}

impl Indent {
    fn try_deindent<'a>(&self, line: Input<'a>) -> Option<Input<'a>> {
        use IndentWidth::*;
        match self.width {
            Tabs => line.skip_char('\t'),
            Spaces(n) => {
                let mut line = line;
                for _ in 0..n {
                    line = line.skip_char(' ')?;
                }
                Some(line)
            },
        }
    }

    fn extract<'a>(&self, mut line: Input<'a>) -> ParseResult<(usize, Input<'a>)> {
        let mut depth = 0;
        while let Some(rest) = self.try_deindent(line.clone()) {
            depth += 1;
            line = rest;
        }
        if line.content().starts_with(|c: char| c.is_whitespace()) {
            Err(SourceError::new(
                ParseError::IndentChars,
                line.offset(),
                "non-indentation whitespace",
            ))
        } else {
            Ok((depth, line))
        }
    }
}
