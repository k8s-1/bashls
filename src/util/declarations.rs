use lsp_types::{Location, Position, Range, SymbolInformation, SymbolKind, Uri};
use std::collections::HashMap;
use tree_sitter::{Node, Tree};

use crate::util::tree_sitter::{
    collect_typed_nodes, find_parent, for_each, is_variable_in_read_command, node_range,
    nodes_same, position_to_point,
};

pub type GlobalDeclarations = HashMap<String, SymbolInformation>;
pub type Declarations = HashMap<String, Vec<SymbolInformation>>;

const GLOBAL_LEAF_NODE_TYPES: &[&str] = &["if_statement", "function_definition"];

pub fn get_global_declarations(tree: &Tree, uri: &Uri, source: &[u8]) -> GlobalDeclarations {
    let mut result: GlobalDeclarations = HashMap::new();
    for_each(tree.root_node(), &mut |node| {
        let follow = !GLOBAL_LEAF_NODE_TYPES.contains(&node.kind());
        if let Some(sym) = get_declaration_symbol(node, uri, source) {
            result.insert(sym.name.clone(), sym);
        }
        follow
    });
    result
}

pub fn get_all_declarations_in_tree(
    tree: &Tree,
    uri: &Uri,
    source: &[u8],
) -> Vec<SymbolInformation> {
    let mut result = Vec::new();
    for_each(tree.root_node(), &mut |node| {
        if let Some(sym) = get_declaration_symbol(node, uri, source) {
            result.push(sym);
        }
        true
    });
    result
}

pub fn get_local_declarations(
    node: Node<'_>,
    root: Node<'_>,
    uri: &Uri,
    source: &[u8],
) -> Declarations {
    let mut declarations: Declarations = HashMap::new();

    // bottom-up: walk from node to root collecting declarations
    walk_up(node, uri, source, &mut declarations);

    // top-down: add global variable declarations not already present
    let globals = get_all_global_variable_declarations(root, uri, source);
    for (name, syms) in globals {
        declarations.entry(name).or_insert(syms);
    }

    declarations
}

fn walk_up(node: Node<'_>, uri: &Uri, source: &[u8], declarations: &mut Declarations) {
    let Some(parent) = node.parent() else { return };
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if let Some(sym) = get_local_symbol_from_child(child, uri, source) {
            declarations.entry(sym.name.clone()).or_default().push(sym);
        }
    }
    walk_up(parent, uri, source, declarations);
}

fn get_local_symbol_from_child(
    node: Node<'_>,
    uri: &Uri,
    source: &[u8],
) -> Option<SymbolInformation> {
    if node.kind() == "declaration_command" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_assignment" {
                return node_to_symbol_information(child, uri, source);
            }
        }
        None
    } else if node.kind() == "for_statement" {
        let var_node = node.child(1)?;
        if var_node.kind() == "variable_name" {
            let name = var_node.utf8_text(source).ok()?.to_string();
            let container_name = find_parent(var_node, |n| n.kind() == "function_definition")
                .and_then(|n| n.named_child(0))
                .and_then(|n| n.utf8_text(source).ok())
                .map(std::string::ToString::to_string);
            #[allow(deprecated)]
            return Some(SymbolInformation {
                name,
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: node_range(var_node),
                },
                container_name,
            });
        }
        None
    } else {
        get_declaration_symbol(node, uri, source)
    }
}

fn get_all_global_variable_declarations(root: Node<'_>, uri: &Uri, source: &[u8]) -> Declarations {
    let mut result: Declarations = HashMap::new();
    for_each(root, &mut |node| {
        if node.kind() == "variable_assignment" {
            let parent_kind = node.parent().map_or("", |p| p.kind());
            if parent_kind != "declaration_command"
                && let Some(sym) = node_to_symbol_information(node, uri, source)
            {
                result.entry(sym.name.clone()).or_default().push(sym);
            }
        }
        true
    });
    result
}

fn node_to_symbol_information(
    node: Node<'_>,
    uri: &Uri,
    source: &[u8],
) -> Option<SymbolInformation> {
    let named = node.named_child(0)?;
    let name = named.utf8_text(source).ok()?.to_string();
    let kind = tree_sitter_type_to_lsp_kind(node.kind())?;
    let container_name = find_parent(node, |p| p.kind() == "function_definition")
        .and_then(|n| n.named_child(0))
        .and_then(|n| n.utf8_text(source).ok())
        .map(std::string::ToString::to_string);
    #[allow(deprecated)]
    Some(SymbolInformation {
        name,
        kind,
        tags: None,
        deprecated: None,
        location: Location {
            uri: uri.clone(),
            range: node_range(node),
        },
        container_name,
    })
}

fn tree_sitter_type_to_lsp_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "variable_assignment" | "environment_variable_assignment" => Some(SymbolKind::VARIABLE),
        "function_definition" => Some(SymbolKind::FUNCTION),
        _ => None,
    }
}

pub fn get_declaration_symbol(
    node: Node<'_>,
    uri: &Uri,
    source: &[u8],
) -> Option<SymbolInformation> {
    if matches!(node.kind(), "variable_assignment" | "function_definition") {
        node_to_symbol_information(node, uri, source)
    } else if node.kind() == "command" {
        // Handle `: "${VAR:="default"}"` pattern
        let text = node.utf8_text(source).unwrap_or("");
        if text.starts_with(": ") {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "string" {
                    let mut c2 = child.walk();
                    for expansion in child.named_children(&mut c2) {
                        if expansion.kind() == "expansion" {
                            let mut c3 = expansion.walk();
                            for var in expansion.named_children(&mut c3) {
                                if var.kind() == "variable_name" {
                                    let name = var.utf8_text(source).ok()?.to_string();
                                    #[allow(deprecated)]
                                    return Some(SymbolInformation {
                                        name,
                                        kind: SymbolKind::VARIABLE,
                                        tags: None,
                                        deprecated: None,
                                        location: Location {
                                            uri: uri.clone(),
                                            range: node_range(var),
                                        },
                                        container_name: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    } else {
        None
    }
}

fn is_defined_variable_in_expression(
    definition: Node<'_>,
    variable: Node<'_>,
    position: Position,
) -> bool {
    let def_end_row = definition.end_position().row;
    let var_end_row = variable.end_position().row;
    let var_end_col = variable.end_position().column;
    def_end_row >= position.line as usize
        && (var_end_col < position.character as usize || var_end_row < position.line as usize)
}

/// Port of TypeScript `findDeclarationUsingGlobalSemantics`.
///
/// Searches `base_node` (a program root or subshell) for the first declaration
/// of `word`. `boundary` is updated when a local declaration is found (to stop
/// searching past it). Returns `(declaration_range, continue_searching)`.
#[allow(clippy::too_many_arguments)]
pub fn find_declaration_using_global_semantics(
    base_node: Node<'_>,
    source: &[u8],
    word: &str,
    kind: SymbolKind,
    original_uri: &str,
    current_uri: &str,
    position: Position,
    boundary: &mut usize,
) -> (Option<Range>, bool) {
    let mut declaration: Option<Range> = None;
    let mut continue_searching = false;

    for_each(base_node, &mut |n: Node<'_>| {
        if (declaration.is_some() && !continue_searching)
            || n.start_position().row > *boundary
            || (n.kind() == "subshell" && !nodes_same(n, base_node))
        {
            return false;
        }

        if kind == SymbolKind::VARIABLE && n.kind() == "declaration_command" {
            let func_def = find_parent(n, |p| p.kind() == "function_definition");
            let first_keyword = n
                .child(0)
                .and_then(|c| c.utf8_text(source).ok())
                .unwrap_or("");
            let is_local_decl = func_def.is_some_and(|fd| {
                let last_kind = fd.children(&mut fd.walk()).last().map_or("", |c| c.kind());
                last_kind == "compound_statement"
                    && matches!(first_keyword, "local" | "declare" | "typeset")
                    && (base_node.kind() != "subshell"
                        || base_node.start_position().row < fd.start_position().row)
            });

            let mut var_nodes = Vec::new();
            collect_typed_nodes(n, &["variable_name"], n.start_position(), &mut var_nodes);
            for node in var_nodes {
                let text = node.utf8_text(source).unwrap_or("");
                if text != word {
                    continue;
                }
                if find_parent(node, |p| {
                    matches!(p.kind(), "simple_expansion" | "expansion")
                })
                .is_some()
                {
                    continue;
                }
                if is_local_decl {
                    *boundary = n.start_position().row;
                    break;
                }
                if original_uri != current_uri
                    || !is_defined_variable_in_expression(n, node, position)
                {
                    declaration = Some(node_range(node));
                    continue_searching = false;
                    break;
                }
            }
            return false;
        }

        if kind == SymbolKind::VARIABLE
            && (n.kind() == "variable_assignment"
                || n.kind() == "for_statement"
                || (n.kind() == "command" && n.utf8_text(source).unwrap_or("").contains(":=")))
        {
            let mut var_nodes = Vec::new();
            collect_typed_nodes(n, &["variable_name"], n.start_position(), &mut var_nodes);
            if let Some(decl_var) = var_nodes.into_iter().next() {
                let decl_var_in_expr = original_uri == current_uri
                    && n.kind() == "variable_assignment"
                    && is_defined_variable_in_expression(n, decl_var, position);
                if decl_var.utf8_text(source).unwrap_or("") == word && !decl_var_in_expr {
                    declaration = Some(node_range(decl_var));
                    continue_searching = base_node.kind() == "subshell" && n.kind() == "command";
                    return false;
                }
            }
            return true;
        }

        if kind == SymbolKind::VARIABLE
            && is_variable_in_read_command(n, source)
            && n.utf8_text(source).unwrap_or("") == word
        {
            declaration = Some(node_range(n));
            continue_searching = false;
            return false;
        }

        if kind == SymbolKind::FUNCTION
            && n.kind() == "function_definition"
            && n.named_child(0)
                .and_then(|c| c.utf8_text(source).ok())
                .unwrap_or("")
                == word
        {
            declaration = n.named_child(0).map(node_range);
            continue_searching = false;
            return false;
        }

        true
    });

    (declaration, continue_searching)
}

pub fn find_occurrences_within_tree(
    root: Node<'_>,
    source: &[u8],
    word: &str,
    kind: SymbolKind,
    start: Option<Position>,
    scope: Option<Range>,
) -> Vec<Range> {
    let scope_node = scope.map(|s| {
        let sp = position_to_point(s.start);
        let ep = position_to_point(s.end);
        root.descendant_for_point_range(sp, ep).unwrap_or(root)
    });

    let base_node = match scope_node {
        Some(sn) if kind == SymbolKind::VARIABLE || sn.kind() == "subshell" => sn,
        _ => root,
    };

    let effective_start = start.map_or_else(|| base_node.start_position(), position_to_point);

    if kind == SymbolKind::VARIABLE {
        find_variable_occurrences(base_node, source, word, effective_start, start)
    } else {
        find_function_occurrences(base_node, source, word, effective_start)
    }
}

fn find_variable_occurrences<'a>(
    base_node: Node<'a>,
    source: &[u8],
    word: &str,
    effective_start: tree_sitter::Point,
    start: Option<Position>,
) -> Vec<Range> {
    let mut nodes = Vec::new();
    collect_typed_nodes(base_node, &["variable_name", "word"], effective_start, &mut nodes);

    let mut ignored_ranges: Vec<Range> = Vec::new();
    let mut result: Vec<Range> = Vec::new();

    for n in nodes {
        let Ok(text) = n.utf8_text(source) else {
            continue;
        };
        if text != word {
            continue;
        }
        if n.kind() == "word" && !is_variable_in_read_command(n, source) {
            continue;
        }

        let definition = find_parent(n, |p| p.kind() == "variable_assignment");
        let defined_var = definition
            .and_then(|d| d.named_child(0))
            .filter(|v| v.kind() == "variable_name");
        let defined_var_matches = defined_var
            .and_then(|v| v.utf8_text(source).ok())
            .is_some_and(|t| t == word);

        if defined_var_matches {
            let is_self = defined_var.is_some_and(|dv| dv.start_byte() == n.start_byte());
            if !is_self {
                let def_row = definition.map_or(0, |d| d.start_position().row);
                if start.is_some_and(|s| def_row == s.line as usize) {
                    continue;
                }
                result.push(node_range(n));
                continue;
            }
        }

        let parent_scope = find_parent(n, |p| {
            p.kind() == "function_definition" || p.kind() == "subshell"
        });
        let in_base = parent_scope.is_none_or(|ps| nodes_same(ps, base_node));
        if in_base {
            result.push(node_range(n));
            continue;
        }

        let include = !in_ignored_range(&ignored_ranges, n);
        let declaration_command = find_parent(n, |p| p.kind() == "declaration_command");
        let kw = declaration_command
            .and_then(|dc| dc.child(0))
            .and_then(|c| c.utf8_text(source).ok())
            .unwrap_or("");
        let is_local_kw = matches!(kw, "local" | "declare" | "typeset");
        let parent_is_subshell = parent_scope.is_some_and(|ps| ps.kind() == "subshell");

        let is_local = ((defined_var_matches
            || (definition.is_none() && declaration_command.is_some()))
            && (parent_is_subshell || is_local_kw))
            || (parent_is_subshell && n.kind() == "word");

        if is_local {
            if include && let Some(ps) = parent_scope {
                ignored_ranges.push(node_range(ps));
            }
            continue;
        }

        if include {
            result.push(node_range(n));
        }
    }

    result
}

fn find_function_occurrences<'a>(
    base_node: Node<'a>,
    source: &[u8],
    word: &str,
    effective_start: tree_sitter::Point,
) -> Vec<Range> {
    let mut nodes = Vec::new();
    collect_typed_nodes(
        base_node,
        &["function_definition", "command_name"],
        effective_start,
        &mut nodes,
    );

    let mut ignored_ranges: Vec<Range> = Vec::new();
    let mut result: Vec<Range> = Vec::new();

    for n in nodes {
        let text = if n.kind() == "function_definition" {
            n.named_child(0)
                .and_then(|c| c.utf8_text(source).ok())
                .unwrap_or("")
                .to_string()
        } else {
            n.utf8_text(source).unwrap_or("").to_string()
        };
        if text != word {
            continue;
        }

        let parent_subshell = find_parent(n, |p| p.kind() == "subshell");
        let in_base = parent_subshell.is_none_or(|ps| nodes_same(ps, base_node));
        if in_base {
            let r = if n.kind() == "function_definition" {
                n.named_child(0).map_or_else(|| node_range(n), node_range)
            } else {
                node_range(n)
            };
            result.push(r);
            continue;
        }

        let include = !in_ignored_range(&ignored_ranges, n);
        if n.kind() == "function_definition" {
            if include && let Some(ps) = parent_subshell {
                ignored_ranges.push(node_range(ps));
            }
            continue;
        }
        if include {
            result.push(node_range(n));
        }
    }

    result
}

fn in_ignored_range(ignored: &[Range], n: Node<'_>) -> bool {
    let start_row = n.start_position().row;
    let end_row = n.end_position().row;
    ignored
        .iter()
        .any(|r| start_row >= r.start.line as usize && end_row <= r.end.line as usize)
}

/// Port of TypeScript `findDeclarationUsingLocalSemantics`.
///
/// Searches `base_node` (a function's `compound_statement`) for a `local`/`declare`/`typeset`
/// declaration of `word`. Returns `(declaration_range, continue_searching)`.
pub fn find_declaration_using_local_semantics(
    base_node: Node<'_>,
    source: &[u8],
    word: &str,
    position: Position,
    boundary: &mut usize,
) -> (Option<Range>, bool) {
    let mut declaration: Option<Range> = None;
    let mut continue_searching = false;

    for_each(base_node, &mut |n: Node<'_>| {
        if (declaration.is_some() && !continue_searching)
            || n.start_position().row > *boundary
            || matches!(n.kind(), "function_definition" | "subshell")
        {
            return false;
        }

        if n.kind() != "declaration_command" {
            return true;
        }

        let first_keyword = n
            .child(0)
            .and_then(|c| c.utf8_text(source).ok())
            .unwrap_or("");
        if !matches!(first_keyword, "local" | "declare" | "typeset") {
            return false;
        }

        let mut var_nodes = Vec::new();
        collect_typed_nodes(n, &["variable_name"], n.start_position(), &mut var_nodes);
        for node in var_nodes {
            let text = node.utf8_text(source).unwrap_or("");
            if text != word {
                continue;
            }
            if find_parent(node, |p| {
                matches!(p.kind(), "simple_expansion" | "expansion")
            })
            .is_some()
            {
                continue;
            }
            if !is_defined_variable_in_expression(n, node, position) {
                declaration = Some(node_range(node));
                continue_searching = false;
                break;
            }
        }

        false
    });

    (declaration, continue_searching)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::create_parser;

    fn parse(content: &str) -> (tree_sitter::Tree, Uri) {
        let mut parser = create_parser().unwrap();
        let tree = parser.parse(content.as_bytes(), None).unwrap();
        let uri: Uri = "file:///test.sh".parse().unwrap();
        (tree, uri)
    }

    #[test]
    fn get_global_declarations_finds_function() {
        let content = "myfunc() { echo hi; }\n";
        let (tree, uri) = parse(content);
        let decls = get_global_declarations(&tree, &uri, content.as_bytes());
        assert!(decls.contains_key("myfunc"), "{decls:?}");
    }

    #[test]
    fn get_global_declarations_finds_variable() {
        let content = "myvar=hello\n";
        let (tree, uri) = parse(content);
        let decls = get_global_declarations(&tree, &uri, content.as_bytes());
        assert!(decls.contains_key("myvar"), "{decls:?}");
    }

    #[test]
    fn get_global_declarations_excludes_variable_inside_if() {
        let content = "if true; then\n  inside=1\nfi\n";
        let (tree, uri) = parse(content);
        let decls = get_global_declarations(&tree, &uri, content.as_bytes());
        assert!(
            !decls.contains_key("inside"),
            "var inside if should not be global"
        );
    }

    #[test]
    fn get_global_declarations_excludes_variable_inside_function() {
        let content = "myfunc() { local x=1; }\n";
        let (tree, uri) = parse(content);
        let decls = get_global_declarations(&tree, &uri, content.as_bytes());
        assert!(
            !decls.contains_key("x"),
            "var inside function should not be global"
        );
        assert!(decls.contains_key("myfunc"));
    }

    #[test]
    fn get_all_declarations_in_tree_includes_nested_variable() {
        let content = "outer() {\n  inner_var=1\n}\n";
        let (tree, uri) = parse(content);
        let decls = get_all_declarations_in_tree(&tree, &uri, content.as_bytes());
        let names: Vec<_> = decls.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"outer"), "{names:?}");
        assert!(names.contains(&"inner_var"), "{names:?}");
    }

    #[test]
    fn get_all_declarations_in_tree_empty_for_no_declarations() {
        let content = "echo hello\n";
        let (tree, uri) = parse(content);
        let decls = get_all_declarations_in_tree(&tree, &uri, content.as_bytes());
        assert!(decls.is_empty(), "{decls:?}");
    }

    #[test]
    fn occurrences_excludes_local_in_single_line_function() {
        // Highlighting the outer x=1 should not also highlight x inside the function body —
        // that x is a separate local variable. When the whole function is on one line the scope
        // range has start_row == end_row
        let content = "x=1\nfoo() { local x=2; echo $x; }\n";
        let (tree, _uri) = parse(content);
        let ranges = find_occurrences_within_tree(
            tree.root_node(),
            content.as_bytes(),
            "x",
            lsp_types::SymbolKind::VARIABLE,
            None,
            None,
        );
        assert_eq!(ranges.len(), 1, "expected only outer x=1, got {ranges:?}");
        assert_eq!(ranges[0].start.line, 0);
    }
}
