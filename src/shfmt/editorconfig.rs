use crate::config::ShfmtConfig;

pub(super) struct EditorconfigShfmt {
    pub(super) binary_next_line: Option<bool>,
    pub(super) case_indent: Option<bool>,
    pub(super) func_next_line: Option<bool>,
    pub(super) keep_padding: Option<bool>,
    pub(super) space_redirects: Option<bool>,
    pub(super) language_dialect: Option<String>,
}

pub(super) fn read_editorconfig(path: &str) -> Option<EditorconfigShfmt> {
    let editorconfig_path = find_editorconfig(path)?;
    let content = std::fs::read_to_string(editorconfig_path).ok()?;

    let mut result = EditorconfigShfmt {
        binary_next_line: None,
        case_indent: None,
        func_next_line: None,
        keep_padding: None,
        space_redirects: None,
        language_dialect: None,
    };

    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.contains('*') || {
                let section = line.trim_start_matches('[').trim_end_matches(']');
                path.ends_with(section.trim_matches('*'))
            };
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "binary_next_line" => result.binary_next_line = Some(val == "true"),
                "switch_case_indent" => result.case_indent = Some(val == "true"),
                "function_next_line" => result.func_next_line = Some(val == "true"),
                "keep_padding" => result.keep_padding = Some(val == "true"),
                "space_redirects" => result.space_redirects = Some(val == "true"),
                "shell_variant" => result.language_dialect = Some(val.to_string()),
                _ => {}
            }
        }
    }

    let has_shfmt_config = result.binary_next_line.is_some()
        || result.case_indent.is_some()
        || result.func_next_line.is_some()
        || result.keep_padding.is_some()
        || result.space_redirects.is_some()
        || result.language_dialect.is_some();

    if has_shfmt_config { Some(result) } else { None }
}

fn find_editorconfig(path: &str) -> Option<std::path::PathBuf> {
    let mut dir = std::path::Path::new(path).parent()?;
    loop {
        let candidate = dir.join(".editorconfig");
        if candidate.exists() {
            return Some(candidate);
        }
        let parent = dir.parent()?;
        if parent == dir {
            break;
        }
        dir = parent;
    }
    None
}

pub(super) fn apply_editorconfig(mut config: ShfmtConfig, ec: EditorconfigShfmt) -> ShfmtConfig {
    if let Some(v) = ec.binary_next_line {
        config.binary_next_line = v;
    }
    if let Some(v) = ec.case_indent {
        config.case_indent = v;
    }
    if let Some(v) = ec.func_next_line {
        config.func_next_line = v;
    }
    if let Some(v) = ec.keep_padding {
        config.keep_padding = v;
    }
    if let Some(v) = ec.space_redirects {
        config.space_redirects = v;
    }
    if let Some(v) = ec.language_dialect {
        config.language_dialect = v;
    }
    config
}
