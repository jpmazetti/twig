export function buildCommitMessage(subject, body) {
  const trimmedSubject = subject.trim();
  const trimmedBody = body.trim();
  return trimmedBody ? `${trimmedSubject}\n\n${trimmedBody}` : trimmedSubject;
}

export function canCommit(message, stagedCount, amend) {
  return message.trim().length > 0 && (amend || stagedCount > 0);
}

export function splitCommitMessage(message) {
  const normalized = message.trim();
  const separator = normalized.indexOf("\n");
  return separator < 0
    ? { subject: normalized, body: "" }
    : { subject: normalized.slice(0, separator).trim(), body: normalized.slice(separator + 1).trim() };
}

export function needsInteractiveInput(stderr, stdout) {
  const output = `${stderr}\n${stdout}`.toLowerCase();
  return [
    "enter passphrase for key",
    "permission denied (publickey)",
    "permission denied (password,publickey)",
    "permission denied, please try again",
    "could not read from remote repository",
    "could not read username",
    "terminal prompts disabled",
    "git requested interactive input",
  ].some((marker) => output.includes(marker));
}

export function formatGitCommand(args) {
  return ["git", ...args]
    .map((argument, index) => index === 0
      ? argument
      : `'${argument.replaceAll("'", "''")}'`)
    .join(" ");
}

export function classifyNetworkFailure(stderr, stdout = "") {
  const output = `${stderr}\n${stdout}`.toLowerCase();
  if (/enter passphrase|permission denied \((?:password,)?publickey\)|permission denied, please try again|could not read from remote repository|sign_and_send_pubkey: signing failed|could not open a connection to your authentication agent/.test(output)) return "ssh-auth";
  if (/host key verification failed|remote host identification has changed/.test(output)) return "host-key";
  if (/could not read username|authentication failed|terminal prompts disabled/.test(output)) return "https-auth";
  if (/could not resolve hostname|connection timed out|connection refused|operation timed out|process timed out/.test(output)) return "network";
  if (/repository(?: .*?)? not found|does not appear to be a git repository|access denied|not authorized/.test(output)) return "repository-access";
  if (/non-fast-forward|fetch first|failed to push some refs/.test(output)) return "non-fast-forward";
  if (/need to specify how to reconcile divergent branches/.test(output)) return "divergent-pull";
  if (/conflict \(|automatic merge failed|could not apply/.test(output)) return "conflict";
  return "other";
}

export function maskCredentials(value) {
  return value.replace(/(https?:\/\/)[^/@\s]+@/gi, "$1[redacted]@");
}
