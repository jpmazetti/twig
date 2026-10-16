import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { confirm } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  ArrowDown,
  ArrowUp,
  Check,
  CircleDot,
  ChevronDown,
  Copy,
  GitCommit,
  File,
  FilePlus2,
  FileText,
  FolderOpen,
  Download,
  GitBranch,
  GitPullRequest,
  History,
  LoaderCircle,
  Moon,
  Minus,
  RefreshCw,
  RotateCcw,
  Plus,
  Sun,
  Upload,
  X,
  TerminalSquare,
} from "lucide-react";
import { Button } from "../components/ui/button";
import { AdvancedGitDialog, type AdvancedOperation } from "../components/AdvancedGitDialog";
import { BranchesPanel, BranchQuickSwitcher } from "../components/BranchesPanel";
import { DiffView } from "../components/DiffView";
import { HistoryPanel } from "../components/HistoryPanel";
import { OperationBanner } from "../components/OperationBanner";
import { TerminalDrawer } from "../components/TerminalDrawer";
import { TwigMark } from "../components/TwigMark";
import { useRepository } from "../hooks/useRepository";
import { useTerminal } from "../hooks/useTerminal";
import type { TerminalSessionInfo } from "../hooks/useTerminal";
import type {
  ChangedFile,
  CommitContext,
  CommitSummary,
  AdvancedOutput,
  AdvancedRequest,
  CommandResult,
  HistoryCommit,
  HistoryPage,
  OperationActionRequest,
  DiffPreview,
  PushOutput,
  WorkingTreeOperation,
  RemoteInfo,
} from "../types/git";
import {
  buildCommitMessage,
  canCommit,
  classifyNetworkFailure,
  formatGitCommand,
  splitCommitMessage,
} from "../lib/commit";
import { buildGitCommand } from "../lib/terminal";
import "../App.css";

interface FileLane {
  file: ChangedFile;
  staged: boolean;
  key: string;
}

type DiscardRequest =
  | { kind: "file"; lane: FileLane }
  | { kind: "all"; files: ChangedFile[]; conflictsExcluded: number };

function App() {
  const {
    git,
    repository,
    status,
    branches,
    operationState,
    branchesRefreshing,
    error,
    loading,
    statusRefreshing,
    networkOperation,
    networkDurationMs,
    lastFetchAt,
    chooseRepository,
    refresh,
    refreshStatus,
    refreshBranches,
    refreshOperationState,
    changeFiles,
    loadDiff,
    listHistory,
    listUnpushedCommits,
    getHistoryFingerprint,
    loadHistoryDetail,
    loadHistoryDiff,
    loadCommitContext,
    createCommit,
    listRemotes,
    listRemoteDetails,
    runNetworkOperation,
    cancelNetworkOperation,
    pushChanges,
    previewAdvanced,
    runAdvancedOperation,
    runOperationAction,
    createBranch,
    switchBranch,
    renameBranch,
    deleteBranch,
    clearError,
    reportError,
  } = useRepository();
  const terminal = useTerminal();
  const {
    sessions: terminalSessions,
    failure: terminalFailure,
    initialized: terminalInitialized,
    createSession: createPtySession,
    write: writeTerminal,
    writeBinary: writeTerminalBinary,
    resize: resizeTerminal,
    closeSession: closeTerminalSessionById,
    closeAll: closeAllTerminals,
    clearHistory: clearTerminalHistory,
    registerOutput: registerTerminalOutput,
    clearActivity: clearTerminalActivity,
  } = terminal;
  const [lightTheme, setLightTheme] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(() => localStorage.getItem("terminal-open") === "true");
  const [alwaysUseTerminalForNetwork, setAlwaysUseTerminalForNetwork] = useState(
    () => localStorage.getItem("terminal-network-operations") === "true",
  );
  const [activeTerminalId, setActiveTerminalId] = useState<string | null>(null);
  const [terminalFocusToken, setTerminalFocusToken] = useState(0);
  const [preferredShell, setPreferredShell] = useState(() => localStorage.getItem("terminal-shell") ?? "pwsh");
  const [activePanel, setActivePanel] = useState<"changes" | "history">("changes");
  const [historyInitialPath, setHistoryInitialPath] = useState<string | null>(null);
  const [historyInitialHash, setHistoryInitialHash] = useState<string | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [selectionCleared, setSelectionCleared] = useState(false);
  const [diffState, setDiffState] = useState<{ key: string; diff: DiffPreview | null; loading: boolean } | null>(null);
  const [discardRequest, setDiscardRequest] = useState<DiscardRequest | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [largeLoadedKey, setLargeLoadedKey] = useState<string | null>(null);
  const [commitSubject, setCommitSubject] = useState("");
  const [commitBody, setCommitBody] = useState("");
  const [descriptionExpanded, setDescriptionExpanded] = useState(false);
  const [amend, setAmend] = useState(false);
  const [amendRepository, setAmendRepository] = useState<string | null>(null);
  const [commitContextState, setCommitContextState] = useState<{ repository: string; value: CommitContext } | null>(null);
  const [commitStage, setCommitStage] = useState<"committing" | "pushing" | null>(null);
  const [commitNotice, setCommitNotice] = useState<CommitSummary | null>(null);
  const [lastPushOutput, setLastPushOutput] = useState<PushOutput | null>(null);
  const [pushFailedAfterCommit, setPushFailedAfterCommit] = useState(false);
  const [retryRemote, setRetryRemote] = useState<string | null>(null);
  const [remoteChoices, setRemoteChoices] = useState<string[] | null>(null);
  const [remoteChoiceAfterCommit, setRemoteChoiceAfterCommit] = useState(false);
  const [selectedRemote, setSelectedRemote] = useState("");
  const [forcePushConfirmation, setForcePushConfirmation] = useState(false);
  const [copyCommandError, setCopyCommandError] = useState<string | null>(null);
  const [commandCopied, setCommandCopied] = useState(false);
  const terminalCreatePromise = useRef<Promise<TerminalSessionInfo | null> | null>(null);
  const terminalRefreshTimers = useRef<number[]>([]);
  const terminalSessionsRef = useRef(terminalSessions);
  useEffect(() => {
    terminalSessionsRef.current = terminalSessions;
  }, [terminalSessions]);
  const [remoteInfoState, setRemoteInfoState] = useState<{ repository: string; remotes: RemoteInfo[] } | null>(null);
  const remoteInfo = remoteInfoState?.repository === repository ? remoteInfoState.remotes : [];
  const [remotePopoverOpen, setRemotePopoverOpen] = useState(false);
  const [remoteNotice, setRemoteNotice] = useState<string | null>(null);
  const [remoteOutput, setRemoteOutput] = useState<PushOutput | null>(null);
  const [advancedDialog, setAdvancedDialog] = useState<{ operation: AdvancedOperation; target: string; mergeParents: number } | null>(null);
  const [advancedBusy, setAdvancedBusy] = useState(false);
  const [advancedOutcome, setAdvancedOutcome] = useState<string | null>(null);
  const savedDraft = useRef<{ subject: string; body: string } | null>(null);
  const mergePrefillRepository = useRef<string | null>(null);
  const currentDraft = useRef({ subject: "", body: "" });
  const commitContext = commitContextState?.repository === repository ? commitContextState.value : null;
  const updateSubject = useCallback((subject: string) => {
    currentDraft.current.subject = subject;
    setCommitSubject(subject);
  }, []);
  const updateBody = useCallback((body: string) => {
    currentDraft.current.body = body;
    setCommitBody(body);
  }, []);

  const createTerminalSession = useCallback(async () => {
    if (!repository) return null;
    const session = await createPtySession(repository, preferredShell);
    if (session) setActiveTerminalId(session.id);
    return session;
  }, [createPtySession, preferredShell, repository]);

  const openTerminal = useCallback(async () => {
    setTerminalOpen(true);
    setTerminalFocusToken((token) => token + 1);
    if (!repository) return null;
    const available = terminalSessions.find((session) => session.id === activeTerminalId)
      ?? terminalSessions[terminalSessions.length - 1];
    if (available) {
      setActiveTerminalId(available.id);
      return available;
    }
    if (terminalCreatePromise.current) return terminalCreatePromise.current;
    const creating = createPtySession(repository, preferredShell);
    terminalCreatePromise.current = creating;
    try {
      const session = await creating;
      if (session) setActiveTerminalId(session.id);
      return session;
    } finally {
      if (terminalCreatePromise.current === creating) terminalCreatePromise.current = null;
    }
  }, [activeTerminalId, createPtySession, preferredShell, repository, terminalSessions]);

  const ensureRunningTerminal = useCallback(async (): Promise<TerminalSessionInfo | null> => {
    setTerminalOpen(true);
    setTerminalFocusToken((token) => token + 1);
    const available = terminalSessions.find((session) => session.id === activeTerminalId && session.running)
      ?? [...terminalSessions].reverse().find((session) => session.running);
    if (available) {
      setActiveTerminalId(available.id);
      return available;
    }
    if (!repository) return null;
    if (terminalCreatePromise.current) return terminalCreatePromise.current;
    const creating = createPtySession(repository, preferredShell);
    terminalCreatePromise.current = creating;
    try {
      const session = await creating;
      if (session) setActiveTerminalId(session.id);
      return session;
    } finally {
      if (terminalCreatePromise.current === creating) terminalCreatePromise.current = null;
    }
  }, [activeTerminalId, createPtySession, preferredShell, repository, terminalSessions]);

  const refreshAfterTerminalInput = useCallback(() => {
    const refreshRepository = () => {
      void refreshStatus();
      void refreshBranches();
      void refreshOperationState();
    };
    terminalRefreshTimers.current.push(
      window.setTimeout(refreshRepository, 1400),
      window.setTimeout(refreshRepository, 5000),
    );
  }, [refreshBranches, refreshOperationState, refreshStatus]);

  const runInTerminal = useCallback(async (args: string[], execute = false): Promise<boolean> => {
    if (!repository) return false;
    const session = await ensureRunningTerminal();
    if (!session) return false;
    const gitArgs = args[0] === "-C" ? args.slice(2) : args;
    try {
      writeTerminal(session.id, buildGitCommand(session.shell, repository, gitArgs, execute));
      if (execute) refreshAfterTerminalInput();
      return true;
    } catch (cause) {
      reportError({
        ok: false,
        operation: "Prepare command in terminal",
        args: ["-C", repository, ...gitArgs],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return false;
    }
  }, [ensureRunningTerminal, refreshAfterTerminalInput, reportError, repository, writeTerminal]);

  const handleTerminalError = useCallback((message: string) => {
    reportError({
      ok: false,
      operation: "Terminal",
      args: [],
      exitCode: null,
      stderr: message,
      stdout: "",
    });
  }, [reportError]);
  const closeTerminalSession = useCallback(async (session: TerminalSessionInfo) => {
    await closeTerminalSessionById(session.id);
  }, [closeTerminalSessionById]);
  const toggleTerminal = useCallback(() => {
    if (terminalOpen) setTerminalOpen(false);
    else void openTerminal();
  }, [openTerminal, terminalOpen]);
  const refreshFromTerminal = useCallback(() => {
    void refreshStatus();
    void refreshBranches();
    void refreshOperationState();
  }, [refreshBranches, refreshOperationState, refreshStatus]);

  useEffect(() => {
    localStorage.setItem("terminal-open", String(terminalOpen));
  }, [terminalOpen]);

  useEffect(() => {
    localStorage.setItem("terminal-shell", preferredShell);
  }, [preferredShell]);

  useEffect(() => {
    localStorage.setItem("terminal-network-operations", String(alwaysUseTerminalForNetwork));
  }, [alwaysUseTerminalForNetwork]);

  useEffect(() => {
    if (terminalOpen && terminalInitialized && repository && terminalSessions.length === 0) {
      const timer = window.setTimeout(() => void openTerminal(), 0);
      return () => window.clearTimeout(timer);
    }
  }, [openTerminal, repository, terminalInitialized, terminalOpen, terminalSessions.length]);

  useEffect(() => {
    const onShortcut = (event: KeyboardEvent) => {
      if (event.ctrlKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void chooseRepository();
        return;
      }
      if (!event.ctrlKey || event.altKey || event.metaKey) return;
      if (event.code !== "Backquote" && event.key.toLowerCase() !== "j") return;
      event.preventDefault();
      toggleTerminal();
    };
    window.addEventListener("keydown", onShortcut);
    return () => window.removeEventListener("keydown", onShortcut);
  }, [chooseRepository, toggleTerminal]);

  useEffect(() => {
    const activeWindow = getCurrentWindow();
    let mounted = true;
    let unlisten: (() => void) | undefined;
    void activeWindow.onCloseRequested(async (event) => {
      if (!terminalSessionsRef.current.some((session) => session.running)) return;
      event.preventDefault();
      if (!await confirm("Fechar o Twig e encerrar todas as sessões do terminal e seus processos?", {
        title: "Twig",
        kind: "warning",
      })) return;
      try {
        const closed = await closeAllTerminals();
        if (closed) await activeWindow.destroy();
        else setTerminalOpen(true);
      } catch (cause) {
        handleTerminalError(cause instanceof Error ? cause.message : String(cause));
      }
    }).then((stop) => {
      if (mounted) unlisten = stop;
      else stop();
    }).catch((cause: unknown) => {
      handleTerminalError(cause instanceof Error ? cause.message : String(cause));
    });
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [closeAllTerminals, handleTerminalError]);

  useEffect(() => () => {
    for (const timer of terminalRefreshTimers.current) window.clearTimeout(timer);
    terminalRefreshTimers.current = [];
  }, []);

  const stagedFiles = useMemo(() => status?.files
    .filter((file) => file.indexStatus !== "." && file.indexStatus !== "?")
    .map((file) => ({ file, staged: true, key: `staged:${file.path}` })) ?? [], [status]);
  const unstagedFiles = useMemo(() => status?.files
    .filter((file) => file.status === "untracked" || file.worktreeStatus !== ".")
    .map((file) => ({ file, staged: false, key: `unstaged:${file.path}` })) ?? [], [status]);
  const allLanes = useMemo(() => [...stagedFiles, ...unstagedFiles], [stagedFiles, unstagedFiles]);
  const previousPath = selectedKey?.slice(selectedKey.indexOf(":") + 1);
  const selectedLane = selectionCleared ? null : allLanes.find((lane) => lane.key === selectedKey)
    ?? allLanes.find((lane) => lane.file.path === previousPath)
    ?? allLanes[0]
    ?? null;
  const selectedDiffState = selectedLane && diffState?.key === selectedLane.key ? diffState : null;
  const selectedDiff = selectedDiffState?.diff ?? null;
  const diffLoading = Boolean(selectedLane && selectedLane.file.status !== "conflicted"
    && (!selectedDiffState || selectedDiffState.loading));
  const diffLoadLarge = selectedLane?.key === largeLoadedKey;
  const commitMessage = useMemo(
    () => buildCommitMessage(commitSubject, commitBody),
    [commitBody, commitSubject],
  );
  const isAmending = amend && amendRepository === repository;
  const canCommitMessage = canCommit(commitMessage, stagedFiles.length, isAmending);
  const commitLocked = actionBusy || commitStage !== null || networkOperation !== null || discardRequest !== null || Boolean(operationState?.operation);
  const commitDisabledReasonText = operationState?.operation
    ? `Finish or abort the ${operationState.operation} in progress before committing.`
    : commitDisabledReason(commitMessage, canCommitMessage, stagedFiles.length, isAmending);

  useEffect(() => {
    if (!repository || !status) return;
    let active = true;
    void loadCommitContext().then((context) => {
      if (!active || !context) return;
      setCommitContextState({ repository, value: context });
      if (context.mergeInProgress && context.mergeMessage && mergePrefillRepository.current !== repository) {
        if (!currentDraft.current.subject.trim() && !currentDraft.current.body.trim()) {
          const parsed = splitCommitMessage(context.mergeMessage);
          updateSubject(parsed.subject);
          updateBody(parsed.body);
        }
        mergePrefillRepository.current = repository;
      }
    });
    return () => { active = false; };
  }, [loadCommitContext, repository, status, updateBody, updateSubject]);

  useEffect(() => {
    let active = true;
    if (repository) {
      void listRemoteDetails().then((items) => {
        if (active && items) setRemoteInfoState({ repository, remotes: items });
      });
    }
    return () => { active = false; };
  }, [listRemoteDetails, repository]);

  useEffect(() => {
    let active = true;
    if (!selectedLane || selectedLane.file.status === "conflicted") return () => { active = false; };
    void loadDiff(
      selectedLane.file.path,
      selectedLane.staged,
      selectedLane.file.status === "untracked",
    ).then((diff) => {
      if (active) setDiffState({ key: selectedLane.key, diff, loading: false });
    });
    return () => { active = false; };
  }, [loadDiff, selectedLane]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (actionBusy || networkOperation || discardRequest || (target instanceof HTMLElement && ["INPUT", "TEXTAREA"].includes(target.tagName))) return;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        if (allLanes.length === 0) return;
        event.preventDefault();
        const current = allLanes.findIndex((lane) => lane.key === selectedLane?.key);
        const direction = event.key === "ArrowDown" ? 1 : -1;
        const next = (current + direction + allLanes.length) % allLanes.length;
        setSelectionCleared(false);
        setSelectedKey(allLanes[next].key);
      } else if (event.key.toLowerCase() === "s" && selectedLane && !selectedLane.staged && selectedLane.file.status !== "conflicted") {
        event.preventDefault();
        setActionBusy(true);
        void changeFiles("Stage", lanePaths(selectedLane.file)).finally(() => setActionBusy(false));
      } else if (event.key.toLowerCase() === "u" && selectedLane?.staged && selectedLane.file.status !== "conflicted") {
        event.preventDefault();
        setActionBusy(true);
        void changeFiles("Unstage", lanePaths(selectedLane.file)).finally(() => setActionBusy(false));
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [actionBusy, allLanes, changeFiles, discardRequest, networkOperation, selectedKey, selectedLane]);

  const runAction = useCallback(async (operation: WorkingTreeOperation, paths: string[]) => {
    setActionBusy(true);
    await changeFiles(operation, paths);
    setActionBusy(false);
  }, [changeFiles]);

  const stageLane = useCallback(async (lane: FileLane) => {
    await runAction("Stage", lanePaths(lane.file));
  }, [runAction]);

  const unstageLane = useCallback(async (lane: FileLane) => {
    await runAction("Unstage", lanePaths(lane.file));
  }, [runAction]);

  async function confirmDiscard() {
    if (!discardRequest) return;
    setActionBusy(true);
    if (discardRequest.kind === "file") {
      const { file } = discardRequest.lane;
      await changeFiles(
        file.status === "untracked" ? "Discard untracked" : "Discard",
        [file.path],
      );
    } else {
      const trackedPaths = discardRequest.files
        .filter((file) => file.status !== "untracked")
        .map((file) => file.path);
      const untrackedPaths = discardRequest.files
        .filter((file) => file.status === "untracked")
        .map((file) => file.path);
      if (trackedPaths.length && !(await changeFiles("Discard all tracked", trackedPaths))) {
        setActionBusy(false);
        setDiscardRequest(null);
        return;
      }
      if (untrackedPaths.length) await changeFiles("Discard all untracked", untrackedPaths);
    }
    setActionBusy(false);
    setDiscardRequest(null);
  }

  async function loadLargeDiff() {
    if (!selectedLane) return;
    setDiffState({ key: selectedLane.key, diff: null, loading: true });
    setLargeLoadedKey(selectedLane.key);
    const diff = await loadDiff(
      selectedLane.file.path,
      selectedLane.staged,
      selectedLane.file.status === "untracked",
      true,
    );
    setDiffState({ key: selectedLane.key, diff, loading: false });
    if (!diff) setLargeLoadedKey(null);
  }

  async function executePush(remote: string | null, forceWithLease: boolean, afterCommit = false) {
    setCommitStage("pushing");
    setCommandCopied(false);
    if (alwaysUseTerminalForNetwork) {
      clearError();
      setRemoteNotice(null);
      setRemoteOutput(null);
      const args = ["push"];
      if (forceWithLease) args.push("--force-with-lease");
      if (!status?.upstream && remote && status?.branch) args.push("-u", remote, status.branch);
      const written = await runInTerminal(args, true);
      setCommitStage(null);
      if (!written) {
        setPushFailedAfterCommit(afterCommit);
        setRetryRemote(remote);
        return;
      }
      setPushFailedAfterCommit(false);
      setRemoteNotice("Comando de push enviado ao terminal.");
      setRemoteChoices(null);
      setForcePushConfirmation(false);
      return;
    }
    const output = await pushChanges(remote, forceWithLease);
    setCommitStage(null);
    if (!output) {
      setPushFailedAfterCommit(afterCommit);
      setRetryRemote(remote);
      return;
    }
    setPushFailedAfterCommit(false);
    setLastPushOutput(output);
    setRemoteOutput(output);
    setRemoteNotice("Push concluído.");
    setRemoteChoices(null);
    setForcePushConfirmation(false);
  }

  async function beginPush(forceWithLease = false, remote: string | null = null, afterCommit = false) {
    if (status?.upstream || remote !== null) {
      await executePush(status?.upstream ? null : remote, forceWithLease, afterCommit);
      return;
    }
    if (!status?.branch) {
      await executePush(null, forceWithLease, afterCommit);
      return;
    }
    const remotes = await listRemotes();
    if (!remotes) {
      setPushFailedAfterCommit(afterCommit);
      setRetryRemote(null);
      setCommitStage(null);
      return;
    }
    setRemoteChoices(remotes);
    setSelectedRemote(remotes[0] ?? "");
    setRemoteChoiceAfterCommit(afterCommit);
    setCommitStage(null);
  }

  async function runCommit(pushAfter: boolean) {
    if (!canCommitMessage || commitLocked) return;
    setCommitStage("committing");
    setLastPushOutput(null);
    setPushFailedAfterCommit(false);
    setRetryRemote(null);
    const summary = await createCommit(commitMessage, isAmending);
    if (!summary) {
      setCommitStage(null);
      return;
    }
    updateSubject("");
    updateBody("");
    setDescriptionExpanded(false);
    savedDraft.current = null;
    setAmend(false);
    setAmendRepository(null);
    setCommitNotice(summary);
    setCommitStage(null);
    if (pushAfter) {
      setCommitStage("pushing");
      await beginPush(false, null, true);
    }
  }

  function handleCommitShortcut(
    event: ReactKeyboardEvent<HTMLInputElement | HTMLTextAreaElement>,
  ) {
    if (event.key === "Enter" && event.ctrlKey) {
      event.preventDefault();
      void runCommit(false);
    }
  }

  function changeAmend(enabled: boolean) {
    if (enabled) {
      savedDraft.current = { subject: commitSubject, body: commitBody };
      const parsed = splitCommitMessage(commitContext?.lastMessage ?? "");
      updateSubject(parsed.subject);
      updateBody(parsed.body);
    } else {
      const previous = savedDraft.current;
      if (previous) {
        updateSubject(previous.subject);
        updateBody(previous.body);
      }
      savedDraft.current = null;
    }
    setAmend(enabled);
    setAmendRepository(enabled ? repository : null);
  }

  async function retryPush(forceWithLease = false) {
    if (forceWithLease) setForcePushConfirmation(false);
    if (!status?.upstream && retryRemote === null) {
      await beginPush(forceWithLease);
      return;
    }
    await executePush(retryRemote, forceWithLease, pushFailedAfterCommit);
  }

  async function confirmRemotePush() {
    if (!selectedRemote) return;
    setRemoteChoices(null);
    await executePush(selectedRemote, false, remoteChoiceAfterCommit);
  }

  async function runRemoteAction(operation: "fetch" | "pull", strategy: "merge" | "rebase" | "ff-only" | null = null) {
    if (operation === "pull" && operationState?.operation) {
      reportError({
        ok: false,
        operation: "Pull",
        args: ["-C", repository ?? "", "pull"],
        exitCode: null,
        stderr: `Não é possível fazer pull enquanto ${operationState.operation} está em andamento. Continue ou aborte primeiro.`,
        stdout: "",
      });
      return;
    }
    const commitsBehindBeforePull = status?.behind ?? 0;
    setRemoteNotice(null);
    setRemoteOutput(null);
    if (alwaysUseTerminalForNetwork) {
      clearError();
      const args = operation === "fetch"
        ? ["fetch", "--prune"]
        : ["pull", ...(strategy === "merge" ? ["--no-rebase"] : strategy === "rebase" ? ["--rebase"] : strategy === "ff-only" ? ["--ff-only"] : [])];
      if (await runInTerminal(args, true)) {
        setRemoteNotice(`Comando de ${operation} enviado ao terminal.`);
      }
      return;
    }
    const output = await runNetworkOperation(operation, null, false, strategy);
    if (!output) return;
    setRemoteOutput(output);
    if (operation === "fetch") {
      setRemoteNotice("Fetch concluído.");
    } else if (commitsBehindBeforePull > 0) {
      setRemoteNotice(`${commitsBehindBeforePull} ${commitsBehindBeforePull === 1 ? "commit recebido" : "commits recebidos"} com pull.`);
    } else if (/already up to date/i.test(`${output.stdout}\n${output.stderr}`)) {
      setRemoteNotice("Já está atualizado.");
    } else {
      setRemoteNotice("Pull concluído.");
    }
  }

  function openAdvancedDialog(operation: AdvancedOperation, target: string, mergeParents = 1) {
    if (operationState?.operation) return;
    clearError();
    setAdvancedOutcome(null);
    setAdvancedDialog({ operation, target, mergeParents });
  }

  async function submitAdvancedOperation(request: AdvancedRequest) {
    setAdvancedBusy(true);
    try {
      const result = await runAdvancedOperation(request);
      if (!result.ok) return result;
      const text = summarizeAdvancedSuccess(request, result.data);
      setAdvancedOutcome(text);
      setAdvancedDialog(null);
      return result;
    } finally {
      setAdvancedBusy(false);
    }
  }

  async function runOperationControl(request: OperationActionRequest) {
    const result = await runOperationAction(request);
    if (result.ok && result.data.previousHead) {
      setAdvancedOutcome(`${request.operation} ${request.action} completed. Previous HEAD was ${result.data.previousHead.slice(0, 12)}.`);
    }
    return result;
  }

  async function copyPushCommand() {
    if (!error) return;
    try {
      await navigator.clipboard.writeText(formatGitCommand(error.args));
      setCopyCommandError(null);
      setCommandCopied(true);
    } catch (cause) {
      setCopyCommandError(cause instanceof Error ? cause.message : String(cause));
      setCommandCopied(false);
    }

  }

  async function runBranchOperation(operation: () => Promise<boolean>, switched = false): Promise<boolean> {
    setActionBusy(true);
    try {
      const succeeded = await operation();
      if (succeeded && switched) {
        setSelectionCleared(true);
        setSelectedKey(null);
        setDiffState(null);
        setLargeLoadedKey(null);
      }
      return succeeded;
    } finally {
      setActionBusy(false);
    }
  }

  const interactivePush = Boolean(error
    && ["Fetch", "Pull", "Push"].includes(error.operation)
    && classifyNetworkFailure(error.stderr, error.stdout) === "ssh-auth");
  const networkFailure = error && ["Fetch", "Pull", "Push"].includes(error.operation)
    ? classifyNetworkFailure(error.stderr, error.stdout)
    : "other";
  const forcePushAvailable = error?.operation === "Push" && networkFailure === "non-fast-forward";
  const divergentPull = error?.operation === "Pull" && networkFailure === "divergent-pull";
  const currentBranchInfo = branches?.local.find((branch) => branch.current);
  const historyRefreshKey = [
    status?.headOid ?? "",
    branches?.local.map((branch) => `${branch.name}:${branch.shortHash}`).join(",") ?? "",
    branches?.remote.map((branch) => `${branch.name}:${branch.shortHash}`).join(",") ?? "",
  ].join("|");
  const detachedHead = !status?.branch;
  const upstreamRemoteName = remoteInfo
    .map((remote) => remote.name)
    .filter((name) => status?.upstream?.startsWith(`${name}/`))
    .sort((left, right) => right.length - left.length)[0];
  return (
    <main className={lightTheme ? "app-shell theme-light" : "app-shell"}>
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark"><TwigMark size={19} /></div>
          <div className="repo-copy topbar-repo">
            <div className="repo-title">
              <span>{repository ? repository.split(/[\\/]/).filter(Boolean).pop() : "Twig"}</span>
            </div>
            <div className="repo-path">{repository ?? "Nenhum repositório aberto"}</div>
          </div>
        </div>

        <div className="topbar-actions">
          {repository && (
            <BranchQuickSwitcher
              branches={branches}
              repository={repository}
              currentBranch={status?.branch ?? null}
              detachedHead={branches?.detachedHead ?? status?.headOid ?? null}
              busy={commitLocked}
              onSwitch={(name, trackRemote = false) => runBranchOperation(() => switchBranch(name, trackRemote), true)}
            />
          )}
          {repository && (
            <RemoteBar
              branch={status?.branch}
              upstream={status?.upstream ?? currentBranchInfo?.upstream ?? null}
              upstreamGone={currentBranchInfo?.upstreamGone ?? false}
              ahead={status?.ahead ?? currentBranchInfo?.ahead ?? null}
              behind={status?.behind ?? currentBranchInfo?.behind ?? null}
              detached={detachedHead}
              hasHead={Boolean(status?.headOid)}
              remotes={remoteInfo}
              remotesLoading={!status}
              networkOperation={networkOperation}
              operationInProgress={Boolean(operationState?.operation)}
              lastFetchAt={lastFetchAt}
              alwaysUseTerminalForNetwork={alwaysUseTerminalForNetwork}
              notice={remoteNotice}
              popoverOpen={remotePopoverOpen}
              onToggleRemotes={async () => {
                if (!remotePopoverOpen) {
                  const remotes = await listRemoteDetails();
                  if (remotes && repository) setRemoteInfoState({ repository, remotes });
                }
                setRemotePopoverOpen((open) => !open);
              }}
              onFetch={() => void runRemoteAction("fetch")}
              onPull={() => void runRemoteAction("pull")}
              onPush={() => void beginPush()}
              onAlwaysUseTerminalForNetworkChange={setAlwaysUseTerminalForNetwork}
              onCancel={() => void cancelNetworkOperation()}
            />
          )}
          {repository && git?.ok && (
            <div className="git-version">
              <span className="connection-dot" />
              Git {git.data.version.replace(/^git version\s+/i, "")} detectado
            </div>
          )}
          <Button
            variant="ghost"
            size="icon"
            aria-label="Atualizar repositório"
            title="Atualizar repositório"
            disabled={loading || statusRefreshing || branchesRefreshing || networkOperation !== null}
            onClick={() => void refresh()}
          >
            <RefreshCw size={16} strokeWidth={1.75} className={loading || statusRefreshing || branchesRefreshing || networkOperation !== null ? "spin" : undefined} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title={lightTheme ? "Mudar para o tema escuro" : "Mudar para o tema claro"}
            aria-label={lightTheme ? "Mudar para o tema escuro" : "Mudar para o tema claro"}
            onClick={() => setLightTheme((current) => !current)}
          >
            {lightTheme ? <Moon size={16} strokeWidth={1.75} /> : <Sun size={16} strokeWidth={1.75} />}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            aria-label={terminalOpen ? "Recolher terminal" : "Abrir terminal"}
            title="Terminal (Ctrl+`)"
            onClick={toggleTerminal}
          >
            <TerminalSquare size={16} strokeWidth={1.75} />
          </Button>
          {repository && (
            <Button className="topbar-open-button" disabled={networkOperation !== null} onClick={() => void chooseRepository()}>
              <FolderOpen size={16} />
              Abrir repositório
            </Button>
          )}
        </div>
      </header>

      <section className="workspace">
        {repository && (
          <OperationBanner
            repository={repository}
            state={operationState}
            busy={advancedBusy}
            onAction={runOperationControl}
            onRefresh={() => void refresh()}
            onOpenTerminal={() => void openTerminal()}
          />
        )}

        <div className={repository ? "repository-layout" : "repository-layout repository-layout-empty"}>
          {repository && (
            <aside className="repository-sidebar">
              <BranchesPanel
                branches={branches}
                repository={repository}
                currentBranch={status?.branch ?? null}
                busy={commitLocked}
                advancedDisabled={Boolean(operationState?.operation) || advancedBusy}
                onAdvanced={(operation, target) => openAdvancedDialog(operation, target)}
                onOpenTerminal={() => void openTerminal()}
                refreshing={branchesRefreshing || loading}
                error={error}
                onClearError={clearError}
                onCreate={(name, startPoint, switchTo) => runBranchOperation(
                  () => createBranch(name, startPoint, switchTo),
                  switchTo,
                )}
                onSwitch={(name, trackRemote = false) => runBranchOperation(
                  () => switchBranch(name, trackRemote),
                  true,
                )}
                onRename={(oldName, newName) => runBranchOperation(() => renameBranch(oldName, newName))}
                onDelete={(name, force = false) => runBranchOperation(() => deleteBranch(name, force))}
              />
            </aside>
          )}
          <div className={repository ? "main-content" : "main-content empty-state-content"}>
          {repository && advancedOutcome && (
            <div className="commit-toast advanced-success" role="status">
              <Check size={14} />
              <span>{advancedOutcome}</span>
              <button type="button" aria-label="Dispensar resultado da operação" onClick={() => setAdvancedOutcome(null)}><X size={14} /></button>
            </div>
          )}
          {repository && error && (
            <section className="error-card" role="alert">
              <div className="error-heading">
                <AlertCircle size={19} />
                <div>
                  <h2>Falha na operação {error.operation}</h2>
                  <p>{pushFailedAfterCommit
                    ? "Commit criado com sucesso, mas o push falhou."
                    : "O Git retornou um erro. A saída original está abaixo."}</p>
                </div>
              </div>
              <p className="error-command">{formatGitCommand(error.args)}</p>
              {interactivePush && (
                <div className="interactive-warning">
                  SSH precisa de autenticação interativa ou a chave não está no agent. Carregue a chave com <code>ssh-add</code> ou digite a senha no terminal.
                </div>
              )}
              {networkFailure === "host-key" && <p className="remote-error-hint">O SSH rejeitou a chave do host. Verifique a impressão digital e a entrada em known_hosts.</p>}
              {networkFailure === "https-auth" && <p className="remote-error-hint">O Git não conseguiu obter as credenciais HTTPS. Verifique o gerenciador de credenciais configurado.</p>}
              {networkFailure === "network" && <p className="remote-error-hint">Verifique a conexão de rede e o nome do host remoto e tente novamente.</p>}
              {networkFailure === "repository-access" && <p className="remote-error-hint">Verifique a URL remota e suas permissões no repositório.</p>}
              {networkFailure === "non-fast-forward" && <p className="remote-error-hint">O push foi rejeitado porque o remote tem commits que você ainda não tem. Faça pull primeiro.</p>}
              {error.operation === "Pull" && status?.files.some((file) => file.status === "conflicted") && (
                <div className="conflict-files">
                  <strong>Arquivos em conflito</strong>
                  <ul>{status.files.filter((file) => file.status === "conflicted").map((file) => <li key={file.path}>{file.path}</li>)}</ul>
                </div>
              )}
              <details className="git-output-details" open={error.operation === "Pull" && status?.files.some((file) => file.status === "conflicted")}>
                <summary>Detalhes</summary>
                <p>Comando: {formatGitCommand(error.args)}</p>
                <p>Duração: {networkDurationMs ?? 0} ms</p>
                <p>Código de saída: {error.exitCode ?? "desconhecido"}</p>
                <pre>{formatGitOutput(error.stdout, error.stderr)}</pre>
              </details>
              {copyCommandError && <p className="copy-command-error">Não foi possível copiar o comando: {copyCommandError}</p>}
              {commandCopied && <p className="copy-command-copied">Comando copiado.</p>}
              <div className="error-actions">
                {interactivePush && (
                  <>
                    <Button variant="secondary" onClick={() => void runInTerminal(error.args)}>
                      <TerminalSquare size={16} />
                      Executar no terminal
                    </Button>
                    <Button variant="outline" onClick={() => void copyPushCommand()}>Copiar comando</Button>
                  </>
                )}
                <Button variant="secondary" onClick={() => void openTerminal()}>
                  <TerminalSquare size={16} />
                  Abrir terminal
                </Button>
                {error.operation === "Pull" && divergentPull && (
                  <>
                    <Button variant="outline" disabled={networkOperation !== null} onClick={() => void runRemoteAction("pull", "merge")}>Merge</Button>
                    <Button variant="outline" disabled={networkOperation !== null} onClick={() => void runRemoteAction("pull", "rebase")}>Rebase</Button>
                    <Button variant="outline" disabled={networkOperation !== null} onClick={() => void runRemoteAction("pull", "ff-only")}>Somente fast-forward</Button>
                  </>
                )}
                {error.operation === "Push" && networkFailure === "non-fast-forward" && (
                  <Button
                    variant="outline"
                    disabled={networkOperation !== null || !status?.upstream}
                    onClick={() => void runRemoteAction("pull")}
                    title={!status?.upstream ? "Nenhum upstream configurado" : "Faça pull antes do push"}
                  >
                    Fazer pull primeiro
                  </Button>
                )}
                {["Fetch", "Pull", "Push"].includes(error.operation) && !divergentPull && (
                  <Button
                    variant="outline"
                    disabled={networkOperation !== null}
                    onClick={() => {
                      if (error.operation === "Push") void retryPush();
                      else void runRemoteAction(error.operation === "Fetch" ? "fetch" : "pull");
                    }}
                  >
                    Tentar novamente
                  </Button>
                )}
                {forcePushAvailable && (
                  <Button className="destructive-button" onClick={() => setForcePushConfirmation(true)}>
                  Push com --force-with-lease
                  </Button>
                )}
                <Button variant="outline" onClick={() => { clearError(); void refresh(); }}>
                  <RefreshCw size={15} />
                  Atualizar
                </Button>
              </div>
            </section>
          )}
          {repository && remoteOutput && !error && (
            <details className="remote-output-details">
              <summary>Detalhes da operação remota</summary>
              <p>Comando: {formatGitCommand(remoteOutput.args)}</p>
              <p>Duração: {networkDurationMs ?? 0} ms</p>
              <p>Código de saída: 0</p>
              <p>SSH: BatchMode=yes foi usado nesta operação em segundo plano. Operações no terminal usam a configuração SSH interativa.</p>
              <pre>{formatGitOutput(remoteOutput.stdout, remoteOutput.stderr)}</pre>
            </details>
          )}

          {!repository && (
            <section className="welcome-card">
              <div className="welcome-icon"><TwigMark size={56} /></div>
              <p className="eyebrow">COMECE COM UM REPOSITÓRIO</p>
              <h1>No que você está trabalhando?</h1>
              <p className="welcome-copy">
                Escolha um repositório local. O Twig mostra a branch atual e o estado do working tree.
              </p>
              <Button onClick={() => void chooseRepository()}>
                <FolderOpen size={16} />
                Abrir repositório
              </Button>
              <span className="open-repository-hint">Atalho <kbd>Ctrl+O</kbd></span>
              {git?.ok ? (
                <span className="version-note"><span className="connection-dot" />Git {git.data.version.replace(/^git version\s+/i, "")} detectado</span>
              ) : git?.stderr ? (
                <p className="git-missing-notice" role="alert">
                  Git não encontrado. Instale o Git for Windows e reinicie o Twig.
                  {" "}<a href="https://git-scm.com/download/win" target="_blank" rel="noreferrer">Baixar Git for Windows</a>
                </p>
              ) : (
                <span className="version-note">Verificando a instalação do Git…</span>
              )}
            </section>
          )}

          {repository && (
            activePanel === "history" ? (
              <HistoryPanel
                branches={branches}
                refreshKey={historyRefreshKey}
                initialPath={historyInitialPath}
                initialHash={historyInitialHash}
                onBack={() => {
                  setActivePanel("changes");
                  setHistoryInitialPath(null);
                  setHistoryInitialHash(null);
                }}
                listHistory={listHistory}
                listUnpushedCommits={listUnpushedCommits}
                getHistoryFingerprint={getHistoryFingerprint}
                loadHistoryDetail={loadHistoryDetail}
                loadHistoryDiff={loadHistoryDiff}
                onOpenTerminal={() => void openTerminal()}
                onCherryPick={(hash, parentCount) => openAdvancedDialog("cherry-pick", hash, parentCount)}
                onReset={(hash) => openAdvancedDialog("reset", hash)}
                advancedDisabled={Boolean(operationState?.operation) || advancedBusy}
              />
            ) : <>
              <section className="changes-panel">
                <div className="panel-heading">
                  <div>
                    <p className="eyebrow">Working tree</p>
                    <h1>Alterações</h1>
                  </div>
                  <span className="count-badge">{stagedFiles.length + unstagedFiles.length}</span>
                  <div className="changes-header-actions">
                    <Button variant="outline" disabled={!unstagedFiles.length || actionBusy || commitStage !== null || networkOperation !== null} onClick={() => void runAction("Stage all", [])}>Preparar tudo</Button>
                    <Button variant="outline" disabled={!stagedFiles.length || actionBusy || commitStage !== null || networkOperation !== null} onClick={() => void runAction("Unstage all", [])}>Remover tudo do stage</Button>
                  </div>
                </div>
                {status && status.files.length === 0 ? (
                  <div className="empty-changes">
                    <div className="empty-check"><Check size={17} /></div>
                    <div>
                      <h2>Working tree limpo</h2>
                      <p>Não há alterações locais neste repositório.</p>
                    </div>
                  </div>
                ) : !status ? (
                  <div className="empty-changes">
                    <RefreshCw size={16} className={loading ? "spin" : undefined} />
                    <div><h2>{loading ? "Lendo o working tree" : "Status indisponível"}</h2></div>
                  </div>
                ) : (
                  <div className="working-tree">
                    <FileSection
                      title="Em stage"
                      files={stagedFiles}
                      selectedKey={selectedLane?.key ?? null}
                      actionBusy={actionBusy || commitStage !== null || networkOperation !== null}
                      onSelect={(key) => { setSelectionCleared(false); setSelectedKey(key); }}
                      onAll={() => void runAction("Unstage all", [])}
                      onRowAction={(lane) => void unstageLane(lane)}
                      actionLabel="Remover do stage"
                      actionIcon={<Minus size={14} />}
                      onShowHistory={(path) => {
                        setHistoryInitialPath(path);
                        setActivePanel("history");
                      }}
                    />
                    <FileSection
                      title="Fora do stage"
                      files={unstagedFiles}
                      selectedKey={selectedLane?.key ?? null}
                      actionBusy={actionBusy || commitStage !== null || networkOperation !== null}
                      onSelect={(key) => { setSelectionCleared(false); setSelectedKey(key); }}
                      onAll={() => void runAction("Stage all", [])}
                      onRowAction={(lane) => void stageLane(lane)}
                      actionLabel="Adicionar ao stage"
                      actionIcon={<Plus size={14} />}
                      onShowHistory={(path) => {
                        setHistoryInitialPath(path);
                        setActivePanel("history");
                      }}
                      onDiscard={(lane) => setDiscardRequest({ kind: "file", lane })}
                      onDiscardAll={() => setDiscardRequest({
                        kind: "all",
                        files: status.files.filter((file) => file.status !== "conflicted" && (file.status === "untracked" || file.worktreeStatus !== ".")),
                        conflictsExcluded: status.files.filter((file) => file.status === "conflicted" && file.worktreeStatus !== ".").length,
                      })}
                    />
                    {selectedLane && (
                      <DiffPanel
                        lane={selectedLane}
                        diff={selectedDiff}
                        loading={diffLoading}
                        loadLarge={diffLoadLarge}
                        onLoadLarge={() => void loadLargeDiff()}
                      />
                    )}
                  </div>
                )}
                <section className="commit-composer" aria-label="Commit">
                  <div className="commit-composer-heading">
                    <div>
                      <p className="eyebrow">Commit em <code>{status?.branch ?? "Detached HEAD"}</code></p>
                      <div className="commit-target">
                        <span>{stagedFiles.length} staged {stagedFiles.length === 1 ? "file" : "files"}</span>
                      </div>
                    </div>
                    <label className="amend-toggle">
                      <input
                        type="checkbox"
                        checked={isAmending}
                        disabled={!commitContext?.lastMessage || commitLocked}
                        onChange={(event) => changeAmend(event.target.checked)}
                        title={!commitContext?.lastMessage ? "Nenhum commit para alterar" : undefined}
                      />
                      Amend
                    </label>
                  </div>
                  <div className="commit-fields">
                    <div className="commit-subject-wrap">
                      <input
                        className="commit-subject"
                        type="text"
                        value={commitSubject}
                        disabled={commitLocked}
                        placeholder="Subject"
                        aria-label="Commit subject"
                        onChange={(event) => updateSubject(event.target.value)}
                        onKeyDown={handleCommitShortcut}
                      />
                      <span className={Array.from(commitSubject).length > 72 ? "subject-count subject-count-long" : "subject-count"}>
                        {Array.from(commitSubject).length}/72
                      </span>
                    </div>
                    <button
                      className="description-toggle"
                      type="button"
                      disabled={commitLocked}
                      onClick={() => setDescriptionExpanded((expanded) => !expanded)}
                    >
                      {descriptionExpanded ? "Ocultar descrição" : "Adicionar descrição"}
                    </button>
                    {descriptionExpanded && (
                      <textarea
                        className="commit-body"
                        value={commitBody}
                        placeholder="Descrição opcional"
                        aria-label="Descrição do commit"
                        rows={3}
                        disabled={commitLocked}
                        onChange={(event) => updateBody(event.target.value)}
                        onKeyDown={handleCommitShortcut}
                      />
                    )}
                  </div>
                  {isAmending && (
                    <div className={commitContext?.published ? "commit-warning commit-warning-strong" : "commit-warning"}>
                      {commitContext?.published
                        ? "Isso reescreve um commit já publicado no upstream. O próximo push exigirá --force-with-lease."
                        : "Isso reescreve o último commit."}
                    </div>
                  )}
                  {commitContext?.mergeInProgress && (
                    <div className="commit-warning">Há um merge em andamento. O Git usará MERGE_MSG, se disponível.</div>
                  )}
                  {commitContext?.otherOperation && (
                    <div className="commit-warning">
                      Há uma operação {commitContext.otherOperation} em andamento; o Git pode rejeitar um commit comum.
                    </div>
                  )}
                  {commitNotice && (
                    <div className="commit-toast" role="status">
                      <Check size={14} />
                      <span>Commit criado {commitNotice.shortHash} · {commitNotice.subject}</span>
                      <button type="button" aria-label="Dispensar aviso de commit" onClick={() => setCommitNotice(null)}><X size={14} /></button>
                    </div>
                  )}
                  {pushFailedAfterCommit && !error && (
                    <Button variant="outline" disabled={commitLocked} onClick={() => void retryPush()}>
                      Tentar push novamente
                    </Button>
                  )}
                  {lastPushOutput && (
                    <details className="push-success-details">
                      <summary>Detalhes do push</summary>
                      <pre>{formatGitOutput(lastPushOutput.stdout, lastPushOutput.stderr)}</pre>
                    </details>
                  )}
                  <div className="commit-actions">
                    <span className="disabled-tooltip" title={commitDisabledReasonText}>
                      <Button
                        disabled={!canCommitMessage || commitLocked}
                        onClick={() => void runCommit(false)}
                      >
                        {commitStage === "committing" ? <LoaderCircle size={14} className="spin" /> : <GitCommit size={14} />}
                        {commitStage === "committing" ? "Criando commit…" : "Commit"}
                      </Button>
                    </span>
                    <span className="disabled-tooltip" title={commitDisabledReasonText}>
                      <Button
                        variant="secondary"
                        disabled={!canCommitMessage || commitLocked}
                        onClick={() => void runCommit(true)}
                      >
                        {commitStage === "pushing" ? <LoaderCircle size={14} className="spin" /> : null}
                        Commit e push
                      </Button>
                    </span>
                    {commitStage === "pushing" && <span className="commit-stage-label">Enviando push…</span>}
                  </div>
                </section>
              </section>

              <RecentHistoryCard
                listHistory={listHistory}
                refreshKey={historyRefreshKey}
                onSelect={(hash) => {
                  setHistoryInitialPath(null);
                  setHistoryInitialHash(hash || null);
                  setActivePanel("history");
                }}
                onError={reportError}
              />
            </>
          )}
          </div>
        </div>

        <footer className="statusbar">
          <span><CircleDot size={13} /> {repository ? `Working tree · ${stagedFiles.length} em stage · ${unstagedFiles.length} fora do stage` : "Pronto"}</span>
          <button className="status-terminal-button" type="button" onClick={toggleTerminal} aria-label={terminalOpen ? "Recolher terminal" : "Abrir terminal"}>
            <TerminalSquare size={14} strokeWidth={1.75} /> Terminal <kbd>Ctrl+`</kbd>
          </button>
        </footer>
        {discardRequest && (
          <DiscardDialog
            request={discardRequest}
            busy={actionBusy || commitStage !== null || networkOperation !== null}
            onCancel={() => setDiscardRequest(null)}
            onDiscard={() => void confirmDiscard()}
          />
        )}
        {remoteChoices !== null && (
          <RemoteDialog
            remotes={remoteChoices}
            repository={repository ?? ""}
            branch={status?.branch ?? ""}
            selectedRemote={selectedRemote}
            busy={commitStage === "pushing"}
            onSelect={setSelectedRemote}
            onCancel={() => {
              setRemoteChoices(null);
              setPushFailedAfterCommit(remoteChoiceAfterCommit);
              setRetryRemote(null);
            }}
            onConfirm={() => void confirmRemotePush()}
          />
        )}
        {forcePushConfirmation && (
          <ForcePushDialog
            busy={commitStage === "pushing"}
            branch={status?.branch ?? "unknown"}
            remote={upstreamRemoteName ?? retryRemote ?? "selected remote"}
            onCancel={() => setForcePushConfirmation(false)}
            onConfirm={() => void retryPush(true)}
          />
        )}
        {advancedDialog && repository && (
          <AdvancedGitDialog
            repository={repository}
            branch={status?.branch ?? null}
            branches={branches}
            operation={advancedDialog.operation}
            initialTarget={advancedDialog.target}
            mergeParents={advancedDialog.mergeParents}
            busy={advancedBusy || Boolean(operationState?.operation)}
            onPreview={previewAdvanced}
            onRun={submitAdvancedOperation}
            onRunInTerminal={(args) => {
              setAdvancedDialog(null);
              void runInTerminal(args);
            }}
            onClose={() => setAdvancedDialog(null)}
          />
        )}
      </section>
      <TerminalDrawer
        open={terminalOpen}
        repository={repository}
        sessions={terminalSessions}
        activeSessionId={activeTerminalId}
        focusToken={terminalFocusToken}
        preferredShell={preferredShell}
        failure={terminalFailure}
        lightTheme={lightTheme}
        onToggle={toggleTerminal}
        onSetActive={(id) => {
          setActiveTerminalId(id);
          clearTerminalActivity(id);
        }}
        onPreferredShellChange={setPreferredShell}
        onCreate={createTerminalSession}
        onWrite={writeTerminal}
        onWriteBinary={writeTerminalBinary}
        onResize={resizeTerminal}
        onClose={closeTerminalSession}
        onClearHistory={(id) => void clearTerminalHistory(id)}
        onRegisterOutput={registerTerminalOutput}
        onPromptSubmit={refreshAfterTerminalInput}
        onTerminalFocus={refreshFromTerminal}
        onRefresh={refreshFromTerminal}
        onError={handleTerminalError}
      />
    </main>
  );
}

function summarizeAdvancedSuccess(request: AdvancedRequest, output: AdvancedOutput): string {
  const { operation } = request;
  const messages = `${output.stdout}\n${output.stderr}`;
  if (operation === "merge") {
    if (/already up to date/i.test(messages)) return "Já está atualizado.";
    if (request.squash) return "Squash aplicado. As alterações estão no stage; nenhum commit de merge foi criado.";
    if (/fast-forward/i.test(messages)) return `Fast-forward concluído${output.currentHead ? ` até ${output.currentHead.slice(0, 12)}` : ""}.`;
    if (request.noCommit) return "Alterações do merge preparadas; nenhum commit de merge foi criado.";
    if (output.currentHead && output.currentHead !== output.previousHead) {
      return `Commit de merge ${output.currentHead.slice(0, 12)} criado.`;
    }
    return "Merge concluído.";
  }
  if (operation === "reset") {
    const previous = output.previousHead?.slice(0, 12) ?? "desconhecido";
    return `Reset concluído. O HEAD anterior era ${previous}. Recupere com git reset --hard ${output.previousHead ?? "<hash>"}; o git reflog pode ajudar a localizar outras posições anteriores.`;
  }
  if (operation === "rebase" && output.previousHead) {
    return `Rebase concluído. O HEAD anterior era ${output.previousHead.slice(0, 12)}.`;
  }
  if (operation === "cherry-pick") return "Cherry-pick concluído.";
  return `${operation} concluído.`;
}

function formatRecentRelativeDate(value: string, now = Date.now()): string {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return value;
  const seconds = Math.round((timestamp - now) / 1000);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["year", 31_536_000],
    ["month", 2_592_000],
    ["week", 604_800],
    ["day", 86_400],
    ["hour", 3_600],
    ["minute", 60],
    ["second", 1],
  ];
  const formatter = new Intl.RelativeTimeFormat("pt-BR", { numeric: "auto" });
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size || size === 1) return formatter.format(Math.round(seconds / size), unit);
  }
  return "now";
}

function RecentHistoryCard({
  listHistory,
  refreshKey,
  onSelect,
  onError,
}: {
  listHistory: (
    revision: string | null,
    allRefs: boolean,
    search: string,
    searchBy: "message" | "author" | "hash",
    path: string | null,
    offset: number,
    limit: number,
  ) => Promise<CommandResult<HistoryPage>>;
  refreshKey: string;
  onSelect: (hash: string) => void;
  onError: (failure: { ok: false; operation: string; args: string[]; exitCode: number | null; stderr: string; stdout: string }) => void;
}) {
  const [historyState, setHistoryState] = useState<{ refreshKey: string; commits: HistoryCommit[]; loading: boolean }>({
    refreshKey: "",
    commits: [],
    loading: true,
  });
  const loading = historyState.refreshKey !== refreshKey || historyState.loading;
  const commits = historyState.refreshKey === refreshKey ? historyState.commits : [];

  useEffect(() => {
    let active = true;
    void listHistory(null, false, "", "message", null, 0, 3).then((result) => {
      if (!active) return;
      if (!result.ok) {
        onError(result);
        setHistoryState({ refreshKey, commits: [], loading: false });
      } else {
        setHistoryState({ refreshKey, commits: result.data.commits.slice(0, 3), loading: false });
      }
    }).catch((cause: unknown) => {
      if (!active) return;
      onError({
        ok: false,
        operation: "List recent history",
        args: ["git", "log", "-n", "3"],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      setHistoryState({ refreshKey, commits: [], loading: false });
    });
    return () => { active = false; };
  }, [listHistory, onError, refreshKey]);

  async function copyHash(hash: string) {
    try {
      await navigator.clipboard.writeText(hash);
    } catch (cause) {
      onError({
        ok: false,
        operation: "Copy commit hash",
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
    }
  }

  return (
    <section className="recent-history-card">
      <header className="recent-history-heading">
        <h2>Histórico recente</h2>
        <button type="button" onClick={() => onSelect("")}>Ver tudo <span aria-hidden="true">→</span></button>
      </header>
      {loading ? (
        <div className="recent-history-list" aria-label="Carregando commits recentes" aria-busy="true">
          {[0, 1, 2].map((row) => <div className="recent-history-skeleton" key={row}><i /><span /><b /></div>)}
        </div>
      ) : commits.length === 0 ? (
        <p className="recent-history-empty">Nenhum commit nesta branch.</p>
      ) : (
        <div className="recent-history-list">
          {commits.map((commit, index) => (
            <div className="recent-history-row" key={commit.hash}>
              <button className="recent-history-select" type="button" onClick={() => onSelect(commit.hash)} title={commit.subject}>
                <span className={`recent-history-marker ${index === 0 ? "recent-history-marker-head" : ""}`}>●</span>
                <span className="recent-history-copy">
                  <span className="recent-history-subject">{commit.subject || "(sem descrição)"}</span>
                  <span className="recent-history-meta">{commit.author} · {formatRecentRelativeDate(commit.authorDate)}</span>
                </span>
              </button>
              <code className="recent-history-hash">{commit.shortHash}</code>
              <button className="recent-history-copy-hash" type="button" aria-label={`Copiar hash ${commit.shortHash}`} title="Copiar hash" onClick={() => void copyHash(commit.hash)}>
                <Copy size={14} strokeWidth={1.75} />
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function RemoteBar({
  branch,
  upstream,
  upstreamGone,
  ahead,
  behind,
  detached,
  hasHead,
  remotes,
  remotesLoading,
  networkOperation,
  operationInProgress,
  lastFetchAt,
  alwaysUseTerminalForNetwork,
  notice,
  popoverOpen,
  onToggleRemotes,
  onFetch,
  onPull,
  onPush,
  onAlwaysUseTerminalForNetworkChange,
  onCancel,
}: {
  branch: string | null | undefined;
  upstream: string | null;
  upstreamGone: boolean;
  ahead: number | null;
  behind: number | null;
  detached: boolean;
  hasHead: boolean;
  remotes: RemoteInfo[];
  remotesLoading: boolean;
  networkOperation: "Fetch" | "Pull" | "Push" | null;
  operationInProgress: boolean;
  lastFetchAt: number | null;
  alwaysUseTerminalForNetwork: boolean;
  notice: string | null;
  popoverOpen: boolean;
  onToggleRemotes: () => void;
  onFetch: () => void;
  onPull: () => void;
  onPush: () => void;
  onAlwaysUseTerminalForNetworkChange: (enabled: boolean) => void;
  onCancel: () => void;
}) {
  const busy = networkOperation !== null;
  const hasRemotes = remotes.length > 0;
  const fetchDisabled = busy || !hasRemotes;
  const pullDisabled = busy || operationInProgress || detached || !hasHead || !upstream || upstreamGone || !hasRemotes;
  const pushDisabled = busy || detached || !hasHead || !hasRemotes;
  const pullReason = operationInProgress ? "Finish or abort the operation in progress first"
    : detached ? "Detached HEAD"
      : !hasHead ? "Repository has no commits"
      : !hasRemotes ? "No remote configured"
        : upstreamGone ? "Upstream is gone"
          : !upstream ? "No upstream configured"
            : undefined;
  const pushReason = detached ? "Detached HEAD"
    : !hasHead ? "Repository has no commits"
      : !hasRemotes ? "No remote configured"
        : undefined;
  const lastFetchLabel = lastFetchAt ? new Date(lastFetchAt).toLocaleString("pt-BR") : "Ainda não nesta sessão";
  return (
    <section
      className="remote-bar"
      aria-label="Operações remotas"
      title={`Branch: ${branch ?? "HEAD destacado"} · Upstream: ${upstream ? `${upstream}${upstreamGone ? " (removido)" : ""}` : "nenhum"} · Último fetch: ${lastFetchLabel}`}
    >
      <div className="remote-actions">
        {networkOperation ? (
          <>
            <span className="network-progress"><LoaderCircle size={14} className="spin" />{networkOperation}…</span>
            <Button variant="outline" onClick={onCancel}>Cancelar</Button>
          </>
        ) : (
          <div className="sync-buttons" role="group" aria-label="Fetch, pull e push">
            <span className="disabled-tooltip" title={!hasRemotes ? "Nenhum remote configurado" : `Buscar atualizações · Último fetch: ${lastFetchLabel}`}>
              <button type="button" disabled={fetchDisabled} onClick={onFetch}><Download size={16} strokeWidth={1.75} />Fetch</button>
            </span>
            <span className="sync-divider" aria-hidden="true" />
            <span className="disabled-tooltip" title={pullDisabled ? pullReason : `Fazer pull de ${upstream}`}>
              <button type="button" disabled={pullDisabled} onClick={onPull}>
                <GitPullRequest size={16} strokeWidth={1.75} />Pull
                <span className="sync-count sync-count-behind"><ArrowDown size={12} />{upstream ? behind ?? 0 : 0}</span>
              </button>
            </span>
            <span className="sync-divider" aria-hidden="true" />
            <span className="disabled-tooltip" title={pushDisabled ? pushReason : `Fazer push para ${upstream ?? "remote"}`}>
              <button type="button" disabled={pushDisabled} onClick={onPush}>
                <Upload size={16} strokeWidth={1.75} />Push
                <span className="sync-count sync-count-ahead"><ArrowUp size={12} />{upstream ? ahead ?? 0 : 0}</span>
              </button>
            </span>
          </div>
        )}
        {notice && <span className="remote-inline-notice" role="status">{notice}</span>}
        <div className="remote-popover-wrap">
          <Button variant="ghost" onClick={onToggleRemotes} aria-expanded={popoverOpen}>
            Remotes <ChevronDown size={13} strokeWidth={1.75} />
          </Button>
          {popoverOpen && (
            <div className="remote-popover">
              <strong>Remotes e configurações de rede</strong>
              <p className="remote-popover-upstream">Upstream atual: <code className={upstreamGone ? "upstream-gone" : ""}>{upstream ?? "nenhum"}{upstreamGone ? " (removido)" : ""}</code></p>
              <p className="remote-popover-fetch">Último fetch: {lastFetchLabel}</p>
              <label className="remote-terminal-setting" title="Executar Fetch, Pull e Push no terminal integrado, onde o SSH pode solicitar uma senha.">
                <input
                  type="checkbox"
                  checked={alwaysUseTerminalForNetwork}
                  disabled={busy}
                  onChange={(event) => onAlwaysUseTerminalForNetworkChange(event.target.checked)}
                />
                Sempre usar o terminal para operações de rede
              </label>
              <strong className="remote-list-title">Remotes configurados</strong>
              {remotesLoading && !remotes.length ? <p>Carregando remotes…</p> : remotes.length ? remotes.map((remote) => (
                <div className="remote-entry" key={remote.name}>
                  <b>{remote.name}</b>
                  <span>Fetch: <code>{remote.fetchUrl || "—"}</code></span>
                  <span>Push: <code>{remote.pushUrl || "—"}</code></span>
                </div>
              )) : <p>Nenhum remote configurado.</p>}
              {notice && <p className="remote-popover-notice" role="status">{notice}</p>}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function FileSection({
  title,
  files,
  selectedKey,
  actionBusy,
  onSelect,
  onAll,
  onRowAction,
  actionLabel,
  actionIcon,
  onDiscard,
  onDiscardAll,
  onShowHistory,
}: {
  title: string;
  files: FileLane[];
  selectedKey: string | null;
  actionBusy: boolean;
  onSelect: (key: string) => void;
  onAll: () => void;
  onRowAction: (lane: FileLane) => void;
  actionLabel: string;
  actionIcon: ReactNode;
  onDiscard?: (lane: FileLane) => void;
  onDiscardAll?: () => void;
  onShowHistory: (path: string) => void;
}) {
  const allLabel = title === "Em stage" ? "Remover tudo do stage" : "Adicionar tudo ao stage";
  return (
    <section className="file-section">
      <header className="file-section-heading">
        <div><h2>{title} · {files.length}</h2></div>
        <div className="section-actions">
          {onDiscardAll && <Button variant="outline" className="discard-all-button" disabled={!files.some((lane) => lane.file.status !== "conflicted") || actionBusy} onClick={onDiscardAll}><RotateCcw size={13} />Descartar tudo</Button>}
          <Button variant="outline" disabled={!files.length || actionBusy} onClick={onAll} title={allLabel}>{allLabel}</Button>
        </div>
      </header>
      {files.length ? <div className="file-list" role="list">
        {files.map((lane) => (
          <div
            key={lane.key}
            className={`file-row ${selectedKey === lane.key ? "file-row-selected" : ""}`}
            role="listitem"
            tabIndex={0}
            aria-selected={selectedKey === lane.key}
            onClick={() => onSelect(lane.key)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onSelect(lane.key);
              }
            }}
          >
            <FileIndicator lane={lane} />
            <span className="file-path" title={lane.file.path}>
              {filePathParts(lane.file.path).directory && <span className="file-directory">{filePathParts(lane.file.path).directory}</span>}
              <strong>{filePathParts(lane.file.path).name}</strong>
              {lane.file.status === "renamed" && lane.file.originalPath && <small>de {lane.file.originalPath}</small>}
            </span>
            {lane.file.status === "conflicted" && <span className="conflict-badge">conflito</span>}
            <span className="file-status">{lane.staged ? lane.file.indexStatus : lane.file.worktreeStatus}</span>
            {lane.file.status !== "conflicted" && (
              <div className="file-row-actions" onClick={(event) => event.stopPropagation()}>
                <Button variant="ghost" size="icon" aria-label={`${actionLabel} ${lane.file.path}`} title={`${actionLabel} (${actionLabel.startsWith("Adicionar") ? "S" : "U"})`} disabled={actionBusy} onClick={() => onRowAction(lane)}>{actionIcon}</Button>
                <Button variant="ghost" size="icon" aria-label={`Mostrar histórico de ${lane.file.path}`} title="Mostrar histórico do arquivo" onClick={() => onShowHistory(lane.file.path)}><History size={14} /></Button>
                {onDiscard && <Button variant="ghost" size="icon" aria-label={`Descartar ${lane.file.path}`} title="Descartar alterações" disabled={actionBusy} onClick={() => onDiscard(lane)}><RotateCcw size={14} /></Button>}
              </div>
            )}
          </div>
        ))}
      </div> : <p className="section-empty">Não há alterações {title.toLowerCase()}.</p>}
    </section>
  );
}

function RemoteDialog({
  remotes,
  repository,
  branch,
  selectedRemote,
  busy,
  onSelect,
  onCancel,
  onConfirm,
}: {
  remotes: string[];
  repository: string;
  branch: string;
  selectedRemote: string;
  busy: boolean;
  onSelect: (remote: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="dialog-backdrop" role="presentation">
      <section className="discard-dialog" role="dialog" aria-modal="true" aria-labelledby="remote-title">
        <button className="dialog-close" aria-label="Cancelar" disabled={busy} onClick={onCancel}><X size={17} /></button>
        <div className="discard-icon"><GitBranch size={19} /></div>
        <h2 id="remote-title">Escolha um remote</h2>
        {remotes.length ? (
          <>
            <p>Esta branch não tem upstream. Escolha um remote para que o Git o configure como upstream.</p>
            <label className="remote-select-label" htmlFor="push-remote">Remote</label>
            <select
              id="push-remote"
              className="remote-select"
              value={selectedRemote}
              onChange={(event) => onSelect(event.target.value)}
            >
              {remotes.map((remote) => <option key={remote} value={remote}>{remote}</option>)}
            </select>
            <p className="remote-command">
              Comando: {formatGitCommand(["-C", repository, "push", "-u", selectedRemote, branch])}
            </p>
          </>
        ) : (
          <p>Este repositório não tem remotes configurados. Adicione um remote antes de fazer push.</p>
        )}
        <div className="dialog-actions">
          <Button variant="outline" disabled={busy} onClick={onCancel}>Cancelar</Button>
          <Button disabled={!remotes.length || !selectedRemote || busy} onClick={onConfirm}>
            {busy ? "Enviando push…" : "Push"}
          </Button>
        </div>
      </section>
    </div>
  );
}

function ForcePushDialog({
  branch,
  remote,
  busy,
  onCancel,
  onConfirm,
}: {
  branch: string;
  remote: string;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="dialog-backdrop" role="presentation">
      <section className="discard-dialog" role="alertdialog" aria-modal="true" aria-labelledby="force-push-title">
        <button className="dialog-close" aria-label="Cancelar" disabled={busy} onClick={onCancel}><X size={17} /></button>
        <div className="discard-icon"><AlertCircle size={19} /></div>
        <h2 id="force-push-title">Reescrever a branch remota?</h2>
        <p>
          Isso executa <code>git push --force-with-lease</code> em <code>{remote}/{branch}</code>.
          A operação reescreve o histórico remoto e pode substituir commits de outros colaboradores.
          Revise a saída do Git e continue somente se quiser atualizar o commit publicado.
        </p>
        <div className="dialog-actions">
          <Button variant="outline" disabled={busy} onClick={onCancel}>Cancelar</Button>
          <Button className="destructive-button" disabled={busy} onClick={onConfirm}>
            {busy ? "Enviando push…" : "Push com --force-with-lease"}
          </Button>
        </div>
      </section>
    </div>
  );
}

function FileIndicator({ lane }: { lane: FileLane }) {
  const status = lane.staged ? lane.file.indexStatus : lane.file.worktreeStatus;
  const indicator = lane.file.status === "untracked" ? "?" : status === "." ? lane.file.code : status;
  const icon = lane.file.status === "untracked" ? <FilePlus2 size={15} /> : <FileText size={15} />;
  const indicatorClass = lane.staged && (indicator === "A" || indicator === "M") ? "staged" : statusClass(indicator);
  return <span className={`file-icon file-icon-${indicatorClass}`} aria-label={lane.file.status}>{icon}<small>{indicator}</small></span>;
}

function DiffPanel({
  lane,
  diff,
  loading,
  loadLarge,
  onLoadLarge,
}: {
  lane: FileLane;
  diff: DiffPreview | null;
  loading: boolean;
  loadLarge: boolean;
  onLoadLarge: () => void;
}) {
  return (
    <section className="diff-panel" aria-label="Diff do arquivo">
      <header className="diff-heading">
        <div><p className="eyebrow">{lane.staged ? "Diff no stage" : "Diff do working tree"}</p><h2>{lane.file.path}</h2></div>
        <File size={16} />
      </header>
      {loading ? <div className="diff-empty"><LoaderCircle size={15} className="spin" /> Carregando diff…</div>
        : lane.file.status === "conflicted" ? <div className="diff-empty">A prévia do diff não está disponível para conflitos não resolvidos.</div>
        : diff ? <DiffView diff={diff} onLoadLarge={onLoadLarge} allowLarge={loadLarge} />
        : <div className="diff-empty">Selecione um arquivo para ver seu diff.</div>}
    </section>
  );
}

function DiscardDialog({
  request,
  busy,
  onCancel,
  onDiscard,
}: {
  request: DiscardRequest;
  busy: boolean;
  onCancel: () => void;
  onDiscard: () => void;
}) {
  const isAll = request.kind === "all";
  const untracked = !isAll && request.lane.file.status === "untracked";
  const files = isAll ? request.files : [request.lane.file];
  const includesUntracked = files.some((file) => file.status === "untracked");
  return (
    <div className="dialog-backdrop" role="presentation">
      <section className="discard-dialog" role="dialog" aria-modal="true" aria-labelledby="discard-title">
        <button className="dialog-close" aria-label="Cancelar" disabled={busy} onClick={onCancel}><X size={17} /></button>
        <div className="discard-icon"><RotateCcw size={19} /></div>
        <h2 id="discard-title">{isAll ? "Descartar todas as alterações fora do stage?" : "Descartar alterações?"}</h2>
        <p>{isAll
          ? `Isso descartará permanentemente as alterações fora do stage em ${files.length} ${files.length === 1 ? "arquivo" : "arquivos"}.${includesUntracked ? " Arquivos não rastreados serão excluídos e não poderão ser recuperados." : ""}${request.kind === "all" && request.conflictsExcluded ? ` ${request.conflictsExcluded} ${request.conflictsExcluded === 1 ? "arquivo em conflito foi mantido" : "arquivos em conflito foram mantidos"} sem alterações.` : ""} As alterações no stage serão mantidas.`
          : untracked
            ? "Este arquivo não rastreado será removido permanentemente e não poderá ser recuperado."
            : "As alterações deste arquivo fora do stage serão descartadas permanentemente. As alterações no stage serão mantidas."}</p>
        <ul>{files.slice(0, 8).map((file) => <li key={file.path}>{file.path}</li>)}{files.length > 8 && <li>and {files.length - 8} more…</li>}</ul>
        <div className="dialog-actions">
          <Button variant="outline" disabled={busy} onClick={onCancel}>Cancelar</Button>
          <Button className="destructive-button" disabled={busy} onClick={onDiscard}>{busy ? "Descartando…" : "Descartar"}</Button>
        </div>
      </section>
    </div>
  );
}

function lanePaths(file: ChangedFile): string[] {
  return file.originalPath && file.originalPath !== file.path
    ? [file.originalPath, file.path]
    : [file.path];
}

function filePathParts(path: string) {
  const displayPath = path.replace(/[\\/]+$/, "") || path;
  const separator = Math.max(displayPath.lastIndexOf("/"), displayPath.lastIndexOf("\\"));
  return {
    directory: separator >= 0 ? displayPath.slice(0, separator + 1) : "",
    name: displayPath.slice(separator + 1),
  };
}

function statusClass(status: string): string {
  if (status === "A" || status === "?") return "new";
  if (status === "D") return "deleted";
  if (status === "R" || status === "C") return "renamed";
  if (status === "U") return "conflicted";
  return "modified";
}

function commitDisabledReason(message: string, canCommitValue: boolean, stagedCount: number, amend: boolean): string {
  if (!message.trim()) return "Message required";
  if (!canCommitValue && stagedCount === 0 && !amend) return "No staged files";
  return "";
}

function formatGitOutput(stdout: string, stderr: string): string {
  const output = [
    stdout ? `stdout:\n${stdout}` : "",
    stderr ? `stderr:\n${stderr}` : "",
  ].filter(Boolean);
  return output.join("\n") || "No additional output from Git.";
}

export default App;
