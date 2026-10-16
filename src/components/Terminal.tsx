import { useCallback, useEffect, useRef, useState } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { TerminalSessionInfo } from "../hooks/useTerminal";
import "@xterm/xterm/css/xterm.css";

export function Terminal({
  session,
  active,
  focusToken,
  lightTheme,
  fontSize,
  scrollback,
  registerOutput,
  write,
  writeBinary,
  resize,
  onPromptSubmit,
  onTerminalFocus,
  onFontSizeChange,
  clearToken,
  onError,
}: {
  session: TerminalSessionInfo;
  active: boolean;
  focusToken: number;
  lightTheme: boolean;
  fontSize: number;
  scrollback: number;
  registerOutput: (id: string, handler: (data: string) => void) => () => void;
  write: (id: string, data: string) => void;
  writeBinary: (id: string, data: string) => void;
  resize: (id: string, cols: number, rows: number) => Promise<void>;
  onPromptSubmit: (id: string) => void;
  onTerminalFocus: () => void;
  onFontSizeChange: (size: number) => void;
  clearToken: number;
  onError: (message: string) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<XTerm | null>(null);
  const fitAddon = useRef<FitAddon | null>(null);
  const searchAddon = useRef<SearchAddon | null>(null);
  const resizeTimer = useRef<number | null>(null);
  const config = useRef({ lightTheme, fontSize, scrollback });
  const lastSize = useRef({ cols: session.cols, rows: session.rows });
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchValue, setSearchValue] = useState("");
  const [selectionAvailable, setSelectionAvailable] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const fitAndResize = useCallback(() => {
    const instance = terminal.current;
    const fit = fitAddon.current;
    if (!instance || !fit || !host.current || host.current.clientWidth === 0 || host.current.clientHeight === 0) return;
    try {
      fit.fit();
      if (instance.cols !== lastSize.current.cols || instance.rows !== lastSize.current.rows) {
        lastSize.current = { cols: instance.cols, rows: instance.rows };
        if (resizeTimer.current !== null) window.clearTimeout(resizeTimer.current);
        resizeTimer.current = window.setTimeout(() => {
          void resize(session.id, instance.cols, instance.rows);
        }, 80);
      }
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [onError, resize, session.id]);

  useEffect(() => {
    config.current = { lightTheme, fontSize, scrollback };
  }, [fontSize, lightTheme, scrollback]);

  useEffect(() => {
    if (!host.current) return;
    const instance = new XTerm({
      allowProposedApi: false,
      cursorBlink: true,
      fontFamily: '"Cascadia Code", "JetBrains Mono", Consolas, monospace',
      fontSize: config.current.fontSize,
      scrollback: config.current.scrollback,
      theme: config.current.lightTheme
        ? {
          background: "#f7f8fa",
          foreground: "#242630",
          cursor: "#5268d8",
          selectionBackground: "#5268d866",
        }
        : {
          background: "#111318",
          foreground: "#e7e8ec",
          cursor: "#9aaaff",
          selectionBackground: "#8b9cff66",
        },
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    const links = new WebLinksAddon((_event, link) => {
      let url: URL;
      try {
        url = new URL(link);
      } catch {
        return;
      }
      if (url.protocol !== "http:" && url.protocol !== "https:") return;
      if (!window.confirm(`Abrir este link no navegador?\n${url.href}`)) return;
      void openUrl(url.href).catch((cause: unknown) => {
        onError(cause instanceof Error ? cause.message : String(cause));
      });
    });
    instance.loadAddon(fit);
    instance.loadAddon(search);
    instance.loadAddon(links);
    instance.parser.registerOscHandler(52, () => true);
    instance.open(host.current);
    instance.onData((data) => {
      write(session.id, data);
      if (data.includes("\r")) onPromptSubmit(session.id);
    });
    const binaryListener = instance.onBinary((data) => writeBinary(session.id, data));
    const selectionListener = instance.onSelectionChange(() => setSelectionAvailable(instance.hasSelection()));
    terminal.current = instance;
    fitAddon.current = fit;
    searchAddon.current = search;
    const unregister = registerOutput(session.id, (data) => instance.write(data));

    const handleKeys = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if (event.ctrlKey && event.shiftKey && key === "c") {
        const selection = instance.getSelection();
        if (selection) {
          event.preventDefault();
          event.stopPropagation();
          void navigator.clipboard.writeText(selection).catch((cause: unknown) => {
            onError(cause instanceof Error ? cause.message : String(cause));
          });
        }
        return;
      }
      if (event.ctrlKey && event.shiftKey && key === "v") {
        event.preventDefault();
        event.stopPropagation();
        void navigator.clipboard.readText().then((text) => {
          const lineCount = text.split(/\r\n|\r|\n/).length;
          if (lineCount > 20 && !window.confirm(`Paste ${lineCount} lines into the terminal?`)) return;
          instance.paste(text);
        }).catch((cause: unknown) => {
          onError(cause instanceof Error ? cause.message : String(cause));
        });
        return;
      }
      if (event.ctrlKey && !event.shiftKey && key === "c" && instance.hasSelection()) {
        event.preventDefault();
        event.stopPropagation();
        void navigator.clipboard.writeText(instance.getSelection()).catch((cause: unknown) => {
          onError(cause instanceof Error ? cause.message : String(cause));
        });
        return;
      }
      if (event.ctrlKey && key === "f") {
        event.preventDefault();
        event.stopPropagation();
        setSearchOpen(true);
        return;
      }
      if (event.ctrlKey && (key === "+" || key === "=")) {
        event.preventDefault();
        onFontSizeChange(Math.min(28, config.current.fontSize + 1));
        return;
      }
      if (event.ctrlKey && key === "-") {
        event.preventDefault();
        onFontSizeChange(Math.max(8, config.current.fontSize - 1));
        return;
      }
      if (event.ctrlKey && key === "0") {
        event.preventDefault();
        onFontSizeChange(13);
      }
    };
    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      setContextMenu({ x: event.clientX, y: event.clientY });
    };
    const handleFocus = () => onTerminalFocus();
    const container = host.current;
    container.addEventListener("keydown", handleKeys, true);
    container.addEventListener("contextmenu", handleContextMenu);
    container.addEventListener("focusin", handleFocus);
    const observer = new ResizeObserver(fitAndResize);
    observer.observe(container);
    requestAnimationFrame(fitAndResize);

    return () => {
      observer.disconnect();
      container.removeEventListener("keydown", handleKeys, true);
      container.removeEventListener("contextmenu", handleContextMenu);
      container.removeEventListener("focusin", handleFocus);
      binaryListener.dispose();
      selectionListener.dispose();
      unregister();
      if (resizeTimer.current !== null) window.clearTimeout(resizeTimer.current);
      instance.dispose();
      terminal.current = null;
      fitAddon.current = null;
      searchAddon.current = null;
    };
  }, [fitAndResize, onError, onFontSizeChange, onPromptSubmit, onTerminalFocus, registerOutput, resize, session.id, write, writeBinary]);

  useEffect(() => {
    if (terminal.current) terminal.current.options.fontSize = fontSize;
  }, [fontSize]);

  useEffect(() => {
    if (terminal.current) terminal.current.options.scrollback = scrollback;
  }, [scrollback]);

  useEffect(() => {
    if (clearToken > 0 && active) terminal.current?.clear();
  }, [active, clearToken]);

  useEffect(() => {
    if (terminal.current) terminal.current.options.theme = lightTheme
      ? { background: "#f7f8fa", foreground: "#242630", cursor: "#5268d8", selectionBackground: "#5268d866" }
      : { background: "#111318", foreground: "#e7e8ec", cursor: "#9aaaff", selectionBackground: "#8b9cff66" };
  }, [lightTheme]);

  useEffect(() => {
    if (active) {
      requestAnimationFrame(() => {
        fitAndResize();
        terminal.current?.focus();
      });
    }
  }, [active, fitAndResize]);

  useEffect(() => {
    if (active) terminal.current?.focus();
  }, [active, focusToken]);

  useEffect(() => {
    if (!session.running) onPromptSubmit(session.id);
  }, [onPromptSubmit, session.id, session.running]);

  useEffect(() => {
    if (searchOpen) {
      const input = document.getElementById(`terminal-search-${session.id}`);
      input?.focus();
    }
  }, [searchOpen, session.id]);

  const doSearch = (direction: "next" | "previous") => {
    if (!searchValue) return;
    if (direction === "next") searchAddon.current?.findNext(searchValue);
    else searchAddon.current?.findPrevious(searchValue);
  };

  const pasteFromClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      const lineCount = text.split(/\r\n|\r|\n/).length;
      if (lineCount > 20 && !window.confirm(`Paste ${lineCount} lines into the terminal?`)) return;
      terminal.current?.paste(text);
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : String(cause));
    }
    setContextMenu(null);
  };

  return (
    <section className="terminal-session" aria-label={`Terminal session ${session.id}`} hidden={!active}>
      {searchOpen && (
        <div className="terminal-search">
          <label htmlFor={`terminal-search-${session.id}`}>Localizar</label>
          <input
            id={`terminal-search-${session.id}`}
            value={searchValue}
            onChange={(event) => {
              setSearchValue(event.target.value);
              if (event.target.value) searchAddon.current?.findNext(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                doSearch(event.shiftKey ? "previous" : "next");
              } else if (event.key === "Escape") {
                setSearchOpen(false);
                terminal.current?.focus();
              }
            }}
            placeholder="Pesquisar na saída"
          />
          <button type="button" aria-label="Resultado anterior" onClick={() => doSearch("previous")}>↑</button>
          <button type="button" aria-label="Próximo resultado" onClick={() => doSearch("next")}>↓</button>
          <button type="button" aria-label="Fechar pesquisa" onClick={() => {
            setSearchOpen(false);
            terminal.current?.focus();
          }}>×</button>
        </div>
      )}
      <div className="terminal-screen" ref={host} />
      {contextMenu && (
        <div
          className="terminal-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          role="menu"
        >
          <button type="button" role="menuitem" disabled={!selectionAvailable} onClick={() => {
            const selection = terminal.current?.getSelection() ?? "";
            void navigator.clipboard.writeText(selection).catch((cause: unknown) => {
              onError(cause instanceof Error ? cause.message : String(cause));
            });
            setContextMenu(null);
          }}>Copiar seleção</button>
          <button type="button" role="menuitem" onClick={() => void pasteFromClipboard()}>Colar</button>
        </div>
      )}
    </section>
  );
}
