use src_ctx::{SourceMap, Origin, normalize};
use treelang::{ParseResult, Indent, Tree, ParseError, Statement, Directive};
use assert_matches::assert_matches;


fn parse(content: &str) -> ParseResult<(Tree, SourceMap)> {
    let mut map = SourceMap::default();
    let index = map.insert(Origin::from_named("test-source"), content.into())
        .try_into_inserted().unwrap();
    let input = map.input(index);
    Tree::parse(input, Indent::try_spaces(2).unwrap()).map(|tree| (tree, map))
}

macro_rules! assert_parsed {
    ($map:ident = $content:expr, $($rest:tt)*) => {
        {
            let content = normalize($content);
            let mut map = SourceMap::new();
            let index = map.insert(Origin::from_named("test-source"), content.into())
                .try_into_inserted().unwrap();
            let input = map.input(index);
            let result = Tree::parse(input, Indent::try_spaces(2).unwrap())
                .map_err(|error| error.into_error());
            #[allow(unused_variables)]
            let $map = map;
            assert_matches!(result, $($rest)*)
        }
    }
}

macro_rules! assert_tree_test_items {
    ($tree:expr, $($rest:tt)*) => {
        {
            let tree = $tree;
            assert_matches!(&tree[..], [node] => {
                assert_matches!(node.statement(), Some(Statement { signature }) => {
                    assert_matches!(&signature[..], $($rest)*)
                })
            })
        }
    }
}

#[test]
fn trees() {
    let (tree, map) = parse(&normalize("
        |abc:
        |  def:
        |
        |    ghi
        |  jkl
    ")).unwrap();

    assert_matches!(&tree[..], [node_abc] => {

        assert!(node_abc.is_directive());
        assert_eq!(map.byte_offset_on_line(node_abc.location), 0);
        assert_matches!(node_abc.children(), [node_def, node_jkl] => {

            assert!(node_def.is_directive());
            assert_eq!(map.byte_offset_on_line(node_def.location), 2);
            assert_matches!(node_def.children(), [node_ghi] => {

                assert!(node_ghi.is_statement());
                assert_eq!(map.byte_offset_on_line(node_ghi.location), 4);
            });

            assert!(node_jkl.is_statement());
            assert_eq!(map.byte_offset_on_line(node_jkl.location), 2);
        });
    });

    assert_parsed!(source = "|  abc", Err(ParseError::IndentDepth));
    assert_parsed!(source = "|     abc", Err(ParseError::IndentChars));
}

#[test]
fn statements() {
    let (mut tree, map) = parse("abc 23").unwrap();
    assert_eq!(tree.len(), 1);

    let stmt = tree.remove(0).kind.try_into_statement().unwrap();
    assert_matches!(&stmt.signature[..], [item_abc, item_23] => {
        assert_eq!(item_abc.word_str(), Some("abc"));
        assert_eq!(map.span_str(item_abc.location), "abc");

        assert_eq!(item_23.int(), Some(23));
        assert_eq!(map.span_str(item_23.location), "23");
    });

    assert_parsed!(source = "|abc\n|  def", Err(ParseError::StatementWithChild));
}

#[test]
fn directives() {
    let (mut tree, map) = parse("abc def: ghi jkl").unwrap();
    assert_eq!(tree.len(), 1);

    let dir = tree.remove(0).kind.try_into_directive().unwrap();
    assert_eq!(dir.children.len(), 0);

    assert_matches!(&dir.signature[..], [item_abc, item_def] => {
        assert_eq!(item_abc.word_str(), Some("abc"));
        assert_eq!(map.span_str(item_abc.location), "abc");

        assert_eq!(item_def.word_str(), Some("def"));
        assert_eq!(map.span_str(item_def.location), "def");
    });

    assert_matches!(&dir.arguments[..], [item_ghi, item_jkl] => {
        assert_eq!(item_ghi.word_str(), Some("ghi"));
        assert_eq!(map.span_str(item_ghi.location), "ghi");

        assert_eq!(item_jkl.word_str(), Some("jkl"));
        assert_eq!(map.span_str(item_jkl.location), "jkl");
    });

    assert_parsed!(
        source = "|abc: def: ghi",
        Err(ParseError::UnexpectedChar { unexpected: ':' })
    );

    assert_parsed!(
        source = "
            |abc:
            |  :def
        ",
        Err(ParseError::EmptyDirectiveSignature)
    );
}

#[test]
fn comments() {
    let (tree, _) = parse(&normalize("
        |    ;comment
        |abc;comment
        |def:ghi;comment
    ")).unwrap();

    assert_matches!(&tree[..], [stmt, dir] => {
        assert_matches!(stmt.statement(), Some(Statement { signature }) => {
            assert_eq!(signature.len(), 1);
            assert_eq!(signature[0].word_str(), Some("abc"));
        });
        assert_matches!(dir.directive(), Some(Directive { signature, arguments, .. }) => {
            assert_eq!(signature.len(), 1);
            assert_eq!(signature[0].word_str(), Some("def"));
            assert_eq!(arguments.len(), 1);
            assert_eq!(arguments[0].word_str(), Some("ghi"));
        });
    });
}

#[test]
fn words() {
    for value in ["a", "a_b", "a-b", "$a$", "a.b", "a23", "+", "&", "/"] {
        let (tree, map) = parse(&format!("test {}", value)).unwrap();
        let item = assert_tree_test_items!(&tree, [_, item] => item);

        assert!(item.is_word());
        assert_eq!(map.span_str(item.location), value);
        assert_eq!(item.word_str(), Some(value));
        assert_eq!(item.word(), Some(&value.into()));
        assert_eq!(item.clone().kind.try_into_word(), Ok(value.into()));
    }
}

#[test]
fn ints() {
    for (value, int_value) in [("0", 0), ("23", 23), ("-0", -0), ("-23", -23)] {
        let (tree, map) = parse(&format!("test {}", value)).unwrap();
        let item = assert_tree_test_items!(&tree, [_, item] => item);

        assert!(item.is_int());
        assert_eq!(map.span_str(item.location), value);
        assert_eq!(item.int(), Some(int_value));
        assert_eq!(item.clone().kind.try_into_int(), Ok(int_value));
    }

    assert_parsed!(
        source = "|test 23abc",
        Err(ParseError::InvalidInt { value }) => {
            assert_eq!(&value, "23abc");
        }
    );

    assert_parsed!(
        source = "|test -23abc",
        Err(ParseError::InvalidInt { value }) => {
            assert_eq!(&value, "-23abc");
        }
    );
}

#[test]
fn floats() {
    for (value, float_value) in [("0.0", 0.0), ("23.0", 23.0), ("-0.0", -0.0), ("-23.0", -23.0)] {
        let (tree, map) = parse(&format!("test {}", value)).unwrap();
        let item = assert_tree_test_items!(&tree, [_, item] => item);

        assert!(item.is_float());
        assert_eq!(map.span_str(item.location), value);
        assert_eq!(item.float(), Some(float_value));
        assert_eq!(item.clone().kind.try_into_float(), Ok(float_value));
    }

    assert_parsed!(
        source = "|test 23.abc",
        Err(ParseError::InvalidFloat { value }) => {
            assert_eq!(&value, "23.abc");
        }
    );

    assert_parsed!(
        source = "|test -23.abc",
        Err(ParseError::InvalidFloat { value }) => {
            assert_eq!(&value, "-23.abc");
        }
    );
}

#[test]
fn parentheses() {
    assert_parsed!(source = "|test (abc def)", Ok(tree) => {
        let item = assert_tree_test_items!(&tree, [_, item] => item);
        assert!(item.is_parenthesized());
        assert_eq!(source.span_str(item.location), "(");
        assert_matches!(item.parenthesized(), Some([item_abc, item_def]) => {
            assert_eq!(item_abc.word_str(), Some("abc"));
            assert_eq!(source.span_str(item_abc.location), "abc");
            assert_eq!(item_def.word_str(), Some("def"));
            assert_eq!(source.span_str(item_def.location), "def");
        });
        assert_matches!(item.clone().kind.try_into_parenthesized(), Ok(_));
    });
    assert_parsed!(source = "|test ()", Ok(tree) => {
        let item = assert_tree_test_items!(&tree, [_, item] => item);
        assert!(item.is_parenthesized());
        assert_eq!(source.span_str(item.location), "(");
        assert_matches!(item.parenthesized(), Some([]));
        assert_matches!(item.clone().kind.try_into_parenthesized(), Ok(_));
    });
    assert_parsed!(source = "|test (", Err(ParseError::UnclosedGroup { missing }) => {
        assert_eq!(missing, ')');
    });
    assert_parsed!(source = "|test )", Err(ParseError::UnexpectedChar { unexpected }) => {
        assert_eq!(unexpected, ')');
    });
}

#[test]
fn brackets() {
    assert_parsed!(source = "|test [abc def]", Ok(tree) => {
        let item = assert_tree_test_items!(&tree, [_, item] => item);
        assert!(item.is_bracketed());
        assert_eq!(source.span_str(item.location), "[");
        assert_matches!(item.bracketed(), Some([item_abc, item_def]) => {
            assert_eq!(item_abc.word_str(), Some("abc"));
            assert_eq!(source.span_str(item_abc.location), "abc");
            assert_eq!(item_def.word_str(), Some("def"));
            assert_eq!(source.span_str(item_def.location), "def");
        });
        assert_matches!(item.clone().kind.try_into_bracketed(), Ok(_));
    });
    assert_parsed!(source = "|test []", Ok(tree) => {
        let item = assert_tree_test_items!(&tree, [_, item] => item);
        assert!(item.is_bracketed());
        assert_eq!(source.span_str(item.location), "[");
        assert_matches!(item.bracketed(), Some([]));
        assert_matches!(item.clone().kind.try_into_bracketed(), Ok(_));
    });
    assert_parsed!(source = "|test [", Err(ParseError::UnclosedGroup { missing }) => {
        assert_eq!(missing, ']');
    });
    assert_parsed!(source = "|test ]", Err(ParseError::UnexpectedChar { unexpected }) => {
        assert_eq!(unexpected, ']');
    });
}

#[test]
fn braces() {
    assert_parsed!(source = "|test {abc def}", Ok(tree) => {
        let item = assert_tree_test_items!(&tree, [_, item] => item);
        assert!(item.is_braced());
        assert_eq!(source.span_str(item.location), "{");
        assert_matches!(item.braced(), Some([item_abc, item_def]) => {
            assert_eq!(item_abc.word_str(), Some("abc"));
            assert_eq!(source.span_str(item_abc.location), "abc");
            assert_eq!(item_def.word_str(), Some("def"));
            assert_eq!(source.span_str(item_def.location), "def");
        });
        assert_matches!(item.clone().kind.try_into_braced(), Ok(_));
    });
    assert_parsed!(source = "|test {}", Ok(tree) => {
        let item = assert_tree_test_items!(&tree, [_, item] => item);
        assert!(item.is_braced());
        assert_eq!(source.span_str(item.location), "{");
        assert_matches!(item.braced(), Some([]));
        assert_matches!(item.clone().kind.try_into_braced(), Ok(_));
    });
    assert_parsed!(source = "|test {", Err(ParseError::UnclosedGroup { missing }) => {
        assert_eq!(missing, '}');
    });
    assert_parsed!(source = "|test }", Err(ParseError::UnexpectedChar { unexpected }) => {
        assert_eq!(unexpected, '}');
    });
}

#[test]
fn indents() {
    assert_matches!(Indent::try_spaces(0), None);
    assert_matches!(Indent::try_spaces(2), Some(_));
}
