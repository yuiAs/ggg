# Keybindings Reference

Complete keyboard shortcuts reference for GGG (Great Grimoire Grabber) terminal UI.

## Layout Overview

```
┌──────────────┬──────────────────────────────────────────────────┐
│              │                                                  │
│   Folder     │           Download List (Center)                 │
│    Tree      │                                                  │
│   (Left)     │  - Status icon, Filename, Size/Progress, Speed   │
│              │                                                  │
│              ├──────────────────────────────────────────────────┤
│              │           Details Panel (Bottom)                 │
│              │  - URL, Save Path, Headers, Logs                 │
└──────────────┴──────────────────────────────────────────────────┘
   Status Bar
```

## Pane Navigation

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus to next pane |
| `Shift+Tab` | Cycle focus to previous pane |
| `h` / `←` | Move focus left (to Folder Tree) |
| `l` / `→` | Move focus right (to Download List / Details) |

## Within-Pane Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down in current pane |
| `k` / `↑` | Move up in current pane |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |
| `Ctrl+d` | Page down |
| `Ctrl+u` | Page up |

## Folder Tree (Left Pane)

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate folders |
| `Enter` | Select folder (filters download list) |

**Special Items:**
- **Folders** - Shows downloads in that folder
- **History** - Shows completed/failed/deleted downloads

## Download List (Center Pane)

| Key | Action |
|-----|--------|
| `a` | Add new download |
| `Space` | Start/Pause selected download |
| `d` | Delete download (with confirmation) |
| `r` | Retry failed download |
| `e` | Change folder for selected download |
| `v` | Toggle selection (multi-select) |
| `V` | Select all downloads |
| `m` | Open context menu |

## Details Panel

| Key | Action |
|-----|--------|
| `D` | Toggle details position (Bottom → Right → Hidden) |

## Multi-Selection

| Key | Action |
|-----|--------|
| `v` | Toggle selection on current item |
| `V` | Select all visible downloads |
| `Esc` | Clear all selections |

Selected items can be deleted or managed together.

## Other

| Key | Action |
|-----|--------|
| `/` | Search/Filter downloads |
| `?` | Show help screen |
| `x` | Open settings |
| `F` | Switch current folder (for new downloads) |
| `Ctrl+z` | Undo last delete |
| `q` / `Ctrl+C` | Quit application |

## Settings Screen

Press `x` to open settings:

| Key | Action |
|-----|--------|
| `Tab` | Switch between Application/Folder sections |
| `j` / `k` | Navigate items |
| `Enter` | Edit selected item |
| `n` | Create new folder |
| `d` | Delete folder |
| `Shift+R` | Reload configuration |
| `Esc` / `q` | Close settings |

### Folder Edit Mode

Press `Enter` on a folder to edit:

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate fields |
| `Enter` | Edit field (text input or toggle) |
| `Esc` | Back to settings |

**Editable Fields:**
- **Save Path** - where downloads are saved
- **Auto-Date Directory** - toggle YYYYMMDD subdirectories
- **Auto-Start Downloads** - start downloads automatically
- **Scripts** - enable/disable/inherit script execution
- **Max Concurrent** - concurrent downloads for this folder
- **User Agent** - custom user-agent string

## Tips

### Adding Downloads

**Method 1: Add Dialog**
1. Press `a`
2. Type or paste URL
3. Press `Enter`

**Method 2: Drag & Drop (Windows Terminal)**
1. Drag URL from browser
2. Drop into terminal window
3. Dialog opens automatically with URL pre-filled
4. Press `Enter` to confirm

**Method 3: Clipboard (Ctrl+V)**
1. Copy URL from browser
2. Press `a` to open dialog
3. Press `Ctrl+V` to paste
4. Press `Enter`

### Managing Downloads

**Start/Pause:**
- Select download with `j`/`k`
- Press `Space`

**Delete:**
- Select download(s) with `v` for multi-select
- Press `d`
- Confirm with `y` (or cancel with `n`/`Esc`)

**Change Folder:**
- Select download
- Press `e`
- Type folder ID (e.g., "images", "videos")
- Press `Enter`

**Retry Failed:**
- Select failed download
- Press `r` to retry

### Navigating the 3-Pane Layout

**Focus Flow:**
```
Folder Tree ←→ Download List ←→ Details Panel
     ↑___________________________________↓
```

- Use `Tab` to cycle through panes
- Use `h`/`l` for direct left/right movement
- `j`/`k` navigate within the focused pane

**Filtering by Folder:**
1. Focus on Folder Tree (`h` or `Tab`)
2. Navigate to desired folder with `j`/`k`
3. Downloads are automatically filtered

**Viewing History:**
1. Navigate to "History" in Folder Tree
2. View completed, failed, and deleted downloads
3. Failed items shown in red

**Toggle Details Panel:**
- Press `D` to cycle: Bottom → Right → Hidden → Bottom
