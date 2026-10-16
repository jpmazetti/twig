use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    CommandFailure, ProcessOutput, git_executable, local_failure, parse_status, resolve_commit,
    strings,
};
use crate::process;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryOperationState {
    pub operation: Option<String>,
    pub conflicts: Vec<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub dirty: bool,
    pub has_commits: bool,
    pub progress: Option<String>,
    pub current_commit: Option<String>,
    pub previous_head: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedPreview {
    pub current_head: Option<String>,
    pub current_subject: Option<String>,
    pub target_hash: Option<String>,
    pub target_subject: Option<String>,
    pub commit_count: usize,
    pub merge_base: Option<String>,
    pub fast_forward_possible: Option<bool>,
    pub published: bool,
    pub affected_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedRequest {
    pub operation: String,
    pub target: String,
    pub mode: Option<String>,
    pub no_ff: bool,
    pub ff_only: bool,
    pub squash: bool,
    pub no_commit: bool,
    pub autostash: bool,
    pub record_origin: bool,
    pub mainline: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationActionRequest {
    pub operation: String,
    pub action: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedOutput {
    pub stdout: String,
    pub stderr: String,
    pub args: Vec<String>,
    pub previous_head: Option<String>,
    pub current_head: Option<String>,
}

pub fn repository_operation_state(
    repository: &str,
) -> Result<RepositoryOperationState, CommandFailure> {
    let git = git_executable("Read repository operation state")?;
    let status_args = strings(&[
        "-C",
        repository,
        "status",
        "--porcelain=v2",
        "--branch",
        "-z",
        "--untracked-files=all",
    ]);
    let output = run_git(&git, "Read repository operation state", &status_args)?;
    let status = parse_status(&output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse repository operation state".to_owned(),
        args: args_to_strings(&status_args),
        exit_code: output.exit_code,
        stderr: message,
        stdout: output.stdout.clone(),
    })?;
    let git_dir_args = strings(&["-C", repository, "rev-parse", "--absolute-git-dir"]);
    let git_dir_output = run_git(&git, "Locate Git directory", &git_dir_args)?;
    let git_dir = PathBuf::from(git_dir_output.stdout.trim());
    let markers = [
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
    ];
    let mut operation = None;
    for (marker, name) in markers {
        if git_dir.join(marker).is_file() {
            operation = Some(name.to_owned());
            break;
        }
    }
    let rebase_dir = if git_dir.join("rebase-merge").is_dir() {
        Some(git_dir.join("rebase-merge"))
    } else if git_dir.join("rebase-apply").is_dir() {
        Some(git_dir.join("rebase-apply"))
    } else {
        None
    };
    if rebase_dir.is_some() {
        operation = Some("rebase".to_owned());
    }
    let conflicts = status
        .files
        .iter()
        .filter(|file| file.status == "conflicted")
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let progress = rebase_dir.as_deref().and_then(read_rebase_progress);
    let current_commit = rebase_dir.as_deref().and_then(|dir| {
        ["stopped-sha", "orig-head"]
            .iter()
            .find_map(|name| fs::read_to_string(dir.join(name)).ok())
            .map(|hash| hash.trim().to_owned())
            .filter(|hash| !hash.is_empty())
    });
    let previous_head = rebase_dir
        .as_deref()
        .and_then(|dir| fs::read_to_string(dir.join("orig-head")).ok())
        .map(|hash| hash.trim().to_owned())
        .filter(|hash| !hash.is_empty());
    let detached = status.branch.is_none();
    let branch = status.branch;
    let head_args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let head = process::run(&git, &head_args, None)
        .map_err(|error| CommandFailure::new("Read repository HEAD", &head_args, error))?;
    let has_commits = match head.exit_code {
        Some(0) => true,
        Some(1) => false,
        _ => {
            return Err(CommandFailure::from_output(
                "Read repository HEAD",
                &head_args,
                head,
            ));
        }
    };
    Ok(RepositoryOperationState {
        operation,
        conflicts,
        branch,
        detached: detached && has_commits,
        dirty: !status.files.is_empty(),
        has_commits,
        progress,
        current_commit,
        previous_head,
    })
}

pub fn advanced_preview(
    repository: &str,
    operation: &str,
    target: &str,
) -> Result<AdvancedPreview, CommandFailure> {
    let state = repository_operation_state(repository)?;
    if let Some(current) = state.operation {
        return Err(local_failure(
            "Preview advanced Git operation",
            &format!("Cannot start another operation while {current} is in progress."),
        ));
    }
    validate_target(operation, target)?;
    let git = git_executable("Preview advanced Git operation")?;
    let head = optional_head(&git, repository)?;
    let current_subject = head
        .as_deref()
        .map(|hash| commit_subject(&git, repository, hash))
        .transpose()?;
    let target_hash = Some(resolve_commit(git.as_path(), repository, target)?);
    let target_commit = target_hash
        .as_deref()
        .map(|hash| commit_subject(&git, repository, hash))
        .transpose()?;
    let mut commit_count = 0;
    let mut merge_base = None;
    let mut fast_forward_possible = None;
    let mut published = false;
    let mut affected_files = Vec::new();

    if let (Some(head_hash), Some(target_hash)) = (head.as_deref(), target_hash.as_deref()) {
        match operation {
            "merge" => {
                commit_count = count_commits(&git, repository, &format!("HEAD..{target}"))?;
                fast_forward_possible =
                    Some(is_ancestor(&git, repository, head_hash, target_hash)?);
            }
            "rebase" => {
                commit_count = count_commits(&git, repository, &format!("{target}..HEAD"))?;
                merge_base = read_merge_base(&git, repository, target_hash, head_hash)?;
                published = published_rewrite(&git, repository, target_hash, head_hash)?;
            }
            "reset" => {
                commit_count = count_commits(&git, repository, &format!("{target}..HEAD"))?;
                published = published_rewrite(&git, repository, target_hash, head_hash)?;
                affected_files = super::load_repository(repository)?
                    .files
                    .into_iter()
                    .filter(|file| file.status != "untracked")
                    .map(|file| file.path)
                    .collect();
            }
            _ => {}
        }
    }

    Ok(AdvancedPreview {
        current_head: head,
        current_subject,
        target_hash,
        target_subject: target_commit,
        commit_count,
        merge_base,
        fast_forward_possible,
        published,
        affected_files,
    })
}

pub fn run_advanced_operation(
    repository: &str,
    request: &AdvancedRequest,
) -> Result<AdvancedOutput, CommandFailure> {
    let state = repository_operation_state(repository)?;
    if state.operation.is_some() {
        return Err(local_failure(
            "Run advanced Git operation",
            "Finish or abort the operation already in progress first.",
        ));
    }
    let git = git_executable("Run advanced Git operation")?;
    validate_target(&request.operation, &request.target)?;
    let previous_head = if request.operation == "rebase" {
        rebase_original_head(&git, repository)?.or(optional_head(&git, repository)?)
    } else {
        optional_head(&git, repository)?
    };
    if previous_head.is_none() {
        return Err(local_failure(
            "Run advanced Git operation",
            "This operation requires a repository with at least one commit.",
        ));
    }
    let mut args = strings(&["-C", repository]);
    match request.operation.as_str() {
        "merge" => {
            if [request.no_ff, request.ff_only, request.squash]
                .iter()
                .filter(|option| **option)
                .count()
                > 1
            {
                return Err(local_failure(
                    "Merge",
                    "Choose at most one of --no-ff, --ff-only, or --squash.",
                ));
            }
            resolve_commit(git.as_path(), repository, &request.target)?;
            args.push(OsString::from("merge"));
            if request.no_ff {
                args.push(OsString::from("--no-ff"));
            }
            if request.ff_only {
                args.push(OsString::from("--ff-only"));
            }
            if request.squash {
                args.push(OsString::from("--squash"));
            }
            if request.no_commit {
                args.push(OsString::from("--no-commit"));
            }
            args.push(OsString::from(&request.target));
        }
        "rebase" => {
            resolve_commit(git.as_path(), repository, &request.target)?;
            args.push(OsString::from("rebase"));
            if request.autostash {
                args.push(OsString::from("--autostash"));
            }
            args.push(OsString::from(&request.target));
        }
        "cherry-pick" => {
            resolve_commit(git.as_path(), repository, &request.target)?;
            let parents = commit_parents(&git, repository, &request.target)?;
            if parents > 1 && request.mainline.is_none() {
                return Err(local_failure(
                    "Cherry-pick",
                    "Cherry-picking a merge commit requires choosing parent 1 or 2.",
                ));
            }
            if request.mainline == Some(2) && parents < 2 {
                return Err(local_failure(
                    "Cherry-pick",
                    "Parent 2 does not exist for this commit.",
                ));
            }
            if request
                .mainline
                .is_some_and(|parent| parent != 1 && parent != 2)
            {
                return Err(local_failure(
                    "Cherry-pick",
                    "Mainline parent must be 1 or 2.",
                ));
            }
            args.push(OsString::from("cherry-pick"));
            if let Some(parent) = request.mainline {
                args.push(OsString::from("-m"));
                args.push(OsString::from(parent.to_string()));
            }
            if request.record_origin {
                args.push(OsString::from("-x"));
            }
            if request.no_commit {
                args.push(OsString::from("--no-commit"));
            }
            args.push(OsString::from(&request.target));
        }
        "reset" => {
            resolve_commit(git.as_path(), repository, &request.target)?;
            let mode = request.mode.as_deref().unwrap_or("mixed");
            if !["soft", "mixed", "hard"].contains(&mode) {
                return Err(local_failure(
                    "Reset",
                    "Reset mode must be soft, mixed, or hard.",
                ));
            }
            args.extend(strings(&["reset", &format!("--{mode}")]));
            args.push(OsString::from(&request.target));
        }
        _ => {
            return Err(local_failure(
                "Run advanced Git operation",
                "Unknown operation.",
            ));
        }
    }
    let output = run_git(&git, &request.operation, &args)?;
    let current_head = optional_head(&git, repository)?;
    Ok(output_result(output, args, previous_head, current_head))
}

pub fn run_operation_action(
    repository: &str,
    request: &OperationActionRequest,
) -> Result<AdvancedOutput, CommandFailure> {
    let state = repository_operation_state(repository)?;
    if state.operation.as_deref() != Some(request.operation.as_str()) {
        return Err(local_failure(
            "Continue repository operation",
            "The requested operation is not currently in progress.",
        ));
    }
    let git = git_executable("Continue repository operation")?;
    let previous_head = if request.operation == "rebase" {
        rebase_original_head(&git, repository)?.or(optional_head(&git, repository)?)
    } else {
        optional_head(&git, repository)?
    };
    let mut args = strings(&["-C", repository]);
    args.push(OsString::from(&request.operation));
    match request.action.as_str() {
        "continue" => {
            if request.operation == "revert" {
                return Err(local_failure(
                    "Continue revert",
                    "Revert is not supported in this milestone.",
                ));
            }
            args.push(OsString::from("--continue"));
        }
        "skip" if matches!(request.operation.as_str(), "rebase" | "cherry-pick") => {
            args.push(OsString::from("--skip"));
        }
        "abort"
            if matches!(
                request.operation.as_str(),
                "merge" | "rebase" | "cherry-pick"
            ) =>
        {
            args.push(OsString::from("--abort"));
        }
        _ => {
            return Err(local_failure(
                "Continue repository operation",
                "Action is not supported.",
            ));
        }
    }
    let env = if request.action == "continue" {
        &[("GIT_EDITOR", "true")][..]
    } else {
        &[][..]
    };
    let output = process::run_with_env(&git, &args, None, env)
        .map_err(|error| CommandFailure::new("Continue repository operation", &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "Continue repository operation",
            &args,
            output,
        ));
    }
    let current_head = optional_head(&git, repository)?;
    Ok(output_result(output, args, previous_head, current_head))
}

fn read_rebase_progress(directory: &std::path::Path) -> Option<String> {
    let (current_file, end_file) = if directory.ends_with("rebase-merge") {
        ("msgnum", "end")
    } else {
        ("next", "last")
    };
    let current = fs::read_to_string(directory.join(current_file)).ok()?;
    let end = fs::read_to_string(directory.join(end_file)).ok()?;
    Some(format!("Applying {} of {}", current.trim(), end.trim()))
}

fn rebase_original_head(
    git: &std::path::Path,
    repository: &str,
) -> Result<Option<String>, CommandFailure> {
    let git_dir_args = strings(&["-C", repository, "rev-parse", "--absolute-git-dir"]);
    let git_dir = run_git(git, "Locate Git directory", &git_dir_args)?;
    let directory = PathBuf::from(git_dir.stdout.trim());
    for name in ["rebase-merge", "rebase-apply"] {
        if let Ok(hash) = fs::read_to_string(directory.join(name).join("orig-head")) {
            let hash = hash.trim().to_owned();
            if !hash.is_empty() {
                return Ok(Some(hash));
            }
        }
    }
    Ok(None)
}

fn validate_target(operation: &str, target: &str) -> Result<(), CommandFailure> {
    if target.trim().is_empty() || target.starts_with('-') {
        return Err(local_failure(
            operation,
            "Target must be non-empty and cannot begin with '-'.",
        ));
    }
    Ok(())
}

fn optional_head(
    git: &std::path::Path,
    repository: &str,
) -> Result<Option<String>, CommandFailure> {
    let args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new("Read repository HEAD", &args, error))?;
    match output.exit_code {
        Some(0) => Ok(Some(output.stdout.trim().to_owned())),
        Some(1) => Ok(None),
        _ => Err(CommandFailure::from_output(
            "Read repository HEAD",
            &args,
            output,
        )),
    }
}

fn count_commits(
    git: &std::path::Path,
    repository: &str,
    revision_range: &str,
) -> Result<usize, CommandFailure> {
    let args = strings(&["-C", repository, "rev-list", "--count", revision_range]);
    let output = run_git(git, "Count commits", &args)?;
    output
        .stdout
        .trim()
        .parse()
        .map_err(|error| CommandFailure {
            ok: false,
            operation: "Parse commit count".to_owned(),
            args: args_to_strings(&args),
            exit_code: output.exit_code,
            stderr: format!("Git returned an invalid commit count: {error}"),
            stdout: output.stdout,
        })
}

fn is_ancestor(
    git: &std::path::Path,
    repository: &str,
    ancestor: &str,
    commit: &str,
) -> Result<bool, CommandFailure> {
    let args = strings(&[
        "-C",
        repository,
        "merge-base",
        "--is-ancestor",
        ancestor,
        commit,
    ]);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new("Check fast-forward", &args, error))?;
    match output.exit_code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(CommandFailure::from_output(
            "Check fast-forward",
            &args,
            output,
        )),
    }
}

fn read_merge_base(
    git: &std::path::Path,
    repository: &str,
    target: &str,
    head: &str,
) -> Result<Option<String>, CommandFailure> {
    let args = strings(&["-C", repository, "merge-base", target, head]);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new("Read merge base", &args, error))?;
    match output.exit_code {
        Some(0) => Ok(Some(output.stdout.trim().to_owned())),
        Some(1) => Ok(None),
        _ => Err(CommandFailure::from_output(
            "Read merge base",
            &args,
            output,
        )),
    }
}

fn published_rewrite(
    git: &std::path::Path,
    repository: &str,
    target: &str,
    head: &str,
) -> Result<bool, CommandFailure> {
    let args = strings(&[
        "-C",
        repository,
        "for-each-ref",
        "--format=%(refname)",
        "refs/remotes",
    ]);
    let output = run_git(git, "Check published commits", &args)?;
    for remote_ref in output.stdout.lines().filter(|line| !line.is_empty()) {
        let range = format!("{target}..{head}");
        let commits = strings(&["-C", repository, "rev-list", &range]);
        let listed = run_git(git, "List rewritten commits", &commits)?;
        for commit in listed.stdout.lines().filter(|line| !line.is_empty()) {
            let contains_args = strings(&[
                "-C",
                repository,
                "merge-base",
                "--is-ancestor",
                commit,
                remote_ref,
            ]);
            let contains = process::run(git, &contains_args, None).map_err(|error| {
                CommandFailure::new("Check published commits", &contains_args, error)
            })?;
            if contains.exit_code == Some(0) {
                return Ok(true);
            }
            if contains.exit_code != Some(1) {
                return Err(CommandFailure::from_output(
                    "Check published commits",
                    &contains_args,
                    contains,
                ));
            }
        }
    }
    Ok(false)
}

fn commit_subject(
    git: &std::path::Path,
    repository: &str,
    hash: &str,
) -> Result<String, CommandFailure> {
    let args = strings(&["-C", repository, "show", "-s", "--format=%s", hash]);
    Ok(run_git(git, "Read target commit", &args)?
        .stdout
        .trim()
        .to_owned())
}

fn commit_parents(
    git: &std::path::Path,
    repository: &str,
    hash: &str,
) -> Result<usize, CommandFailure> {
    let args = strings(&["-C", repository, "rev-list", "--parents", "-n", "1", hash]);
    Ok(run_git(git, "Read commit parents", &args)?
        .stdout
        .split_whitespace()
        .count()
        .saturating_sub(1))
}

fn run_git(
    git: &std::path::Path,
    operation: &str,
    args: &[OsString],
) -> Result<ProcessOutput, CommandFailure> {
    let output = process::run(git, args, None)
        .map_err(|error| CommandFailure::new(operation, args, error))?;
    if output.exit_code == Some(0) {
        Ok(output)
    } else {
        Err(CommandFailure::from_output(operation, args, output))
    }
}

fn output_result(
    output: ProcessOutput,
    args: Vec<OsString>,
    previous_head: Option<String>,
    current_head: Option<String>,
) -> AdvancedOutput {
    AdvancedOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        args: args_to_strings(&args),
        previous_head,
        current_head,
    }
}

fn args_to_strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Output;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(0);

    struct TestRepository {
        path: PathBuf,
    }

    impl TestRepository {
        fn new() -> Self {
            let suffix = NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("git-gui-advanced-{}-{suffix}", std::process::id()));
            fs::create_dir_all(&path).expect("create temporary repository directory");
            let repo = Self { path };
            repo.git(&["init", "-q"]);
            repo.git(&["config", "user.name", "Git GUI Test"]);
            repo.git(&["config", "user.email", "git-gui-test@example.invalid"]);
            repo
        }

        fn path(&self) -> &str {
            self.path.to_str().expect("temporary path is valid UTF-8")
        }

        fn git(&self, args: &[&str]) -> Output {
            let output = crate::process::command("git")
                .arg("-C")
                .arg(&self.path)
                .args(args)
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .expect("run git in test repository");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
            output
        }

        fn commit_file(&self, path: &str, contents: &str, message: &str) -> String {
            fs::write(self.path.join(path), contents).expect("write tracked test file");
            self.git(&["add", "--", path]);
            self.git(&["commit", "-m", message]);
            String::from_utf8(self.git(&["rev-parse", "HEAD"]).stdout)
                .expect("commit hash is UTF-8")
                .trim()
                .to_owned()
        }

        fn branch_main(&self) {
            self.git(&["branch", "-M", "main"]);
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn request(operation: &str, target: &str) -> AdvancedRequest {
        AdvancedRequest {
            operation: operation.to_owned(),
            target: target.to_owned(),
            mode: None,
            no_ff: false,
            ff_only: false,
            squash: false,
            no_commit: false,
            autostash: false,
            record_origin: false,
            mainline: None,
        }
    }

    #[test]
    fn detects_unborn_detached_and_worktree_merge_conflicts() {
        let repo = TestRepository::new();
        let unborn = repository_operation_state(repo.path()).expect("read unborn state");
        assert!(!unborn.has_commits);
        assert!(!unborn.detached);

        repo.commit_file("conflict.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        repo.commit_file("conflict.txt", "feature\n", "feature");
        repo.git(&["switch", "main"]);
        repo.commit_file("conflict.txt", "main\n", "main");

        let worktree = std::env::temp_dir().join(format!(
            "git-gui-advanced-linked-{}-{}",
            std::process::id(),
            NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
        ));
        let worktree_text = worktree.to_str().expect("worktree path is valid UTF-8");
        repo.git(&["worktree", "add", "--detach", worktree_text, "main"]);
        assert!(worktree.join(".git").is_file());
        let conflict = crate::process::command("git")
            .arg("-C")
            .arg(&worktree)
            .args(["merge", "feature"])
            .output()
            .expect("start merge in linked worktree");
        assert!(!conflict.status.success());
        let state =
            repository_operation_state(worktree_text).expect("read worktree operation state");
        assert_eq!(state.operation.as_deref(), Some("merge"));
        assert_eq!(state.conflicts, vec!["conflict.txt"]);
        assert!(crate::git::switch_branch(worktree_text, "main", false).is_err());
        assert!(crate::git::create_commit(worktree_text, "unexpected commit", false).is_err());
        assert!(
            crate::git::pull(
                worktree_text,
                None,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .is_err()
        );
        run_operation_action(
            worktree_text,
            &OperationActionRequest {
                operation: "merge".to_owned(),
                action: "abort".to_owned(),
            },
        )
        .expect("abort merge through shared operation action");
        let conflict = crate::process::command("git")
            .arg("-C")
            .arg(&worktree)
            .args(["merge", "feature"])
            .output()
            .expect("restart test merge");
        assert!(!conflict.status.success());
        fs::write(worktree.join("conflict.txt"), "resolved\n").expect("resolve merge conflict");
        let stage = crate::process::command("git")
            .arg("-C")
            .arg(&worktree)
            .args(["add", "--", "conflict.txt"])
            .output()
            .expect("stage merge resolution");
        assert!(stage.status.success());
        run_operation_action(
            worktree_text,
            &OperationActionRequest {
                operation: "merge".to_owned(),
                action: "continue".to_owned(),
            },
        )
        .expect("continue merge with the existing message");
        assert!(
            repository_operation_state(worktree_text)
                .expect("read completed merge")
                .operation
                .is_none()
        );
        repo.git(&["worktree", "remove", worktree_text]);
        let _ = fs::remove_dir_all(&worktree);

        repo.git(&["checkout", "--detach", "HEAD"]);
        let detached = repository_operation_state(repo.path()).expect("read detached state");
        assert!(detached.detached);
    }

    #[test]
    fn merges_fast_forward_and_forced_merge_commit_with_real_git() {
        let repo = TestRepository::new();
        repo.commit_file("base.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        repo.commit_file("feature.txt", "feature\n", "feature");
        repo.git(&["switch", "main"]);
        let preview = advanced_preview(repo.path(), "merge", "feature").expect("preview merge");
        assert_eq!(preview.fast_forward_possible, Some(true));
        assert_eq!(preview.commit_count, 1);
        run_advanced_operation(repo.path(), &request("merge", "feature"))
            .expect("fast-forward merge");

        repo.git(&["switch", "-c", "feature-two"]);
        repo.commit_file("feature-two.txt", "second\n", "second feature");
        repo.git(&["switch", "main"]);
        let mut no_ff = request("merge", "feature-two");
        no_ff.no_ff = true;
        run_advanced_operation(repo.path(), &no_ff).expect("no-ff merge");
        let head = repo.git(&["rev-list", "--parents", "-n", "1", "HEAD"]);
        assert_eq!(
            String::from_utf8_lossy(&head.stdout)
                .split_whitespace()
                .count(),
            3
        );
    }

    #[test]
    fn merge_squash_and_no_commit_options_are_passed_explicitly() {
        let squash_repo = TestRepository::new();
        squash_repo.commit_file("base.txt", "base\n", "base");
        squash_repo.branch_main();
        squash_repo.git(&["switch", "-c", "feature"]);
        squash_repo.commit_file("feature.txt", "feature\n", "feature");
        squash_repo.git(&["switch", "main"]);
        let mut squash = request("merge", "feature");
        squash.squash = true;
        let squash_output =
            run_advanced_operation(squash_repo.path(), &squash).expect("squash merge");
        assert!(squash_output.args.ends_with(&[
            "merge".to_owned(),
            "--squash".to_owned(),
            "feature".to_owned()
        ]));
        assert!(
            String::from_utf8_lossy(&squash_repo.git(&["diff", "--cached", "--name-only"]).stdout)
                .contains("feature.txt")
        );

        let no_commit_repo = TestRepository::new();
        no_commit_repo.commit_file("base.txt", "base\n", "base");
        no_commit_repo.branch_main();
        no_commit_repo.git(&["switch", "-c", "feature"]);
        no_commit_repo.commit_file("feature.txt", "feature\n", "feature");
        no_commit_repo.git(&["switch", "main"]);
        let mut no_commit = request("merge", "feature");
        no_commit.no_ff = true;
        no_commit.no_commit = true;
        let no_commit_output =
            run_advanced_operation(no_commit_repo.path(), &no_commit).expect("no-commit merge");
        assert!(no_commit_output.args.ends_with(&[
            "merge".to_owned(),
            "--no-ff".to_owned(),
            "--no-commit".to_owned(),
            "feature".to_owned(),
        ]));
        run_operation_action(
            no_commit_repo.path(),
            &OperationActionRequest {
                operation: "merge".to_owned(),
                action: "abort".to_owned(),
            },
        )
        .expect("abort no-commit merge");
    }

    #[test]
    fn reset_modes_report_previous_head_and_keep_untracked_files_on_hard_reset() {
        let repo = TestRepository::new();
        let initial = repo.commit_file("tracked.txt", "base\n", "base");
        repo.commit_file("later.txt", "later\n", "later");
        let head_before = String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_owned();
        fs::write(repo.path.join("tracked.txt"), "local edit\n").expect("edit tracked file");
        fs::write(repo.path.join("untracked.txt"), "untracked\n").expect("create untracked file");
        let mut reset = request("reset", &initial);
        reset.mode = Some("hard".to_owned());
        let output = run_advanced_operation(repo.path(), &reset).expect("hard reset");
        assert_eq!(output.previous_head.as_deref(), Some(head_before.as_str()));
        assert_eq!(output.current_head.as_deref(), Some(initial.as_str()));
        assert!(repo.path.join("untracked.txt").exists());
        assert_eq!(
            fs::read_to_string(repo.path.join("tracked.txt"))
                .expect("read reset file")
                .replace("\r\n", "\n"),
            "base\n"
        );

        let mut invalid = request("reset", "-danger");
        invalid.mode = Some("hard".to_owned());
        assert!(run_advanced_operation(repo.path(), &invalid).is_err());
    }

    #[test]
    fn rebases_conflicts_and_continues_with_the_original_head_recorded() {
        let repo = TestRepository::new();
        repo.commit_file("shared.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        let original_head = repo.commit_file("shared.txt", "feature\n", "feature change");
        repo.git(&["switch", "main"]);
        repo.commit_file("shared.txt", "main\n", "main change");
        repo.git(&["switch", "feature"]);

        let result = run_advanced_operation(repo.path(), &request("rebase", "main"));
        assert!(result.is_err());
        let state = repository_operation_state(repo.path()).expect("read conflicted rebase state");
        assert_eq!(state.operation.as_deref(), Some("rebase"));
        assert_eq!(state.conflicts, vec!["shared.txt"]);
        assert_eq!(state.previous_head.as_deref(), Some(original_head.as_str()));
        assert!(
            run_operation_action(
                repo.path(),
                &OperationActionRequest {
                    operation: "rebase".to_owned(),
                    action: "continue".to_owned()
                },
            )
            .is_err()
        );

        fs::write(repo.path.join("shared.txt"), "resolved\n").expect("resolve rebase conflict");
        repo.git(&["add", "--", "shared.txt"]);
        let continued = run_operation_action(
            repo.path(),
            &OperationActionRequest {
                operation: "rebase".to_owned(),
                action: "continue".to_owned(),
            },
        )
        .expect("continue rebase with existing message");
        assert_eq!(
            continued.previous_head.as_deref(),
            Some(original_head.as_str())
        );
        assert!(
            repository_operation_state(repo.path())
                .expect("read completed rebase")
                .operation
                .is_none()
        );
    }

    #[test]
    fn rebases_cleanly_and_replays_commits_onto_the_selected_branch() {
        let repo = TestRepository::new();
        let base = repo.commit_file("base.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        let original_feature_head = repo.commit_file("feature.txt", "feature\n", "feature");
        repo.git(&["switch", "main"]);
        let main_head = repo.commit_file("main.txt", "main\n", "main");
        repo.git(&["switch", "feature"]);

        let result =
            run_advanced_operation(repo.path(), &request("rebase", "main")).expect("clean rebase");
        assert_eq!(
            result.previous_head.as_deref(),
            Some(original_feature_head.as_str())
        );
        assert_ne!(
            String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD"]).stdout).trim(),
            original_feature_head
        );
        assert!(
            repo.git(&["merge-base", "--is-ancestor", &main_head, "HEAD"])
                .status
                .success()
        );
        assert_eq!(
            String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD~1"]).stdout).trim(),
            main_head
        );
        assert!(
            repository_operation_state(repo.path())
                .expect("read completed clean rebase")
                .operation
                .is_none()
        );
        assert!(repo.path.join("base.txt").exists());
        assert_ne!(base, main_head);
    }

    #[test]
    fn detects_rebase_apply_backend_state() {
        let repo = TestRepository::new();
        repo.commit_file("shared.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        repo.commit_file("shared.txt", "feature\n", "feature");
        repo.git(&["switch", "main"]);
        repo.commit_file("shared.txt", "main\n", "main");
        repo.git(&["switch", "feature"]);

        let result = crate::process::command("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["rebase", "--apply", "main"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("start apply-backend rebase");
        assert!(!result.status.success());
        let state = repository_operation_state(repo.path()).expect("read apply-backend rebase");
        assert_eq!(state.operation.as_deref(), Some("rebase"));
        assert_eq!(state.conflicts, vec!["shared.txt"]);
        assert!(state.progress.is_some());
        run_operation_action(
            repo.path(),
            &OperationActionRequest {
                operation: "rebase".to_owned(),
                action: "abort".to_owned(),
            },
        )
        .expect("abort apply-backend rebase");
    }

    #[test]
    fn rebases_can_be_aborted_or_skip_the_conflicting_commit() {
        let repo = TestRepository::new();
        repo.commit_file("shared.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        let original_head = repo.commit_file("shared.txt", "feature\n", "feature change");
        repo.git(&["switch", "main"]);
        repo.commit_file("shared.txt", "main\n", "main change");
        repo.git(&["switch", "feature"]);

        assert!(run_advanced_operation(repo.path(), &request("rebase", "main")).is_err());
        run_operation_action(
            repo.path(),
            &OperationActionRequest {
                operation: "rebase".to_owned(),
                action: "abort".to_owned(),
            },
        )
        .expect("abort rebase");
        assert_eq!(
            String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD"]).stdout).trim(),
            original_head
        );
        assert!(
            repository_operation_state(repo.path())
                .expect("read aborted rebase")
                .operation
                .is_none()
        );

        assert!(run_advanced_operation(repo.path(), &request("rebase", "main")).is_err());
        run_operation_action(
            repo.path(),
            &OperationActionRequest {
                operation: "rebase".to_owned(),
                action: "skip".to_owned(),
            },
        )
        .expect("skip conflicted rebase commit");
        assert!(
            repository_operation_state(repo.path())
                .expect("read skipped rebase")
                .operation
                .is_none()
        );
        assert_ne!(
            String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD"]).stdout).trim(),
            original_head
        );
    }

    #[test]
    fn refuses_cherry_picking_a_merge_commit_without_mainline_parent() {
        let repo = TestRepository::new();
        let parent = repo.commit_file("base.txt", "base\n", "base");
        repo.branch_main();
        repo.git(&["switch", "-c", "feature"]);
        repo.commit_file("feature.txt", "feature\n", "feature");
        repo.git(&["switch", "main"]);
        repo.commit_file("main.txt", "main\n", "main");
        repo.git(&["merge", "--no-ff", "-m", "merge feature", "feature"]);
        let merge = String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_owned();
        let missing_parent = run_advanced_operation(repo.path(), &request("cherry-pick", &merge));
        assert!(missing_parent.is_err());

        repo.git(&["switch", "-c", "target", &parent]);
        let mut cherry = request("cherry-pick", &merge);
        cherry.mainline = Some(1);
        run_advanced_operation(repo.path(), &cherry).expect("cherry-pick merge with parent 1");
        assert!(repo.path.join("feature.txt").exists());
    }

    #[test]
    fn cherry_picks_plain_commits_and_continues_conflicts() {
        let clean = TestRepository::new();
        let base = clean.commit_file("base.txt", "base\n", "base");
        clean.branch_main();
        clean.git(&["switch", "-c", "source"]);
        let source = clean.commit_file("plain.txt", "plain\n", "plain commit");
        clean.git(&["switch", "-c", "target", &base]);
        run_advanced_operation(clean.path(), &request("cherry-pick", &source))
            .expect("cherry-pick plain commit");
        assert!(clean.path.join("plain.txt").exists());

        let conflict_repo = TestRepository::new();
        conflict_repo.commit_file("shared.txt", "base\n", "base");
        conflict_repo.branch_main();
        conflict_repo.git(&["switch", "-c", "source"]);
        let conflicting_commit = conflict_repo.commit_file("shared.txt", "source\n", "source");
        conflict_repo.git(&["switch", "main"]);
        conflict_repo.commit_file("shared.txt", "target\n", "target");
        assert!(
            run_advanced_operation(
                conflict_repo.path(),
                &request("cherry-pick", &conflicting_commit),
            )
            .is_err()
        );
        let state =
            repository_operation_state(conflict_repo.path()).expect("read cherry-pick state");
        assert_eq!(state.operation.as_deref(), Some("cherry-pick"));
        assert_eq!(state.conflicts, vec!["shared.txt"]);
        fs::write(conflict_repo.path.join("shared.txt"), "resolved\n")
            .expect("resolve cherry-pick");
        conflict_repo.git(&["add", "--", "shared.txt"]);
        run_operation_action(
            conflict_repo.path(),
            &OperationActionRequest {
                operation: "cherry-pick".to_owned(),
                action: "continue".to_owned(),
            },
        )
        .expect("continue cherry-pick");
        assert!(
            repository_operation_state(conflict_repo.path())
                .expect("read completed cherry-pick")
                .operation
                .is_none()
        );
    }

    #[test]
    fn reset_soft_and_mixed_keep_working_tree_changes_with_different_indexes() {
        for mode in ["soft", "mixed"] {
            let repo = TestRepository::new();
            let base = repo.commit_file("tracked.txt", "base\n", "base");
            repo.commit_file("added.txt", "later\n", "later");
            fs::write(repo.path.join("tracked.txt"), "local edit\n").expect("edit tracked file");
            let mut reset = request("reset", &base);
            reset.mode = Some(mode.to_owned());
            run_advanced_operation(repo.path(), &reset).expect("reset with selected mode");
            assert_eq!(
                String::from_utf8_lossy(&repo.git(&["rev-parse", "HEAD"]).stdout).trim(),
                base
            );
            let staged_output = repo.git(&["diff", "--cached", "--name-only"]);
            let staged = String::from_utf8_lossy(&staged_output.stdout);
            if mode == "soft" {
                assert!(staged.contains("added.txt"));
            } else {
                assert!(staged.trim().is_empty());
                let status =
                    repository_operation_state(repo.path()).expect("read mixed reset state");
                assert!(status.dirty);
                assert!(status.conflicts.is_empty());
            }
            assert!(repo.path.join("added.txt").exists());
            assert_eq!(
                fs::read_to_string(repo.path.join("tracked.txt"))
                    .expect("read preserved working tree")
                    .replace("\r\n", "\n"),
                "local edit\n"
            );
        }
    }

    #[test]
    fn reset_and_rebase_preview_warn_when_rewritten_commits_are_published() {
        let repo = TestRepository::new();
        let base = repo.commit_file("base.txt", "base\n", "base");
        repo.branch_main();
        repo.commit_file("published.txt", "published\n", "published commit");
        let bare = repo.path.with_file_name(format!(
            "git-gui-advanced-remote-{}-{}",
            std::process::id(),
            NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&bare).expect("create temporary bare repository directory");
        let bare_text = bare.to_str().expect("bare path is valid UTF-8");
        let remote_init = crate::process::command("git")
            .args(["init", "--bare", "-q", bare_text])
            .output()
            .expect("initialize local bare remote");
        assert!(remote_init.status.success());
        repo.git(&["remote", "add", "origin", bare_text]);
        repo.git(&["push", "-u", "origin", "main"]);
        repo.commit_file("local.txt", "local\n", "local unpublished commit");

        assert!(
            advanced_preview(repo.path(), "rebase", &base)
                .expect("preview rebase")
                .published
        );
        assert!(
            advanced_preview(repo.path(), "reset", &base)
                .expect("preview reset")
                .published
        );
        let _ = fs::remove_dir_all(bare);
    }

    #[test]
    fn detects_revert_state_without_exposing_revert_controls() {
        let repo = TestRepository::new();
        repo.commit_file("shared.txt", "base\n", "base");
        let reverted = repo.commit_file("shared.txt", "change\n", "change to revert");
        repo.commit_file("shared.txt", "later\n", "later conflicting change");
        let output = repo.git_fails_for_revert(&reverted);
        assert!(!output.status.success());
        let state = repository_operation_state(repo.path()).expect("read revert operation state");
        assert_eq!(state.operation.as_deref(), Some("revert"));
        assert_eq!(state.conflicts, vec!["shared.txt"]);

        let abort = crate::process::command("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["revert", "--abort"])
            .output()
            .expect("abort fixture revert");
        assert!(abort.status.success());
        assert!(
            repository_operation_state(repo.path())
                .expect("read aborted revert")
                .operation
                .is_none()
        );
    }

    #[test]
    fn parses_rebase_progress_files() {
        let root = std::env::temp_dir().join(format!(
            "git-gui-rebase-progress-{}-{}",
            std::process::id(),
            NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
        ));
        let directory = root.join("rebase-merge");
        fs::create_dir_all(&directory).expect("create progress fixture");
        fs::write(directory.join("msgnum"), "3\n").expect("write current step");
        fs::write(directory.join("end"), "7\n").expect("write step count");
        assert_eq!(
            read_rebase_progress(&directory),
            Some("Applying 3 of 7".to_owned())
        );
        fs::remove_dir_all(&root).expect("remove merge-backend progress fixture");
        let apply_directory = root.join("rebase-apply");
        fs::create_dir_all(&apply_directory).expect("create apply-backend progress fixture");
        fs::write(apply_directory.join("next"), "2\n").expect("write apply current step");
        fs::write(apply_directory.join("last"), "5\n").expect("write apply step count");
        assert_eq!(
            read_rebase_progress(&apply_directory),
            Some("Applying 2 of 5".to_owned())
        );
        fs::remove_dir_all(root).expect("remove progress fixture");
    }

    impl TestRepository {
        fn git_fails_for_revert(&self, commit: &str) -> Output {
            crate::process::command("git")
                .arg("-C")
                .arg(&self.path)
                .args(["revert", "--no-edit", commit])
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .expect("start fixture revert")
        }
    }
}
