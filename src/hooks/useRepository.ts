import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { load } from "@tauri-apps/plugin-store";
import type {
  CommandFailure,
  CommandResult,
  BranchList,
  CommitContext,
  CommitSummary,
  AdvancedOutput,
  AdvancedPreview,
  AdvancedRequest,
  DiffPreview,
  HistoryDetail,
  HistoryFingerprint,
  HistoryPage,
  GitInstallation,
  PushOutput,
  RemoteInfo,
  RepositoryOperationState,
  OperationActionRequest,
  RepositoryStatus,
  WorkingTreeOperation,
} from "../types/git";

const LAST_REPOSITORY_KEY = "lastRepository";
const SETTINGS_FILE = "settings.json";

async function invokeHistory<T>(
  command: string,
  params: Record<string, unknown>,
  operation: string,
  args: string[],
): Promise<CommandResult<T>> {
  try {
    return await invoke<CommandResult<T>>(command, params);
  } catch (cause) {
    return {
      ok: false,
      operation,
      args,
      exitCode: null,
      stderr: cause instanceof Error ? cause.message : String(cause),
      stdout: "",
    };
  }
}

export function useRepository() {
  const [git, setGit] = useState<CommandResult<GitInstallation> | null>(null);
  const [repository, setRepository] = useState<string | null>(null);
  const [status, setStatus] = useState<RepositoryStatus | null>(null);
  const [branches, setBranches] = useState<BranchList | null>(null);
  const [operationStateValue, setOperationStateValue] = useState<{ repository: string; value: RepositoryOperationState } | null>(null);
  const [branchesRefreshing, setBranchesRefreshing] = useState(false);
  const [error, setError] = useState<CommandFailure | null>(null);
  const [loading, setLoading] = useState(false);
  const [statusRefreshing, setStatusRefreshing] = useState(false);
  const [networkOperation, setNetworkOperation] = useState<"Fetch" | "Pull" | "Push" | null>(null);
  const [networkDurationMs, setNetworkDurationMs] = useState<number | null>(null);
  const [lastFetchState, setLastFetchState] = useState<{ repository: string; timestamp: number } | null>(null);
  const loadingRef = useRef(false);
  const statusInFlight = useRef(false);
  const statusQueued = useRef(false);
  const statusRequestLock = useRef<Promise<void> | null>(null);
  const branchesInFlight = useRef(false);
  const branchesQueued = useRef(false);
  const branchesRepository = useRef<string | null>(null);
  const networkOperationRef = useRef(false);
  const advancedOperationRef = useRef(false);

  const fetchStatus = useCallback(async (path: string) => {
    while (statusRequestLock.current) {
      await statusRequestLock.current;
    }
    let release = () => {};
    const lock = new Promise<void>((resolve) => { release = resolve; });
    statusRequestLock.current = lock;
    try {
      return await invoke<CommandResult<RepositoryStatus>>("load_repository", { path });
    } finally {
      if (statusRequestLock.current === lock) statusRequestLock.current = null;
      release();
    }
  }, []);

  const refreshOperationState = useCallback(async (path: string) => {
    const result = await invokeHistory<RepositoryOperationState>(
      "repository_operation_state",
      { repository: path },
      "Read repository operation state",
      ["-C", path, "status", "--porcelain=v2", "--branch", "-z"],
    );
    if (result.ok) {
      setOperationStateValue({ repository: path, value: result.data });
    } else {
      setError(result);
    }
    return result;
  }, []);

  const loadRepository = useCallback(async (path: string, remember: boolean) => {
    loadingRef.current = true;
    setLoading(true);
    setError(null);
    branchesRepository.current = path;
    setRepository(path);
    setStatus(null);
    setBranches(null);
    setOperationStateValue(null);
    try {
      const result = await fetchStatus(path);
      if (!result.ok) {
        setError(result);
        return false;
      }
      setRepository(path);
      setStatus(result.data);
      await refreshOperationState(path);
      if (remember) {
        try {
          const store = await load(SETTINGS_FILE, { autoSave: true });
          await store.set(LAST_REPOSITORY_KEY, path);
        } catch (cause) {
          setError({
            ok: false,
            operation: "Remember repository",
            args: [],
            exitCode: null,
            stderr: cause instanceof Error ? cause.message : String(cause),
            stdout: "",
          });
        }
      }
      return true;
    } catch (cause) {
      setError({
        ok: false,
        operation: "Load repository",
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return false;
    } finally {
      loadingRef.current = false;
      setLoading(false);
    }
  }, [fetchStatus, refreshOperationState]);

  const detectGit = useCallback(async () => {
    setError(null);
    try {
      const result = await invoke<CommandResult<GitInstallation>>("detect_git");
      setGit(result);
      setError(result.ok ? null : result);
      return result;
    } catch (cause) {
      const failure: CommandFailure = {
        ok: false,
        operation: "Detect Git",
        args: ["git.exe", "--version"],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      };
      setGit(failure);
      setError(failure);
      return failure;
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    if (!repository || loadingRef.current) return;
    if (networkOperationRef.current) {
      statusQueued.current = true;
      return;
    }
    if (statusInFlight.current) {
      statusQueued.current = true;
      return;
    }
    statusInFlight.current = true;
    setStatusRefreshing(true);
    try {
      do {
        statusQueued.current = false;
        if (networkOperationRef.current) {
          statusQueued.current = true;
          break;
        }
        try {
          const result = await fetchStatus(repository);
          if (!result.ok) {
            setError(result);
            break;
          }
          setStatus(result.data);
          const operationState = await refreshOperationState(repository);
          if (operationState.ok) setError(null);
        } catch (cause) {
          setError({
            ok: false,
            operation: "Read repository status",
            args: [],
            exitCode: null,
            stderr: cause instanceof Error ? cause.message : String(cause),
            stdout: "",
          });
          break;
        }
      } while (statusQueued.current);
    } finally {
      statusInFlight.current = false;
      setStatusRefreshing(false);
    }
  }, [fetchStatus, refreshOperationState, repository]);

  const refreshBranches = useCallback(async () => {
    if (!repository) return;
    branchesRepository.current = repository;
    if (networkOperationRef.current) {
      branchesQueued.current = true;
      return;
    }
    if (loadingRef.current) return;
    if (branchesInFlight.current) {
      branchesQueued.current = true;
      return;
    }
    branchesInFlight.current = true;
    setBranchesRefreshing(true);
    try {
      do {
        branchesQueued.current = false;
        if (networkOperationRef.current) {
          branchesQueued.current = true;
          break;
        }
        const path: string | null = branchesRepository.current;
        if (!path) break;
        try {
          const result = await invoke<CommandResult<BranchList>>("list_branches", { repository: path });
          if (!result.ok) {
            if (branchesRepository.current === path) setError(result);
            if (branchesQueued.current) continue;
            break;
          }
          if (branchesRepository.current === path) {
            setBranches(result.data);
            setError((current) => current && ["List branches", "Parse branches", "Read current branch", "Read detached HEAD"].includes(current.operation)
              ? null
              : current);
          }
        } catch (cause) {
          if (branchesRepository.current === path) {
            setError({
              ok: false,
              operation: "List branches",
              args: ["-C", path, "for-each-ref"],
              exitCode: null,
              stderr: cause instanceof Error ? cause.message : String(cause),
              stdout: "",
            });
          }
          if (branchesQueued.current) continue;
          break;
        }
      } while (branchesQueued.current);
    } finally {
      branchesInFlight.current = false;
      setBranchesRefreshing(false);
    }
  }, [repository]);

  const runBranchCommand = useCallback(async (
    command: "create_branch" | "switch_branch" | "rename_branch" | "delete_branch",
    args: Record<string, string | boolean | null>,
  ): Promise<boolean> => {
    if (!repository) return false;
    setError(null);
    try {
      const result = await invoke<CommandResult<null>>(command, { repository, ...args });
      if (!result.ok) {
        setError(result);
        return false;
      }
      await refreshStatus();
      await refreshBranches();
      return true;
    } catch (cause) {
      setError({
        ok: false,
        operation: command.replace("_", " "),
        args: ["-C", repository],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return false;
    }
  }, [refreshBranches, refreshStatus, repository]);

  const createBranch = useCallback((name: string, startPoint: string | null, switchTo: boolean) => (
    runBranchCommand("create_branch", { name, startPoint, switchTo })
  ), [runBranchCommand]);

  const switchBranch = useCallback((name: string, trackRemote = false) => (
    runBranchCommand("switch_branch", { name, trackRemote })
  ), [runBranchCommand]);

  const renameBranch = useCallback((oldName: string, newName: string) => (
    runBranchCommand("rename_branch", { oldName, newName })
  ), [runBranchCommand]);

  const deleteBranch = useCallback((name: string, force = false) => (
    runBranchCommand("delete_branch", { name, force })
  ), [runBranchCommand]);

  const refresh = useCallback(async () => {
    const detected = await detectGit();
    if (detected.ok && repository) {
      await loadRepository(repository, false);
    }
  }, [detectGit, loadRepository, repository]);

  useEffect(() => {
    if (repository && !loading) void refreshBranches();
  }, [loading, refreshBranches, repository]);

  const changeFiles = useCallback(async (operation: WorkingTreeOperation, paths: string[]) => {
    if (!repository) return false;
    setError(null);
    try {
      const result = await invoke<CommandResult<null>>("change_files", {
        repository,
        operation,
        paths,
      });
      if (!result.ok) {
        setError(result);
        return false;
      }
      await refreshStatus();
      return true;
    } catch (cause) {
      setError({
        ok: false,
        operation,
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return false;
    }
  }, [refreshStatus, repository]);

  const loadDiff = useCallback(async (
    path: string,
    staged: boolean,
    untracked: boolean,
    loadLarge = false,
  ): Promise<DiffPreview | null> => {
    if (!repository) return null;
    setError(null);
    try {
      const result = await invoke<CommandResult<DiffPreview>>("load_diff", {
        repository,
        path,
        staged,
        untracked,
        loadLarge,
      });
      if (!result.ok) {
        setError(result);
        return null;
      }
      return result.data;
    } catch (cause) {
      setError({
        ok: false,
        operation: "Load diff",
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return null;
    }
  }, [repository]);

  const listHistory = useCallback(async (
    revision: string | null,
    allRefs: boolean,
    search: string,
    searchBy: "message" | "author" | "hash",
    path: string | null,
    offset: number,
    limit: number,
  ): Promise<CommandResult<HistoryPage>> => {
    if (!repository) {
      return { ok: false, operation: "List history", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    return invokeHistory<HistoryPage>("list_history", {
      repository,
      options: { revision, allRefs, search: search || null, searchBy, path, offset, limit },
    }, "List history", ["-C", repository, "log"]);
  }, [repository]);

  const listUnpushedCommits = useCallback(async (): Promise<CommandResult<string[]>> => {
    if (!repository) {
      return { ok: false, operation: "List unpushed commits", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    return invokeHistory<string[]>("list_unpushed_commits", { repository },
      "List unpushed commits", ["-C", repository, "rev-list", "@{upstream}..HEAD"]);
  }, [repository]);

  const getHistoryFingerprint = useCallback(async (): Promise<CommandResult<HistoryFingerprint>> => {
    if (!repository) {
      return { ok: false, operation: "Check history changes", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    return invokeHistory<HistoryFingerprint>("history_fingerprint", { repository },
      "Check history changes", ["-C", repository, "rev-parse", "HEAD"]);
  }, [repository]);

  const loadHistoryDetail = useCallback(async (
    hash: string,
    parent: string | null,
  ): Promise<CommandResult<HistoryDetail>> => {
    if (!repository) {
      return { ok: false, operation: "Read commit details", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    return invokeHistory<HistoryDetail>("load_history_detail", { repository, hash, parent },
      "Read commit details", ["-C", repository, "show", hash]);
  }, [repository]);

  const loadHistoryDiff = useCallback(async (
    hash: string,
    parent: string | null,
    path: string | null,
    loadLarge: boolean,
  ): Promise<CommandResult<DiffPreview>> => {
    if (!repository) {
      return { ok: false, operation: "Load commit diff", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    return invokeHistory<DiffPreview>("load_history_diff", { repository, hash, parent, path, loadLarge },
      "Load commit diff", ["-C", repository, "show", hash, ...(path ? ["--", path] : [])]);
  }, [repository]);

  const loadCommitContext = useCallback(async (): Promise<CommitContext | null> => {
    if (!repository) return null;
    try {
      const result = await invoke<CommandResult<CommitContext>>("load_commit_context", { repository });
      if (!result.ok) {
        setError(result);
        return null;
      }
      return result.data;
    } catch (cause) {
      setError({
        ok: false,
        operation: "Load commit context",
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return null;
    }
  }, [repository]);

  const createCommit = useCallback(async (message: string, amend: boolean): Promise<CommitSummary | null> => {
    if (!repository) return null;
    setError(null);
    try {
      const result = await invoke<CommandResult<CommitSummary>>("create_commit", {
        repository,
        message,
        amend,
      });
      if (!result.ok) {
        setError(result);
        return null;
      }
      await refreshStatus();
      await refreshBranches();
      return result.data;
    } catch (cause) {
      setError({
        ok: false,
        operation: amend ? "Amend commit" : "Commit",
        args: ["-C", repository, "commit", amend ? "--amend" : "", "-F", "-"].filter(Boolean),
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return null;
    }
  }, [refreshBranches, refreshStatus, repository]);

  const listRemotes = useCallback(async (): Promise<string[] | null> => {
    if (!repository) return null;
    try {
      const result = await invoke<CommandResult<string[]>>("list_remotes", { repository });
      if (!result.ok) {
        setError(result);
        return null;
      }
      return result.data;
    } catch (cause) {
      setError({
        ok: false,
        operation: "List remotes",
        args: ["-C", repository, "remote"],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return null;
    }
  }, [repository]);

  const listRemoteDetails = useCallback(async (): Promise<RemoteInfo[] | null> => {
    if (!repository) return null;
    try {
      const result = await invoke<CommandResult<RemoteInfo[]>>("list_remote_details", { repository });
      if (!result.ok) {
        setError(result);
        return null;
      }
      return result.data;
    } catch (cause) {
      setError({
        ok: false,
        operation: "List remotes",
        args: ["-C", repository, "remote", "-v"],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
      return null;
    }
  }, [repository]);

  const runNetworkOperation = useCallback(async (
    operation: "fetch" | "pull" | "push",
    remote: string | null = null,
    forceWithLease = false,
    strategy: "merge" | "rebase" | "ff-only" | null = null,
  ): Promise<PushOutput | null> => {
    if (!repository || networkOperationRef.current) return null;
    const label = operation[0].toUpperCase() + operation.slice(1) as "Fetch" | "Pull" | "Push";
    const startedAt = Date.now();
    networkOperationRef.current = true;
    setNetworkOperation(label);
    setError(null);
    let failure: CommandFailure | null = null;
    let output: PushOutput | null = null;
    try {
      const result = await invoke<CommandResult<PushOutput>>(operation, {
        repository,
        ...(operation === "push" ? { remote, forceWithLease } : {}),
        ...(operation === "pull" ? { strategy } : {}),
      });
      if (result.ok) {
        output = result.data;
        if (operation === "fetch") setLastFetchState({ repository, timestamp: Date.now() });
      } else {
        failure = result;
      }
    } catch (cause) {
      failure = {
        ok: false,
        operation: label,
        args: ["-C", repository, operation],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      };
    } finally {
      setNetworkDurationMs(Date.now() - startedAt);
      networkOperationRef.current = false;
      setNetworkOperation(null);
    }
    await refreshStatus();
    await refreshBranches();
    if (failure) setError(failure);
    return output;
  }, [refreshBranches, refreshStatus, repository]);

  const cancelNetworkOperation = useCallback(async () => {
    if (!repository || !networkOperationRef.current) return;
    try {
      const result = await invoke<CommandResult<boolean>>("cancel_network_operation", { repository });
      if (!result.ok) setError(result);
    } catch (cause) {
      setError({
        ok: false,
        operation: "Cancel network operation",
        args: ["-C", repository],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
    }
  }, [repository]);

  const pushChanges = useCallback(async (
    remote: string | null,
    forceWithLease: boolean,
  ): Promise<PushOutput | null> => {
    return runNetworkOperation("push", remote, forceWithLease);
  }, [runNetworkOperation]);

  const previewAdvanced = useCallback(async (
    operation: AdvancedRequest["operation"],
    target: string,
  ): Promise<CommandResult<AdvancedPreview>> => {
    if (!repository) {
      return { ok: false, operation: "Preview advanced Git operation", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    const result = await invokeHistory<AdvancedPreview>("advanced_preview", {
      repository,
      operation,
      target,
    }, "Preview advanced Git operation", ["-C", repository, "rev-list", "--count"]);
    if (!result.ok) setError(result);
    return result;
  }, [repository]);

  const runAdvancedOperation = useCallback(async (
    request: AdvancedRequest,
  ): Promise<CommandResult<AdvancedOutput>> => {
    if (!repository) {
      return { ok: false, operation: "Run advanced Git operation", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    setError(null);
    advancedOperationRef.current = true;
    const result = await invokeHistory<AdvancedOutput>("run_advanced_operation", {
      repository,
      request,
    }, request.operation, ["-C", repository, request.operation, request.target]);
    advancedOperationRef.current = false;
    if (!result.ok) {
      await refreshOperationState(repository);
      await refreshStatus();
      setError(result);
      return result;
    }
    await refreshStatus();
    await refreshBranches();
    return result;
  }, [refreshBranches, refreshOperationState, refreshStatus, repository]);

  const runOperationAction = useCallback(async (
    request: OperationActionRequest,
  ): Promise<CommandResult<AdvancedOutput>> => {
    if (!repository) {
      return { ok: false, operation: "Continue repository operation", args: [], exitCode: null, stderr: "No repository is open.", stdout: "" };
    }
    setError(null);
    advancedOperationRef.current = true;
    const result = await invokeHistory<AdvancedOutput>("run_operation_action", {
      repository,
      request,
    }, `${request.operation} ${request.action}`, ["-C", repository, request.operation, `--${request.action}`]);
    advancedOperationRef.current = false;
    await refreshStatus();
    await refreshBranches();
    if (!result.ok) setError(result);
    return result;
  }, [refreshBranches, refreshStatus, repository]);

  const chooseRepository = useCallback(async () => {
    try {
      const selection = await open({ directory: true, multiple: false, title: "Open Git repository" });
      if (typeof selection === "string") await loadRepository(selection, true);
    } catch (cause) {
      setError({
        ok: false,
        operation: "Choose repository",
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
    }
  }, [loadRepository]);

  const reportError = useCallback((failure: CommandFailure) => setError(failure), []);

  useEffect(() => {
    let active = true;
    const bootstrap = async () => {
      const detected = await detectGit();
      if (!active || !detected.ok) return;
      try {
        const store = await load(SETTINGS_FILE, { autoSave: true });
        const previous = await store.get<string>(LAST_REPOSITORY_KEY);
        if (active && previous) await loadRepository(previous, false);
      } catch (cause) {
        if (active) {
          setError({
            ok: false,
            operation: "Restore last repository",
            args: [],
            exitCode: null,
            stderr: cause instanceof Error ? cause.message : String(cause),
            stdout: "",
          });
        }
      }
    };
    void bootstrap();
    return () => { active = false; };
  }, [detectGit, loadRepository]);

  useEffect(() => {
    let timer: number | undefined;
    const onFocus = () => {
      if (timer !== undefined) window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        void refreshStatus();
        void refreshBranches();
      }, 250);
    };
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [refreshBranches, refreshStatus]);

  useEffect(() => {
    if (!repository) return;
    const timer = window.setInterval(() => {
      if (!loadingRef.current && !networkOperationRef.current && !advancedOperationRef.current) {
        void refreshOperationState(repository);
      }
    }, 1500);
    return () => window.clearInterval(timer);
  }, [refreshOperationState, repository]);

  const refreshCurrentOperationState = useCallback(
    () => repository ? refreshOperationState(repository) : Promise.resolve(null),
    [refreshOperationState, repository],
  );

  return {
    git,
    repository,
    status,
    branches,
    operationState: operationStateValue?.repository === repository ? operationStateValue.value : null,
    branchesRefreshing,
    error,
    loading,
    statusRefreshing,
    networkOperation,
    networkDurationMs,
    lastFetchAt: lastFetchState?.repository === repository ? lastFetchState.timestamp : null,
    chooseRepository,
    refresh,
    refreshStatus,
    refreshBranches,
    changeFiles,
    loadDiff,
    listHistory,
    listUnpushedCommits,
    getHistoryFingerprint,
    loadHistoryDetail,
    loadHistoryDiff,
    loadCommitContext,
    refreshOperationState: refreshCurrentOperationState,
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
    clearError: () => setError(null),
    reportError,
  };
}
