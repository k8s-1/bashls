use std::path::{Path, PathBuf};

pub fn uri_to_path(uri: &str) -> PathBuf {
    uri_to_path_opt(uri).unwrap_or_else(|| PathBuf::from(uri))
}

pub fn uri_to_path_opt(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    let decoded = percent_decode(path);
    // On Windows, file:///C:/foo decodes to /C:/foo — strip the leading slash.
    let decoded = match decoded.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => &decoded[1..],
        _ => &decoded,
    };
    Some(PathBuf::from(decoded))
}

pub fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut out = String::with_capacity(s.len() + 7);
    out.push_str("file://");
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(b as char);
            }
            _ => {
                let hi = char::from_digit(u32::from(b >> 4), 16)
                    .unwrap()
                    .to_ascii_uppercase();
                let lo = char::from_digit(u32::from(b & 0xf), 16)
                    .unwrap()
                    .to_ascii_uppercase();
                out.push('%');
                out.push(hi);
                out.push(lo);
            }
        }
    }
    out
}

pub fn untildify(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~')
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}{rest}");
    }
    path.to_string()
}

pub fn get_file_paths(workspace_root: &Path, glob_pattern: &str, max_items: usize) -> Vec<PathBuf> {
    let suffixes: Vec<String> = expand_extglob(glob_pattern)
        .into_iter()
        .filter_map(|p| p.rfind('*').map(|i| p[i + 1..].to_string()))
        .collect();

    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(workspace_root).into_iter().flatten() {
        if paths.len() >= max_items {
            break;
        }
        let path = entry.path();
        if path.is_file() {
            let name = path.to_string_lossy();
            if suffixes.iter().any(|s| name.ends_with(s.as_str())) {
                paths.push(path.to_path_buf());
            }
        }
    }
    paths
}

fn expand_extglob(pattern: &str) -> Vec<String> {
    if let Some(at_pos) = pattern.find("@(")
        && let Some(close) = pattern[at_pos..].find(')')
    {
        let prefix = &pattern[..at_pos];
        let alts = &pattern[at_pos + 2..at_pos + close];
        let suffix = &pattern[at_pos + close + 1..];
        return alts
            .split('|')
            .map(|alt| format!("{prefix}{alt}{suffix}"))
            .collect();
    }
    vec![pattern.to_string()]
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let original = PathBuf::from("/home/user/my file.sh");
        assert_eq!(uri_to_path_opt(&path_to_uri(&original)), Some(original));
    }

    #[test]
    fn non_file_uri_returns_none() {
        assert!(uri_to_path_opt("untitled:foo.sh").is_none());
    }

    #[test]
    fn non_file_uri_fallback() {
        assert_eq!(
            uri_to_path("/already/a/path"),
            PathBuf::from("/already/a/path")
        );
    }

    #[test]
    fn windows_drive_path() {
        assert_eq!(
            uri_to_path("file:///C:/Users/foo/bar.sh"),
            PathBuf::from("C:/Users/foo/bar.sh")
        );
    }
}
