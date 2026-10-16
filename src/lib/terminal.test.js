import assert from "node:assert/strict";
import test from "node:test";
import { buildCdCommand, buildGitCommand, quoteShellArgument } from "./terminal.js";

test("quotes paths with spaces, accents and apostrophes for PowerShell", () => {
  assert.equal(
    quoteShellArgument("PowerShell 7", "C:\\Dev\\ação d'água"),
    "'C:\\Dev\\ação d''água'",
  );
});

test("quotes paths with apostrophes for Git Bash", () => {
  assert.equal(
    quoteShellArgument("Git Bash", "/work/ação d'água"),
    "'/work/ação d'\\''água'",
  );
});

test("builds a shell-quoted git command without an Enter key", () => {
  const command = buildGitCommand("PowerShell 7", "C:\\Dev Repo\\árvore", ["push"]);
  assert.equal(command, "git -C 'C:\\Dev Repo\\árvore' 'push'");
  assert.equal(command.endsWith("\r"), false);
  assert.equal(command.endsWith("\n"), false);
});

test("appends Enter only when terminal execution is explicitly requested", () => {
  assert.equal(
    buildGitCommand("PowerShell 7", "C:\\repo", ["fetch", "--prune"], true),
    "git -C 'C:\\repo' 'fetch' '--prune'\r",
  );
});

test("quotes cd commands without appending Enter", () => {
  assert.equal(buildCdCommand("Git Bash", "/work/repo with spaces"), "cd -- '/work/repo with spaces'");
  assert.equal(buildCdCommand("Command Prompt", "C:\\repo"), 'cd /d "C:\\repo"');
});

test("rejects command prompt values that could escape its quoting", () => {
  assert.throws(() => quoteShellArgument("Command Prompt", 'C:\\repo" & whoami'));
  assert.throws(() => quoteShellArgument("Command Prompt", "%PATH%"));
});
