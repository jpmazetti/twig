export function quoteShellArgument(shell: string, value: string): string;
export function buildShellCommand(shell: string, args: string[]): string;
export function buildGitCommand(shell: string, repository: string, gitArgs: string[], execute?: boolean): string;
export function buildCdCommand(shell: string, repository: string): string;
