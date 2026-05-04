# Git Governance

Rules for how git is used in the Nyx project.

## Branch Strategy

- `main` — stable, always buildable
- `feature/<name>` — all new work happens on feature branches
- `fix/<name>` — bug fixes
- `refactor/<name>` — structural changes without behavior change

## Commit Rules

- Use [Conventional Commits](https://www.conventionalcommits.org/) format
- Single-line commits preferred, no body unless genuinely necessary
- Examples:
  - `feat: add filetree module`
  - `fix: correct cursor position after paste`
  - `refactor: extract rope operations into separate module`
  - `docs: update project plan with phase 2 progress`

## Restrictions

- **Never add co-authors** to commits
- **Never push to remote** — the maintainer handles all pushes
- **Never force-push** to any branch
- **Never commit files listed in `.gitignore`**
- **Never commit secrets, credentials, or environment files**

## Workflow

1. Create a feature branch from `main`
2. Make changes, commit with conventional commit messages
3. When work is done, inform the maintainer — they handle merging and pushing

## Branch Naming

Use lowercase, hyphen-separated names:

- `feature/vim-engine`
- `feature/panel-system`
- `fix/cursor-wrap-bug`
- `refactor/config-loading`
