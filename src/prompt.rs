use crate::terminal::execute_line;
use hsab::{Evaluator, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================
// Git prompt info cache (issue #35)
// ============================================
//
// The prompt used to shell out to `git` three times on every render. Instead
// we run a single `git status --porcelain=v2 --branch` and cache the parsed
// result keyed on (cwd, .git/HEAD mtime, .git/index mtime). Non-git
// directories are detected by walking up for a `.git` entry, spawning no
// subprocess at all.
//
// Known limitation (per the issue spec): an unstaged edit to a tracked file
// does not touch `.git/index`, so the dirty flag may lag until HEAD/index
// change (checkout, add, commit) or the cwd changes.

/// Cached git info for the prompt.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GitPromptInfo {
    pub branch: String,
    pub dirty: bool,
    pub repo: String,
}

/// Cache key: invalidates when the cwd or the repo metadata files change.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GitCacheKey {
    cwd: PathBuf,
    head_mtime: Option<SystemTime>,
    index_mtime: Option<SystemTime>,
}

static GIT_CACHE: Mutex<Option<(GitCacheKey, GitPromptInfo)>> = Mutex::new(None);

/// Walk up from `cwd` looking for a `.git` entry. Returns the repo root
/// (directory containing `.git`) and the git dir itself.
/// Cheap: pure filesystem metadata, no subprocess.
pub(crate) fn find_git_dir(cwd: &Path) -> Option<(PathBuf, PathBuf)> {
    for dir in cwd.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some((dir.to_path_buf(), dot_git));
        }
        if dot_git.is_file() {
            // Worktree/submodule: `.git` is a file with "gitdir: <path>"
            if let Ok(contents) = std::fs::read_to_string(&dot_git) {
                if let Some(gitdir) = contents.strip_prefix("gitdir:") {
                    let gitdir = gitdir.trim();
                    let gitdir_path = if Path::new(gitdir).is_absolute() {
                        PathBuf::from(gitdir)
                    } else {
                        dir.join(gitdir)
                    };
                    return Some((dir.to_path_buf(), gitdir_path));
                }
            }
        }
    }
    None
}

/// Build the cache key for a repo. `None` mtimes are fine (missing files);
/// they still participate in equality so appearance/disappearance invalidates.
pub(crate) fn git_cache_key(cwd: &Path, git_dir: &Path) -> GitCacheKey {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    GitCacheKey {
        cwd: cwd.to_path_buf(),
        head_mtime: mtime(&git_dir.join("HEAD")),
        index_mtime: mtime(&git_dir.join("index")),
    }
}

/// Parse `git status --porcelain=v2 --branch` output into (branch, dirty).
pub(crate) fn parse_porcelain_v2(output: &str) -> (String, bool) {
    let mut branch = String::new();
    let mut dirty = false;
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = rest.trim().to_string();
        } else if !line.starts_with('#') && !line.is_empty() {
            dirty = true;
        }
    }
    (branch, dirty)
}

/// Compute git info for the prompt with caching. Returns `None` outside a
/// git repo (and spawns no subprocess in that case).
pub(crate) fn git_prompt_info(cwd: &Path) -> Option<GitPromptInfo> {
    let (repo_root, git_dir) = find_git_dir(cwd)?;
    let key = git_cache_key(cwd, &git_dir);

    {
        let cache = hsab::util::lock_or_recover(&GIT_CACHE);
        if let Some((cached_key, cached_info)) = cache.as_ref() {
            if *cached_key == key {
                return Some(cached_info.clone());
            }
        }
    }

    // Single git invocation for branch + dirty state.
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain=v2", "--branch"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let (branch, dirty) = parse_porcelain_v2(&String::from_utf8_lossy(&output.stdout));

    let repo = repo_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let info = GitPromptInfo {
        branch,
        dirty,
        repo,
    };
    *hsab::util::lock_or_recover(&GIT_CACHE) = Some((key, info.clone()));
    Some(info)
}

/// Set prompt context variables for PS1/PS2/STACK_HINT functions
pub(crate) fn set_prompt_context(eval: &Evaluator, cmd_num: usize) {
    // Version info
    let version_parts: Vec<&str> = VERSION.split('.').collect();
    std::env::set_var("_VERSION", VERSION);
    std::env::set_var("_VERSION_MAJOR", version_parts.first().unwrap_or(&"0"));
    std::env::set_var("_VERSION_MINOR", version_parts.get(1).unwrap_or(&"0"));
    std::env::set_var("_VERSION_PATCH", version_parts.get(2).unwrap_or(&"0"));

    // Shell state
    let depth = eval.stack().iter().filter(|v| v.as_arg().is_some()).count();
    std::env::set_var("_DEPTH", depth.to_string());
    std::env::set_var("_EXIT", eval.last_exit_code().to_string());
    std::env::set_var("_JOBS", eval.job_count().to_string());
    std::env::set_var("_LIMBO", eval.limbo_count().to_string());
    std::env::set_var("_FUTURES", eval.futures_count().to_string());
    std::env::set_var("_CMD_NUM", cmd_num.to_string());
    std::env::set_var(
        "_SHLVL",
        std::env::var("SHLVL").unwrap_or_else(|_| "1".to_string()),
    );

    // Environment
    std::env::set_var("_CWD", eval.cwd().display().to_string());
    std::env::set_var(
        "_USER",
        std::env::var("USER").unwrap_or_else(|_| "".to_string()),
    );
    std::env::set_var(
        "_HOST",
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "".to_string()),
    );

    // Time
    let now = chrono::Local::now();
    std::env::set_var("_TIME", now.format("%H:%M:%S").to_string());
    std::env::set_var("_DATE", now.format("%Y-%m-%d").to_string());

    // Git info (only if in a git repo); cached, single subprocess on miss
    match git_prompt_info(eval.cwd()) {
        Some(info) => {
            std::env::set_var("_GIT_BRANCH", &info.branch);
            std::env::set_var("_GIT_DIRTY", if info.dirty { "1" } else { "0" });
            std::env::set_var("_GIT_REPO", &info.repo);
        }
        None => {
            std::env::set_var("_GIT_BRANCH", "");
            std::env::set_var("_GIT_DIRTY", "0");
            std::env::set_var("_GIT_REPO", "");
        }
    }
}

/// Evaluate a prompt definition (PS1, PS2, STACK_HINT) and return the output string
pub(crate) fn eval_prompt_definition(eval: &mut Evaluator, name: &str) -> Option<String> {
    if !eval.has_definition(name) {
        return None;
    }

    // Save current stack and exit code (predicates in PS1 shouldn't affect user's exit code)
    let saved_stack = eval.stack().to_vec();
    let saved_exit_code = eval.last_exit_code();

    // Clear stack for prompt evaluation
    eval.clear_stack();

    // Execute the definition
    let result = execute_line(eval, name, false);

    // Get the output from stack
    let prompt = if result.is_ok() {
        eval.stack()
            .iter()
            .filter_map(|v| v.as_arg())
            .collect::<Vec<_>>()
            .join("")
    } else {
        // On error, return None to use default
        eval.restore_stack(saved_stack);
        eval.set_last_exit_code(saved_exit_code);
        return None;
    };

    // Restore stack and exit code
    eval.restore_stack(saved_stack);
    eval.set_last_exit_code(saved_exit_code);

    if prompt.is_empty() {
        None
    } else {
        Some(prompt)
    }
}

/// Extract hint format (prefix, separator, suffix) from STACK_HINT definition
/// Calls STACK_HINT with test input "A\nB" (two items) and parses the result
pub(crate) fn extract_hint_format(eval: &mut Evaluator) -> (String, String, String) {
    let default = ("".to_string(), " ".to_string(), "".to_string());

    if !eval.has_definition("STACK_HINT") {
        return default;
    }

    // Save current stack and exit code
    let saved_stack = eval.stack().to_vec();
    let saved_exit_code = eval.last_exit_code();

    // Clear and push test input: two items separated by newline
    eval.clear_stack();
    eval.push_value(Value::Literal("A\nB".to_string()));

    // Execute STACK_HINT
    if execute_line(eval, "STACK_HINT", false).is_ok() {
        if let Some(result) = eval.stack().last().and_then(|v| v.as_arg()) {
            // Parse result to extract prefix, separator, and suffix
            // Expected output like " [A, B]" -> prefix=" [", sep=", ", suffix="]"
            if let Some(pos_a) = result.find('A') {
                if let Some(pos_b) = result.find('B') {
                    // Trim leading newlines from prefix and trailing newlines from suffix
                    // to avoid double newlines in hint display
                    let prefix = result[..pos_a].trim_start_matches('\n').to_string();
                    let separator = result[pos_a + 1..pos_b].to_string();
                    let suffix = result[pos_b + 1..].trim_end_matches('\n').to_string();
                    eval.restore_stack(saved_stack);
                    eval.set_last_exit_code(saved_exit_code);
                    return (prefix, separator, suffix);
                }
            }
        }
    }

    // Restore stack and exit code on failure
    eval.restore_stack(saved_stack);
    eval.set_last_exit_code(saved_exit_code);
    default
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_porcelain_v2_clean() {
        let out = "# branch.oid abc123\n# branch.head main\n# branch.upstream origin/main\n";
        let (branch, dirty) = parse_porcelain_v2(out);
        assert_eq!(branch, "main");
        assert!(!dirty);
    }

    #[test]
    fn test_parse_porcelain_v2_dirty() {
        let out = "# branch.oid abc123\n# branch.head feature/x\n1 .M N... 100644 100644 100644 abc def src/main.rs\n";
        let (branch, dirty) = parse_porcelain_v2(out);
        assert_eq!(branch, "feature/x");
        assert!(dirty);
    }

    #[test]
    fn test_parse_porcelain_v2_untracked_is_dirty() {
        let out = "# branch.head main\n? newfile.txt\n";
        let (_, dirty) = parse_porcelain_v2(out);
        assert!(dirty);
    }

    #[test]
    fn test_find_git_dir_none_outside_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_git_dir(tmp.path()).is_none());
    }

    #[test]
    fn test_find_git_dir_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join(".git")).unwrap();
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        let (repo_root, git_dir) = find_git_dir(&nested).unwrap();
        assert_eq!(repo_root, root);
        assert_eq!(git_dir, root.join(".git"));
    }

    #[test]
    fn test_git_cache_key_invalidates_on_head_change() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let git_dir = root.join(".git");
        std::fs::create_dir(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

        let key1 = git_cache_key(root, &git_dir);
        let key2 = git_cache_key(root, &git_dir);
        assert_eq!(key1, key2, "unchanged repo must produce an equal key");

        // Simulate `git checkout`: HEAD is rewritten with a newer mtime
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/other\n").unwrap();
        let key3 = git_cache_key(root, &git_dir);
        assert_ne!(key1, key3, "HEAD mtime change must invalidate the key");
    }

    #[test]
    fn test_git_cache_key_invalidates_on_index_change() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let git_dir = root.join(".git");
        std::fs::create_dir(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

        let key1 = git_cache_key(root, &git_dir);
        // Simulate `git add`: index appears / changes
        std::fs::write(git_dir.join("index"), "fake index").unwrap();
        let key2 = git_cache_key(root, &git_dir);
        assert_ne!(key1, key2, "index change must invalidate the key");
    }

    #[test]
    fn test_git_cache_key_differs_per_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let git_dir = root.join(".git");
        std::fs::create_dir(&git_dir).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let sub = root.join("sub");
        std::fs::create_dir(&sub).unwrap();

        let key1 = git_cache_key(root, &git_dir);
        let key2 = git_cache_key(&sub, &git_dir);
        assert_ne!(key1, key2, "cache must be per-cwd");
    }

    #[test]
    fn test_git_prompt_info_none_outside_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(git_prompt_info(tmp.path()).is_none());
    }
}
