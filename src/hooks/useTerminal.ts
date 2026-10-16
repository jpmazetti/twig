import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CommandFailure, CommandResult } from "../types/git";

export interface TerminalSessionInfo {
  id: string;
  shell: string;
  cwd: string;
  cols: number;
  rows: number;
  running: boolean;
  exitCode: number | null;
  notice: string | null;
  activity?: boolean;
}

interface TerminalOutputEvent {
  sessionId: string;
  data: string;
}

interface TerminalExitEvent {
  sessionId: string;
  exitCode: number | null;
}

const MAX_FRONTEND_HISTORY = 1_000_000;

function mergeStreamedHistory(snapshot: string, streamed: string) {
  if (!streamed) return snapshot;
  const maxOverlap = Math.min(snapshot.length, streamed.length);
  for (let overlap = maxOverlap; overlap > 0; overlap -= 1) {
    if (snapshot.endsWith(streamed.slice(0, overlap))) {
      return trimHistory(snapshot + streamed.slice(overlap));
    }
  }
  return trimHistory(snapshot + streamed);
}

function trimHistory(value: string) {
  if (value.length <= MAX_FRONTEND_HISTORY) return value;
  let start = value.length - MAX_FRONTEND_HISTORY;
  if (start > 0 && value.charCodeAt(start - 1) >= 0xd800 && value.charCodeAt(start - 1) <= 0xdbff) start += 1;
  return value.slice(start);
}

function failureMessage(failure: CommandFailure): string {
  return `${failure.operation} failed: ${failure.stderr || `Exit code: ${failure.exitCode ?? "unknown"}`}`;
}

export function useTerminal() {
  const [sessions, setSessions] = useState<TerminalSessionInfo[]>([]);
  const [failure, setFailure] = useState<string | null>(null);
  const [initialized, setInitialized] = useState(false);
  const sessionsRef = useRef(sessions);
  const outputHistory = useRef(new Map<string, string>());
  const outputHandlers = useRef(new Map<string, (data: string) => void>());
  const writeQueues = useRef(new Map<string, Promise<void>>());
  const ready = useRef<Promise<void> | null>(null);

  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  useEffect(() => {
    let active = true;
    let unlisteners: UnlistenFn[] = [];
    const setup = async () => {
      const [stopOutput, stopExit, stopReaderError] = await Promise.all([
        listen<TerminalOutputEvent>("terminal-output", ({ payload }) => {
          const oldHistory = outputHistory.current.get(payload.sessionId) ?? "";
          const nextHistory = trimHistory(oldHistory + payload.data);
          outputHistory.current.set(payload.sessionId, nextHistory);
          outputHandlers.current.get(payload.sessionId)?.(payload.data);
          if (sessionsRef.current.some((session) => session.id === payload.sessionId)) {
            setSessions((current) => current.some((session) => session.id === payload.sessionId && !session.activity)
              ? current.map((session) => session.id === payload.sessionId ? { ...session, activity: true } : session)
              : current);
          }
        }),
        listen<TerminalExitEvent>("terminal-exit", ({ payload }) => {
          setSessions((current) => current.map((session) => session.id === payload.sessionId
            ? { ...session, running: false, exitCode: payload.exitCode, activity: true }
            : session));
        }),
        listen<{ sessionId: string }>("terminal-reader-error", ({ payload }) => {
          setFailure(`Terminal output reader failed for session ${payload.sessionId}.`);
        }),
      ]);
      if (!active) {
        stopOutput();
        stopExit();
        stopReaderError();
        return;
      }
      unlisteners = [stopOutput, stopExit, stopReaderError];
      const result = await invoke<CommandResult<TerminalSessionInfo[]>>("terminal_list");
      if (!active) return;
      if (!result.ok) {
        setFailure(failureMessage(result));
        setInitialized(true);
        return;
      }
      await Promise.all(result.data.map(async (session) => {
        const history = await invoke<CommandResult<string>>("terminal_history", { id: session.id });
        if (!active) return;
        if (history.ok) {
          const streamed = outputHistory.current.get(session.id) ?? "";
          outputHistory.current.set(session.id, mergeStreamedHistory(history.data, streamed));
        } else setFailure(failureMessage(history));
      }));
      if (active) {
        setSessions(result.data);
        setInitialized(true);
      }
    };
    ready.current = setup();
    void ready.current.catch((cause: unknown) => {
      if (active) {
        setFailure(cause instanceof Error ? cause.message : String(cause));
        setInitialized(true);
      }
    });
    return () => {
      active = false;
      for (const unlisten of unlisteners) unlisten();
      ready.current = null;
    };
  }, []);

  const createSession = useCallback(async (
    cwd: string,
    shell: string | null,
  ): Promise<TerminalSessionInfo | null> => {
    if (ready.current) await ready.current;
    setFailure(null);
    const result = await invoke<CommandResult<TerminalSessionInfo>>("terminal_create", {
      cwd,
      shell,
      cols: 100,
      rows: 30,
    });
    if (!result.ok) {
      setFailure(failureMessage(result));
      return null;
    }
    if (!outputHistory.current.has(result.data.id)) outputHistory.current.set(result.data.id, "");
    setSessions((current) => [...current, result.data]);
    return result.data;
  }, []);

  const enqueueWrite = useCallback((id: string, operation: () => Promise<CommandResult<null>>) => {
    const previous = writeQueues.current.get(id) ?? Promise.resolve();
    const next = previous.then(async () => {
      const result = await operation();
      if (!result.ok) setFailure(failureMessage(result));
    }).catch((cause: unknown) => {
      setFailure(cause instanceof Error ? cause.message : String(cause));
    });
    writeQueues.current.set(id, next);
  }, []);

  const write = useCallback((id: string, data: string) => {
    enqueueWrite(id, () => invoke<CommandResult<null>>("terminal_write", { id, data }));
  }, [enqueueWrite]);

  const writeBinary = useCallback((id: string, data: string) => {
    const bytes = Array.from(data, (character) => character.charCodeAt(0));
    enqueueWrite(id, () => invoke<CommandResult<null>>("terminal_write_bytes", { id, data: bytes }));
  }, [enqueueWrite]);

  const resize = useCallback(async (id: string, cols: number, rows: number) => {
    const result = await invoke<CommandResult<null>>("terminal_resize", { id, cols, rows });
    if (!result.ok) setFailure(failureMessage(result));
  }, []);

  const closeSession = useCallback(async (id: string) => {
    setFailure(null);
    const result = await invoke<CommandResult<null>>("terminal_close", { id });
    if (!result.ok) {
      setFailure(failureMessage(result));
      return false;
    }
    outputHandlers.current.delete(id);
    outputHistory.current.delete(id);
    writeQueues.current.delete(id);
    setSessions((current) => current.filter((session) => session.id !== id));
    return true;
  }, []);

  const closeAll = useCallback(async () => {
    const result = await invoke<CommandResult<null>>("terminal_close_all");
    if (!result.ok) {
      setFailure(failureMessage(result));
      return false;
    }
    outputHandlers.current.clear();
    outputHistory.current.clear();
    writeQueues.current.clear();
    setSessions([]);
    return true;
  }, []);

  const clearHistory = useCallback(async (id: string) => {
    const result = await invoke<CommandResult<null>>("terminal_clear_history", { id });
    if (!result.ok) setFailure(failureMessage(result));
    outputHistory.current.set(id, "");
  }, []);

  const registerOutput = useCallback((id: string, handler: (data: string) => void) => {
    outputHandlers.current.set(id, handler);
    handler(outputHistory.current.get(id) ?? "");
    return () => {
      if (outputHandlers.current.get(id) === handler) outputHandlers.current.delete(id);
    };
  }, []);

  const clearActivity = useCallback((id: string) => {
    setSessions((current) => current.map((session) => session.id === id
      ? { ...session, activity: false }
      : session));
  }, []);

  return {
    sessions,
    failure,
    initialized,
    createSession,
    write,
    writeBinary,
    resize,
    closeSession,
    closeAll,
    clearHistory,
    registerOutput,
    clearActivity,
  };
}
