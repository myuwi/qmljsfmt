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
}

pub(crate) fn discover(tree: &tree_sitter::Tree) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    discover_in_node(tree.root_node(), &mut fragments);
    fragments.sort_by_key(|fragment| fragment.range.start);

    debug_assert!(
        fragments
            .windows(2)
            .all(|pair| pair[0].range.end <= pair[1].range.start)
    );

    fragments
}

fn discover_in_node(node: tree_sitter::Node<'_>, fragments: &mut Vec<Fragment>) {
    match node.kind() {
        "ui_binding" | "ui_property" => {
            if let Some(value) = node.child_by_field_name("value") {
                match value.kind() {
                    "expression_statement" => {
                        if let Some(expression) = value
                            .named_children(&mut value.walk())
                            .find(|child| child.kind() != "comment")
                        {
                            fragments.push(Fragment {
                                kind: FragmentKind::Expression,
                                range: expression.byte_range(),
                            });
                        }
                        return;
                    }
                    "statement_block" => {
                        fragments.push(Fragment {
                            kind: FragmentKind::BindingBlockContents,
                            range: value.start_byte() + 1..value.end_byte() - 1,
                        });
                        return;
                    }
                    "if_statement" | "switch_statement" | "try_statement" => {
                        fragments.push(Fragment {
                            kind: FragmentKind::BindingStatement,
                            range: value.byte_range(),
                        });
                        return;
                    }
                    _ => {}
                }
            }
        }
        "function_declaration" | "generator_function_declaration" if is_qml_object_member(node) => {
            fragments.push(Fragment {
                kind: FragmentKind::FunctionDeclaration,
                range: node.byte_range(),
            });
            return;
        }
        "ui_annotation" => return,
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        discover_in_node(child, fragments);
    }
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
    use super::*;

    fn fragments(source: &str) -> Vec<(FragmentKind, &str)> {
        let tree = crate::qml::parse(source).unwrap();
        discover(&tree)
            .into_iter()
            .map(|fragment| (fragment.kind, &source[fragment.range]))
            .collect()
    }

    #[test]
    fn discovers_expressions() {
        let source = r#"import QtQuick

Item {
    property int count: model.count+1
    width: parent.width+1
    onClicked: doThing(foo+1)
}
"#;

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
        let source = r#"import QtQuick

Item {
    onPressed: { let x=1; function nested() { return x } nested() }
    Rectangle { y: parent.y+1 }
}
"#;

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

    #[test]
    fn discovers_complete_function_declarations() {
        let source = r#"import QtQuick

Item {
    function calculate(value: real, options={enabled:true}): real { return value+1 }
}
"#;

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
        let source = r#"import QtQuick

Item {
    @Deprecated { reason: "Use newFunction instead" }
    function oldFunction(value: int): int { return value+1 }
}
"#;

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
        let source = r#"import QtQuick

Item {
    onClicked: if (ready) activate()
    onPressed: switch (mode) { case 1: activate(); break; default: deactivate() }
    onReleased: try { save() } catch (error) { report(error) }
    onCanceled: with (context) reset()
}
"#;

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
            ]
        );
    }

    #[test]
    fn discovers_generator_function_declarations() {
        let source = r#"import QtQuick

Item {
    function* values() { yield 1 }
}
"#;

        assert_eq!(
            fragments(source),
            vec![(
                FragmentKind::FunctionDeclaration,
                "function* values() { yield 1 }"
            )]
        );
    }
}
