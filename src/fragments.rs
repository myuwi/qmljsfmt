use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FragmentKind {
    Expression,
    BindingBlockContents,
    BindingStatement,
    FunctionDeclaration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fragment {
    pub(crate) kind: FragmentKind,
    pub(crate) range: Range<usize>,
    pub(crate) replacement_start: usize,
    pub(crate) qml_depth: usize,
    pub(crate) qml_member_start: usize,
    pub(crate) parenthesize: bool,
}

pub(crate) fn discover(source: &str, tree: &tree_sitter::Tree) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    discover_in_node(source, tree.root_node(), 0, &mut fragments);
    fragments.sort_by_key(|fragment| fragment.replacement_start);

    debug_assert!(
        fragments
            .windows(2)
            .all(|pair| pair[0].range.end <= pair[1].replacement_start)
    );

    fragments
}

fn discover_in_node(
    source: &str,
    node: tree_sitter::Node<'_>,
    qml_depth: usize,
    fragments: &mut Vec<Fragment>,
) {
    if node.kind() == "ui_annotation" {
        return;
    }

    if let Some(fragment) = fragment_for_node(node, qml_depth) {
        // Formatting inline members would require reflowing their surrounding QML scope.
        if starts_line(source, fragment.qml_member_start) {
            fragments.push(fragment);
        }
    } else {
        let child_qml_depth = qml_depth + usize::from(node.kind() == "ui_object_initializer");
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            discover_in_node(source, child, child_qml_depth, fragments);
        }
    }
}

fn fragment_for_node(node: tree_sitter::Node<'_>, qml_depth: usize) -> Option<Fragment> {
    match node.kind() {
        "ui_binding" | "ui_property" => binding_fragment(node, qml_depth),
        "function_declaration" | "generator_function_declaration" if is_qml_object_member(node) => {
            Some(Fragment {
                kind: FragmentKind::FunctionDeclaration,
                range: node.byte_range(),
                replacement_start: node.start_byte(),
                qml_depth,
                qml_member_start: node.start_byte(),
                parenthesize: false,
            })
        }
        _ => None,
    }
}

fn binding_fragment(binding: tree_sitter::Node<'_>, qml_depth: usize) -> Option<Fragment> {
    let value = binding.child_by_field_name("value")?;
    let (kind, range, parenthesize) = match value.kind() {
        "expression_statement" => {
            let expression = value
                .named_children(&mut value.walk())
                .find(|child| child.kind() != "comment")?;
            (
                FragmentKind::Expression,
                expression.start_byte()..code_end(expression),
                expression.kind() == "sequence_expression",
            )
        }
        "statement_block" => (
            FragmentKind::BindingBlockContents,
            block_contents(value)?,
            false,
        ),
        "if_statement" | "with_statement" | "switch_statement" | "try_statement" => (
            FragmentKind::BindingStatement,
            value.start_byte()..code_end(value),
            false,
        ),
        _ => return None,
    };
    let replacement_start = if matches!(
        kind,
        FragmentKind::Expression | FragmentKind::BindingStatement
    ) {
        colon_end(binding)?
    } else {
        range.start
    };

    Some(Fragment {
        kind,
        range,
        replacement_start,
        qml_depth,
        qml_member_start: binding.start_byte(),
        parenthesize,
    })
}

fn block_contents(block: tree_sitter::Node<'_>) -> Option<Range<usize>> {
    let open = block.child(0)?;
    let close = last_code_child(block)?;

    (open.kind() == "{" && close.kind() == "}").then(|| open.end_byte()..close.start_byte())
}

/// Excludes trailing comments, which the grammar attaches to the node they follow.
fn code_end(node: tree_sitter::Node<'_>) -> usize {
    match last_code_child(node) {
        Some(child) if child.end_byte() == node.end_byte() => code_end(child),
        Some(child) => child.end_byte(),
        None => node.end_byte(),
    }
}

fn last_code_child(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    (0..node.child_count())
        .rev()
        .filter_map(|index| node.child(index))
        .find(|child| child.kind() != "comment")
}

fn colon_end(node: tree_sitter::Node<'_>) -> Option<usize> {
    node.children(&mut node.walk())
        .find(|child| child.kind() == ":")
        .map(|colon| colon.end_byte())
}

fn starts_line(source: &str, byte: usize) -> bool {
    let line_start = source[..byte].rfind('\n').map_or(0, |newline| newline + 1);
    source[line_start..byte]
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
}

fn is_qml_object_member(node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    parent.kind() == "ui_object_initializer"
        || (parent.kind() == "ui_annotated_object_member"
            && parent.child_by_field_name("definition") == Some(node)
            && parent
                .parent()
                .is_some_and(|parent| parent.kind() == "ui_object_initializer"))
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    fn fragments(source: &str) -> Vec<(FragmentKind, &str)> {
        let tree = crate::qml::parse(source).unwrap();
        discover(source, &tree)
            .into_iter()
            .map(|fragment| (fragment.kind, &source[fragment.range]))
            .collect()
    }

    #[test]
    fn discovers_expressions() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property int count: model.count+1
                width: parent.width+1
                onClicked: doThing(foo+1)
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![
                (FragmentKind::Expression, "model.count+1"),
                (FragmentKind::Expression, "parent.width+1"),
                (FragmentKind::Expression, "doThing(foo+1)"),
            ]
        );
    }

    #[test]
    fn discovers_block_contents_and_nested_qml() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onPressed: { let x=1; function nested() { return x } nested() }
                Rectangle {
                    y: parent.y+1
                }
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![
                (
                    FragmentKind::BindingBlockContents,
                    " let x=1; function nested() { return x } nested() "
                ),
                (FragmentKind::Expression, "parent.y+1"),
            ]
        );
    }

    /// The grammar attaches a trailing comment to the block it follows, so the naive
    /// range would slice into the comment and hand Oxfmt an unterminated one.
    #[test]
    fn excludes_trailing_comments_from_ranges() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                property var handler: function () { return a } /* c */
                onPressed: { activate() } /* c */
                onClicked: if (ready) { activate() } /* c */
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![
                (FragmentKind::Expression, "function () { return a }"),
                (FragmentKind::BindingBlockContents, " activate() "),
                (FragmentKind::BindingStatement, "if (ready) { activate() }"),
            ]
        );
    }

    #[test]
    fn discovers_complete_function_declarations() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                function calculate(value: real, options={enabled:true}): real { return value+1 }
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![(
                FragmentKind::FunctionDeclaration,
                "function calculate(value: real, options={enabled:true}): real { return value+1 }"
            )]
        );
    }

    #[test]
    fn discovers_annotated_function_declarations() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                @Deprecated { reason: "Use newFunction instead" }
                function oldFunction(value: int): int { return value+1 }
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![(
                FragmentKind::FunctionDeclaration,
                "function oldFunction(value: int): int { return value+1 }"
            )]
        );
    }

    #[test]
    fn discovers_direct_binding_statements() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                onClicked: if (ready) activate()
                onPressed: switch (mode) { case 1: activate(); break; default: deactivate() }
                onReleased: try { save() } catch (error) { report(error) }
                onCanceled: with (context) reset()
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![
                (FragmentKind::BindingStatement, "if (ready) activate()"),
                (
                    FragmentKind::BindingStatement,
                    "switch (mode) { case 1: activate(); break; default: deactivate() }"
                ),
                (
                    FragmentKind::BindingStatement,
                    "try { save() } catch (error) { report(error) }"
                ),
                (FragmentKind::BindingStatement, "with (context) reset()"),
            ]
        );
    }

    #[test]
    fn discovers_generator_function_declarations() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                function* values() { yield 1 }
            }
        "#};

        assert_eq!(
            fragments(source),
            vec![(
                FragmentKind::FunctionDeclaration,
                "function* values() { yield 1 }"
            )]
        );
    }

    #[test]
    fn ignores_inline_qml_members() {
        let source = indoc! {r#"
            import QtQuick

            Item { width: parent.width+1 }
        "#};

        assert!(fragments(source).is_empty());
    }

    #[test]
    fn retains_whitespace_after_a_binding_colon_for_replacement() {
        let source = indoc! {r#"
            import QtQuick

            Item {
                width:
                    parent.width+1
            }
        "#};
        let tree = crate::qml::parse(source).unwrap();
        let fragment = discover(source, &tree).into_iter().next().unwrap();

        assert_eq!(
            &source[fragment.replacement_start - 1..fragment.replacement_start],
            ":"
        );
        assert_eq!(
            &source[fragment.replacement_start..fragment.range.start],
            "\n        "
        );
    }
}
