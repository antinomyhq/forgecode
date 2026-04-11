#!/usr/bin/env zsh

# Configuration variables for forge plugin
# Using typeset -gh (global + hidden) so variables survive lazy-loading
# from within a function scope (e.g. zinit, zplug, zsh-defer) while
# staying hidden from `typeset` listings.

typeset -gh _FORGE_BIN="${FORGE_BIN:-forge}"
typeset -gh _FORGE_CONVERSATION_PATTERN=":"
typeset -gh _FORGE_MAX_COMMIT_DIFF="${FORGE_MAX_COMMIT_DIFF:-100000}"
typeset -gh _FORGE_DELIMITER='\s\s+'
typeset -gh _FORGE_PREVIEW_WINDOW="--preview-window=bottom:75%:wrap:border-sharp"

# Detect fd command - Ubuntu/Debian use 'fdfind', others use 'fd'
typeset -gh _FORGE_FD_CMD="$(command -v fdfind 2>/dev/null || command -v fd 2>/dev/null || echo 'fd')"

# Detect bat command - use bat if available, otherwise fall back to cat
if command -v bat &>/dev/null; then
    typeset -gh _FORGE_CAT_CMD="bat --color=always --style=numbers,changes --line-range=:500"
else
    typeset -gh _FORGE_CAT_CMD="cat"
fi

# Commands cache - loaded lazily on first use
typeset -gh _FORGE_COMMANDS=""

# Hidden variables to be used only via the ForgeCLI
typeset -gh _FORGE_CONVERSATION_ID
typeset -gh _FORGE_ACTIVE_AGENT

# Previous conversation ID for :conversation - (like cd -)
typeset -gh _FORGE_PREVIOUS_CONVERSATION_ID

# Session-scoped model and provider overrides (set via :model / :m).
# When non-empty, these are passed as --model / --provider to every forge
# invocation for the lifetime of the current shell session.
typeset -gh _FORGE_SESSION_MODEL
typeset -gh _FORGE_SESSION_PROVIDER

# Session-scoped reasoning effort override (set via :reasoning-effort / :re).
# When non-empty, exported as FORGE_REASONING__EFFORT for every forge invocation.
typeset -gh _FORGE_SESSION_REASONING_EFFORT
