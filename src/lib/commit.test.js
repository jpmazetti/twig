import assert from "node:assert/strict";
import test from "node:test";

import {
  buildCommitMessage,
  canCommit,
  classifyNetworkFailure,
  formatGitCommand,
  maskCredentials,
  needsInteractiveInput,
  splitCommitMessage,
} from "./commit.js";

test("assembles a trimmed subject and optional body with one blank line", () => {
  assert.equal(buildCommitMessage("  Fix billing  ", "  Handle fees\n\n safely  "), "Fix billing\n\nHandle fees\n\n safely");
  assert.equal(buildCommitMessage("  Fix billing  ", " \n "), "Fix billing");
});

test("preserves UTF-8 accents and emoji", () => {
  assert.equal(buildCommitMessage("Corrigir cobrança 🚀", "Preço atualizado: café"), "Corrigir cobrança 🚀\n\nPreço atualizado: café");
});

test("splits a previous commit message into subject and body", () => {
  assert.deepEqual(splitCommitMessage("Fix billing\n\nHandle fees\n"), {
    subject: "Fix billing",
    body: "Handle fees",
  });
});

test("enables commits only for non-empty messages and staged files, except amend", () => {
  assert.equal(canCommit(" \n ", 1, false), false);
  assert.equal(canCommit("Fix billing", 0, false), false);
  assert.equal(canCommit("Fix billing", 1, false), true);
  assert.equal(canCommit("Fix billing", 0, true), true);
});

test("recognizes SSH and credential prompts from Git output", () => {
  assert.equal(needsInteractiveInput("Enter passphrase for key 'id_ed25519':", ""), true);
  assert.equal(needsInteractiveInput("", "Permission denied (publickey)."), true);
  assert.equal(needsInteractiveInput("fatal: unable to access: 403 Forbidden", ""), false);
});

test("formats a copyable PowerShell Git command", () => {
  assert.equal(formatGitCommand(["-C", "C:\\repo", "push", "remote's-name"]), "git '-C' 'C:\\repo' 'push' 'remote''s-name'");
});

test("classifies network, SSH, HTTPS, pull, and push error fixtures", () => {
  const realSshFailure = [
    "Permission denied, please try again.",
    "Permission denied, please try again.",
    "git@ssh.dev.azure.com: Permission denied (password,publickey).",
    "fatal: Could not read from remote repository.",
  ].join("\n");
  assert.equal(classifyNetworkFailure("Enter passphrase for key 'id_ed25519':"), "ssh-auth");
  assert.equal(classifyNetworkFailure("sign_and_send_pubkey: signing failed"), "ssh-auth");
  assert.equal(classifyNetworkFailure("Could not open a connection to your authentication agent"), "ssh-auth");
  assert.equal(classifyNetworkFailure("Permission denied (publickey)."), "ssh-auth");
  assert.equal(classifyNetworkFailure(realSshFailure), "ssh-auth");
  assert.equal(classifyNetworkFailure("Permission denied, please try again."), "ssh-auth");
  assert.equal(classifyNetworkFailure("Permission denied (password,publickey)."), "ssh-auth");
  assert.equal(classifyNetworkFailure("fatal: Could not read from remote repository."), "ssh-auth");
  assert.equal(classifyNetworkFailure("Host key verification failed."), "host-key");
  assert.equal(classifyNetworkFailure("REMOTE HOST IDENTIFICATION HAS CHANGED"), "host-key");
  assert.equal(classifyNetworkFailure("fatal: could not read Username for remote"), "https-auth");
  assert.equal(classifyNetworkFailure("fatal: Authentication failed"), "https-auth");
  assert.equal(classifyNetworkFailure("ssh: Could not resolve hostname example.invalid"), "network");
  assert.equal(classifyNetworkFailure("Connection timed out"), "network");
  assert.equal(classifyNetworkFailure("Connection refused"), "network");
  assert.equal(classifyNetworkFailure("ERROR: Repository not found."), "repository-access");
  assert.equal(classifyNetworkFailure("remote: Access denied"), "repository-access");
  assert.equal(classifyNetworkFailure("rejected: non-fast-forward"), "non-fast-forward");
  assert.equal(classifyNetworkFailure("fatal: Need to specify how to reconcile divergent branches."), "divergent-pull");
  assert.equal(classifyNetworkFailure("CONFLICT (content): Merge conflict in file.txt"), "conflict");
});

test("masks embedded credentials in HTTPS URLs", () => {
  assert.equal(
    maskCredentials("fatal: https://alice:secret@host.invalid/repo"),
    "fatal: https://[redacted]@host.invalid/repo",
  );
});
