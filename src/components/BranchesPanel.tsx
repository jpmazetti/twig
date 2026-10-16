import { useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";
import {
  Check,
  ChevronDown,
  Circle,
  CircleDot,
  Copy,
  GitBranch,
  GitMerge,
  LoaderCircle,
  Plus,
  GitPullRequest,
} from "lucide-react";
import { Button } from "./ui/button";
import type { BranchInfo, BranchList, CommandFailure } from "../types/git";
import { formatGitCommand } from "../lib/commit";

type BranchAction = (name: string, trackRemote?: boolean) => Promise<boolean>;
type AdvancedBranchOperation = "merge" | "rebase";

export function BranchesPanel({
  branches,
  repository,
  currentBranch,
  busy,
  refreshing,
  error,
  onClearError,
  onCreate,
  onSwitch,
  onRename,
  onDelete,
  onAdvanced,
  onOpenTerminal,
  advancedDisabled,
}: {
  branches: BranchList | null;
  repository: string;
  currentBranch: string | null;
  busy: boolean;
  refreshing: boolean;
  error: CommandFailure | null;
  onClearError: () => void;
  onCreate: (name: string, startPoint: string | null, switchTo: boolean) => Promise<boolean>;
  onSwitch: BranchAction;
  onRename: (oldName: string, newName: string) => Promise<boolean>;
  onDelete: (name: string, force?: boolean) => Promise<boolean>;
  onAdvanced: (operation: AdvancedBranchOperation, target: string) => void;
  onOpenTerminal: () => void;
  advancedDisabled: boolean;
}) {
  const [filter, setFilter] = useState("");
  const [dialog, setDialog] = useState<"create" | "rename" | null>(null);
  const [branchName, setBranchName] = useState("");
  const [startSelection, setStartSelection] = useState("HEAD");
  const [commitHash, setCommitHash] = useState("");
  const [switchToNew, setSwitchToNew] = useState(true);
  const [editingBranch, setEditingBranch] = useState<BranchInfo | null>(null);
  const [deleteBranch, setDeleteBranch] = useState<BranchInfo | null>(null);
  const [trackBranch, setTrackBranch] = useState<BranchInfo | null>(null);
  const [activeTab, setActiveTab] = useState<"local" | "remote">("local");
  const [visibleCount, setVisibleCount] = useState(60);
  const [copiedName, setCopiedName] = useState<string | null>(null);
  const [copyError, setCopyError] = useState("");
  const [branchMenu, setBranchMenu] = useState<{ name: string; x: number; y: number } | null>(null);
  const filterRef = useRef<HTMLInputElement>(null);
  const localBranches = useMemo(
    () => {
      const local = branches?.local ?? [];
      const current = local.find((branch) => branch.current);
      const matching = local.filter((branch) => branch !== current && branch.name.toLowerCase().includes(filter.toLowerCase()));
      return [...(current ? [current] : []), ...matching];
    },
    [branches, filter],
  );
  const remoteBranches = useMemo(
    () => branches?.remote.filter((branch) => branch.name.toLowerCase().includes(filter.toLowerCase())) ?? [],
    [branches, filter],
  );
  const activeBranches = activeTab === "local" ? localBranches : remoteBranches;
  const visibleBranches = activeBranches.slice(0, visibleCount);
  const deletingCurrent = deleteBranch?.name === currentBranch;
  const canForceDelete = error?.operation === "Delete branch"
    && /not fully merged|not merged/i.test(`${error.stderr}\n${error.stdout}`);
  const localChangeConflict = error?.operation === "Switch branch"
    && /(local changes|untracked working tree files)[\s\S]*(overwritten|would be overwritten)/i.test(`${error.stderr}\n${error.stdout}`);
  const operationInProgress = error?.operation === "Switch branch"
    && /(merge|rebase).*(in progress|cannot switch|must finish|not concluded)|cannot switch branches|MERGE_HEAD|rebase-merge/i.test(`${error.stderr}\n${error.stdout}`);
  const dialogError = dialog === "create"
    && ["Validate branch name", "Resolve branch start point", "Create branch"].includes(error?.operation ?? "")
    ? error?.stderr || error?.stdout
    : dialog === "rename" && ["Validate branch name", "Rename branch"].includes(error?.operation ?? "")
      ? error?.stderr || error?.stdout
      : "";

  useEffect(() => {
    if (!branchMenu) return;
    const close = () => setBranchMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [branchMenu]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (event.ctrlKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "k"
        && !(target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))) {
        event.preventDefault();
        filterRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  async function submitCreate() {
    const startPoint = startSelection === "__commit__" ? commitHash.trim() : startSelection;
    const created = await onCreate(branchName.trim(), startPoint === "HEAD" ? null : startPoint, switchToNew);
    if (created) closeDialog();
  }

  async function submitRename() {
    if (!editingBranch) return;
    const renamed = await onRename(editingBranch.name, branchName.trim());
    if (renamed) closeDialog();
  }

  async function submitDelete(force: boolean) {
    if (!deleteBranch) return;
    const deleted = await onDelete(deleteBranch.name, force);
    if (deleted) setDeleteBranch(null);
  }

  async function copyName(name: string) {
    try {
      await navigator.clipboard.writeText(name);
      setCopiedName(name);
      setCopyError("");
      window.setTimeout(() => setCopiedName(null), 1600);
    } catch (cause) {
      setCopyError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  function openCreate() {
    setBranchName("");
    setStartSelection("HEAD");
    setCommitHash("");
    setSwitchToNew(true);
    onClearError();
    setDialog("create");
  }

  function openRename(branch: BranchInfo) {
    setEditingBranch(branch);
    setBranchName(branch.name);
    onClearError();
    setDialog("rename");
  }

  function openDelete(branch: BranchInfo) {
    onClearError();
    setDeleteBranch(branch);
  }

  function openTracking(branch: BranchInfo) {
    onClearError();
    setTrackBranch(branch);
  }

  function closeDialog() {
    setDialog(null);
    setEditingBranch(null);
  }

  return (
    <>
      <section className="branches-panel">
        <header className="branches-heading">
          <div className="branches-label">BRANCHES</div>
          <div className="branches-heading-actions">
            <Button disabled={busy || refreshing || !branches} onClick={openCreate}><Plus size={14} />Nova</Button>
          </div>
        </header>
        <div className="branches-toolbar">
          <input
            ref={filterRef}
            aria-label="Filtrar branches por nome"
            placeholder="Filtrar branches…  Ctrl+K"
            value={filter}
            onChange={(event) => {
              setVisibleCount(60);
              setFilter(event.target.value);
            }}
          />
          {refreshing && <span className="branches-loading"><LoaderCircle size={13} className="spin" /> Atualizando</span>}
        </div>
        <div className="branch-tabs" role="tablist" aria-label="Tipo de branch">
          <button type="button" role="tab" aria-selected={activeTab === "local"} className={activeTab === "local" ? "branch-tab-active" : ""} onClick={() => { setVisibleCount(60); setActiveTab("local"); }}>Locais ({branches?.local.length ?? 0})</button>
          <button type="button" role="tab" aria-selected={activeTab === "remote"} className={activeTab === "remote" ? "branch-tab-active" : ""} onClick={() => { setVisibleCount(60); setActiveTab("remote"); }}>Remotas ({branches?.remote.length ?? 0})</button>
        </div>
        {copyError && <p className="branch-inline-error">{copyError}</p>}
        {branches?.detachedHead && (
          <div className="detached-head-note">HEAD destacado em <code>{branches.detachedHead}</code></div>
        )}
        <BranchGroup
          title={activeTab === "local" ? "Local" : "Remotas"}
          branches={visibleBranches}
          totalCount={activeBranches.length}
          busy={busy}
          repository={repository}
          noBranches={Boolean(branches && activeBranches.length === 0 && !filter)}
          remote={activeTab === "remote"}
          onShowMore={() => setVisibleCount((count) => count + 60)}
          onSwitch={(branch) => void onSwitch(branch.name, false)}
          onTrack={openTracking}
          hasLocalTracking={(branch) => Boolean(branches?.local.some((local) => local.name === remoteLocalName(branch.name)))}
          onCopy={(branch) => void copyName(branch.name)}
          copiedName={copiedName}
          onBranchOperation={onAdvanced}
          onAdvanced={(name, event) => {
            event.preventDefault();
            setBranchMenu({ name, x: event.clientX, y: event.clientY });
          }}
        />
        {branchMenu && (
          <div className="branch-context-menu" style={{ top: branchMenu.y, left: branchMenu.x }} role="menu" onClick={(event) => event.stopPropagation()}>
            <button disabled={advancedDisabled} onClick={() => {
              onAdvanced("merge", branchMenu.name);
              setBranchMenu(null);
            }}>Fazer merge na atual</button>
            <button disabled={advancedDisabled} onClick={() => {
              onAdvanced("rebase", branchMenu.name);
              setBranchMenu(null);
            }}>Fazer rebase da atual nesta</button>
            {branches?.local.some((branch) => branch.name === branchMenu.name) && (
              <>
                <button onClick={() => {
                  const branch = branches?.local.find((item) => item.name === branchMenu.name);
                  if (branch) void copyName(branch.name);
                  setBranchMenu(null);
                }}>{copiedName === branchMenu.name ? "Copiado" : "Copiar nome"}</button>
                <button disabled={busy} onClick={() => {
                  const branch = branches?.local.find((item) => item.name === branchMenu.name);
                  if (branch) openRename(branch);
                  setBranchMenu(null);
                }}>Renomear</button>
                <button disabled={busy || branchMenu.name === currentBranch} onClick={() => {
                  const branch = branches?.local.find((item) => item.name === branchMenu.name);
                  if (branch) openDelete(branch);
                  setBranchMenu(null);
                }}>Excluir</button>
              </>
            )}
            {!branches?.local.some((branch) => branch.name === branchMenu.name) && (
              <button onClick={() => {
                void copyName(branchMenu.name);
                setBranchMenu(null);
              }}>{copiedName === branchMenu.name ? "Copiado" : "Copiar nome"}</button>
            )}
            <button onClick={() => setBranchMenu(null)}>Cancelar</button>
          </div>
        )}
        {!branches && !refreshing && <p className="section-empty">Lista de branches indisponível. Atualize para tentar novamente.</p>}
      </section>

      <footer className="branch-operations">
        <p>OPERAÇÕES</p>
        <div>
          <Button variant="outline" disabled={busy || refreshing || !branches || advancedDisabled} onClick={() => onAdvanced("merge", "")}><GitMerge size={14} />Merge…</Button>
          <Button variant="outline" disabled={busy || refreshing || !branches || advancedDisabled} onClick={() => onAdvanced("rebase", "")}><GitPullRequest size={14} />Rebase…</Button>
        </div>
      </footer>
      {localChangeConflict && (
        <section className="branch-conflict-note" role="alert">
          <p>O Git recusou a troca porque as alterações locais seriam sobrescritas. Crie um commit ou guarde-as (stash) no terminal e tente novamente.</p>
          <div className="dialog-actions">
            <Button variant="outline" onClick={onOpenTerminal}>Abrir terminal</Button>
            <Button variant="outline" onClick={onClearError}>Cancelar</Button>
          </div>
        </section>
      )}
      {operationInProgress && (
        <section className="branch-conflict-note" role="alert">
          O Git recusou a troca enquanto há um merge ou rebase em andamento. Conclua ou aborte essa operação antes de trocar de branch.
        </section>
      )}

      {dialog && (
        <div className="dialog-backdrop" role="presentation">
          <section className="discard-dialog branch-dialog" role="dialog" aria-modal="true" aria-labelledby="branch-dialog-title">
            <h2 id="branch-dialog-title">{dialog === "create" ? "Criar branch" : "Renomear branch"}</h2>
            <label className="remote-select-label" htmlFor="branch-name">Nome da branch</label>
            <input id="branch-name" className="remote-select" autoFocus value={branchName} onChange={(event) => setBranchName(event.target.value)} />
            {dialog === "create" && (
              <>
                <label className="remote-select-label" htmlFor="branch-start">Ponto de partida</label>
                <select id="branch-start" className="remote-select" value={startSelection} onChange={(event) => setStartSelection(event.target.value)}>
                  <option value="HEAD">HEAD (commit atual)</option>
                  {branches?.local.map((branch) => <option key={`local:${branch.name}`} value={branch.name}>Local: {branch.name}</option>)}
                  {branches?.remote.map((branch) => <option key={`remote:${branch.name}`} value={branch.name}>Remota: {branch.name}</option>)}
                  <option value="__commit__">Hash do commit…</option>
                </select>
                {startSelection === "__commit__" && (
                  <input className="remote-select branch-hash-input" aria-label="Hash do commit inicial" placeholder="Hash do commit" value={commitHash} onChange={(event) => setCommitHash(event.target.value)} />
                )}
                <label className="branch-switch-toggle">
                  <input type="checkbox" checked={switchToNew} onChange={(event) => setSwitchToNew(event.target.checked)} />
                  Trocar para a nova branch
                </label>
              </>
            )}
            {dialog === "rename" && editingBranch?.upstream && (
              <p className="branch-upstream-warning">
                O Git não renomeia a branch remota. O upstream continuará apontando para <code>{editingBranch.upstream}</code>.
              </p>
            )}
            {dialogError && <p className="branch-inline-error">{dialogError}</p>}
            <p className="remote-command">Comando: {dialog === "create"
              ? formatGitCommand(["-C", repository, ...(switchToNew ? ["switch", "-c"] : ["branch"]), branchName || "<name>", ...(startSelection === "HEAD" ? [] : [startSelection === "__commit__" ? commitHash || "<hash>" : startSelection])])
              : formatGitCommand(["-C", repository, "branch", "-m", editingBranch?.name ?? "", branchName || "<new-name>"])}</p>
            <div className="dialog-actions">
              <Button variant="outline" disabled={busy} onClick={closeDialog}>Cancelar</Button>
              <Button disabled={busy || !branchName.trim() || (dialog === "create" && startSelection === "__commit__" && !commitHash.trim())} onClick={() => void (dialog === "create" ? submitCreate() : submitRename())}>
                {busy ? "Processando…" : dialog === "create" ? "Criar branch" : "Renomear branch"}
              </Button>
            </div>
          </section>
        </div>
      )}

      {deleteBranch && (
        <div className="dialog-backdrop" role="presentation">
          <section className="discard-dialog branch-dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-branch-title">
            <h2 id="delete-branch-title">Excluir branch?</h2>
            <p>Excluir a branch local <code>{deleteBranch.name}</code> no commit <code>{deleteBranch.shortHash}</code>?</p>
            {canForceDelete && (
              <p className="branch-destructive-warning">
                Esta branch não foi totalmente integrada. Seus commits exclusivos podem ficar inacessíveis. Recupere a ponta pelo reflog usando {deleteBranch.shortHash}.
              </p>
            )}
            <p className="remote-command">Comando: {formatGitCommand(["-C", repository, "branch", canForceDelete ? "-D" : "-d", deleteBranch.name])}</p>
            <div className="dialog-actions">
              <Button variant="outline" disabled={busy} onClick={() => setDeleteBranch(null)}>Cancelar</Button>
              {canForceDelete ? (
                <Button className="destructive-button" disabled={busy} onClick={() => void submitDelete(true)}>{busy ? "Excluindo…" : "Excluir mesmo assim (-D)"}</Button>
              ) : (
                <Button className="destructive-button" disabled={busy || deletingCurrent} title={deletingCurrent ? "Troque para outra branch primeiro" : undefined} onClick={() => void submitDelete(false)}>{busy ? "Excluindo…" : "Excluir"}</Button>
              )}
            </div>
          </section>
        </div>
      )}

      {trackBranch && (
        <div className="dialog-backdrop" role="presentation">
          <section className="discard-dialog branch-dialog" role="dialog" aria-modal="true" aria-labelledby="track-branch-title">
            <h2 id="track-branch-title">Criar branch local de acompanhamento?</h2>
            <p>Criar uma branch local que acompanha <code>{trackBranch.name}</code>.</p>
            <p className="remote-command">Comando: {formatGitCommand(["-C", repository, "switch", "--track", trackBranch.name])}</p>
            <div className="dialog-actions">
              <Button variant="outline" disabled={busy} onClick={() => setTrackBranch(null)}>Cancelar</Button>
              <Button disabled={busy} onClick={async () => {
                if (await onSwitch(trackBranch.name, true)) setTrackBranch(null);
              }}>{busy ? "Trocando…" : "Criar e trocar"}</Button>
            </div>
          </section>
        </div>
      )}
    </>
  );
}

export function BranchQuickSwitcher({
  branches,
  repository,
  currentBranch,
  detachedHead,
  busy,
  onSwitch,
}: {
  branches: BranchList | null;
  repository: string;
  currentBranch: string | null;
  detachedHead: string | null;
  busy: boolean;
  onSwitch: BranchAction;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const [trackBranch, setTrackBranch] = useState<BranchInfo | null>(null);
  const filteredLocal = branches?.local.filter((branch) => branch.name.toLowerCase().includes(filter.toLowerCase())) ?? [];
  const filteredRemote = branches?.remote.filter((branch) => branch.name.toLowerCase().includes(filter.toLowerCase())) ?? [];

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (event.ctrlKey && event.key.toLowerCase() === "b"
        && !(target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))) {
        event.preventDefault();
        setOpen((value) => !value);
      } else if (event.key === "Escape") {
        setOpen(false);
        setTrackBranch(null);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  async function switchTo(name: string, trackRemote: boolean) {
    if (await onSwitch(name, trackRemote)) {
      setOpen(false);
      setTrackBranch(null);
    }
  }

  return (
    <div className="branch-quick-control">
      <button className="branch-indicator" aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
        <GitBranch size={15} />
        <span>{currentBranch ?? `HEAD detached at ${detachedHead ?? "unknown"}`}</span>
        <ChevronDown size={13} />
      </button>
      {open && (
        <section className="branch-quick-popover" role="dialog" aria-label="Trocar branch">
          <input autoFocus aria-label="Pesquisar branches" placeholder="Pesquisar branches… (Ctrl+B)" value={filter} onChange={(event) => setFilter(event.target.value)} />
          <div className="branch-quick-list">
            <strong>Locais</strong>
            {filteredLocal.map((branch) => (
              <button key={branch.name} disabled={busy || branch.current} title={formatGitCommand(["-C", repository, "switch", branch.name])} onClick={() => void switchTo(branch.name, false)}>
                <GitBranch size={14} /><code>{branch.name}</code>                {branch.current && <span>atual</span>}
              </button>
            ))}
            <strong>Remotas</strong>
            {filteredRemote.map((branch) => {
              const localName = remoteLocalName(branch.name);
              const localMatch = branches?.local.some((local) => local.name === localName) ?? false;
              return (
                <button key={branch.name} disabled={busy} title={formatGitCommand(["-C", repository, ...(localMatch ? ["switch", localName] : ["switch", "--track", branch.name])])} onClick={() => {
                  if (localMatch) void switchTo(localName, false);
                  else setTrackBranch(branch);
                }}>
                  <GitBranch size={14} /><code>{branch.name}</code><span>{localMatch ? "trocar local" : "acompanhar"}</span>
                </button>
              );
            })}
            {filteredLocal.length + filteredRemote.length === 0 && <p className="section-empty">Nenhuma branch correspondente.</p>}
          </div>
        </section>
      )}
      {trackBranch && (
        <div className="dialog-backdrop" role="presentation">
          <section className="discard-dialog branch-dialog" role="dialog" aria-modal="true" aria-labelledby="quick-track-title">
            <h2 id="quick-track-title">Criar branch local de acompanhamento?</h2>
            <p>Criar uma branch local que acompanha <code>{trackBranch.name}</code>.</p>
            <p className="remote-command">Command: {formatGitCommand(["-C", repository, "switch", "--track", trackBranch.name])}</p>
            <div className="dialog-actions">
              <Button variant="outline" disabled={busy} onClick={() => setTrackBranch(null)}>Cancelar</Button>
              <Button disabled={busy} onClick={() => void switchTo(trackBranch.name, true)}>{busy ? "Trocando…" : "Criar e trocar"}</Button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

function BranchGroup({
  title,
  branches,
  totalCount,
  repository,
  busy,
  noBranches,
  remote,
  onShowMore,
  onSwitch,
  onTrack,
  hasLocalTracking,
  onCopy,
  copiedName,
  onBranchOperation,
  onAdvanced,
}: {
  title: string;
  branches: BranchInfo[];
  totalCount: number;
  repository: string;
  busy: boolean;
  noBranches: boolean;
  remote: boolean;
  onShowMore: () => void;
  onSwitch: (branch: BranchInfo) => void;
  onTrack: (branch: BranchInfo) => void;
  hasLocalTracking: (branch: BranchInfo) => boolean;
  onCopy: (branch: BranchInfo) => void;
  copiedName: string | null;
  onBranchOperation: (operation: AdvancedBranchOperation, name: string) => void;
  onAdvanced: (name: string, event: ReactMouseEvent<HTMLElement>) => void;
}) {
  return (
    <section className="branch-group">
      <div className="branch-group-heading">{title}<span className="branch-group-count">{totalCount}</span></div>
      {branches.length ? <div className="branch-list">
        {branches.map((branch) => (
          <article
            className={`branch-row ${branch.current ? "branch-row-current" : ""}`}
            key={branch.name}
            onContextMenu={(event) => onAdvanced(branch.name, event)}
            onDoubleClick={() => { if (!remote && !branch.current && !busy) onSwitch(branch); }}
          >
            {branch.current ? <CircleDot size={16} className="branch-row-icon" /> : <Circle size={16} className="branch-row-icon branch-row-icon-muted" />}
            <div className="branch-row-main">
              <code title={branch.name}>{branch.name}</code>
              <BranchMeta branch={branch} local={!remote} />
            </div>
            <div className="branch-row-actions">
              {remote ? (
                hasLocalTracking(branch)
                  ? <Button variant="ghost" title="Trocar para a branch local de acompanhamento" disabled={busy} onClick={() => onSwitch({ ...branch, name: remoteLocalName(branch.name), current: false })}>Trocar</Button>
                  : <Button variant="ghost" title={formatGitCommand(["-C", repository, "switch", "--track", branch.name])} disabled={busy} onClick={() => onTrack(branch)}>Acompanhar</Button>
              ) : !branch.current && (
                <Button variant="ghost" title={formatGitCommand(["-C", repository, "switch", branch.name])} disabled={busy} onClick={() => onSwitch(branch)}>Trocar</Button>
              )}
              {!branch.current && (
                <>
                  <Button variant="ghost" size="icon" aria-label={`Fazer merge de ${branch.name} na atual`} title="Fazer merge na atual" disabled={busy} onClick={() => onBranchOperation("merge", branch.name)}><GitMerge size={14} /></Button>
                  <Button variant="ghost" size="icon" aria-label={`Fazer rebase da atual sobre ${branch.name}`} title="Fazer rebase sobre" disabled={busy} onClick={() => onBranchOperation("rebase", branch.name)}><GitPullRequest size={14} /></Button>
                </>
              )}
              <Button variant="ghost" size="icon" aria-label={`Mais ações para ${branch.name}`} title="Mais ações da branch" onClick={(event) => {
                event.stopPropagation();
                onAdvanced(branch.name, event);
              }}>
                <span className="branch-more-icon">…</span>
              </Button>
              <Button variant="ghost" size="icon" aria-label={`Copiar ${branch.name}`} title="Copiar nome" onClick={() => onCopy(branch)}>
                {copiedName === branch.name ? <Check size={14} /> : <Copy size={14} />}
              </Button>
            </div>
          </article>
        ))}
        {branches.length < totalCount && <button className="branch-show-more" type="button" onClick={onShowMore}>Mostrar mais ({totalCount - branches.length})</button>}
      </div> : <p className="section-empty">{noBranches ? (remote ? "Nenhuma branch remota." : "Ainda não há branches locais.") : "Nenhuma branch corresponde a este filtro."}</p>}
    </section>
  );
}

function BranchMeta({ branch, local = false }: { branch: BranchInfo; local?: boolean }) {
  const title = [
    branch.upstream ? `Acompanha ${branch.upstream}${branch.upstreamGone ? " (removida)" : ""}` : local ? "Somente local" : "",
    branch.shortHash ? `Commit ${branch.shortHash}` : "",
    branch.subject,
  ].filter(Boolean).join(" · ");
  const ahead = branch.ahead !== null && branch.ahead > 0;
  const behind = branch.behind !== null && branch.behind > 0;
  return (
    <span className="branch-meta" title={title || undefined}>
      {ahead && <span className="branch-count-up">↑{branch.ahead}</span>}
      {behind && <span className="branch-count-down">↓{branch.behind}</span>}
      {!ahead && !behind && branch.commitDate && <time dateTime={branch.commitDate}>{relativeDate(branch.commitDate)}</time>}
    </span>
  );
}

function relativeDate(value: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) return value;
  const seconds = Math.round((timestamp - Date.now()) / 1000);
  const ranges: [Intl.RelativeTimeFormatUnit, number][] = [
    ["year", 31_536_000],
    ["month", 2_592_000],
    ["week", 604_800],
    ["day", 86_400],
    ["hour", 3_600],
    ["minute", 60],
  ];
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  for (const [unit, size] of ranges) {
    if (Math.abs(seconds) >= size) return formatter.format(Math.round(seconds / size), unit);
  }
  return formatter.format(seconds, "second");
}

function remoteLocalName(remoteBranch: string): string {
  return remoteBranch.slice(remoteBranch.indexOf("/") + 1);
}
