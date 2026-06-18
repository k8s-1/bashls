use bashls::analyser::Analyser;
use bashls::parser::create_parser;
use lsp_types::{Position, Range, SymbolKind};
use std::fs;

const URI: &str = "file:///test.sh";

fn analyser_with(content: &str) -> Analyser {
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(URI, content);
    a
}

#[test]
fn document_symbols_finds_functions_and_variables() {
    let a = analyser_with("#!/bin/bash\nmy_func() { echo hello; }\nmy_var=42\n");
    let syms = a.get_declarations_for_uri(URI);
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"my_func"), "missing my_func in {:?}", names);
    assert!(names.contains(&"my_var"), "missing my_var in {:?}", names);
}

#[test]
fn document_symbols_distinguishes_function_from_variable() {
    let a = analyser_with("greet() { echo hi; }\nname=world\n");
    let syms = a.get_declarations_for_uri(URI);
    let func = syms.iter().find(|s| s.name == "greet");
    let var = syms.iter().find(|s| s.name == "name");
    assert_eq!(func.map(|s| s.kind), Some(SymbolKind::FUNCTION));
    assert_eq!(var.map(|s| s.kind), Some(SymbolKind::VARIABLE));
}

#[test]
fn goto_definition_resolves_function_call_to_definition_line() {
    let mut a = analyser_with("my_func() { echo hello; }\nmy_func\n");
    let locs = a.find_declaration_locations(URI, "my_func", Position::new(1, 0));
    assert!(
        !locs.is_empty(),
        "expected at least one definition location"
    );
    assert_eq!(
        locs[0].range.start.line, 0,
        "definition should be on line 0"
    );
}

#[test]
fn goto_definition_resolves_variable_to_assignment() {
    let mut a = analyser_with("my_var=hello\necho $my_var\n");
    let locs = a.find_declaration_locations(URI, "my_var", Position::new(1, 6));
    assert!(!locs.is_empty(), "expected variable definition");
    assert_eq!(locs[0].range.start.line, 0);
}

#[test]
fn find_occurrences_returns_all_uses_of_symbol() {
    let a = analyser_with("my_var=1\necho $my_var\necho $my_var\n");
    let locs = a.find_occurrences(URI, "my_var");
    assert!(
        locs.len() >= 2,
        "expected >= 2 occurrences, got {}",
        locs.len()
    );
}

#[test]
fn find_occurrences_includes_definition_and_references() {
    let a = analyser_with("count=0\ncount=1\necho $count\n");
    let locs = a.find_occurrences(URI, "count");
    let lines: Vec<u32> = locs.iter().map(|l| l.range.start.line).collect();
    assert!(lines.contains(&0), "expected occurrence on line 0");
    assert!(lines.contains(&2), "expected occurrence on line 2");
}

#[test]
fn word_at_point_returns_token_under_cursor() {
    let a = analyser_with("my_func() { echo hello; }\n");
    let word = a.word_at_point(URI, 0, 2);
    assert_eq!(word.as_deref(), Some("my_func"));
}

#[test]
fn word_at_point_returns_none_for_whitespace() {
    let a = analyser_with("foo bar\n");
    let word = a.word_at_point(URI, 0, 3);
    assert!(word.is_none());
}

#[test]
fn comments_above_captures_block_comment_before_function() {
    let content = "#!/bin/bash\n# Does something useful\nmy_func() { echo hello; }\n";
    let a = analyser_with(content);
    let comment = a.comments_above(URI, 2);
    assert!(comment.is_some(), "expected comment above my_func");
    assert!(
        comment.unwrap().contains("Does something useful"),
        "comment content not found"
    );
}

#[test]
fn comments_above_returns_none_when_no_comment_present() {
    let a = analyser_with("my_func() { echo hello; }\n");
    let comment = a.comments_above(URI, 0);
    assert!(comment.is_none());
}

#[test]
fn symbol_at_point_identifies_variable() {
    let a = analyser_with("my_var=hello\n");
    let result = a.symbol_at_point(URI, 0, 2);
    assert!(result.is_some(), "expected symbol at point");
    let (name, _range, kind) = result.unwrap();
    assert_eq!(name, "my_var");
    assert_eq!(kind, SymbolKind::VARIABLE);
}

#[test]
fn symbol_at_point_identifies_function() {
    let a = analyser_with("my_func() { echo hi; }\n");
    let result = a.symbol_at_point(URI, 0, 2);
    assert!(result.is_some(), "expected symbol at point");
    let (name, _range, kind) = result.unwrap();
    assert_eq!(name, "my_func");
    assert_eq!(kind, SymbolKind::FUNCTION);
}

#[test]
fn fuzzy_search_finds_symbols_by_partial_name() {
    let mut a = analyser_with("my_function() { echo hi; }\nanother_func() { echo bye; }\n");
    let results = a.find_declarations_with_fuzzy_search("func");
    let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"my_function") || names.contains(&"another_func"),
        "fuzzy search returned: {:?}",
        names
    );
}

#[test]
fn fuzzy_search_empty_query_returns_all_symbols() {
    let mut a = analyser_with("func_a() { :; }\nfunc_b() { :; }\nvar_x=1\n");
    let results = a.find_declarations_with_fuzzy_search("");
    assert!(
        results.len() >= 3,
        "expected all symbols, got {}",
        results.len()
    );
}

#[test]
fn analyze_returns_no_diagnostics_for_valid_script() {
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    let diags = a.analyze(URI, "#!/bin/bash\necho hello\n");
    assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
}

#[test]
fn declarations_matching_word_with_prefix_match() {
    let mut a = analyser_with("configure() { :; }\nconfig_val=1\n");
    let results = a.find_declarations_matching_word(URI, "config", None, false);
    let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"configure"),
        "prefix match missing configure"
    );
    assert!(
        names.contains(&"config_val"),
        "prefix match missing config_val"
    );
}

// --- Completion ---

#[test]
fn variable_completion_returns_all_variables_in_scope() {
    let content = "alpha=1\nbeta=2\ngamma=3\n";
    let mut a = analyser_with(content);
    let vars = a.get_all_variables(URI, Position::new(2, 7));
    let names: Vec<&str> = vars.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"alpha"), "missing alpha in {:?}", names);
    assert!(names.contains(&"beta"), "missing beta in {:?}", names);
    assert!(names.contains(&"gamma"), "missing gamma in {:?}", names);
}

#[test]
fn symbol_completion_prefix_matches_functions_and_variables() {
    let mut a = analyser_with("configure() { :; }\nconnect() { :; }\nunrelated=1\n");
    let results = a.find_declarations_matching_word(URI, "con", None, false);
    let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"configure"), "missing configure");
    assert!(names.contains(&"connect"), "missing connect");
    assert!(
        !names.contains(&"unrelated"),
        "unrelated should not match prefix 'con'"
    );
}

// --- Rename ---

#[test]
fn rename_occurrences_cover_definition_and_all_call_sites() {
    let content = "my_func() { echo hi; }\nmy_func\nmy_func\n";
    let a = analyser_with(content);
    let locs = a.find_occurrences(URI, "my_func");
    assert_eq!(
        locs.len(),
        3,
        "expected 3 occurrences (1 def + 2 calls), got {}",
        locs.len()
    );
}

#[test]
fn rename_occurrence_ranges_point_to_correct_text() {
    let content = "my_func() { echo hi; }\nmy_func\n";
    let a = analyser_with(content);
    let locs = a.find_occurrences(URI, "my_func");
    for loc in &locs {
        let line = content.lines().nth(loc.range.start.line as usize).unwrap();
        let start = loc.range.start.character as usize;
        let end = loc.range.end.character as usize;
        assert_eq!(
            &line[start..end],
            "my_func",
            "range at line {} was wrong",
            loc.range.start.line
        );
    }
}

// --- Diagnostics ---

#[test]
fn source_error_diagnostic_emitted_when_sourced_file_not_found() {
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.set_enable_source_error_diagnostics(true);
    let diags = a.analyze(URI, "source /nonexistent/path/to/missing.sh\n");
    assert!(!diags.is_empty(), "expected a source-error diagnostic");
    assert!(
        diags[0]
            .message
            .contains("Source command could not be analyzed"),
        "unexpected message: {}",
        diags[0].message
    );
}

// --- Cross-file references ---

#[test]
fn find_references_spans_all_analyzed_files() {
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    let uri1 = "file:///refs_a.sh";
    let uri2 = "file:///refs_b.sh";
    a.analyze(uri1, "shared_func() { echo a; }\n");
    a.analyze(uri2, "shared_func\n");
    let locs = a.find_references("shared_func");
    let uris: Vec<&str> = locs.iter().map(|l| l.uri.as_str()).collect();
    assert!(uris.contains(&uri1), "expected reference in refs_a.sh");
    assert!(uris.contains(&uri2), "expected reference in refs_b.sh");
    assert!(locs.len() >= 2, "expected at least 2 total references");
}

// --- Cross-file sourcing ---

#[test]
fn sourced_file_symbols_visible_from_main_file() {
    let dir = std::env::temp_dir().join("bashls_test_sourcing");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("helper.sh"),
        "helper_func() { echo hi; }\nhelper_var=42\n",
    )
    .unwrap();
    fs::write(dir.join("main.sh"), "source ./helper.sh\n").unwrap();

    let main_path = dir.join("main.sh");
    let main_uri = format!("file://{}", main_path.display());
    let content = fs::read_to_string(&main_path).unwrap();

    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(&main_uri, &content);

    let funcs = a.find_declarations_matching_word(&main_uri, "helper_func", None, true);
    assert!(
        !funcs.is_empty(),
        "helper_func from sourced file should be visible"
    );

    let vars = a.find_declarations_matching_word(&main_uri, "helper_var", None, true);
    assert!(
        !vars.is_empty(),
        "helper_var from sourced file should be visible"
    );

    fs::remove_dir_all(&dir).ok();
}

// --- commentsAbove edge cases ---

#[test]
fn comments_above_multi_line() {
    let content = "# doc for func_two\n# has two lines\nfunc_two() { echo hi; }\n";
    let a = analyser_with(content);
    assert_eq!(
        a.comments_above(URI, 2).as_deref(),
        Some("```txt\ndoc for func_two\nhas two lines\n```"),
    );
}

#[test]
fn comments_above_only_connected_block() {
    // blank line between two comment blocks — only the immediately-connected one is returned
    let content = "# this is not included\n\n# doc for func_three\nfunc_three() { echo hi; }\n";
    let a = analyser_with(content);
    assert_eq!(
        a.comments_above(URI, 3).as_deref(),
        Some("```txt\ndoc for func_three\n```"),
    );
}

#[test]
fn comments_above_works_for_variables() {
    let content = "# works for variables\nmy_var=\"pizza\"\n";
    let a = analyser_with(content);
    assert_eq!(
        a.comments_above(URI, 1).as_deref(),
        Some("```txt\nworks for variables\n```"),
    );
}

#[test]
fn comments_above_includes_empty_comment_line() {
    // `#` with no text is included as a blank line in the block
    let content = "# this is also included\n#\n# doc for func_four\nfunc_four() { echo hi; }\n";
    let a = analyser_with(content);
    assert_eq!(
        a.comments_above(URI, 3).as_deref(),
        Some("```txt\nthis is also included\n\ndoc for func_four\n```"),
    );
}

#[test]
fn comments_above_none_when_blank_line_above() {
    let content = "\nmy_other_var=\"no comments\"\n";
    let a = analyser_with(content);
    assert!(a.comments_above(URI, 1).is_none());
}

// --- commandNameAtPoint ---

#[test]
fn command_name_at_point_in_pipeline() {
    let content = "echo hello | grep world\n";
    let a = analyser_with(content);
    assert_eq!(a.command_name_at_point(URI, 0, 0), Some("echo".to_string()));
    assert_eq!(
        a.command_name_at_point(URI, 0, 13),
        Some("grep".to_string())
    );
}

#[test]
fn command_name_at_point_returns_none_for_non_command_position() {
    // the `if` keyword is not a command node
    let content = "if [ 1 ]; then\n  echo hi\nfi\n";
    let a = analyser_with(content);
    assert_eq!(a.command_name_at_point(URI, 0, 0), None);
}

#[test]
fn command_name_at_point_mid_argument() {
    // position on an argument still returns the command name
    let content = "curl -f -L https://example.com\n";
    let a = analyser_with(content);
    assert_eq!(
        a.command_name_at_point(URI, 0, 15),
        Some("curl".to_string())
    );
}

// --- find_occurrences_within ---

#[test]
fn find_occurrences_within_no_scope_finds_all() {
    let content = "myvar=1\nfunc() { myvar=2; echo $myvar; }\necho $myvar\n";
    let a = analyser_with(content);
    let ranges = a.find_occurrences_within(URI, "myvar", SymbolKind::VARIABLE, None, None);
    assert!(
        ranges.len() >= 3,
        "expected all occurrences, got {}",
        ranges.len()
    );
}

#[test]
fn find_occurrences_within_function_scope_excludes_outside() {
    // local declaration inside function: scope = the function's range
    let content = "outervar=1\nfunc() {\n  local outervar=2\n  echo $outervar\n}\necho $outervar\n";
    let a = analyser_with(content);
    // function body covers lines 1-4 (0-indexed)
    let scope = Range {
        start: Position::new(1, 0),
        end: Position::new(4, 1),
    };
    let ranges =
        a.find_occurrences_within(URI, "outervar", SymbolKind::VARIABLE, None, Some(scope));
    // Should only include occurrences within the function (lines 2 and 3), not line 0 or 5
    for r in &ranges {
        assert!(
            r.start.line >= 1 && r.start.line <= 4,
            "range outside function scope: {:?}",
            r,
        );
    }
}

#[test]
fn find_occurrences_within_function_kind() {
    let content = "myfunc() { echo hi; }\nmyfunc\nmyfunc\n";
    let a = analyser_with(content);
    let ranges = a.find_occurrences_within(URI, "myfunc", SymbolKind::FUNCTION, None, None);
    assert_eq!(
        ranges.len(),
        3,
        "expected def + 2 calls, got {}",
        ranges.len()
    );
}

// --- find_all_linked_uris ---

#[test]
fn find_all_linked_uris_returns_reverse_sourcers() {
    let dir = std::env::temp_dir().join("bashls_test_linked");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("lib.sh"), "lib_func() { echo lib; }\n").unwrap();
    let main_path = dir.join("main.sh");
    fs::write(&main_path, "source ./lib.sh\n").unwrap();

    let lib_uri = format!("file://{}", dir.join("lib.sh").display());
    let main_uri = format!("file://{}", main_path.display());

    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(&lib_uri, "lib_func() { echo lib; }\n");
    a.analyze(&main_uri, &fs::read_to_string(&main_path).unwrap());

    let linked = a.find_all_linked_uris(&lib_uri);
    assert!(
        linked.contains(&main_uri),
        "main.sh should be linked to lib.sh; got {:?}",
        linked,
    );

    fs::remove_dir_all(&dir).ok();
}

// --- find_declaration_locations for source paths ---

#[test]
fn find_declaration_locations_for_sourced_file_path() {
    let dir = std::env::temp_dir().join("bashls_test_decl_src");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("helper.sh"), "helper_func() { echo hi; }\n").unwrap();

    let main_path = dir.join("main.sh");
    fs::write(&main_path, "source ./helper.sh\nhelper_func\n").unwrap();

    let main_uri = format!("file://{}", main_path.display());
    let content = fs::read_to_string(&main_path).unwrap();

    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(&main_uri, &content);

    // position on the path in the source statement (line 0, col 7 = within "./helper.sh")
    let locs = a.find_declaration_locations(&main_uri, "./helper.sh", Position::new(0, 10));
    assert!(!locs.is_empty(), "expected location for sourced file path");
    let loc_uri = locs[0].uri.as_str();
    assert!(
        loc_uri.contains("helper.sh"),
        "expected helper.sh in location uri, got {}",
        loc_uri,
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn definition_resolves_function_call_in_sourced_file() {
    let dir = std::env::temp_dir().join("bashls_test_decl_src_call");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("lib.sh"), "greet() {\n  echo \"hi, $1\"\n}\n").unwrap();

    let main_path = dir.join("main.sh");
    fs::write(&main_path, "source ./lib.sh\n\ngreet \"world\"\n").unwrap();

    let main_uri = format!("file://{}", main_path.display());
    let content = fs::read_to_string(&main_path).unwrap();

    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(&main_uri, &content);

    // position on the `greet` call (line 2, col 0)
    let locs = a.find_declaration_locations(&main_uri, "greet", Position::new(2, 0));
    assert!(
        !locs.is_empty(),
        "expected `greet` call to resolve into the sourced file"
    );
    assert!(
        locs[0].uri.as_str().contains("lib.sh"),
        "expected lib.sh in location uri, got {}",
        locs[0].uri.as_str(),
    );
    assert_eq!(locs[0].range.start.line, 0, "expected jump to `greet() {{`");

    fs::remove_dir_all(&dir).ok();
}

// --- scope-aware declarations (findDeclarationsMatchingWord) ---

#[test]
fn declarations_matching_word_scope_aware_no_results_before_definition() {
    // X is defined at line 1; at position (0, 0) it should not be visible yet
    let content = "echo start\nX=\"Horse\"\nX=\"Mouse\"\n";
    let mut a = analyser_with(content);
    let result = a.find_declarations_matching_word(URI, "X", Some(Position::new(0, 0)), true);
    assert!(
        result.is_empty(),
        "X should not be visible before its definition"
    );
}

#[test]
fn declarations_matching_word_returns_last_global_at_end() {
    let content = "X=\"Horse\"\nX=\"Mouse\"\n";
    let mut a = analyser_with(content);
    // At line 1000 (past end of file), should see the last global definition
    let result = a.find_declarations_matching_word(URI, "X", Some(Position::new(1000, 0)), true);
    assert!(!result.is_empty(), "X should be visible at end of file");
}

// --- find_original_declaration ---

#[test]
fn find_original_declaration_global_variable() {
    let content = "myvar=42\necho $myvar\n";
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(URI, content);
    let (decl, parent) =
        a.find_original_declaration(URI, Position::new(1, 6), "myvar", SymbolKind::VARIABLE);
    assert!(decl.is_some(), "expected declaration location");
    assert!(parent.is_none(), "global var should have no parent scope");
    assert_eq!(decl.unwrap().range.start.line, 0);
}

#[test]
fn find_original_declaration_local_variable_has_parent_scope() {
    let content = "func() {\n  local myvar=42\n  echo $myvar\n}\n";
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(URI, content);
    // position on $myvar usage at line 2
    let (decl, parent) =
        a.find_original_declaration(URI, Position::new(2, 8), "myvar", SymbolKind::VARIABLE);
    assert!(decl.is_some(), "expected declaration location");
    assert!(
        parent.is_some(),
        "local var should have a parent scope (the function)"
    );
}

// --- Rename scope-awareness ---

#[test]
fn rename_local_variable_stays_within_function() {
    // `local x` inside func: occurrences of x within func should be found; x outside is separate
    let content = "x=global\nfunc() {\n  local x=local\n  echo $x\n}\necho $x\n";
    let a = analyser_with(content);
    let parser2 = create_parser().unwrap();
    let mut a2 = Analyser::new(parser2, None);
    a2.analyze(URI, content);
    let (decl, parent) =
        a2.find_original_declaration(URI, Position::new(3, 8), "x", SymbolKind::VARIABLE);
    // local x: should have a parent scope
    assert!(
        parent.is_some(),
        "local variable should be scoped to function"
    );
    let scope = parent.map(|p| p.range);
    let start = decl.map(|d| d.range.start);
    let ranges = a.find_occurrences_within(URI, "x", SymbolKind::VARIABLE, start, scope);
    // Should not include line 0 (x=global) or line 5 (echo $x outside function)
    for r in &ranges {
        assert!(
            r.start.line >= 1 && r.start.line <= 4,
            "local rename should not touch outside scope: line {}",
            r.start.line,
        );
    }
}

#[test]
fn rename_global_variable_finds_all_occurrences() {
    let content = "myvar=1\necho $myvar\nmyvar=2\n";
    let a = analyser_with(content);
    let locs = a.find_occurrences(URI, "myvar");
    assert_eq!(locs.len(), 3, "global rename should cover all occurrences");
}

// --- getDeclarationsForUri ---

#[test]
fn get_declarations_for_uri_returns_empty_for_unknown_uri() {
    let a = analyser_with("foo=1\n");
    let result = a.get_declarations_for_uri("file:///nonexistent.sh");
    assert!(result.is_empty());
}

#[test]
fn get_declarations_for_uri_returns_symbols() {
    let a = analyser_with("myfunc() { echo hi; }\nmyvar=1\n");
    let result = a.get_declarations_for_uri(URI);
    assert!(!result.is_empty());
    let names: Vec<&str> = result.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"myfunc"));
    assert!(names.contains(&"myvar"));
}

// --- word_at_point edge cases ---

#[test]
fn word_at_point_returns_none_at_equals_sign() {
    // In `ret=$?`, column 3 (=) should not return a word
    let content = "ret=$?\n";
    let a = analyser_with(content);
    // "ret" is at 0-2, "=" at 3 — the `=` is not a word node
    let w = a.word_at_point(URI, 0, 3);
    assert!(
        w.is_none() || w.as_deref() == Some("="),
        "col 3 is the = sign: {:?}",
        w
    );
}

#[test]
fn word_at_point_keyword() {
    let a = analyser_with("if true; then\n  echo ok\nelse\n  echo no\nfi\n");
    let w = a.word_at_point(URI, 2, 2);
    assert_eq!(w.as_deref(), Some("else"));
}

// --- find_references returns empty for unknown word ---

#[test]
fn find_references_empty_for_unknown_word() {
    let a = analyser_with("echo hello\n");
    let locs = a.find_references("foobar_unknown");
    assert!(locs.is_empty());
}

// --- background_analysis ---

#[test]
fn background_analysis_max_files_zero_skips() {
    let dir = std::env::temp_dir().join("bashls_test_bg_zero");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.sh"), "#!/bin/bash\na=1\n").unwrap();
    let workspace_uri = format!("file://{}", dir.display());
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, Some(workspace_uri));
    let count = a.background_analysis("**/*.sh", 0);
    assert_eq!(count, 0, "max_files=0 should analyze 0 files");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn background_analysis_finds_bash_files() {
    let dir = std::env::temp_dir().join("bashls_test_bg_find");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.sh"), "#!/bin/bash\na=1\n").unwrap();
    fs::write(dir.join("b.sh"), "#!/bin/bash\nb=2\n").unwrap();
    fs::write(dir.join("skip.py"), "print('not bash')\n").unwrap();
    let workspace_uri = format!("file://{}", dir.display());
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, Some(workspace_uri));
    let count = a.background_analysis("**/*.sh", 100);
    assert_eq!(count, 2, "should find exactly 2 .sh files");
    fs::remove_dir_all(&dir).ok();
}

// --- get_all_variables includes sourced file symbols ---

#[test]
fn get_all_variables_includes_sourced_file_vars() {
    let dir = std::env::temp_dir().join("bashls_test_vars_sourced");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("lib.sh"), "lib_var=sourced_value\n").unwrap();
    let main_path = dir.join("main.sh");
    fs::write(&main_path, "source ./lib.sh\nmain_var=local\n").unwrap();
    let main_uri = format!("file://{}", main_path.display());
    let content = fs::read_to_string(&main_path).unwrap();
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.analyze(&main_uri, &content);
    let vars = a.get_all_variables(&main_uri, Position::new(9999, 0));
    let names: Vec<&str> = vars.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"main_var"), "should include local var");
    assert!(
        names.contains(&"lib_var"),
        "should include sourced var: {:?}",
        names
    );
    fs::remove_dir_all(&dir).ok();
}

// --- include_all_workspace_symbols ---

#[test]
fn include_all_workspace_symbols_finds_symbols_across_files() {
    let uri2 = "file:///other.sh";
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.set_include_all_workspace_symbols(true);
    a.analyze(URI, "local_func() { echo hi; }\n");
    a.analyze(uri2, "other_func() { echo there; }\n");
    let result = a.find_declarations_matching_word(URI, "other_func", None, false);
    assert!(
        !result.is_empty(),
        "include_all_workspace_symbols should find symbols in other files"
    );
}

// --- source error diagnostics ---

#[test]
fn source_error_diagnostic_non_constant_source() {
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.set_enable_source_error_diagnostics(true);
    let diags = a.analyze(URI, "source \"$LIBPATH\"\n");
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0]
            .message
            .contains("non-constant source not supported"),
        "unexpected message: {}",
        diags[0].message,
    );
}

#[test]
fn source_error_diagnostic_failed_to_resolve_path() {
    let parser = create_parser().unwrap();
    let mut a = Analyser::new(parser, None);
    a.set_enable_source_error_diagnostics(true);
    let diags = a.analyze(URI, "source ./no_such_file_xyz.sh\n");
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0].message.contains("failed to resolve path"),
        "unexpected message: {}",
        diags[0].message,
    );
}
