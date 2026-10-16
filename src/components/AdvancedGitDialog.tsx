import { useEffect, useState } from "react";
import { Check, Copy, LoaderCircle } from "lucide-react";
import { Button } from "./ui/button";
import { formatGitCommand } from "../lib/commit";
import type {
  AdvancedOutput,
  AdvancedPreview,
  AdvancedRequest,
  BranchList,
  CommandFailure,
  CommandResult,
} from "../types/git";

export type AdvancedOperation = AdvancedRequest["operation"];

export function AdvancedGitDialog({
  repository,
  branch,
  branches,
  operation,
  initialTarget,
  mergeParents,
  busy,
  onPreview,
  onRun,
  onRunInTerminal,
  onClose,
}: {
  repository: string;
  branch: string | null;
  branches: BranchList | null;
  operation: AdvancedOperation;
  initialTarget: string;
  mergeParents: number;
  busy: boolean;
  onPreview: (operation: AdvancedOperation, target: string) => Promise<CommandResult<AdvancedPreview>>;
  onRun: (request: AdvancedRequest) => Promise<CommandResult<AdvancedOutput>>;
  onRunInTerminal: (args: string[]) => void;
  onClose: () => void;
}) {
  const [target, setTarget] = useState(initialTarget);
  const [previewState, setPreviewState] = useState<{
    target: string;
    preview?: AdvancedPreview;
    failure?: CommandFailure;
    loading: boolean;
  } | null>(null);
  const [noFf, setNoFf] = useState(false);
  const [ffOnly, setFfOnly] = useState(false);
  const [squash, setSquash] = useState(false);
  const [noCommit, setNoCommit] = useState(false);
  const [autostash, setAutostash] = useState(false);
  const [recordOrigin, setRecordOrigin] = useState(false);
  const [mainline, setMainline] = useState<1 | 2 | null>(null);
  const [resetMode, setResetMode] = useState<"soft" | "mixed" | "hard">("mixed");
  const [hardConfirmation, setHardConfirmation] = useState("");
  const [copied, setCopied] = useState(false);
  const [copyFailure, setCopyFailure] = useState("");
  const request: AdvancedRequest = {
    operation,
    target: target.trim(),
    mode: operation === "reset" ? resetMode : null,
    noFf,
    ffOnly,
    squash,
    noCommit,
    autostash,
    recordOrigin,
    mainline,
  };
  const command = formatGitCommand([
    "-C",
    repository,
    ...operationArgs(request),
  ]);
  const currentPreview = previewState?.target === target.trim() ? previewState : null;
  const preview = currentPreview?.preview ?? null;
  const previewFailure = currentPreview?.failure ?? null;
  const previewLoading = Boolean(target.trim()) && (!currentPreview || currentPreview.loading);

  useEffect(() => {
    let active = true;
    if (!target.trim()) return () => { active = false; };
    const timer = window.setTimeout(() => {
      setPreviewState({ target: target.trim(), loading: true });
      void onPreview(operation, target.trim()).then((result) => {
        if (!active) return;
        if (result.ok) setPreviewState({ target: target.trim(), preview: result.data, loading: false });
        else setPreviewState({ target: target.trim(), failure: result, loading: false });
      }).catch((cause: unknown) => {
        if (!active) return;
        setPreviewState({
          target: target.trim(),
          failure: {
            ok: false,
            operation: "Preview advanced Git operation",
            args: [],
            exitCode: null,
            stderr: cause instanceof Error ? cause.message : String(cause),
            stdout: "",
          },
          loading: false,
        });
      });
    }, 180);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [onPreview, operation, target]);

  const isHardReset = operation === "reset" && resetMode === "hard";
  const confirmationText = branch || "reset";
  const hardResetConfirmed = !isHardReset || hardConfirmation === confirmationText;
  const cherryParentRequired = operation === "cherry-pick" && mergeParents > 1;
  const canRun = !busy && !previewLoading && Boolean(preview) && Boolean(target.trim())
    && hardResetConfirmed && (!cherryParentRequired || mainline !== null);

  async function copyCommand() {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setCopyFailure("");
      window.setTimeout(() => setCopied(false), 1600);
    } catch (cause) {
      setCopyFailure(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function submit() {
    if (!canRun) return;
    await onRun(request);
  }

  const title = operationTitle(operation);
  return (
    <div className="dialog-backdrop" role="presentation">
      <section className={`discard-dialog advanced-dialog ${isHardReset ? "advanced-hard-reset" : ""}`} role="dialog" aria-modal="true" aria-labelledby="advanced-dialog-title">
        <h2 id="advanced-dialog-title">{title}</h2>
        {operation === "merge" && <p>Fazer merge de <code>{target || "branch"}</code> em <code>{branch ?? "HEAD atual"}</code>.</p>}
        {operation === "rebase" && <p>Fazer rebase de <code>{branch ?? "HEAD atual"}</code> sobre <code>{target || "destino"}</code>.</p>}
        {operation === "cherry-pick" && <p>Aplicar o commit <code>{preview?.targetHash?.slice(0, 12) ?? target}</code> sobre <code>{branch ?? "HEAD atual"}</code>.</p>}
        {operation === "reset" && <p>Redefinir <code>{branch ?? "HEAD destacado"}</code> do commit <code>{preview?.currentHead?.slice(0, 12) ?? "HEAD"}</code> para o commit selecionado.</p>}

        {operation === "reset" && (
          <>
            <div className="advanced-quick-targets">
              <span>Destino rápido</span>
              <Button variant="outline" onClick={() => setTarget("HEAD")}>HEAD</Button>
              <Button variant="outline" onClick={() => setTarget("HEAD~1")}>HEAD~1</Button>
            </div>
            <fieldset className="advanced-mode-options">
              <legend>Modo de reset</legend>
              <label><input type="radio" name="reset-mode" checked={resetMode === "soft"} onChange={() => setResetMode("soft")} /> <code>--soft</code><span>Mover HEAD; manter o índice e o working tree.</span></label>
              <label><input type="radio" name="reset-mode" checked={resetMode === "mixed"} onChange={() => setResetMode("mixed")} /> <code>--mixed</code><span>Mover HEAD e redefinir o índice; manter o working tree.</span></label>
              <label><input type="radio" name="reset-mode" checked={resetMode === "hard"} onChange={() => setResetMode("hard")} /> <code>--hard</code><span>Mover HEAD, redefinir o índice e descartar alterações locais rastreadas.</span></label>
            </fieldset>
          </>
        )}

        <label className="advanced-target-label">
          {operation === "merge" ? "Branch para merge" : operation === "rebase" ? "Destino do rebase" : operation === "cherry-pick" ? "Commit para cherry-pick" : "Commit de destino"}
          <input
            autoFocus
            list="advanced-target-options"
            value={target}
            onChange={(event) => setTarget(event.target.value)}
          />
        </label>
        <datalist id="advanced-target-options">
          {branches?.local.map((item) => <option key={`local:${item.name}`} value={item.name}>Branch local</option>)}
          {branches?.remote.map((item) => <option key={`remote:${item.name}`} value={item.name}>Branch remota</option>)}
        </datalist>

        {operation === "merge" && (
          <fieldset className="advanced-checks">
            <legend>Opções de merge</legend>
            <label><input type="checkbox" checked={noFf} onChange={(event) => { setNoFf(event.target.checked); if (event.target.checked) { setFfOnly(false); setSquash(false); } }} /> <code>--no-ff</code> Sempre criar um commit de merge</label>
            <label><input type="checkbox" checked={ffOnly} onChange={(event) => { setFfOnly(event.target.checked); if (event.target.checked) { setNoFf(false); setSquash(false); } }} /> <code>--ff-only</code> Abortar se não for possível fazer fast-forward</label>
            <label><input type="checkbox" checked={squash} onChange={(event) => { setSquash(event.target.checked); if (event.target.checked) { setNoFf(false); setFfOnly(false); } }} /> <code>--squash</code> Preparar alterações sem criar commit de merge</label>
            <label><input type="checkbox" checked={noCommit} onChange={(event) => setNoCommit(event.target.checked)} /> <code>--no-commit</code> Parar antes de criar um commit</label>
          </fieldset>
        )}
        {operation === "rebase" && (
          <>
            {(target.startsWith("refs/remotes/") || branches?.remote.some((item) => item.name === target)) && (
              <p className="advanced-note">As refs remotas podem estar desatualizadas. Faça fetch antes, se necessário; esta operação local não fará fetch.</p>
            )}
            <fieldset className="advanced-checks">
              <legend>Opções de rebase</legend>
              <label><input type="checkbox" checked={autostash} onChange={(event) => setAutostash(event.target.checked)} /> <code>--autostash</code> Guardar temporariamente (stash) e restaurar as alterações locais</label>
            </fieldset>
            <p className="advanced-note">Continuar usa <code>GIT_EDITOR=true</code> e aceita a mensagem de commit existente; o editor não será aberto.</p>
          </>
        )}
        {operation === "cherry-pick" && (
          <>
            {cherryParentRequired && (
              <fieldset className="advanced-checks">
                <legend>Linha principal do commit de merge</legend>
                <p>Um commit de merge precisa de um commit pai para definir quais alterações aplicar.</p>
                <label><input type="radio" name="cherry-mainline" checked={mainline === 1} onChange={() => setMainline(1)} /> <code>-m 1</code> Primeiro pai</label>
                <label><input type="radio" name="cherry-mainline" checked={mainline === 2} onChange={() => setMainline(2)} /> <code>-m 2</code> Segundo pai</label>
              </fieldset>
            )}
            <fieldset className="advanced-checks">
              <legend>Opções de cherry-pick</legend>
              <label><input type="checkbox" checked={recordOrigin} onChange={(event) => setRecordOrigin(event.target.checked)} /> <code>-x</code> Registrar a origem do cherry-pick</label>
              <label><input type="checkbox" checked={noCommit} onChange={(event) => setNoCommit(event.target.checked)} /> <code>--no-commit</code> Aplicar sem criar um commit</label>
            </fieldset>
            <p className="advanced-note">Continuar usa <code>GIT_EDITOR=true</code> e aceita a mensagem de commit existente.</p>
          </>
        )}

        {previewLoading && <p className="advanced-loading"><LoaderCircle size={13} className="spin" />Calculando impacto…</p>}
        {preview && (
          <div className="advanced-impact">
            {operation === "merge" && <p>{preview.commitCount} commits recebidos · {preview.fastForwardPossible ? "fast-forward possível" : "fast-forward impossível"}</p>}
            {operation === "rebase" && <p>{preview.commitCount} commits para reaplicar · merge-base <code>{preview.mergeBase?.slice(0, 12) ?? "indisponível"}</code></p>}
            {operation === "cherry-pick" && <p><code>{preview.targetHash?.slice(0, 12)}</code> · {preview.targetSubject || "(sem descrição)"}</p>}
            {operation === "reset" && (
              <>
                <p>Atual: <code>{preview.currentHead?.slice(0, 12)}</code> {preview.currentSubject || ""}</p>
                <p>Destino: <code>{preview.targetHash?.slice(0, 12)}</code> {preview.targetSubject || ""}</p>
                <p>{preview.commitCount} commits deixarão de estar acessíveis por esta branch.</p>
                {preview.published && <p className="advanced-warning">Alguns commits afetados já foram publicados. Um próximo push exigirá force.</p>}
                {isHardReset && (
                  <>
                    <p className="advanced-warning">⚠ Hard reset — Isso descartará as alterações locais. Arquivos não rastreados não são afetados por <code>reset --hard</code>.</p>
                    <p>HEAD anterior: <code>{preview.currentHead}</code> · recuperação: <code>git reset --hard {preview.currentHead}</code></p>
                    {preview.affectedFiles.length
                      ? <ul>{preview.affectedFiles.map((file) => <li key={file}>{file}</li>)}</ul>
                      : <p>Nenhuma alteração local rastreada será perdida.</p>}
                  </>
                )}
              </>
            )}
            {operation === "rebase" && preview.published && <p className="advanced-warning">Isso reescreve commits já publicados. Um próximo push exigirá force.</p>}
          </div>
        )}
        {previewFailure && <p className="advanced-error">{previewFailure.stderr || previewFailure.stdout}</p>}
        <div className="advanced-command">
          <strong>Comando</strong>
          <code>{command}</code>
          <Button variant="outline" onClick={() => onRunInTerminal(["-C", repository, ...operationArgs(request)])}>Executar no terminal</Button>
          <Button variant="ghost" size="icon" aria-label="Copiar comando" title="Copiar comando" onClick={() => void copyCommand()}>{copied ? <Check size={14} /> : <Copy size={14} />}</Button>
        </div>
        {copyFailure && <p className="advanced-error">{copyFailure}</p>}
        {isHardReset && (
          <label className="advanced-confirm-label">
            Digite <code>{confirmationText}</code> para confirmar
            <input value={hardConfirmation} onChange={(event) => setHardConfirmation(event.target.value)} />
          </label>
        )}
        <div className="dialog-actions">
          <Button variant="outline" disabled={busy} onClick={onClose}>Cancelar</Button>
          <Button className={isHardReset ? "destructive-button" : ""} disabled={!canRun} onClick={() => void submit()}>
            {busy ? <><LoaderCircle size={13} className="spin" />Executando…</> : operation === "reset" ? "Resetar" : title}
          </Button>
        </div>
      </section>
    </div>
  );
}

function operationTitle(operation: AdvancedOperation): string {
  switch (operation) {
    case "merge": return "Fazer merge da branch";
    case "rebase": return "Fazer rebase sobre";
    case "cherry-pick": return "Fazer cherry-pick do commit";
    case "reset": return "Resetar";
  }
}

function operationArgs(request: AdvancedRequest): string[] {
  switch (request.operation) {
    case "merge":
      return ["merge", ...(request.noFf ? ["--no-ff"] : []), ...(request.ffOnly ? ["--ff-only"] : []), ...(request.squash ? ["--squash"] : []), ...(request.noCommit ? ["--no-commit"] : []), request.target || "<branch>"];
    case "rebase":
      return ["rebase", ...(request.autostash ? ["--autostash"] : []), request.target || "<destination>"];
    case "cherry-pick":
      return ["cherry-pick", ...(request.mainline ? ["-m", String(request.mainline)] : []), ...(request.recordOrigin ? ["-x"] : []), ...(request.noCommit ? ["--no-commit"] : []), request.target || "<commit>"];
    case "reset":
      return ["reset", `--${request.mode ?? "mixed"}`, request.target || "<target>"];
  }
}
