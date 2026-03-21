# ForgeCode PowerShell Plugin

A PowerShell Core 7+ port of the ForgeCode shell plugin, providing intelligent command transformation, AI conversation management, and interactive completion for the Forge AI assistant.

## Features

- **Smart Command Transformation**: Type `:command [args]` and press Enter to dispatch to forge
- **AI Conversation Management**: Persistent sessions with automatic context tracking
- **Interactive Completion**: fzf-based fuzzy completion for `:commands` and `@[file]` references
- **Agent Selection**: Switch between forge agents on the fly
- **Provider / Model Picker**: Interactive fzf selectors for providers and models
- **Right Prompt**: Agent and model info displayed on the right side of the prompt
- **Cross-Platform**: Works on Windows, macOS, and Linux with PowerShell Core 7+

## Prerequisites

| Tool | Required | Description |
|------|----------|-------------|
| [PowerShell Core 7+](https://github.com/PowerShell/PowerShell) | Yes | PowerShell 7.0 or later |
| [PSReadLine](https://github.com/PowerShell/PSReadLine) | Yes | Ships with PowerShell 7+ |
| [forge](https://github.com/antinomyhq/forge) | Yes | The Forge CLI tool |
| [fzf](https://github.com/junegunn/fzf) | Yes | Command-line fuzzy finder |
| [fd](https://github.com/sharkdp/fd) | Recommended | Fast file finder (for `@` completion) |
| [bat](https://github.com/sharkdp/bat) | Optional | Syntax-highlighted file previews |

### Installing Prerequisites

```bash
# macOS (Homebrew)
brew install --cask powershell
brew install fzf fd bat

# Ubuntu / Debian
sudo apt install fzf fd-find bat
# Install PowerShell: https://learn.microsoft.com/en-us/powershell/scripting/install/install-ubuntu

# Windows (winget)
winget install Microsoft.PowerShell
winget install junegunn.fzf
winget install sharkdp.fd
winget install sharkdp.bat

# Arch Linux
sudo pacman -S powershell-bin fzf fd bat
```

## Installation

### Option 1: Copy to PowerShell Modules Directory

```powershell
# Find your modules directory
$modulesPath = ($env:PSModulePath -split [IO.Path]::PathSeparator)[0]

# Create module directory and copy files
$dest = Join-Path $modulesPath 'ForgeCode'
New-Item -ItemType Directory -Path $dest -Force
Copy-Item ForgeCode.psm1, ForgeCode.psd1 -Destination $dest
```

### Option 2: Direct Import

```powershell
Import-Module /path/to/ForgeCode.psd1
```

### Activating the Plugin

Add these lines to your PowerShell profile (`$PROFILE`):

```powershell
Import-Module ForgeCode
Enable-ForgePlugin
```

To edit your profile:

```powershell
# Create profile if it doesn't exist
if (-not (Test-Path $PROFILE)) { New-Item -Path $PROFILE -Force }

# Open in your editor
code $PROFILE    # VS Code
notepad $PROFILE # Notepad (Windows)
nano $PROFILE    # nano (macOS/Linux)
```

### Disabling the Plugin

```powershell
Disable-ForgePlugin
```

This restores the default Enter/Tab key handlers and the original prompt.

## Usage

### Starting a Conversation

Type `:` followed by a space and your prompt, then press Enter:

```
: What's the weather like?
: Explain the MVC pattern
```

### Using Specific Agents

Specify an agent by name after the colon:

```
:sage How does caching work in this system?
:muse Create a deployment strategy for my app
```

### File Tagging

Reference files in your prompts with `@[filename]` syntax. Press Tab after `@` for interactive file selection:

```
: Review this code @[src/main.rs]
: Explain the config in @[config.yaml]
```

### Conversation Continuity

Commands within the same session maintain context:

```
: My project uses React and TypeScript
: How can I optimize the build process?
```

## Command Reference

| Command | Alias | Description |
|---------|-------|-------------|
| `:new` | `:n` | Start a new conversation |
| `:info` | `:i` | Show session info |
| `:env` | `:e` | Show environment info |
| `:agent` | `:a` | Select or switch agent (fzf picker) |
| `:conversation` | `:c` | List/switch conversations (fzf picker) |
| `:conversation -` | `:c -` | Toggle between current and previous conversation |
| `:provider` | `:p` | Select LLM provider (fzf picker) |
| `:model` | `:m` | Select model (fzf picker) |
| `:config-commit-model` | `:ccm` | Select commit message model |
| `:config-suggest-model` | `:csm` | Select command suggest model |
| `:commit` | | AI-generated commit |
| `:commit-preview` | | Preview AI commit message, place git command in buffer |
| `:suggest` | `:s` | Generate shell command from description |
| `:edit` | `:ed` | Open external editor for multi-line input |
| `:tools` | `:t` | Show available tools |
| `:config` | | Show configuration |
| `:skill` | | Show available skills |
| `:clone` | | Clone a conversation |
| `:copy` | | Copy last assistant message to clipboard |
| `:dump` | `:d` | Dump conversation (supports `html` arg) |
| `:compact` | | Compact conversation |
| `:retry` | `:r` | Retry last message |
| `:sync` | | Sync workspace for semantic search |
| `:sync-status` | | Show workspace sync status |
| `:sync-info` | | Show workspace info |
| `:login` | | Login to a provider |
| `:logout` | | Logout from a provider |
| `:doctor` | | Run environment diagnostics |
| `:keyboard-shortcuts` | `:kb` | Show keyboard shortcuts |
| `:ask` | | Alias for `:sage` agent |
| `:plan` | | Alias for `:muse` agent |

### Session Management

#### Toggle Between Conversations

Works like `cd -` in your shell:

```
:c -
```

Swaps between the current and previous conversation. If you use `:new` to start a fresh conversation, `:c -` takes you back.

#### Clone a Conversation

```
:clone            # Interactive picker
:clone <id>       # Clone specific conversation
```

Creates a copy of a conversation and switches to it.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FORGE_BIN` | `~/.local/bin/forge` | Path to the forge binary |
| `FORGE_EDITOR` | `$EDITOR` or `nano` | Editor for `:edit` command |
| `FORGE_SYNC_ENABLED` | `true` | Enable/disable automatic workspace sync |
| `FORGE_MAX_COMMIT_DIFF` | `100000` | Maximum diff size (bytes) for AI commits |

## Key Bindings

| Key | Behavior |
|-----|----------|
| **Enter** | If the line starts with `:`, dispatches to forge. Otherwise, normal execution. |
| **Tab** | If on a `:command` or `@file`, opens fzf picker. Otherwise, normal tab completion. |
| **Ctrl+C** | Interrupt running forge commands. |

## Exported Functions

| Function | Description |
|----------|-------------|
| `Enable-ForgePlugin` | Activate the plugin (registers key handlers and prompt) |
| `Disable-ForgePlugin` | Deactivate the plugin (restores defaults) |
| `Invoke-ForgeDispatch` | Main command dispatcher (called by Enter handler) |
| `Invoke-ForgePrompt` | Handle bare `: prompt text` input |
| `Invoke-ForgeTabComplete` | Handle fzf-based tab completion |
| `Get-ForgePromptInfo` | Get forge agent/model info for the prompt |

## Differences from ZSH / Fish Plugins

| Aspect | ZSH / Fish | PowerShell |
|--------|-----------|------------|
| **Key interception** | ZLE widgets / fish `bind` | PSReadLine `Set-PSReadLineKeyHandler` |
| **Background jobs** | `&!` / `disown` | `Start-Job` |
| **Clipboard** | `pbcopy` / `xclip` / `xsel` | `Set-Clipboard` (cross-platform, with fallback) |
| **Syntax highlighting** | `zsh-syntax-highlighting` / fish built-in | Not available (PSReadLine has limited support) |
| **Right prompt** | `RPROMPT` / fish `fish_right_prompt` | ANSI cursor positioning in `prompt` function |
| **Module structure** | Single file (fish) or multi-file (zsh) | Single `.psm1` + manifest `.psd1` |
| **Activation** | Auto-loaded on shell start | Explicit `Enable-ForgePlugin` call |
| **Variable scope** | Global / typeset | `$script:` scoped module variables |
| **Interactive I/O** | `</dev/tty >/dev/tty` | Shell-out to `/bin/sh` on Unix, direct on Windows |

## Troubleshooting

### "forge binary not found"

Set the `FORGE_BIN` environment variable to point to your forge installation:

```powershell
$env:FORGE_BIN = '/path/to/forge'
```

Or ensure `forge` is in your `$env:PATH`.

### "fzf not found"

Install fzf and ensure it is in your PATH. Interactive selection features (agent picker, model picker, file completion) require fzf.

### Key handlers not working

Ensure PSReadLine is loaded:

```powershell
Get-Module PSReadLine
```

If not loaded:

```powershell
Import-Module PSReadLine
```

### Editor issues with `:edit`

Set your preferred editor:

```powershell
$env:FORGE_EDITOR = 'code --wait'  # VS Code
$env:FORGE_EDITOR = 'vim'          # Vim
$env:FORGE_EDITOR = 'nano'         # nano
```

### Tab completion conflicts

The plugin only intercepts Tab when the line matches a forge pattern (`:command` or `@file`). All other tab completions pass through to the default PowerShell handler.

## Architecture

```
ForgeCode.psm1   # All functions, handlers, and logic
ForgeCode.psd1   # Module manifest (metadata, exports)
README.md         # This file
```

The module uses `$script:` scoped variables for state management, ensuring isolation from the global scope. PSReadLine key handlers use the `RevertLine` + `Insert` + `AcceptLine` pattern to break out of the handler context and call named functions that can interact with fzf and forge.

## License

Same license as the ForgeCode project.
