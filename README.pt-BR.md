<div align="center">

# 🌿 Twig

**Git CLI para quem quer usar o mouse.**

Uma interface desktop moderna, leve e transparente sobre o Git que você já tem instalado.

![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-TypeScript-61DAFB?logo=react&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-backend-000000?logo=rust&logoColor=white)
![Windows](https://img.shields.io/badge/Windows-10%2B-0078D4?logo=windows&logoColor=white)
[![deploy: no cuzinho](https://img.shields.io/badge/deploy-no%20cuzinho-d6331f)](https://cuzinho.dev)

**[English](./README.md)** · **Português (BR)**

<img src="./twig-icon.png" alt="Twig" width="215" />

</div>

---

## Por que o Twig?

O Twig **não reimplementa o Git** e não tenta competir com GitKraken, Fork ou GitHub Desktop. Ele é uma camada visual fina sobre o `git.exe` instalado na sua máquina, pensada para quem já conhece Git e quer executar as operações comuns sem digitar os mesmos comandos o dia inteiro.

- **Git de verdade.** Cada ação executa o comando Git real, e as mensagens originais do Git nunca são escondidas.
- **Respeita a sua configuração.** Usa seu `.gitconfig`, seu SSH, seu Credential Manager. Sem login próprio, sem tokens, sem servidor intermediário.
- **Terminal integrado de verdade.** PTY completo, com prompts interativos e passphrase SSH, no mesmo repositório que está aberto.
- **Leve e rápido.** Tauri + Rust, sem Electron.
- **Dark e light mode.**

## Funcionalidades

### Changes
- Arquivos *staged* e *unstaged* separados, com status (modificado, novo, removido, renomeado)
- Stage / unstage individual ou em lote
- Descarte de alterações com confirmação
- Diff textual com limites para arquivos grandes
- Atualização automática ao recuperar o foco da janela

### Commit
- Commit, **Commit & Push** e **Amend** do último commit
- Bloqueio de commit sem mensagem

### Branches
- Listar, criar, trocar (*switch*), renomear e deletar branches locais
- Branch atual e upstream sempre visíveis

### Remote
- Fetch, Pull e Push com indicador de **ahead/behind**
- Operações de rede canceláveis, com timeout configurável
- Menu de remotes com URLs de fetch/push (credenciais mascaradas)

### History
- Lista virtualizada e paginada de commits
- Busca feita pelo próprio Git (mensagem, autor ou hash)
- Escopo por branch e filtro por arquivo
- Detalhes do commit: metadados, pais, refs, estatísticas e diff
- Copiar hash

### Git avançado
- **Merge**, **Rebase**, **Cherry-pick** e **Reset** (soft, mixed e hard)
- Os argumentos exatos do comando são exibidos antes da confirmação
- Detecta operações em andamento e oferece **Continue**, **Skip** e **Abort**
- Avisos para rebase/reset sobre commits já publicados, com hash anterior para recuperação
- Pull, Switch e Commit são bloqueados enquanto há uma operação ativa

### Terminal integrado
- Sessões PTY persistentes, com abas para múltiplas sessões
- Começa no repositório aberto e herda o ambiente do usuário
- Comandos interativos, editores do Git e prompts SSH
- Atalho: ``Ctrl+` `` ou `Ctrl+J`
- Buffer limitado, nunca gravado em disco
- Confirmação antes de fechar a janela com sessões ativas

## Começando

### Requisitos

- [Node.js](https://nodejs.org/) (LTS)
- [Rust](https://www.rust-lang.org/tools/install) e os [pré-requisitos do Tauri para Windows](https://tauri.app/start/prerequisites/)
- [Git for Windows](https://git-scm.com/download/win)

### Rodando em desenvolvimento

```powershell
git clone https://github.com/<seu-usuario>/twig.git
cd twig
npm install
npm run tauri dev
```

### Gerando o instalador / .exe

```powershell
npm run tauri build
```

Os artefatos ficam em `src-tauri\target\release\bundle\`.

### Validação local

```powershell
cargo check  --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
cargo test   --manifest-path src-tauri\Cargo.toml
npx tsc --noEmit
npm run lint
```

## SSH no Windows

O Twig **não coleta nem armazena passphrases**. Quem cuida da autenticação é o Git/SSH, como sempre.

**Fetch, Pull e Push pela interface** rodam sem prompts interativos e têm timeout de 60 s por padrão (ajustável com a variável de ambiente `GIT_GUI_NETWORK_TIMEOUT_SECS`, entre 1 e 86400). Se a operação precisar de passphrase, abra o erro e escolha **Run in terminal**: o comando é escrito no prompt **sem Enter**, você revisa e executa, e o SSH pede a passphrase dentro do PTY (com echo desativado). Nada do que é digitado ou exibido é gravado em log ou em disco. O terminal funciona sem `ssh-agent`.

<details>
<summary><strong>Usando o ssh-agent (opcional)</strong></summary>

Com a chave carregada no agente, Fetch/Pull/Push funcionam direto pela interface, sem pedir passphrase.

Em um PowerShell **como administrador**, habilite o serviço `OpenSSH Authentication Agent`:

```powershell
Set-Service ssh-agent -StartupType Automatic
Start-Service ssh-agent
```

Em um PowerShell normal, adicione a chave e confira se foi carregada:

```powershell
ssh-add $env:USERPROFILE\.ssh\id_ed25519
ssh-add -l
```

> **Atenção:** o cliente SSH usado pelo Git precisa acessar o mesmo agente. No Windows, `where.exe ssh` pode listar dois `ssh.exe` (o OpenSSH do Windows e o do Git for Windows), que podem usar agentes diferentes. Verifique também `SSH_AUTH_SOCK`, se definido.

`git config --show-origin --get core.sshCommand` mostra se o repositório já escolhe um cliente SSH. O Twig respeita `core.sshCommand` e `GIT_SSH_COMMAND`; nas operações de rede da interface, acrescenta `-o BatchMode=yes` para evitar bloqueio sem TTY. O terminal integrado **não** injeta `GIT_TERMINAL_PROMPT=0` nem `BatchMode`, para que Git/SSH possam interagir normalmente.

</details>

## Segurança e privacidade

- Nenhuma senha, passphrase ou token é armazenado ou enviado a servidores.
- Logs da aplicação não contêm dados sensíveis.
- URLs de remotes são exibidas com credenciais mascaradas.
- Operações destrutivas (descarte, hard reset, rebase) exigem confirmação explícita.

## Arquitetura

```text
Frontend (React + TypeScript + Tailwind)
        ↓
Comandos Tauri
        ↓
Git Service  →  Process Runner / PTY
        ↓
git.exe instalado na máquina
```

```text
src/          interface (components, hooks, pages, lib, types)
src-tauri/    ponte Rust: commands, git, process, terminal
```

Sempre que possível, o Twig usa saídas estruturadas do Git (como `git status --porcelain=v2`) em vez de interpretar texto feito para humanos.

## Fora do escopo (por enquanto)

Stash, resolução visual de conflitos, force-push, push de tags, Git LFS, submodules, worktrees, reflog, visualização de grafo (DAG), integração com APIs do GitHub/GitLab (PRs, issues, CI) e gerenciamento de contas ou chaves SSH.

Conflitos podem ser resolvidos no terminal integrado e marcados como resolvidos ao fazer *stage* dos arquivos.

## Contribuindo

Issues e sugestões são bem-vindos. Antes de abrir um PR, rode a [validação local](#validação-local). A regra de ouro do projeto: entre criar mais uma abstração visual e simplesmente executar o comando Git real mostrando o resultado de forma clara, **prefira executar o comando Git real**.

## Licença

Distribuído sob a licença MIT. Veja o arquivo [`LICENSE`](LICENSE).

---

<div align="center">

*Git continua sendo o Git. O Twig só torna o uso dele melhor.*

</div>
