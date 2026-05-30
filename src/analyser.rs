use std::collections::{HashMap, HashSet};

use crate::util::declarations::{
    GlobalDeclarations, find_declaration_using_global_semantics,
    find_declaration_using_local_semantics, get_all_declarations_in_tree, get_global_declarations,
    get_local_declarations,
};
use crate::util::fs::{get_file_paths, path_to_uri, uri_to_path};
use crate::util::lsp::parse_uri;
use crate::util::shebang::analyze_file;
use crate::util::sourcing::{SourceCommand, get_source_commands};
use crate::util::tree_sitter::{
    find_parent, for_each, is_variable_in_read_command, node_range,
    position_to_point,
};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, Location, Position, Range, SymbolInformation, SymbolKind, Uri,
};
use tree_sitter::{Parser, Tree};

struct AnalyzedDocument {
    source: String,
    tree: Tree,
    global_declarations: GlobalDeclarations,
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

    pub const fn set_enable_source_error_diagnostics(&mut self, v: bool) {
        self.enable_source_error_diagnostics = v;
    }

    pub const fn set_include_all_workspace_symbols(&mut self, v: bool) {
        self.include_all_workspace_symbols = v;
    }

    pub fn remove(&mut self, uri: &str) {
        self.docs.remove(uri);
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
        let pt = position_to_point(Position {
            line,
            character: col,
        });
        let node = doc.tree.root_node().descendant_for_point_range(pt, pt)?;
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
                        uri: parse_uri(sourced),
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
        let q = query.to_lowercase();
        all.into_iter()
            .filter(|s| s.name.to_lowercase().contains(&q))
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
        let url: Uri = parse_uri(uri);
        get_all_declarations_in_tree(&doc.tree, &url, doc.source.as_bytes())
    }

    #[must_use]
    pub fn find_occurrences(&self, uri: &str, word: &str) -> Vec<Location> {
        let Some(doc) = self.docs.get(uri) else {
            return vec![];
        };
        let url: Uri = parse_uri(uri);
        let source = doc.source.as_bytes();
        let mut locations = Vec::new();
        let mut seen_ranges: HashSet<Range> = HashSet::new();

        for_each(doc.tree.root_node(), &mut |n| {
            let named_node = match n.kind() {
                "variable_name" | "command_name" => n.named_child(0).or(Some(n)),
                "variable_assignment" | "function_definition" => n.named_child(0),
                _ => None,
            };
            if let Some(named) = named_node {
                let text = named.utf8_text(source).unwrap_or("");
                if text == word {
                    let range = node_range(named);
                    if seen_ranges.insert(range) {
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
        self.docs
            .keys()
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

        for direct_uri in &sourced {
            for transitive_uri in self.find_all_sourced_uris(direct_uri) {
                if let Some(pos) = ordered.iter().position(|u| *u == transitive_uri) {
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
        let mut containing_scope_range: Option<Range> = None;
        let mut found_in_scope = false;

        let mut cur_scope = find_parent(node, |p| {
            matches!(p.kind(), "function_definition" | "subshell")
        });

        while let Some(scope) = cur_scope {
            let (found_range, keep_searching) =
                if kind == SymbolKind::VARIABLE && scope.kind() == "function_definition" {
                    let mut walker = scope.walk();
                    let func_body = scope.children(&mut walker).last();
                    func_body.map_or((None, false), |func_body| {
                        find_declaration_using_local_semantics(
                            func_body,
                            source,
                            word,
                            position,
                            &mut boundary,
                        )
                    })
                } else if scope.kind() == "subshell" {
                    find_declaration_using_global_semantics(
                        scope,
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

            if found_range.is_some() {
                decl_range = found_range;
                if keep_searching {
                    continue_searching = true;
                } else {
                    containing_scope_range = Some(node_range(scope));
                    found_in_scope = true;
                    break;
                }
            }

            boundary = scope.start_position().row;
            cur_scope = find_parent(scope, |ancestor| {
                matches!(ancestor.kind(), "function_definition" | "subshell")
            });
        }

        if !found_in_scope && (decl_range.is_none() || continue_searching) {
            let mut found_uri: Option<String> = None;
            for search_uri in ordered_uris {
                let Some(search_doc) = self.docs.get(search_uri.as_str()) else {
                    continue;
                };
                let search_source = search_doc.source.as_bytes();
                let search_root = search_doc.tree.root_node();
                let mut search_boundary = if search_uri == uri {
                    position.line as usize
                } else {
                    search_root.end_position().row
                };
                let (found_range, keep_searching) = find_declaration_using_global_semantics(
                    search_root,
                    search_source,
                    word,
                    kind,
                    uri,
                    search_uri,
                    position,
                    &mut search_boundary,
                );
                if found_range.is_some() {
                    decl_range = found_range;
                    found_uri = Some(search_uri.clone());
                    if !keep_searching {
                        break;
                    }
                }
            }

            if let Some(decl_uri_str) = found_uri {
                let decl_uri: Uri = parse_uri(&decl_uri_str);
                return (
                    decl_range.map(|r| Location {
                        uri: decl_uri,
                        range: r,
                    }),
                    None,
                );
            }
        }

        let uri_parsed: Uri = parse_uri(uri);
        (
            decl_range.map(|r| Location {
                uri: uri_parsed.clone(),
                range: r,
            }),
            containing_scope_range.map(|r| Location {
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
        let mut linked: HashSet<String> = HashSet::new();
        let mut changed = true;
        while changed {
            changed = false;
            for (analyzed_uri, doc) in &self.docs {
                if analyzed_uri.as_str() == uri || linked.contains(analyzed_uri.as_str()) {
                    continue;
                }
                for sourced in doc.source_commands.iter().filter_map(|sc| sc.uri.as_ref()) {
                    if sourced.as_str() == uri || linked.contains(sourced.as_str()) {
                        linked.insert(analyzed_uri.clone());
                        changed = true;
                        break;
                    }
                }
            }
        }
        linked.into_iter().collect()
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
        crate::util::declarations::find_occurrences_within_tree(
            doc.tree.root_node(),
            doc.source.as_bytes(),
            word,
            kind,
            start,
            scope,
        )
    }

    fn find_all_sourced_uris(&self, uri: &str) -> Vec<String> {
        let mut ordered = Vec::new();
        let mut seen = HashSet::new();
        self.collect_sourced_uris(uri, &mut ordered, &mut seen);
        ordered
    }

    fn collect_sourced_uris(
        &self,
        uri: &str,
        ordered: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        for sourced_uri in doc.source_commands.iter().filter_map(|sc| sc.uri.as_ref()) {
            if seen.insert(sourced_uri.clone()) {
                ordered.push(sourced_uri.clone());
                self.collect_sourced_uris(sourced_uri, ordered, seen);
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
            let url: Uri = parse_uri(uri);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::create_parser;

    const URI: &str = "file:///test.sh";

    fn make_analyser(content: &str) -> Analyser {
        let parser = create_parser().unwrap();
        let mut a = Analyser::new(parser, None);
        a.analyze(URI, content);
        a
    }

    #[test]
    fn word_at_point_returns_first_token() {
        let a = make_analyser("echo hello\n");
        assert_eq!(a.word_at_point(URI, 0, 0), Some("echo".to_string()));
    }

    #[test]
    fn word_at_point_returns_second_token() {
        let a = make_analyser("echo hello\n");
        assert_eq!(a.word_at_point(URI, 0, 6), Some("hello".to_string()));
    }

    #[test]
    fn word_at_point_returns_none_for_whitespace() {
        let a = make_analyser("echo hello\n");
        assert_eq!(a.word_at_point(URI, 0, 4), None);
    }

    #[test]
    fn word_at_point_returns_none_for_unknown_uri() {
        let a = make_analyser("echo hi\n");
        assert_eq!(a.word_at_point("file:///other.sh", 0, 0), None);
    }

    #[test]
    fn get_declarations_for_uri_finds_function() {
        let a = make_analyser("myfunc() { echo hi; }\n");
        let syms = a.get_declarations_for_uri(URI);
        assert!(syms.iter().any(|s| s.name == "myfunc"), "{syms:?}");
    }

    #[test]
    fn get_declarations_for_uri_finds_variable() {
        let a = make_analyser("myvar=hello\n");
        let syms = a.get_declarations_for_uri(URI);
        assert!(syms.iter().any(|s| s.name == "myvar"), "{syms:?}");
    }

    #[test]
    fn get_declarations_for_uri_returns_empty_for_unknown_uri() {
        let a = make_analyser("myvar=1\n");
        assert!(a.get_declarations_for_uri("file:///other.sh").is_empty());
    }

    #[test]
    fn find_occurrences_counts_all_instances() {
        let a = make_analyser("myvar=1\necho $myvar\nmyvar=2\n");
        let locs = a.find_occurrences(URI, "myvar");
        assert_eq!(locs.len(), 3);
    }

    #[test]
    fn find_occurrences_returns_empty_for_absent_word() {
        let a = make_analyser("echo hello\n");
        assert!(a.find_occurrences(URI, "nonexistent").is_empty());
    }

    #[test]
    fn find_declarations_with_fuzzy_search_empty_returns_all() {
        let mut a = make_analyser("myfunc() { echo hi; }\nmyvar=1\n");
        let syms = a.find_declarations_with_fuzzy_search("");
        assert!(syms.len() >= 2, "{syms:?}");
    }

    #[test]
    fn find_declarations_with_fuzzy_search_filters_by_substring() {
        let mut a = make_analyser("myfunc() { echo hi; }\nother=1\n");
        let syms = a.find_declarations_with_fuzzy_search("myf");
        assert!(syms.iter().all(|s| s.name.contains("myf")));
        assert!(!syms.iter().any(|s| s.name == "other"));
    }

    #[test]
    fn analyze_produces_no_diagnostics_for_valid_source() {
        let mut a = Analyser::new(create_parser().unwrap(), None);
        let diags = a.analyze(URI, "echo hello\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn remove_clears_document() {
        let mut a = make_analyser("echo hello\n");
        assert!(a.word_at_point(URI, 0, 0).is_some());
        a.remove(URI);
        assert!(a.word_at_point(URI, 0, 0).is_none());
    }
}
