<div align="center">

# 🌿 Twig

**Git CLI for people who want to use the mouse.**

A modern, lightweight and transparent desktop interface on top of the Git you already have installed.

![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-TypeScript-61DAFB?logo=react&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-backend-000000?logo=rust&logoColor=white)
![Windows](https://img.shields.io/badge/Windows-10%2B-0078D4?logo=windows&logoColor=white)
[![deploy: no cuzinho](https://img.shields.io/badge/deploy-no%20cuzinho-d6331f)](https://cuzinho.dev)

**English** · **[Português (BR)](./README.pt-BR.md)**

<img src="./twig-icon.png" alt="Twig" width="215" />

</div>

---

## Why Twig?

Twig **does not reimplement Git** and does not try to compete with GitKraken, Fork or GitHub Desktop. It is a thin visual layer over the `git.exe` installed on your machine, built for people who already know Git and want to run common operations without typing the same commands all day.

- **Real Git.** Every action runs the actual Git command, and Git's original messages are never hidden.
- **Respects your setup.** Uses your `.gitconfig`, your SSH and your Credential Manager. No custom login, no tokens, no intermediary server.
- **A truly integrated terminal.** Full PTY with interactive prompts and SSH passphrase support, in the same repository you have open.
- **Light and fast.** Tauri + Rust, no Electron.
- **Dark and light mode.**

## Features

### Changes
- Separate *staged* and *unstaged* lists with status (modified, new, deleted, renamed)
- Stage / unstage individually or in bulk
- Discard changes with confirmation
- Text diffs with limits for large files
- Automatic refresh when the window regains focus

### Commit
- Commit, **Commit & Push** and **Amend** last commit
- Commits without a message are blocked

### Branches
- List, create, switch, rename and delete local branches
- Current branch and upstream always visible

### Remote
- Fetch, Pull and Push with **ahead/behind** indicator
- Cancellable network operations with a configurable timeout
- Remotes menu with fetch/push URLs (credentials masked)

### History
- Virtualized, paginated commit list
- Git-side search (message, author or hash)
- Branch scope and per-file filter
- Commit details: metadata, parents, refs, stats and diff
- Copy hash

### Advanced Git
- **Merge**, **Rebase**, **Cherry-pick** and **Reset** (soft, mixed and hard)
- The exact command arguments are shown before you confirm
- Detects in-progress operations and offers **Continue**, **Skip** and **Abort**
- Warnings for rebase/reset over already-published commits, with the previous hash for recovery
- Pull, Switch and Commit are blocked while an operation is active

### Integrated terminal
- Persistent PTY sessions with tabs for multiple sessions
- Starts in the open repository and inherits the user's environment
- Interactive commands, Git editors and SSH prompts
- Shortcut: ``Ctrl+` `` or `Ctrl+J`
- Output buffer is capped and never written to disk
- Confirmation before closing the window with active sessions

## Getting started

### Requirements

- [Node.js](https://nodejs.org/) (LTS)
- [Rust](https://www.rust-lang.org/tools/install) and the [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/)
- [Git for Windows](https://git-scm.com/download/win)

### Running in development

```powershell
git clone https://github.com/<your-username>/twig.git
cd twig
npm install
npm run tauri dev
```

### Building the installer / .exe

```powershell
npm run tauri build
```

Artifacts are placed in `src-tauri\target\release\bundle\`.

### Local validation

```powershell
cargo check  --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test   --manifest-path src-tauri\Cargo.toml
npx tsc --noEmit
npm run lint
```

## SSH on Windows

Twig **never collects or stores passphrases**. Git/SSH handles authentication, as always.

**Fetch, Pull and Push from the UI** run without interactive prompts and time out after 60 s by default (adjustable via the `GIT_GUI_NETWORK_TIMEOUT_SECS` environment variable, between 1 and 86400). If an operation needs a passphrase, open the error and choose **Run in terminal**: the command is typed into the prompt **without Enter**, you review and run it, and SSH asks for the passphrase inside the PTY (with echo disabled). Nothing typed or displayed is written to logs or disk. The terminal works without `ssh-agent`.

<details>
<summary><strong>Using ssh-agent (optional)</strong></summary>

With the key loaded in the agent, Fetch/Pull/Push work directly from the UI without asking for the passphrase.

In a PowerShell **run as administrator**, enable and start the `OpenSSH Authentication Agent` service:

```powershell
Set-Service ssh-agent -StartupType Automatic
Start-Service ssh-agent
```

In a regular PowerShell, add your key and check that it was loaded:

```powershell
ssh-add $env:USERPROFILE\.ssh\id_ed25519
ssh-add -l
```

> **Note:** the SSH client used by Git must be able to reach the same agent. On Windows, `where.exe ssh` may list two `ssh.exe` binaries (Windows OpenSSH and the one bundled with Git for Windows), which can use different agents. Also check `SSH_AUTH_SOCK` if it is set.

`git config --show-origin --get core.sshCommand` shows whether the repository already selects an SSH client. Twig respects `core.sshCommand` and `GIT_SSH_COMMAND`; for network operations started from the UI it appends `-o BatchMode=yes` to avoid hanging without a TTY. The integrated terminal does **not** inject `GIT_TERMINAL_PROMPT=0` or `BatchMode`, so Git/SSH can interact normally.

</details>

## Security and privacy

- No password, passphrase or token is stored or sent to any server.
- Application logs contain no sensitive data.
- Remote URLs are displayed with credentials masked.
- Destructive operations (discard, hard reset, rebase) require explicit confirmation.

## Architecture

```text
Frontend (React + TypeScript + Tailwind)
        ↓
Tauri commands
        ↓
Git Service  →  Process Runner / PTY
        ↓
git.exe installed on the machine
```

```text
src/          UI (components, hooks, pages, lib, types)
src-tauri/    Rust bridge: commands, git, process, terminal
```

Whenever possible, Twig uses Git's structured output (such as `git status --porcelain=v2`) instead of parsing text meant for humans.

## Out of scope (for now)

Stash, visual conflict resolution, force-push, tag push, Git LFS, submodules, worktrees, reflog, graph (DAG) visualization, GitHub/GitLab API integration (PRs, issues, CI) and account or SSH key management.

Conflicts can be resolved in the integrated terminal and marked as resolved by staging the files.

## Contributing

Issues and suggestions are welcome. Before opening a PR, run the [local validation](#local-validation). The project's golden rule: when in doubt between adding another visual abstraction and simply running the real Git command and showing the result clearly, **prefer running the real Git command**.

## License

Distributed under the MIT license. See the [`LICENSE`](LICENSE) file.

---

<div align="center">

*Git is still Git. Twig just makes using it better.*

</div>
