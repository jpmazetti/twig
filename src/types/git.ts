export interface GitInstallation {
  version: string;
  path: string;
}

export interface ChangedFile {
  path: string;
  originalPath: string | null;
  code: string;
  status: "untracked" | "changed" | "renamed" | "conflicted";
  indexStatus: string;
  worktreeStatus: string;
}

export interface DiffLine {
  kind: "context" | "addition" | "removal" | "marker";
  content: string;
  oldLine: number | null;
  newLine: number | null;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface DiffPreview {
  binary: boolean;
  tooLarge: boolean;
  metadata: string[];
  hunks: DiffHunk[];
}

export type WorkingTreeOperation =
  | "Stage"
  | "Stage all"
  | "Unstage"
  | "Unstage all"
  | "Discard"
  | "Discard untracked"
  | "Discard all tracked"
  | "Discard all untracked";

export interface RepositoryStatus {
  branch: string | null;
  upstream: string | null;
  headOid: string | null;
  ahead: number | null;
  behind: number | null;
  files: ChangedFile[];
}

export interface BranchInfo {
  name: string;
  current: boolean;
  upstream: string | null;
  upstreamGone: boolean;
  ahead: number | null;
  behind: number | null;
  shortHash: string;
  commitDate: string;
  subject: string;
}

export interface BranchList {
  local: BranchInfo[];
  remote: BranchInfo[];
  detachedHead: string | null;
}

export interface CommitContext {
  lastMessage: string | null;
  mergeMessage: string | null;
  mergeInProgress: boolean;
  otherOperation: string | null;
  published: boolean;
}

export interface CommitSummary {
  shortHash: string;
  subject: string;
}

export interface HistoryRef {
  name: string;
  kind: "head" | "local" | "remote" | "tag";
  head: boolean;
}

export interface HistoryCommit {
  hash: string;
  shortHash: string;
  parents: string[];
  author: string;
  authorEmail: string;
  authorDate: string;
  committer: string;
  committerEmail: string;
  commitDate: string;
  subject: string;
  body: string;
  refs: HistoryRef[];
}

export interface HistoryPage {
  commits: HistoryCommit[];
  hasMore: boolean;
  shallow: boolean;
  detached: boolean;
  headHash: string | null;
  upstream: string | null;
  refsHash: string;
}

export interface HistoryFingerprint {
  headHash: string | null;
  refsHash: string;
}

export interface HistoryFile {
  path: string;
  originalPath: string | null;
  status: string;
  additions: number | null;
  deletions: number | null;
  binary: boolean;
}

export interface HistoryDetail {
  commit: HistoryCommit;
  files: HistoryFile[];
  shortstat: string;
  comparisonParent: string | null;
}

export interface PushOutput {
  stdout: string;
  stderr: string;
  args: string[];
}

export interface RemoteInfo {
  name: string;
  fetchUrl: string;
  pushUrl: string;
}

export interface RepositoryOperationState {
  operation: "merge" | "rebase" | "cherry-pick" | "revert" | null;
  conflicts: string[];
  branch: string | null;
  detached: boolean;
  dirty: boolean;
  hasCommits: boolean;
  progress: string | null;
  currentCommit: string | null;
  previousHead: string | null;
}

export interface AdvancedPreview {
  currentHead: string | null;
  currentSubject: string | null;
  targetHash: string | null;
  targetSubject: string | null;
  commitCount: number;
  mergeBase: string | null;
  fastForwardPossible: boolean | null;
  published: boolean;
  affectedFiles: string[];
}

export interface AdvancedRequest {
  operation: "merge" | "rebase" | "cherry-pick" | "reset";
  target: string;
  mode: "soft" | "mixed" | "hard" | null;
  noFf: boolean;
  ffOnly: boolean;
  squash: boolean;
  noCommit: boolean;
  autostash: boolean;
  recordOrigin: boolean;
  mainline: 1 | 2 | null;
}

export interface OperationActionRequest {
  operation: "merge" | "rebase" | "cherry-pick";
  action: "continue" | "skip" | "abort";
}

export interface AdvancedOutput {
  stdout: string;
  stderr: string;
  args: string[];
  previousHead: string | null;
  currentHead: string | null;
}

export interface CommandFailure {
  ok: false;
  operation: string;
  args: string[];
  exitCode: number | null;
  stderr: string;
  stdout: string;
}

export type CommandResult<T> = { ok: true; data: T } | CommandFailure;
