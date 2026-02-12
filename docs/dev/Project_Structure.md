# Project Structure

This document describes the directory structure and organization of this application codebase.

## Module Organization

### `src/app/` - Application Configuration and State

Contains application-wide configuration, settings resolution, and state management.

- **config.rs** - TOML configuration file parsing and structures
- **keybindings.rs** - Keybinding definitions and customization
- **settings.rs** - Hierarchical settings system (app → folder → queue)
- **state.rs** - Application state and runtime data

### `src/cli/` - CLI Command Handlers

Command-line interface handlers for all CLI operations.

- Batch operations
- Priority management
- Script management
- Debug and diagnostic tools
- Export/import functionality

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
