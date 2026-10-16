import type { DiffPreview } from "../types/git";
import { Button } from "./ui/button";

export function DiffView({
  diff,
  onLoadLarge,
  allowLarge = false,
}: {
  diff: DiffPreview;
  onLoadLarge: () => void;
  allowLarge?: boolean;
}) {
  if (diff.tooLarge && !allowLarge) {
    return <div className="diff-empty large-diff-warning">Este diff tem mais de 128 KB. <Button variant="outline" onClick={onLoadLarge}>Carregar mesmo assim</Button></div>;
  }
  if (diff.binary) return <div className="diff-empty">Arquivo binário, sem prévia.</div>;
  return (
    <div className="diff-content">
      {diff.metadata.length > 0 && <div className="diff-metadata">{diff.metadata.map((line, index) => <div key={`${index}-${line}`}>{line}</div>)}</div>}
      {diff.hunks.map((hunk, index) => (
        <div className="diff-hunk" key={`${hunk.header}-${index}`}>
          <div className="diff-hunk-header">{hunk.header}</div>
          {hunk.lines.map((line, lineIndex) => (
            <div className={`diff-line diff-line-${line.kind}`} key={`${lineIndex}-${line.content}`}>
              <span className="diff-line-number">{line.oldLine ?? ""}</span>
              <span className="diff-line-number">{line.newLine ?? ""}</span>
              <code title={line.content.endsWith("\r") ? "CRLF line ending" : undefined}>{line.content.replace(/\r$/, "")}</code>
            </div>
          ))}
        </div>
      ))}
      {diff.hunks.length === 0 && <div className="diff-empty">Nenhuma alteração textual.</div>}
    </div>
  );
}
