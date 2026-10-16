import { useState } from "react";
import { AlertCircle, RefreshCw } from "lucide-react";
import { Button } from "./ui/button";
import { formatGitCommand } from "../lib/commit";
import type { OperationActionRequest, RepositoryOperationState } from "../types/git";

export function OperationBanner({
  repository,
  state,
  busy,
  onAction,
  onRefresh,
  onOpenTerminal,
}: {
  repository: string;
  state: RepositoryOperationState | null;
  busy: boolean;
  onAction: (request: OperationActionRequest) => Promise<unknown>;
  onRefresh: () => void;
  onOpenTerminal: () => void;
}) {
  const [confirmAbort, setConfirmAbort] = useState(false);
  if (!state?.operation) return null;

  const operation = state.operation;
  const actionableOperation = operation === "merge" || operation === "rebase" || operation === "cherry-pick"
    ? operation
    : null;
  const supportsActions = actionableOperation !== null;
  const continueCommand = operation === "merge" ? "merge" : operation;
  const operationName = operation === "cherry-pick" ? "cherry-pick" : operation;
  const label = operation === "revert"
    ? "Revert em andamento — conclua fora do aplicativo"
    : `${operationName[0].toUpperCase()}${operationName.slice(1)} em andamento${state.conflicts.length ? ` — ${state.conflicts.length} ${state.conflicts.length === 1 ? "conflito" : "conflitos"}` : ""}`;

  function run(action: OperationActionRequest["action"]) {
    if (!actionableOperation) return;
    if (action === "abort") {
      setConfirmAbort(true);
      return;
    }
    void onAction({ operation: actionableOperation, action });
  }

  return (
    <section className="operation-banner" role="status">
      <div className="operation-banner-heading">
        <AlertCircle size={16} />
        <strong>{label}</strong>
        {state.progress && <span>{state.progress}</span>}
        {state.currentCommit && <code>{state.currentCommit.slice(0, 12)}</code>}
      </div>
      {state.conflicts.length > 0 && (
        <div className="operation-conflicts">
          <strong>Resolva estes arquivos e adicione cada resolução ao stage antes de continuar:</strong>
          <ul>{state.conflicts.map((file) => <li key={file}><code>{file}</code></li>)}</ul>
        </div>
      )}
      {state.previousHead && operation === "rebase" && (
        <p className="operation-editor-note">HEAD original: <code>{state.previousHead.slice(0, 12)}</code> · recupere com <code>git reset --hard {state.previousHead}</code>.</p>
      )}
      {operation !== "revert" && <p className="operation-editor-note">Continuar aceita a mensagem de commit existente com <code>GIT_EDITOR=true</code>; nenhum editor será aberto.</p>}
      {supportsActions ? (
        <div className="operation-banner-actions">
          <span className="disabled-tooltip" title={state.conflicts.length ? "Resolva os conflitos e adicione os arquivos ao stage antes de continuar." : undefined}>
            <Button disabled={busy || state.conflicts.length > 0} onClick={() => run("continue")}>Continuar</Button>
          </span>
          {(operation === "rebase" || operation === "cherry-pick") && <Button variant="outline" disabled={busy} onClick={() => run("skip")}>Pular</Button>}
          <Button variant="outline" disabled={busy} onClick={() => run("abort")}>Abortar</Button>
          <Button variant="outline" disabled={busy} onClick={onRefresh}><RefreshCw size={13} />Atualizar</Button>
          <Button variant="outline" onClick={onOpenTerminal}>Abrir terminal</Button>
        </div>
      ) : (
        <div className="operation-banner-actions">
          <Button variant="outline" disabled={busy} onClick={onRefresh}><RefreshCw size={13} />Atualizar</Button>
          <Button variant="outline" onClick={onOpenTerminal}>Abrir terminal</Button>
        </div>
      )}
      {supportsActions && (
        <div className="operation-command-list">
          <span>Comandos:</span>
          <code>{formatGitCommand(["-C", repository, continueCommand, "--continue"])}</code>
          {(operation === "rebase" || operation === "cherry-pick") && <code>{formatGitCommand(["-C", repository, operation, "--skip"])}</code>}
          <code>{formatGitCommand(["-C", repository, continueCommand, "--abort"])}</code>
        </div>
      )}
      {confirmAbort && (
        <div className="operation-abort-confirm" role="alertdialog" aria-label="Confirmar cancelamento da operação">
          <span>Abortar {operation} e descartar o progresso, retornando ao estado anterior?</span>
          <code>{formatGitCommand(["-C", repository, continueCommand, "--abort"])}</code>
          <Button variant="outline" disabled={busy} onClick={() => setConfirmAbort(false)}>Cancelar</Button>
          <Button className="destructive-button" disabled={busy} onClick={() => {
            setConfirmAbort(false);
            if (actionableOperation) void onAction({ operation: actionableOperation, action: "abort" });
          }}>Abortar {operation}</Button>
        </div>
      )}
    </section>
  );
}
