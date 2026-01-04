# Markdown Formatter

A command-line tool for formatting markdown text with consistent empty lines and spacing.

## Features

- Add spaces between CJK and Latin/ASCII characters
- Add blank lines after header/table/code block
- Add blank lines before table/code block
- Remove extra blank lines
- Align table columns
- Format ordered and unordered lists

## Installation

### From Source

1. Ensure Rust 1.85+ is installed
2. Build release binary:

```bash
cd mdformat
cargo build --release
```

3. The binary will be in `target/release/mdformat`

## Usage

Basic formatting:

```bash
mdformat input.md -o formatted.md
```

Pipe from stdin/stdout:

```bash
cat input.md | mdformat > formatted.md
```

## Configuration

### Configuration Files

mdformat automatically searches for configuration files in the following order:

1. **Project config**: Searches upward from the current directory for `.mdformat.toml`
   - Starts from the current working directory
   - Searches parent directories up to the filesystem root
   - Stops at the first `.mdformat.toml` found

2. **Global config** (used only if no project config found):
   - `~/.config/mdformat/config.toml`
   - `~/.mdformat.toml`

3. **Explicit config**: `--config <path>` overrides all automatic discovery

Generate a default configuration file:

```bash
mdformat --init-config
```

### Configuration File Discovery

mdformat uses an upward search strategy similar to git, eslint, and other tools:

**Example directory structure:**
```
/home/user/project/          # Contains .mdformat.toml
├── docs/
│   └── guide/
│       └── README.md        # mdformat will find /home/user/project/.mdformat.toml
└── src/
    └── components/
        └── ui/
            └── button.md    # mdformat will find /home/user/project/.mdformat.toml
```

**Search behavior:**
- When you run `mdformat` from any subdirectory, it searches upward for `.mdformat.toml`
- The search stops at the first config file found
- If no project config is found, it falls back to global config
- Use `--config <path>` to explicitly specify a config file (bypasses all automatic search)

**Example usage:**
```bash
# From project root
cd /home/user/project
mdformat docs/guide/README.md  # Uses ./.mdformat.toml

# From deep subdirectory
cd /home/user/project/src/components/ui
mdformat button.md             # Finds ../../.mdformat.toml automatically

# Explicit config (ignores automatic discovery)
mdformat --config custom.toml button.md
```

### Configuration Options

Example `.mdformat.toml`:

```toml
[formatting]
format_tables = true        # Align table columns
format_lists = true         # Normalize list markers
blank_lines = true          # Add blank lines between elements
merge_blank_lines = true    # Merge consecutive blank lines

[lists]
indent = 2                  # Spaces per indentation level
unordered_marker = "-"      # Unordered list marker: "-", "*", or "+"
renumber_ordered = true     # Renumber ordered lists

[headings]
numbering_start_level = 0   # Add numbering: 0=off, 1=from H1, 2=from H2...
blank_line_after = true     # Add blank line after headings

[spacing]
cjk_ascii = true            # Add spaces between CJK and ASCII
around_code_spans = true    # Add spaces around inline code spans
```

### Command Line Overrides

Command line options override configuration file settings:

```bash
# Custom indentation
mdformat input.md --indent 4

# Add heading numbering
mdformat input.md --heading-numbering 1

# Use custom unordered marker
mdformat input.md --unordered-marker "*"

# Disable specific features
mdformat input.md --no-format-tables --no-cjk-spacing
```

## Command Line Options

```
Formats Markdown code with consistent empty lines and spacing

Usage: mdformat [OPTIONS] [INPUT]

Arguments:
  [INPUT]  Input file (default: stdin)

Options:
  -o, --output <OUTPUT>  Output file (default: stdout)
  -i, --indent <INDENT>  Number of spaces for indentation [default: 4]
  -h, --help             Print help
  -V, --version          Print version
```

## License

MIT Licensed
