# Git GUI — Especificação do MVP

## 1. Visão do projeto

Criar uma aplicação desktop moderna para Git, focada inicialmente em uso pessoal.

A ideia **não é implementar Git** nem competir com GitKraken/Fork/GitHub Desktop. A aplicação deve ser uma camada visual limpa sobre o Git CLI instalado na máquina.

Princípio:

> **Git CLI para quem quer usar mouse.**

O usuário já conhece Git e quer executar as operações comuns sem precisar digitar os comandos repetidamente.

Prioridades:

1. Leveza
2. Interface moderna e limpa
3. Experiência rápida
4. Transparência sobre o que o Git está fazendo
5. Compatibilidade com a instalação/configuração de Git existente na máquina
6. Terminal integrado para comandos e operações que exigem interação

---

# 2. Stack definida

## Frontend

- React
- TypeScript
- Tailwind CSS
- Componentes com estética moderna inspirada em shadcn/ui
- Lucide Icons ou equivalente

## Desktop

- Tauri 2

## Backend / integração

- Rust/Tauri apenas como ponte entre a interface e o sistema operacional
- Executar o `git.exe` instalado na máquina
- Não implementar uma engine Git própria

## Plataforma inicial

- Windows

A arquitetura deve permitir futura expansão para macOS/Linux, mas isso não é prioridade do MVP.

---

# 3. Princípio fundamental de integração com Git

A aplicação deve utilizar o Git instalado na máquina do usuário.

Exemplo:

```text
Interface
   ↓
Tauri
   ↓
git.exe
   ↓
SSH / Credential Manager / configuração local
   ↓
GitHub / GitLab / Bitbucket / servidor corporativo
```

Não implementar no MVP:

- sistema próprio de autenticação
- armazenamento de senha Git
- armazenamento de tokens
- OAuth
- gerenciamento próprio de chaves SSH
- servidor intermediário
- API própria de GitHub/GitLab

A aplicação deve respeitar a configuração existente do Git/SSH do sistema sempre que possível.

---

# 4. SSH e terminal integrado

## Requisito importante

O usuário utiliza autenticação SSH com chave privada protegida por senha/passphrase.

Operações como:

```bash
git pull
git push
```

podem solicitar a senha da chave SSH.

Portanto, a aplicação **precisa ter um terminal integrado funcional**.

O terminal não deve ser apenas decorativo. Ele deve permitir:

- executar comandos Git
- executar comandos shell básicos
- visualizar stdout
- visualizar stderr
- receber input interativo
- responder a solicitações como passphrase SSH
- manter uma sessão de terminal enquanto a janela estiver aberta

Exemplo de fluxo:

```text
Usuário clica em Push
        ↓
Aplicação executa:
git push
        ↓
SSH solicita:
Enter passphrase for key...
        ↓
Terminal integrado recebe a solicitação
        ↓
Usuário digita a passphrase
        ↓
Push continua
```

### Importante

Não armazenar a passphrase.

Não fazer log da passphrase.

Fetch, Pull e Push executados em background devem usar `BatchMode=yes` para não bloquear esperando input que não está conectado a um PTY. Em caso de autenticação SSH interativa, oferecer a execução equivalente no terminal sem pressionar Enter. A configuração opcional **Always use terminal for network operations**, desativada por padrão, pode escrever e executar essas operações no PTY; nesse caso, a passphrase é digitada diretamente pelo usuário e nunca é interceptada, armazenada ou registrada.

Não tentar criar um sistema próprio de SSH.

A aplicação deve permitir que o Git/SSH faça seu trabalho normalmente.

---

# 5. Layout geral

A interface deve parecer uma aplicação moderna de 2026, não um software corporativo antigo.

Evitar:

- excesso de botões
- menus complexos
- aparência de TortoiseGit antigo
- interfaces visualmente carregadas
- ícones sem significado
- modais desnecessários

A interface deve ser limpa, escura/clara de forma consistente e com boa hierarquia visual.

Conceito aproximado:

```text
┌─────────────────────────────────────────────────────────────┐
│  ◈ Git GUI                         develop ▼    ↻   ⚙       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  CHANGES                                                    │
│                                                             │
│  Staged        2                                            │
│  ─────────────────────────────────────────────────────────  │
│  ✓ M  src/api/users.py                              [−]      │
│  ✓ M  src/db/models.py                              [−]      │
│                                                             │
│  Unstaged      1                                            │
│  ─────────────────────────────────────────────────────────  │
│  ○ M  README.md                                       [+]    │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Commit message                                        │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│             [ Commit ]      [ Commit & Push ]               │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  BRANCHES                              REMOTE               │
│                                                             │
│  ● develop                             ↑ 0   ↓ 2            │
│    main                                                       │
│    feature/login                       [ Pull ] [ Push ]     │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  HISTORY                                                    │
│                                                             │
│  ● 8f31a2  Fix billing calculation                          │
│  ● 91c20b  Add XML parser                                   │
│  ● a82c11  Merge feature/login                              │
└─────────────────────────────────────────────────────────────┘
```

O layout acima é apenas uma referência conceitual. A implementação pode melhorar a organização visual.

---

# 6. MVP — funcionalidades

## Repository

- Abrir/selecionar repositório local
- Detectar se uma pasta é um repositório Git
- Detectar `.git`
- Mostrar caminho do repositório
- Mostrar branch atual
- Detectar remote(s)
- Mostrar estado do working tree
- Refresh manual
- Atualização automática do status quando apropriado

## Changes

Implementar:

- `git status`
- visualizar arquivos modificados
- visualizar arquivos novos
- visualizar arquivos deletados
- visualizar arquivos renomeados
- Stage individual
- Unstage individual
- Stage all
- Unstage all
- Discard changes
- visualizar diff

Idealmente, clicar em um arquivo abre seu diff de forma visual.

---

# 7. Commit

Implementar:

- Commit
- Commit + Push
- Amend last commit

Interface:

```text
┌───────────────────────────────────────┐
│ Commit message                        │
│                                       │
│ Fix billing calculation               │
│                                       │
└───────────────────────────────────────┘

[ Commit ]       [ Commit & Push ]
```

Não permitir commit sem mensagem, salvo se o comportamento do Git for explicitamente desejado.

---

# 8. Branches

Implementar:

- Listar branches locais
- Mostrar branch atual
- Criar branch
- Switch
- Renomear branch
- Deletar branch

Comandos equivalentes:

```bash
git branch
git switch
git switch -c
git branch -m
git branch -d
```

Branch remota deve ser identificada visualmente quando relevante.

---

# 9. Remote

Implementar:

- Fetch
- Pull
- Push
- Mostrar upstream
- Mostrar ahead/behind

Exemplo:

```text
origin/develop

↑ 0 ahead
↓ 2 behind
```

Botões:

```text
[ Fetch ] [ Pull ] [ Push ]
```

---

# 10. History

Implementar:

- Mostrar histórico de commits
- Hash curto
- Mensagem
- Autor
- Data/hora
- Branch/ref quando relevante
- Selecionar commit
- Visualizar diff do commit
- Copiar hash

Exemplo:

```text
● 8f31a2  Fix billing calculation
  João Pedro                         2m ago

● 91c20b  Add XML parser
  João Pedro                         1h ago

● a82c11  Merge feature/login
  João Pedro                         3h ago
```

---

# 11. Operações avançadas do MVP

Apesar de serem mais avançadas, incluir:

## Merge

```text
[ Merge branch... ]
```

## Rebase

```text
[ Rebase onto... ]
```

## Cherry-pick

```text
[ Cherry-pick commit... ]
```

## Reset

Permitir pelo menos:

- Soft
- Mixed
- Hard

Operações destrutivas devem exigir confirmação clara.

Exemplo:

```text
⚠ Hard reset

This will discard local changes.

[ Cancel ] [ Reset ]
```

---

# 12. Terminal integrado

O terminal deve ser acessível de qualquer lugar da aplicação.

Sugestão:

```text
┌─────────────────────────────────────────────────────────────┐
│ TERMINAL                                                    │
├─────────────────────────────────────────────────────────────┤
│ PS C:\Projects\my-repo> git status                         │
│                                                             │
│ On branch develop                                           │
│ Your branch is up to date with 'origin/develop'.            │
│                                                             │
│ PS C:\Projects\my-repo> _                                 │
└─────────────────────────────────────────────────────────────┘
```

Requisitos:

- sessão persistente
- working directory inicial = repositório aberto
- stdin
- stdout
- stderr
- comandos interativos
- suporte a prompts
- suporte a passphrase SSH
- copiar/colar
- seleção de texto
- limpar terminal
- novo terminal

Idealmente, o terminal deve parecer um terminal moderno, não um `cmd.exe` cru.

---

# 13. Tratamento de erros

A aplicação não deve esconder mensagens reais do Git.

Se o Git retornar:

```text
CONFLICT (content): Merge conflict...
Automatic merge failed
```

a aplicação deve apresentar:

- operação que falhou
- mensagem original do Git
- arquivos afetados quando possível
- opção para abrir/usar o terminal
- opção para atualizar o estado do repositório

Exemplo:

```text
┌──────────────────────────────────────────────┐
│ Rebase failed                                │
│                                              │
│ Git reported conflicts in 3 files.            │
│                                              │
│ ! src/api/users.py                           │
│ ! src/db/models.py                           │
│ ! README.md                                  │
│                                              │
│ [ Resolve conflicts ] [ Open terminal ]       │
└──────────────────────────────────────────────┘
```

Não transformar erros do Git em mensagens genéricas como:

> "Something went wrong."

O usuário precisa conseguir entender o que aconteceu.

---

# 14. Filosofia de UX

A aplicação deve assumir que o usuário conhece Git.

Portanto:

- não explicar conceitos básicos desnecessariamente
- não esconder comandos
- não criar abstrações estranhas
- usar terminologia real do Git
- deixar operações importantes explícitas
- permitir fallback para o terminal

A GUI deve ser uma camada de conveniência.

Não deve tentar substituir o Git CLI.

---

# 15. Segurança

Nunca:

- armazenar senha SSH
- armazenar passphrase
- registrar passphrase em logs
- enviar credenciais para servidores próprios
- implementar autenticação paralela sem necessidade

O terminal deve mascarar/ocultar input sensível quando apropriado.

Logs de aplicação não podem conter dados sensíveis.

---

# 16. Arquitetura sugerida

Separar claramente:

```text
Frontend
    ↓
Tauri Commands
    ↓
Git Service
    ↓
Process Runner
    ↓
git.exe
```

Exemplo conceitual:

```text
src/
├── components/
│   ├── ChangesPanel
│   ├── CommitPanel
│   ├── BranchPanel
│   ├── HistoryPanel
│   ├── Terminal
│   └── ...
│
├── pages/
│   └── Repository
│
├── hooks/
│   ├── useRepository
│   ├── useGitStatus
│   └── ...
│
├── lib/
│   └── ...
│
└── types/
```

Backend:

```text
src-tauri/
├── src/
│   ├── commands/
│   ├── git/
│   ├── process/
│   └── terminal/
└── ...
```

A estrutura exata pode ser adaptada durante a implementação.

---

# 17. Git command strategy

Sempre que possível, preferir comandos com output estruturado/máquina-legível para parsing.

Por exemplo:

```bash
git status --porcelain=v2
```

em vez de depender de texto humano de:

```bash
git status
```

Da mesma forma, usar opções apropriadas de Git para obter informações confiáveis sobre:

- branches
- commits
- status
- remotes
- ahead/behind

Não fazer parsing frágil de mensagens destinadas exclusivamente a humanos quando houver uma saída estruturada disponível.

---

# 18. Primeiro milestone

Não tentar construir tudo de uma vez.

A implementação deve seguir esta ordem:

### Milestone 1 — Shell

- Criar projeto Tauri + React + TypeScript
- Criar janela desktop
- Criar layout base
- Detectar `git.exe`
- Abrir um repositório
- Executar `git status`
- Mostrar branch atual

### Milestone 2 — Changes

- status
- stage
- unstage
- discard
- diff

### Milestone 3 — Commit

- commit
- amend
- commit + push

### Milestone 4 — Branches

- list
- create
- switch
- rename
- delete

### Milestone 5 — Remote

- fetch
- pull
- push
- ahead/behind

### Milestone 6 — History

- log
- commit details
- diff

### Milestone 7 — Advanced Git

- merge
- rebase
- cherry-pick
- reset

### Milestone 8 — Terminal

- terminal integrado completo
- stdin/stdout/stderr
- comandos interativos
- SSH passphrase
- múltiplas sessões, se viável

---

# 19. Critérios de sucesso do MVP

O MVP está pronto quando o usuário consegue abrir um repositório real e fazer, pela interface:

```text
Open repository
      ↓
View changes
      ↓
Stage
      ↓
Commit
      ↓
Create/switch branch
      ↓
Pull
      ↓
Push
      ↓
View history
      ↓
Merge/Rebase
```

E, quando uma operação exigir interação ou fugir do escopo da GUI, o usuário pode abrir o terminal integrado e continuar trabalhando **no mesmo repositório**.

O projeto não precisa implementar todos os recursos do Git.

Ele precisa fazer muito bem as operações mais comuns.

---

# 20. Fora do escopo do MVP

Não implementar agora:

- GitHub/GitLab API
- Pull Requests
- Issues
- CI/CD
- GitHub OAuth
- gerenciamento de contas
- gerenciamento próprio de SSH keys
- Git LFS UI
- submodules UI
- worktrees UI
- reflog UI
- visualização complexa de DAG
- IA
- colaboração
- sincronização cloud
- plugins
- marketplace
- atualização automática complexa

Esses recursos podem ser considerados futuramente.

---

# 21. Direção visual

A aplicação deve ter aparência:

- moderna
- minimalista
- profissional
- rápida
- limpa
- boa tipografia
- bom espaçamento
- ícones consistentes
- dark mode bem feito
- light mode bem feito

Referências de qualidade visual podem incluir aplicações modernas de desenvolvimento, mas **não copiar visualmente nenhum produto específico**.

Evitar deliberadamente:

- estética de TortoiseGit/TortoiseSVN antigo
- excesso de tabelas
- excesso de toolbars
- aparência de Windows Forms
- botões gigantes
- gradients exagerados
- visual "AI slop"

---

# 22. Regra de ouro

Sempre que houver dúvida entre:

**A)** adicionar mais uma abstração visual

ou

**B)** simplesmente executar o comando Git real e mostrar seu resultado de maneira bonita

preferir **B**.

A aplicação deve permanecer pequena, previsível e confiável.

> **Git continua sendo o Git. A aplicação só torna o uso dele melhor.**
