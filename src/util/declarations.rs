use lsp_types::{Location, Position, Range, SymbolInformation, SymbolKind, Uri};
use std::collections::HashMap;
use tree_sitter::{Node, Tree};

use crate::util::tree_sitter::{
    collect_typed_nodes, find_parent, for_each, is_definition, is_variable_in_read_command,
    node_range, nodes_same,
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
    if is_definition(node) {
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
            let first_kw = n
                .child(0)
                .and_then(|c| c.utf8_text(source).ok())
                .unwrap_or("");
            let is_local_decl = func_def.is_some_and(|fd| {
                let count = fd.child_count();
                let last_kind = if count > 0 {
                    fd.child((count - 1) as u32).map_or("", |c| c.kind())
                } else {
                    ""
                };
                last_kind == "compound_statement"
                    && matches!(first_kw, "local" | "declare" | "typeset")
                    && (base_node.kind() != "subshell"
                        || base_node.start_position().row < fd.start_position().row)
            });

            let mut var_nodes = Vec::new();
            collect_typed_nodes(n, &["variable_name"], n.start_position(), &mut var_nodes);
            for v in var_nodes {
                let vtext = v.utf8_text(source).unwrap_or("");
                if vtext != word {
                    continue;
                }
                if find_parent(v, |p| matches!(p.kind(), "simple_expansion" | "expansion"))
                    .is_some()
                {
                    continue;
                }
                if is_local_decl {
                    *boundary = n.start_position().row;
                    break;
                }
                if original_uri != current_uri || !is_defined_variable_in_expression(n, v, position)
                {
                    declaration = Some(node_range(v));
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
            if let Some(dv) = var_nodes.into_iter().next() {
                let dv_in_expr = original_uri == current_uri
                    && n.kind() == "variable_assignment"
                    && is_defined_variable_in_expression(n, dv, position);
                if dv.utf8_text(source).unwrap_or("") == word && !dv_in_expr {
                    declaration = Some(node_range(dv));
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

/// Port of TypeScript `findDeclarationUsingLocalSemantics`.
/// Searches `base_node` (a function's compound_statement) for a `local`/`declare`/`typeset`
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

        let first_kw = n
            .child(0)
            .and_then(|c| c.utf8_text(source).ok())
            .unwrap_or("");
        if !matches!(first_kw, "local" | "declare" | "typeset") {
            return false;
        }

        let mut var_nodes = Vec::new();
        collect_typed_nodes(n, &["variable_name"], n.start_position(), &mut var_nodes);
        for v in var_nodes {
            let vtext = v.utf8_text(source).unwrap_or("");
            if vtext != word {
                continue;
            }
            if find_parent(v, |p| matches!(p.kind(), "simple_expansion" | "expansion")).is_some() {
                continue;
            }
            if !is_defined_variable_in_expression(n, v, position) {
                declaration = Some(node_range(v));
                continue_searching = false;
                break;
            }
        }

        false
    });

    (declaration, continue_searching)
}
