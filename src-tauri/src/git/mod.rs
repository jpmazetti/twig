use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::process::{self, ProcessError, ProcessOutput};

mod advanced;
pub use advanced::{
    AdvancedOutput, AdvancedPreview, AdvancedRequest, OperationActionRequest,
    RepositoryOperationState, advanced_preview, repository_operation_state, run_advanced_operation,
    run_operation_action,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInstallation {
    pub version: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub original_path: Option<String>,
    pub code: String,
    pub status: String,
    pub index_status: String,
    pub worktree_status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: String,
    pub content: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPreview {
    pub binary: bool,
    pub too_large: bool,
    pub metadata: Vec<String>,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryStatus {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub head_oid: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub files: Vec<ChangedFile>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub upstream_gone: bool,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub short_hash: String,
    pub commit_date: String,
    pub subject: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchList {
    pub local: Vec<BranchInfo>,
    pub remote: Vec<BranchInfo>,
    pub detached_head: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitContext {
    pub last_message: Option<String>,
    pub merge_message: Option<String>,
    pub merge_in_progress: bool,
    pub other_operation: Option<String>,
    pub published: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub short_hash: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRef {
    pub name: String,
    pub kind: String,
    pub head: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCommit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub author_email: String,
    pub author_date: String,
    pub committer: String,
    pub committer_email: String,
    pub commit_date: String,
    pub subject: String,
    pub body: String,
    pub refs: Vec<HistoryRef>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub commits: Vec<HistoryCommit>,
    pub has_more: bool,
    pub shallow: bool,
    pub detached: bool,
    pub head_hash: Option<String>,
    pub upstream: Option<String>,
    pub refs_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFingerprint {
    pub head_hash: Option<String>,
    pub refs_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryOptions {
    pub revision: Option<String>,
    pub all_refs: bool,
    pub search: Option<String>,
    pub search_by: String,
    pub path: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFile {
    pub path: String,
    pub original_path: Option<String>,
    pub status: String,
    pub additions: Option<usize>,
    pub deletions: Option<usize>,
    pub binary: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDetail {
    pub commit: HistoryCommit,
    pub files: Vec<HistoryFile>,
    pub shortstat: String,
    pub comparison_parent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushOutput {
    pub stdout: String,
    pub stderr: String,
    pub args: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Debug, PartialEq, Eq)]
enum GitFailureKind {
    MissingIdentity,
    NothingToCommit,
    HookRejected,
    NonFastForward,
    NoUpstream,
    InteractiveInput,
    Other,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum BranchFailureKind {
    AlreadyExists,
    NotFullyMerged,
    LocalChangesWouldBeOverwritten,
    CurrentBranch,
    Worktree,
    InvalidName,
    Other,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandFailure {
    pub ok: bool,
    pub operation: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub stderr: String,
    pub stdout: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CommandResult<T: Serialize> {
    Success { ok: bool, data: T },
    Failure(CommandFailure),
}

impl<T: Serialize> CommandResult<T> {
    pub fn success(data: T) -> Self {
        Self::Success { ok: true, data }
    }

    pub fn failure(failure: CommandFailure) -> Self {
        Self::Failure(failure)
    }
}

impl CommandFailure {
    pub fn new(operation: &str, args: &[OsString], error: ProcessError) -> Self {
        Self {
            ok: false,
            operation: operation.to_owned(),
            args: args
                .iter()
                .map(|arg| mask_credentials(&arg.to_string_lossy()))
                .collect(),
            exit_code: error.exit_code,
            stderr: mask_credentials(&error.stderr),
            stdout: mask_credentials(&error.stdout),
        }
    }

    fn from_output(operation: &str, args: &[OsString], output: ProcessOutput) -> Self {
        Self {
            ok: false,
            operation: operation.to_owned(),
            args: args
                .iter()
                .map(|arg| mask_credentials(&arg.to_string_lossy()))
                .collect(),
            exit_code: output.exit_code,
            stderr: mask_credentials(&output.stderr),
            stdout: mask_credentials(&output.stdout),
        }
    }
}

pub fn detect_git() -> Result<GitInstallation, CommandFailure> {
    let path = find_git_executable().ok_or_else(|| CommandFailure {
        ok: false,
        operation: "Detect Git".to_owned(),
        args: vec!["git.exe".to_owned(), "--version".to_owned()],
        exit_code: None,
        stderr: "Git was not found in PATH or the common Git for Windows install locations. Install Git for Windows and make sure git.exe is available in PATH.".to_owned(),
        stdout: String::new(),
    })?;
    let args = strings(&["--version"]);
    let output = process::run(&path, &args, None)
        .map_err(|error| CommandFailure::new("Detect Git", &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output("Detect Git", &args, output));
    }

    Ok(GitInstallation {
        version: output.stdout.trim().to_owned(),
        path: path.to_string_lossy().into_owned(),
    })
}

pub fn load_repository(path: &str) -> Result<RepositoryStatus, CommandFailure> {
    let git = find_git_executable().ok_or_else(|| CommandFailure {
        ok: false,
        operation: "Load repository".to_owned(),
        args: vec!["git.exe".to_owned()],
        exit_code: None,
        stderr: "Git was not found in PATH or the common Git for Windows install locations."
            .to_owned(),
        stdout: String::new(),
    })?;

    let validation_args = strings(&["-C", path, "rev-parse", "--is-inside-work-tree"]);
    let validation = process::run(&git, &validation_args, None)
        .map_err(|error| CommandFailure::new("Validate repository", &validation_args, error))?;
    if validation.exit_code != Some(0) || validation.stdout.trim() != "true" {
        return Err(CommandFailure::from_output(
            "Validate repository",
            &validation_args,
            validation,
        ));
    }

    let status_args = strings(&[
        "-C",
        path,
        "status",
        "--porcelain=v2",
        "--branch",
        "-z",
        "--untracked-files=all",
    ]);
    let output = process::run(&git, &status_args, None)
        .map_err(|error| CommandFailure::new("Read repository status", &status_args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "Read repository status",
            &status_args,
            output,
        ));
    }
    parse_status(&output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse repository status".to_owned(),
        args: status_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: output.exit_code,
        stderr: message,
        stdout: output.stdout,
    })
}

pub fn list_branches(repository: &str) -> Result<BranchList, CommandFailure> {
    let git = git_executable("List branches")?;
    let format = "%(refname)%00%(refname:short)%00%(HEAD)%00%(upstream:short)%00%(upstream:track)%00%(objectname:short)%00%(committerdate:iso-strict)%00%(contents:subject)";
    let args = strings(&[
        "-C",
        repository,
        "for-each-ref",
        "--sort=-committerdate",
        &format!("--format={format}"),
        "refs/heads",
        "refs/remotes",
    ]);
    let output = process::run(&git, &args, None)
        .map_err(|error| CommandFailure::new("List branches", &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output("List branches", &args, output));
    }
    let (local, remote) = parse_branches(&output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse branches".to_owned(),
        args: args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: output.exit_code,
        stderr: message,
        stdout: output.stdout,
    })?;
    let detached_head = read_detached_head(&git, repository)?;
    Ok(BranchList {
        local,
        remote,
        detached_head,
    })
}

fn read_detached_head(git: &Path, repository: &str) -> Result<Option<String>, CommandFailure> {
    let symbolic_args = strings(&[
        "-C",
        repository,
        "symbolic-ref",
        "--quiet",
        "--short",
        "HEAD",
    ]);
    let symbolic = process::run(git, &symbolic_args, None)
        .map_err(|error| CommandFailure::new("Read current branch", &symbolic_args, error))?;
    match symbolic.exit_code {
        Some(0) => return Ok(None),
        Some(1) => {}
        _ => {
            return Err(CommandFailure::from_output(
                "Read current branch",
                &symbolic_args,
                symbolic,
            ));
        }
    }
    let head_args = strings(&[
        "-C",
        repository,
        "rev-parse",
        "--verify",
        "--quiet",
        "--short",
        "HEAD",
    ]);
    let head = process::run(git, &head_args, None)
        .map_err(|error| CommandFailure::new("Read detached HEAD", &head_args, error))?;
    match head.exit_code {
        Some(0) => Ok(Some(head.stdout.trim().to_owned())),
        Some(1) => Ok(None),
        _ => Err(CommandFailure::from_output(
            "Read detached HEAD",
            &head_args,
            head,
        )),
    }
}

pub fn create_branch(
    repository: &str,
    name: &str,
    start_point: Option<&str>,
    switch_to: bool,
) -> Result<(), CommandFailure> {
    validate_branch_name(repository, name)?;
    let git = git_executable("Create branch")?;
    let start_point = start_point.filter(|point| !point.is_empty() && *point != "HEAD");
    if let Some(point) = start_point {
        resolve_commit(git.as_path(), repository, point)?;
    }
    let mut args = strings(&["-C", repository]);
    if switch_to {
        args.extend(strings(&["switch", "-c", name]));
    } else {
        args.extend(strings(&["branch", name]));
    }
    if let Some(start) = start_point {
        args.push(OsString::from(start));
    }
    run_branch_operation(&git, "Create branch", args)
}

pub fn switch_branch(
    repository: &str,
    name: &str,
    track_remote: bool,
) -> Result<(), CommandFailure> {
    ensure_no_operation(repository, "Switch branch")?;
    validate_branch_name(repository, name)?;
    let git = git_executable("Switch branch")?;
    let mut args = strings(&["-C", repository, "switch"]);
    if track_remote {
        args.push(OsString::from("--track"));
    }
    args.push(OsString::from(name));
    run_branch_operation(&git, "Switch branch", args)
}

pub fn rename_branch(
    repository: &str,
    old_name: &str,
    new_name: &str,
) -> Result<(), CommandFailure> {
    validate_branch_name(repository, old_name)?;
    validate_branch_name(repository, new_name)?;
    let git = git_executable("Rename branch")?;
    let args = strings(&["-C", repository, "branch", "-m", old_name, new_name]);
    run_branch_operation(&git, "Rename branch", args)
}

pub fn delete_branch(repository: &str, name: &str, force: bool) -> Result<(), CommandFailure> {
    let status = load_repository(repository)?;
    if status.branch.as_deref() == Some(name) {
        let args = strings(&[
            "-C",
            repository,
            "branch",
            if force { "-D" } else { "-d" },
            name,
        ]);
        return Err(CommandFailure {
            ok: false,
            operation: "Delete branch".to_owned(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            exit_code: None,
            stderr: "Cannot delete the current branch. Switch to another branch first.".to_owned(),
            stdout: String::new(),
        });
    }
    validate_branch_name(repository, name)?;
    let git = git_executable("Delete branch")?;
    let args = strings(&[
        "-C",
        repository,
        "branch",
        if force { "-D" } else { "-d" },
        name,
    ]);
    run_branch_operation(&git, "Delete branch", args)
}

fn validate_branch_name(repository: &str, name: &str) -> Result<(), CommandFailure> {
    let git = git_executable("Validate branch name")?;
    let args = strings(&["-C", repository, "check-ref-format", "--branch", name]);
    let output = process::run(&git, &args, None)
        .map_err(|error| CommandFailure::new("Validate branch name", &args, error))?;
    if output.exit_code == Some(0) {
        Ok(())
    } else {
        Err(CommandFailure::from_output(
            "Validate branch name",
            &args,
            output,
        ))
    }
}

fn resolve_commit(git: &Path, repository: &str, point: &str) -> Result<String, CommandFailure> {
    if point.starts_with('-') {
        return Err(local_failure(
            "Resolve branch start point",
            "Start points cannot begin with '-'.",
        ));
    }
    let expression = format!("{point}^{{commit}}");
    let args = strings(&[
        "-C",
        repository,
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &expression,
    ]);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new("Resolve branch start point", &args, error))?;
    if output.exit_code == Some(0) {
        Ok(output.stdout.trim().to_owned())
    } else {
        Err(CommandFailure::from_output(
            "Resolve branch start point",
            &args,
            output,
        ))
    }
}

fn run_branch_operation(
    git: &Path,
    operation: &str,
    args: Vec<OsString>,
) -> Result<(), CommandFailure> {
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new(operation, &args, error))?;
    if output.exit_code == Some(0) {
        Ok(())
    } else {
        Err(CommandFailure::from_output(operation, &args, output))
    }
}

pub fn load_commit_context(repository: &str) -> Result<CommitContext, CommandFailure> {
    let git = git_executable("Load commit context")?;
    let head_args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let head_output = process::run(&git, &head_args, None)
        .map_err(|error| CommandFailure::new("Read repository HEAD", &head_args, error))?;
    let has_head = head_output.exit_code == Some(0);
    let last_message = if has_head {
        Some(git_output(&git, repository, &["log", "-1", "--format=%B"])?.stdout)
    } else {
        None
    };
    let merge_path = git_path(&git, repository, "MERGE_HEAD")?;
    let merge_in_progress = merge_path.is_file();
    let merge_message = if merge_in_progress {
        let message_path = git_path(&git, repository, "MERGE_MSG")?;
        Some(
            fs::read_to_string(&message_path).map_err(|error| CommandFailure {
                ok: false,
                operation: "Read merge commit message".to_owned(),
                args: vec![
                    "git".to_owned(),
                    "-C".to_owned(),
                    repository.to_owned(),
                    "rev-parse".to_owned(),
                    "--git-path".to_owned(),
                    "MERGE_MSG".to_owned(),
                ],
                exit_code: None,
                stderr: error.to_string(),
                stdout: String::new(),
            })?,
        )
    } else {
        None
    };
    let mut other_operation = None;
    for (marker, operation) in [
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
    ] {
        if git_path(&git, repository, marker)?.exists() {
            other_operation = Some(operation.to_owned());
            break;
        }
    }
    let published = if has_head {
        let output = git_output(&git, repository, &["branch", "-r", "--contains", "HEAD"])?;
        is_published(&output.stdout)
    } else {
        false
    };
    Ok(CommitContext {
        last_message,
        merge_message,
        merge_in_progress,
        other_operation,
        published,
    })
}

pub fn list_remotes(repository: &str) -> Result<Vec<String>, CommandFailure> {
    let git = git_executable("List remotes")?;
    let output = git_output(&git, repository, &["remote"])?;
    Ok(output.stdout.lines().map(str::to_owned).collect())
}

pub fn list_remote_details(repository: &str) -> Result<Vec<RemoteInfo>, CommandFailure> {
    let git = git_executable("List remotes")?;
    let output = git_output(&git, repository, &["remote", "-v"])?;
    let mut remotes: Vec<RemoteInfo> = Vec::new();
    for line in output.stdout.lines() {
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let Some(url) = fields.next() else { continue };
        let Some(kind) = fields.next() else { continue };
        let remote = if let Some(existing) = remotes.iter_mut().find(|remote| remote.name == name) {
            existing
        } else {
            remotes.push(RemoteInfo {
                name: name.to_owned(),
                fetch_url: String::new(),
                push_url: String::new(),
            });
            remotes
                .last_mut()
                .ok_or_else(|| local_failure("List remotes", "Could not parse remote list."))?
        };
        match kind {
            "(fetch)" => remote.fetch_url = mask_credentials(url),
            "(push)" => remote.push_url = mask_credentials(url),
            _ => {}
        }
    }
    Ok(remotes)
}

pub fn create_commit(
    repository: &str,
    message: &str,
    amend: bool,
) -> Result<CommitSummary, CommandFailure> {
    ensure_no_operation(repository, if amend { "Amend commit" } else { "Commit" })?;
    if message.trim().is_empty() {
        return Err(local_failure("Commit", "A commit message is required."));
    }
    let git = git_executable("Commit")?;
    let status = load_repository(repository)?;
    let staged = status
        .files
        .iter()
        .any(|file| file.index_status != "." && file.index_status != "?");
    if !staged && !amend {
        return Err(local_failure("Commit", "No staged files."));
    }
    let head_args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let head = process::run(&git, &head_args, None)
        .map_err(|error| CommandFailure::new("Read repository HEAD", &head_args, error))?;
    if amend && head.exit_code != Some(0) {
        return Err(local_failure(
            "Amend commit",
            "There is no commit to amend.",
        ));
    }
    let mut args = vec![OsString::from("-C"), OsString::from(repository)];
    args.extend(strings(&["-c", "i18n.commitEncoding=utf-8", "commit"]));
    if amend {
        args.push(OsString::from("--amend"));
    }
    args.extend(strings(&["-F", "-"]));
    let output = process::run_with_input(
        &git,
        &args,
        None,
        message.as_bytes(),
        Duration::from_secs(30),
    )
    .map_err(|error| {
        CommandFailure::new(if amend { "Amend commit" } else { "Commit" }, &args, error)
    })?;
    if output.exit_code != Some(0) {
        let mut failure = CommandFailure::from_output(
            if amend { "Amend commit" } else { "Commit" },
            &args,
            output,
        );
        if let Err(identity_failure) = add_identity_hint(&git, repository, &mut failure) {
            failure.stderr.push_str(&format!(
                "\n\nCould not inspect Git identity configuration:\nstdout:\n{}\nstderr:\n{}",
                identity_failure.stdout, identity_failure.stderr
            ));
        }
        return Err(failure);
    }
    let hash_args = strings(&["-C", repository, "rev-parse", "--short", "HEAD"]);
    let hash = process::run(&git, &hash_args, None)
        .map_err(|error| CommandFailure::new("Read committed revision", &hash_args, error))?;
    if hash.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "Read committed revision",
            &hash_args,
            hash,
        ));
    }
    Ok(CommitSummary {
        short_hash: hash.stdout.trim().to_owned(),
        subject: message.lines().next().unwrap_or_default().trim().to_owned(),
    })
}

#[cfg(test)]
pub fn push(
    repository: &str,
    remote: Option<&str>,
    force_with_lease: bool,
) -> Result<PushOutput, CommandFailure> {
    push_cancellable(
        repository,
        remote,
        force_with_lease,
        &AtomicBool::new(false),
    )
}

pub fn push_cancellable(
    repository: &str,
    remote: Option<&str>,
    force_with_lease: bool,
    cancellation: &AtomicBool,
) -> Result<PushOutput, CommandFailure> {
    let git = git_executable("Push")?;
    let status = load_repository(repository)?;
    if status.head_oid.is_none() {
        return Err(local_failure(
            "Push",
            "Cannot push a repository with no commits.",
        ));
    }
    let mut args = vec![OsString::from("-C"), OsString::from(repository)];
    args.push(OsString::from("push"));
    if force_with_lease {
        args.push(OsString::from("--force-with-lease"));
    }
    match (status.upstream.as_deref(), remote) {
        (Some(_), None) => {}
        (None, Some(selected_remote)) => {
            if selected_remote.starts_with('-') {
                return Err(local_failure(
                    "Push",
                    "Remote names beginning with '-' cannot be used for this push.",
                ));
            }
            if !list_remotes(repository)?
                .iter()
                .any(|item| item == selected_remote)
            {
                return Err(local_failure(
                    "Push",
                    "Select a remote configured in this repository.",
                ));
            }
            let branch = status.branch.ok_or_else(|| {
                local_failure("Push", "Cannot set an upstream while HEAD is detached.")
            })?;
            args.push(OsString::from("-u"));
            args.push(OsString::from(selected_remote));
            args.push(OsString::from(branch));
        }
        (None, None) => return Err(local_failure("Push", "This branch has no upstream branch.")),
        (Some(_), Some(_)) => {
            return Err(local_failure(
                "Push",
                "A remote can only be selected when the branch has no upstream.",
            ));
        }
    }
    let ssh_command = ssh_command_for_network(&git, repository);
    let output = process::run_network_operation(
        &git,
        &args,
        None,
        network_timeout("Push")?,
        Some(cancellation),
        ssh_command.as_deref(),
    )
    .map_err(|error| {
        network_failure(&git, repository, CommandFailure::new("Push", &args, error))
    })?;
    if output.exit_code != Some(0) {
        return Err(network_failure(
            &git,
            repository,
            CommandFailure::from_output("Push", &args, output),
        ));
    }
    Ok(PushOutput {
        stdout: mask_credentials(&output.stdout),
        stderr: mask_credentials(&output.stderr),
        args: args
            .iter()
            .map(|arg| mask_credentials(&arg.to_string_lossy()))
            .collect(),
    })
}

pub fn fetch(repository: &str, cancellation: &AtomicBool) -> Result<PushOutput, CommandFailure> {
    let git = git_executable("Fetch")?;
    let status = load_repository(repository)?;
    let remotes = list_remotes(repository)?;
    let selected = choose_fetch_remote(status.upstream.as_deref(), &remotes)
        .ok_or_else(|| local_failure("Fetch", "No remote is configured in this repository."))?;
    let mut args = vec![OsString::from("-C"), OsString::from(repository)];
    args.extend(selected.iter().cloned().map(OsString::from));
    let ssh_command = ssh_command_for_network(&git, repository);
    let output = process::run_network_operation(
        &git,
        &args,
        None,
        network_timeout("Fetch")?,
        Some(cancellation),
        ssh_command.as_deref(),
    )
    .map_err(|error| {
        network_failure(&git, repository, CommandFailure::new("Fetch", &args, error))
    })?;
    if output.exit_code != Some(0) {
        return Err(network_failure(
            &git,
            repository,
            CommandFailure::from_output("Fetch", &args, output),
        ));
    }
    Ok(PushOutput {
        stdout: mask_credentials(&output.stdout),
        stderr: mask_credentials(&output.stderr),
        args: args
            .iter()
            .map(|arg| mask_credentials(&arg.to_string_lossy()))
            .collect(),
    })
}

pub fn pull(
    repository: &str,
    strategy: Option<&str>,
    cancellation: &AtomicBool,
) -> Result<PushOutput, CommandFailure> {
    ensure_no_operation(repository, "Pull")?;
    let git = git_executable("Pull")?;
    let status = load_repository(repository)?;
    if status.head_oid.is_none() {
        return Err(local_failure(
            "Pull",
            "Cannot pull a repository with no commits.",
        ));
    }
    if status.upstream.is_none() {
        return Err(local_failure("Pull", "This branch has no upstream branch."));
    }
    let mut args = vec![OsString::from("-C"), OsString::from(repository)];
    args.push(OsString::from("pull"));
    match strategy {
        None => {}
        Some("merge") => args.push(OsString::from("--no-rebase")),
        Some("rebase") => args.push(OsString::from("--rebase")),
        Some("ff-only") => args.push(OsString::from("--ff-only")),
        Some(_) => return Err(local_failure("Pull", "Unknown pull strategy.")),
    }
    let ssh_command = ssh_command_for_network(&git, repository);
    let output = process::run_network_operation(
        &git,
        &args,
        None,
        network_timeout("Pull")?,
        Some(cancellation),
        ssh_command.as_deref(),
    )
    .map_err(|error| {
        network_failure(&git, repository, CommandFailure::new("Pull", &args, error))
    })?;
    if output.exit_code != Some(0) {
        return Err(network_failure(
            &git,
            repository,
            CommandFailure::from_output("Pull", &args, output),
        ));
    }
    Ok(PushOutput {
        stdout: mask_credentials(&output.stdout),
        stderr: mask_credentials(&output.stderr),
        args: args
            .iter()
            .map(|arg| mask_credentials(&arg.to_string_lossy()))
            .collect(),
    })
}

fn choose_fetch_remote(upstream: Option<&str>, remotes: &[String]) -> Option<Vec<String>> {
    if let Some(remote) = upstream.and_then(|value| {
        remotes
            .iter()
            .filter(|remote| value.strip_prefix(&format!("{remote}/")).is_some())
            .max_by_key(|remote| remote.len())
    }) {
        return Some(vec![
            "fetch".to_owned(),
            "--prune".to_owned(),
            remote.clone(),
        ]);
    }
    match remotes {
        [remote] => Some(vec![
            "fetch".to_owned(),
            "--prune".to_owned(),
            remote.clone(),
        ]),
        [] => None,
        _ => Some(vec![
            "fetch".to_owned(),
            "--all".to_owned(),
            "--prune".to_owned(),
        ]),
    }
}

fn network_timeout(operation: &str) -> Result<Duration, CommandFailure> {
    let Some(value) = env::var_os("GIT_GUI_NETWORK_TIMEOUT_SECS") else {
        return Ok(Duration::from_secs(60));
    };
    let seconds = value
        .to_string_lossy()
        .parse::<u64>()
        .ok()
        .filter(|seconds| (1..=86_400).contains(seconds))
        .ok_or_else(|| {
            local_failure(
                operation,
                "GIT_GUI_NETWORK_TIMEOUT_SECS must be an integer between 1 and 86400.",
            )
        })?;
    Ok(Duration::from_secs(seconds))
}

fn ssh_command_for_network(git: &Path, repository: &str) -> Option<String> {
    let configured = env::var("GIT_SSH_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let args = strings(&["-C", repository, "config", "--get", "core.sshCommand"]);
            process::run(git, &args, None)
                .ok()
                .filter(|output| output.exit_code == Some(0))
                .map(|output| output.stdout.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            env::var("GIT_SSH")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "ssh".to_owned());
    Some(ssh_command_with_batch_mode(configured))
}

fn ssh_command_with_batch_mode(configured: String) -> String {
    let lower = configured.to_ascii_lowercase();
    if lower.contains("batchmode=yes") {
        configured
    } else if let Some(start) = lower.find("batchmode=no") {
        let mut command = configured;
        command.replace_range(start..start + "batchmode=no".len(), "BatchMode=yes");
        command
    } else {
        format!("{configured} -o BatchMode=yes")
    }
}

fn network_failure(git: &Path, repository: &str, mut failure: CommandFailure) -> CommandFailure {
    let output = format!("{}\n{}", failure.stderr, failure.stdout).to_ascii_lowercase();
    if [
        "enter passphrase for key",
        "permission denied (publickey)",
        "permission denied (password,publickey)",
        "permission denied, please try again",
        "could not read from remote repository",
    ]
    .iter()
    .any(|marker| output.contains(marker))
    {
        failure.stderr.push_str("\n\nSSH diagnostics:\n");
        failure.stderr.push_str(&ssh_diagnostics(git, repository));
    }
    failure
}

fn ssh_diagnostics(git: &Path, repository: &str) -> String {
    let source = if env::var("GIT_SSH_COMMAND")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        "GIT_SSH_COMMAND"
    } else {
        let args = strings(&["-C", repository, "config", "--get", "core.sshCommand"]);
        if process::run(git, &args, None)
            .ok()
            .is_some_and(|output| output.exit_code == Some(0) && !output.stdout.trim().is_empty())
        {
            "core.sshCommand"
        } else if env::var("GIT_SSH")
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
        {
            "GIT_SSH"
        } else {
            "system default (ssh)"
        }
    };
    let socket_set = env::var_os("SSH_AUTH_SOCK").is_some();
    let args = strings(&["-l"]);
    let agent = match process::run_with_timeout(
        Path::new("ssh-add"),
        &args,
        None,
        Duration::from_secs(3),
    ) {
        Ok(output) if output.exit_code == Some(0) => "accessible (identities loaded)",
        Ok(output)
            if format!("{}\n{}", output.stdout, output.stderr)
                .to_ascii_lowercase()
                .contains("no identities") =>
        {
            "accessible (no identities loaded)"
        }
        _ => "not accessible or ssh-add is unavailable",
    };
    format!(
        "SSH command source: {source}\nSSH_AUTH_SOCK: {}\nSSH agent: {agent}",
        if socket_set { "set" } else { "not set" }
    )
}

fn mask_credentials(value: &str) -> String {
    let mut masked = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(scheme_start) = rest.find("://") {
        let after_scheme = scheme_start + 3;
        masked.push_str(&rest[..after_scheme]);
        let authority_end = rest[after_scheme..]
            .find(['/', '?', '#', '\n', '\r', ' '])
            .map(|offset| after_scheme + offset)
            .unwrap_or(rest.len());
        let authority = &rest[after_scheme..authority_end];
        if let Some(at) = authority.rfind('@') {
            masked.push_str("[redacted]@");
            masked.push_str(&authority[at + 1..]);
        } else {
            masked.push_str(authority);
        }
        rest = &rest[authority_end..];
    }
    masked.push_str(rest);
    masked
}

fn git_executable(operation: &str) -> Result<PathBuf, CommandFailure> {
    find_git_executable().ok_or_else(|| CommandFailure {
        ok: false,
        operation: operation.to_owned(),
        args: vec!["git.exe".to_owned()],
        exit_code: None,
        stderr: "Git was not found in PATH or the common Git for Windows install locations."
            .to_owned(),
        stdout: String::new(),
    })
}

fn git_output(
    git: &Path,
    repository: &str,
    command: &[&str],
) -> Result<ProcessOutput, CommandFailure> {
    let mut args = strings(&["-C", repository]);
    args.extend(strings(command));
    let operation = command.first().copied().unwrap_or("Git");
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new(operation, &args, error))?;
    if output.exit_code == Some(0) {
        Ok(output)
    } else {
        Err(CommandFailure::from_output(operation, &args, output))
    }
}

fn git_path(git: &Path, repository: &str, marker: &str) -> Result<PathBuf, CommandFailure> {
    let output = git_output(git, repository, &["rev-parse", "--git-path", marker])?;
    let path = PathBuf::from(output.stdout.trim());
    Ok(if path.is_absolute() {
        path
    } else {
        Path::new(repository).join(path)
    })
}

fn local_failure(operation: &str, message: &str) -> CommandFailure {
    CommandFailure {
        ok: false,
        operation: operation.to_owned(),
        args: Vec::new(),
        exit_code: None,
        stderr: message.to_owned(),
        stdout: String::new(),
    }
}

fn ensure_no_operation(repository: &str, action: &str) -> Result<(), CommandFailure> {
    let state = repository_operation_state(repository)?;
    if let Some(operation) = state.operation {
        let action_label = action.to_lowercase();
        return Err(local_failure(
            action,
            &format!(
                "Cannot {action_label} while {operation} is in progress. Continue or abort it first."
            ),
        ));
    }
    Ok(())
}

fn add_identity_hint(
    git: &Path,
    repository: &str,
    failure: &mut CommandFailure,
) -> Result<(), CommandFailure> {
    if classify_git_failure(&format!("{}\n{}", failure.stderr, failure.stdout))
        != GitFailureKind::MissingIdentity
    {
        return Ok(());
    }
    let mut missing = Vec::new();
    for key in ["user.name", "user.email"] {
        let args = strings(&["-C", repository, "config", "--get", key]);
        let output = process::run(git, &args, None)
            .map_err(|error| CommandFailure::new("Read Git identity", &args, error))?;
        if output.exit_code == Some(1) || output.stdout.trim().is_empty() {
            missing.push(key);
        } else if output.exit_code != Some(0) {
            return Err(CommandFailure::from_output(
                "Read Git identity",
                &args,
                output,
            ));
        }
    }
    if !missing.is_empty() {
        failure
            .stderr
            .push_str("\n\nGit identity is not configured. Set the missing value(s):");
        for key in missing {
            failure
                .stderr
                .push_str(&format!("\ngit config --global {key} \"...\""));
        }
    }
    Ok(())
}

fn classify_git_failure(output: &str) -> GitFailureKind {
    let lower = output.to_ascii_lowercase();
    if lower.contains("author identity unknown")
        || lower.contains("committer identity unknown")
        || lower.contains("please tell me who you are")
        || lower.contains("unable to auto-detect email address")
    {
        GitFailureKind::MissingIdentity
    } else if lower.contains("nothing to commit") || lower.contains("no changes added to commit") {
        GitFailureKind::NothingToCommit
    } else if lower.contains("hook")
        && [
            "failed",
            "declined",
            "exited with code",
            "not successful",
            "rejected",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        GitFailureKind::HookRejected
    } else if lower.contains("non-fast-forward") || lower.contains("non fast forward") {
        GitFailureKind::NonFastForward
    } else if lower.contains("no upstream branch") || lower.contains("has no upstream") {
        GitFailureKind::NoUpstream
    } else if lower.contains("enter passphrase for key")
        || lower.contains("permission denied (publickey)")
        || lower.contains("permission denied (password,publickey)")
        || lower.contains("permission denied, please try again")
        || lower.contains("could not read from remote repository")
        || lower.contains("could not read username")
        || lower.contains("terminal prompts disabled")
        || lower.contains("git requested interactive input")
    {
        GitFailureKind::InteractiveInput
    } else {
        GitFailureKind::Other
    }
}

#[cfg(test)]
fn classify_branch_failure(output: &str) -> BranchFailureKind {
    let lower = output.to_ascii_lowercase();
    if lower.contains("already exists") {
        BranchFailureKind::AlreadyExists
    } else if lower.contains("not fully merged") || lower.contains("not merged") {
        BranchFailureKind::NotFullyMerged
    } else if lower.contains("local changes")
        && (lower.contains("overwritten") || lower.contains("would be overwritten"))
    {
        BranchFailureKind::LocalChangesWouldBeOverwritten
    } else if lower.contains("worktree") {
        BranchFailureKind::Worktree
    } else if lower.contains("cannot delete branch") && lower.contains("checked out") {
        BranchFailureKind::CurrentBranch
    } else if lower.contains("not a valid branch name") || lower.contains("invalid branch name") {
        BranchFailureKind::InvalidName
    } else {
        BranchFailureKind::Other
    }
}

fn is_published(remote_branches: &str) -> bool {
    remote_branches.lines().any(|line| !line.trim().is_empty())
}

pub fn parse_status(output: &str) -> Result<RepositoryStatus, String> {
    let mut branch = None;
    let mut upstream = None;
    let mut head_oid = None;
    let mut ahead = None;
    let mut behind = None;
    let mut files = Vec::new();
    let mut tokens = output
        .split('\0')
        .filter(|token| !token.is_empty())
        .peekable();

    while let Some(token) = tokens.next() {
        if let Some(header) = token.strip_prefix("# branch.oid ") {
            if header != "(initial)" {
                head_oid = Some(header.chars().take(7).collect());
            }
            continue;
        }
        if let Some(header) = token.strip_prefix("# branch.head ") {
            if header != "(detached)" {
                branch = Some(header.to_owned());
            }
            continue;
        }
        if let Some(header) = token.strip_prefix("# branch.upstream ") {
            upstream = Some(header.to_owned());
            continue;
        }
        if let Some(header) = token.strip_prefix("# branch.ab ") {
            let mut values = header.split_whitespace();
            ahead = values
                .next()
                .and_then(|value| value.strip_prefix('+'))
                .and_then(|value| value.parse().ok());
            behind = values
                .next()
                .and_then(|value| value.strip_prefix('-'))
                .and_then(|value| value.parse().ok());
            continue;
        }
        if token.starts_with("# ") {
            continue;
        }

        let (code, file_path, original_path, status) =
            if let Some(record) = token.strip_prefix("1 ") {
                let fields = record.splitn(8, ' ').collect::<Vec<_>>();
                if fields.len() != 8 {
                    return Err(format!("Malformed porcelain v2 ordinary record: {token}"));
                }
                (
                    fields[0].to_owned(),
                    fields[7].to_owned(),
                    None,
                    classify(fields[0]),
                )
            } else if let Some(record) = token.strip_prefix("2 ") {
                let fields = record.splitn(9, ' ').collect::<Vec<_>>();
                if fields.len() != 9 {
                    return Err(format!("Malformed porcelain v2 rename record: {token}"));
                }
                let original_path = tokens.next().ok_or_else(|| {
                    "Missing original path in porcelain v2 rename record.".to_owned()
                })?;
                (
                    fields[0].to_owned(),
                    fields[8].to_owned(),
                    Some(original_path.to_owned()),
                    "renamed",
                )
            } else if let Some(file_path) = token.strip_prefix("? ") {
                ("??".to_owned(), file_path.to_owned(), None, "untracked")
            } else if token.starts_with("u ") {
                let fields = token.splitn(11, ' ').collect::<Vec<_>>();
                if fields.len() != 11 {
                    return Err(format!("Malformed porcelain v2 unmerged record: {token}"));
                }
                (
                    fields[1].to_owned(),
                    fields[10].to_owned(),
                    None,
                    "conflicted",
                )
            } else if token.starts_with("# ") {
                continue;
            } else {
                return Err(format!("Unknown porcelain v2 status record: {token}"));
            };

        let mut status_chars = code.chars();
        let index_status = status_chars
            .next()
            .ok_or_else(|| format!("Missing index status in porcelain record: {token}"))?;
        let worktree_status = status_chars
            .next()
            .ok_or_else(|| format!("Missing worktree status in porcelain record: {token}"))?;
        if status_chars.next().is_some() {
            return Err(format!("Invalid status code in porcelain record: {token}"));
        }
        files.push(ChangedFile {
            path: file_path,
            original_path,
            index_status: index_status.to_string(),
            worktree_status: worktree_status.to_string(),
            code,
            status: status.to_owned(),
        });
    }
    Ok(RepositoryStatus {
        branch,
        upstream,
        head_oid,
        ahead,
        behind,
        files,
    })
}

pub fn parse_branches(output: &str) -> Result<(Vec<BranchInfo>, Vec<BranchInfo>), String> {
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for line in output.lines().filter(|line| !line.is_empty()) {
        let fields = line.trim_end_matches('\r').split('\0').collect::<Vec<_>>();
        if fields.len() != 8 {
            return Err(format!(
                "Malformed for-each-ref record: expected 8 NUL-separated fields, got {}.",
                fields.len()
            ));
        }
        let (full_ref, name, head, upstream, track, short_hash, commit_date, subject) = (
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6], fields[7],
        );
        if full_ref.starts_with("refs/remotes/") && name.ends_with("/HEAD") {
            continue;
        }
        let (ahead, behind) = parse_upstream_track(track);
        let branch = BranchInfo {
            name: name.to_owned(),
            current: head == "*",
            upstream: (!upstream.is_empty()).then(|| upstream.to_owned()),
            upstream_gone: track == "[gone]",
            ahead,
            behind,
            short_hash: short_hash.to_owned(),
            commit_date: commit_date.to_owned(),
            subject: subject.to_owned(),
        };
        if full_ref.starts_with("refs/heads/") {
            local.push(branch);
        } else if full_ref.starts_with("refs/remotes/") {
            remote.push(branch);
        } else {
            return Err(format!("Unexpected branch ref namespace: {full_ref}"));
        }
    }
    local.sort_by_key(|branch| !branch.current);
    Ok((local, remote))
}

fn parse_upstream_track(track: &str) -> (Option<usize>, Option<usize>) {
    if track == "[gone]" {
        return (None, None);
    }
    let mut ahead = None;
    let mut behind = None;
    for item in track.trim_matches(['[', ']']).split(", ") {
        if let Some(value) = item.strip_prefix("ahead ") {
            ahead = value.parse().ok();
        } else if let Some(value) = item.strip_prefix("behind ") {
            behind = value.parse().ok();
        }
    }
    (ahead, behind)
}

const DIFF_SIZE_LIMIT: usize = 128 * 1024;

const HISTORY_LOG_FORMAT: &str =
    "%H%x00%h%x00%P%x00%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%s%x00%B";

fn run_history_command(
    git: &Path,
    repository: &str,
    operation: &str,
    command: &[OsString],
) -> Result<ProcessOutput, CommandFailure> {
    let mut args = strings(&[
        "-C",
        repository,
        "-c",
        "i18n.logOutputEncoding=utf-8",
        "--literal-pathspecs",
    ]);
    args.extend_from_slice(command);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new(operation, &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(operation, &args, output));
    }
    Ok(output)
}

fn validate_history_revision(
    git: &Path,
    repository: &str,
    revision: &str,
) -> Result<(), CommandFailure> {
    if revision.starts_with("refs/heads/") || revision.starts_with("refs/remotes/") {
        let args = strings(&["-C", repository, "check-ref-format", revision]);
        let output = process::run(git, &args, None)
            .map_err(|error| CommandFailure::new("Validate history revision", &args, error))?;
        if output.exit_code == Some(0) {
            return Ok(());
        }
        return Err(CommandFailure::from_output(
            "Validate history revision",
            &args,
            output,
        ));
    }
    if revision == "HEAD" {
        return Ok(());
    }
    Err(local_failure(
        "Validate history revision",
        "History revisions must be HEAD or a local/remote branch ref.",
    ))
}

fn history_refs(
    git: &Path,
    repository: &str,
    head_hash: Option<&str>,
    current_branch: Option<&str>,
) -> Result<(HashMap<String, Vec<HistoryRef>>, String), CommandFailure> {
    let args = strings(&[
        "-C",
        repository,
        "for-each-ref",
        "--format=%(refname)%00%(objectname)%00%(*objectname)",
        "refs/heads",
        "refs/remotes",
        "refs/tags",
    ]);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new("List history refs", &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "List history refs",
            &args,
            output,
        ));
    }
    let mut hasher = DefaultHasher::new();
    output.stdout.hash(&mut hasher);
    let refs_hash = format!("{:016x}", hasher.finish());
    let mut refs: HashMap<String, Vec<HistoryRef>> = HashMap::new();
    for line in output.stdout.lines().filter(|line| !line.is_empty()) {
        let fields = line.split('\0').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(CommandFailure {
                ok: false,
                operation: "Parse history refs".to_owned(),
                args: args
                    .iter()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect(),
                exit_code: output.exit_code,
                stderr: format!("Malformed ref record: {line:?}"),
                stdout: output.stdout.clone(),
            });
        }
        let refname = fields[0];
        let short_name = refname
            .strip_prefix("refs/heads/")
            .or_else(|| refname.strip_prefix("refs/remotes/"))
            .or_else(|| refname.strip_prefix("refs/tags/"))
            .unwrap_or(refname)
            .to_owned();
        let kind = if refname.starts_with("refs/heads/") {
            "local"
        } else if refname.starts_with("refs/remotes/") {
            "remote"
        } else {
            "tag"
        };
        let is_head = current_branch == Some(short_name.as_str());
        let value = HistoryRef {
            name: short_name,
            kind: kind.to_owned(),
            head: is_head,
        };
        for object in fields[1..].iter().filter(|object| !object.is_empty()) {
            refs.entry((*object).to_owned())
                .or_default()
                .push(value.clone());
        }
    }
    if let Some(head_hash) = head_hash {
        refs.entry(head_hash.to_owned())
            .or_default()
            .push(HistoryRef {
                name: "HEAD".to_owned(),
                kind: "head".to_owned(),
                head: true,
            });
    }
    Ok((refs, refs_hash))
}

pub fn history_fingerprint(repository: &str) -> Result<HistoryFingerprint, CommandFailure> {
    let git = git_executable("Check history changes")?;
    let head_args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let head_output = process::run(&git, &head_args, None)
        .map_err(|error| CommandFailure::new("Read history HEAD", &head_args, error))?;
    let head_hash = match head_output.exit_code {
        Some(0) => Some(head_output.stdout.trim().to_owned()),
        Some(1) => None,
        _ => {
            return Err(CommandFailure::from_output(
                "Read history HEAD",
                &head_args,
                head_output,
            ));
        }
    };
    let (_, refs_hash) = history_refs(&git, repository, None, None)?;
    Ok(HistoryFingerprint {
        head_hash,
        refs_hash,
    })
}

pub fn parse_history_log(output: &str) -> Result<Vec<HistoryCommit>, String> {
    let fields = output.split('\0').collect::<Vec<_>>();
    let mut commits = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        if fields[index].is_empty() {
            index += 1;
            continue;
        }
        if fields.len() - index < 11 {
            return Err("Incomplete NUL-delimited git log record.".to_owned());
        }
        let record = &fields[index..index + 11];
        if record[0].is_empty() || record[1].is_empty() {
            return Err("Git log record is missing its commit hash.".to_owned());
        }
        commits.push(HistoryCommit {
            hash: record[0].to_owned(),
            short_hash: record[1].to_owned(),
            parents: record[2].split_whitespace().map(str::to_owned).collect(),
            author: record[3].to_owned(),
            author_email: record[4].to_owned(),
            author_date: record[5].to_owned(),
            committer: record[6].to_owned(),
            committer_email: record[7].to_owned(),
            commit_date: record[8].to_owned(),
            subject: record[9].to_owned(),
            body: record[10].to_owned(),
            refs: Vec::new(),
        });
        index += 11;
    }
    Ok(commits)
}

struct HistoryState {
    head_hash: Option<String>,
    current_branch: Option<String>,
    shallow: bool,
    upstream: Option<String>,
}

fn read_history_state(git: &Path, repository: &str) -> Result<HistoryState, CommandFailure> {
    let head_args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let head_output = process::run(git, &head_args, None)
        .map_err(|error| CommandFailure::new("Read history HEAD", &head_args, error))?;
    let head_hash = match head_output.exit_code {
        Some(0) => Some(head_output.stdout.trim().to_owned()),
        Some(1) => None,
        _ => {
            return Err(CommandFailure::from_output(
                "Read history HEAD",
                &head_args,
                head_output,
            ));
        }
    };

    let branch_args = strings(&[
        "-C",
        repository,
        "symbolic-ref",
        "--quiet",
        "--short",
        "HEAD",
    ]);
    let branch_output = process::run(git, &branch_args, None)
        .map_err(|error| CommandFailure::new("Read current branch", &branch_args, error))?;
    let current_branch = match branch_output.exit_code {
        Some(0) => Some(branch_output.stdout.trim().to_owned()),
        Some(1) => None,
        _ => {
            return Err(CommandFailure::from_output(
                "Read current branch",
                &branch_args,
                branch_output,
            ));
        }
    };

    let shallow_args = strings(&["-C", repository, "rev-parse", "--is-shallow-repository"]);
    let shallow_output = process::run(git, &shallow_args, None)
        .map_err(|error| CommandFailure::new("Check shallow repository", &shallow_args, error))?;
    if shallow_output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "Check shallow repository",
            &shallow_args,
            shallow_output,
        ));
    }
    let shallow = shallow_output.stdout.trim() == "true";

    let upstream_args = strings(&[
        "-C",
        repository,
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        "@{upstream}",
    ]);
    let upstream_output = process::run(git, &upstream_args, None)
        .map_err(|error| CommandFailure::new("Read history upstream", &upstream_args, error))?;
    let upstream = match upstream_output.exit_code {
        Some(0) => Some(upstream_output.stdout.trim().to_owned()),
        Some(1) => None,
        Some(128)
            if upstream_output
                .stderr
                .contains("no upstream configured for branch")
                || upstream_output.stderr.contains("no such branch:")
                || upstream_output
                    .stderr
                    .contains("HEAD does not point to a branch") =>
        {
            None
        }
        _ => {
            return Err(CommandFailure::from_output(
                "Read history upstream",
                &upstream_args,
                upstream_output,
            ));
        }
    };
    Ok(HistoryState {
        head_hash,
        current_branch,
        shallow,
        upstream,
    })
}

pub fn list_history(
    repository: &str,
    options: &HistoryOptions,
) -> Result<HistoryPage, CommandFailure> {
    let git = git_executable("List history")?;
    let revision = options.revision.as_deref();
    let all_refs = options.all_refs;
    let search = options.search.as_deref();
    let search_by = options.search_by.as_str();
    let path = options.path.as_deref();
    let offset = options.offset;
    let limit = options.limit.clamp(1, 500);
    if let Some(path) = path {
        validate_relative_path("Filter history by file", path)?;
    }
    let state = read_history_state(&git, repository)?;
    let hash_query = search
        .filter(|query| !query.is_empty())
        .filter(|_| search_by == "hash");
    let resolved_hash = if let Some(query) = hash_query {
        if !query.bytes().all(|byte| byte.is_ascii_hexdigit())
            || query.len() < 4
            || query.len() > 64
        {
            return Err(local_failure(
                "Search history by hash",
                "Enter at least four hexadecimal characters.",
            ));
        }
        let args = strings(&[
            "-C",
            repository,
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{query}^{{commit}}"),
        ]);
        let output = process::run(&git, &args, None)
            .map_err(|error| CommandFailure::new("Resolve history hash", &args, error))?;
        if output.exit_code != Some(0) {
            return Err(CommandFailure::from_output(
                "Resolve history hash",
                &args,
                output,
            ));
        }
        Some(output.stdout.trim().to_owned())
    } else {
        None
    };
    let scope = resolved_hash
        .clone()
        .or_else(|| (!all_refs).then(|| revision.unwrap_or("HEAD").to_owned()));
    let hash_target = resolved_hash.is_some();
    if !all_refs
        && !hash_target
        && let Some(scope) = scope.as_deref()
    {
        validate_history_revision(&git, repository, scope)?;
    }
    let (refs, refs_hash) = history_refs(
        &git,
        repository,
        state.head_hash.as_deref(),
        state.current_branch.as_deref(),
    )?;
    if state.head_hash.is_none() && !hash_target {
        return Ok(HistoryPage {
            commits: Vec::new(),
            has_more: false,
            shallow: state.shallow,
            detached: state.current_branch.is_none(),
            head_hash: state.head_hash,
            upstream: state.upstream,
            refs_hash,
        });
    }
    let mut command = vec![OsString::from("log"), OsString::from("-z")];
    command.push(OsString::from("--date-order"));
    command.push(OsString::from(format!("--format={HISTORY_LOG_FORMAT}")));
    if let Some(query) = search.filter(|query| !query.is_empty()) {
        match search_by {
            "message" if !hash_target => command.push(OsString::from(format!("--grep={query}"))),
            "author" if !hash_target => command.push(OsString::from(format!("--author={query}"))),
            "hash" => {}
            _ => {
                return Err(local_failure(
                    "Search history",
                    "Unsupported history search field.",
                ));
            }
        }
    }
    if path.is_some() {
        command.push(OsString::from("--follow"));
    }
    command.push(OsString::from(format!(
        "--max-count={}",
        if hash_target {
            1
        } else {
            limit.saturating_add(1)
        }
    )));
    command.push(OsString::from(format!(
        "--skip={}",
        if hash_target { 0 } else { offset }
    )));
    if hash_target {
        if let Some(scope) = scope.as_deref() {
            command.push(OsString::from("--end-of-options"));
            command.push(OsString::from(scope));
        }
    } else if all_refs {
        command.push(OsString::from("--all"));
    } else if let Some(scope) = scope.as_deref() {
        command.push(OsString::from("--end-of-options"));
        command.push(OsString::from(scope));
    } else {
        command.push(OsString::from("HEAD"));
    }
    if let Some(path) = path {
        command.push(OsString::from("--"));
        command.push(OsString::from(path));
    }
    let output = run_history_command(&git, repository, "List history", &command)?;
    let mut commits = parse_history_log(&output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse history".to_owned(),
        args: command
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: output.exit_code,
        stderr: message,
        stdout: output.stdout.clone(),
    })?;
    let has_more = !hash_target && commits.len() > limit;
    commits.truncate(limit);
    for commit in &mut commits {
        commit.refs = refs.get(&commit.hash).cloned().unwrap_or_default();
    }
    Ok(HistoryPage {
        commits,
        has_more,
        shallow: state.shallow,
        detached: state.current_branch.is_none(),
        head_hash: state.head_hash,
        upstream: state.upstream,
        refs_hash,
    })
}

pub fn list_unpushed_commits(repository: &str) -> Result<Vec<String>, CommandFailure> {
    let git = git_executable("List unpushed commits")?;
    if read_history_state(&git, repository)?.upstream.is_none() {
        return Ok(Vec::new());
    }
    let args = strings(&[
        "-C",
        repository,
        "rev-list",
        "--end-of-options",
        "@{upstream}..HEAD",
    ]);
    let output = process::run(&git, &args, None)
        .map_err(|error| CommandFailure::new("List unpushed commits", &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "List unpushed commits",
            &args,
            output,
        ));
    }
    Ok(output.stdout.lines().map(str::to_owned).collect())
}

fn resolve_history_commit(
    git: &Path,
    repository: &str,
    hash: &str,
) -> Result<HistoryCommit, CommandFailure> {
    if hash.len() != 40 && hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(local_failure(
            "Read commit details",
            "Commit hash must be a full hexadecimal object id.",
        ));
    }
    let command = [
        OsString::from("log"),
        OsString::from("-z"),
        OsString::from("--max-count=1"),
        OsString::from(format!("--format={HISTORY_LOG_FORMAT}")),
        OsString::from("--end-of-options"),
        OsString::from(hash),
    ];
    let output = run_history_command(git, repository, "Read commit details", &command)?;
    let mut commits = parse_history_log(&output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse commit details".to_owned(),
        args: command
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: output.exit_code,
        stderr: message,
        stdout: output.stdout.clone(),
    })?;
    commits.pop().ok_or_else(|| {
        local_failure(
            "Read commit details",
            "Git returned no commit for this hash.",
        )
    })
}

fn parse_name_status(output: &str) -> Result<Vec<(String, Option<String>, String)>, String> {
    let fields = output
        .split('\0')
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let status = fields[index];
        index += 1;
        let code = status
            .chars()
            .next()
            .ok_or_else(|| "Missing file status.".to_owned())?;
        match code {
            'R' | 'C' => {
                if index + 1 >= fields.len() {
                    return Err("Incomplete rename/copy path record.".to_owned());
                }
                let original = fields[index].to_owned();
                let path = fields[index + 1].to_owned();
                index += 2;
                files.push((path, Some(original), code.to_string()));
            }
            'A' | 'M' | 'D' | 'T' | 'U' => {
                let path = fields
                    .get(index)
                    .ok_or_else(|| "Incomplete file status record.".to_owned())?
                    .to_string();
                index += 1;
                files.push((path, None, code.to_string()));
            }
            _ => return Err(format!("Unknown file status record: {status}")),
        }
    }
    Ok(files)
}

type Numstat = (Option<usize>, Option<usize>, bool);

fn parse_numstat(output: &str) -> Result<HashMap<String, Numstat>, String> {
    let fields = output.split_terminator('\0').collect::<Vec<_>>();
    let mut files = HashMap::new();
    let mut index = 0;
    while index < fields.len() {
        let record = fields[index];
        index += 1;
        let mut columns = record.splitn(3, '\t');
        let additions = columns
            .next()
            .ok_or_else(|| "Missing numstat additions.".to_owned())?;
        let deletions = columns
            .next()
            .ok_or_else(|| "Missing numstat deletions.".to_owned())?;
        let path = columns
            .next()
            .ok_or_else(|| "Missing numstat path.".to_owned())?;
        let binary = additions == "-" && deletions == "-";
        let addition_count = if binary {
            None
        } else {
            Some(
                additions
                    .parse()
                    .map_err(|_| "Invalid numstat additions.".to_owned())?,
            )
        };
        let deletion_count = if binary {
            None
        } else {
            Some(
                deletions
                    .parse()
                    .map_err(|_| "Invalid numstat deletions.".to_owned())?,
            )
        };
        let actual_path = if path.is_empty() {
            let _old = fields
                .get(index)
                .ok_or_else(|| "Missing old rename path in numstat.".to_owned())?;
            let new = fields
                .get(index + 1)
                .ok_or_else(|| "Missing new rename path in numstat.".to_owned())?;
            index += 2;
            (*new).to_owned()
        } else {
            path.to_owned()
        };
        files.insert(actual_path, (addition_count, deletion_count, binary));
    }
    Ok(files)
}

fn history_comparison_args(
    commit: &HistoryCommit,
    parent: Option<&str>,
) -> Result<Vec<OsString>, CommandFailure> {
    if commit.parents.is_empty() {
        if parent.is_some() {
            return Err(local_failure(
                "Compare commit",
                "Root commits do not have a parent to compare against.",
            ));
        }
        return Ok(vec![OsString::from("--root"), OsString::from(&commit.hash)]);
    }
    let comparison_parent = parent.unwrap_or(&commit.parents[0]);
    if !commit
        .parents
        .iter()
        .any(|candidate| candidate == comparison_parent)
    {
        return Err(local_failure(
            "Compare commit",
            "The selected comparison parent is not a parent of this commit.",
        ));
    }
    Ok(vec![
        OsString::from(comparison_parent),
        OsString::from(&commit.hash),
    ])
}

pub fn load_history_detail(
    repository: &str,
    hash: &str,
    parent: Option<&str>,
) -> Result<HistoryDetail, CommandFailure> {
    let git = git_executable("Read commit details")?;
    let mut commit = resolve_history_commit(&git, repository, hash)?;
    let state = read_history_state(&git, repository)?;
    let (refs, _) = history_refs(
        &git,
        repository,
        state.head_hash.as_deref(),
        state.current_branch.as_deref(),
    )?;
    commit.refs = refs.get(&commit.hash).cloned().unwrap_or_default();
    let comparison_args = history_comparison_args(&commit, parent)?;

    let mut status_command = vec![
        OsString::from("diff-tree"),
        OsString::from("--no-commit-id"),
        OsString::from("--name-status"),
        OsString::from("-r"),
        OsString::from("-M"),
        OsString::from("-C"),
        OsString::from("-z"),
    ];
    status_command.extend(comparison_args.iter().cloned());
    let status_output =
        run_history_command(&git, repository, "List commit files", &status_command)?;
    let statuses = parse_name_status(&status_output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse commit files".to_owned(),
        args: status_command
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: status_output.exit_code,
        stderr: message,
        stdout: status_output.stdout.clone(),
    })?;

    let mut numstat_command = vec![
        OsString::from("diff-tree"),
        OsString::from("--no-commit-id"),
        OsString::from("--numstat"),
        OsString::from("-r"),
        OsString::from("-M"),
        OsString::from("-C"),
        OsString::from("-z"),
    ];
    numstat_command.extend(comparison_args.iter().cloned());
    let numstat_output =
        run_history_command(&git, repository, "Read commit statistics", &numstat_command)?;
    let numstats = parse_numstat(&numstat_output.stdout).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse commit statistics".to_owned(),
        args: numstat_command
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: numstat_output.exit_code,
        stderr: message,
        stdout: numstat_output.stdout.clone(),
    })?;

    let mut shortstat_command = vec![
        OsString::from("diff-tree"),
        OsString::from("--shortstat"),
        OsString::from("-r"),
        OsString::from("-M"),
        OsString::from("-C"),
    ];
    shortstat_command.extend(comparison_args);
    let shortstat_output =
        run_history_command(&git, repository, "Read commit summary", &shortstat_command)?;
    let comparison_parent = parent
        .map(str::to_owned)
        .or_else(|| commit.parents.first().cloned());
    let files = statuses
        .into_iter()
        .map(|(path, original_path, status)| {
            let (additions, deletions, binary) =
                numstats.get(&path).cloned().unwrap_or((None, None, false));
            HistoryFile {
                path,
                original_path,
                status,
                additions,
                deletions,
                binary,
            }
        })
        .collect();
    Ok(HistoryDetail {
        commit,
        files,
        shortstat: shortstat_output.stdout.trim().to_owned(),
        comparison_parent,
    })
}

pub fn load_history_diff(
    repository: &str,
    hash: &str,
    parent: Option<&str>,
    path: Option<&str>,
    load_large: bool,
) -> Result<DiffPreview, CommandFailure> {
    if let Some(path) = path {
        validate_relative_path("Load commit diff", path)?;
    }
    let git = git_executable("Load commit diff")?;
    let commit = resolve_history_commit(&git, repository, hash)?;
    let comparison_args = history_comparison_args(&commit, parent)?;
    let mut command = vec![
        OsString::from("diff"),
        OsString::from("--no-ext-diff"),
        OsString::from("--no-color"),
        OsString::from("-M"),
        OsString::from("-C"),
    ];
    if commit.parents.is_empty() {
        command = vec![
            OsString::from("show"),
            OsString::from("--format="),
            OsString::from("--root"),
            OsString::from("--no-ext-diff"),
            OsString::from("--no-color"),
            OsString::from("-M"),
            OsString::from("-C"),
            OsString::from(&commit.hash),
        ];
    } else {
        command.extend(comparison_args);
    }
    if let Some(path) = path {
        command.push(OsString::from("--"));
        command.push(OsString::from(path));
    }
    let args = {
        let mut args = strings(&[
            "-C",
            repository,
            "-c",
            "i18n.logOutputEncoding=utf-8",
            "--literal-pathspecs",
        ]);
        args.extend(command);
        args
    };
    let (output, truncated) = if load_large {
        (
            process::run(&git, &args, None)
                .map_err(|error| CommandFailure::new("Load commit diff", &args, error))?,
            false,
        )
    } else {
        process::run_with_output_limit(&git, &args, None, DIFF_SIZE_LIMIT + 1)
            .map_err(|error| CommandFailure::new("Load commit diff", &args, error))?
    };
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(
            "Load commit diff",
            &args,
            output,
        ));
    }
    let too_large = truncated || output.stdout.len() > DIFF_SIZE_LIMIT;
    if too_large && !load_large {
        return Ok(DiffPreview {
            binary: false,
            too_large: true,
            metadata: Vec::new(),
            hunks: Vec::new(),
        });
    }
    parse_diff(&output.stdout, too_large).map_err(|message| CommandFailure {
        ok: false,
        operation: "Parse commit diff".to_owned(),
        args: args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exit_code: output.exit_code,
        stderr: message,
        stdout: output.stdout,
    })
}

pub fn load_diff(
    repository: &str,
    path: &str,
    staged: bool,
    untracked: bool,
    load_large: bool,
) -> Result<DiffPreview, CommandFailure> {
    validate_relative_path("Load diff", path)?;
    let git = find_git_executable().ok_or_else(|| CommandFailure {
        ok: false,
        operation: "Load diff".to_owned(),
        args: vec!["git.exe".to_owned()],
        exit_code: None,
        stderr: "Git was not found in PATH or the common Git for Windows install locations."
            .to_owned(),
        stdout: String::new(),
    })?;
    let mut args = vec![OsString::from("-C"), OsString::from(repository)];
    args.extend(strings(&[
        "--literal-pathspecs",
        "diff",
        "--no-ext-diff",
        "--no-color",
    ]));
    if untracked {
        args.push(OsString::from("--no-index"));
    } else if staged {
        args.push(OsString::from("--cached"));
    }
    args.push(OsString::from("--"));
    if untracked && cfg!(windows) {
        args.push(OsString::from("NUL"));
    } else if untracked {
        args.push(OsString::from("/dev/null"));
    }
    args.push(OsString::from(path));
    let (output, stdout_truncated) = if load_large {
        (
            process::run(&git, &args, None)
                .map_err(|error| CommandFailure::new("Load diff", &args, error))?,
            false,
        )
    } else {
        process::run_with_output_limit(&git, &args, None, DIFF_SIZE_LIMIT + 1)
            .map_err(|error| CommandFailure::new("Load diff", &args, error))?
    };
    if output.exit_code != Some(0) && !(untracked && output.exit_code == Some(1)) {
        return Err(CommandFailure::from_output("Load diff", &args, output));
    }

    let too_large = stdout_truncated || output.stdout.len() > DIFF_SIZE_LIMIT;
    let preview = if too_large && !load_large {
        DiffPreview {
            binary: false,
            too_large: true,
            metadata: Vec::new(),
            hunks: Vec::new(),
        }
    } else {
        parse_diff(&output.stdout, too_large).map_err(|message| CommandFailure {
            ok: false,
            operation: "Parse diff".to_owned(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            exit_code: output.exit_code,
            stderr: message,
            stdout: output.stdout.clone(),
        })?
    };
    Ok(preview)
}

pub fn run_change_operation(
    repository: &str,
    operation: &str,
    paths: &[String],
) -> Result<(), CommandFailure> {
    if matches!(
        operation,
        "Discard untracked" | "Discard all untracked" | "Discard all tracked"
    ) && paths.is_empty()
    {
        return Ok(());
    }
    validate_relative_paths(operation, paths)?;
    let git = find_git_executable().ok_or_else(|| CommandFailure {
        ok: false,
        operation: operation.to_owned(),
        args: vec!["git.exe".to_owned()],
        exit_code: None,
        stderr: "Git was not found in PATH or the common Git for Windows install locations."
            .to_owned(),
        stdout: String::new(),
    })?;

    let has_head = if matches!(operation, "Unstage" | "Unstage all") {
        Some(has_head(&git, repository)?)
    } else {
        None
    };
    let mut args = vec![
        OsString::from("-C"),
        OsString::from(repository),
        OsString::from("--literal-pathspecs"),
    ];
    match operation {
        "Stage" => {
            args.extend(strings(&["add", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        "Stage all" => args.extend(strings(&["add", "-A"])),
        "Unstage" if has_head == Some(true) => {
            args.extend(strings(&["restore", "--staged", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        "Unstage" => {
            args.extend(strings(&["rm", "--cached", "--ignore-unmatch", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        "Unstage all" if has_head == Some(true) => {
            args.extend(strings(&["restore", "--staged", "."]))
        }
        "Unstage all" => args.extend(strings(&[
            "rm",
            "-r",
            "--cached",
            "--ignore-unmatch",
            "--",
            ".",
        ])),
        "Discard" => {
            args.extend(strings(&["restore", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        "Discard untracked" => {
            args.extend(strings(&["clean", "-f", "-d", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        "Discard all tracked" => {
            args.extend(strings(&["restore", "--worktree", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        "Discard all untracked" => {
            args.extend(strings(&["clean", "-f", "-d", "--"]));
            args.extend(paths.iter().map(OsString::from));
        }
        _ => {
            return Err(CommandFailure {
                ok: false,
                operation: operation.to_owned(),
                args: Vec::new(),
                exit_code: None,
                stderr: "Unsupported working-tree operation.".to_owned(),
                stdout: String::new(),
            });
        }
    }

    let output = process::run(&git, &args, None)
        .map_err(|error| CommandFailure::new(operation, &args, error))?;
    if output.exit_code != Some(0) {
        return Err(CommandFailure::from_output(operation, &args, output));
    }
    Ok(())
}

fn validate_relative_paths(operation: &str, paths: &[String]) -> Result<(), CommandFailure> {
    for path in paths {
        validate_relative_path(operation, path)?;
    }
    Ok(())
}

fn validate_relative_path(operation: &str, path: &str) -> Result<(), CommandFailure> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::CurDir
            )
        })
    {
        return Err(CommandFailure {
            ok: false,
            operation: operation.to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: format!("Refusing to use a path outside the repository: {path}"),
            stdout: String::new(),
        });
    }
    Ok(())
}

fn has_head(git: &std::path::Path, repository: &str) -> Result<bool, CommandFailure> {
    let args = strings(&["-C", repository, "rev-parse", "--verify", "--quiet", "HEAD"]);
    let output = process::run(git, &args, None)
        .map_err(|error| CommandFailure::new("Check repository HEAD", &args, error))?;
    if output.exit_code == Some(0) {
        Ok(true)
    } else if output.exit_code == Some(1) {
        Ok(false)
    } else {
        Err(CommandFailure::from_output(
            "Check repository HEAD",
            &args,
            output,
        ))
    }
}

pub fn parse_diff(output: &str, too_large: bool) -> Result<DiffPreview, String> {
    let binary = output.contains("Binary files ") || output.contains("GIT binary patch");
    let mut metadata = Vec::new();
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut old_line = 0usize;
    let mut new_line = 0usize;
    let mut in_hunk = false;

    for line in output.split_terminator('\n') {
        if line.starts_with("@@ ") {
            let (old_start, new_start) = parse_hunk_start(line)
                .ok_or_else(|| format!("Malformed unified diff hunk header: {line}"))?;
            old_line = old_start;
            new_line = new_start;
            hunks.push(DiffHunk {
                header: line.to_owned(),
                lines: Vec::new(),
            });
            in_hunk = true;
        } else if line.starts_with("diff --git ") {
            metadata.push(line.to_owned());
            in_hunk = false;
        } else if line == r"\ No newline at end of file" {
            if in_hunk && let Some(hunk) = hunks.last_mut() {
                hunk.lines.push(DiffLine {
                    kind: "marker".to_owned(),
                    content: line.to_owned(),
                    old_line: None,
                    new_line: None,
                });
            }
        } else if in_hunk {
            let (kind, old_number, new_number) = match line.as_bytes().first() {
                Some(b' ') => {
                    let result = ("context", Some(old_line), Some(new_line));
                    old_line += 1;
                    new_line += 1;
                    result
                }
                Some(b'+') => {
                    let result = ("addition", None, Some(new_line));
                    new_line += 1;
                    result
                }
                Some(b'-') => {
                    let result = ("removal", Some(old_line), None);
                    old_line += 1;
                    result
                }
                _ => continue,
            };
            if let Some(hunk) = hunks.last_mut() {
                hunk.lines.push(DiffLine {
                    kind: kind.to_owned(),
                    content: line.to_owned(),
                    old_line: old_number,
                    new_line: new_number,
                });
            }
        } else if line.starts_with("index ")
            || line.starts_with("new file mode ")
            || line.starts_with("deleted file mode ")
            || line.starts_with("old mode ")
            || line.starts_with("new mode ")
            || line.starts_with("similarity index ")
            || line.starts_with("rename from ")
            || line.starts_with("rename to ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("Binary files ")
            || line.starts_with("GIT binary patch")
        {
            metadata.push(line.to_owned());
        }
    }

    Ok(DiffPreview {
        binary,
        too_large,
        metadata,
        hunks,
    })
}

fn parse_hunk_start(header: &str) -> Option<(usize, usize)> {
    let mut ranges = header
        .split_whitespace()
        .filter(|part| part.starts_with('-') || part.starts_with('+'));
    let old = ranges
        .next()?
        .strip_prefix('-')?
        .split(',')
        .next()?
        .parse()
        .ok()?;
    let new = ranges
        .next()?
        .strip_prefix('+')?
        .split(',')
        .next()?
        .parse()
        .ok()?;
    Some((old, new))
}

fn classify(code: &str) -> &'static str {
    if code.contains('U') {
        "conflicted"
    } else if code.contains('R') || code.contains('C') {
        "renamed"
    } else {
        "changed"
    }
}

fn strings(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn find_git_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        candidates
            .extend(env::split_paths(&path).map(|directory| directory.join(git_executable_name())));
    }
    candidates.extend(common_git_locations());
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn common_git_locations() -> impl Iterator<Item = PathBuf> {
    let mut candidates = Vec::new();
    #[cfg(windows)]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
            if let Some(root) = env::var_os(variable) {
                let root = PathBuf::from(root);
                candidates.push(root.join("Git").join("cmd").join("git.exe"));
                candidates.push(root.join("Git").join("bin").join("git.exe"));
                candidates.push(
                    root.join("Programs")
                        .join("Git")
                        .join("cmd")
                        .join("git.exe"),
                );
            }
        }
        candidates.push(PathBuf::from(r"C:\Program Files\Git\cmd\git.exe"));
        candidates.push(PathBuf::from(r"C:\Program Files\Git\bin\git.exe"));
        candidates.push(PathBuf::from(r"C:\Program Files (x86)\Git\cmd\git.exe"));
    }
    #[cfg(not(windows))]
    candidates.push(PathBuf::from("/usr/bin/git"));
    candidates.into_iter()
}

#[cfg(windows)]
fn git_executable_name() -> &'static str {
    "git.exe"
}

#[cfg(not(windows))]
fn git_executable_name() -> &'static str {
    "git"
}

#[cfg(test)]
mod tests {
    use super::{
        BranchFailureKind, GitFailureKind, HistoryOptions, choose_fetch_remote,
        classify_branch_failure, classify_git_failure, create_commit, history_comparison_args,
        history_fingerprint, is_published, list_branches, list_history, list_remote_details,
        list_unpushed_commits, load_commit_context, load_history_detail, load_history_diff,
        mask_credentials, parse_branches, parse_diff, parse_history_log, parse_name_status,
        parse_numstat, parse_status, validate_history_revision,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Output;
    use std::sync::atomic::AtomicBool;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TemporaryRepository(PathBuf);

    impl TemporaryRepository {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "git-gui-commit-test-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryRepository {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn run_git(repository: &Path, args: &[&str]) -> Output {
        let git = super::find_git_executable().unwrap();
        let output = crate::process::command(git)
            .arg("-C")
            .arg(repository)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn setup_repository(path: &Path) {
        run_git(path, &["init", "--quiet", "--initial-branch=main"]);
        run_git(path, &["config", "user.name", "Git GUI Test"]);
        run_git(
            path,
            &["config", "user.email", "git-gui-test@example.invalid"],
        );
        fs::write(path.join("first.txt"), "first\n").unwrap();
        run_git(path, &["add", "first.txt"]);
    }

    fn commit_file(repository: &Path, filename: &str, contents: &str, message: &str) {
        fs::write(repository.join(filename), contents).unwrap();
        run_git(repository, &["add", filename]);
        run_git(repository, &["commit", "-m", message]);
    }

    #[test]
    fn classifies_common_commit_and_push_error_fixtures() {
        assert_eq!(
            classify_git_failure("Author identity unknown\nPlease tell me who you are."),
            GitFailureKind::MissingIdentity
        );
        assert_eq!(
            classify_git_failure("nothing to commit, working tree clean"),
            GitFailureKind::NothingToCommit
        );
        assert_eq!(
            classify_git_failure("pre-commit hook exited with code 1"),
            GitFailureKind::HookRejected
        );
        assert_eq!(
            classify_git_failure("rejected: non-fast-forward"),
            GitFailureKind::NonFastForward
        );
        assert_eq!(
            classify_git_failure("fatal: The current branch has no upstream branch."),
            GitFailureKind::NoUpstream
        );
        assert_eq!(
            classify_git_failure("Enter passphrase for key 'id_ed25519':"),
            GitFailureKind::InteractiveInput
        );
        assert_eq!(
            classify_git_failure("Permission denied (publickey)."),
            GitFailureKind::InteractiveInput
        );
        assert_eq!(
            classify_git_failure(
                "Permission denied, please try again.\nPermission denied (password,publickey).\nfatal: Could not read from remote repository."
            ),
            GitFailureKind::InteractiveInput
        );
    }

    #[test]
    fn classifies_branch_failure_fixtures() {
        assert_eq!(
            classify_branch_failure("fatal: a branch named 'feature/login' already exists"),
            BranchFailureKind::AlreadyExists
        );
        assert_eq!(
            classify_branch_failure("error: The branch 'topic' is not fully merged."),
            BranchFailureKind::NotFullyMerged
        );
        assert_eq!(
            classify_branch_failure("Your local changes would be overwritten by checkout."),
            BranchFailureKind::LocalChangesWouldBeOverwritten
        );
        assert_eq!(
            classify_branch_failure("error: Cannot delete branch 'main' checked out at 'repo'"),
            BranchFailureKind::CurrentBranch
        );
        assert_eq!(
            classify_branch_failure("fatal: 'bad name' is not a valid branch name"),
            BranchFailureKind::InvalidName
        );
        assert_eq!(
            classify_branch_failure("fatal: branch 'topic' is checked out at '/other/worktree'"),
            BranchFailureKind::Worktree
        );
    }

    #[test]
    fn detects_whether_a_remote_branch_contains_head() {
        assert!(is_published("  origin/main\n"));
        assert!(!is_published(""));
        assert!(!is_published(" \n"));
    }

    #[test]
    fn creates_first_commit_amends_message_and_tracks_published_state() {
        let temporary = TemporaryRepository::new();
        let repository = temporary.path().join("repository");
        fs::create_dir(&repository).unwrap();
        setup_repository(&repository);

        let first = create_commit(
            repository.to_str().unwrap(),
            "Corrigir cobrança 🚀\n\nPreço atualizado: café",
            false,
        )
        .unwrap();
        assert_eq!(first.subject, "Corrigir cobrança 🚀");
        assert!(!first.short_hash.is_empty());
        let first_message = run_git(&repository, &["log", "-1", "--format=%B"]);
        assert_eq!(
            String::from_utf8(first_message.stdout).unwrap().trim(),
            "Corrigir cobrança 🚀\n\nPreço atualizado: café"
        );

        let bare = temporary.path().join("remote.git");
        fs::create_dir(&bare).unwrap();
        run_git(&bare, &["init", "--bare", "--quiet"]);
        run_git(
            &repository,
            &["remote", "add", "origin", bare.to_str().unwrap()],
        );
        super::push(repository.to_str().unwrap(), Some("origin"), false).unwrap();
        super::push(repository.to_str().unwrap(), None, false).unwrap();
        assert!(
            load_commit_context(repository.to_str().unwrap())
                .unwrap()
                .published
        );

        let amended = create_commit(
            repository.to_str().unwrap(),
            "Corrigir cobrança revisada",
            true,
        )
        .unwrap();
        assert_ne!(first.short_hash, amended.short_hash);
        assert!(
            !load_commit_context(repository.to_str().unwrap())
                .unwrap()
                .published
        );
        assert_eq!(
            String::from_utf8(run_git(&repository, &["log", "-1", "--format=%B"]).stdout)
                .unwrap()
                .trim(),
            "Corrigir cobrança revisada"
        );
    }

    #[test]
    fn parses_branch_upstream_and_changed_files_from_porcelain_v2() {
        let fixture = concat!(
            "# branch.oid 8f4147b50d45fcb569754b54918486c2bc50a99b\0",
            "# branch.head develop\0",
            "# branch.upstream origin/develop\0",
            "# branch.ab +0 -0\0",
            "1 .M N... 100644 100644 100644 e79c5e8f964493290a409888d5413a737e8e5dd5 e79c5e8f964493290a409888d5413a737e8e5dd5 app.ts\0",
            "? new file.txt\0",
        );
        let parsed = parse_status(fixture).expect("valid porcelain fixture");
        assert_eq!(parsed.branch.as_deref(), Some("develop"));
        assert_eq!(parsed.upstream.as_deref(), Some("origin/develop"));
        assert_eq!(parsed.head_oid.as_deref(), Some("8f4147b"));
        assert_eq!(parsed.ahead, Some(0));
        assert_eq!(parsed.behind, Some(0));
        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.files[0].path, "app.ts");
        assert_eq!(parsed.files[0].code, ".M");
        assert_eq!(parsed.files[0].index_status, ".");
        assert_eq!(parsed.files[0].worktree_status, "M");
        assert_eq!(parsed.files[0].status, "changed");
        assert_eq!(parsed.files[1].path, "new file.txt");
        assert_eq!(parsed.files[1].status, "untracked");
    }

    #[test]
    fn parses_renames_with_paths_containing_spaces() {
        let fixture = concat!(
            "# branch.oid 40688a320659cfc2d45d8d9548e297fbfe4a09d1\0",
            "# branch.head main\0",
            "2 R. N... 100644 100644 100644 df967b96a579e45a18b8251732d16804b2e56a55 df967b96a579e45a18b8251732d16804b2e56a55 R100 renamed file.txt\0",
            "old file.txt\0",
        );
        let parsed = parse_status(fixture).expect("valid rename fixture");
        assert_eq!(parsed.files[0].path, "renamed file.txt");
        assert_eq!(
            parsed.files[0].original_path.as_deref(),
            Some("old file.txt")
        );
        assert_eq!(parsed.files[0].status, "renamed");
    }

    #[test]
    fn recognizes_detached_head_and_conflicts() {
        let detached = parse_status(
            "# branch.oid 78135e9e632ed59003f86b4ef139ed35372bd20a\0# branch.head (detached)\0",
        )
        .expect("valid detached-head fixture");
        assert_eq!(detached.head_oid.as_deref(), Some("78135e9"));
        let fixture = concat!(
            "# branch.oid 78135e9e632ed59003f86b4ef139ed35372bd20a\0",
            "# branch.head main\0",
            "u UU N... 100644 100644 100644 100644 df967b96a579e45a18b8251732d16804b2e56a55 b19a1e93bec1317dc6097229e12afaffbfa74dc2 950b81b7eee953d050aa05a641f8e056c85dd1bd old file.txt\0",
        );
        let parsed = parse_status(fixture).expect("valid conflict fixture");
        assert_eq!(detached.branch, None);
        assert_eq!(parsed.branch.as_deref(), Some("main"));
        assert_eq!(parsed.files[0].path, "old file.txt");
        assert_eq!(parsed.files[0].status, "conflicted");
    }

    #[test]
    fn parses_separate_staged_and_unstaged_records_and_special_paths() {
        let fixture = concat!(
            "# branch.oid 8f4147b50d45fcb569754b54918486c2bc50a99b\0",
            "# branch.head main\0",
            "1 M. N... 100644 100644 100644 e79c5e8f964493290a409888d5413a737e8e5dd5 e79c5e8f964493290a409888d5413a737e8e5dd5 staged.txt\0",
            "1 .D N... 100644 100644 000000 e79c5e8f964493290a409888d5413a737e8e5dd5 0000000000000000000000000000000000000000 folder/deleted file.txt\0",
            "1 MM N... 100644 100644 100644 e79c5e8f964493290a409888d5413a737e8e5dd5 e79c5e8f964493290a409888d5413a737e8e5dd5 both.txt\0",
            "? café file.txt\0",
            "? path with [special] chars.txt\0",
        );
        let parsed = parse_status(fixture).expect("valid status fixture");
        assert_eq!(parsed.files.len(), 5);
        assert_eq!(parsed.files[1].status, "changed");
        assert_eq!(parsed.files[1].worktree_status, "D");
        assert_eq!(parsed.files[2].index_status, "M");
        assert_eq!(parsed.files[2].worktree_status, "M");
        assert_eq!(parsed.files[3].path, "café file.txt");
        assert_eq!(parsed.files[3].status, "untracked");
        assert_eq!(parsed.files[4].path, "path with [special] chars.txt");
    }

    #[test]
    fn parses_multiple_diff_hunks_and_line_numbers() {
        let fixture = concat!(
            "diff --git a/file.txt b/file.txt\n",
            "index 1234567..abcdefg 100644\n",
            "--- a/file.txt\n",
            "+++ b/file.txt\n",
            "@@ -1,3 +1,3 @@ heading\n",
            " context\n",
            "-old value\n",
            "+new value\n",
            "\\ No newline at end of file\n",
            "@@ -10 +10,2 @@\n",
            "-tail\n",
            "+tail changed\n",
            "+extra\n",
        );
        let parsed = parse_diff(fixture, false).expect("valid unified diff");
        assert!(!parsed.binary);
        assert_eq!(parsed.hunks.len(), 2);
        assert_eq!(parsed.hunks[0].lines[0].old_line, Some(1));
        assert_eq!(parsed.hunks[0].lines[0].new_line, Some(1));
        assert_eq!(parsed.hunks[0].lines[1].kind, "removal");
        assert_eq!(parsed.hunks[0].lines[2].kind, "addition");
        assert_eq!(parsed.hunks[0].lines[3].kind, "marker");
        assert_eq!(parsed.hunks[1].lines[2].new_line, Some(11));
    }

    #[test]
    fn keeps_diff_content_that_looks_like_file_headers_inside_hunks() {
        let parsed = parse_diff(
            "@@ -1,2 +1,2 @@\n--- content line\n+++ content line\n",
            false,
        )
        .expect("valid unified diff");
        assert_eq!(parsed.hunks[0].lines[0].kind, "removal");
        assert_eq!(parsed.hunks[0].lines[1].kind, "addition");
    }

    #[test]
    fn parses_binary_rename_empty_and_crlf_diff_fixtures() {
        let binary = parse_diff("Binary files a/image.png and b/image.png differ\n", false)
            .expect("valid binary diff");
        assert!(binary.binary);
        assert!(binary.hunks.is_empty());

        let rename = parse_diff(
            "diff --git a/old.txt b/new.txt\nsimilarity index 100%\nrename from old.txt\nrename to new.txt\n",
            false,
        )
        .expect("valid rename diff");
        assert!(
            rename
                .metadata
                .iter()
                .any(|line| line == "rename from old.txt")
        );

        let empty = parse_diff("", false).expect("empty diff is valid");
        assert!(empty.hunks.is_empty());

        let crlf = parse_diff(
            "diff --git a/crlf.txt b/crlf.txt\n@@ -1 +1 @@\n-old\r\n+new\r\n",
            false,
        )
        .expect("valid CRLF diff");
        assert_eq!(crlf.hunks[0].lines[0].content, "-old\r");
        assert_eq!(crlf.hunks[0].lines[1].content, "+new\r");
    }

    #[test]
    fn reports_malformed_diff_hunk_headers() {
        assert!(parse_diff("@@ malformed @@\n+line\n", false).is_err());
    }

    #[test]
    fn parses_structured_branch_refs_with_upstream_state_and_special_subjects() {
        let fixture = concat!(
            "refs/heads/main\0main\0*\0origin/main\0[ahead 2, behind 1]\0a1b2c3\02026-10-14T12:00:00-03:00\0Fix parser: ação 🚀\n",
            "refs/heads/feature/login\0feature/login\0 \0origin/feature/login\0[gone]\0d4e5f6\02026-10-13T12:00:00-03:00\0Keep local feature\n",
            "refs/heads/local-only\0local-only\0 \0\0\0f7a8b9\02026-10-12T12:00:00-03:00\0Local only\n",
            "refs/remotes/origin/HEAD\0origin/HEAD\0 \0\0\0a1b2c3\02026-10-14T12:00:00-03:00\0Remote HEAD\n",
            "refs/remotes/origin/develop\0origin/develop\0 \0\0\0b2c3d4\02026-10-11T12:00:00-03:00\0Remote develop\n",
        );
        let (local, remote) = parse_branches(fixture).expect("valid for-each-ref fixture");
        assert_eq!(local.len(), 3);
        assert_eq!(local[0].name, "main");
        assert!(local[0].current);
        assert_eq!(local[0].upstream.as_deref(), Some("origin/main"));
        assert_eq!(local[0].ahead, Some(2));
        assert_eq!(local[0].behind, Some(1));
        assert_eq!(local[0].subject, "Fix parser: ação 🚀");
        assert_eq!(local[1].name, "feature/login");
        assert!(local[1].upstream_gone);
        assert_eq!(local[2].name, "local-only");
        assert_eq!(local[2].upstream, None);
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].name, "origin/develop");
    }

    #[test]
    fn chooses_fetch_remote_from_upstream_or_remote_count() {
        assert_eq!(
            choose_fetch_remote(
                Some("origin/develop"),
                &["origin".to_owned(), "backup".to_owned()]
            ),
            Some(vec![
                "fetch".to_owned(),
                "--prune".to_owned(),
                "origin".to_owned()
            ])
        );
        assert_eq!(
            choose_fetch_remote(None, &["origin".to_owned()]),
            Some(vec![
                "fetch".to_owned(),
                "--prune".to_owned(),
                "origin".to_owned()
            ])
        );
        assert_eq!(
            choose_fetch_remote(
                Some("team/origin/develop"),
                &["team".to_owned(), "team/origin".to_owned()]
            ),
            Some(vec![
                "fetch".to_owned(),
                "--prune".to_owned(),
                "team/origin".to_owned()
            ])
        );
        assert_eq!(
            choose_fetch_remote(None, &["origin".to_owned(), "backup".to_owned()]),
            Some(vec![
                "fetch".to_owned(),
                "--all".to_owned(),
                "--prune".to_owned()
            ])
        );
        assert_eq!(choose_fetch_remote(None, &[]), None);
    }

    #[test]
    fn masks_embedded_http_credentials_in_text() {
        assert_eq!(
            mask_credentials("fatal: https://alice:secret@host.invalid/repo"),
            "fatal: https://[redacted]@host.invalid/repo"
        );
        assert_eq!(
            mask_credentials("https://host.invalid/repo"),
            "https://host.invalid/repo"
        );
    }

    #[test]
    fn adds_batch_mode_to_the_default_and_respects_explicit_yes() {
        let configured = "C:\\OpenSSH\\ssh.exe".to_owned();
        assert_eq!(
            super::ssh_command_with_batch_mode(configured),
            "C:\\OpenSSH\\ssh.exe -o BatchMode=yes"
        );
        assert_eq!(
            super::ssh_command_with_batch_mode("ssh -o batchmode=yes".to_owned()),
            "ssh -o batchmode=yes"
        );
        assert_eq!(
            super::ssh_command_with_batch_mode("ssh -o BatchMode=no".to_owned()),
            "ssh -o BatchMode=yes"
        );
        assert_eq!(
            super::ssh_command_with_batch_mode("ssh".to_owned()),
            "ssh -o BatchMode=yes"
        );
    }

    #[test]
    fn fetch_pull_and_push_work_with_a_local_bare_remote() {
        let temporary = TemporaryRepository::new();
        let bare = temporary.path().join("remote.git");
        fs::create_dir(&bare).unwrap();
        run_git(
            &bare,
            &["init", "--bare", "--quiet", "--initial-branch=main"],
        );

        let local = temporary.path().join("local");
        fs::create_dir(&local).unwrap();
        setup_repository(&local);
        create_commit(local.to_str().unwrap(), "Initial", false).unwrap();
        run_git(&local, &["remote", "add", "origin", bare.to_str().unwrap()]);
        super::push(local.to_str().unwrap(), Some("origin"), false).unwrap();
        assert_eq!(
            super::load_repository(local.to_str().unwrap())
                .unwrap()
                .upstream
                .as_deref(),
            Some("origin/main")
        );

        let other = temporary.path().join("other");
        let git = super::find_git_executable().unwrap();
        let clone = crate::process::command(git)
            .arg("clone")
            .arg("--quiet")
            .arg(&bare)
            .arg(&other)
            .output()
            .unwrap();
        assert!(
            clone.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&clone.stderr)
        );
        run_git(&other, &["config", "user.name", "Git GUI Test"]);
        run_git(
            &other,
            &["config", "user.email", "git-gui-test@example.invalid"],
        );
        commit_file(&other, "first.txt", "fast-forward\n", "Remote fast-forward");
        run_git(&other, &["push"]);
        let cancellation = AtomicBool::new(false);
        super::fetch(local.to_str().unwrap(), &cancellation).unwrap();
        let behind = super::load_repository(local.to_str().unwrap()).unwrap();
        assert_eq!(behind.behind, Some(1));
        super::pull(local.to_str().unwrap(), None, &cancellation).unwrap();
        assert_eq!(
            super::load_repository(local.to_str().unwrap())
                .unwrap()
                .behind,
            Some(0)
        );

        commit_file(&local, "first.txt", "local divergent\n", "Local change");
        commit_file(&other, "first.txt", "remote divergent\n", "Remote change");
        run_git(&other, &["push"]);
        super::fetch(local.to_str().unwrap(), &cancellation).unwrap();
        let rejected = super::push(local.to_str().unwrap(), None, false)
            .expect_err("non-fast-forward push must be rejected");
        assert!(format!("{}\n{}", rejected.stderr, rejected.stdout).contains("non-fast-forward"));
        let conflicted = super::pull(local.to_str().unwrap(), Some("merge"), &cancellation)
            .expect_err("divergent conflicting pull must fail");
        assert_eq!(conflicted.operation, "Pull");
        let status = super::load_repository(local.to_str().unwrap()).unwrap();
        assert!(status.files.iter().any(|file| file.status == "conflicted"));

        let remotes = list_remote_details(local.to_str().unwrap()).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].fetch_url, bare.to_string_lossy());
        assert_eq!(remotes[0].push_url, bare.to_string_lossy());
    }

    #[test]
    fn push_without_upstream_requires_a_valid_remote_choice() {
        let temporary = TemporaryRepository::new();
        let repository = temporary.path().join("repository");
        let bare = temporary.path().join("remote.git");
        fs::create_dir(&repository).unwrap();
        fs::create_dir(&bare).unwrap();
        setup_repository(&repository);
        create_commit(repository.to_str().unwrap(), "Initial", false).unwrap();
        run_git(&bare, &["init", "--bare", "--quiet"]);
        run_git(
            &repository,
            &["remote", "add", "origin", bare.to_str().unwrap()],
        );
        super::push(repository.to_str().unwrap(), Some("origin"), false).unwrap();
        assert!(
            super::load_repository(repository.to_str().unwrap())
                .unwrap()
                .upstream
                .is_some()
        );
    }

    #[test]
    fn branch_operations_work_in_a_temporary_repository_without_remote_operations() {
        let temporary = TemporaryRepository::new();
        let repository = temporary.path().join("repository");
        fs::create_dir(&repository).unwrap();
        run_git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        let empty = list_branches(repository.to_str().unwrap()).expect("empty repository refs");
        assert!(empty.local.is_empty());
        assert!(empty.remote.is_empty());
        assert_eq!(empty.detached_head, None);

        setup_repository(&repository);
        create_commit(repository.to_str().unwrap(), "Initial commit", false).unwrap();
        let initial_oid = String::from_utf8(run_git(&repository, &["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_owned();
        let bare_remote = temporary.path().join("remote.git");
        fs::create_dir(&bare_remote).unwrap();
        run_git(&bare_remote, &["init", "--bare", "--quiet"]);
        run_git(
            &repository,
            &["remote", "add", "origin", bare_remote.to_str().unwrap()],
        );
        run_git(
            &repository,
            &[
                "update-ref",
                "refs/remotes/origin/remote-feature",
                &initial_oid,
            ],
        );
        super::switch_branch(repository.to_str().unwrap(), "origin/remote-feature", true).unwrap();
        assert_eq!(
            super::load_repository(repository.to_str().unwrap())
                .unwrap()
                .upstream
                .as_deref(),
            Some("origin/remote-feature")
        );
        super::switch_branch(repository.to_str().unwrap(), "main", false).unwrap();
        super::delete_branch(repository.to_str().unwrap(), "remote-feature", false).unwrap();

        super::create_branch(
            repository.to_str().unwrap(),
            "feature/no-switch",
            Some("main"),
            false,
        )
        .unwrap();
        assert_eq!(
            super::load_repository(repository.to_str().unwrap())
                .unwrap()
                .branch
                .as_deref(),
            Some("main")
        );
        super::switch_branch(repository.to_str().unwrap(), "feature/no-switch", false).unwrap();
        super::rename_branch(
            repository.to_str().unwrap(),
            "feature/no-switch",
            "feature/renamed",
        )
        .unwrap();
        assert_eq!(
            super::load_repository(repository.to_str().unwrap())
                .unwrap()
                .branch
                .as_deref(),
            Some("feature/renamed")
        );
        super::switch_branch(repository.to_str().unwrap(), "main", false).unwrap();
        super::delete_branch(repository.to_str().unwrap(), "feature/renamed", false).unwrap();
        super::create_branch(
            repository.to_str().unwrap(),
            "feature/rename-other",
            None,
            false,
        )
        .unwrap();
        super::rename_branch(
            repository.to_str().unwrap(),
            "feature/rename-other",
            "feature/renamed-other",
        )
        .unwrap();
        super::delete_branch(repository.to_str().unwrap(), "feature/renamed-other", false).unwrap();
        let branches = list_branches(repository.to_str().unwrap()).unwrap();
        assert_eq!(branches.local.len(), 1);
        assert_eq!(branches.local[0].name, "main");
        let current_delete = super::delete_branch(repository.to_str().unwrap(), "main", false)
            .expect_err("current branch cannot be deleted");
        assert!(
            current_delete
                .stderr
                .contains("Switch to another branch first")
        );

        super::create_branch(
            repository.to_str().unwrap(),
            "feature/unmerged",
            Some("main"),
            true,
        )
        .unwrap();
        fs::write(repository.join("exclusive.txt"), "unique commit\n").unwrap();
        run_git(&repository, &["add", "exclusive.txt"]);
        run_git(&repository, &["commit", "-m", "Unique feature commit"]);
        run_git(&repository, &["switch", "main"]);
        let unmerged =
            super::delete_branch(repository.to_str().unwrap(), "feature/unmerged", false)
                .expect_err("safe delete must refuse an unmerged branch");
        assert_eq!(
            classify_branch_failure(&unmerged.stderr),
            BranchFailureKind::NotFullyMerged
        );
        super::delete_branch(repository.to_str().unwrap(), "feature/unmerged", true).unwrap();

        run_git(&repository, &["checkout", "--detach", "HEAD"]);
        let detached = list_branches(repository.to_str().unwrap()).unwrap();
        assert_eq!(
            detached.detached_head.as_deref(),
            Some(
                String::from_utf8(run_git(&repository, &["rev-parse", "--short", "HEAD"]).stdout)
                    .unwrap()
                    .trim()
            )
        );
        run_git(&repository, &["switch", "main"]);

        let invalid = super::create_branch(repository.to_str().unwrap(), "bad name", None, false)
            .expect_err("invalid ref must be rejected before creation");
        assert_eq!(invalid.operation, "Validate branch name");
        assert_eq!(
            classify_branch_failure(&invalid.stderr),
            BranchFailureKind::InvalidName
        );
    }

    #[test]
    fn parses_nul_delimited_history_records_with_unicode_and_merge_parents() {
        let fixture = concat!(
            "1111111111111111111111111111111111111111\0",
            "1111111\0\0",
            "Jos\u{e9} 🚀\0",
            "jose@example.invalid\0",
            "2026-10-14T12:00:00+00:00\0",
            "Jos\u{e9} 🚀\0",
            "jose@example.invalid\0",
            "2026-10-14T12:00:00+00:00\0",
            "Root subject\0",
            "Root subject\n\nBody with acentuação e emoji 🌈\n\0",
            "2222222222222222222222222222222222222222\0",
            "2222222\0",
            "1111111111111111111111111111111111111111 3333333333333333333333333333333333333333\0",
            "Merge author <x>\0",
            "merge@example.invalid\0",
            "2026-10-14T13:00:00+00:00\0",
            "Committer\0",
            "committer@example.invalid\0",
            "2026-10-14T13:01:00+00:00\0",
            "Merge with feature\0",
            "Merge with feature\n\nTwo parents.\n\0",
        );
        let commits = parse_history_log(fixture).expect("valid history fixture");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents.len(), 0);
        assert!(commits[0].body.contains("acentuação e emoji 🌈"));
        assert_eq!(commits[0].author, "José 🚀");
        assert_eq!(commits[1].parents.len(), 2);
        assert_eq!(commits[1].subject, "Merge with feature");
    }

    #[test]
    fn parses_numstat_and_name_status_rename_and_binary_records() {
        let numstat = concat!(
            "4\t1\tdir/arquivo com acento-é.txt\0",
            "-\t-\timage.png\0",
            "7\t2\t\0old name.txt\0new name é.txt\0",
        );
        let parsed = parse_numstat(numstat).expect("valid numstat fixture");
        assert_eq!(
            parsed["dir/arquivo com acento-é.txt"],
            (Some(4), Some(1), false)
        );
        assert_eq!(parsed["image.png"], (None, None, true));
        assert_eq!(parsed["new name é.txt"], (Some(7), Some(2), false));

        let names = parse_name_status("M\0ordinary.txt\0R100\0old name.txt\0new name.txt\0")
            .expect("valid name-status fixture");
        assert_eq!(
            names[1],
            (
                "new name.txt".to_owned(),
                Some("old name.txt".to_owned()),
                "R".to_owned()
            )
        );
    }

    #[test]
    fn validates_history_revisions_and_merge_parent_selection() {
        assert!(validate_history_revision(Path::new("git"), "unused", "-option").is_err());
        let merge = super::HistoryCommit {
            hash: "2222".to_owned(),
            short_hash: "2222".to_owned(),
            parents: vec!["1111".to_owned(), "3333".to_owned()],
            author: String::new(),
            author_email: String::new(),
            author_date: String::new(),
            committer: String::new(),
            committer_email: String::new(),
            commit_date: String::new(),
            subject: String::new(),
            body: String::new(),
            refs: Vec::new(),
        };
        assert_eq!(
            history_comparison_args(&merge, None).expect("first parent selected"),
            vec![
                std::ffi::OsString::from("1111"),
                std::ffi::OsString::from("2222")
            ]
        );
        assert!(history_comparison_args(&merge, Some("4444")).is_err());
    }

    #[test]
    fn lists_commit_details_and_diff_from_a_temporary_merge_repository() {
        let repo = TemporaryRepository::new();
        setup_repository(repo.path());
        run_git(
            repo.path(),
            &["commit", "-m", "Root with acentuação 🌈", "-m", "Root body"],
        );
        let root_hash = String::from_utf8(run_git(repo.path(), &["rev-parse", "HEAD"]).stdout)
            .expect("UTF-8 hash")
            .trim()
            .to_owned();

        run_git(repo.path(), &["switch", "-c", "feature"]);
        run_git(repo.path(), &["mv", "first.txt", "café file.txt"]);
        run_git(repo.path(), &["commit", "-m", "Rename file"]);
        let rename_hash = String::from_utf8(run_git(repo.path(), &["rev-parse", "HEAD"]).stdout)
            .expect("UTF-8 hash")
            .trim()
            .to_owned();
        fs::write(repo.path().join("picture.bin"), [0, 1, 2, 255]).unwrap();
        run_git(repo.path(), &["add", "picture.bin"]);
        run_git(repo.path(), &["commit", "-m", "Add binary file"]);
        let binary_hash = String::from_utf8(run_git(repo.path(), &["rev-parse", "HEAD"]).stdout)
            .expect("UTF-8 hash")
            .trim()
            .to_owned();

        run_git(repo.path(), &["switch", "main"]);
        commit_file(repo.path(), "main.txt", "main\n", "Main change");
        run_git(
            repo.path(),
            &["merge", "--no-ff", "feature", "-m", "Merge feature"],
        );
        run_git(repo.path(), &["tag", "-a", "v1.0", "-m", "Version 1"]);
        run_git(
            repo.path(),
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        let page = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("history list");
        assert_eq!(page.commits.len(), 5);
        assert_eq!(
            page.refs_hash,
            history_fingerprint(repo.path().to_str().expect("repository path"))
                .expect("history fingerprint")
                .refs_hash
        );
        assert_eq!(page.commits[0].parents.len(), 2);
        assert!(page.commits.iter().any(|commit| commit.hash == root_hash));
        assert!(page.commits.iter().any(|commit| commit.hash == rename_hash));
        assert!(
            page.commits[0]
                .refs
                .iter()
                .any(|reference| reference.name == "HEAD")
        );
        assert!(
            page.commits[0]
                .refs
                .iter()
                .any(|reference| reference.name == "main" && reference.kind == "local")
        );
        assert!(
            page.commits[0]
                .refs
                .iter()
                .any(|reference| reference.name == "origin/main" && reference.kind == "remote")
        );
        assert!(
            page.commits[0]
                .refs
                .iter()
                .any(|reference| reference.name == "v1.0" && reference.kind == "tag")
        );
        let paged = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 2,
            },
        )
        .expect("first history page");
        assert_eq!(paged.commits.len(), 2);
        assert!(paged.has_more);
        let next_page = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 2,
                limit: 2,
            },
        )
        .expect("next history page");
        assert!(paged.commits.iter().all(|first| {
            next_page
                .commits
                .iter()
                .all(|second| first.hash != second.hash)
        }));
        let feature_history = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: Some("refs/heads/feature".to_owned()),
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("local branch history");
        assert!(
            feature_history
                .commits
                .iter()
                .any(|commit| commit.hash == rename_hash)
        );
        let all_history = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: true,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("all branches history");
        assert_eq!(all_history.commits.len(), 5);
        let message_search = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: Some("Rename".to_owned()),
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("message search");
        assert_eq!(message_search.commits.len(), 1);
        assert_eq!(message_search.commits[0].hash, rename_hash);
        let author_search = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: Some("Git GUI Test".to_owned()),
                search_by: "author".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("author search");
        assert_eq!(author_search.commits.len(), 5);
        let hash_search = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: Some(root_hash[..7].to_owned()),
                search_by: "hash".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("hash search");
        assert_eq!(hash_search.commits.len(), 1);
        assert_eq!(hash_search.commits[0].hash, root_hash);

        let merge = load_history_detail(
            repo.path().to_str().expect("repository path"),
            &page.commits[0].hash,
            None,
        )
        .expect("merge details");
        assert_eq!(merge.commit.parents.len(), 2);
        assert_eq!(
            merge.comparison_parent.as_deref(),
            Some(merge.commit.parents[0].as_str())
        );
        let second_parent = load_history_detail(
            repo.path().to_str().expect("repository path"),
            &merge.commit.hash,
            Some(&merge.commit.parents[1]),
        )
        .expect("explicit second parent details");
        assert_eq!(
            second_parent.comparison_parent.as_deref(),
            Some(merge.commit.parents[1].as_str())
        );

        let root = load_history_detail(
            repo.path().to_str().expect("repository path"),
            &root_hash,
            None,
        )
        .expect("root details");
        assert!(root.commit.parents.is_empty());
        assert!(
            root.files
                .iter()
                .any(|file| file.path == "first.txt" && file.status == "A")
        );
        let root_diff = load_history_diff(
            repo.path().to_str().expect("repository path"),
            &root_hash,
            None,
            Some("first.txt"),
            false,
        )
        .expect("root diff");
        assert!(!root_diff.hunks.is_empty());

        let rename = load_history_detail(
            repo.path().to_str().expect("repository path"),
            &rename_hash,
            None,
        )
        .expect("rename details");
        assert!(rename.files.iter().any(|file| {
            file.path == "café file.txt" && file.original_path.as_deref() == Some("first.txt")
        }));
        let diff = load_history_diff(
            repo.path().to_str().expect("repository path"),
            &rename_hash,
            None,
            Some("café file.txt"),
            false,
        )
        .expect("rename diff");
        assert!(!diff.hunks.is_empty());

        let file_history = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: Some("café file.txt".to_owned()),
                offset: 0,
                limit: 200,
            },
        )
        .expect("file history");
        assert!(
            file_history
                .commits
                .iter()
                .any(|commit| commit.hash == rename_hash)
        );

        let binary = load_history_detail(
            repo.path().to_str().expect("repository path"),
            &binary_hash,
            None,
        )
        .expect("binary details");
        assert!(
            binary
                .files
                .iter()
                .any(|file| file.path == "picture.bin" && file.binary)
        );
        let binary_diff = load_history_diff(
            repo.path().to_str().expect("repository path"),
            &binary_hash,
            None,
            Some("picture.bin"),
            false,
        )
        .expect("binary diff");
        assert!(binary_diff.binary);

        run_git(
            repo.path(),
            &["commit", "--allow-empty", "-m", "Empty commit"],
        );
        let empty_hash = String::from_utf8(run_git(repo.path(), &["rev-parse", "HEAD"]).stdout)
            .expect("UTF-8 hash")
            .trim()
            .to_owned();
        let empty = load_history_detail(
            repo.path().to_str().expect("repository path"),
            &empty_hash,
            None,
        )
        .expect("empty commit details");
        assert!(empty.files.is_empty());
        assert!(empty.shortstat.is_empty());

        let bare = TemporaryRepository::new();
        run_git(bare.path(), &["init", "--bare", "--quiet"]);
        run_git(
            repo.path(),
            &[
                "remote",
                "add",
                "origin",
                bare.path().to_str().expect("remote path"),
            ],
        );
        run_git(repo.path(), &["push", "--set-upstream", "origin", "main"]);
        commit_file(repo.path(), "unpushed.txt", "local\n", "Unpushed commit");
        let unpushed_hash = String::from_utf8(run_git(repo.path(), &["rev-parse", "HEAD"]).stdout)
            .expect("UTF-8 hash")
            .trim()
            .to_owned();
        assert!(
            list_unpushed_commits(repo.path().to_str().expect("repository path"))
                .expect("unpushed commit list")
                .contains(&unpushed_hash)
        );
    }

    #[test]
    fn returns_an_empty_history_for_an_unborn_repository() {
        let repo = TemporaryRepository::new();
        run_git(repo.path(), &["init", "--quiet", "--initial-branch=main"]);
        let page = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("empty repository history");
        assert!(page.commits.is_empty());
        assert!(!page.shallow);
        assert!(!page.detached);
    }

    #[test]
    fn marks_detached_head_and_validates_branch_revision() {
        let repo = TemporaryRepository::new();
        setup_repository(repo.path());
        run_git(repo.path(), &["commit", "-m", "Initial"]);
        let head = String::from_utf8(run_git(repo.path(), &["rev-parse", "HEAD"]).stdout)
            .expect("UTF-8 hash")
            .trim()
            .to_owned();
        run_git(repo.path(), &["switch", "--detach", &head]);
        let page = list_history(
            repo.path().to_str().expect("repository path"),
            &HistoryOptions {
                revision: None,
                all_refs: false,
                search: None,
                search_by: "message".to_owned(),
                path: None,
                offset: 0,
                limit: 200,
            },
        )
        .expect("detached history");
        assert!(page.detached);
        assert_eq!(page.head_hash.as_deref(), Some(head.as_str()));
    }
}
