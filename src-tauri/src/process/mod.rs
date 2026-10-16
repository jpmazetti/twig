use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

#[derive(Debug)]
pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug)]
pub struct ProcessError {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

pub fn run(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
) -> Result<ProcessOutput, ProcessError> {
    run_with_timeout(program, args, cwd, Duration::from_secs(30))
}

pub fn run_with_timeout(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<ProcessOutput, ProcessError> {
    run_with_timeout_limit(program, args, cwd, timeout, None).map(|(output, _)| output)
}

pub fn run_with_env(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    environment: &[(&str, &str)],
) -> Result<ProcessOutput, ProcessError> {
    run_with_timeout_limit_and_input(
        program,
        args,
        cwd,
        Duration::from_secs(30),
        None,
        None,
        environment,
    )
    .map(|(output, _)| output)
}

pub fn run_network_operation(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    timeout: Duration,
    cancellation: Option<&AtomicBool>,
    ssh_command: Option<&str>,
) -> Result<ProcessOutput, ProcessError> {
    // Interactive commands can be routed through a PTY here in the terminal milestone.
    let mut command = command(program);
    command
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(ssh_command) = ssh_command {
        command.env("GIT_SSH_COMMAND", ssh_command);
    }
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    let mut child = command.spawn().map_err(|error| ProcessError {
        stdout: String::new(),
        stderr: error.to_string(),
        exit_code: None,
    })?;
    let stdout = child.stdout.take().ok_or_else(|| ProcessError {
        stdout: String::new(),
        stderr: "Failed to capture process stdout.".to_owned(),
        exit_code: None,
    })?;
    let stderr = child.stderr.take().ok_or_else(|| ProcessError {
        stdout: String::new(),
        stderr: "Failed to capture process stderr.".to_owned(),
        exit_code: None,
    })?;
    let stdout_reader = read_pipe(stdout, None);
    let (prompt_sender, prompt_receiver) = mpsc::channel();
    let stderr_reader = read_pipe_observed(stderr, prompt_sender);
    let started = Instant::now();
    let mut observed_stderr = String::new();
    let (status, timed_out, input_requested, cancelled) = loop {
        collect_output(&prompt_receiver, &mut observed_stderr);
        let input_requested = has_interactive_prompt(&observed_stderr);
        if input_requested {
            let status = match child.try_wait() {
                Ok(Some(status)) => status,
                Ok(None) => {
                    terminate_process_tree(&mut child).map_err(|error| ProcessError {
                        stdout: String::new(),
                        stderr: format!(
                            "Git requested interactive input but could not be terminated: {error}"
                        ),
                        exit_code: None,
                    })?;
                    child.wait().map_err(|error| ProcessError {
                        stdout: String::new(),
                        stderr: error.to_string(),
                        exit_code: None,
                    })?
                }
                Err(error) => {
                    let _ = child.kill();
                    return Err(ProcessError {
                        stdout: String::new(),
                        stderr: error.to_string(),
                        exit_code: None,
                    });
                }
            };
            break (status, false, true, false);
        }
        if cancellation.is_some_and(|token| token.load(Ordering::Relaxed)) {
            terminate_process_tree(&mut child).map_err(|error| ProcessError {
                stdout: String::new(),
                stderr: format!(
                    "Operation was cancelled, but the process tree could not be terminated: {error}"
                ),
                exit_code: None,
            })?;
            let status = child.wait().map_err(|error| ProcessError {
                stdout: String::new(),
                stderr: error.to_string(),
                exit_code: None,
            })?;
            break (status, false, false, true);
        }
        match child.try_wait() {
            Ok(Some(status)) => break (status, false, false, false),
            Ok(None) if started.elapsed() >= timeout => {
                terminate_process_tree(&mut child).map_err(|error| ProcessError {
                    stdout: String::new(),
                    stderr: format!("Process timed out, but could not be terminated: {error}"),
                    exit_code: None,
                })?;
                let status = child.wait().map_err(|error| ProcessError {
                    stdout: String::new(),
                    stderr: error.to_string(),
                    exit_code: None,
                })?;
                break (status, true, false, false);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                return Err(ProcessError {
                    stdout: String::new(),
                    stderr: error.to_string(),
                    exit_code: None,
                });
            }
        }
    };
    let (stdout, _) = read_output(stdout_reader).map_err(|error| ProcessError {
        stdout: String::new(),
        stderr: error,
        exit_code: status.code(),
    })?;
    let (stderr, _) = read_output(stderr_reader).map_err(|error| ProcessError {
        stdout: stdout.clone(),
        stderr: error,
        exit_code: status.code(),
    })?;
    if input_requested || has_interactive_prompt(&stderr.to_ascii_lowercase()) {
        return Err(ProcessError {
            stdout,
            stderr: format!(
                "{stderr}\nGit requested interactive input; the process was terminated."
            ),
            exit_code: status.code(),
        });
    }
    if cancelled {
        return Err(ProcessError {
            stdout,
            stderr: format!("{stderr}\nOperation cancelled by user."),
            exit_code: status.code(),
        });
    }
    if timed_out {
        return Err(ProcessError {
            stdout,
            stderr: format!(
                "Process timed out after {} seconds.\n{stderr}",
                timeout.as_secs()
            ),
            exit_code: status.code(),
        });
    }
    Ok(ProcessOutput {
        stdout,
        stderr,
        exit_code: status.code(),
    })
}

pub fn run_with_output_limit(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    stdout_limit: usize,
) -> Result<(ProcessOutput, bool), ProcessError> {
    run_with_timeout_limit(
        program,
        args,
        cwd,
        Duration::from_secs(30),
        Some(stdout_limit),
    )
}

fn read_pipe_observed<T: Read + Send + 'static>(
    mut pipe: T,
    sender: Sender<Vec<u8>>,
) -> thread::JoinHandle<io::Result<(Vec<u8>, bool)>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let read = pipe.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
            if sender.send(buffer[..read].to_vec()).is_err() {
                break;
            }
        }
        Ok((output, false))
    })
}

fn collect_output(receiver: &Receiver<Vec<u8>>, output: &mut String) {
    for bytes in receiver.try_iter() {
        output.push_str(&String::from_utf8_lossy(&bytes).to_ascii_lowercase());
    }
    if output.len() > 4096 {
        let excess = output.len() - 4096;
        let mut boundary = excess;
        while !output.is_char_boundary(boundary) {
            boundary += 1;
        }
        output.drain(..boundary);
    }
}

fn has_interactive_prompt(output: &str) -> bool {
    [
        "enter passphrase for key",
        "permission denied (publickey)",
        "permission denied (password,publickey)",
        "permission denied, please try again",
        "could not read username",
        "terminal prompts disabled",
    ]
    .iter()
    .any(|marker| output.contains(marker))
}

fn terminate_process_tree(child: &mut Child) -> Result<(), String> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        match command("taskkill.exe")
            .args(["/PID", &pid, "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => fallback_terminate(
                child,
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ),
            Err(error) => fallback_terminate(child, error.to_string()),
        }
    }
    #[cfg(not(windows))]
    {
        child.kill().map_err(|error| error.to_string())
    }
}

#[cfg(windows)]
fn fallback_terminate(child: &mut Child, initial_error: String) -> Result<(), String> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(kill_error) => match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(format!(
                "{initial_error}; direct termination failed: {kill_error}"
            )),
            Err(wait_error) => Err(format!(
                "{initial_error}; direct termination failed: {kill_error}; checking process failed: {wait_error}"
            )),
        },
    }
}

fn run_with_timeout_limit(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    timeout: Duration,
    stdout_limit: Option<usize>,
) -> Result<(ProcessOutput, bool), ProcessError> {
    run_with_timeout_limit_and_input(program, args, cwd, timeout, stdout_limit, None, &[])
}

pub fn run_with_input(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    input: &[u8],
    timeout: Duration,
) -> Result<ProcessOutput, ProcessError> {
    run_with_timeout_limit_and_input(program, args, cwd, timeout, None, Some(input), &[])
        .map(|(output, _)| output)
}

fn run_with_timeout_limit_and_input(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    timeout: Duration,
    stdout_limit: Option<usize>,
    input: Option<&[u8]>,
    environment: &[(&str, &str)],
) -> Result<(ProcessOutput, bool), ProcessError> {
    let mut command = command(program);
    command
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.envs(environment.iter().copied());
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }

    let mut child = command.spawn().map_err(|error| ProcessError {
        stdout: String::new(),
        stderr: error.to_string(),
        exit_code: None,
    })?;

    let stdout = child.stdout.take().ok_or_else(|| ProcessError {
        stdout: String::new(),
        stderr: "Failed to capture process stdout.".to_owned(),
        exit_code: None,
    })?;
    let stderr = child.stderr.take().ok_or_else(|| ProcessError {
        stdout: String::new(),
        stderr: "Failed to capture process stderr.".to_owned(),
        exit_code: None,
    })?;
    let stdout_reader = read_pipe(stdout, stdout_limit);
    let stderr_reader = read_pipe(stderr, None);
    let stdin_writer = if let Some(bytes) = input {
        let Some(mut stdin) = child.stdin.take() else {
            return Err(ProcessError {
                stdout: String::new(),
                stderr: "Failed to open process stdin.".to_owned(),
                exit_code: None,
            });
        };
        let bytes = bytes.to_vec();
        Some(thread::spawn(move || stdin.write_all(&bytes)))
    } else {
        None
    };
    let started = Instant::now();
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false),
            Ok(None) if started.elapsed() >= timeout => {
                if let Err(error) = terminate_process_tree(&mut child) {
                    return Err(ProcessError {
                        stdout: String::new(),
                        stderr: format!("Process timed out, but could not be terminated: {error}"),
                        exit_code: None,
                    });
                }
                let status = child.wait().map_err(|error| ProcessError {
                    stdout: String::new(),
                    stderr: error.to_string(),
                    exit_code: None,
                })?;
                break (status, true);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                return Err(ProcessError {
                    stdout: String::new(),
                    stderr: error.to_string(),
                    exit_code: None,
                });
            }
        }
    };
    let (stdout, stdout_truncated) = read_output(stdout_reader).map_err(|error| ProcessError {
        stdout: String::new(),
        stderr: error,
        exit_code: status.code(),
    })?;
    let (stderr, _) = read_output(stderr_reader).map_err(|error| ProcessError {
        stdout: stdout.clone(),
        stderr: error,
        exit_code: status.code(),
    })?;
    if let Some(writer) = stdin_writer {
        match writer.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) if error.kind() == io::ErrorKind::BrokenPipe => {}
            Ok(Err(error)) => {
                return Err(ProcessError {
                    stdout,
                    stderr: format!("Failed to send process stdin: {error}\n{stderr}"),
                    exit_code: status.code(),
                });
            }
            Err(_) => {
                return Err(ProcessError {
                    stdout,
                    stderr: format!("Process stdin writer stopped unexpectedly.\n{stderr}"),
                    exit_code: status.code(),
                });
            }
        }
    }

    if timed_out {
        return Err(ProcessError {
            stdout,
            stderr: format!(
                "Process timed out after {} seconds.\n{stderr}",
                timeout.as_secs()
            ),
            exit_code: status.code(),
        });
    }
    Ok((
        ProcessOutput {
            stdout,
            stderr,
            exit_code: status.code(),
        },
        stdout_truncated,
    ))
}

fn read_pipe<T: Read + Send + 'static>(
    mut pipe: T,
    byte_limit: Option<usize>,
) -> thread::JoinHandle<io::Result<(Vec<u8>, bool)>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut truncated = false;
        let mut buffer = [0u8; 8192];
        loop {
            let read = pipe.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            if let Some(limit) = byte_limit {
                let available = limit.saturating_sub(output.len());
                let capture = read.min(available);
                output.extend_from_slice(&buffer[..capture]);
                truncated |= capture < read;
            } else {
                output.extend_from_slice(&buffer[..read]);
            }
        }
        Ok((output, truncated))
    })
}

fn read_output(
    reader: thread::JoinHandle<io::Result<(Vec<u8>, bool)>>,
) -> Result<(String, bool), String> {
    let (bytes, truncated) = reader
        .join()
        .map_err(|_| "Output reader thread stopped unexpectedly.".to_owned())?
        .map_err(|error| error.to_string())?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

#[cfg(test)]
mod tests {
    use super::{has_interactive_prompt, run_network_operation};
    use std::ffi::OsString;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn recognizes_ssh_prompts_without_treating_regular_push_errors_as_prompts() {
        assert!(has_interactive_prompt(
            "enter passphrase for key 'id_ed25519':"
        ));
        assert!(has_interactive_prompt("permission denied (publickey)."));
        assert!(has_interactive_prompt(
            "permission denied, please try again."
        ));
        assert!(has_interactive_prompt(
            "permission denied (password,publickey)."
        ));
        assert!(!has_interactive_prompt("rejected: non-fast-forward"));
    }

    #[cfg(windows)]
    #[test]
    fn cancellation_terminates_the_windows_process_tree() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let cancel_signal = Arc::clone(&cancellation);
        let signal = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel_signal.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        let output = run_network_operation(
            Path::new("powershell.exe"),
            &[
                OsString::from("-NoProfile"),
                OsString::from("-Command"),
                OsString::from("Start-Sleep -Seconds 30"),
            ],
            None,
            Duration::from_secs(10),
            Some(&cancellation),
            None,
        )
        .expect_err("cancelled process must fail");
        signal.join().unwrap();
        assert!(output.stderr.contains("Operation cancelled by user."));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
