use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::State;

use crate::git::{
    self, BranchList, CommandFailure, CommandResult, CommitContext, CommitSummary, DiffPreview,
    GitInstallation, HistoryDetail, HistoryFingerprint, HistoryPage, PushOutput, RemoteInfo,
    RepositoryStatus,
};
use crate::terminal::{TerminalManager, TerminalSessionInfo};

#[derive(Default)]
pub struct NetworkOperationState {
    active: Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>,
}

impl NetworkOperationState {
    fn start(&self, repository: &str, operation: &str) -> Result<Arc<AtomicBool>, CommandFailure> {
        let mut active = self.active.lock().map_err(|error| CommandFailure {
            ok: false,
            operation: operation.to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: format!("Could not lock network operation state: {error}"),
            stdout: String::new(),
        })?;
        if active.contains_key(repository) {
            return Err(CommandFailure {
                ok: false,
                operation: operation.to_owned(),
                args: vec!["-C".to_owned(), repository.to_owned()],
                exit_code: None,
                stderr: "Another network operation is already running for this repository."
                    .to_owned(),
                stdout: String::new(),
            });
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        active.insert(repository.to_owned(), Arc::clone(&cancellation));
        Ok(cancellation)
    }

    fn finish(&self, repository: &str, operation: &str) -> Result<(), CommandFailure> {
        let mut active = self.active.lock().map_err(|error| CommandFailure {
            ok: false,
            operation: operation.to_owned(),
            args: vec!["-C".to_owned(), repository.to_owned()],
            exit_code: None,
            stderr: format!("Could not unlock network operation state: {error}"),
            stdout: String::new(),
        })?;
        active.remove(repository);
        Ok(())
    }

    fn cancel(&self, repository: &str) -> Result<bool, CommandFailure> {
        let active = self.active.lock().map_err(|error| CommandFailure {
            ok: false,
            operation: "Cancel network operation".to_owned(),
            args: vec!["-C".to_owned(), repository.to_owned()],
            exit_code: None,
            stderr: format!("Could not lock network operation state: {error}"),
            stdout: String::new(),
        })?;
        if let Some(cancellation) = active.get(repository) {
            cancellation.store(true, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn detect_git() -> CommandResult<GitInstallation> {
    match tauri::async_runtime::spawn_blocking(git::detect_git).await {
        Ok(Ok(installation)) => CommandResult::success(installation),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Detect Git".to_owned(),
            args: vec!["git.exe".to_owned(), "--version".to_owned()],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn load_repository(path: String) -> CommandResult<RepositoryStatus> {
    match tauri::async_runtime::spawn_blocking(move || git::load_repository(&path)).await {
        Ok(Ok(status)) => CommandResult::success(status),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Load repository".to_owned(),
            args: vec![],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn change_files(
    repository: String,
    operation: String,
    paths: Vec<String>,
) -> CommandResult<()> {
    let label = operation_label(&operation);
    match tauri::async_runtime::spawn_blocking(move || {
        git::run_change_operation(&repository, &operation, &paths)
    })
    .await
    {
        Ok(Ok(())) => CommandResult::success(()),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: label,
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn load_diff(
    repository: String,
    path: String,
    staged: bool,
    untracked: bool,
    load_large: bool,
) -> CommandResult<DiffPreview> {
    match tauri::async_runtime::spawn_blocking(move || {
        git::load_diff(&repository, &path, staged, untracked, load_large)
    })
    .await
    {
        Ok(Ok(diff)) => CommandResult::success(diff),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Load diff".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn list_history(
    repository: String,
    options: git::HistoryOptions,
) -> CommandResult<HistoryPage> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || git::list_history(&repository, &options))
        .await
    {
        Ok(Ok(page)) => CommandResult::success(page),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "List history".to_owned(),
            args: vec!["-C".to_owned(), error_repository],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn list_unpushed_commits(repository: String) -> CommandResult<Vec<String>> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || git::list_unpushed_commits(&repository))
        .await
    {
        Ok(Ok(commits)) => CommandResult::success(commits),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "List unpushed commits".to_owned(),
            args: vec![
                "-C".to_owned(),
                error_repository,
                "rev-list".to_owned(),
                "@{upstream}..HEAD".to_owned(),
            ],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn history_fingerprint(repository: String) -> CommandResult<HistoryFingerprint> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || git::history_fingerprint(&repository)).await
    {
        Ok(Ok(fingerprint)) => CommandResult::success(fingerprint),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Check history changes".to_owned(),
            args: vec![
                "-C".to_owned(),
                error_repository,
                "rev-parse".to_owned(),
                "HEAD".to_owned(),
            ],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn load_history_detail(
    repository: String,
    hash: String,
    parent: Option<String>,
) -> CommandResult<HistoryDetail> {
    let error_repository = repository.clone();
    let error_hash = hash.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        git::load_history_detail(&repository, &hash, parent.as_deref())
    })
    .await
    {
        Ok(Ok(detail)) => CommandResult::success(detail),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Read commit details".to_owned(),
            args: vec![
                "-C".to_owned(),
                error_repository,
                "show".to_owned(),
                error_hash,
            ],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn load_history_diff(
    repository: String,
    hash: String,
    parent: Option<String>,
    path: Option<String>,
    load_large: bool,
) -> CommandResult<DiffPreview> {
    let error_repository = repository.clone();
    let error_hash = hash.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        git::load_history_diff(
            &repository,
            &hash,
            parent.as_deref(),
            path.as_deref(),
            load_large,
        )
    })
    .await
    {
        Ok(Ok(diff)) => CommandResult::success(diff),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Load commit diff".to_owned(),
            args: vec![
                "-C".to_owned(),
                error_repository,
                "show".to_owned(),
                error_hash,
            ],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn load_commit_context(repository: String) -> CommandResult<CommitContext> {
    match tauri::async_runtime::spawn_blocking(move || git::load_commit_context(&repository)).await
    {
        Ok(Ok(context)) => CommandResult::success(context),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Load commit context".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn list_remotes(repository: String) -> CommandResult<Vec<String>> {
    match tauri::async_runtime::spawn_blocking(move || git::list_remotes(&repository)).await {
        Ok(Ok(remotes)) => CommandResult::success(remotes),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "List remotes".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn list_remote_details(repository: String) -> CommandResult<Vec<RemoteInfo>> {
    match tauri::async_runtime::spawn_blocking(move || git::list_remote_details(&repository)).await
    {
        Ok(Ok(remotes)) => CommandResult::success(remotes),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "List remotes".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn list_branches(repository: String) -> CommandResult<BranchList> {
    match tauri::async_runtime::spawn_blocking(move || git::list_branches(&repository)).await {
        Ok(Ok(branches)) => CommandResult::success(branches),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "List branches".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn create_branch(
    repository: String,
    name: String,
    start_point: Option<String>,
    switch_to: bool,
) -> CommandResult<()> {
    match tauri::async_runtime::spawn_blocking(move || {
        git::create_branch(&repository, &name, start_point.as_deref(), switch_to)
    })
    .await
    {
        Ok(Ok(())) => CommandResult::success(()),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Create branch".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn switch_branch(
    repository: String,
    name: String,
    track_remote: bool,
) -> CommandResult<()> {
    match tauri::async_runtime::spawn_blocking(move || {
        git::switch_branch(&repository, &name, track_remote)
    })
    .await
    {
        Ok(Ok(())) => CommandResult::success(()),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Switch branch".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn rename_branch(
    repository: String,
    old_name: String,
    new_name: String,
) -> CommandResult<()> {
    match tauri::async_runtime::spawn_blocking(move || {
        git::rename_branch(&repository, &old_name, &new_name)
    })
    .await
    {
        Ok(Ok(())) => CommandResult::success(()),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Rename branch".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn delete_branch(repository: String, name: String, force: bool) -> CommandResult<()> {
    match tauri::async_runtime::spawn_blocking(move || {
        git::delete_branch(&repository, &name, force)
    })
    .await
    {
        Ok(Ok(())) => CommandResult::success(()),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Delete branch".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn create_commit(
    repository: String,
    message: String,
    amend: bool,
) -> CommandResult<CommitSummary> {
    match tauri::async_runtime::spawn_blocking(move || {
        git::create_commit(&repository, &message, amend)
    })
    .await
    {
        Ok(Ok(summary)) => CommandResult::success(summary),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: if amend { "Amend commit" } else { "Commit" }.to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn repository_operation_state(
    repository: String,
) -> CommandResult<git::RepositoryOperationState> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || git::repository_operation_state(&repository))
        .await
    {
        Ok(Ok(state)) => CommandResult::success(state),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Read repository operation state".to_owned(),
            args: vec!["-C".to_owned(), error_repository],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn advanced_preview(
    repository: String,
    operation: String,
    target: String,
) -> CommandResult<git::AdvancedPreview> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        git::advanced_preview(&repository, &operation, &target)
    })
    .await
    {
        Ok(Ok(preview)) => CommandResult::success(preview),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Preview advanced Git operation".to_owned(),
            args: vec!["-C".to_owned(), error_repository],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn run_advanced_operation(
    repository: String,
    request: git::AdvancedRequest,
) -> CommandResult<git::AdvancedOutput> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        git::run_advanced_operation(&repository, &request)
    })
    .await
    {
        Ok(Ok(output)) => CommandResult::success(output),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Run advanced Git operation".to_owned(),
            args: vec!["-C".to_owned(), error_repository],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn run_operation_action(
    repository: String,
    request: git::OperationActionRequest,
) -> CommandResult<git::AdvancedOutput> {
    let error_repository = repository.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        git::run_operation_action(&repository, &request)
    })
    .await
    {
        Ok(Ok(output)) => CommandResult::success(output),
        Ok(Err(failure)) => CommandResult::failure(failure),
        Err(error) => CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Continue repository operation".to_owned(),
            args: vec!["-C".to_owned(), error_repository],
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        }),
    }
}

#[tauri::command]
pub async fn push(
    repository: String,
    remote: Option<String>,
    force_with_lease: bool,
    operations: State<'_, NetworkOperationState>,
) -> Result<CommandResult<PushOutput>, String> {
    let cancellation = match operations.start(&repository, "Push") {
        Ok(token) => token,
        Err(failure) => return Ok(CommandResult::failure(failure)),
    };
    let operation_repository = repository.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        git::push_cancellable(
            &repository,
            remote.as_deref(),
            force_with_lease,
            &cancellation,
        )
    })
    .await;
    if let Err(failure) = operations.finish(&operation_repository, "Push") {
        return Ok(CommandResult::failure(failure));
    }
    match result {
        Ok(Ok(output)) => Ok(CommandResult::success(output)),
        Ok(Err(failure)) => Ok(CommandResult::failure(failure)),
        Err(error) => Ok(CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Push".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        })),
    }
}

#[tauri::command]
pub async fn fetch(
    repository: String,
    operations: State<'_, NetworkOperationState>,
) -> Result<CommandResult<PushOutput>, String> {
    network_command(repository, "Fetch", operations, git::fetch).await
}

#[tauri::command]
pub async fn pull(
    repository: String,
    strategy: Option<String>,
    operations: State<'_, NetworkOperationState>,
) -> Result<CommandResult<PushOutput>, String> {
    let cancellation = match operations.start(&repository, "Pull") {
        Ok(token) => token,
        Err(failure) => return Ok(CommandResult::failure(failure)),
    };
    let operation_repository = repository.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        git::pull(&repository, strategy.as_deref(), &cancellation)
    })
    .await;
    if let Err(failure) = operations.finish(&operation_repository, "Pull") {
        return Ok(CommandResult::failure(failure));
    }
    match result {
        Ok(Ok(output)) => Ok(CommandResult::success(output)),
        Ok(Err(failure)) => Ok(CommandResult::failure(failure)),
        Err(error) => Ok(CommandResult::failure(CommandFailure {
            ok: false,
            operation: "Pull".to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        })),
    }
}

async fn network_command(
    repository: String,
    operation: &'static str,
    operations: State<'_, NetworkOperationState>,
    command: fn(&str, &AtomicBool) -> Result<PushOutput, CommandFailure>,
) -> Result<CommandResult<PushOutput>, String> {
    let cancellation = match operations.start(&repository, operation) {
        Ok(token) => token,
        Err(failure) => return Ok(CommandResult::failure(failure)),
    };
    let operation_repository = repository.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || command(&repository, &cancellation)).await;
    if let Err(failure) = operations.finish(&operation_repository, operation) {
        return Ok(CommandResult::failure(failure));
    }
    match result {
        Ok(Ok(output)) => Ok(CommandResult::success(output)),
        Ok(Err(failure)) => Ok(CommandResult::failure(failure)),
        Err(error) => Ok(CommandResult::failure(CommandFailure {
            ok: false,
            operation: operation.to_owned(),
            args: Vec::new(),
            exit_code: None,
            stderr: error.to_string(),
            stdout: String::new(),
        })),
    }
}

#[tauri::command]
pub fn cancel_network_operation(
    repository: String,
    operations: State<'_, NetworkOperationState>,
) -> CommandResult<bool> {
    match operations.cancel(&repository) {
        Ok(cancelled) => CommandResult::success(cancelled),
        Err(failure) => CommandResult::failure(failure),
    }
}

fn operation_label(operation: &str) -> String {
    match operation {
        "Stage"
        | "Stage all"
        | "Unstage"
        | "Unstage all"
        | "Discard"
        | "Discard untracked"
        | "Discard all tracked"
        | "Discard all untracked" => operation.to_owned(),
        _ => "Working-tree operation".to_owned(),
    }
}

#[tauri::command]
fn terminal_failure(operation: &str, args: Vec<String>, stderr: String) -> CommandFailure {
    CommandFailure {
        ok: false,
        operation: operation.to_owned(),
        args,
        exit_code: None,
        stderr,
        stdout: String::new(),
    }
}

#[tauri::command]
pub fn terminal_create(
    app: tauri::AppHandle,
    manager: State<'_, TerminalManager>,
    cwd: String,
    shell: Option<String>,
    cols: u16,
    rows: u16,
) -> CommandResult<TerminalSessionInfo> {
    match manager.create(app, cwd.clone(), shell.clone(), cols, rows) {
        Ok(session) => CommandResult::success(session),
        Err(error) => CommandResult::failure(terminal_failure(
            "Create terminal",
            vec!["terminal_create".to_owned(), cwd, shell.unwrap_or_default()],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_write(
    manager: State<'_, TerminalManager>,
    id: String,
    data: String,
) -> CommandResult<()> {
    match manager.write(&id, &data) {
        Ok(()) => CommandResult::success(()),
        Err(error) => CommandResult::failure(terminal_failure(
            "Write to terminal",
            vec!["terminal_write".to_owned(), id],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_write_bytes(
    manager: State<'_, TerminalManager>,
    id: String,
    data: Vec<u8>,
) -> CommandResult<()> {
    match manager.write_bytes(&id, &data) {
        Ok(()) => CommandResult::success(()),
        Err(error) => CommandResult::failure(terminal_failure(
            "Write binary data to terminal",
            vec!["terminal_write_bytes".to_owned(), id],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_resize(
    manager: State<'_, TerminalManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> CommandResult<()> {
    match manager.resize(&id, cols, rows) {
        Ok(()) => CommandResult::success(()),
        Err(error) => CommandResult::failure(terminal_failure(
            "Resize terminal",
            vec!["terminal_resize".to_owned(), id],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_close(manager: State<'_, TerminalManager>, id: String) -> CommandResult<()> {
    match manager.close(&id) {
        Ok(()) => CommandResult::success(()),
        Err(error) => CommandResult::failure(terminal_failure(
            "Close terminal",
            vec!["terminal_close".to_owned(), id],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_close_all(manager: State<'_, TerminalManager>) -> CommandResult<()> {
    match manager.close_all() {
        Ok(()) => CommandResult::success(()),
        Err(error) => CommandResult::failure(terminal_failure(
            "Close terminal sessions",
            vec!["terminal_close_all".to_owned()],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_list(
    manager: State<'_, TerminalManager>,
) -> CommandResult<Vec<TerminalSessionInfo>> {
    match manager.list() {
        Ok(sessions) => CommandResult::success(sessions),
        Err(error) => CommandResult::failure(terminal_failure(
            "List terminals",
            vec!["terminal_list".to_owned()],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_history(manager: State<'_, TerminalManager>, id: String) -> CommandResult<String> {
    match manager.history(&id) {
        Ok(history) => CommandResult::success(history),
        Err(error) => CommandResult::failure(terminal_failure(
            "Read terminal buffer",
            vec!["terminal_history".to_owned(), id],
            error,
        )),
    }
}

#[tauri::command]
pub fn terminal_clear_history(
    manager: State<'_, TerminalManager>,
    id: String,
) -> CommandResult<()> {
    match manager.clear_history(&id) {
        Ok(()) => CommandResult::success(()),
        Err(error) => CommandResult::failure(terminal_failure(
            "Clear terminal buffer",
            vec!["terminal_clear_history".to_owned(), id],
            error,
        )),
    }
}
