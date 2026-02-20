# file-journal

A CLI tool that creates timestamped journal entries organized in year/month folders. Entries are stored as markdown files with automatic date-based directory structure.

## File Structure

```
~/Documents/journals/
├── 2026/
│   ├── 02/
│   │   ├── 17-081503-meeting-with-team.md
│   │   └── 18-120245-ideas-for-project.md
│   └── 03/
│       └── 01-143022-daily-log.md
└── 2025/
    └── 12/
        └── 24-090000-christmas-thoughts.md
```

**Filename format:** `dd-HHMMSS-title.md`  
**Date in file:** `DD-MM-YYYY`

## Configuration

Create `~/.config/file-journal/config.toml`:

```toml
default_path = "/Users/t/Documents/journals"
```

Or initialize interactively:
```bash
file-journal init
```

## Installation

### From source (requires Rust):
```bash
git clone <repo-url>
cd file-journal
cargo build --release
# Binary will be at: target/release/file-journal
```

### Usage

```bash
# Create a new entry
file-journal new "meeting.md" "Discussed Q1 planning"

# Retrieve entries
file-journal get                    # Today's entries
file-journal get --day 17           # Specific day
file-journal get --month 2 --year 2026  # All February 2026
```
