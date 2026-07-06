# mac-cleaner

[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-macOS-blue?style=flat-square)](https://www.apple.com/macos/)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](#license)

![mac-cleaner dashboard — reclaimable space by category](assets/dashboard.png)

**Data-driven macOS cleanup with a fast terminal UI and one simple rule: review the evidence before deleting anything.**

mac-cleaner measures real disk usage, verifies duplicates by hash, groups reclaimable space by category, and shows exactly what will be removed before you commit. No mystery buttons. No blind "optimize" switch. Just the data you need to reclaim space with confidence.

## Contents

- [Why mac-cleaner](#why-mac-cleaner)
- [What it finds](#what-it-finds)
- [Safety model](#safety-model)
- [Requirements](#requirements)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Command line](#command-line)
- [Permissions](#permissions)
- [Configuration](#configuration)
- [How it works](#how-it-works)
- [Development](#development)
- [Troubleshooting](#troubleshooting)
- [License](#license)

## Why mac-cleaner

Most cleaners ask you to trust them. mac-cleaner shows its work.

- **See the space first.** The dashboard breaks reclaimable space down by category and refreshes as you clean.
- **Clean only what is redundant.** Duplicates are hash-verified, caches and logs are matched by known signatures, and iCloud files are evicted rather than deleted.
- **Stay in control.** Every item is visible, selectable, and color-coded by risk before anything happens.
- **Recover by default.** Normal cleanup moves files to the Trash unless you explicitly choose permanent deletion.
- **Respect macOS.** Sparse files, Trash semantics, Full Disk Access, iCloud offload, and protected app data are all handled deliberately.

## What it finds

| Category | What mac-cleaner looks for |
| --- | --- |
| **Caches** | Cache-signature directories (`Cache`, `Code Cache`, `GPUCache`, `*.ShipIt`, …), developer tool caches, and Docker prune targets |
| **Logs** | `logs` / `log` directories and `*.log` files across your home folder |
| **Duplicates** | Same-size files confirmed with a partial hash and a full `blake3` hash |
| **iCloud** | Large local iCloud Drive copies that can be evicted while staying available in iCloud |
| **Large Files** | Big files in common user folders: Downloads, Documents, Desktop, Movies |
| **Trash** | Current Trash size, with the option to empty it permanently |

## Safety model

mac-cleaner is conservative by default.

- **Review first.** Nothing is cleaned until you select it and confirm.
- **Trash by default.** Use Finder's *Put Back* to undo a normal cleanup.
- **Permanent delete is explicit.** Press `D`, never `d`, to bypass the Trash.
- **Duplicate keepers are locked.** The oldest copy in each set is kept unless you pick a different keeper.
- **Protected paths are skipped.** Browser profiles, keychains, SSH/GPG data, the Photos library, Steam, and similar sensitive folders are never touched.
- **Real sizes only.** Space is measured from allocated blocks, so sparse files (like `Docker.raw`) are never overcounted.

## Requirements

- macOS
- [Rust and Cargo](https://rustup.rs/) (stable) to build from source
- *(Optional)* Docker Desktop — enables build-cache and image reclaim under Caches

## Installation

Install the latest version straight from GitHub:

```bash
cargo install --git https://github.com/mat50013/mac-cleaner.git
```

This places the `mac-cleaner` binary in `~/.cargo/bin`. Make sure that directory is on your `PATH` (rustup usually configures this for you):

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # add to ~/.zshrc if it isn't already
```

Confirm it resolves:

```bash
which mac-cleaner   # → /Users/<you>/.cargo/bin/mac-cleaner
```

Prefer to run from a clone instead? That never needs a `PATH` change:

```bash
git clone https://github.com/mat50013/mac-cleaner.git
cd mac-cleaner
cargo run --release
```

## Quick start

Launch the interactive TUI:

```bash
mac-cleaner
```

Scan, review the dashboard, open a category, select what you want, then clean.

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | Cycle between the Dashboard and each category |
| `↑` / `↓` or `j` / `k` | Move the selection |
| `Space` | Toggle the highlighted item |
| `a` / `A` | Select all / deselect all in the category |
| `s` | Select every Safe item across all categories |
| `n` | Clear all selections |
| `i` | Invert the selection in the category |
| `Enter` | Duplicates: choose which copy to keep |
| `d` | Move selected items to the Trash |
| `D` | Delete selected items permanently |
| `r` | Rescan |
| `?` | Help |
| `q` / `Esc` | Quit |

## Command line

Every workflow is scriptable without opening the TUI.

```bash
# Scan and print a summary table
mac-cleaner scan

# Machine-readable output
mac-cleaner scan --json

# Limit the scan to specific categories
mac-cleaner scan --categories caches,logs,duplicates

# Clean the auto-selected Safe items in a category (skips the prompt)
mac-cleaner clean --categories caches --yes

# Preview any action without deleting a thing
mac-cleaner --dry-run

# Write a default config file you can edit
mac-cleaner init-config
```

Valid category slugs: `caches`, `logs`, `duplicates`, `icloud`, `large`, `trash`.

## Permissions

mac-cleaner requests administrator privileges at launch (via `sudo`) so it can size system-level caches and other users' data. Your password is entered directly in the terminal, and the TUI renders normally afterward.

Skip elevation when you only want to clean your own user folders:

```bash
mac-cleaner --no-elevate
```

**Full Disk Access** is separate from `sudo`. macOS may require it for Mail, Safari, and other TCC-protected locations. If access is limited, mac-cleaner detects it and can open the correct pane in System Settings → Privacy & Security → Full Disk Access.

## Configuration

Configuration is optional. Generate a starter file:

```bash
mac-cleaner init-config   # writes ~/.config/mac-cleaner/config.toml
```

Every field is optional — anything you omit falls back to a sensible default.

```toml
# Trash by default; use "permanent" to skip the Trash entirely.
delete_mode = "trash"

[cache]
roots = [
  "~/Library/Caches",
  "~/Library/Application Support",
]

[logs]
age_days = 7            # logs older than this are treated as Safe

[large]
min_bytes = 104857600   # 100 MB — the Large Files threshold

[duplicates]
min_bytes = 1048576     # 1 MB — ignore anything smaller when hashing

[privilege]
auto_elevate = true     # request sudo at launch
```

## How it works

1. **Parallel scanning.** Each category walks its configured roots with the `ignore` crate, using `rayon` for hashing work.
2. **Real-size accounting.** Sizes come from allocated blocks, so sparse files are never overcounted.
3. **Duplicate detection.** Files are bucketed by size, screened with a partial hash, then confirmed with a full-file `blake3` hash.
4. **Risk scoring.** Size, safety tier, and staleness combine into a priority that surfaces the biggest safe wins first.
5. **Background workers.** Scanning and cleaning run off the UI thread and stream progress back over channels, so the interface stays responsive.

## Development

```bash
# Build
cargo build

# Run the full test suite (unit + integration)
cargo test

# Include tests that touch the real macOS Trash
cargo test -- --ignored

# Format
cargo fmt
```

## Troubleshooting

**`mac-cleaner: command not found` after install.** `~/.cargo/bin` isn't on your `PATH`. Add `export PATH="$HOME/.cargo/bin:$PATH"` to `~/.zshrc`, then restart your shell — or run the binary by full path.

**"The file … is locked. (-45)" when cleaning.** The file has macOS's locked/immutable flag set. Clear it and clean again:

```bash
chflags nouchg "/path/to/file"
```

Or uncheck **Locked** in Finder's *Get Info*. Permanent delete (`D`) can also get past a locked file once the flag is cleared.

**A delete fails with "Operation not permitted."** Some files (often in `~/Downloads` when running elevated) resist being moved to the Trash as root. Try permanent delete (`D`), or relaunch with `--no-elevate` for user-only cleanup.

## License

MIT
