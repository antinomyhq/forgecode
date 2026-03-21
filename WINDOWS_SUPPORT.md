# Windows Platform Support Implementation

This document describes the Windows-specific functionality added to code-forge to achieve feature parity with the codex codebase.

## Overview

Four critical features were implemented to provide full Windows support:

1. **Text Encoding Detection** - Automatic handling of Windows code pages
2. **PowerShell Support** - Native PowerShell integration with type-safe shell abstraction
3. **Program Resolution** - Windows script execution support for MCP servers
4. **Git Bash Compatibility** - Fixed readline arrow key issues in Git Bash/MinTTY terminals

## 1. Text Encoding Detection

### Problem
Windows users in non-English locales (Russian, Chinese, Japanese, etc.) see corrupted shell output because Windows uses legacy code pages (CP866, CP1251, etc.) instead of UTF-8.

### Solution
- Added `chardetng` and `encoding_rs` crates for automatic encoding detection
- Implemented smart heuristics to distinguish between similar encodings
- Applied to all shell command output in `executor.rs`

### Supported Encodings
- **CP866** (IBM866) - Russian Cyrillic console encoding
- **CP1251** (Windows-1251) - Russian Cyrillic
- **Windows-1252** - Western European with smart punctuation
- Plus 15+ additional encodings via `encoding_rs`

### Code Location
- `crates/forge_infra/src/text_encoding.rs` - Core encoding logic
- `crates/forge_infra/src/executor.rs:142-145` - Applied to shell output

### Testing
12 comprehensive tests covering:
- UTF-8 passthrough (fast path)
- CP1251 Russian text
- CP866 Russian text and uppercase
- Windows-1252 smart punctuation
- Fallback to lossy conversion
- Edge cases and mixed encodings

## 2. PowerShell Support

### Problem
Code-forge only supported cmd.exe on Windows, forcing users to use legacy syntax and missing modern PowerShell features.

### Solution
- Created `ShellType` enum for type-safe shell handling
- Implemented automatic shell discovery with PowerShell priority on Windows
- Added PowerShell-specific argument formatting (`-NoProfile -Command`)
- Detects both `pwsh` (PowerShell 7+) and `powershell` (Windows PowerShell 5.1)

### Shell Types Supported
- `PowerShell` - Modern PowerShell (pwsh) or Windows PowerShell
- `Cmd` - Windows Command Prompt
- `Bash` - Bourne Again Shell (Linux/macOS)
- `Zsh` - Z Shell (macOS default)
- `Sh` - POSIX shell (fallback)

### Code Location
- `crates/forge_infra/src/shell_type.rs` - Shell type abstraction
- `crates/forge_infra/src/env.rs:27-30` - Shell discovery integration

### Testing
7 tests covering:
- Shell type detection from paths
- Argument derivation for each shell type
- Shell discovery functionality
- Platform-specific path handling

## 3. Program Resolution

### Problem
Windows cannot execute script files (`.cmd`, `.bat`) without file extensions, breaking MCP servers that use npm tools like `npx`, `pnpm`, `yarn`.

### Solution
- Added `which` crate for cross-platform executable lookup
- Implemented Windows-specific resolver using `PATHEXT` environment variable
- Automatic extension resolution for Windows scripts
- Transparent pass-through on Unix systems

### Code Location
- `crates/forge_infra/src/program_resolver.rs` - Cross-platform resolution logic

### Testing
2 tests covering:
- Basic resolution functionality
- Platform-specific behavior

## 4. Git Bash Compatibility

### Problem
When running code-forge in Git Bash or MinTTY terminals on Windows, arrow keys (up/down) and delete key stop working after the first input. This is caused by bracketed paste mode interfering with terminal escape sequences in these terminals.

### Solution
- Added automatic detection of Git Bash and MinTTY terminals
- Disabled bracketed paste mode specifically for these terminals  
- Detection uses environment variables: `MSYSTEM`, `MSYS`, and `TERM`

### Code Location
- `crates/forge_main/src/editor.rs:66-94` - Git Bash/MinTTY detection
- `crates/forge_main/src/editor.rs:113-127` - Conditional bracketed paste

### Detection Logic
```rust
// Checks for:
// 1. MSYSTEM env var (MINGW64, MINGW32, MSYS)
// 2. TERM=xterm or TERM=cygwin (MinTTY indicators)
// 3. MSYS env var (Git Bash marker)
if is_git_bash_or_mintty() { 
    use_bracketed_paste = false;
}
```

### Impact
- ✅ Arrow keys work correctly in Git Bash
- ✅ Delete key functions properly
- ✅ Multi-line editing works in MinTTY terminals
- ✅ Other terminals (PowerShell, cmd.exe, Windows Terminal) unaffected

**Note:** This fix enables using code-forge with **zsh installed in Git Bash** (e.g., via MSYS2 or manual installation). For zsh shell integration (`forge zsh doctor`, keyboard shortcuts, etc.) to work properly on Git Bash, you need zsh installed - the default bash shell in Git Bash won't work.

### ZSH Script Execution Fix
Additionally, `forge zsh doctor` and other zsh commands now work correctly in Git Bash by explicitly passing environment variables (PATH, HOME, USERPROFILE, ZDOTDIR) to the zsh subprocess. This ensures the scripts can locate user configuration files and execute properly.

## Dependencies Added

```toml
chardetng = "0.1.17"  # Encoding detection
encoding_rs = "0.8.35"  # Multi-encoding support  
which = "8.0.0"  # Cross-platform executable lookup
```

## Breaking Changes

**None** - All changes are backwards compatible additions.

## Usage Examples

### Text Encoding (Automatic)
```rust
// In executor.rs - automatically applied to all shell output
let output = CommandOutput {
    stdout: crate::text_encoding::bytes_to_string_smart(&stdout_buffer),
    stderr: crate::text_encoding::bytes_to_string_smart(&stderr_buffer),
    // ...
};
```

### Shell Discovery
```rust
use forge_infra::{discover_shell, ShellType};

// Automatically discovers the best shell for the platform
let (shell_path, shell_type) = discover_shell(false);

// On Windows: Returns PowerShell or cmd.exe
// On macOS: Returns zsh or bash
// On Linux: Returns bash or sh
```

### Shell-Specific Arguments
```rust
let args = shell_type.derive_exec_args(&shell_path, "echo hello", false);

// PowerShell: ["pwsh.exe", "-NoProfile", "-Command", "echo hello"]
// Cmd: ["cmd.exe", "/c", "echo hello"]
// Bash: ["/bin/bash", "-c", "echo hello"]
```

### Program Resolution (Automatic)
```rust
use forge_infra::resolve_program;

let env = HashMap::new();
let program = OsString::from("npx");  // No .cmd extension needed!

// On Windows: Resolves to "C:\\...\\npx.cmd"
// On Unix: Returns "npx" unchanged
let resolved = resolve_program(program, &env)?;
```

## Testing

### Run All Tests
```bash
cd ../code-forge-windows-fixes
cargo test --package forge_infra --lib
```

### Run Specific Module Tests
```bash
# Text encoding tests
cargo test --package forge_infra --lib text_encoding

# Shell type tests  
cargo test --package forge_infra --lib shell_type

# Program resolver tests
cargo test --package forge_infra --lib program_resolver
```

## Impact

### For Windows Users
- ✅ **No more corrupted output** for non-English locales
- ✅ **Modern PowerShell support** instead of legacy cmd.exe
- ✅ **MCP servers work** with Node.js-based tools
- ✅ **Arrow keys work in Git Bash** - no more broken readline navigation
- ✅ **Seamless experience** matching Unix/macOS

### For Developers
- ✅ **Type-safe shell handling** via `ShellType` enum
- ✅ **Automatic encoding detection** - no manual conversion needed
- ✅ **Cross-platform compatibility** with platform-specific optimizations
- ✅ **Comprehensive test coverage** for Windows scenarios

## Future Enhancements

### Potential Additions (Not Critical)
1. **Windows-specific environment variables** for MCP servers:
   - `PATHEXT`, `SYSTEMROOT`, `PROGRAMFILES`, etc.
   - Currently handled generically via `get_env_var()`

2. **Windows-specific test timeouts**:
   - Longer timeouts for Windows (7s vs 2s)
   - Currently using same timeout for all platforms

3. **Windows Sandbox support**:
   - Similar to codex's `windows-sandbox-rs` crate
   - Provides ACL management and process isolation
   - Lower priority - restricted mode uses `rbash` on Unix only

## Comparison with Codex

| Feature | Codex | Code-Forge (Before) | Code-Forge (After) |
|---------|-------|-------------------|------------------|
| Text Encoding Detection | ✅ Full | ❌ None | ✅ Full |
| PowerShell Support | ✅ Full | ❌ None | ✅ Full |
| Program Resolution | ✅ Full | ❌ None | ✅ Full |
| Git Bash Compatibility | ❌ N/A | ❌ Broken | ✅ Fixed |
| Shell Type Abstraction | ✅ Enum | ❌ String | ✅ Enum |
| Windows Env Vars | ✅ 20+ vars | ⚠️ Generic | ⚠️ Generic |
| Platform-specific Timeouts | ✅ Yes | ❌ No | ❌ No |
| Windows Sandbox | ✅ Full crate | ❌ None | ❌ None |

Legend:
- ✅ Full support
- ⚠️ Partial/generic support
- ❌ Not implemented

## Files Modified

```
crates/forge_infra/
├── Cargo.toml                    # Added chardetng, encoding_rs, which
├── src/
│   ├── lib.rs                    # Exported new modules
│   ├── env.rs                    # Integrated shell discovery
│   ├── executor.rs               # Applied text encoding detection
│   ├── text_encoding.rs          # NEW: Encoding detection logic
│   ├── shell_type.rs             # NEW: Shell type abstraction
│   └── program_resolver.rs       # NEW: Windows program resolution

crates/forge_main/
└── src/
    └── editor.rs                  # MODIFIED: Git Bash/MinTTY detection
```

## References

- Original codex implementation: `../codex/codex-rs/core/src/`
- Text encoding: `text_encoding.rs`
- Shell types: `shell.rs` 
- Program resolution: `../codex/codex-rs/rmcp-client/src/program_resolver.rs`
