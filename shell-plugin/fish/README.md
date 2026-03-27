# ForgeCode Fish Shell Plugin

A fish shell port of the ForgeCode ZSH plugin, providing the same `:` command interface for interacting with Forge directly from your shell prompt.

## Installation

### Option 1: Symlink (recommended for development)

```fish
ln -s (pwd)/forge.fish ~/.config/fish/conf.d/forge.fish
ln -s (pwd)/fish_right_prompt.fish ~/.config/fish/functions/fish_right_prompt.fish
```

### Option 2: Copy

```fish
cp forge.fish ~/.config/fish/conf.d/forge.fish
cp fish_right_prompt.fish ~/.config/fish/functions/fish_right_prompt.fish
```

Then reload fish or open a new terminal.

## Usage

Type `:` followed by a space and your prompt to send it to Forge:

```
: explain the auth flow in this codebase
```

### Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `:new` | `:n` | Start a new conversation |
| `:info` | `:i` | Show session info |
| `:env` | `:e` | Show environment info |
| `:agent` | `:a` | Select/switch agent |
| `:conversation` | `:c` | List/switch conversations |
| `:conversation -` | | Toggle to previous conversation |
| `:provider` | `:p` | Select AI provider |
| `:model` | `:m` | Select model |
| `:commit` | | AI-generated commit |
| `:commit-preview` | | Preview AI commit message |
| `:suggest` | `:s` | Generate shell command from description |
| `:edit` | `:ed` | Open editor for multi-line input |
| `:tools` | `:t` | List available tools |
| `:config` | | Show current config |
| `:clone` | | Clone a conversation |
| `:copy` | | Copy last message to clipboard |
| `:dump` | `:d` | Dump conversation (supports `html`) |
| `:compact` | | Compact conversation context |
| `:retry` | `:r` | Retry last message |
| `:sync` | | Sync workspace for code search |
| `:login` | | Login to provider |
| `:logout` | | Logout from provider |
| `:doctor` | | Run environment diagnostics |
| `:kb` | | Show keyboard shortcuts |
| `:ask` | | Alias for sage agent |
| `:plan` | | Alias for muse agent |

### Tab Completion

- Type `:` then press Tab to fuzzy-search available commands via fzf
- Type `@` then press Tab to fuzzy-search files via fd + fzf

## Dependencies

- [forge](https://forgecode.dev) — the ForgeCode CLI
- [fzf](https://github.com/junegunn/fzf) — fuzzy finder (required for interactive selection)
- [fd](https://github.com/sharkdp/fd) — file finder (for `@` file completion)
- [bat](https://github.com/sharkdp/bat) — syntax highlighting in previews (optional)

## Files

- `forge.fish` — Main plugin (conf.d auto-loaded). Contains all functions, keybindings, and dispatcher.
- `fish_right_prompt.fish` — Right prompt showing active model/agent info.
- `README.md` — This file.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FORGE_BIN` | `forge` | Path to forge binary |
| `FORGE_MAX_COMMIT_DIFF` | `100000` | Max diff size for AI commits |
| `FORGE_EDITOR` | `$EDITOR` or `nano` | Editor for `:edit` command |
| `FORGE_SYNC_ENABLED` | `true` | Enable background workspace sync |

## Differences from ZSH Plugin

- No ZLE widget system — uses fish `bind` and `commandline` builtins
- Right prompt via `fish_right_prompt` function instead of `RPROMPT`
- Fish auto-loads from `conf.d/` and `functions/` — no manual sourcing needed
- Uses fish syntax throughout (`set`, `test`, `string match`, etc.)
