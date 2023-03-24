use smol_str::SmolStr;

use crate::{
    Tree, Span, Offset, Item, Node, NodeKind, ItemKind, Statement, Directive,
    SectionDisplay, SourceContext,
};

use self::input::Input;


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
pub type ParseResult<T = ()> = Result<T, ParseError>;

type GroupWrapper = fn(Vec<Item>) -> ItemKind;
type Group = (char, char, GroupWrapper);

/// Errors encountered during [`Tree::parse`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid indentation characters on line {}", span.line_number())]
    IndentChars { span: Span },
    #[error("Invalid indentation depth on line {}", offset.line_number())]
    IndentDepth { offset: Offset },
    #[error("Child node attached to statement on line {}", child_offset.line_number())]
    StatementWithChild { child_offset: Offset },
    #[error("Unexpected character `{unexpected}` on line {}", offset.line_number())]
    UnexpectedChar { offset: Offset, unexpected: char },
    #[error("Missing closing `{missing}` character on line {}", open_offset.line_number())]
    UnclosedGroup { open_offset: Offset, missing: char },
    #[error("Invalid integer format `{value}` on line {}", span.line_number())]
    InvalidInt { span: Span, value: SmolStr },
    #[error("Invalid floating point format `{value}` on line {}", span.line_number())]
    InvalidFloat { span: Span, value: SmolStr },
    #[error("Empty directive signature on line {}", offset.line_number())]
    EmptyDirectiveSignature { offset: Offset },
}

impl ParseError {
    /// Produce the corresponding [`SectionDisplay`] for the error.
    pub fn section_display<'a>(&self, source: &'a str) -> SectionDisplay<'a> {
        match *self {
            Self::IndentChars { span } |
            Self::InvalidInt { span, .. } |
            Self::InvalidFloat { span, .. } => {
                source.span_section_display(span)
            },
            Self::IndentDepth { offset } |
            Self::StatementWithChild { child_offset: offset } |
            Self::UnexpectedChar { offset, .. } |
            Self::UnclosedGroup { open_offset: offset, .. } |
            Self::EmptyDirectiveSignature { offset } => {
                source.offset_section_display(offset)
            },
        }
    }
}

pub(crate) fn parse_str(content: &str, indent: Indent) -> ParseResult<Tree> {
    let mut stack = DepthStack::default();
    let mut input = Some(Input::new(content));
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
            } else if let Some(rest) = input.try_skip_char(':') {
                if items.is_empty() {
                    Err(ParseError::EmptyDirectiveSignature { offset: node_offset })
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
                Err(ParseError::UnclosedGroup { open_offset, missing: end })
            } else if let Some(rest) = input.try_skip_char(end) {
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
        Ok((Item { location: input.offset().span(1), kind: wrap_kind(items) }, rest))
    } else if let Some((value, span, rest)) = input.try_take_chars(|c| !is_structure_char(c)) {
        if value.starts_with(|c: char| c.is_ascii_digit()) || value.starts_with('-') {
            if value.contains('.') {
                if let Some(value) = value.parse().ok() {
                    Ok((Item { location: span, kind: ItemKind::Float(value) }, rest))
                } else {
                    Err(ParseError::InvalidFloat { value: value.into(), span })
                }
            } else {
                if let Some(value) = value.parse().ok() {
                    Ok((Item { location: span, kind: ItemKind::Int(value) }, rest))
                } else {
                    Err(ParseError::InvalidInt { value: value.into(), span })
                }
            }
        } else {
            Ok((Item { location: span, kind: ItemKind::Word(value.into()) }, rest))
        }
    } else {
        Err(ParseError::UnexpectedChar {
            offset: input.offset(),
            unexpected: input.next_char().expect("empty input reached `parse_item`"),
        })
    }
}

fn try_skip_group_open<'a>(input: &Input<'a>) -> Option<(Input<'a>, Group)> {
    for &group in token::PAIRS {
        let (open, ..) = group;
        if let Some(rest) = input.try_skip_char(open) {
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
            return Err(ParseError::IndentDepth { offset: node.location });
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
                        return Err(ParseError::StatementWithChild { child_offset: node.location });
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
    pub fn tabs() -> Self {
        Self { width: IndentWidth::Tabs }
    }

    /// Indentation by `count` spaces.
    ///
    /// Returns `None` if the number of spaces is 0.
    pub fn spaces(count: u8) -> Option<Self> {
        (count > 0).then_some(Self { width: IndentWidth::Spaces(count) })
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
            Tabs => line.try_skip_char('\t'),
            Spaces(n) => line.try_skip_char_sequence(std::iter::repeat(' ').take(n.into())),
        }
    }

    fn extract<'a>(&self, mut line: Input<'a>) -> ParseResult<(usize, Input<'a>)> {
        let mut depth = 0;
        while let Some(rest) = self.try_deindent(line.clone()) {
            depth += 1;
            line = rest;
        }
        if let Some(span) = line.leading_whitespace_span() {
            Err(ParseError::IndentChars { span })
        } else {
            Ok((depth, line))
        }
    }
}
