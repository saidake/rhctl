# Contributing

Welcome! This short guide explains how to contribute effectively to **rhctl**.

## Submitting an issue

- If you find a bug, please submit an issue to our GitHub [repository](https://github.com/saidake/rhctl/issues).
- Before submitting, search the issue tracker to see if your problem already exists. Existing issues may already have workarounds or ongoing fixes.
- Include the `rhctl` version, OS, and enough detail to reproduce the problem (commands, config snippets, and logs help a lot).

## Branch Naming Convention

Use lowercase, kebab-case, and a type prefix:

- `feature/<short-title>`
- `bugfix/<short-title>`
- `docs/<short-title>`

**Example**: `bugfix/fix-ssh-cert-auth`

For release preparation branches:

- `release/<version>`

**Example**: `release/1.0.1`

## Commits

- Keep commits small and focused.
- This makes it easier for reviewers to understand and track changes.
- Prefer this repository's commit style:

```text
[Component] type: Short summary
```

**Examples**:

- `[SSH] feat: Support identity and certificate authentication`
- `[Upload] fix: Failed to move items when using sudo`
- `[README] doc: Add SSH auth setup and fix path-mapping examples`

Common types: `feat`, `fix`, `docs`/`doc`, `style`, `refactor`, `chore`, `ci`.

See [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) for additional inspiration.

## Development Setup

The Rust crate lives under `main/`.

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- A Unix-like shell for exercising remote scripts (Linux, macOS, or WSL on Windows)

### Build

```bash
cd main && cargo build --release && cd ..
# Temporarily add `rhctl` to your PATH for the current terminal session.
export PATH="$(pwd)/target/release:$PATH"
```

On Windows (PowerShell):

```powershell
cd main; cargo build --release; cd ..
$env:PATH = "$(Get-Location)\target\release;$env:PATH"
```

### Project layout

| Path | Purpose |
| - | - |
| `main/` | Rust CLI crate (`Cargo.toml`, `src/`) |
| `assets/` | Example scripts and files used in docs |
| `config/` | Example path-mapping and related config |
| `scripts/` | Environment setup helpers (Docker, Redis, MongoDB, …) |
| `docs/` | Documentation assets (logo, command diagrams) |
| `.github/workflows/` | CI / release workflows |

### Checks before opening a PR

```bash
cd main
cargo build --release
cargo test
```

Manually smoke-test the commands you touched (`execute`, `upload`, `patch`, `run`) against a disposable SSH host when possible.

## Pull Requests

Use the following procedure to submit a pull request:

1. Fork rhctl on GitHub (_[How to fork a repo?](https://docs.github.com/en/github/getting-started-with-github/fork-a-repo)_)

2. Create a branch from `main` (see [Branch Naming](#branch-naming-convention))

```bash
git checkout -b bugfix/<short-title>
```

3. Make the changes and push to your branch (see [Commits](#commits))

```bash
git push origin bugfix/<short-title>
```

4. Initiate a pull request on GitHub (_[How to create a PR?](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-a-pull-request)_)

Try to provide as much description behind the context of your changes and how to verify them. Screenshots and videos are always welcome ^_^

5. Ensure the project builds cleanly (`cargo build --release` in `main/`) and that any relevant smoke tests pass.

Done :)

By following these conventions, you help us keep rhctl stable, reliable, and easy to maintain. Thank you for contributing!
