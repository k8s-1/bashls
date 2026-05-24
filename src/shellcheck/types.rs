use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ShellCheckResult {
    pub comments: Vec<ShellCheckComment>,
}

impl ShellCheckResult {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCheckComment {
    pub line: u32,
    pub end_line: u32,
    pub column: u32,
    pub end_column: u32,
    pub level: ShellCheckLevel,
    pub code: u32,
    pub message: String,
    pub fix: Option<ShellCheckFix>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ShellCheckLevel {
    Error,
    Warning,
    Info,
    Style,
}

impl ShellCheckLevel {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellCheckLevel::Error => "error",
            ShellCheckLevel::Warning => "warning",
            ShellCheckLevel::Info => "info",
            ShellCheckLevel::Style => "style",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ShellCheckFix {
    pub replacements: Vec<ShellCheckReplacement>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShellCheckReplacement {
    pub line: u32,
    pub end_line: u32,
    pub column: u32,
    pub end_column: u32,
    pub replacement: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_COMMENT: &str = r#"{
        "line": 3, "endLine": 3, "column": 7, "endColumn": 13,
        "level": "warning", "code": 2086,
        "message": "Double quote to prevent globbing and word splitting.",
        "fix": null
    }"#;

    fn wrap(comments: &str) -> String {
        format!(r#"{{"comments": [{comments}]}}"#)
    }

    #[test]
    fn valid_single_comment_deserializes() {
        let json = wrap(VALID_COMMENT);
        let r = ShellCheckResult::from_json(&json).unwrap();
        assert_eq!(r.comments.len(), 1);
        let c = &r.comments[0];
        assert_eq!(c.line, 3);
        assert_eq!(c.end_line, 3);
        assert_eq!(c.column, 7);
        assert_eq!(c.end_column, 13);
        assert_eq!(c.code, 2086);
        assert!(matches!(c.level, ShellCheckLevel::Warning));
        assert!(c.fix.is_none());
    }

    #[test]
    fn valid_two_comment_array_deserializes() {
        let c2 = r#"{"line":1,"endLine":1,"column":1,"endColumn":2,"level":"error","code":1000,"message":"x","fix":null}"#;
        let json = wrap(&format!("{VALID_COMMENT},{c2}"));
        let r = ShellCheckResult::from_json(&json).unwrap();
        assert_eq!(r.comments.len(), 2);
    }

    #[test]
    fn comments_null_fails() {
        let r = ShellCheckResult::from_json(r#"{"comments": null}"#);
        assert!(r.is_err());
    }

    #[test]
    fn comments_string_array_fails() {
        let r = ShellCheckResult::from_json(r#"{"comments": ["foo"]}"#);
        assert!(r.is_err());
    }

    #[test]
    fn wrong_field_types_fail() {
        let bad_line = r#"{"comments": [{"line":"three","endLine":1,"column":1,"endColumn":2,"level":"error","code":1000,"message":"x","fix":null}]}"#;
        assert!(ShellCheckResult::from_json(bad_line).is_err());

        let missing_message = r#"{"comments": [{"line":1,"endLine":1,"column":1,"endColumn":2,"level":"error","code":1000,"fix":null}]}"#;
        assert!(ShellCheckResult::from_json(missing_message).is_err());
    }
}
