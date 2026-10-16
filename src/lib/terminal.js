function quotePowerShell(value) {
  return `'${value.replaceAll("'", "''")}'`;
}

function quoteBash(value) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function quoteCmd(value) {
  if (/["\r\n%!]/.test(value)) {
    throw new Error("This path or argument cannot be safely quoted in Command Prompt.");
  }
  return `"${value}"`;
}

export function quoteShellArgument(shell, value) {
  if (shell === "PowerShell 7" || shell === "Windows PowerShell") return quotePowerShell(value);
  if (shell === "Git Bash") return quoteBash(value);
  if (shell === "Command Prompt") return quoteCmd(value);
  throw new Error(`Unsupported terminal shell: ${shell}`);
}

export function buildShellCommand(shell, args) {
  return args.map((argument) => quoteShellArgument(shell, argument)).join(" ");
}

export function buildGitCommand(shell, repository, gitArgs, execute = false) {
  const command = `git -C ${quoteShellArgument(shell, repository)} ${gitArgs.map(
    (argument) => quoteShellArgument(shell, argument),
  ).join(" ")}`;
  return execute ? `${command}\r` : command;
}

export function buildCdCommand(shell, repository) {
  if (shell === "PowerShell 7" || shell === "Windows PowerShell") {
    return `Set-Location -LiteralPath ${quotePowerShell(repository)}`;
  }
  if (shell === "Git Bash") {
    return `cd -- ${quoteBash(repository)}`;
  }
  if (shell === "Command Prompt") {
    return `cd /d ${quoteCmd(repository)}`;
  }
  throw new Error(`Unsupported terminal shell: ${shell}`);
}
