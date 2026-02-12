# Configuration Guide

This guide explains the configuration system for GGG (Great Grimoire Grabber).

## File Structure

Configuration files are in TOML format and organized in a hierarchical structure:

```
config/
├── settings.toml              # Application-wide settings
├── settings.toml.example      # Example configuration template
├── default/
│   └── settings.toml          # Default folder settings
└── {folder_name}/
    └── settings.toml          # Folder-specific settings
```

## Configuration Directory Search Order

ggg searches for configuration directories in the following priority order:

1. **`--config` flag** (highest priority)
2. **`GGG_CONFIG_DIR` environment variable**
3. **User configuration directory** (platform standard)
   - **Windows**: `%APPDATA%\ggg\`
   - **Linux/macOS**: `~/.config/ggg/`
4. **Current directory** (`./config/`)
   - Useful for development or portable installations
5. **Executable directory** (`<exe_dir>/config/`)
   - Useful for portable installations

If no configuration directory is found, it will be automatically created in the user configuration directory.

## Quick Start

For a complete configuration example, see `config/settings.toml.example` in the release archive or repository. Copy this file to your configuration directory and customize as needed.

## Application Settings (`config/settings.toml`)

Application-wide configuration file that defines default settings for all folders.

### General Settings (`[general]`)

```toml
[general]
language = "en"              # UI language: "en" or "ja" (requires restart)
theme = "classic"            # Theme (currently only "classic" supported)
minimize_to_tray = true      # Minimize to system tray
start_minimized = false      # Start minimized
skip_download_preview = true # Skip preview dialog when adding downloads
```

**Options:**
- `language` - Display language (`"en"` or `"ja"`, requires restart to apply)
- `theme` - UI theme (currently only `"classic"` is available)
- `minimize_to_tray` - Minimize to system tray (default: `true`)
- `start_minimized` - Start application minimized (default: `false`)
- `skip_download_preview` - Skip Add Download preview dialog (default: `true`)

### Download Settings (`[download]`)

```toml
[download]
default_directory = "C:\\Downloads"
max_concurrent = 3           # Global concurrent download limit
retry_count = 3              # Number of retries on failure
retry_delay = 5              # Delay between retries (seconds)
bandwidth_limit = 0          # 0 = unlimited (bytes/sec)
max_redirects = 5            # Maximum HTTP redirects to follow
user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"

# Optional: Override global limits with per-folder limits
# max_concurrent_per_folder = 2  # Max concurrent downloads per folder
# parallel_folder_count = 2      # Max folders downloading simultaneously
```

**Options:**
- `default_directory` - Default save location
- `max_concurrent` - Global concurrent download limit (default: `3`)
- `retry_count` - Number of retry attempts on failure (default: `3`)
- `retry_delay` - Seconds between retries (default: `5`)
- `bandwidth_limit` - Bandwidth limit in bytes/sec (`0` = unlimited)
- `max_redirects` - Maximum HTTP redirects to follow (default: `5`)
- `user_agent` - Default User-Agent string
- `max_concurrent_per_folder` - *(Optional)* Per-folder concurrent limit
- `parallel_folder_count` - *(Optional)* Max folders downloading simultaneously

### Network Settings (`[network]`)

```toml
[network]
proxy_enabled = false
proxy_type = "http"          # "http", "https", or "socks5"
proxy_host = ""
proxy_port = 8080
proxy_auth = false
proxy_user = ""
proxy_pass = ""
```

**Options:**
- `proxy_enabled` - Enable HTTP proxy (default: `false`)
- `proxy_type` - Proxy type: `"http"`, `"https"`, or `"socks5"`
- `proxy_host`, `proxy_port` - Proxy server address
- `proxy_auth` - Enable proxy authentication (default: `false`)
- `proxy_user`, `proxy_pass` - Proxy credentials (if auth enabled)

### Script Settings (`[scripts]`)

```toml
[scripts]
enabled = true               # Enable JavaScript script hooks
directory = "<config_dir>/scripts"  # Scripts directory (resolved at runtime)
timeout = 30                 # Script execution timeout (seconds)

# Optional: Per-script file enable/disable
[scripts.script_files]
"twitter_referer.js" = true
"filename_cleanup.js" = false
```

**Options:**
- `enabled` - Enable JavaScript script hooks (default: `true`)
- `directory` - Scripts directory (default: `<config_dir>/scripts`)
- `timeout` - Script execution timeout in seconds (default: `30`)
- `script_files` - *(Optional)* Per-script enable/disable map

### Keybindings (`[keybindings]`)

Customize keyboard shortcuts for the TUI. Each action can be bound to one or more keys.

```toml
[keybindings]
# Navigation
move_up = ["k", "Up"]
move_down = ["j", "Down"]
move_to_top = ["g", "Home"]
move_to_bottom = ["G", "End"]
page_up = "Ctrl+u"
page_down = "Ctrl+d"
focus_next_pane = "Tab"
focus_prev_pane = "BackTab"
focus_left = ["h", "Left"]
focus_right = ["l", "Right"]

# Selection
select_item = "Enter"
toggle_selection = "v"
select_all = "V"
deselect_all = "Escape"

# Actions
add_download = "a"
delete_download = "d"
toggle_download = "Space"
retry_download = "r"
resume_all = "S"
pause_all = "P"
open_context_menu = "m"
edit_item = "e"

# View
toggle_details = "i"
open_search = "/"
open_help = "?"
open_settings = "x"
switch_folder = "F"

# System
quit = ["q", "Ctrl+c"]
undo = "Ctrl+z"
refresh = "R"
```

**Key Format:**
- Single character: `"a"`, `"j"`, `"G"` (uppercase for shift)
- Special keys: `"Enter"`, `"Space"`, `"Tab"`, `"BackTab"`, `"Escape"`, `"Up"`, `"Down"`, `"Left"`, `"Right"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`, `"Delete"`, `"Insert"`, `"F1"` to `"F12"`
- Modifiers: `"Ctrl+z"`, `"Alt+x"`, `"Shift+Enter"`
- Multiple keys: Use array format `["k", "Up"]` to bind multiple keys to one action

**Available Actions:**
- **Navigation**: `move_up`, `move_down`, `move_to_top`, `move_to_bottom`, `page_up`, `page_down`, `focus_next_pane`, `focus_prev_pane`, `focus_left`, `focus_right`
- **Selection**: `select_item`, `toggle_selection`, `select_all`, `deselect_all`
- **Actions**: `add_download`, `delete_download`, `toggle_download`, `retry_download`, `resume_all`, `pause_all`, `open_context_menu`, `edit_item`
- **View**: `toggle_details`, `open_search`, `open_help`, `open_settings`, `switch_folder`
- **System**: `quit`, `undo`, `refresh`

## Folder Settings (`config/{folder_name}/settings.toml`)

Folder-specific configuration files that can override application settings. Each field is optional and inherits from application settings when omitted.

### Basic Configuration

```toml
save_path = "C:\\Downloads\\Anime"
auto_date_directory = true          # Creates YYYYMMDD subdirectories
auto_start_downloads = true         # Starts downloads immediately
```

**Required:**
- `save_path` - Download destination directory

**Optional:**
- `auto_date_directory` - Create YYYYMMDD subdirectories (default: `false`)
- `auto_start_downloads` - Auto-start downloads when added (default: `false`)

### Inheritance and Override

All folder settings are optional. When omitted, they inherit from application-level settings.

```toml
# Override script enable/disable (inherits from app settings if omitted)
scripts_enabled = true

# Folder-specific script file settings
[script_files]
"twitter_referer.js" = false        # Disable for this folder
"folder_specific.js" = true         # Enable only for this folder

# Override concurrent downloads (inherits from app settings if omitted)
max_concurrent = 2

# Override user-agent (inherits from app settings if omitted)
user_agent = "CustomAgent/1.0"

# Default headers for this folder
[default_headers]
referer = "https://example.com"
authorization = "Bearer token123"
```

**Optional Override Fields:**
- `scripts_enabled` - Override app scripts setting (`None` = inherit)
- `script_files` - Override specific script files enable/disable
- `max_concurrent` - Override global concurrent limit (`None` = inherit)
- `user_agent` - Custom User-Agent (`None` = inherit)
- `default_headers` - Default HTTP headers (e.g., `referer`)

### Settings Priority

Settings are applied in the following priority order (highest to lowest):

1. **Queue/Task Level** - Individual download task settings
2. **Folder Level** - Folder configuration file
3. **Application Level** - Application configuration file

**Example:**
- If `user_agent` is not specified at queue or folder level → Application setting is used
- If folder settings specify `user_agent` → Folder setting is used
- If task/queue specifies `user_agent` → Task setting is used (highest priority)

## Configuration Examples

### Simple Configuration

Minimal configuration example:

**config/settings.toml**:
```toml
[general]
language = "en"
theme = "classic"

[download]
default_directory = "C:\\Downloads"
max_concurrent = 3
retry_count = 3
retry_delay = 5
user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"

[network]
proxy_enabled = false

[scripts]
enabled = false
timeout = 30
```

### Folder-Specific Configuration

**Example: Pixiv Downloads**

**config/pixiv/settings.toml**:
```toml
save_path = "D:\\Art\\Pixiv"
auto_date_directory = true
auto_start_downloads = true
scripts_enabled = true
max_concurrent = 2

[default_headers]
referer = "https://www.pixiv.net/"

[script_files]
"pixiv_rewrite.js" = true      # Enable Pixiv-specific script
"filename_cleanup.js" = false  # Disable filename cleanup
```

**Example: Anime Downloads**

**config/anime/settings.toml**:
```toml
save_path = "D:\\Downloads\\Anime"
auto_date_directory = true          # Auto-create date directories
scripts_enabled = true

[default_headers]
referer = "https://anime-site.example.com"

[script_files]
"anime_downloader.js" = true
```

**Example: Images with High Concurrency**

**config/images/settings.toml**:
```toml
save_path = "D:\\Pictures\\Downloaded"
max_concurrent = 5                  # Higher concurrency for this folder
user_agent = "ImageBot/1.0"

[script_files]
"image_processor.js" = true
```

## Configuration Reload

### From TUI (Terminal UI)

1. Press `x` to open Settings screen
2. Press `Shift+R` to reload configuration files

**Note**: Configuration cannot be reloaded while downloads are active. Pause all downloads before reloading.

### Manual Editing

After manually editing configuration files, either restart the application or use the reload function described above.

## Troubleshooting

### Configuration File Not Loading

1. **Check file location**
   - Verify the search paths shown in the logs
2. **Validate TOML syntax**
   - Use an online TOML validator to check for syntax errors
3. **Escape file paths correctly**
   - Windows: Use single quotes `'C:\path'` or escape backslashes `"C:\\path"`

### Settings Not Applied

1. **Reload configuration**
   - Use `Shift+R` in TUI Settings screen
2. **Check priority**
   - Queue/task-level settings may override folder settings
   - Verify which level is setting the value
3. **Validation errors**
   - Check logs for validation errors
   - Example: `max_concurrent_per_folder * parallel_folder_count > max_concurrent` is invalid

### Permission Errors

- Verify write permissions for the configuration directory
- Windows: Ensure write access to `%APPDATA%\ggg\`
- Linux/macOS: Ensure write access to `~/.config/ggg/`

## See Also

- [Script User Guide](Script_UserGuide.md) - Complete script hook system documentation
- `config/settings.toml.example` - Complete configuration template
