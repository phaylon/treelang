#![doc = include_str!("../README.md")]
use smol_str::SmolStr;

pub use parse::*;
use src_ctx::{Input, Offset, Span};


mod parse;

/// A collection of [`Node`] roots.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tree {
    pub roots: Vec<Node>,
}

impl Tree {
    /// Try to parse a tree from a `&str` assuming the given [`Indent`].
    pub fn parse(input: Input<'_>, indent: Indent) -> ParseResult<Self> {
        parse_input(input, indent)
    }
}

impl std::ops::Deref for Tree {
    type Target = Vec<Node>;

    fn deref(&self) -> &Self::Target {
        &self.roots
    }
}

impl std::ops::DerefMut for Tree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.roots
    }
}

macro_rules! fn_enum_is_variant {
    ($name:ident, $variant:ident $(,)?) => {
        pub fn $name(&self) -> bool {
            matches!(self, Self::$variant { .. })
        }
    }
}

macro_rules! fn_enum_try_into_variant {
    ($name:ident, $variant:ident, $output:ty $(,)?) => {
        pub fn $name(self) -> Result<$output, Self> {
            if let Self::$variant(value) = self {
                Ok(value)
            } else {
                Err(self)
            }
        }
    }
}

macro_rules! fn_enum_variant_access {
    ($name:ident -> $output:ty, $variant:pat => $access:expr) => {
        pub fn $name(&self) -> Option<$output> {
            if let $variant = self {
                Some($access)
            } else {
                None
            }
        }
    }
}

/// A parsed node in a [`Tree`] with a specific [`NodeKind`].
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub location: Offset,
}

impl Node {
    /// Get a slice of children independent of the [`NodeKind`].
    ///
    /// Statements will produce an empty slice.
    pub fn children(&self) -> &[Self] {
        match &self.kind {
            NodeKind::Directive(directive) => &directive.children,
            NodeKind::Statement(_) => &[],
        }
    }
}

impl std::ops::Deref for Node {
    type Target = NodeKind;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

/// Data for a [`NodeKind::Directive`] in a [`Tree`].
#[derive(Debug, Clone, PartialEq)]
pub struct Directive {
    pub signature: Vec<Item>,
    pub arguments: Vec<Item>,
    pub children: Vec<Node>,
}

/// Data for a [`NodeKind::Statement`] in a [`Tree`].
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub signature: Vec<Item>,
}

/// The different kinds of [`Node`].
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Directive(Directive),
    Statement(Statement),
}

impl NodeKind {
    fn_enum_is_variant!(is_directive, Directive);
    fn_enum_is_variant!(is_statement, Statement);

    fn_enum_try_into_variant!(try_into_directive, Directive, Directive);
    fn_enum_try_into_variant!(try_into_statement, Statement, Statement);

    fn_enum_variant_access!(directive -> &Directive, Self::Directive(directive) => directive);
    fn_enum_variant_access!(statement -> &Statement, Self::Statement(statement) => statement);
}

/// An item of [`ItemKind`] found in a [`Statement`] or [`Directive`].
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub location: Span,
}

impl std::ops::Deref for Item {
    type Target = ItemKind;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

/// The different kinds of [`Item`].
#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Word(SmolStr),
    Int(i32),
    Float(f32),
    Parentheses(Vec<Item>),
    Brackets(Vec<Item>),
    Braces(Vec<Item>),
}

impl ItemKind {
    fn_enum_is_variant!(is_word, Word);
    fn_enum_is_variant!(is_int, Int);
    fn_enum_is_variant!(is_float, Float);
    fn_enum_is_variant!(is_parenthesized, Parentheses);
    fn_enum_is_variant!(is_bracketed, Brackets);
    fn_enum_is_variant!(is_braced, Braces);

    fn_enum_try_into_variant!(try_into_word, Word, SmolStr);
    fn_enum_try_into_variant!(try_into_int, Int, i32);
    fn_enum_try_into_variant!(try_into_float, Float, f32);
    fn_enum_try_into_variant!(try_into_parenthesized, Parentheses, Vec<Item>);
    fn_enum_try_into_variant!(try_into_bracketed, Brackets, Vec<Item>);
    fn_enum_try_into_variant!(try_into_braced, Braces, Vec<Item>);

    fn_enum_variant_access!(word -> &SmolStr, Self::Word(word) => word);
    fn_enum_variant_access!(word_str -> &str, Self::Word(word) => word.as_str());
    fn_enum_variant_access!(int -> i32, Self::Int(value) => *value);
    fn_enum_variant_access!(float -> f32, Self::Float(value) => *value);
    fn_enum_variant_access!(parenthesized -> &[Item], Self::Parentheses(items) => items);
    fn_enum_variant_access!(bracketed -> &[Item], Self::Brackets(items) => items);
    fn_enum_variant_access!(braced -> &[Item], Self::Braces(items) => items);
}
