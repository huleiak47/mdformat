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
