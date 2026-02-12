# Download Queue Architecture

This document describes the download queue system architecture, including terminology definitions, data structures, and concurrency control mechanisms.

## Terminology

| Term | Definition | Code Location |
|------|------------|---------------|
| **Task** | A single download unit representing one URL to download. Contains metadata like URL, filename, status, progress, etc. | `DownloadTask` in `src/download/task.rs` |
| **Queue** | An ordered collection of Tasks belonging to a single Folder. Each Folder has exactly one Queue. | `FolderQueue` in `src/download/folder_queue.rs` |
| **Folder** | A logical grouping that owns a Queue and has its own configuration (save path, concurrency limits, etc.). Identified by `folder_id`. | `FolderConfig` in `src/app/config.rs` |
| **Slot** | An execution permit for downloading. One Slot = one concurrent download. Controlled by semaphores. | `Semaphore` permits |
| **History** | Completed downloads storage. Separate from active Queues. | `DownloadHistory` in `src/download/history.rs` |

### Status Lifecycle

```
Pending → Downloading → Completed
    ↓         ↓            ↓
    └─────→ Failed ←───────┘
              ↓
           Deleted (removed from queue)
```

| Status | Description |
|--------|-------------|
| `Pending` | Waiting in queue, not yet started |
| `Downloading` | Actively downloading (occupies a Slot) |
| `Completed` | Download finished successfully → moved to History |
| `Failed` | Download failed after all retries |
| `Deleted` | Marked for removal (cleanup state) |

## Data Structures

### DownloadTask

The fundamental unit of work.

```rust
pub struct DownloadTask {
    pub id: Uuid,                    // Unique identifier
    pub url: String,                 // Download URL
    pub filename: String,            // Target filename
    pub save_path: PathBuf,          // Directory to save file
    pub folder_id: String,           // Parent folder ID
    pub size: Option<u64>,           // Total size (if known)
    pub downloaded: u64,             // Bytes downloaded
    pub status: DownloadStatus,      // Current status
    pub priority: i32,               // Queue priority (higher = first)
    pub created_at: DateTime<Utc>,   // Creation timestamp
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    // ... resume support fields, error info, etc.
}
```

### FolderQueue

Manages Tasks for a single Folder with concurrency control.

```rust
pub struct FolderQueue {
    pub folder_id: String,
    tasks: Arc<RwLock<VecDeque<DownloadTask>>>,  // Ordered task list
    semaphore: Arc<Semaphore>,                    // Per-folder slot limit
    counts: Arc<RwLock<FolderTaskCounts>>,        // O(1) status counts
}

pub struct FolderTaskCounts {
    pub pending: usize,
    pub downloading: usize,
}
```

### DownloadManager

Central orchestrator that manages all FolderQueues.

```rust
pub struct DownloadManager {
    // Per-folder queues
    folder_queues: Arc<RwLock<HashMap<String, FolderQueue>>>,
    
    // Global concurrency control
    global_semaphore: Arc<Semaphore>,    // Global slot limit
    max_concurrent: usize,               // Global max concurrent downloads
    
    // Per-folder concurrency control
    max_concurrent_per_folder: usize,    // Max slots per folder
    parallel_folder_count: usize,        // Max folders downloading simultaneously
    active_folders: Arc<RwLock<HashSet<String>>>,  // Currently active folders
    
    // Shared resources
    http_client: Arc<HttpClient>,
    history: Arc<RwLock<DownloadHistory>>,
    circuit_breaker: Arc<CircuitBreaker>,
    // ...
}
```

## Concurrency Model

### Three-Level Slot Control

```
┌─────────────────────────────────────────────────────────┐
│                    Global Semaphore                      │
│              (max_concurrent = 8 slots)                  │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─────────────────┐  ┌─────────────────┐               │
│  │ Active Folders  │  │ Waiting Folders │               │
│  │ (max 2 folders) │  │                 │               │
│  ├─────────────────┤  ├─────────────────┤               │
│  │ Folder A        │  │ Folder C        │               │
│  │ ├─ Semaphore(3) │  │ (waiting)       │               │
│  │ ├─ Task 1 [DL]  │  │                 │               │
│  │ ├─ Task 2 [DL]  │  └─────────────────┘               │
│  │ └─ Task 3 [P]   │                                    │
│  │                 │                                    │
│  │ Folder B        │                                    │
│  │ ├─ Semaphore(3) │                                    │
│  │ ├─ Task 4 [DL]  │                                    │
│  │ └─ Task 5 [P]   │                                    │
│  └─────────────────┘                                    │
│                                                          │
│  [DL] = Downloading (using slot)                        │
│  [P]  = Pending (waiting for slot)                      │
└─────────────────────────────────────────────────────────┘
```

### Configuration Parameters

| Parameter | Config Key | Default | Description |
|-----------|------------|---------|-------------|
| Global Max | `download.max_concurrent` | 4 | Total simultaneous downloads |
| Per-Folder Max | `download.max_concurrent_per_folder` | 4 | Max downloads per folder |
| Active Folders | `download.parallel_folder_count` | 1 | Max folders downloading at once |

### Slot Allocation Algorithm

1. **Folder Activation**: When a folder has pending tasks and `active_folders.len() < parallel_folder_count`, the folder is activated
2. **Slot Acquisition**: Task must acquire both:
   - Global semaphore permit (respects `max_concurrent`)
   - Folder semaphore permit (respects `max_concurrent_per_folder`)
3. **Slot Release**: On completion/failure, permits are released and next pending task is scheduled
4. **Folder Deactivation**: When a folder has no more active tasks, it's removed from `active_folders`

## Persistence

### Queue Storage

Each folder's queue is stored separately:

```
config/
└── {folder_id}/
    ├── config.toml    # Folder configuration
    └── queue.toml     # Task queue
```

**queue.toml format:**
```toml
[[tasks]]
id = "550e8400-e29b-41d4-a716-446655440000"
url = "https://example.com/file.zip"
filename = "file.zip"
save_path = "C:/Downloads"
folder_id = "folder1"
status = "Pending"
priority = 0
# ...
```

### History Storage

Completed downloads are stored in a single file:

```
config/
└── history.toml
```

### Load/Save Operations

| Operation | Method | Description |
|-----------|--------|-------------|
| Load all queues | `load_queue_from_folders()` | Scans config directories, creates FolderQueue per folder |
| Save all queues | `save_queue_to_folders()` | Saves each FolderQueue to its folder's queue.toml |
| Load history | `load_history()` | Loads completed downloads from history.toml |
| Save history | `save_history()` | Persists history to history.toml |

## Batch Operations

### Folder-Level Operations

```rust
// Start all pending tasks in a folder
manager.start_folder_tasks(folder_id) -> usize  // Returns count started

// Stop all downloading tasks in a folder
manager.stop_folder_tasks(folder_id) -> usize   // Returns count stopped
```

### Application-Level Operations

```rust
// Start all pending tasks across all folders
manager.start_all_tasks() -> usize

// Stop all downloading tasks across all folders
manager.stop_all_tasks() -> usize
```

### Individual Task Operations

```rust
// Start a specific task
manager.start_download(task_id, script_sender, config)

// Pause a specific task
manager.pause_download(task_id)
```

## TUI State Integration

The TUI maintains a view of folder downloads:

```rust
pub struct TuiState {
    // Per-folder task lists (synced from DownloadManager)
    folder_downloads: HashMap<String, Vec<DownloadTask>>,
    
    // Current view state
    current_folder_id: String,          // Selected folder for downloads
    tree_selected_index: usize,         // Visual selection in folder tree
    
    // Navigation: tree selection vs current folder
    // - ↑↓ keys: Move tree_selected_index (visual only)
    // - Enter: Sync current_folder_id with selection
    // - Mouse click: Immediate sync
}
```

### View Modes

| Tree Selection | Downloads Shown |
|----------------|-----------------|
| Folder | Tasks from `folder_downloads[folder_id]` |
| History | Completed tasks from `DownloadHistory` |

## Circuit Breaker

Protects against repeatedly failing domains:

```
Normal → (failures > threshold) → Open → (timeout) → Half-Open → (success) → Normal
                                    ↑                     │
                                    └─────── (failure) ───┘
```

| State | Behavior |
|-------|----------|
| Closed | Normal operation, requests allowed |
| Open | Requests blocked, fast-fail |
| Half-Open | Single test request allowed |

## Error Handling

### Retry Strategy

1. **Retriable errors**: Network timeouts, 5xx responses, connection reset
2. **Non-retriable errors**: 4xx responses (except 408, 429), invalid URL
3. **Retry delay**: Configurable via `download.retry_delay` (seconds)
4. **Max retries**: Configurable via `download.retry_count`

### Resume Support

Tasks support resume via HTTP Range requests:
- `resume_supported`: Set after first response if server supports Range
- `etag` / `last_modified`: Validates partial file hasn't changed
- On resume: Sends `Range: bytes={downloaded}-` header

---

# Script Hook System Architecture

This section describes the JavaScript scripting system that allows users to customize download behavior through event hooks.

## Terminology

| Term | Definition | Code Location |
|------|------------|---------------|
| **Hook** | An event point where user scripts can intercept and modify behavior | `HookEvent` enum in `src/script/events.rs` |
| **Handler** | A JavaScript callback function registered for a specific hook | `EventHandler` in `src/script/engine.rs` |
| **Context** | Data passed to handlers, may be read-only or modifiable | `*Context` structs in `src/script/events.rs` |
| **Script** | A `.js` file containing handler registrations | Loaded by `ScriptLoader` |
| **Filter** | URL pattern (regex) to conditionally execute handlers | `UrlFilter` in `src/script/engine.rs` |

## Module Structure

```
src/script/
├── mod.rs          # ScriptManager: high-level orchestration
├── engine.rs       # ScriptEngine: Deno runtime wrapper, handler registry
├── executor.rs     # script_executor_loop: message processing thread
├── message.rs      # ScriptRequest: inter-thread communication protocol
├── sender.rs       # Helper functions for sending requests
├── events.rs       # HookEvent enum and Context structs
├── loader.rs       # ScriptLoader: file discovery and reading
├── error.rs        # ScriptError types
└── api.rs          # (placeholder for future ggg.* API)
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Main Thread (Tokio)                         │
│  ┌─────────────────┐     ┌─────────────────┐                       │
│  │ DownloadManager │     │    TuiApp       │                       │
│  │                 │     │                 │                       │
│  │ download_task() │     │ reload_scripts  │                       │
│  └────────┬────────┘     └────────┬────────┘                       │
│           │                       │                                 │
│           ▼                       ▼                                 │
│  ┌────────────────────────────────────────┐                        │
│  │     mpsc::Sender<ScriptRequest>        │                        │
│  │         (script_sender)                │                        │
│  └────────────────────┬───────────────────┘                        │
└───────────────────────┼─────────────────────────────────────────────┘
                        │ Channel
                        ▼
┌───────────────────────────────────────────────────────────────────┐
│                    Script Executor Thread                          │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                   script_executor_loop()                     │  │
│  │  ┌─────────────────┐    ┌─────────────────────────────────┐ │  │
│  │  │  ScriptManager  │───▶│        ScriptEngine             │ │  │
│  │  │                 │    │  ┌───────────────────────────┐  │ │  │
│  │  │ trigger_*()     │    │  │ rustyscript Runtime       │  │ │  │
│  │  │ methods         │    │  │ (Deno core)               │  │ │  │
│  │  └─────────────────┘    │  └───────────────────────────┘  │ │  │
│  │                         │  handlers: Vec<EventHandler>    │ │  │
│  │                         └─────────────────────────────────┘ │  │
│  └─────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────┘
```

## Hook Events

| Hook | Timing | Context Type | Modifiable | Use Case |
|------|--------|--------------|------------|----------|
| `beforeRequest` | Before HTTP request | `BeforeRequestContext` | ✅ Yes | Modify URL, headers, user-agent |
| `headersReceived` | After response headers | `HeadersReceivedContext` | ❌ No | Inspect status, content-type |
| `authRequired` | On 401/407 response | `AuthRequiredContext` | ✅ Yes | Provide credentials |
| `completed` | After successful download | `CompletedContext` | ✅ Yes | Rename/move file |
| `error` | On download failure | `ErrorContext` | ❌ No | Log errors, notifications |
| `progress` | During download | `ProgressContext` | ❌ No | Progress tracking |

### Sync vs Async Hooks

| Type | Hooks | Behavior |
|------|-------|----------|
| **Sync** | `beforeRequest`, `headersReceived`, `authRequired`, `completed` | Blocks download until handler returns |
| **Async** | `error`, `progress` | Fire-and-forget, no response waited |

## Data Flow

### Sync Hook (e.g., beforeRequest)

```
DownloadManager                    Channel                    Executor Thread
      │                               │                              │
      │ 1. Create BeforeRequestContext│                              │
      │ 2. Create oneshot channel     │                              │
      │                               │                              │
      ├──── ScriptRequest::BeforeRequest ────────────────────────────▶
      │     { ctx, response_tx }      │                              │
      │                               │                              │
      │ 3. await response_rx          │         4. execute_handlers()│
      │    (blocks download)          │            for each handler  │
      │                               │                              │
      ◀────────────────────────────── (modified_ctx, result) ────────┤
      │                               │                              │
      │ 5. Use modified context       │                              │
      │    for HTTP request           │                              │
```

### Async Hook (e.g., progress)

```
DownloadManager                    Channel                    Executor Thread
      │                               │                              │
      │ 1. Create ProgressContext     │                              │
      │                               │                              │
      ├──── ScriptRequest::Progress ─────────────────────────────────▶
      │     { ctx }                   │                              │
      │                               │         2. execute_handlers()│
      │ (continues immediately)       │            (fire-and-forget) │
      │                               │                              │
```

## Key Data Structures

### ScriptRequest

Message protocol for inter-thread communication.

```rust
pub enum ScriptRequest {
    BeforeRequest {
        ctx: BeforeRequestContext,
        effective_script_files: HashMap<String, bool>,  // Script enable/disable
        response: mpsc::Sender<(BeforeRequestContext, ScriptResult<()>)>,
    },
    HeadersReceived { /* similar */ },
    AuthRequired { /* similar */ },
    Completed { /* similar */ },
    Error { ctx, effective_script_files },      // No response channel
    Progress { ctx, effective_script_files },   // No response channel
    Reload { response },                        // Reload scripts from disk
}
```

### Context Structs

```rust
// Modifiable context (beforeRequest)
pub struct BeforeRequestContext {
    pub url: String,           // Can be modified
    pub headers: HashMap<String, String>,
    pub user_agent: String,
    pub download_id: String,
}

// Read-only context (headersReceived)
pub struct HeadersReceivedContext {
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

// Post-download context (completed)
pub struct CompletedContext {
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub new_filename: Option<String>,  // Set by script to rename
    pub move_to_path: Option<String>,  // Set by script to move
    pub size: u64,
    pub duration: f64,
}
```

### EventHandler

```rust
pub struct EventHandler {
    pub callback_id: String,    // JS function reference
    pub filter: Option<UrlFilter>,
    pub script_path: String,    // Source script for enable/disable
}

pub struct UrlFilter {
    pub pattern: String,
    pub regex: Regex,
}
```

## Script Enable/Disable System

Scripts can be enabled/disabled at two levels:

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Level                         │
│  config.toml:                                               │
│    [scripts]                                                │
│    enabled = true                                           │
│    files = { "auth.js" = true, "rename.js" = false }       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ (inherited, can override)
┌─────────────────────────────────────────────────────────────┐
│                     Folder Level                             │
│  config/{folder_id}/config.toml:                            │
│    scripts_enabled = true  # or false to disable all        │
│    script_files = { "auth.js" = false }  # override app     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              effective_script_files (computed)              │
│  HashMap<String, bool> passed to each ScriptRequest         │
│  Handler only executes if its script_path is enabled        │
└─────────────────────────────────────────────────────────────┘
```

### Computation Logic

```rust
fn compute_effective_script_files(
    app_config: &Config,
    folder_config: Option<&FolderConfig>,
) -> HashMap<String, bool> {
    // 1. Start with app-level script_files
    // 2. If folder has scripts_enabled = false, disable all
    // 3. Apply folder-level script_files overrides
}
```

## Handler Execution

### Execution Order

1. Handlers execute in **script load order** (alphabetical by filename)
2. Within a script, handlers execute in **registration order**
3. Handler can call `e.stopPropagation()` to prevent subsequent handlers

### Filter Matching

```javascript
// Handler only runs for URLs matching the filter
ggg.on("beforeRequest", (e) => {
    e.setHeader("Authorization", "Bearer token");
}, { filter: "https://api\\.example\\.com/.*" });
```

### Error Handling

- Script errors are logged but don't crash the download
- Failed scripts are skipped, remaining scripts continue
- Sync hooks return error status to caller

## Thread Safety

| Component | Thread | Sync Mechanism |
|-----------|--------|----------------|
| ScriptEngine | Executor thread only | Single-threaded access |
| ScriptRequest | Cross-thread | mpsc channel |
| Context structs | Passed by value | Clone on send |
| effective_script_files | Per-request | Computed fresh each time |

## JavaScript Runtime

Built on **rustyscript** (Deno core wrapper):

- Full ES2022+ support
- No Node.js APIs (no `require`, no `fs`)
- Sandboxed execution (no network, no file system from JS)
- Configurable timeout per script execution

### Available APIs in Scripts

```javascript
// Event registration
ggg.on(eventName, callback, options?)

// Logging
ggg.log(message)
console.log(message)  // Also available

// Context methods (event-specific)
e.setUrl(url)           // beforeRequest
e.setHeader(key, value) // beforeRequest
e.setUserAgent(ua)      // beforeRequest
e.rename(filename)      // completed
e.moveTo(path)          // completed
e.stopPropagation()     // all events
```

## See Also

- [Script_Guide.md](../Script_Guide.md) - User guide for writing scripts
- [Project Structure](./Project_Structure.md) - Overall codebase organization
- [Config.md](../Config.md) - Configuration file reference
