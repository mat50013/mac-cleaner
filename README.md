# mac-cleaner

Fast, safe, and free macOS storage cleaner with an impressive terminal UI (TUI). Finds hidden caches buried inside `Application Support`, generic `logs` directories, duplicate files, large files, iCloud offload candidates, and your Trash — then lets you review and clean with one key.

## Features

| Category | What it finds |
|----------|----------------|
| **Caches** | Recursive cache-signature detection (`Cache`, `Code Cache`, `GPUCache`, `*ShipIt`, etc.) across `~/Library`, dev tool caches, Docker prune targets |
| **Logs** | Any `logs`/`log` directory and `*.log` files anywhere under your home (pm2, npm, docker, app logs) |
| **Duplicates** | Size-bucket → partial hash → blake3 full hash; keeps oldest, flags newer |
| **iCloud** | Large locally-downloaded iCloud Drive files that can be evicted (`brctl evict`) |
| **Large Files** | Files ≥ 100 MB in Downloads, Documents, Desktop, Movies |
| **Trash** | Reports Trash size and lets you empty it |

### Safety

- **Trash by default** — everything reversible via Finder → Put Back
- **Tier coloring** — green (safe), yellow (moderate), red (risky)
- **Protected paths** — never touches `User`, `Default`, `Profile *`, `node_modules`, `.ssh`, Steam, Photos, etc.
- **Confirm modal** before any deletion
- **Sparse-aware sizing** — `Docker.raw` shows real disk usage, not logical 1 TB

## Install

```bash
git clone https://github.com/you/mac-cleaner.git
cd mac-cleaner
cargo install --path .
```

Or run directly:

```bash
cargo run
```

## Permissions

mac-cleaner requests **administrator privileges** at launch (via `sudo`) so it can scan system-level caches and other users' data. Your password is entered in the terminal; the TUI renders normally after elevation.

To skip auto-elevation:

```bash
mac-cleaner --no-elevate
```

**Full Disk Access** (separate from sudo) may be needed for Mail, Safari, and some TCC-protected paths. On first run the app shows a modal with a link to System Settings → Privacy & Security → Full Disk Access.

## Usage

### TUI (default)

```bash
mac-cleaner
```

#### Keybindings

| Key | Action |
|-----|--------|
| `↑`/`↓` or `j`/`k` | Move selection |
| `Tab` / `Shift+Tab` | Switch category |
| `Space` | Toggle item |
| `a` | **Select ALL in category** |
| `A` | Deselect category |
| `s` | Select all Safe items (all categories) |
| `n` | Clear all selections |
| `i` | Invert category selection |
| `Enter` | Flip duplicate keeper |
| `d` | Clean selected |
| `r` | Rescan |
| `?` | Help |
| `q` / `Esc` | Quit |

On-screen hints in the footer and detail panel always show `press a to select all`.

### CLI

```bash
# Scan and print results
mac-cleaner scan

# JSON output
mac-cleaner scan --json

# Scan specific categories
mac-cleaner scan --categories caches,logs

# Headless clean (safe items pre-selected)
mac-cleaner clean --categories caches --yes

# Dry run
mac-cleaner --dry-run

# Write default config
mac-cleaner init-config
```

## Configuration

Optional TOML at `~/.config/mac-cleaner/config.toml`:

```toml
[cache]
roots = ["~/Library/Caches", "~/Library/Application Support"]

[logs]
age_days = 7

[large]
min_bytes = 104857600  # 100 MB

[privilege]
auto_elevate = true

[delete]
mode = "trash"  # or "permanent"
```

Run `mac-cleaner init-config` to generate the full default file.

## How it works

1. **Parallel scan** — `ignore` crate multi-threaded walk with `rayon` for hashing
2. **Cache signatures** — directory names like `Cache`, `Code Cache`, `GPUCache`, `*ShipIt` anywhere under `~/Library`
3. **Real disk sizing** — `st_blocks × 512`, not `metadata().len()` (critical for sparse files)
4. **Scoring** — `size × regen_factor × staleness` ranks biggest safe wins first
5. **Background workers** — scan/clean on threads; UI updates via channels at ~30 fps

## License

MIT
