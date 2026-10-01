use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub background_analysis_max_files: usize,
    pub enable_source_error_diagnostics: bool,
    pub glob_pattern: String,
    pub include_all_workspace_symbols: bool,
    pub shellcheck_external_sources: bool,
    pub shellcheck_arguments: Vec<String>,
    pub shellcheck_path: String,
    pub shfmt: ShfmtConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            background_analysis_max_files: 500,
            enable_source_error_diagnostics: false,
            glob_pattern: "**/*@(.sh|.inc|.bash|.command)".to_string(),
            include_all_workspace_symbols: false,
            shellcheck_external_sources: true,
            shellcheck_arguments: vec![],
            shellcheck_path: "shellcheck".to_string(),
            shfmt: ShfmtConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShfmtConfig {
    pub path: String,
    pub ignore_editorconfig: bool,
    pub language_dialect: String,
    pub binary_next_line: bool,
    pub case_indent: bool,
    pub func_next_line: bool,
    pub keep_padding: bool,
    pub simplify_code: bool,
    pub space_redirects: bool,
}

impl Default for ShfmtConfig {
    fn default() -> Self {
        Self {
            path: "shfmt".to_string(),
            ignore_editorconfig: false,
            language_dialect: "auto".to_string(),
            binary_next_line: false,
            case_indent: false,
            func_next_line: false,
            keep_padding: false,
            simplify_code: false,
            space_redirects: false,
        }
    }
}

fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(existing) => merge_json(existing, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

impl Config {
    #[must_use]
    pub fn with_overrides(&self, value: &serde_json::Value) -> Option<Self> {
        let overlay = value.get("bashIde").unwrap_or(value);
        if !overlay.is_object() {
            return None;
        }
        let mut merged = serde_json::to_value(self).ok()?;
        merge_json(&mut merged, overlay);
        serde_json::from_value(merged).ok()
    }

    #[must_use]
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("BACKGROUND_ANALYSIS_MAX_FILES")
            && let Ok(n) = v.parse()
        {
            cfg.background_analysis_max_files = n;
        }
        if let Ok(v) = std::env::var("GLOB_PATTERN") {
            cfg.glob_pattern = v;
        }
        if let Ok(v) = std::env::var("SHELLCHECK_PATH") {
            cfg.shellcheck_path = v;
        }
        if let Ok(v) = std::env::var("SHELLCHECK_ARGUMENTS") {
            cfg.shellcheck_arguments = v
                .split_whitespace()
                .map(std::string::ToString::to_string)
                .collect();
        }
        if let Ok(v) = std::env::var("SHELLCHECK_EXTERNAL_SOURCES") {
            cfg.shellcheck_external_sources = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("ENABLE_SOURCE_ERROR_DIAGNOSTICS") {
            cfg.enable_source_error_diagnostics = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("INCLUDE_ALL_WORKSPACE_SYMBOLS") {
            cfg.include_all_workspace_symbols = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_PATH") {
            cfg.shfmt.path = v;
        }
        if let Ok(v) = std::env::var("SHFMT_LANGUAGE_DIALECT") {
            cfg.shfmt.language_dialect = v;
        }
        if let Ok(v) = std::env::var("SHFMT_BINARY_NEXT_LINE") {
            cfg.shfmt.binary_next_line = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_CASE_INDENT") {
            cfg.shfmt.case_indent = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_FUNC_NEXT_LINE") {
            cfg.shfmt.func_next_line = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_KEEP_PADDING") {
            cfg.shfmt.keep_padding = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_SIMPLIFY_CODE") {
            cfg.shfmt.simplify_code = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_SPACE_REDIRECTS") {
            cfg.shfmt.space_redirects = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("SHFMT_IGNORE_EDITORCONFIG") {
            cfg.shfmt.ignore_editorconfig = v == "true" || v == "1";
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ShfmtConfig};
    use serde_json::json;

    #[test]
    fn overrides_under_bash_ide() {
        let cfg = Config::default()
            .with_overrides(
                &json!({"bashIde": {"shellcheckPath": "/x/sc", "shfmt": {"path": "/x/fmt"}}}),
            )
            .unwrap();
        assert_eq!(cfg.shellcheck_path, "/x/sc");
        assert_eq!(cfg.shfmt.path, "/x/fmt");
    }

    #[test]
    fn overrides_flat_object() {
        let cfg = Config::default()
            .with_overrides(&json!({"globPattern": "*.sh"}))
            .unwrap();
        assert_eq!(cfg.glob_pattern, "*.sh");
    }

    #[test]
    fn overrides_preserve_unspecified_base_values() {
        let base = Config {
            shellcheck_path: "/env/sc".to_string(),
            shfmt: ShfmtConfig {
                language_dialect: "bash".to_string(),
                ..ShfmtConfig::default()
            },
            ..Config::default()
        };
        let cfg = base
            .with_overrides(&json!({"bashIde": {"shfmt": {"path": "/x/fmt"}}}))
            .unwrap();
        assert_eq!(cfg.shellcheck_path, "/env/sc");
        assert_eq!(cfg.shfmt.language_dialect, "bash");
        assert_eq!(cfg.shfmt.path, "/x/fmt");
    }

    #[test]
    fn overrides_reject_non_object() {
        assert!(Config::default().with_overrides(&json!(null)).is_none());
        assert!(Config::default().with_overrides(&json!("x")).is_none());
    }

    #[test]
    fn overrides_reject_wrong_types() {
        assert!(
            Config::default()
                .with_overrides(&json!({"bashIde": {"shellcheckPath": 5}}))
                .is_none()
        );
    }
}
