import { useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  Circle,
  Maximize2,
  Minus,
  Plus,
  RefreshCw,
  TerminalSquare,
  X,
} from "lucide-react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { Button } from "./ui/button";
import { Terminal } from "./Terminal";
import type { TerminalSessionInfo } from "../hooks/useTerminal";
import { buildCdCommand } from "../lib/terminal";

const SHELL_OPTIONS = [
  { value: "pwsh", label: "PowerShell 7" },
  { value: "powershell", label: "Windows PowerShell" },
  { value: "git-bash", label: "Git Bash" },
  { value: "cmd", label: "Command Prompt" },
];

function samePath(left: string, right: string) {
  const normalize = (value: string) => value.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
  return normalize(left) === normalize(right);
}

export function TerminalDrawer({
  open,
  repository,
  sessions,
  activeSessionId,
  focusToken,
  preferredShell,
  failure,
  lightTheme,
  onToggle,
  onSetActive,
  onPreferredShellChange,
  onCreate,
  onWrite,
  onWriteBinary,
  onResize,
  onClose,
  onClearHistory,
  onRegisterOutput,
  onPromptSubmit,
  onTerminalFocus,
  onRefresh,
  onError,
}: {
  open: boolean;
  repository: string | null;
  sessions: TerminalSessionInfo[];
  activeSessionId: string | null;
  focusToken: number;
  preferredShell: string;
  failure: string | null;
  lightTheme: boolean;
  onToggle: () => void;
  onSetActive: (id: string) => void;
  onPreferredShellChange: (shell: string) => void;
  onCreate: () => Promise<TerminalSessionInfo | null>;
  onWrite: (id: string, data: string) => void;
  onWriteBinary: (id: string, data: string) => void;
  onResize: (id: string, cols: number, rows: number) => Promise<void>;
  onClose: (session: TerminalSessionInfo) => Promise<void>;
  onClearHistory: (id: string) => void;
  onRegisterOutput: (id: string, handler: (data: string) => void) => () => void;
  onPromptSubmit: (id: string) => void;
  onTerminalFocus: () => void;
  onRefresh: () => void;
  onError: (message: string) => void;
}) {
  const [height, setHeight] = useState(() => {
    const saved = Number(localStorage.getItem("terminal-height"));
    return Number.isFinite(saved) && saved >= 180 && saved <= 720 ? saved : 320;
  });
  const [maximized, setMaximized] = useState(false);
  const [fontSize, setFontSize] = useState(() => {
    const saved = Number(localStorage.getItem("terminal-font-size"));
    return Number.isFinite(saved) && saved >= 8 && saved <= 28 ? saved : 13;
  });
  const [scrollback, setScrollback] = useState(() => {
    const saved = Number(localStorage.getItem("terminal-scrollback"));
    return [5000, 10000, 20000].includes(saved) ? saved : 10000;
  });
  const [clearToken, setClearToken] = useState(0);
  const dragState = useRef<{ y: number; height: number } | null>(null);
  const activeSession = sessions.find((session) => session.id === activeSessionId) ?? sessions[0] ?? null;

  useEffect(() => localStorage.setItem("terminal-height", String(height)), [height]);
  useEffect(() => localStorage.setItem("terminal-font-size", String(fontSize)), [fontSize]);
  useEffect(() => localStorage.setItem("terminal-scrollback", String(scrollback)), [scrollback]);

  function beginResize(event: ReactPointerEvent<HTMLDivElement>) {
    if (maximized) return;
    event.preventDefault();
    dragState.current = { y: event.clientY, height };
    const move = (moveEvent: globalThis.PointerEvent) => {
      if (!dragState.current) return;
      setHeight(Math.max(180, Math.min(720, dragState.current.height + dragState.current.y - moveEvent.clientY)));
    };
    const end = () => {
      dragState.current = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
  }

  async function closeTab(session: TerminalSessionInfo) {
    if (session.running && !window.confirm("Fechar este terminal e encerrar o shell e todos os processos filhos em execução?")) return;
    await onClose(session);
  }

  return (
    <aside
      className={`terminal-drawer ${open ? "terminal-drawer-open" : "terminal-drawer-closed"} ${maximized ? "terminal-drawer-maximized" : ""}`}
      style={{ height: maximized ? undefined : height }}
      aria-label="Terminal integrado"
      aria-hidden={!open}
    >
      <div className="terminal-resize-handle" role="separator" aria-label="Redimensionar painel do terminal" onPointerDown={beginResize} />
      <header className="terminal-toolbar">
        <div className="terminal-tabs" role="tablist" aria-label="Sessões do terminal">
          {sessions.map((session) => (
            <div className={`terminal-tab ${session.id === activeSession?.id ? "terminal-tab-active" : ""}`} key={session.id}>
              <button type="button" role="tab" aria-selected={session.id === activeSession?.id} onClick={() => onSetActive(session.id)}>
                <Circle size={8} fill={session.running ? "currentColor" : "none"} />
                <span>{session.shell}</span>
                {session.activity && session.id !== activeSession?.id && <span className="terminal-tab-activity" aria-label="Nova saída do terminal" />}
              </button>
              <button type="button" className="terminal-tab-close" aria-label={`Fechar sessão ${session.shell}`} onClick={() => void closeTab(session)}>
                <X size={12} />
              </button>
            </div>
          ))}
          <Button variant="ghost" size="icon" aria-label="Novo terminal" title="Novo terminal" disabled={!repository || sessions.length >= 8} onClick={() => void onCreate().then((session) => {
            if (session) onSetActive(session.id);
          })}><Plus size={15} /></Button>
        </div>
        <div className="terminal-toolbar-actions">
          <span className="terminal-shell-label">{activeSession?.shell ?? "Terminal"}</span>
          <select
            aria-label="Shell padrão do terminal"
            value={preferredShell}
            onChange={(event) => onPreferredShellChange(event.target.value)}
            title="Shell usado em novos terminais"
          >
            {SHELL_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </select>
          <select aria-label="Histórico do terminal" value={scrollback} onChange={(event) => setScrollback(Number(event.target.value))}>
            <option value={5000}>5 mil linhas</option>
            <option value={10000}>10 mil linhas</option>
            <option value={20000}>20 mil linhas</option>
          </select>
          <Button variant="ghost" size="icon" aria-label="Atualizar estado do repositório" title="Atualizar estado do repositório" onClick={onRefresh}><RefreshCw size={14} /></Button>
          <Button variant="ghost" size="icon" aria-label="Limpar terminal" title="Limpar terminal (Ctrl+L também funciona)" disabled={!activeSession} onClick={() => {
            if (!activeSession) return;
            setClearToken((token) => token + 1);
            onClearHistory(activeSession.id);
          }}><Minus size={15} /></Button>
          <Button variant="ghost" size="icon" aria-label="Diminuir tamanho da fonte do terminal" title="Diminuir tamanho da fonte" onClick={() => setFontSize((size) => Math.max(8, size - 1))}><span className="terminal-zoom-icon">A−</span></Button>
          <Button variant="ghost" size="icon" aria-label="Aumentar tamanho da fonte do terminal" title="Aumentar tamanho da fonte" onClick={() => setFontSize((size) => Math.min(28, size + 1))}><span className="terminal-zoom-icon">A+</span></Button>
          <Button variant="ghost" size="icon" aria-label="Redefinir tamanho da fonte do terminal" title="Redefinir tamanho da fonte" onClick={() => setFontSize(13)}><span className="terminal-zoom-icon">A</span></Button>
          <Button variant="ghost" size="icon" aria-label={maximized ? "Restaurar tamanho do terminal" : "Maximizar terminal"} title={maximized ? "Restaurar" : "Maximizar"} onClick={() => setMaximized((value) => !value)}>
            {maximized ? <ChevronDown size={15} /> : <Maximize2 size={14} />}
          </Button>
          <Button variant="ghost" size="icon" aria-label={open ? "Recolher terminal" : "Expandir terminal"} title={open ? "Recolher terminal" : "Expandir terminal"} onClick={onToggle}>
            {open ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
          </Button>
        </div>
      </header>
      {failure && <div className="terminal-error" role="alert">{failure}</div>}
      {activeSession && repository && !samePath(activeSession.cwd, repository) && (
        <div className="terminal-repository-notice" role="status">
          <span>O repositório mudou</span>
          <button type="button" onClick={() => {
            try {
              onWrite(activeSession.id, buildCdCommand(activeSession.shell, repository));
            } catch (cause) {
              onError(cause instanceof Error ? cause.message : String(cause));
            }
          }}>Alterar para <code>{repository}</code></button>
        </div>
      )}
      {activeSession?.notice && <div className="terminal-repository-notice" role="status">{activeSession.notice}</div>}
      <div className="terminal-body">
        {activeSession ? sessions.map((session) => (
          <Terminal
            key={session.id}
            session={session}
            active={open && session.id === activeSession.id}
            focusToken={focusToken}
            lightTheme={lightTheme}
            fontSize={fontSize}
            scrollback={scrollback}
            registerOutput={onRegisterOutput}
            write={onWrite}
            writeBinary={onWriteBinary}
            resize={onResize}
            onPromptSubmit={onPromptSubmit}
            onTerminalFocus={onTerminalFocus}
            onFontSizeChange={setFontSize}
            clearToken={clearToken}
            onError={onError}
          />
        )) : (
          <div className="terminal-empty">
            <TerminalSquare size={22} />
            <span>{repository ? "Crie uma sessão do terminal para começar." : "Abra um repositório para iniciar o terminal na pasta correspondente."}</span>
            <Button disabled={!repository} onClick={() => void onCreate().then((session) => {
              if (session) onSetActive(session.id);
            })}><Plus size={14} />Novo terminal</Button>
          </div>
        )}
        {activeSession && !activeSession.running && (
          <div className="terminal-exited" role="status">
            <span>O shell foi encerrado{activeSession.exitCode === null ? "." : ` com código ${activeSession.exitCode}.`}</span>
            <Button variant="outline" onClick={() => void onCreate().then((session) => {
              if (session) onSetActive(session.id);
            })}>Reiniciar</Button>
          </div>
        )}
      </div>
    </aside>
  );
}
