export function buildCommitMessage(subject: string, body: string): string;
export function canCommit(message: string, stagedCount: number, amend: boolean): boolean;
export function splitCommitMessage(message: string): { subject: string; body: string };
export function needsInteractiveInput(stderr: string, stdout: string): boolean;
export function formatGitCommand(args: string[]): string;
export function classifyNetworkFailure(
  stderr: string,
  stdout?: string,
): "ssh-auth" | "host-key" | "https-auth" | "network" | "repository-access" | "non-fast-forward" | "divergent-pull" | "conflict" | "other";
export function maskCredentials(value: string): string;
