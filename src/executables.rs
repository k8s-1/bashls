use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::util::fs::untildify;

pub struct Executables {
    executables: HashSet<String>,
}

impl Executables {
    #[must_use]
    pub fn from_path(path_var: &str) -> Self {
        let mut executables = HashSet::new();
        for dir in path_var.split(':') {
            let dir = untildify(dir);
            let dir = Path::new(&dir);
            if dir.is_dir()
                && let Ok(entries) = fs::read_dir(dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && is_executable(&path)
                        && let Some(name) = path.file_name()
                    {
                        executables.insert(name.to_string_lossy().into_owned());
                    }
                }
            }
        }
        Executables { executables }
    }

    #[must_use]
    pub fn list(&self) -> Vec<&str> {
        self.executables
            .iter()
            .map(std::string::String::as_str)
            .collect()
    }

    #[must_use]
    pub fn is_on_path(&self, name: &str) -> bool {
        self.executables.contains(name)
    }
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_dir(prefix: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("{}_{}", prefix, n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_executable(dir: &std::path::Path, name: &str) {
        let path = dir.join(name);
        fs::write(&path, "#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }

    fn make_non_executable(dir: &std::path::Path, name: &str) {
        let path = dir.join(name);
        fs::write(&path, "data\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&path, perms).unwrap();
    }

    #[test]
    fn finds_executables_on_path() {
        let dir = unique_dir("bls_exec_find");
        make_executable(&dir, "my-tool");
        let execs = Executables::from_path(&dir.to_string_lossy());
        assert!(execs.is_on_path("my-tool"), "should find my-tool");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ignores_non_executable_files() {
        let dir = unique_dir("bls_exec_noexec");
        make_non_executable(&dir, "readme.txt");
        let execs = Executables::from_path(&dir.to_string_lossy());
        assert!(
            !execs.is_on_path("readme.txt"),
            "non-executable should not be found"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_includes_direct_path_entries() {
        let dir = unique_dir("bls_exec_direct");
        let subdir = dir.join("sub");
        fs::create_dir_all(&subdir).unwrap();
        make_executable(&subdir, "nested-tool");
        let execs = Executables::from_path(&dir.to_string_lossy());
        assert!(
            !execs.is_on_path("nested-tool"),
            "nested executables should not be found"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn is_on_path_with_custom_path() {
        let dir = unique_dir("bls_exec_custom");
        make_executable(&dir, "iam-executable");
        let execs = Executables::from_path(&dir.to_string_lossy());
        assert!(execs.is_on_path("iam-executable"));
        assert!(!execs.is_on_path("not-on-path"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_returns_all_executables() {
        let dir = unique_dir("bls_exec_list");
        make_executable(&dir, "tool-a");
        make_executable(&dir, "tool-b");
        make_non_executable(&dir, "not-exec");
        let execs = Executables::from_path(&dir.to_string_lossy());
        let list = execs.list();
        assert!(list.contains(&"tool-a"), "list should contain tool-a");
        assert!(list.contains(&"tool-b"), "list should contain tool-b");
        assert!(
            !list.contains(&"not-exec"),
            "list should not contain not-exec"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_path_returns_no_executables() {
        let execs = Executables::from_path("");
        assert!(execs.list().is_empty());
    }

    #[test]
    fn multiple_path_dirs_combined() {
        let dir1 = unique_dir("bls_exec_multi1");
        let dir2 = unique_dir("bls_exec_multi2");
        make_executable(&dir1, "exec-from-dir1");
        make_executable(&dir2, "exec-from-dir2");
        let path_var = format!("{}:{}", dir1.display(), dir2.display());
        let execs = Executables::from_path(&path_var);
        assert!(execs.is_on_path("exec-from-dir1"));
        assert!(execs.is_on_path("exec-from-dir2"));
        fs::remove_dir_all(&dir1).ok();
        fs::remove_dir_all(&dir2).ok();
    }
}
