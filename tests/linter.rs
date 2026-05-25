#![deny(warnings)]
use bashls::shellcheck::Linter;

const SHELLCHECK: &str = "/usr/bin/shellcheck";

fn linter() -> Linter {
    Linter::new(SHELLCHECK.to_string(), false)
}

const URI: &str = "file:///test.sh";

#[test]
fn new_starts_with_can_lint_true() {
    let l = linter();
    assert!(l.can_lint);
}

#[test]
fn executable_not_found_disables_linting() {
    let l = Linter::new("/nonexistent_shellcheck_xyz".to_string(), false);
    assert!(!l.can_lint);
}

#[test]
fn lints_unquoted_variable() {
    let l = linter();
    let content = "#!/bin/bash\nfoo=$1\necho $foo\n";
    let result = l.lint(URI, content, &[], &[]);
    assert!(!result.diagnostics.is_empty(), "expected SC2086 diagnostic");
    let codes: Vec<String> = result
        .diagnostics
        .iter()
        .filter_map(|d| {
            if let Some(lsp_types::NumberOrString::String(ref s)) = d.code {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        codes.iter().any(|c| c == "SC2086"),
        "expected SC2086, got: {:?}",
        codes,
    );
}

#[test]
fn lints_uninitialized_variable_produces_code_and_range() {
    let l = linter();
    let content = "#!/bin/bash\necho $undefined_var\n";
    let result = l.lint(URI, content, &[], &[]);
    assert!(!result.diagnostics.is_empty());
    let d = &result.diagnostics[0];
    assert!(d.range.start.line < 10, "range should be valid");
    assert!(d.source.as_deref() == Some("shellcheck"));
}

#[test]
fn non_file_uri_returns_empty() {
    let l = linter();
    let result = l.lint(
        "webdav://server/script.sh",
        "#!/bin/bash\necho hi\n",
        &[],
        &[],
    );
    assert!(
        result.diagnostics.is_empty(),
        "non-file URI should produce no diagnostics"
    );
}

#[test]
fn clean_script_produces_no_diagnostics() {
    let l = linter();
    let content = "#!/bin/bash\nfoo=\"bar\"\necho \"$foo\"\n";
    let result = l.lint(URI, content, &[], &[]);
    assert!(
        result.diagnostics.is_empty(),
        "well-formed script should be clean"
    );
}

#[test]
fn code_action_created_for_fixable_issue() {
    let l = linter();
    let content = "#!/bin/bash\nfoo=$1\necho $foo\n";
    let result = l.lint(URI, content, &[], &[]);
    assert!(
        !result.code_actions.is_empty(),
        "expected code action for SC2086"
    );
}

#[test]
fn source_path_arg_passed_to_shellcheck() {
    use std::fs;
    let dir = std::env::temp_dir().join("bashls_test_linter_src");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("sourced.sh"), "sourced_var=1\n").unwrap();

    let content = format!("#!/bin/bash\nsource sourced.sh\necho \"$sourced_var\"\n");
    let l = Linter::new(SHELLCHECK.to_string(), true);
    let source_paths = vec![dir.to_string_lossy().into_owned()];
    let result = l.lint(URI, &content, &source_paths, &[]);
    let codes: Vec<String> = result
        .diagnostics
        .iter()
        .filter_map(|d| {
            if let Some(lsp_types::NumberOrString::String(ref s)) = d.code {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        !codes.contains(&"SC1091".to_string()),
        "source-path should resolve sourced.sh; got: {:?}",
        codes,
    );
    fs::remove_dir_all(&dir).ok();
}
