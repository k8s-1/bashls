use std::collections::{HashMap, HashSet};

use crate::util::declarations::{
    GlobalDeclarations, find_declaration_using_global_semantics,
    find_declaration_using_local_semantics, get_all_declarations_in_tree, get_global_declarations,
    get_local_declarations,
};
use crate::util::fs::{get_file_paths, path_to_uri, uri_to_path};
use crate::util::shebang::analyze_file;
use crate::util::sourcing::{SourceCommand, get_source_commands};
use crate::util::tree_sitter::{
    collect_typed_nodes, find_parent, for_each, is_definition, is_reference,
    is_variable_in_read_command, node_range, nodes_same, position_to_point,
};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, Location, Position, Range, SymbolInformation, SymbolKind, Uri,
};
use tree_sitter::{Parser, Tree};

struct AnalyzedDocument {
    source: String,
    tree: Tree,
    global_declarations: GlobalDeclarations,
    sourced_uris: HashSet<String>,
    source_commands: Vec<SourceCommand>,
}

pub struct Analyser {
    parser: Parser,
    docs: HashMap<String, AnalyzedDocument>,
    workspace_folder: Option<String>,
    enable_source_error_diagnostics: bool,
    include_all_workspace_symbols: bool,
}

impl Analyser {
    #[must_use]
    pub fn new(parser: Parser, workspace_folder: Option<String>) -> Self {
        Self {
            parser,
            docs: HashMap::new(),
            workspace_folder,
            enable_source_error_diagnostics: false,
            include_all_workspace_symbols: false,
        }
    }

    pub fn set_enable_source_error_diagnostics(&mut self, v: bool) {
        self.enable_source_error_diagnostics = v;
    }

    pub fn set_include_all_workspace_symbols(&mut self, v: bool) {
        self.include_all_workspace_symbols = v;
    }

    pub fn analyze(&mut self, uri: &str, source: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let url: Uri = match uri.parse() {
            Ok(u) => u,
            Err(_) => return diagnostics,
        };

        let Some(tree) = self.parser.parse(source.as_bytes(), None) else {
            return diagnostics;
        };

        if tree.root_node().has_error() {
            log::warn!("Syntax error while parsing {uri}");
        }

        let source_bytes = source.as_bytes();
        let global_declarations = get_global_declarations(&tree, &url, source_bytes);

        let source_commands =
            get_source_commands(&tree, uri, self.workspace_folder.as_deref(), source_bytes);

        let sourced_uris: HashSet<String> = source_commands
            .iter()
            .filter_map(|sc| sc.uri.clone())
            .collect();

        if !self.include_all_workspace_symbols {
            for sc in &source_commands {
                if let Some(ref err) = sc.error {
                    log::warn!("{} line {}: {}", uri, sc.range.start.line, err);
                    if self.enable_source_error_diagnostics {
                        diagnostics.push(Diagnostic {
                            range: sc.range,
                            severity: Some(DiagnosticSeverity::INFORMATION),
                            source: Some("bash-language-server".to_string()),
                            message: format!(
                                "Source command could not be analyzed: {err}.\nConsider adding a ShellCheck directive above this line."
                            ),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        let valid_source_commands: Vec<_> = source_commands
            .into_iter()
            .filter(|sc| sc.error.is_none())
            .collect();

        self.docs.insert(
            uri.to_string(),
            AnalyzedDocument {
                source: source.to_string(),
                tree,
                global_declarations,
                sourced_uris,
                source_commands: valid_source_commands,
            },
        );

        diagnostics
    }

    pub fn background_analysis(&mut self, glob_pattern: &str, max_files: usize) -> usize {
        let Some(workspace) = self.workspace_folder.as_deref() else {
            return 0;
        };
        if max_files == 0 {
            return 0;
        }

        let workspace_path = uri_to_path(workspace);
        let root = workspace_path.as_path();

        let paths = get_file_paths(root, glob_pattern, max_files);
        let count = paths.len();

        log::info!("BackgroundAnalysis: found {count} files in {workspace}");

        for path in paths {
            let uri = path_to_uri(&path);
            if self.docs.contains_key(&uri) {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let analysis = analyze_file(&uri, &content);
                    if analysis.dialect.is_none() {
                        continue;
                    }
                    self.analyze(&uri, &content);
                }
                Err(e) => log::warn!(
                    "BackgroundAnalysis: failed reading {}: {}",
                    path.display(),
                    e
                ),
            }
        }

        count
    }

    #[must_use]
    pub fn word_at_point(&self, uri: &str, line: u32, col: u32) -> Option<String> {
        let doc = self.docs.get(uri)?;
        let node = doc.tree.root_node().descendant_for_point_range(
            position_to_point(Position {
                line,
                character: col,
            }),
            position_to_point(Position {
                line,
                character: col,
            }),
        )?;
        if node.child_count() > 0 {
            return None;
        }
        let text = node
            .utf8_text(doc.source.as_bytes())
            .ok()?
            .trim()
            .to_string();
        if text.is_empty() { None } else { Some(text) }
    }

    pub fn find_declaration_locations(
        &mut self,
        uri: &str,
        word: &str,
        position: Position,
    ) -> Vec<Location> {
        // Check if position is on a source command
        if let Some(doc) = self.docs.get(uri) {
            for sc in &doc.source_commands {
                if crate::util::lsp::is_position_in_range(position, sc.range)
                    && let Some(ref sourced) = sc.uri
                {
                    return vec![Location {
                        uri: sourced.parse().unwrap_or_else(|_| uri.parse().unwrap()),
                        range: Range {
                            start: Position::new(0, 0),
                            end: Position::new(0, 0),
                        },
                    }];
                }
            }
        }

        self.find_declarations_matching_word(uri, word, Some(position), true)
            .into_iter()
            .map(|s| s.location)
            .collect()
    }

    pub fn find_declarations_with_fuzzy_search(&mut self, query: &str) -> Vec<SymbolInformation> {
        let all = self.get_all_declarations(None, None);
        if query.is_empty() {
            return all;
        }
        all.into_iter()
            .filter(|s| s.name.to_lowercase().contains(&query.to_lowercase()))
            .collect()
    }

    pub fn find_declarations_matching_word(
        &mut self,
        uri: &str,
        word: &str,
        position: Option<Position>,
        exact: bool,
    ) -> Vec<SymbolInformation> {
        self.get_all_declarations(Some(uri), position)
            .into_iter()
            .filter(|s| {
                if exact {
                    s.name == word
                } else {
                    s.name.starts_with(word)
                }
            })
            .collect()
    }

    pub fn get_all_variables(&mut self, uri: &str, position: Position) -> Vec<SymbolInformation> {
        self.get_all_declarations(Some(uri), Some(position))
            .into_iter()
            .filter(|s| s.kind == SymbolKind::VARIABLE)
            .collect()
    }

    #[must_use]
    pub fn get_declarations_for_uri(&self, uri: &str) -> Vec<SymbolInformation> {
        let Some(doc) = self.docs.get(uri) else {
            return vec![];
        };
        let url: Uri = uri.parse().unwrap_or_else(|_| "file:///".parse().unwrap());
        get_all_declarations_in_tree(&doc.tree, &url, doc.source.as_bytes())
    }

    #[must_use]
    pub fn find_occurrences(&self, uri: &str, word: &str) -> Vec<Location> {
        let Some(doc) = self.docs.get(uri) else {
            return vec![];
        };
        let url: Uri = uri.parse().unwrap_or_else(|_| "file:///".parse().unwrap());
        let source = doc.source.as_bytes();
        let mut locations = Vec::new();
        let mut seen_ranges: Vec<Range> = Vec::new();

        for_each(doc.tree.root_node(), &mut |n| {
            let named_node = if is_reference(n) {
                n.named_child(0).or(Some(n))
            } else if is_definition(n) {
                n.named_child(0)
            } else {
                None
            };
            if let Some(named) = named_node {
                let text = named.utf8_text(source).unwrap_or("");
                if text == word {
                    let range = node_range(named);
                    if !seen_ranges.contains(&range) {
                        seen_ranges.push(range);
                        locations.push(Location {
                            uri: url.clone(),
                            range,
                        });
                    }
                }
            }
            true
        });

        locations
    }

    #[must_use]
    pub fn find_references(&self, word: &str) -> Vec<Location> {
        let uris: Vec<String> = self.docs.keys().cloned().collect();
        uris.iter()
            .flat_map(|uri| self.find_occurrences(uri, word))
            .collect()
    }

    #[must_use]
    pub fn symbol_at_point(
        &self,
        uri: &str,
        line: u32,
        col: u32,
    ) -> Option<(String, Range, SymbolKind)> {
        let doc = self.docs.get(uri)?;
        let pt = position_to_point(Position {
            line,
            character: col,
        });
        let node = doc.tree.root_node().descendant_for_point_range(pt, pt)?;
        let source = doc.source.as_bytes();

        if node.kind() == "variable_name" {
            let text = node.utf8_text(source).ok()?.to_string();
            return Some((text, node_range(node), SymbolKind::VARIABLE));
        }
        if node.kind() == "word" {
            let parent_kind = node.parent().map_or("", |p| p.kind());
            if matches!(parent_kind, "function_definition" | "command_name") {
                let text = node.utf8_text(source).ok()?.to_string();
                return Some((text, node_range(node), SymbolKind::FUNCTION));
            }
        }
        if is_variable_in_read_command(node, source) {
            let text = node.utf8_text(source).ok()?.to_string();
            return Some((text, node_range(node), SymbolKind::VARIABLE));
        }
        None
    }

    #[must_use]
    pub fn comments_above(&self, uri: &str, line: u32) -> Option<String> {
        let doc = self.docs.get(uri)?;
        let lines: Vec<&str> = doc.source.lines().collect();
        let mut block = Vec::new();
        let comment_re = |l: &str| -> Option<String> {
            let trimmed = l.trim_start();
            let rest = trimmed.strip_prefix('#')?.trim_start();
            Some(rest.trim_end().to_string())
        };

        let mut idx = line.saturating_sub(1) as usize;
        loop {
            let l = lines.get(idx)?;
            match comment_re(l) {
                Some(c) => {
                    block.push(c);
                    if idx == 0 {
                        break;
                    }
                    idx -= 1;
                }
                None => break,
            }
        }

        if block.is_empty() {
            return None;
        }
        block.reverse();
        let mut result = vec!["```txt".to_string()];
        result.extend(block);
        result.push("```".to_string());
        Some(result.join("\n"))
    }

    fn get_ordered_reachable_uris(&self, from_uri: &str) -> Vec<String> {
        let sourced = self.find_all_sourced_uris(from_uri);
        let mut ordered: Vec<String> = sourced.clone();

        for u1 in &sourced {
            for u2 in self.find_all_sourced_uris(u1) {
                if let Some(pos) = ordered.iter().position(|u| *u == u2) {
                    let item = ordered.remove(pos);
                    ordered.push(item);
                }
            }
        }

        ordered.reverse();
        ordered.push(from_uri.to_string());

        if self.include_all_workspace_symbols {
            for uri in self.docs.keys() {
                if !ordered.contains(uri) {
                    ordered.push(uri.clone());
                }
            }
        }

        ordered
    }

    pub fn find_original_declaration(
        &mut self,
        uri: &str,
        position: Position,
        word: &str,
        kind: SymbolKind,
    ) -> (Option<Location>, Option<Location>) {
        let ordered = self.get_ordered_reachable_uris(uri);
        self.ensure_reachable_files_analyzed(&ordered);
        self.do_find_original_declaration(uri, &ordered, position, word, kind)
    }

    fn do_find_original_declaration(
        &self,
        uri: &str,
        ordered_uris: &[String],
        position: Position,
        word: &str,
        kind: SymbolKind,
    ) -> (Option<Location>, Option<Location>) {
        let Some(doc) = self.docs.get(uri) else {
            return (None, None);
        };
        let source = doc.source.as_bytes();
        let root = doc.tree.root_node();
        let pt = position_to_point(position);
        let Some(node) = root.descendant_for_point_range(pt, pt) else {
            return (None, None);
        };

        let mut boundary = position.line as usize;
        let mut decl_range: Option<Range> = None;
        let mut continue_searching = false;
        let mut found_parent: Option<Range> = None;
        let mut found_in_parent = false;

        let mut cur_parent = find_parent(node, |p| {
            matches!(p.kind(), "function_definition" | "subshell")
        });

        while let Some(p) = cur_parent {
            let (d, cont) = if kind == SymbolKind::VARIABLE && p.kind() == "function_definition" {
                let count = p.child_count();
                let body = if count > 0 {
                    p.child((count - 1) as u32)
                } else {
                    None
                };
                if let Some(b) = body {
                    find_declaration_using_local_semantics(b, source, word, position, &mut boundary)
                } else {
                    (None, false)
                }
            } else if p.kind() == "subshell" {
                find_declaration_using_global_semantics(
                    p,
                    source,
                    word,
                    kind,
                    uri,
                    uri,
                    position,
                    &mut boundary,
                )
            } else {
                (None, false)
            };

            if d.is_some() {
                if cont {
                    decl_range = d;
                    continue_searching = true;
                } else {
                    decl_range = d;
                    found_parent = Some(node_range(p));
                    found_in_parent = true;
                    break;
                }
            }

            boundary = p.start_position().row;
            cur_parent = find_parent(p, |pp| {
                matches!(pp.kind(), "function_definition" | "subshell")
            });
        }

        if !found_in_parent && (decl_range.is_none() || continue_searching) {
            let mut found_uri: Option<String> = None;
            for search_uri in ordered_uris {
                let Some(sdoc) = self.docs.get(search_uri.as_str()) else {
                    continue;
                };
                let ssource = sdoc.source.as_bytes();
                let sroot = sdoc.tree.root_node();
                let mut sboundary = if search_uri == uri {
                    position.line as usize
                } else {
                    sroot.end_position().row
                };
                let (d, cont) = find_declaration_using_global_semantics(
                    sroot,
                    ssource,
                    word,
                    kind,
                    uri,
                    search_uri,
                    position,
                    &mut sboundary,
                );
                if d.is_some() {
                    decl_range = d;
                    found_uri = Some(search_uri.clone());
                    if !cont {
                        break;
                    }
                }
            }

            if let Some(ref du) = found_uri {
                let decl_uri: Uri = du.parse().unwrap_or_else(|_| uri.parse().unwrap());
                return (
                    decl_range.map(|r| Location {
                        uri: decl_uri,
                        range: r,
                    }),
                    None,
                );
            }
        }

        let uri_parsed: Uri = uri.parse().unwrap_or_else(|_| "file:///".parse().unwrap());
        (
            decl_range.map(|r| Location {
                uri: uri_parsed.clone(),
                range: r,
            }),
            found_parent.map(|r| Location {
                uri: uri_parsed,
                range: r,
            }),
        )
    }

    #[must_use]
    pub fn command_name_at_point(&self, uri: &str, line: u32, col: u32) -> Option<String> {
        let doc = self.docs.get(uri)?;
        let pt = position_to_point(Position {
            line,
            character: col,
        });
        let mut node = doc.tree.root_node().descendant_for_point_range(pt, pt)?;
        loop {
            if node.kind() == "command" {
                break;
            }
            node = node.parent()?;
        }
        let first_child = node.named_child(0)?;
        if first_child.kind() != "command_name" {
            return None;
        }
        Some(
            first_child
                .utf8_text(doc.source.as_bytes())
                .ok()?
                .trim()
                .to_string(),
        )
    }

    #[must_use]
    pub fn find_all_linked_uris(&self, uri: &str) -> Vec<String> {
        if self.include_all_workspace_symbols {
            return self
                .docs
                .keys()
                .filter(|u| u.as_str() != uri)
                .cloned()
                .collect();
        }
        let mut linked: Vec<String> = Vec::new();
        let mut changed = true;
        while changed {
            changed = false;
            for (analyzed_uri, doc) in &self.docs {
                if analyzed_uri.as_str() == uri || linked.contains(analyzed_uri) {
                    continue;
                }
                for sourced in &doc.sourced_uris {
                    if sourced.as_str() == uri || linked.contains(sourced) {
                        linked.push(analyzed_uri.clone());
                        changed = true;
                        break;
                    }
                }
            }
        }
        linked
    }

    #[must_use]
    pub fn find_occurrences_within(
        &self,
        uri: &str,
        word: &str,
        kind: SymbolKind,
        start: Option<Position>,
        scope: Option<Range>,
    ) -> Vec<Range> {
        let Some(doc) = self.docs.get(uri) else {
            return vec![];
        };
        let source = doc.source.as_bytes();
        let root = doc.tree.root_node();

        let scope_node = scope.map(|s| {
            let sp = position_to_point(s.start);
            let ep = position_to_point(s.end);
            root.descendant_for_point_range(sp, ep).unwrap_or(root)
        });

        let base_node = match scope_node {
            Some(sn) if kind == SymbolKind::VARIABLE || sn.kind() == "subshell" => sn,
            _ => root,
        };

        let effective_start = start
            .map(position_to_point)
            .unwrap_or_else(|| base_node.start_position());

        let kinds: &[&str] = if kind == SymbolKind::VARIABLE {
            &["variable_name", "word"]
        } else {
            &["function_definition", "command_name"]
        };

        let mut nodes = Vec::new();
        collect_typed_nodes(base_node, kinds, effective_start, &mut nodes);

        let mut ignored_ranges: Vec<Range> = Vec::new();
        let mut result: Vec<Range> = Vec::new();

        if kind == SymbolKind::VARIABLE {
            for n in nodes {
                let text = match n.utf8_text(source) {
                    Ok(t) => t,
                    Err(_) => continue,
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
                        let def_row = definition.map(|d| d.start_position().row).unwrap_or(0);
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
        } else {
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
                        n.named_child(0)
                            .map(node_range)
                            .unwrap_or_else(|| node_range(n))
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
        }

        result
    }

    fn find_all_sourced_uris(&self, uri: &str) -> Vec<String> {
        let mut result = Vec::new();
        self.collect_sourced_uris(uri, &mut result);
        result
    }

    fn collect_sourced_uris(&self, uri: &str, result: &mut Vec<String>) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        let sourced: Vec<String> = doc.sourced_uris.iter().cloned().collect();
        for sourced_uri in sourced {
            if !result.contains(&sourced_uri) {
                result.push(sourced_uri.clone());
                self.collect_sourced_uris(&sourced_uri, result);
            }
        }
    }

    fn ensure_reachable_files_analyzed(&mut self, uris: &[String]) {
        let to_analyze: Vec<String> = uris
            .iter()
            .filter(|u| !self.docs.contains_key(u.as_str()))
            .cloned()
            .collect();
        for uri in to_analyze {
            self.ensure_uri_analyzed(&uri);
        }
    }

    fn get_all_declarations(
        &mut self,
        from_uri: Option<&str>,
        position: Option<Position>,
    ) -> Vec<SymbolInformation> {
        let reachable = match from_uri {
            Some(uri) => self.get_reachable_uris(uri),
            None => self.docs.keys().cloned().collect(),
        };

        self.ensure_reachable_files_analyzed(&reachable);

        let mut symbols = Vec::new();

        for uri in &reachable {
            let Some(doc) = self.docs.get(uri.as_str()) else {
                continue;
            };
            let url: Uri = uri.parse().unwrap_or_else(|_| "file:///".parse().unwrap());
            let source_bytes = doc.source.as_bytes();

            if from_uri.is_some_and(|f| f == uri.as_str())
                && let Some(pos) = position
            {
                let pt = position_to_point(pos);
                let node = doc.tree.root_node().descendant_for_point_range(pt, pt);
                if let Some(n) = node {
                    let root = doc.tree.root_node();
                    let local_decls = get_local_declarations(n, root, &url, source_bytes);
                    for syms in local_decls.values() {
                        // pick latest before position
                        let best = syms
                            .iter()
                            .filter(|s| s.location.range.start.line <= pos.line)
                            .max_by_key(|s| s.location.range.start.line);
                        if let Some(s) = best {
                            symbols.push(s.clone());
                        }
                    }
                }
                continue;
            }

            // Use global declarations for other files or no position
            for sym in doc.global_declarations.values() {
                symbols.push(sym.clone());
            }
        }

        symbols
    }

    fn get_reachable_uris(&self, from_uri: &str) -> Vec<String> {
        let mut uris = vec![from_uri.to_string()];
        let sourced = self.find_all_sourced_uris(from_uri);
        uris.extend(sourced);
        if self.include_all_workspace_symbols {
            for uri in self.docs.keys() {
                if !uris.contains(uri) {
                    uris.push(uri.clone());
                }
            }
        }
        uris
    }

    fn ensure_uri_analyzed(&mut self, uri: &str) {
        if self.docs.contains_key(uri) {
            return;
        }
        let Some(path) = crate::util::fs::uri_to_path_opt(uri) else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.analyze(uri, &content);
            }
            Err(e) => log::warn!("Failed to analyze {uri}: {e}"),
        }
    }
}

fn in_ignored_range(ignored: &[Range], n: tree_sitter::Node<'_>) -> bool {
    let start_row = n.start_position().row;
    let end_row = n.end_position().row;
    ignored
        .iter()
        .any(|r| start_row > r.start.line as usize && end_row < r.end.line as usize)
}
