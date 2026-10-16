import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Check,
  ChevronLeft,
  Copy,
  GitBranch,
  GitCommitHorizontal,
  GitMerge,
  LoaderCircle,
  RefreshCw,
  Search,
  Tag,
  X,
} from "lucide-react";
import { DiffView } from "./DiffView";
import { Button } from "./ui/button";
import { formatGitCommand } from "../lib/commit";
import { formatAbsoluteDate, formatRelativeDate } from "../lib/history.js";
import type {
  BranchList,
  CommandFailure,
  CommandResult,
  DiffPreview,
  HistoryCommit,
  HistoryDetail,
  HistoryFingerprint,
  HistoryPage,
} from "../types/git";

type SearchBy = "message" | "author" | "hash";

interface ContextMenu {
  hash: string;
  shortHash: string;
  subject: string;
  parentCount: number;
  path: string | null;
  x: number;
  y: number;
}

interface DiffState {
  key: string;
  diff: DiffPreview;
  allowLarge: boolean;
}

export function HistoryPanel({
  branches,
  refreshKey,
  initialPath,
  initialHash = null,
  onBack,
  listHistory,
  loadHistoryDetail,
  loadHistoryDiff,
  listUnpushedCommits,
  getHistoryFingerprint,
  onOpenTerminal,
  onCherryPick,
  onReset,
  advancedDisabled,
}: {
  branches: BranchList | null;
  refreshKey: string;
  initialPath: string | null;
  initialHash?: string | null;
  onBack: () => void;
  listHistory: (
    revision: string | null,
    allRefs: boolean,
    search: string,
    searchBy: SearchBy,
    path: string | null,
    offset: number,
    limit: number,
  ) => Promise<CommandResult<HistoryPage>>;
  loadHistoryDetail: (hash: string, parent: string | null) => Promise<CommandResult<HistoryDetail>>;
  loadHistoryDiff: (
    hash: string,
    parent: string | null,
    path: string | null,
    loadLarge: boolean,
  ) => Promise<CommandResult<DiffPreview>>;
  listUnpushedCommits: () => Promise<CommandResult<string[]>>;
  getHistoryFingerprint: () => Promise<CommandResult<HistoryFingerprint>>;
  onOpenTerminal: () => void;
  onCherryPick: (hash: string, parentCount: number) => void;
  onReset: (hash: string) => void;
  advancedDisabled: boolean;
}) {
  const [revision, setRevision] = useState<string | null>(null);
  const [allRefs, setAllRefs] = useState(false);
  const [searchBy, setSearchBy] = useState<SearchBy>("message");
  const [searchText, setSearchText] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [filePath, setFilePath] = useState(initialPath ?? "");
  const [commits, setCommits] = useState<HistoryCommit[]>([]);
  const [unpushedHashes, setUnpushedHashes] = useState<Set<string>>(new Set());
  const [hasMore, setHasMore] = useState(false);
  const [pageState, setPageState] = useState<HistoryPage | null>(null);
  const [listLoading, setListLoading] = useState(true);
  const [moreLoading, setMoreLoading] = useState(false);
  const [selectedHash, setSelectedHash] = useState<string | null>(null);
  const [detail, setDetail] = useState<HistoryDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [diffState, setDiffState] = useState<DiffState | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [fullDiffOpen, setFullDiffOpen] = useState(false);
  const [error, setError] = useState<CommandFailure | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenu | null>(null);
  const [copyNotice, setCopyNotice] = useState("");
  const [refreshNonce, setRefreshNonce] = useState(0);
  const [pathDraft, setPathDraft] = useState(initialPath ?? "");
  const scrollRef = useRef<HTMLDivElement>(null);
  const rowRefs = useRef(new Map<number, HTMLDivElement>());
  const fileRowRefs = useRef(new Map<number, HTMLButtonElement>());
  const commitsRef = useRef<HistoryCommit[]>([]);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);
  const diffGeneration = useRef(0);
  const listInFlight = useRef<Promise<CommandResult<HistoryPage>> | null>(null);
  const loadingMoreRef = useRef(false);
  const previousQueryKey = useRef("");

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(searchText.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [searchText]);

  const filterKey = useMemo(() => JSON.stringify([
    revision, allRefs, debouncedSearch, searchBy, filePath.trim(),
  ]), [allRefs, debouncedSearch, filePath, revision, searchBy]);
  const queryKey = `${filterKey}|${refreshKey}|${refreshNonce}`;
  const pageHeadHash = pageState?.headHash;
  const pageRefsHash = pageState?.refsHash;

  const loadPage = useCallback(async (generation: number, append: boolean) => {
    if (append && loadingMoreRef.current) return;
    if (append) {
      loadingMoreRef.current = true;
      setMoreLoading(true);
    } else {
      setListLoading(true);
    }
    try {
      while (listInFlight.current) await listInFlight.current;
      if (generation !== listGeneration.current) return;
      const offset = append ? commitsRef.current.length : 0;
      const pending = listHistory(
        revision,
        allRefs,
        debouncedSearch,
        searchBy,
        filePath.trim() || null,
        offset,
        200,
      );
      listInFlight.current = pending;
      const result = await pending;
      if (listInFlight.current === pending) listInFlight.current = null;
      if (generation !== listGeneration.current) return;
      if (!result.ok) {
        setError(result);
        return;
      }
      setError(null);
      setPageState(result.data);
      setHasMore(result.data.hasMore);
      const updated = append ? [...commitsRef.current, ...result.data.commits] : result.data.commits;
      commitsRef.current = updated;
      setCommits(updated);
    } catch (cause) {
      setError({
        ok: false,
        operation: "List history",
        args: ["git", "log"],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
    } finally {
      if (generation === listGeneration.current) setListLoading(false);
      loadingMoreRef.current = false;
      setMoreLoading(false);
    }
  }, [allRefs, debouncedSearch, filePath, listHistory, revision, searchBy]);

  useEffect(() => {
    const generation = ++listGeneration.current;
    const changedQuery = previousQueryKey.current !== queryKey;
    const changedFilters = previousQueryKey.current && !previousQueryKey.current.startsWith(`${filterKey}|`);
    previousQueryKey.current = queryKey;
    if (changedQuery) {
      setError(null);
      if (changedFilters) {
        setCommits([]);
        commitsRef.current = [];
        setHasMore(false);
        setPageState(null);
        scrollRef.current?.scrollTo({ top: 0 });
      }
    }
    void loadPage(generation, false);
  }, [filterKey, loadPage, queryKey, refreshKey]);

  useEffect(() => {
    if (!initialHash) return;
    const commit = commits.find((item) => item.hash === initialHash || item.shortHash === initialHash);
    if (commit) selectCommit(commit);
  }, [commits, initialHash]);

  // The virtualizer is an imperative measurement API and is intentionally used outside memoized props.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: commits.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 62,
    overscan: 8,
  });
  const virtualItems = virtualizer.getVirtualItems();

  useEffect(() => {
    const last = virtualItems[virtualItems.length - 1];
    if (last && hasMore && last.index >= commits.length - 8 && !listLoading && !moreLoading) {
      void loadPage(listGeneration.current, true);
    }
  }, [commits.length, hasMore, listLoading, loadPage, moreLoading, virtualItems]);

  useEffect(() => {
    let active = true;
    if (allRefs || !pageState?.upstream) {
      setUnpushedHashes(new Set());
      return () => { active = false; };
    }
    void listUnpushedCommits().then((result) => {
      if (!active) return;
      if (!result.ok) setError(result);
      else setUnpushedHashes(new Set(result.data));
    });
    return () => { active = false; };
  }, [allRefs, listUnpushedCommits, pageState?.upstream, refreshKey]);

  useEffect(() => {
    if (pageHeadHash === undefined || pageRefsHash === undefined) return;
    let active = true;
    const checkHistoryRefs = async () => {
      const result = await getHistoryFingerprint();
      if (!active) return;
      if (!result.ok) {
        setError(result);
        return;
      }
      if (result.data.headHash !== pageHeadHash || result.data.refsHash !== pageRefsHash) {
        setRefreshNonce((current) => current + 1);
      }
    };
    window.addEventListener("focus", checkHistoryRefs);
    return () => {
      active = false;
      window.removeEventListener("focus", checkHistoryRefs);
    };
  }, [getHistoryFingerprint, pageHeadHash, pageRefsHash]);

  useEffect(() => {
    if (!selectedHash) {
      setDetail(null);
      setSelectedPath(null);
      setDiffState(null);
      setFullDiffOpen(false);
      return;
    }
    const generation = ++detailGeneration.current;
    setDetailLoading(true);
    setDetail(null);
    setSelectedPath(null);
    setDiffState(null);
    void loadHistoryDetail(selectedHash, null).then((result) => {
      if (generation !== detailGeneration.current) return;
      if (!result.ok) setError(result);
      else {
        setError(null);
        setDetail(result.data);
      }
    }).finally(() => {
      if (generation === detailGeneration.current) setDetailLoading(false);
    });
  }, [loadHistoryDetail, selectedHash]);

  const requestDiff = useCallback(async (
    path: string | null,
    allowLarge = false,
  ) => {
    if (!selectedHash) return;
    const key = `${selectedHash}:${detail?.comparisonParent ?? ""}:${path ?? "*"}`;
    const generation = ++diffGeneration.current;
    setDiffLoading(true);
    const result = await loadHistoryDiff(selectedHash, detail?.comparisonParent ?? null, path, allowLarge);
    if (generation !== diffGeneration.current) return;
    if (!result.ok) setError(result);
    else {
      setError(null);
      setDiffState({ key, diff: result.data, allowLarge });
    }
    setDiffLoading(false);
  }, [detail?.comparisonParent, loadHistoryDiff, selectedHash]);

  useEffect(() => {
    if (selectedPath) void requestDiff(selectedPath);
    else if (fullDiffOpen) void requestDiff(null);
  }, [fullDiffOpen, requestDiff, selectedPath]);

  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [contextMenu]);

  async function copyText(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopyNotice(`${label} copiado`);
      window.setTimeout(() => setCopyNotice(""), 1800);
    } catch (cause) {
      setError({
        ok: false,
        operation: "Copy to clipboard",
        args: [],
        exitCode: null,
        stderr: cause instanceof Error ? cause.message : String(cause),
        stdout: "",
      });
    }
    setContextMenu(null);
  }

  function selectCommit(commit: HistoryCommit) {
    setSelectedHash(commit.hash);
    setError(null);
  }

  function handleFileKey(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const files = detail?.files ?? [];
    if (!files.length) return;
    const current = files.findIndex((file) => file.path === selectedPath);
    const next = event.key === "ArrowDown"
      ? Math.min(files.length - 1, current < 0 ? 0 : current + 1)
      : Math.max(0, current < 0 ? 0 : current - 1);
    event.preventDefault();
    event.stopPropagation();
    setSelectedPath(files[next].path);
    fileRowRefs.current.get(next)?.focus();
  }

  function handleCommitKey(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      if (selectedHash) setSelectedHash(null);
      else onBack();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp" && event.key !== "Enter") return;
    const current = commits.findIndex((commit) => commit.hash === selectedHash);
    const next = event.key === "ArrowDown"
      ? Math.min(commits.length - 1, current < 0 ? 0 : current + 1)
      : event.key === "ArrowUp" ? Math.max(0, current < 0 ? 0 : current - 1) : current;
    if (next < 0 || !commits[next]) return;
    event.preventDefault();
    if (event.key === "Enter") selectCommit(commits[next]);
    else {
      setSelectedHash(commits[next].hash);
      rowRefs.current.get(next)?.focus();
      virtualizer.scrollToIndex(next, { align: "auto" });
    }
  }

  const currentBranch = branches?.local.find((branch) => branch.current);
  const activeDiffKey = selectedHash && `${selectedHash}:${detail?.comparisonParent ?? ""}:${selectedPath ?? "*"}`;
  const visibleDiff = diffState?.key === activeDiffKey ? diffState : null;
  const fileContextPath = contextMenu?.path;
  return (
    <section className="history-panel" onKeyDown={handleCommitKey}>
      <header className="history-heading">
        <div>
          <p className="eyebrow">Repositório</p>
          <h1>Histórico</h1>
        </div>
        <div className="history-actions">
          <Button variant="outline" onClick={() => onBack()}><ChevronLeft size={14} />Alterações</Button>
          <Button variant="ghost" size="icon" aria-label="Atualizar histórico" onClick={() => {
            const generation = ++listGeneration.current;
            void loadPage(generation, false);
          }}><RefreshCw size={15} /></Button>
        </div>
      </header>
      <div className="history-toolbar">
        <label>
          <span>Escopo</span>
          <select value={allRefs ? "__all__" : revision ?? "__head__"} onChange={(event) => {
            const value = event.target.value;
            setAllRefs(value === "__all__");
            setRevision(value === "__head__" || value === "__all__" ? null : value);
          }}>
            <option value="__head__">Branch atual (HEAD)</option>
            {branches?.local.map((branch) => (
              <option key={`local:${branch.name}`} value={`refs/heads/${branch.name}`}>Local: {branch.name}</option>
            ))}
            {branches?.remote.map((branch) => (
              <option key={`remote:${branch.name}`} value={`refs/remotes/${branch.name}`}>Remote: {branch.name}</option>
            ))}
            <option value="__all__">Todas as branches</option>
          </select>
        </label>
        <label>
          <span>Pesquisar por</span>
          <select value={searchBy} onChange={(event) => setSearchBy(event.target.value as SearchBy)}>
            <option value="message">Mensagem</option>
            <option value="author">Autor</option>
            <option value="hash">Hash</option>
          </select>
        </label>
        <label className="history-search">
          <span>Pesquisar no histórico</span>
          <Search size={14} />
          <input value={searchText} placeholder={`Search by ${searchBy}…`} onChange={(event) => setSearchText(event.target.value)} />
          {searchText && <button type="button" aria-label="Limpar pesquisa" onClick={() => setSearchText("")}><X size={13} /></button>}
        </label>
        <form className="history-file-filter" onSubmit={(event) => {
          event.preventDefault();
          setFilePath(pathDraft.trim());
        }}>
          <label>
            <span>Caminho do arquivo</span>
            <input value={pathDraft} placeholder="Filtrar por arquivo…" onChange={(event) => setPathDraft(event.target.value)} />
          </label>
          <Button variant="outline" type="submit">Filtrar</Button>
          {filePath && <Button variant="ghost" size="icon" type="button" aria-label="Limpar filtro de arquivo" onClick={() => {
            setPathDraft("");
            setFilePath("");
          }}><X size={13} /></Button>}
        </form>
      </div>
      {(pageState?.shallow || pageState?.detached) && (
        <div className="history-notices">
          {pageState.shallow && <span>Este repositório é raso; o histórico anterior pode estar indisponível.</span>}
          {pageState.detached && <span>HEAD destacado · exibindo o histórico a partir de HEAD.</span>}
          {currentBranch && <span>Branch: {currentBranch.name}</span>}
        </div>
      )}
      {error && <HistoryError failure={error} onRefresh={() => {
        setError(null);
        const generation = ++listGeneration.current;
        void loadPage(generation, false);
      }} onOpenTerminal={onOpenTerminal} />}
      <div className="history-layout">
        <div className="history-list-column">
          <div className="history-list" ref={scrollRef} onScroll={() => {
            const element = scrollRef.current;
            if (element && element.scrollTop + element.clientHeight >= element.scrollHeight - 160 && hasMore && !moreLoading) {
              void loadPage(listGeneration.current, true);
            }
          }}>
            {listLoading && !commits.length ? <div className="history-empty"><LoaderCircle size={15} className="spin" />Carregando commits…</div>
              : !commits.length && !error ? <div className="history-empty">{debouncedSearch || filePath ? "Nenhum commit correspondente." : "Ainda não há commits."}</div>
                : <div className="history-virtual-content" style={{ height: `${virtualizer.getTotalSize()}px` }}>
                  {virtualItems.map((item) => {
                    const commit = commits[item.index];
                    const isHead = commit.hash === pageState?.headHash;
                    return (
                      <div
                        key={commit.hash}
                        ref={(element) => {
                          if (element) rowRefs.current.set(item.index, element);
                          else rowRefs.current.delete(item.index);
                        }}
                        className={`history-row ${selectedHash === commit.hash ? "history-row-selected" : ""} ${isHead ? "history-row-head" : ""}`}
                        style={{ transform: `translateY(${item.start}px)` }}
                        role="option"
                        tabIndex={selectedHash === commit.hash ? 0 : -1}
                        aria-selected={selectedHash === commit.hash}
                        onClick={() => selectCommit(commit)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault();
                            event.stopPropagation();
                            selectCommit(commit);
                          }
                        }}
                        onContextMenu={(event) => {
                          event.preventDefault();
                          setContextMenu({ hash: commit.hash, shortHash: commit.shortHash, subject: commit.subject, parentCount: commit.parents.length, path: null, x: event.clientX, y: event.clientY });
                        }}
                      >
                        <span className="history-marker">{commit.parents.length > 1 ? <GitMerge size={15} /> : <GitCommitHorizontal size={15} />}</span>
                        <span className="history-row-main">
                          <span className="history-subject" title={commit.subject}>{commit.subject || "(no subject)"}</span>
                          <span className="history-meta">
                            <code>{commit.shortHash}</code>
                            <span>{commit.author}</span>
                            <time title={formatAbsoluteDate(commit.commitDate)}>{formatRelativeDate(commit.commitDate)}</time>
                            {commit.parents.length > 1 && <span className="history-merge-badge">merge</span>}
                            {unpushedHashes.has(commit.hash) && <span className="history-unpushed-badge" title="Ainda não enviado ao upstream">Não enviado</span>}
                            {commit.refs.map((ref) => <RefBadge key={`${ref.kind}:${ref.name}`} refInfo={ref} />)}
                          </span>
                        </span>
                        <span className="history-row-actions">
                          <button type="button" aria-label="Copiar hash completo do commit" title="Copiar hash completo" onClick={(event) => {
                            event.stopPropagation();
                            void copyText(commit.hash, "Hash");
                          }}><Copy size={13} /></button>
                        </span>
                      </div>
                    );
                  })}
                </div>}
            {moreLoading && <div className="history-loading-more"><LoaderCircle size={13} className="spin" />Carregando mais commits…</div>}
          </div>
          <div className="history-list-footer">{commits.length} commits carregados{pageState?.upstream && !allRefs ? ` · upstream ${pageState.upstream}` : ""}</div>
        </div>
        <section className="history-detail">
          {!selectedHash ? <div className="history-empty">Selecione um commit para ver os detalhes e o diff.</div>
            : detailLoading ? <div className="history-empty"><LoaderCircle size={15} className="spin" />Carregando detalhes do commit…</div>
              : detail ? (
                <>
                  <header className="history-detail-heading">
                    <div>
                      <p className="eyebrow">Detalhes do commit</p>
                      <h2>{detail.commit.subject || "(no subject)"}</h2>
                    </div>
                    <div className="history-detail-actions">
                      <Button variant="outline" disabled={advancedDisabled} onClick={() => onCherryPick(detail.commit.hash, detail.commit.parents.length)}>Cherry-pick</Button>
                      <Button variant="outline" disabled={advancedDisabled} onClick={() => onReset(detail.commit.hash)}>Resetar branch aqui…</Button>
                      <Button variant="outline" onClick={() => void copyText(detail.commit.hash, "Hash")}><Copy size={13} />{copyNotice === "Hash copiado" ? "Copiado" : "Copiar hash"}</Button>
                    </div>
                  </header>
                  <div className="history-detail-body">
                    <div className="history-hash"><code>{detail.commit.hash}</code></div>
                    <div className="history-identities">
                      <div><strong>Autor</strong><span>{detail.commit.author} &lt;{detail.commit.authorEmail}&gt;</span><time title={formatAbsoluteDate(detail.commit.authorDate)}>{formatAbsoluteDate(detail.commit.authorDate)}</time></div>
                      {(detail.commit.committer !== detail.commit.author || detail.commit.committerEmail !== detail.commit.authorEmail) && (
                        <div><strong>Autor do commit</strong><span>{detail.commit.committer} &lt;{detail.commit.committerEmail}&gt;</span><time title={formatAbsoluteDate(detail.commit.commitDate)}>{formatAbsoluteDate(detail.commit.commitDate)}</time></div>
                      )}
                    </div>
                    <div className="history-parents"><strong>Commits pais</strong>
                      {detail.commit.parents.length ? detail.commit.parents.map((parent, index) => <button key={parent} onClick={() => setSelectedHash(parent)} title={`Abrir commit pai ${index + 1}`}><code>{parent.slice(0, 12)}</code>{index === 0 ? " · primeiro pai" : ""}</button>) : <span>Commit raiz</span>}
                    </div>
                    {detail.commit.parents.length > 1 && (
                      <label className="history-parent-select"><span>Comparar com</span>
                        <select value={detail.comparisonParent ?? ""} onChange={(event) => {
                          const parent = event.target.value || null;
                          const generation = ++detailGeneration.current;
                          setDetailLoading(true);
                          void loadHistoryDetail(detail.commit.hash, parent).then((result) => {
                            if (generation !== detailGeneration.current) return;
                            if (result.ok) {
                              setDetail(result.data);
                              setSelectedPath(null);
                              setDiffState(null);
                            } else setError(result);
                          }).finally(() => {
                            if (generation === detailGeneration.current) setDetailLoading(false);
                          });
                        }}>
                          {detail.commit.parents.map((parent, index) => <option key={parent} value={parent}>Parent {index + 1}: {parent.slice(0, 12)}{index === 0 ? " (default)" : ""}</option>)}
                        </select>
                        <small>{detail.comparisonParent ? `Diff em relação a ${detail.comparisonParent.slice(0, 12)}` : "Sem comparação com commit pai"}</small>
                      </label>
                    )}
                    <div className="history-refs">{detail.commit.refs.map((ref) => <RefBadge key={`${ref.kind}:${ref.name}`} refInfo={ref} />)}</div>
                    <pre className="history-message">{detail.commit.body || detail.commit.subject}</pre>
                    <div className="history-stat">{detail.shortstat || "Nenhuma alteração em arquivos"} · {detail.files.length} {detail.files.length === 1 ? "arquivo" : "arquivos"}</div>
                    <div className="history-files-heading"><strong>Arquivos alterados</strong>
                      <details className="history-full-diff" open={fullDiffOpen} onToggle={(event) => {
                        const opened = event.currentTarget.open;
                        setFullDiffOpen(opened);
                      }}>
                        <summary>Diff completo do commit</summary>
                        {fullDiffOpen && (diffLoading && selectedPath === null ? <div className="history-empty"><LoaderCircle size={13} className="spin" />Carregando diff completo…</div>
                          : visibleDiff && selectedPath === null ? <DiffView diff={visibleDiff.diff} allowLarge={visibleDiff.allowLarge} onLoadLarge={() => void requestDiff(null, true)} />
                            : <span className="history-inline-note">Abra para carregar os diffs dos arquivos sob demanda.</span>)}
                      </details>
                    </div>
                    <div className="history-files" onKeyDown={handleFileKey}>
                      {detail.files.length ? detail.files.map((file, index) => (
                        <button
                          key={`${file.originalPath ?? ""}:${file.path}`}
                          ref={(element) => {
                            if (element) fileRowRefs.current.set(index, element);
                            else fileRowRefs.current.delete(index);
                          }}
                          className={`history-file-row ${selectedPath === file.path ? "history-file-selected" : ""}`}
                          onClick={() => {
                            setSelectedPath(file.path);
                            setFullDiffOpen(false);
                          }}
                          onContextMenu={(event) => {
                            event.preventDefault();
                            setContextMenu({ hash: detail.commit.hash, shortHash: detail.commit.shortHash, subject: detail.commit.subject, parentCount: detail.commit.parents.length, path: file.path, x: event.clientX, y: event.clientY });
                          }}
                        >
                          <span className={`history-file-status history-file-${file.status.toLowerCase()}`}>{file.status}</span>
                          <span className="history-file-name" title={file.originalPath ? `${file.originalPath} → ${file.path}` : file.path}>{file.originalPath ? `${file.originalPath} → ${file.path}` : file.path}</span>
                          <span className="history-file-counts">{file.binary ? "bin" : `+${file.additions ?? 0} −${file.deletions ?? 0}`}</span>
                        </button>
                      )) : <div className="history-empty">Este commit não contém alterações em arquivos.</div>}
                    </div>
                    {selectedPath && (
                      <div className="history-selected-diff">
                        <div className="history-selected-diff-heading"><strong>{selectedPath}</strong><button type="button" onClick={() => setSelectedPath(null)} aria-label="Fechar diff do arquivo"><X size={13} /></button></div>
                        {diffLoading ? <div className="history-empty"><LoaderCircle size={13} className="spin" />Carregando diff do arquivo…</div>
                          : visibleDiff ? <DiffView diff={visibleDiff.diff} allowLarge={visibleDiff.allowLarge} onLoadLarge={() => void requestDiff(selectedPath, true)} />
                            : <div className="history-empty">Selecione um arquivo para carregar seu diff.</div>}
                      </div>
                    )}
                  </div>
                </>
              ) : <div className="history-empty">Selecione um commit para ver os detalhes e o diff.</div>}
        </section>
      </div>
      {copyNotice && <div className="history-copy-toast" role="status"><Check size={13} />{copyNotice}</div>}
      {contextMenu && <div className="history-context-menu" style={{ top: contextMenu.y, left: contextMenu.x }} onClick={(event) => event.stopPropagation()}>
        <button onClick={() => void copyText(contextMenu.hash, "Hash completo")}>Copiar hash completo</button>
        <button onClick={() => void copyText(contextMenu.shortHash, "Hash curto")}>Copiar hash curto</button>
        <button onClick={() => void copyText(contextMenu.subject, "Descrição")}>Copiar descrição</button>
        {!fileContextPath && <button disabled={advancedDisabled} onClick={() => {
          onCherryPick(contextMenu.hash, contextMenu.parentCount);
          setContextMenu(null);
        }}>Cherry-pick this commit onto {currentBranch?.name ?? "current HEAD"}</button>}
        {!fileContextPath && <button disabled={advancedDisabled} onClick={() => {
          onReset(contextMenu.hash);
          setContextMenu(null);
        }}>Resetar a branch atual para este commit…</button>}
        {fileContextPath && <button onClick={() => {
          setPathDraft(fileContextPath);
          setFilePath(fileContextPath);
          setContextMenu(null);
        }}>Mostrar histórico do arquivo</button>}
      </div>}
    </section>
  );
}

function RefBadge({ refInfo }: { refInfo: HistoryCommit["refs"][number] }) {
  const Icon = refInfo.kind === "tag" ? Tag : GitBranch;
  const label = refInfo.kind === "head" ? "HEAD" : refInfo.name;
  return <span className={`history-ref-badge history-ref-${refInfo.kind} ${refInfo.head ? "history-ref-current" : ""}`} title={`${refInfo.kind}: ${label}`}><Icon size={10} />{label}</span>;
}

function HistoryError({
  failure,
  onRefresh,
  onOpenTerminal,
}: {
  failure: CommandFailure;
  onRefresh: () => void;
  onOpenTerminal: () => void;
}) {
  return (
    <section className="history-error" role="alert">
      <strong>{failure.operation} failed</strong>
      <details open><summary>Detalhes</summary>
        <code>{formatGitCommand(failure.args)}</code>
        <pre>{[failure.stdout, failure.stderr].filter(Boolean).join("\n") || `Exit code: ${failure.exitCode ?? "unknown"}`}</pre>
      </details>
      <div>
        <Button variant="outline" onClick={onOpenTerminal}>Abrir terminal</Button>
        <Button variant="outline" onClick={onRefresh}><RefreshCw size={13} />Atualizar</Button>
      </div>
    </section>
  );
}
