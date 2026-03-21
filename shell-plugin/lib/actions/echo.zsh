#!/usr/bin/env zsh

# Echo action handler

# Action handler: Echo the input text
function _forge_action_echo() {
    local input_text="$1"
    if [[ -n "$input_text" ]]; then
        echo "$input_text"
    fi
}
