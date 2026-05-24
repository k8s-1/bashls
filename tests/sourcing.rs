use bashls::parser::create_parser;
use bashls::util::sourcing::get_source_commands;
use std::fs;

fn parse(content: &str) -> tree_sitter::Tree {
    create_parser()
        .unwrap()
        .parse(content.as_bytes(), None)
        .unwrap()
}

const FILE_URI: &str = "file:///test/file.sh";

#[test]
fn empty_content_returns_no_source_commands() {
    let tree = parse("");
    let cmds = get_source_commands(&tree, FILE_URI, None, b"");
    assert!(cmds.is_empty());
}

#[test]
fn absolute_path_resolved_when_file_exists() {
    let dir = std::env::temp_dir().join("bashls_test_sourcing_abs");
    fs::create_dir_all(&dir).unwrap();
    let abs = dir.join("lib.sh");
    fs::write(&abs, "").unwrap();
    let abs_str = abs.to_string_lossy();
    let content = format!("source {abs_str}\n");
    let tree = parse(&content);
    let cmds = get_source_commands(&tree, FILE_URI, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].uri.is_some(), "absolute path should resolve");
    assert!(cmds[0].error.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn relative_path_resolved_against_file_dir() {
    let dir = std::env::temp_dir().join("bashls_test_sourcing_rel");
    fs::create_dir_all(&dir).unwrap();
    let helper = dir.join("helper.sh");
    fs::write(&helper, "").unwrap();
    let main = dir.join("main.sh");
    let content = "source ./helper.sh\n";
    fs::write(&main, content).unwrap();
    let file_uri = format!("file://{}", main.to_string_lossy());
    let tree = parse(content);
    let cmds = get_source_commands(&tree, &file_uri, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(
        cmds[0].uri.is_some(),
        "relative path should resolve against file dir"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn dynamic_source_produces_error() {
    let content = "source \"$LIBPATH\"\n";
    let tree = parse(content);
    let cmds = get_source_commands(&tree, FILE_URI, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].uri.is_none());
    assert!(cmds[0].error.as_deref() == Some("non-constant source not supported"));
}

#[test]
fn shellcheck_source_devnull_suppresses_command() {
    let content = "# shellcheck source=/dev/null\nsource ./IM_NOT_THERE.sh\n";
    let tree = parse(content);
    let cmds = get_source_commands(&tree, FILE_URI, None, content.as_bytes());
    // source=/dev/null means "ignore this source command"
    assert!(
        cmds.is_empty(),
        "source=/dev/null should suppress the command"
    );
}

#[test]
fn shellcheck_source_overrides_dynamic() {
    let dir = std::env::temp_dir().join("bashls_test_sourcing_sc");
    fs::create_dir_all(&dir).unwrap();
    let target = dir.join("override.sh");
    fs::write(&target, "").unwrap();
    let target_str = target.to_string_lossy();
    let content = format!("# shellcheck source={target_str}\nsource \"$X\"\n");
    let tree = parse(&content);
    let cmds = get_source_commands(&tree, FILE_URI, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(
        cmds[0].uri.is_some(),
        "shellcheck source= should override dynamic path"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn shellcheck_source_path_resolves_relative_name() {
    let dir = std::env::temp_dir().join("bashls_test_sourcing_sp");
    fs::create_dir_all(&dir).unwrap();
    let target = dir.join("utils.sh");
    fs::write(&target, "").unwrap();
    let dir_str = dir.to_string_lossy();
    let content = format!("# shellcheck source-path={dir_str}\nsource utils.sh\n");
    let tree = parse(&content);
    let cmds = get_source_commands(&tree, FILE_URI, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(
        cmds[0].uri.is_some(),
        "source-path= should resolve the name"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn dynamic_with_leading_var_stripped() {
    // source "$SCRIPT_DIR/staging.sh" — leading expansion stripped, resolves "./staging.sh"
    let dir = std::env::temp_dir().join("bashls_test_sourcing_dyn");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("staging.sh"), "").unwrap();
    let main = dir.join("main.sh");
    let content = "source \"$SCRIPT_DIR/staging.sh\"\n";
    fs::write(&main, content).unwrap();
    let file_uri = format!("file://{}", main.to_string_lossy());
    let tree = parse(content);
    let cmds = get_source_commands(&tree, &file_uri, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(
        cmds[0].uri.is_some(),
        "leading dynamic segment should be stripped"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn source_inside_heredoc_ignored() {
    // source inside a here-doc Python block should not be picked up as a Bash source
    let content = "python2 - <<END\nsource ./this-should-be-ignored.sh\nEND\n";
    let tree = parse(content);
    let cmds = get_source_commands(&tree, FILE_URI, None, content.as_bytes());
    assert!(
        cmds.is_empty() || cmds.iter().all(|c| c.uri.is_none()),
        "source inside heredoc should not be resolved as a Bash source",
    );
}

#[test]
fn dot_command_is_treated_as_source() {
    let dir = std::env::temp_dir().join("bashls_test_dot_source");
    fs::create_dir_all(&dir).unwrap();
    let helper = dir.join("helper.sh");
    fs::write(&helper, "").unwrap();
    let main = dir.join("main.sh");
    let content = ". ./helper.sh\n";
    fs::write(&main, content).unwrap();
    let file_uri = format!("file://{}", main.to_string_lossy());
    let tree = parse(content);
    let cmds = get_source_commands(&tree, &file_uri, None, content.as_bytes());
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].uri.is_some(), ". (dot) should be treated as source");
    fs::remove_dir_all(&dir).ok();
}
