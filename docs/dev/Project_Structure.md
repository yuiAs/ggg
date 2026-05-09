# Project Structure

This document describes the directory structure and organization of this application codebase.

## Workspace Layout

The repository is a Cargo workspace with the main `ggg` binary at the root
and three Windows-only sibling crates under `windows/`:

```
ggg/
├── src/                  # Main `ggg` binary (TUI download manager)
└── windows/
    ├── ggg-ipc/          # lib    — shared IPC protocol & Named Pipe client
    ├── ggg-dnd/          # binary — Win32 drag & drop GUI helper
    └── ggg-bridge/       # binary — Chrome Native Messaging host
```

See the [Helper Components](#helper-components-windows) section below for
details on the `windows/` crates.

## Module Organization (`src/`)

### `src/app/` - Application Configuration and State

Contains application-wide configuration, settings resolution, and state management.

- **config.rs** - TOML configuration file parsing and structures
- **keybindings.rs** - Keybinding definitions and customization
- **settings.rs** - Hierarchical settings system (app → folder → queue)
- **state.rs** - Application state and runtime data

### `src/cli/` - CLI Command Handlers

Command-line interface handlers for all CLI operations.

- **mod.rs** - `clap` derive definitions for `Cli`, `Commands`, and per-command action enums
- **handler.rs** - Top-level dispatcher mapping `Commands` to handler functions
- **daemon.rs** - Headless daemon mode entry point
- **bridge.rs** - `ggg bridge install / uninstall / status` (Chrome Native Messaging host registration; Windows-only behavior)
- **error.rs** - CLI exit codes and error helpers
- **output.rs** - Output formatting (table / JSON)

Covers add/list/start/pause/remove, batch operations, priority management,
script management, debug & diagnostic tools, export/import, and bridge
registration.

### `src/download/` - Download Engine

Core download management system with HTTP client, concurrent download manager, and queue persistence.

- **circuit_breaker.rs** - Circuit breaker for failing domains
- **completion_log.rs** - Completion logging for analytics
- **folder_queue.rs** - Per-folder queue management
- **history.rs** - Download history management (completed/failed/deleted items)
- **http_client.rs** - HTTP/HTTPS client with streaming and resume support
- **http_errors.rs** - HTTP error categorization and user-friendly messages
- **manager.rs** - Concurrent download manager with global and per-folder limits
- **queue.rs** - Legacy single-queue persistence
- **task.rs** - Task data structures and state management (DownloadStatus enum)

### `src/file/` - File Operations

File-related operations including naming, sanitization, and metadata handling.

- **manager.rs** - File management operations
- **metadata.rs** - File metadata handling (Last-Modified timestamps)
- **naming.rs** - Cross-platform filename sanitization

### `src/ipc/` - Inter-Process Communication

Server-side IPC for the Named Pipe used by `ggg-dnd` and `ggg-bridge`.
Wire types live in the workspace crate `ggg-ipc` and are re-exported
here for backwards compatibility.

- **mod.rs** - Re-exports `ggg_ipc::protocol` as `crate::ipc::protocol`
- **pipe_server.rs** - Tokio-based Named Pipe server (Windows only); accepts clients on `\\.\pipe\ggg-dnd`, parses JSON-line requests, and forwards `UrlReceived` events to the TUI loop

### `src/script/` - JavaScript Runtime

JavaScript/TypeScript runtime integration using rustyscript (Deno core wrapper).

- **api.rs** - Script API definitions (ggg.* bindings)
- **engine.rs** - Script engine and execution environment
- **error.rs** - Script error types
- **events.rs** - Event types and context structures
- **executor.rs** - Script execution coordinator
- **loader.rs** - Script filesystem loader
- **message.rs** - Message-passing types for thread-safe execution
- **sender.rs** - Script request sender

### `src/tui/` - Terminal User Interface

Terminal UI using ratatui with vim-style navigation and a 3-pane layout.

- **app.rs** - TUI application logic, keyboard handlers, and state management
- **events.rs** - Keyboard and terminal event handling
- **state.rs** - UI state (pane focus, tree selection, dialogs, history)
- **ui.rs** - Main rendering logic (3-pane layout, folder tree, download list, details panel)

#### 3-Pane Layout

```
┌──────────────┬──────────────────────────────────────────────────┐
│              │                                                  │
│   Folder     │           Download List (Center)                 │
│    Tree      │                                                  │
│   (Left)     │  - Status icon, Filename, Size/Progress, Speed   │
│   ~22 cols   │                                                  │
│              ├──────────────────────────────────────────────────┤
│              │           Details Panel (Bottom)                 │
│              │  - URL, Save Path, Headers, Logs                 │
└──────────────┴──────────────────────────────────────────────────┘
   Status Bar
```

**Key UI State Types:**
- `FocusPane` - Currently focused pane (FolderTree, DownloadList, DetailsPanel)
- `FolderTreeItem` - Tree item type (Folder or CompletedNode)
- `DetailsPosition` - Details panel position (Bottom, Right, Hidden)

### `src/ui/` - UI Commands Module

Shared UI command definitions and handlers.

- **commands.rs** - Command structures and execution

### `src/util/` - Shared Utilities

Common utilities used across the application.

- **i18n.rs** - Internationalization (Mozilla Fluent integration)
- **paths.rs** - Path handling and directory management
- **sanitize.rs** - Input sanitization utilities
- **url_expansion.rs** - URL pattern expansion (e.g., range notation)

## Helper Components (`windows/`)

Windows-only sibling crates that depend on `ggg-ipc` and talk to the
running `ggg` instance over `\\.\pipe\ggg-dnd`.

### `windows/ggg-ipc/` — Shared IPC crate (lib)

The single source of truth for the IPC wire format. Pulled in by `ggg`,
`ggg-dnd`, and `ggg-bridge` so the protocol is defined exactly once.

- **src/protocol.rs** - `IpcRequest` / `IpcResponse` enums and the `\\.\pipe\ggg-dnd` constants (cross-platform)
- **src/client.rs** - Synchronous Named Pipe client: `send_request`, `send_url`, `ping`, `ClientError` (`#[cfg(windows)]`)

### `windows/ggg-dnd/` — Drag & Drop GUI helper (binary)

Lightweight Win32 GUI window that registers itself as an OLE drop target.
URLs dropped onto the window are forwarded to `ggg` via the Named Pipe.

- **src/main.rs** - Entry point, single-instance mutex, shared state
- **src/window.rs** - Win32 window class, message loop, painting
- **src/drop_target.rs** - `IDropTarget` implementation
- **src/ipc_client.rs** - Thin wrapper around `ggg-ipc` (per-call connection plus a background ping monitor)

### `windows/ggg-bridge/` — Chrome Native Messaging host (binary)

Console-subsystem binary launched by Chrome as a child process. Reads
4-byte LE length-prefixed JSON frames on stdin (per the Chrome Native
Messaging spec) and forwards each request to the Named Pipe through
`ggg-ipc::send_request`. Loops until Chrome closes stdin, so both
`sendNativeMessage` (one-shot) and `connectNative` (persistent port)
work without changes on the host side.

- **src/main.rs** - stdio framing, request dispatch, response writeback

The companion Chrome extension lives at `windows/ggg-extension/` (a
plain Manifest V3 unpacked extension; not a Cargo crate). Registration
of the host manifest and `HKCU\Software\Google\Chrome\NativeMessagingHosts`
registry entry is performed by the `ggg bridge install` subcommand
(see `src/cli/bridge.rs`).

## Key Design Patterns

### Three-Tier Settings Hierarchy

Settings are resolved in priority order:
1. **Queue/Task Level** - Highest priority
2. **Folder Level** - Overrides application defaults
3. **Application Level** - Base defaults

### Message-Passing for Scripts

Scripts execute in a separate thread with message-passing architecture for thread safety:
- Main thread sends requests to script thread
- Script thread executes JavaScript and sends results back
- No shared mutable state between threads

### Event-Driven TUI with 3-Pane Layout

The TUI uses event-driven architecture with a 3-pane layout:
- **Folder Tree** (left) - Navigate folders and access download history
- **Download List** (center) - Shows downloads filtered by selected folder
- **Details Panel** (bottom/right) - Shows details for selected download

Key navigation:
- `Tab` / `Shift+Tab` - Cycle focus between panes
- `h` / `l` - Move focus left/right between panes
- `j` / `k` - Navigate within current pane
- `D` - Toggle details position (Bottom → Right → Hidden)

The "Completed" node in the folder tree shows download history (completed, failed, deleted items).

### IPC: One Server, Many Clients, One Protocol

`ggg` runs a single `tokio::net::windows::named_pipe` server bound to
`\\.\pipe\ggg-dnd` (or a `-{pid}` fallback when the default is taken).
Both `ggg-dnd` (drag & drop GUI) and `ggg-bridge` (Chrome Native
Messaging host) act as transient clients: open pipe, send one
newline-delimited JSON request, read the response, close. Sharing the
`ggg-ipc` crate keeps `IpcRequest` / `IpcResponse` definitions in lock
step across all three binaries.

## Testing

### Unit Tests

Located within module files using `#[cfg(test)]`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // Test cases
}
```

### Integration Tests

Located in `tests/` directory:
- Download manager tests
- Queue persistence tests
- Script execution tests

## See Also

- [Architecture](ARCHITECTURE.md) - System architecture and component interaction
- [Development Guidelines](../../CLAUDE.md) - Coding standards and best practices
- [Configuration Guide](../Config.md) - Configuration system documentation
- [Script User Guide](../Script_UserGuide.md) - Script hook system documentation
