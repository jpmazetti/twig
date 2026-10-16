use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
const MAX_SESSIONS: usize = 8;
const OUTPUT_BATCH_WINDOW: Duration = Duration::from_millis(12);
const OUTPUT_BATCH_LIMIT: usize = 32 * 1024;
const SESSION_HISTORY_LIMIT: usize = 1_000_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSessionInfo {
    pub id: String,
    pub shell: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalOutput {
    session_id: String,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalExit {
    session_id: String,
    exit_code: Option<i32>,
}

struct PtySession {
    info: Mutex<TerminalSessionInfo>,
    history: Mutex<String>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
}

#[derive(Default)]
pub struct TerminalManager {
    sessions: Mutex<HashMap<String, Arc<PtySession>>>,
}

impl TerminalManager {
    pub fn create(
        &self,
        app: AppHandle,
        cwd: String,
        preferred_shell: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSessionInfo, String> {
        let cwd_path = PathBuf::from(&cwd);
        if !cwd_path.is_dir() {
            return Err(format!("Terminal working directory does not exist: {cwd}"));
        }

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|error| format!("Could not lock terminal sessions: {error}"))?;
        if sessions.len() >= MAX_SESSIONS {
            return Err(format!(
                "At most {MAX_SESSIONS} terminal sessions can be open."
            ));
        }

        let candidates = shell_candidates(preferred_shell.as_deref())?;
        let mut last_error = None;
        let mut started = None;
        for (index, candidate) in candidates.iter().enumerate() {
            match spawn_shell(candidate, &cwd_path, cols, rows) {
                Ok(parts) => {
                    started = Some((candidate, index, parts));
                    break;
                }
                Err(error) => last_error = Some(format!("{}: {error}", candidate.label)),
            }
        }
        let (shell, candidate_index, (master, child, writer, reader)) =
            started.ok_or_else(|| {
                format!(
                    "Could not start a terminal shell. Tried {}. {}",
                    candidates
                        .iter()
                        .map(|candidate| candidate.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    last_error.unwrap_or_else(|| "No supported shell was found.".to_owned())
                )
            })?;

        let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed).to_string();
        let notice = if candidate_index > 0 {
            Some(format!(
                "{} was unavailable; using {} instead.",
                candidates[0].label, shell.label
            ))
        } else {
            None
        };
        let info = TerminalSessionInfo {
            id: id.clone(),
            shell: shell.label.clone(),
            cwd,
            cols,
            rows,
            running: true,
            exit_code: None,
            notice,
        };
        let session = Arc::new(PtySession {
            info: Mutex::new(info.clone()),
            history: Mutex::new(String::new()),
            master: Mutex::new(master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
        });
        sessions.insert(id.clone(), Arc::clone(&session));
        drop(sessions);

        spawn_output_threads(app.clone(), id.clone(), reader, Arc::clone(&session));
        spawn_exit_monitor(app, id, session);
        Ok(info)
    }

    pub fn write(&self, id: &str, data: &str) -> Result<(), String> {
        self.write_bytes(id, data.as_bytes())
    }

    pub fn write_bytes(&self, id: &str, data: &[u8]) -> Result<(), String> {
        let session = self.session(id)?;
        let mut writer = session
            .writer
            .lock()
            .map_err(|error| format!("Could not lock terminal input: {error}"))?;
        writer
            .write_all(data)
            .map_err(|error| format!("Could not write to terminal session {id}: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("Could not flush terminal input for session {id}: {error}"))
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let session = self.session(id)?;
        let master = session
            .master
            .lock()
            .map_err(|error| format!("Could not lock terminal PTY: {error}"))?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Could not resize terminal session {id}: {error}"))?;
        let mut info = session
            .info
            .lock()
            .map_err(|error| format!("Could not lock terminal metadata: {error}"))?;
        info.cols = cols;
        info.rows = rows;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<TerminalSessionInfo>, String> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|error| format!("Could not lock terminal sessions: {error}"))?;
        sessions
            .values()
            .map(|session| {
                session
                    .info
                    .lock()
                    .map(|info| info.clone())
                    .map_err(|error| format!("Could not read terminal metadata: {error}"))
            })
            .collect()
    }

    pub fn history(&self, id: &str) -> Result<String, String> {
        let session = self.session(id)?;
        session
            .history
            .lock()
            .map(|history| history.clone())
            .map_err(|error| format!("Could not read terminal history: {error}"))
    }

    pub fn clear_history(&self, id: &str) -> Result<(), String> {
        let session = self.session(id)?;
        session
            .history
            .lock()
            .map_err(|error| format!("Could not lock terminal history: {error}"))?
            .clear();
        Ok(())
    }

    pub fn close(&self, id: &str) -> Result<(), String> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|error| format!("Could not lock terminal sessions: {error}"))?;
            sessions
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Terminal session {id} was not found."))?
        };
        close_process_tree(&session, id)?;
        self.sessions
            .lock()
            .map_err(|error| format!("Could not lock terminal sessions: {error}"))?
            .remove(id);
        Ok(())
    }

    pub fn close_all(&self) -> Result<(), String> {
        let sessions = match self.sessions.lock() {
            Ok(sessions) => sessions
                .iter()
                .map(|(id, session)| (id.clone(), Arc::clone(session)))
                .collect::<Vec<_>>(),
            Err(error) => return Err(format!("Could not lock terminal sessions: {error}")),
        };
        let mut failures = Vec::new();
        for (id, session) in sessions {
            match close_process_tree(&session, &id) {
                Ok(()) => {
                    if let Ok(mut sessions) = self.sessions.lock() {
                        sessions.remove(&id);
                    }
                }
                Err(error) => failures.push(error),
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    }

    fn session(&self, id: &str) -> Result<Arc<PtySession>, String> {
        self.sessions
            .lock()
            .map_err(|error| format!("Could not lock terminal sessions: {error}"))?
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Terminal session {id} was not found."))
    }
}

struct ShellCandidate {
    key: &'static str,
    label: String,
    program: OsString,
    args: Vec<OsString>,
}

use std::ffi::OsString;

fn shell_candidates(preferred: Option<&str>) -> Result<Vec<ShellCandidate>, String> {
    let candidates = available_shells();
    if let Some(preferred) = preferred {
        let Some(selected) = candidates
            .iter()
            .find(|candidate| candidate.key == preferred)
        else {
            return Err(format!("Unsupported terminal shell selection: {preferred}"));
        };
        let mut ordered = vec![clone_candidate(selected)];
        ordered.extend(
            candidates
                .iter()
                .filter(|candidate| candidate.key != preferred)
                .map(clone_candidate),
        );
        Ok(ordered)
    } else {
        Ok(candidates)
    }
}

fn clone_candidate(candidate: &ShellCandidate) -> ShellCandidate {
    ShellCandidate {
        key: candidate.key,
        label: candidate.label.clone(),
        program: candidate.program.clone(),
        args: candidate.args.clone(),
    }
}

fn available_shells() -> Vec<ShellCandidate> {
    #[cfg(windows)]
    {
        let mut candidates = vec![
            ShellCandidate {
                key: "pwsh",
                label: "PowerShell 7".to_owned(),
                program: OsString::from("pwsh.exe"),
                args: vec![OsString::from("-NoLogo")],
            },
            ShellCandidate {
                key: "powershell",
                label: "Windows PowerShell".to_owned(),
                program: OsString::from("powershell.exe"),
                args: vec![OsString::from("-NoLogo")],
            },
        ];
        candidates.push(ShellCandidate {
            key: "git-bash",
            label: "Git Bash".to_owned(),
            program: find_git_bash()
                .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"))
                .into_os_string(),
            args: vec![OsString::from("--login"), OsString::from("-i")],
        });
        candidates.push(ShellCandidate {
            key: "cmd",
            label: "Command Prompt".to_owned(),
            program: OsString::from("cmd.exe"),
            args: Vec::new(),
        });
        candidates
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("sh"));
        vec![ShellCandidate {
            key: "shell",
            label: shell.to_string_lossy().into_owned(),
            program: shell,
            args: Vec::new(),
        }]
    }
}

#[cfg(windows)]
fn find_git_bash() -> Option<PathBuf> {
    let mut paths = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        paths.push(PathBuf::from(program_files).join("Git\\bin\\bash.exe"));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        paths.push(PathBuf::from(program_files_x86).join("Git\\bin\\bash.exe"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&path) {
            let git = entry.join("git.exe");
            if git.is_file()
                && let Some(root) = git.parent().and_then(Path::parent)
            {
                paths.push(root.join("bin\\bash.exe"));
            }
        }
    }
    paths.into_iter().find(|path| path.is_file())
}

type SpawnedShell = (
    Box<dyn MasterPty + Send>,
    Box<dyn Child + Send + Sync>,
    Box<dyn Write + Send>,
    Box<dyn Read + Send>,
);

fn spawn_shell(
    shell: &ShellCandidate,
    cwd: &Path,
    cols: u16,
    rows: u16,
) -> Result<SpawnedShell, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())?;
    let mut command = CommandBuilder::new(shell.program.clone());
    command.args(shell.args.iter());
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| error.to_string())?;
    drop(pair.slave);
    Ok((pair.master, child, writer, reader))
}

fn spawn_output_threads(
    app: AppHandle,
    session_id: String,
    mut reader: Box<dyn Read + Send>,
    session: Arc<PtySession>,
) {
    let (sender, receiver) = mpsc::sync_channel(64);
    let reader_session = session_id.clone();
    let reader_app = app.clone();
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(length) => {
                    if sender.send(buffer[..length].to_vec()).is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let _ = reader_app.emit(
                        "terminal-reader-error",
                        TerminalExit {
                            session_id: reader_session,
                            exit_code: None,
                        },
                    );
                    break;
                }
            }
        }
    });
    thread::spawn(move || forward_batched_output(app, session_id, receiver, session));
}

fn forward_batched_output(
    app: AppHandle,
    session_id: String,
    receiver: Receiver<Vec<u8>>,
    session: Arc<PtySession>,
) {
    let mut decoder = Utf8ChunkDecoder::default();
    while let Ok(first) = receiver.recv() {
        let batch = collect_output_batch(&receiver, first);
        let text = decoder.decode(&batch);
        if let Ok(mut history) = session.history.lock() {
            history.push_str(&text);
            trim_history(&mut history);
        }
        if !text.is_empty()
            && app
                .emit(
                    "terminal-output",
                    TerminalOutput {
                        session_id: session_id.clone(),
                        data: text,
                    },
                )
                .is_err()
        {
            break;
        }
    }
    let tail = decoder.finish();
    if !tail.is_empty() {
        if let Ok(mut history) = session.history.lock() {
            history.push_str(&tail);
            trim_history(&mut history);
        }
        let _ = app.emit(
            "terminal-output",
            TerminalOutput {
                session_id,
                data: tail,
            },
        );
    }

    fn trim_history(history: &mut String) {
        if history.len() <= SESSION_HISTORY_LIMIT {
            return;
        }
        let mut start = history.len() - SESSION_HISTORY_LIMIT;
        while !history.is_char_boundary(start) {
            start += 1;
        }
        history.drain(..start);
    }
}

fn collect_output_batch(receiver: &Receiver<Vec<u8>>, mut batch: Vec<u8>) -> Vec<u8> {
    let deadline = Instant::now() + OUTPUT_BATCH_WINDOW;
    while batch.len() < OUTPUT_BATCH_LIMIT {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(next) => batch.extend(next),
            Err(_) => break,
        }
    }
    batch
}

#[derive(Default)]
struct Utf8ChunkDecoder {
    pending: Vec<u8>,
}

impl Utf8ChunkDecoder {
    fn decode(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        match std::str::from_utf8(&self.pending) {
            Ok(text) => {
                let decoded = text.to_owned();
                self.pending.clear();
                decoded
            }
            Err(error) => {
                let valid_end = error.valid_up_to();
                let mut decoded = String::from_utf8_lossy(&self.pending[..valid_end]).into_owned();
                if error.error_len().is_none() {
                    let remainder = self.pending[valid_end..].to_vec();
                    self.pending = remainder;
                } else {
                    decoded.push_str(&String::from_utf8_lossy(&self.pending[valid_end..]));
                    self.pending.clear();
                }
                decoded
            }
        }
    }

    fn finish(&mut self) -> String {
        let decoded = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        decoded
    }
}

fn spawn_exit_monitor(app: AppHandle, session_id: String, session: Arc<PtySession>) {
    thread::spawn(move || {
        loop {
            let status = {
                let mut child = match session.child.lock() {
                    Ok(child) => child,
                    Err(_) => return,
                };
                child.try_wait()
            };
            match status {
                Ok(Some(exit)) => {
                    let exit_code = i32::try_from(exit.exit_code()).ok();
                    if let Ok(mut info) = session.info.lock() {
                        info.running = false;
                        info.exit_code = exit_code;
                    }
                    let _ = app.emit(
                        "terminal-exit",
                        TerminalExit {
                            session_id,
                            exit_code,
                        },
                    );
                    break;
                }
                Ok(None) => thread::sleep(Duration::from_millis(200)),
                Err(_) => thread::sleep(Duration::from_millis(500)),
            }
        }
    });
}

fn close_process_tree(session: &PtySession, id: &str) -> Result<(), String> {
    let mut child = session
        .child
        .lock()
        .map_err(|error| format!("Could not lock terminal child process: {error}"))?;
    if child
        .try_wait()
        .map_err(|error| format!("Could not inspect terminal session {id}: {error}"))?
        .is_some()
    {
        return Ok(());
    }
    let process_id = child.process_id();
    #[cfg(windows)]
    if let Some(process_id) = process_id {
        let result = crate::process::command("taskkill.exe")
            .args(["/PID", &process_id.to_string(), "/T", "/F"])
            .output()
            .map_err(|error| format!("Could not terminate terminal process tree {id}: {error}"));
        let tree_error = match result {
            Ok(result) if result.status.success() => None,
            Ok(result) => Some(format!(
                "taskkill.exe failed for terminal session {id}: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            )),
            Err(error) => Some(error),
        };
        if let Some(error) = tree_error {
            if child.try_wait().ok().flatten().is_some() {
                return Ok(());
            }
            let child_error = child
                .kill()
                .err()
                .map(|kill_error| format!(" Shell termination also failed: {kill_error}"))
                .unwrap_or_default();
            return Err(format!("{error}{child_error}"));
        }
        child
            .wait()
            .map_err(|error| format!("Could not reap terminal session {id}: {error}"))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    let _ = process_id;
    child
        .kill()
        .map_err(|error| format!("Could not terminate terminal session {id}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        PtySize, ShellCandidate, Utf8ChunkDecoder, available_shells, collect_output_batch,
        shell_candidates, spawn_shell,
    };
    use std::ffi::OsString;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::sync::{Arc, Mutex};

    #[test]
    fn utf8_sequences_split_across_chunks_are_preserved() {
        let mut decoder = Utf8ChunkDecoder::default();
        assert_eq!(decoder.decode(&[0xF0, 0x9F]), "");
        assert_eq!(decoder.decode(&[0x8C, 0x8D]), "🌍");
        assert_eq!(decoder.decode("ação".as_bytes()), "ação");
        assert_eq!(decoder.finish(), "");
    }

    #[test]
    fn incomplete_utf8_is_replaced_only_when_stream_finishes() {
        let mut decoder = Utf8ChunkDecoder::default();
        assert_eq!(decoder.decode(&[0xE2, 0x82]), "");
        assert_eq!(decoder.finish(), "�");
    }

    #[test]
    fn output_chunks_are_batched_within_the_short_window() {
        let (sender, receiver) = mpsc::channel();
        sender.send(b"first".to_vec()).expect("send first chunk");
        sender.send(b"second".to_vec()).expect("send second chunk");
        let first = receiver.recv().expect("receive first chunk");
        assert_eq!(collect_output_batch(&receiver, first), b"firstsecond");
    }

    #[test]
    fn preferred_shell_is_tried_before_fallbacks() {
        let candidates = shell_candidates(Some("cmd")).expect("resolve supported shell");
        assert_eq!(
            candidates.first().map(|candidate| candidate.key),
            Some("cmd")
        );
        assert!(shell_candidates(Some("not-a-shell")).is_err());
    }

    #[test]
    fn supported_shell_candidates_have_a_stable_default_order() {
        let candidates = available_shells();
        #[cfg(windows)]
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.key)
                .collect::<Vec<_>>(),
            vec!["pwsh", "powershell", "git-bash", "cmd"],
        );
        #[cfg(not(windows))]
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn real_pty_accepts_input_and_resizes() {
        #[cfg(windows)]
        let shell = ShellCandidate {
            key: "cmd",
            label: "Command Prompt".to_owned(),
            program: OsString::from("cmd.exe"),
            args: Vec::new(),
        };
        #[cfg(not(windows))]
        let shell = ShellCandidate {
            key: "shell",
            label: "sh".to_owned(),
            program: OsString::from("sh"),
            args: Vec::new(),
        };
        let cwd = std::env::current_dir().expect("read current directory");
        let (master, child, writer, reader) =
            spawn_shell(&shell, &cwd, 80, 24).expect("spawn test shell in PTY");
        master
            .resize(PtySize {
                rows: 35,
                cols: 110,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize test PTY");
        assert_eq!(master.get_size().expect("read PTY size").cols, 110);
        let session = Arc::new(super::PtySession {
            info: Mutex::new(super::TerminalSessionInfo {
                id: "pty-integration".to_owned(),
                shell: shell.label.clone(),
                cwd: cwd.to_string_lossy().into_owned(),
                cols: 110,
                rows: 35,
                running: true,
                exit_code: None,
                notice: None,
            }),
            history: Mutex::new(String::new()),
            master: Mutex::new(master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
        });
        let manager = super::TerminalManager::default();
        manager
            .sessions
            .lock()
            .expect("lock session map")
            .insert("pty-integration".to_owned(), Arc::clone(&session));

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = reader;
            let mut buffer = [0_u8; 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(length) => {
                        if sender.send(buffer[..length].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(error.to_string().into_bytes());
                        break;
                    }
                }
            }
        });
        #[cfg(not(windows))]
        manager
            .write("pty-integration", "printf 'PTY_MILESTONE8_MARKER\\n'\n")
            .expect("write test command");
        let mut output = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut timed_out = false;
        #[cfg(windows)]
        let mut command_sent = false;
        while !String::from_utf8_lossy(&output).contains("PTY_MILESTONE8_MARKER") {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match receiver.recv_timeout(remaining) {
                Ok(chunk) => output.extend(chunk),
                Err(_) => {
                    timed_out = true;
                    break;
                }
            }
            #[cfg(windows)]
            if !command_sent && String::from_utf8_lossy(&output).contains("\u{1b}[6n") {
                session
                    .writer
                    .lock()
                    .expect("lock test writer")
                    .write_all(b"\x1b[1;1R")
                    .expect("answer terminal cursor query");
                manager
                    .write("pty-integration", "echo PTY_MILESTONE8_MARKER\r\n")
                    .expect("write test command");
                command_sent = true;
            }
        }
        let output = String::from_utf8_lossy(&output);
        manager.close("pty-integration").expect("close and kill PTY process tree");
        assert!(!timed_out, "timed out waiting for PTY output: {output:?}");
        assert!(output.contains("PTY_MILESTONE8_MARKER"), "{output:?}");
    }
}
