# ForgeCode Fish Right Prompt Theme
# Displays forge session info (model, agent, conversation stats)
# Converts forge's zsh-formatted rprompt output to fish-compatible ANSI
#
# If a fish_right_prompt already existed before this file was sourced,
# it is saved as _forge_original_fish_right_prompt and appended after
# the forge info.

# Save any existing right prompt before we overwrite it.
# This runs once at source time; subsequent sources are a no-op because
# the function will already be our version (not the original).
if functions -q fish_right_prompt; and not functions -q _forge_original_fish_right_prompt
    # Only save if the current fish_right_prompt is NOT ours
    # (i.e., it doesn't contain "_FORGE_BIN" in its body)
    set -l body (functions fish_right_prompt)
    if not string match -q '*_FORGE_BIN*' -- "$body"
        functions -c fish_right_prompt _forge_original_fish_right_prompt
    end
end

function fish_right_prompt --description "Right prompt with ForgeCode session info"
    # Preserve the last command's exit status so callers/other prompt
    # segments can inspect it.
    set -l last_status $status

    # Resolve forge binary path
    set -l forge_bin
    if test -n "$_FORGE_BIN"
        set forge_bin "$_FORGE_BIN"
    else if test -n "$FORGE_BIN"
        set forge_bin "$FORGE_BIN"
    else
        set forge_bin "$HOME/.local/bin/forge"
    end

    if test -x "$forge_bin"
        set -l forge_info (
            env \
                _FORGE_CONVERSATION_ID="$_FORGE_CONVERSATION_ID" \
                _FORGE_ACTIVE_AGENT="$_FORGE_ACTIVE_AGENT" \
                $forge_bin zsh rprompt 2>/dev/null \
            | string collect
        )

        if test -n "$forge_info"
            # Convert zsh prompt escapes to fish/ANSI sequences.
            set -l out "$forge_info"

            # Strip zsh bold markers (fish uses set_color --bold instead)
            set out (string replace -a '%B' '' -- "$out")
            set out (string replace -a '%b' '' -- "$out")

            # ── Numeric 256-color: %F{NNN} ──────────────────────────
            set out (string replace -a '%F{240}' (set_color 888888) -- "$out")
            set out (string replace -a '%F{245}' (set_color 8a8a8a) -- "$out")
            set out (string replace -a '%F{250}' (set_color bcbcbc) -- "$out")
            set out (string replace -a '%F{255}' (set_color eeeeee) -- "$out")
            set out (string replace -a '%F{196}' (set_color red) -- "$out")
            set out (string replace -a '%F{208}' (set_color ff8700) -- "$out")
            set out (string replace -a '%F{226}' (set_color yellow) -- "$out")
            set out (string replace -a '%F{46}'  (set_color green) -- "$out")
            set out (string replace -a '%F{33}'  (set_color blue) -- "$out")
            set out (string replace -a '%F{51}'  (set_color cyan) -- "$out")
            set out (string replace -a '%F{201}' (set_color magenta) -- "$out")

            # Fallback: any remaining %F{NNN} with numeric code
            set out (string replace -ra '%F\{[0-9]+\}' (set_color 888888) -- "$out")

            # ── Named colors: %F{name} ──────────────────────────────
            set out (string replace -a '%F{red}'     (set_color red) -- "$out")
            set out (string replace -a '%F{green}'   (set_color green) -- "$out")
            set out (string replace -a '%F{blue}'    (set_color blue) -- "$out")
            set out (string replace -a '%F{yellow}'  (set_color yellow) -- "$out")
            set out (string replace -a '%F{cyan}'    (set_color cyan) -- "$out")
            set out (string replace -a '%F{magenta}' (set_color magenta) -- "$out")
            set out (string replace -a '%F{white}'   (set_color white) -- "$out")
            set out (string replace -a '%F{black}'   (set_color black) -- "$out")

            # Fallback: any remaining %F{name}
            set out (string replace -ra '%F\{[a-z_]+\}' (set_color normal) -- "$out")

            # Reset foreground
            set out (string replace -a '%f' (set_color normal) -- "$out")

            printf '%s' "$out"
        end
    end

    # Append the original right prompt if one was saved
    if functions -q _forge_original_fish_right_prompt
        set -l original (_forge_original_fish_right_prompt)
        if test -n "$original"
            printf ' %s' "$original"
        end
    end

    return $last_status
end