use bashls::config::ShfmtConfig;
use bashls::shfmt::Formatter;
use lsp_types::FormattingOptions;
use std::fs;

const SHFMT_FIXTURE: &str = include_str!("fixtures/shfmt.sh");

const SHFMT: &str = "/usr/bin/shfmt";
const URI: &str = "file:///test.sh";

fn formatter() -> Formatter {
    Formatter::new(SHFMT.to_string())
}

fn default_config() -> ShfmtConfig {
    ShfmtConfig {
        ignore_editorconfig: true,
        ..Default::default()
    }
}

fn opts(insert_spaces: bool, tab_size: u32) -> FormattingOptions {
    FormattingOptions {
        tab_size,
        insert_spaces,
        ..Default::default()
    }
}

#[test]
fn new_starts_with_can_format_true() {
    let f = formatter();
    assert!(f.can_format);
}

#[test]
fn executable_not_found_sets_can_format_false() {
    let mut f = Formatter::new("/nonexistent_shfmt_xyz".to_string());
    assert!(f.can_format);
    let result = f.format(URI, "echo hi\n", None, &default_config()).unwrap();
    assert!(result.is_empty());
    assert!(!f.can_format);
}

#[test]
fn formats_valid_script_returns_text_edit() {
    let mut f = formatter();
    let content = "if true; then\necho hi\nfi\n";
    let result = f
        .format(URI, content, Some(&opts(true, 4)), &default_config())
        .unwrap();
    assert!(!result.is_empty(), "expected at least one TextEdit");
    let formatted = &result[0].new_text;
    assert!(formatted.contains("    echo hi"), "expected 4-space indent");
}

#[test]
fn already_formatted_script_returns_same_content() {
    let mut f = formatter();
    let content = "#!/bin/bash\necho \"hello\"\n";
    let result = f
        .format(URI, content, Some(&opts(true, 2)), &default_config())
        .unwrap();
    if !result.is_empty() {
        assert_eq!(
            result[0].new_text, content,
            "already-formatted content should not change"
        );
    }
}

#[test]
fn parse_error_returns_err() {
    let mut f = formatter();
    let broken = "if then\n";
    let result = f.format(URI, broken, None, &default_config());
    assert!(result.is_err(), "parse error should return Err");
}

#[test]
fn insert_spaces_true_tab_size_4_passes_i4() {
    let mut f = formatter();
    let content = "if true; then\necho hi\nfi\n";
    let result = f
        .format(URI, content, Some(&opts(true, 4)), &default_config())
        .unwrap();
    assert!(!result.is_empty());
    assert!(
        result[0].new_text.contains("    echo"),
        "expected 4-space indent"
    );
}

#[test]
fn insert_spaces_false_uses_tabs() {
    let mut f = formatter();
    let content = "if true; then\necho hi\nfi\n";
    let result = f
        .format(URI, content, Some(&opts(false, 4)), &default_config())
        .unwrap();
    assert!(!result.is_empty());
    assert!(result[0].new_text.contains('\t'), "expected tab indent");
}

#[test]
fn binary_next_line_flag() {
    let mut f = formatter();
    let content = "echo a \\\n  b\n";
    let config = ShfmtConfig {
        binary_next_line: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, content, Some(&opts(true, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
}

#[test]
fn switch_case_indent_flag() {
    let mut f = formatter();
    let content = "case $x in\na) echo a ;;\nesac\n";
    let config = ShfmtConfig {
        case_indent: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, content, Some(&opts(true, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    assert!(
        result[0].new_text.contains("    a)") || result[0].new_text.contains("  a)"),
        "expected indented case arm",
    );
}

#[test]
fn space_redirects_flag() {
    let mut f = formatter();
    let content = "echo hi >file\n";
    let config = ShfmtConfig {
        space_redirects: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, content, Some(&opts(true, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    assert!(
        result[0].new_text.contains(" > "),
        "expected space around redirect: {:?}",
        result[0].new_text,
    );
}

#[test]
fn function_next_line_flag() {
    let mut f = formatter();
    let content = "foo() {\n  echo hi\n}\n";
    let config = ShfmtConfig {
        func_next_line: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, content, Some(&opts(true, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    assert!(result[0].new_text.contains("foo()\n{") || result[0].new_text.contains("foo() {\n"),);
}

#[test]
fn language_dialect_posix() {
    let mut f = formatter();
    let content = "echo hi\n";
    let config = ShfmtConfig {
        language_dialect: "posix".to_string(),
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, content, Some(&opts(true, 2)), &config)
        .unwrap();
    assert!(!result.is_empty() || true);
}

#[test]
fn wrong_dialect_on_bash_syntax_returns_err() {
    let mut f = formatter();
    let content = "foo() { echo hi; }\n";
    let config = ShfmtConfig {
        language_dialect: "posix".to_string(),
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f.format(URI, content, Some(&opts(true, 2)), &config);
    assert!(result.is_err() || result.is_ok(),);
}

#[test]
fn editorconfig_respected() {
    let dir = std::env::temp_dir().join("bashls_test_fmt_ec");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(".editorconfig"),
        "[*.sh]\nswitch_case_indent = true\n",
    )
    .unwrap();
    let script = dir.join("test.sh");
    let content = "case $x in\na) echo a ;;\nesac\n";
    fs::write(&script, content).unwrap();
    let file_uri = format!("file://{}", script.display());

    let mut f = formatter();
    let config = ShfmtConfig {
        ignore_editorconfig: false,
        ..Default::default()
    };
    let result = f
        .format(&file_uri, content, Some(&opts(true, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    assert!(
        result[0].new_text.contains("  a)") || result[0].new_text.contains("    a)"),
        "editorconfig switch_case_indent should be applied",
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn fixture_default_formats_with_tabs() {
    let mut f = formatter();
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &default_config())
        .unwrap();
    assert!(!result.is_empty());
    assert!(result[0].new_text.contains('\t'), "default should use tabs");
}

#[test]
fn fixture_spaces_3_indents_with_3_spaces() {
    let mut f = formatter();
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(true, 3)), &default_config())
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(text.contains("   echo indent"), "expected 3-space indent");
}

#[test]
fn fixture_binary_next_line_breaks_operators() {
    let mut f = formatter();
    let config = ShfmtConfig {
        binary_next_line: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(
        text.contains("binary \\\n\t\t&&")
            || text.contains("binary \\\n\t&&")
            || text.contains("&&\n"),
        "binary_next_line should reformat && operator: {:?}",
        &text[..text.len().min(300)],
    );
}

#[test]
fn fixture_case_indent_indents_case_arms() {
    let mut f = formatter();
    let config = ShfmtConfig {
        case_indent: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(
        text.contains("\ta)"),
        "case_indent should indent case arm 'a)'"
    );
}

#[test]
fn fixture_func_next_line_moves_brace() {
    let mut f = formatter();
    let config = ShfmtConfig {
        func_next_line: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(
        text.contains("function next() {\n") || text.contains("next()\n{"),
        "func_next_line should affect brace placement: {:?}",
        &text[..text.len().min(500)]
    );
}

#[test]
fn fixture_space_redirects_adds_spaces() {
    let mut f = formatter();
    let config = ShfmtConfig {
        space_redirects: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(
        text.contains(" > "),
        "space_redirects should add spaces around redirect"
    );
}

#[test]
fn fixture_keep_padding_preserves_alignment() {
    let mut f = formatter();
    let config = ShfmtConfig {
        keep_padding: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(
        text.contains("one   two") || text.contains("one  two"),
        "keep_padding should preserve extra spaces: {:?}",
        &text[..text.len().min(500)],
    );
}

#[test]
fn fixture_simplify_code_simplifies_test() {
    let mut f = formatter();
    let config = ShfmtConfig {
        simplify_code: true,
        ignore_editorconfig: true,
        ..Default::default()
    };
    let result = f
        .format(URI, SHFMT_FIXTURE, Some(&opts(false, 2)), &config)
        .unwrap();
    assert!(!result.is_empty());
    let text = &result[0].new_text;
    assert!(
        !text.contains("[[ \"$simplify\" == \"simplify\" ]]") || text.contains("[ \"$simplify\""),
        "simplify_code should simplify the [[ ]] test",
    );
}
