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
            let section = line.trim_start_matches('[').trim_end_matches(']');
            let suffix = section.rfind('*').map_or(section, |i| &section[i + 1..]);
            in_section = suffix.is_empty() || path.ends_with(suffix);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(suffix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bashls_ec_{}_{}", suffix, std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn glob_section_matches_only_files_with_matching_extension() {
        let dir = make_temp_dir("glob");
        fs::write(dir.join(".editorconfig"), "[*.sh]\nshell_variant = posix\n").unwrap();
        let sh = dir.join("foo.sh");
        let bash = dir.join("foo.bash");
        fs::write(&sh, "").unwrap();
        fs::write(&bash, "").unwrap();

        assert!(
            read_editorconfig(&sh.to_string_lossy()).is_some(),
            "[*.sh] should match foo.sh"
        );
        assert!(
            read_editorconfig(&bash.to_string_lossy()).is_none(),
            "[*.sh] should not match foo.bash"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn star_section_matches_all_files() {
        let dir = make_temp_dir("star");
        fs::write(dir.join(".editorconfig"), "[*]\nshell_variant = bash\n").unwrap();
        let sh = dir.join("foo.sh");
        let bash = dir.join("foo.bash");
        fs::write(&sh, "").unwrap();
        fs::write(&bash, "").unwrap();

        assert!(
            read_editorconfig(&sh.to_string_lossy()).is_some(),
            "[*] should match foo.sh"
        );
        assert!(
            read_editorconfig(&bash.to_string_lossy()).is_some(),
            "[*] should match foo.bash"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exact_filename_section_matches_only_that_file() {
        let dir = make_temp_dir("exact");
        fs::write(
            dir.join(".editorconfig"),
            "[foo.sh]\nshell_variant = posix\n",
        )
        .unwrap();
        let sh = dir.join("foo.sh");
        let other = dir.join("bar.sh");
        fs::write(&sh, "").unwrap();
        fs::write(&other, "").unwrap();

        assert!(
            read_editorconfig(&sh.to_string_lossy()).is_some(),
            "[foo.sh] should match foo.sh"
        );
        assert!(
            read_editorconfig(&other.to_string_lossy()).is_none(),
            "[foo.sh] should not match bar.sh"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn double_glob_section_matches_files_with_matching_extension() {
        let dir = make_temp_dir("doubleglob");
        fs::write(
            dir.join(".editorconfig"),
            "[**/*.sh]\nshell_variant = posix\n",
        )
        .unwrap();
        let sh = dir.join("foo.sh");
        let bash = dir.join("foo.bash");
        fs::write(&sh, "").unwrap();
        fs::write(&bash, "").unwrap();

        assert!(
            read_editorconfig(&sh.to_string_lossy()).is_some(),
            "[**/*.sh] should match foo.sh"
        );
        assert!(
            read_editorconfig(&bash.to_string_lossy()).is_none(),
            "[**/*.sh] should not match foo.bash"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_walks_up_to_parent_directory() {
        let dir = make_temp_dir("walkup");
        let subdir = dir.join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(dir.join(".editorconfig"), "[*]\nshell_variant = mksh\n").unwrap();
        let file = subdir.join("foo.sh");
        fs::write(&file, "").unwrap();

        let result = read_editorconfig(&file.to_string_lossy());
        assert!(
            result.is_some(),
            "should find .editorconfig in an ancestor directory"
        );
        assert_eq!(result.unwrap().language_dialect.as_deref(), Some("mksh"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_all_boolean_keys() {
        let dir = make_temp_dir("boolkeys");
        fs::write(
            dir.join(".editorconfig"),
            "[*]\nbinary_next_line = true\nfunction_next_line = true\nkeep_padding = true\nspace_redirects = true\n",
        )
        .unwrap();
        let file = dir.join("foo.sh");
        fs::write(&file, "").unwrap();

        let result = read_editorconfig(&file.to_string_lossy()).expect("should parse config");
        assert_eq!(result.binary_next_line, Some(true));
        assert_eq!(result.func_next_line, Some(true));
        assert_eq!(result.keep_padding, Some(true));
        assert_eq!(result.space_redirects, Some(true));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_relevant_keys_returns_none() {
        let dir = make_temp_dir("norelevant");
        fs::write(dir.join(".editorconfig"), "[*]\nindent_size = 2\n").unwrap();
        let file = dir.join("foo.sh");
        fs::write(&file, "").unwrap();

        assert!(
            read_editorconfig(&file.to_string_lossy()).is_none(),
            "editorconfig with no shfmt-relevant keys should yield None"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_editorconfig_overrides_only_set_fields() {
        let base = ShfmtConfig {
            path: "custom_shfmt".to_string(),
            simplify_code: true,
            ..ShfmtConfig::default()
        };
        let ec = EditorconfigShfmt {
            binary_next_line: Some(true),
            case_indent: None,
            func_next_line: None,
            keep_padding: None,
            space_redirects: None,
            language_dialect: None,
        };

        let result = apply_editorconfig(base, ec);

        assert!(result.binary_next_line, "set field should be applied");
        assert!(!result.case_indent, "unset field should keep default");
        assert_eq!(
            result.path, "custom_shfmt",
            "fields not touched by editorconfig should be preserved"
        );
        assert!(
            result.simplify_code,
            "fields not touched by editorconfig should be preserved"
        );
    }
}
