A foundational parser for whitespace-sensitive tree expression languages.

# Description

Parses a tree syntax into an AST structure.

```rust
use src_ctx::{SourceMap, Origin, normalize};
use treelang::{Tree, Indent};

let source = normalize("
    |directive a: b
    |  first:
    |    statement x 23
    |  second:
    |    statement x 42
");

let mut map = SourceMap::new();
let index = map.insert(Origin::from_named("example"), source.into())
    .try_into_inserted()
    .expect("single source should not conflict");

let indent = Indent::spaces(2);
let input = map.input(index);
let result = Tree::parse(input, indent);

assert!(result.is_ok());
```

# Syntax

All nodes (statements or directives) must fit on a single line.

## Statements

Are a whitespace-separated list of items.

Statements cannot have child nodes.

## Directives

Have the following form:

```plaintext
<signature> : <arguments>
```

Where `<signature>` is a non-empty list of items, and `<arguments>` is a possibly
empty list of items.

Directives can have child nodes.

## Items

* Numbers (floats and integers).
* Words (a collection of non-structural non-whitespace characters).
* Groups
  * Parenthesized lists of items `(...)`.
  * Bracketed lists of items `[...]`.
  * Braced lists of items `{...}`.