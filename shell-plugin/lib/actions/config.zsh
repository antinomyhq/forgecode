#!/usr/bin/env zsh

# Configuration action handlers (agent, provider, model, tools, skill)

# Action handler: Select agent
function _forge_action_agent() {
    local input_text="$1"
    
    echo
    
    # If an agent ID is provided directly, use it
    if [[ -n "$input_text" ]]; then
        local agent_id="$input_text"
        
        # Validate that the agent exists (skip header line)
        local agent_exists=$($_FORGE_BIN list agents --porcelain 2>/dev/null | tail -n +2 | grep -q "^${agent_id}\b" && echo "true" || echo "false")
        if [[ "$agent_exists" == "false" ]]; then
            _forge_log error "Agent '\033[1m${agent_id}\033[0m' not found"
            return 0
        fi
        
        # Set the agent as active
        _FORGE_ACTIVE_AGENT="$agent_id"
        
        # Print log about agent switching
        _forge_log success "Switched to agent \033[1m${agent_id}\033[0m"
        
        return 0
    fi
    
    # Get agents list
    local agents_output
    agents_output=$($_FORGE_BIN list agents --porcelain 2>/dev/null)
    
    if [[ -n "$agents_output" ]]; then
        # Get current agent ID
        local current_agent="$_FORGE_ACTIVE_AGENT"
        
        local sorted_agents="$agents_output"
        
        # Create prompt with current agent - show agent ID, title, provider, model and reasoning
        local prompt_text="Agent ❯ "
        local fzf_args=(
            --prompt="$prompt_text"
            --delimiter="$_FORGE_DELIMITER"
            --with-nth="1,2,4,5,6"
        )

        # If there's a current agent, position cursor on it
        if [[ -n "$current_agent" ]]; then
            local index=$(_forge_find_index "$sorted_agents" "$current_agent")
            fzf_args+=(--bind="start:pos($index)")
        fi

        local selected_agent
        # Use fzf without preview for simple selection like provider/model
        selected_agent=$(echo "$sorted_agents" | _forge_fzf --header-lines=1 "${fzf_args[@]}")
        
        if [[ -n "$selected_agent" ]]; then
            # Extract the first field (agent ID)
            local agent_id=$(echo "$selected_agent" | awk '{print $1}')
            
            # Set the selected agent as active
            _FORGE_ACTIVE_AGENT="$agent_id"
            
            # Print log about agent switching
            _forge_log success "Switched to agent \033[1m${agent_id}\033[0m"
            
        fi
    else
        _forge_log error "No agents found"
    fi
}

# Action handler: Select provider
function _forge_action_provider() {
    local input_text="$1"
    echo
    local selected
    # Only show LLM providers (exclude context_engine and other non-LLM types)
    # Pass input_text as query parameter for fuzzy search
    selected=$(_forge_select_provider "" "" "llm" "$input_text")
    
    if [[ -n "$selected" ]]; then
        # Extract the second field (provider ID) from the selected line
        # Format: "DisplayName  provider_id  host  type  status"
        local provider_id=$(echo "$selected" | awk '{print $2}')
        # Use _forge_exec_interactive because config-set may trigger
        # interactive authentication prompts (rustyline) when the provider
        # is not yet configured. Without /dev/tty redirection, ZLE's pipes
        # cause rustyline to see EOF and fail with "API key input cancelled".
        _forge_exec_interactive config set provider "$provider_id"
    fi
}

# Helper: Open an fzf model picker and print the raw selected line.
#
# Model list columns (from `forge list models --porcelain`):
#   1:model_id  2:model_name  3:provider(display)  4:provider_id(raw)  5:context  6:tools  7:image
# The picker hides model_id (field 1) and provider_id (field 4) via --with-nth.
#
# Arguments:
#   $1  prompt_text      - fzf prompt label (e.g. "Model ❯ ")
#   $2  current_model    - model_id to pre-position the cursor on (may be empty)
#   $3  input_text       - optional pre-fill query for fzf
#   $4  current_provider - provider value to disambiguate when model names collide (may be empty)
#   $5  provider_field   - which porcelain field to match the provider against
#                          (3 for display name, 4 for raw id)
#
# Outputs the raw selected line to stdout, or nothing if cancelled.
function _forge_pick_model() {
    local prompt_text="$1"
    local current_model="$2"
    local input_text="$3"
    local current_provider="${4:-}"
    local provider_field="${5:-}"

    local output
    output=$($_FORGE_BIN list models --porcelain 2>/dev/null)

    if [[ -z "$output" ]]; then
        return 1
    fi

    local fzf_args=(
        --delimiter="$_FORGE_DELIMITER"
        --prompt="$prompt_text"
        --with-nth="2,3,5.."
    )

    if [[ -n "$input_text" ]]; then
        fzf_args+=(--query="$input_text")
    fi

    if [[ -n "$current_model" ]]; then
        # Match on both model_id (field 1) and provider to disambiguate
        # when the same model name exists across multiple providers
        local index
        if [[ -n "$current_provider" && -n "$provider_field" ]]; then
            index=$(_forge_find_index "$output" "$current_model" 1 "$provider_field" "$current_provider")
        else
            index=$(_forge_find_index "$output" "$current_model" 1)
        fi
        fzf_args+=(--bind="start:pos($index)")
    fi

    echo "$output" | _forge_fzf --header-lines=1 "${fzf_args[@]}"
}

# Action handler: Select model (across all configured providers)
# When the selected model belongs to a different provider, switches it first.
function _forge_action_model() {
    local input_text="$1"
    (
        echo
        local current_model current_provider
        current_model=$($_FORGE_BIN config get model 2>/dev/null)
        # config get provider returns the display name (e.g. "OpenAI"),
        # which corresponds to porcelain field 3 (provider display)
        current_provider=$($_FORGE_BIN config get provider 2>/dev/null)
        local selected
        selected=$(_forge_pick_model "Model ❯ " "$current_model" "$input_text" "$current_provider" 3)

        if [[ -n "$selected" ]]; then
            # Field 1 = model_id (raw), field 3 = provider display name,
            # field 4 = provider_id (raw, for config set)
            local model_id provider_display provider_id
            read -r model_id provider_display provider_id <<<$(echo "$selected" | awk -F '  +' '{print $1, $3, $4}')
            model_id=${model_id//[[:space:]]/}
            provider_id=${provider_id//[[:space:]]/}
            provider_display=${provider_display//[[:space:]]/}

            # Switch provider first if it differs from the current one
            # current_provider (fetched above) is the display name, compare against that
            if [[ -n "$provider_display" && "$provider_display" != "$current_provider" ]]; then
                _forge_exec_interactive config set provider "$provider_id" --model "$model_id"
                return
            fi

            _forge_exec config set model "$model_id"
        fi
    )
}

# Action handler: Select model for commit message generation
# Calls `forge config set commit <provider_id> <model_id>` on selection.
function _forge_action_commit_model() {
    local input_text="$1"
    (
        echo
        # config get commit outputs two lines: provider_id (raw) then model_id
        local commit_output current_commit_model current_commit_provider
        commit_output=$(_forge_exec config get commit 2>/dev/null)
        current_commit_provider=$(echo "$commit_output" | head -n 1)
        current_commit_model=$(echo "$commit_output" | tail -n 1)

        local selected
        # provider_id from config get commit is the raw id, matching porcelain field 4
        selected=$(_forge_pick_model "Commit Model ❯ " "$current_commit_model" "$input_text" "$current_commit_provider" 4)

        if [[ -n "$selected" ]]; then
            # Field 1 = model_id (raw), field 4 = provider_id (raw)
            local model_id provider_id
            read -r model_id provider_id <<<$(echo "$selected" | awk -F '  +' '{print $1, $4}')

            model_id=${model_id//[[:space:]]/}
            provider_id=${provider_id//[[:space:]]/}

            _forge_exec config set commit "$provider_id" "$model_id"
        fi
    )
}

# Action handler: Select model for command suggestion generation
# Calls `forge config set suggest <provider_id> <model_id>` on selection.
function _forge_action_suggest_model() {
    local input_text="$1"
    (
        echo
        # config get suggest outputs two lines: provider_id (raw) then model_id
        local suggest_output current_suggest_model current_suggest_provider
        suggest_output=$(_forge_exec config get suggest 2>/dev/null)
        current_suggest_provider=$(echo "$suggest_output" | head -n 1)
        current_suggest_model=$(echo "$suggest_output" | tail -n 1)

        local selected
        # provider_id from config get suggest is the raw id, matching porcelain field 4
        selected=$(_forge_pick_model "Suggest Model ❯ " "$current_suggest_model" "$input_text" "$current_suggest_provider" 4)

        if [[ -n "$selected" ]]; then
            # Field 1 = model_id (raw), field 4 = provider_id (raw)
            local model_id provider_id
            read -r model_id provider_id <<<$(echo "$selected" | awk -F '  +' '{print $1, $4}')

            model_id=${model_id//[[:space:]]/}
            provider_id=${provider_id//[[:space:]]/}

            _forge_exec config set suggest "$provider_id" "$model_id"
        fi
    )
}

# Action handler: Sync workspace for codebase search
function _forge_action_sync() {
    echo
    # Execute sync with stdin redirected to prevent hanging
    # Sync doesn't need interactive input, so close stdin immediately
    _forge_exec workspace sync </dev/null
}

# Action handler: inits workspace for codebase search
function _forge_action_sync_init() {
    echo
    _forge_exec workspace init </dev/null
}

# Action handler: Show sync status of workspace files
function _forge_action_sync_status() {
    echo
    _forge_exec workspace status "."
}

# Action handler: Show workspace info with sync details
function _forge_action_sync_info() {
    echo
    _forge_exec workspace info "."
}

# Helper function to select and set config values with fzf
function _forge_select_and_set_config() {
    local show_command="$1"
    local config_flag="$2"
    local prompt_text="$3"
    local default_value="$4"
    local with_nth="${5:-}"  # Optional column selection parameter
    local query="${6:-}"     # Optional query parameter for fuzzy search
    (
        echo
        local output
        # Handle multi-word commands properly
        if [[ "$show_command" == *" "* ]]; then
            # Split the command into words and execute with --porcelain
            local cmd_parts=(${=show_command})
            output=$($_FORGE_BIN "${cmd_parts[@]}" --porcelain 2>/dev/null)
        else
            output=$($_FORGE_BIN "$show_command" --porcelain 2>/dev/null)
        fi
        
        if [[ -n "$output" ]]; then
            local selected
            local fzf_args=(--delimiter="$_FORGE_DELIMITER" --prompt="$prompt_text ❯ ")

            if [[ -n "$with_nth" ]]; then
                fzf_args+=(--with-nth="$with_nth")
            fi

            # Add query parameter if provided
            if [[ -n "$query" ]]; then
                fzf_args+=(--query="$query")
            fi

            if [[ -n "$default_value" ]]; then
                # For models, compare against the first field (model_id)
                local index=$(_forge_find_index "$output" "$default_value" 1)
                
                fzf_args+=(--bind="start:pos($index)")
                
            fi
            selected=$(echo "$output" | _forge_fzf --header-lines=1 "${fzf_args[@]}")

            if [[ -n "$selected" ]]; then
                local name="${selected%% *}"
                _forge_exec config set "$config_flag" "$name"
            fi
        fi
    )
}

# Action handler: Select model for the current session only.
# Sets _FORGE_SESSION_MODEL and _FORGE_SESSION_PROVIDER in the shell environment
# so that every subsequent forge invocation uses those values via --model /
# --provider flags without touching the permanent global configuration.
function _forge_action_session_model() {
    local input_text="$1"
    echo

    local current_model current_provider provider_index
    # Use session overrides as the starting selection if already set,
    # otherwise fall back to the globally configured values.
    if [[ -n "$_FORGE_SESSION_MODEL" ]]; then
        current_model="$_FORGE_SESSION_MODEL"
        provider_index=4
    else
        current_model=$($_FORGE_BIN config get model 2>/dev/null)
        provider_index=3
    fi
    if [[ -n "$_FORGE_SESSION_PROVIDER" ]]; then
        current_provider="$_FORGE_SESSION_PROVIDER"
        provider_index=4
    else
        current_provider=$($_FORGE_BIN config get provider 2>/dev/null)
        provider_index=3
    fi

    local selected
    selected=$(_forge_pick_model "Session Model ❯ " "$current_model" "$input_text" "$current_provider" "$provider_index")

    if [[ -n "$selected" ]]; then
        local model_id provider_display provider_id
        read -r model_id provider_display provider_id <<<$(echo "$selected" | awk -F '  +' '{print $1, $3, $4}')
        model_id=${model_id//[[:space:]]/}
        provider_id=${provider_id//[[:space:]]/}

        _FORGE_SESSION_MODEL="$model_id"
        _FORGE_SESSION_PROVIDER="$provider_id"

        _forge_log success "Session model set to \033[1m${model_id}\033[0m (provider: \033[1m${provider_id}\033[0m)"
    fi
}

# Action handler: Reset session model and provider to defaults.
# Clears both _FORGE_SESSION_MODEL and _FORGE_SESSION_PROVIDER,
# reverting to global config for subsequent forge invocations.
function _forge_action_model_reset() {
    echo

    if [[ -z "$_FORGE_SESSION_MODEL" && -z "$_FORGE_SESSION_PROVIDER" ]]; then
        _forge_log info "Session model already cleared (using global config)"
        return 0
    fi

    _FORGE_SESSION_MODEL=""
    _FORGE_SESSION_PROVIDER=""

    _forge_log success "Session model reset to global config"
}

# Action handler: Show config list
function _forge_action_config() {
    echo
    $_FORGE_BIN config list
}

# Action handler: Show tools
function _forge_action_tools() {
    echo
    # Ensure FORGE_ACTIVE_AGENT always has a value, default to "forge"
    local agent_id="${_FORGE_ACTIVE_AGENT:-forge}"
    _forge_exec list tools "$agent_id"
}

# Action handler: Show skills
function _forge_action_skill() {
    echo
    _forge_exec list skill
}
