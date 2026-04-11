#!/usr/bin/env zsh

# Configuration variables for forge plugin
# Using typeset to keep variables local to plugin scope and prevent public exposure

typeset -h _FORGE_BIN="${FORGE_BIN:-forge}"
typeset -h _FORGE_CONVERSATION_PATTERN=":"
typeset -h _FORGE_MAX_COMMIT_DIFF="${FORGE_MAX_COMMIT_DIFF:-100000}"
typeset -h _FORGE_DELIMITER='\s\s+'
typeset -h _FORGE_PREVIEW_WINDOW="--preview-window=bottom:75%:wrap:border-sharp"

# Detect fd command - Ubuntu/Debian use 'fdfind', others use 'fd'
typeset -h _FORGE_FD_CMD="$(command -v fdfind 2>/dev/null || command -v fd 2>/dev/null || echo 'fd')"

# Detect bat command - use bat if available, otherwise fall back to cat
if command -v bat &>/dev/null; then
    typeset -h _FORGE_CAT_CMD="bat --color=always --style=numbers,changes --line-range=:500"
else
    typeset -h _FORGE_CAT_CMD="cat"
fi

# Commands cache - loaded lazily on first use
typeset -h _FORGE_COMMANDS=""

# Hidden variables to be used only via the ForgeCLI
typeset -h _FORGE_CONVERSATION_ID
typeset -h _FORGE_ACTIVE_AGENT

# Previous conversation ID for :conversation - (like cd -)
typeset -h _FORGE_PREVIOUS_CONVERSATION_ID

# Session-scoped model and provider overrides (set via :model / :m).
# When non-empty, these are passed as --model / --provider to every forge
# invocation for the lifetime of the current shell session.
typeset -h _FORGE_SESSION_MODEL
typeset -h _FORGE_SESSION_PROVIDER

# Session-scoped reasoning effort override (set via :reasoning-effort / :re).
# When non-empty, exported as FORGE_REASONING__EFFORT for every forge invocation.
typeset -h _FORGE_SESSION_REASONING_EFFORT

# Terminal context capture settings
# Master switch for terminal context capture (preexec/precmd hooks)
typeset -h _FORGE_CTX_ENABLED="${FORGE_CTX_ENABLED:-true}"
# Maximum number of commands to keep in the ring buffer (metadata: cmd + exit code)
typeset -h _FORGE_CTX_MAX_ENTRIES="${FORGE_CTX_MAX_ENTRIES:-10}"
# Number of recent commands to include full output for
typeset -h _FORGE_CTX_FULL_OUTPUT_COUNT="${FORGE_CTX_FULL_OUTPUT_COUNT:-5}"
# Maximum output lines per command block
typeset -h _FORGE_CTX_MAX_LINES_PER_CMD="${FORGE_CTX_MAX_LINES_PER_CMD:-200}"
# Scrollback lines to capture from the terminal for command block extraction
typeset -h _FORGE_CTX_SCROLLBACK_LINES="${FORGE_CTX_SCROLLBACK_LINES:-1000}"
# OSC 133 semantic prompt marker emission: "auto", "on", or "off"
typeset -h _FORGE_CTX_OSC133="${FORGE_CTX_OSC133:-auto}"
# Ring buffer arrays for context capture
typeset -ha _FORGE_CTX_COMMANDS=()
typeset -ha _FORGE_CTX_EXIT_CODES=()
typeset -ha _FORGE_CTX_TIMESTAMPS=()
