#!/usr/bin/env zsh

# Terminal context capture for forge plugin
#
# Provides three layers of terminal context:
# 1. preexec/precmd hooks: ring buffer of recent commands + exit codes
# 2. OSC 133 emission: semantic terminal markers for compatible terminals
# 3. Terminal-specific output capture: Kitty > WezTerm > tmux
#
# Context is organized by command blocks: each command's metadata and its
# full output are grouped together, using the known command strings from
# the ring buffer to detect boundaries in the terminal scrollback.

# ---------------------------------------------------------------------------
# OSC 133 helpers
# ---------------------------------------------------------------------------

# Determines whether OSC 133 semantic markers should be emitted.
# Auto-detection is conservative: only emit for terminals known to support it
# to avoid garbled output in unsupported terminals.
function _forge_osc133_should_emit() {
    case "$_FORGE_CTX_OSC133" in
        on)  return 0 ;;
        off) return 1 ;;
        auto)
            # Kitty sets KITTY_PID
            [[ -n "${KITTY_PID:-}" ]] && return 0
            # Detect by TERM_PROGRAM
            case "${TERM_PROGRAM:-}" in
                WezTerm|iTerm.app|vscode) return 0 ;;
            esac
            # Foot terminal
            [[ "${TERM:-}" == "foot"* ]] && return 0
            # Ghostty
            [[ "${TERM_PROGRAM:-}" == "ghostty" ]] && return 0
            # Unknown terminal: don't emit
            return 1
            ;;
        *)   return 1 ;;
    esac
}

# Emits an OSC 133 marker if the terminal supports it.
# Usage: _forge_osc133_emit "A"  or  _forge_osc133_emit "D;0"
function _forge_osc133_emit() {
    _forge_osc133_should_emit || return 0
    printf '\e]133;%s\a' "$1"
}

# ---------------------------------------------------------------------------
# preexec / precmd hooks
# ---------------------------------------------------------------------------

# Ring buffer storage uses parallel arrays declared in config.zsh:
#   _FORGE_CTX_COMMANDS, _FORGE_CTX_EXIT_CODES, _FORGE_CTX_TIMESTAMPS
# Pending command state:
typeset -g _FORGE_CTX_PENDING_CMD=""
typeset -g _FORGE_CTX_PENDING_TS=""

# Called before each command executes.
# Records the command text and timestamp, emits OSC 133 B+C markers.
function _forge_context_preexec() {
    [[ "$_FORGE_CTX_ENABLED" != "true" ]] && return
    _FORGE_CTX_PENDING_CMD="$1"
    _FORGE_CTX_PENDING_TS="$(date +%s)"
    # OSC 133 B: prompt end / command start
    _forge_osc133_emit "B"
    # OSC 133 C: command output start
    _forge_osc133_emit "C"
}

# Called after each command completes, before the next prompt is drawn.
# Captures exit code, pushes to ring buffer, emits OSC 133 D+A markers.
function _forge_context_precmd() {
    local last_exit=$?  # MUST be first line to capture exit code
    [[ "$_FORGE_CTX_ENABLED" != "true" ]] && return

    # OSC 133 D: command finished with exit code
    _forge_osc133_emit "D;$last_exit"

    # Only record if we have a pending command from preexec
    if [[ -n "$_FORGE_CTX_PENDING_CMD" ]]; then
        _FORGE_CTX_COMMANDS+=("$_FORGE_CTX_PENDING_CMD")
        _FORGE_CTX_EXIT_CODES+=("$last_exit")
        _FORGE_CTX_TIMESTAMPS+=("$_FORGE_CTX_PENDING_TS")

        # Trim ring buffer to max size
        while (( ${#_FORGE_CTX_COMMANDS} > _FORGE_CTX_MAX_ENTRIES )); do
            shift _FORGE_CTX_COMMANDS
            shift _FORGE_CTX_EXIT_CODES
            shift _FORGE_CTX_TIMESTAMPS
        done

        _FORGE_CTX_PENDING_CMD=""
        _FORGE_CTX_PENDING_TS=""
    fi

    # OSC 133 A: prompt start (for the next prompt)
    _forge_osc133_emit "A"
}

# ---------------------------------------------------------------------------
# Terminal scrollback capture
# ---------------------------------------------------------------------------

# Captures raw scrollback text from the terminal. The amount captured is
# controlled by _FORGE_CTX_SCROLLBACK_LINES.
# Returns the scrollback on stdout, or returns 1 if unavailable.
# Priority: Kitty > WezTerm > Zellij > tmux > none
function _forge_capture_scrollback() {
    local lines="${_FORGE_CTX_SCROLLBACK_LINES:-1000}"
    local output=""

    # Priority 1: Kitty — get full scrollback (OSC 133 aware)
    if [[ -n "${KITTY_PID:-}" ]] && command -v kitty &>/dev/null; then
        output=$(kitty @ get-text --extent=all 2>/dev/null)
        if [[ -n "$output" ]]; then
            echo "$output" | tail -"$lines"
            return 0
        fi
    fi

    # Priority 2: WezTerm
    if [[ "${TERM_PROGRAM:-}" == "WezTerm" ]] && command -v wezterm &>/dev/null; then
        output=$(wezterm cli get-text 2>/dev/null)
        if [[ -n "$output" ]]; then
            echo "$output" | tail -"$lines"
            return 0
        fi
    fi

    # Priority 3: Zellij — full scrollback dump
    if [[ -n "${ZELLIJ:-}" ]] && command -v zellij &>/dev/null; then
        output=$(zellij action dump-screen --full 2>/dev/null)
        if [[ -n "$output" ]]; then
            echo "$output" | tail -"$lines"
            return 0
        fi
    fi

    # Priority 4: tmux scrollback
    if [[ -n "${TMUX:-}" ]] && command -v tmux &>/dev/null; then
        output=$(tmux capture-pane -p -S -"$lines" 2>/dev/null)
        if [[ -n "$output" ]]; then
            echo "$output"
            return 0
        fi
    fi

    # No terminal-specific capture available
    return 1
}

# ---------------------------------------------------------------------------
# Command block extraction
# ---------------------------------------------------------------------------

# Given raw scrollback text, extracts the output block for a specific command
# by finding the command string and capturing everything until the next known
# command (or end of text). Uses fixed-string grep for reliability.
#
# Args: $1=scrollback, $2=command string, $3=next command string (or empty)
# Outputs the extracted block on stdout, truncated to max lines per command.
function _forge_extract_block() {
    local scrollback="$1"
    local cmd="$2"
    local next_cmd="$3"
    local max_lines="${_FORGE_CTX_MAX_LINES_PER_CMD:-200}"

    # Find the LAST occurrence of this command in scrollback (most recent run)
    local cmd_line
    cmd_line=$(echo "$scrollback" | grep -n -F -- "$cmd" | tail -1 | cut -d: -f1)
    [[ -z "$cmd_line" ]] && return 1

    # Start from the line AFTER the command itself (that's the output)
    local output_start=$(( cmd_line + 1 ))

    if [[ -n "$next_cmd" ]]; then
        # Find where the next command appears after our command
        local next_line
        next_line=$(echo "$scrollback" | tail -n +"$output_start" | grep -n -F -- "$next_cmd" | head -1 | cut -d: -f1)
        if [[ -n "$next_line" ]]; then
            # next_line is relative to output_start, adjust to absolute
            # Subtract 2: one for the prompt line before the command, one for 1-indexing
            local output_end=$(( output_start + next_line - 2 ))
            if (( output_end >= output_start )); then
                echo "$scrollback" | sed -n "${output_start},${output_end}p" | head -"$max_lines"
                return 0
            fi
        fi
    fi

    # No next command found — take everything from output_start to end
    echo "$scrollback" | tail -n +"$output_start" | head -"$max_lines"
    return 0
}

# ---------------------------------------------------------------------------
# Context builder
# ---------------------------------------------------------------------------

# Builds a shell context file containing:
# 1. Metadata for all commands in ring buffer (last N commands + exit codes)
# 2. Full output blocks for the most recent M commands (extracted from scrollback)
#
# Writes to a temp file and echoes the path on stdout.
# Returns non-zero if context is disabled or empty.
function _forge_build_shell_context() {
    [[ "$_FORGE_CTX_ENABLED" != "true" ]] && return 1
    [[ ${#_FORGE_CTX_COMMANDS} -eq 0 ]] && return 1

    local ctx_file
    ctx_file=$(mktemp "${TMPDIR:-/tmp}/forge-ctx-XXXXXX") || return 1

    local count=${#_FORGE_CTX_COMMANDS}
    local full_output_count="${_FORGE_CTX_FULL_OUTPUT_COUNT:-5}"

    # Determine which commands get full output (the most recent N)
    local full_output_start=$(( count - full_output_count + 1 ))
    (( full_output_start < 1 )) && full_output_start=1

    # Capture scrollback once (expensive operation, do it only once)
    local scrollback=""
    scrollback=$(_forge_capture_scrollback 2>/dev/null)

    {
        echo "# Terminal Context"
        echo ""
        echo "The following is the user's recent terminal activity. Commands are listed"
        echo "from oldest to newest. The last ${full_output_count} commands include their full output"
        echo "when terminal capture is available."
        echo ""

        # --- Section 1: Metadata-only commands (older ones) ---
        if (( full_output_start > 1 )); then
            echo "## Earlier Commands"
            echo ""
            for (( i=1; i < full_output_start; i++ )); do
                local ts_human
                ts_human=$(date -d "@${_FORGE_CTX_TIMESTAMPS[$i]}" '+%H:%M:%S' 2>/dev/null \
                    || date -r "${_FORGE_CTX_TIMESTAMPS[$i]}" '+%H:%M:%S' 2>/dev/null \
                    || echo "${_FORGE_CTX_TIMESTAMPS[$i]}")
                local exit_marker=""
                if [[ "${_FORGE_CTX_EXIT_CODES[$i]}" != "0" ]]; then
                    exit_marker=" [EXIT CODE: ${_FORGE_CTX_EXIT_CODES[$i]}]"
                fi
                echo "- \`${_FORGE_CTX_COMMANDS[$i]}\` at ${ts_human}${exit_marker}"
            done
            echo ""
        fi

        # --- Section 2: Full output command blocks (recent ones) ---
        echo "## Recent Commands (with output)"
        echo ""

        for (( i=full_output_start; i <= count; i++ )); do
            local cmd="${_FORGE_CTX_COMMANDS[$i]}"
            local exit_code="${_FORGE_CTX_EXIT_CODES[$i]}"
            local ts_human
            ts_human=$(date -d "@${_FORGE_CTX_TIMESTAMPS[$i]}" '+%H:%M:%S' 2>/dev/null \
                || date -r "${_FORGE_CTX_TIMESTAMPS[$i]}" '+%H:%M:%S' 2>/dev/null \
                || echo "${_FORGE_CTX_TIMESTAMPS[$i]}")

            local status_label="ok"
            [[ "$exit_code" != "0" ]] && status_label="FAILED (exit ${exit_code})"

            echo "### \`${cmd}\` — ${status_label} at ${ts_human}"
            echo ""

            # Try to extract this command's output from scrollback
            if [[ -n "$scrollback" ]]; then
                # Determine the next command string for boundary detection
                local next_cmd=""
                if (( i < count )); then
                    next_cmd="${_FORGE_CTX_COMMANDS[$((i+1))]}"
                fi

                local block
                block=$(_forge_extract_block "$scrollback" "$cmd" "$next_cmd")
                if [[ -n "$block" ]]; then
                    echo '```'
                    echo "$block"
                    echo '```'
                else
                    echo "_No output captured._"
                fi
            else
                echo "_Terminal output capture not available._"
            fi
            echo ""
        done
    } > "$ctx_file"

    echo "$ctx_file"
    return 0
}

# ---------------------------------------------------------------------------
# Hook registration
# ---------------------------------------------------------------------------

# Register using standard zsh hook arrays for coexistence with other plugins.
# precmd is prepended so it runs first and captures the real $? from the
# command, before other plugins (powerlevel10k, starship, etc.) overwrite it.
if [[ "$_FORGE_CTX_ENABLED" == "true" ]]; then
    preexec_functions+=(_forge_context_preexec)
    precmd_functions=(_forge_context_precmd "${precmd_functions[@]}")
fi
