use anyhow::{Context, Result};
use clap::Parser;
use fancy_regex::{Captures, Regex};
use lazy_static::lazy_static;
use log::debug;
use markdown_table_formatter::format_tables;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MdFormatConfig {
    pub formatting: FormattingOptions,
    pub lists: ListOptions,
    pub headings: HeadingOptions,
    pub spacing: SpacingOptions,
}

/// Formatting master switches
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormattingOptions {
    /// Whether to format tables (default: true)
    pub format_tables: bool,
    /// Whether to format lists (default: true)
    pub format_lists: bool,
    /// Whether to add blank lines between elements (default: true)
    pub blank_lines: bool,
    /// Whether to merge consecutive blank lines (default: true)
    pub merge_blank_lines: bool,
}

/// List formatting options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ListOptions {
    /// Number of spaces for list indentation (default: 2)
    pub indent: usize,
    /// Unordered list marker (default: "-")
    pub unordered_marker: String,
    /// Whether to renumber ordered lists (default: true)
    pub renumber_ordered: bool,
}

/// Heading formatting options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadingOptions {
    /// Heading numbering start level 0-6 (default: 0, 0 means no numbering)
    pub numbering_start_level: u8,
    /// Whether to enforce blank line after headings (default: true)
    pub blank_line_after: bool,
}

/// Spacing processing options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpacingOptions {
    /// Whether to add spaces between CJK and ASCII/digits (default: true)
    pub cjk_ascii: bool,
    /// Whether to add spaces around code spans (default: true)
    pub around_code_spans: bool,
}

// Default value implementations
impl Default for MdFormatConfig {
    fn default() -> Self {
        Self {
            formatting: FormattingOptions::default(),
            lists: ListOptions::default(),
            headings: HeadingOptions::default(),
            spacing: SpacingOptions::default(),
        }
    }
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self {
            format_tables: true,
            format_lists: true,
            blank_lines: true,
            merge_blank_lines: true,
        }
    }
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            indent: 2,
            unordered_marker: "-".to_string(),
            renumber_ordered: true,
        }
    }
}

impl Default for HeadingOptions {
    fn default() -> Self {
        Self {
            numbering_start_level: 0,
            blank_line_after: true,
        }
    }
}

impl Default for SpacingOptions {
    fn default() -> Self {
        Self {
            cjk_ascii: true,
            around_code_spans: true,
        }
    }
}

/// Default configuration file template
const DEFAULT_CONFIG_TEMPLATE: &str = r#"# mdformat configuration file
# Generated with 'mdformat --init-config'

[formatting]
# Whether to format table alignment
format_tables = true
# Whether to format lists (unify markers, renumber)
format_lists = true
# Whether to add blank lines between different elements (headings, code blocks, tables, lists, quotes, etc.)
blank_lines = true
# Whether to merge consecutive blank lines
merge_blank_lines = true

[lists]
# Number of spaces for list indentation (per level)
indent = 2
# Unordered list marker: "-", "*", or "+"
unordered_marker = "-"
# Whether to renumber ordered lists
renumber_ordered = true

[headings]
# Heading numbering start level (0=no numbering, 1=from H1, 2=from H2...)
numbering_start_level = 0
# Whether to enforce blank line after headings
blank_line_after = true

[spacing]
# Whether to add spaces between CJK and ASCII/digits
cjk_ascii = true
# Whether to add spaces around inline code spans
around_code_spans = true
"#;

/// Find project configuration file by searching upward from start directory
///
/// Starting from start_dir, traverses up the directory tree looking for the
/// first .mdformat.toml file. Returns immediately upon finding a config file,
/// or None if no config is found up to the filesystem root.
///
/// # Arguments
/// - `start_dir`: Starting directory for the search (typically current working directory)
///
/// # Returns
/// - `Some(PathBuf)`: Path to the found configuration file
/// - `None`: No configuration file found up to filesystem root
fn find_project_config_upward(start_dir: &Path) -> Option<PathBuf> {
    // Reason: Resolve to absolute path and handle symbolic links to avoid path issues
    let start_dir = start_dir
        .canonicalize()
        .unwrap_or_else(|_| start_dir.to_path_buf());

    let mut current = start_dir.as_path();

    loop {
        let config_file = current.join(".mdformat.toml");

        // Reason: Check is_file() to exclude directories and other special files
        if config_file.exists() && config_file.is_file() {
            return Some(config_file);
        }

        // Move up to parent directory
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                // Reached filesystem root
                return None;
            }
        }
    }
}

/// Find global configuration files
///
/// Returns existing global configuration files in priority order:
/// 1. ~/.config/mdformat/config.toml
/// 2. ~/.mdformat.toml
fn find_global_configs() -> Vec<PathBuf> {
    let mut configs = Vec::new();

    // 1. Global config 1: ~/.config/mdformat/config.toml
    if let Some(config_dir) = dirs::config_dir() {
        let global_config = config_dir.join("mdformat").join("config.toml");
        if global_config.exists() {
            configs.push(global_config);
        }
    }

    // 2. Global config 2: ~/.mdformat.toml
    if let Some(home_dir) = dirs::home_dir() {
        let home_config = home_dir.join(".mdformat.toml");
        if home_config.exists() {
            configs.push(home_config);
        }
    }

    configs
}

/// Find configuration files (by priority from high to low)
///
/// Search strategy:
/// 1. Search upward from working_dir for first .mdformat.toml (project config)
/// 2. If project config found, return immediately (ignore global configs)
/// 3. If no project config found, return global configuration list
///
/// # Arguments
/// - `working_dir`: Current working directory
///
/// # Returns
/// Configuration file paths in priority order:
/// - Found project config: `[project config path]`
/// - No project config: `[global config 1, global config 2]` (if they exist)
/// - None found: `[]`
fn find_config_files(working_dir: &Path) -> Vec<PathBuf> {
    // Reason: Ensure using absolute path for search
    let working_dir = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    let mut configs = Vec::new();

    // 1. Search upward for project config: find first .mdformat.toml upward from working_dir
    if let Some(project_config) = find_project_config_upward(&working_dir) {
        configs.push(project_config);
        // Reason: Return immediately after finding project config, don't search global configs
        return configs;
    }

    // 2. Only search for global configs if no project config found
    configs.extend(find_global_configs());

    configs
}

/// Load and merge configuration files
fn load_config(working_dir: &Path, explicit_config: Option<&Path>) -> Result<MdFormatConfig> {
    // If a config file is specified, use only that file
    if let Some(config_path) = explicit_config {
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
        let config: MdFormatConfig = toml::from_str(&content)
            .with_context(|| format!("Config file format error: {:?}", config_path))?;
        validate_config(&config)?;
        return Ok(config);
    }

    // Otherwise start with default values
    let mut config = MdFormatConfig::default();

    // Find and load config files (reverse order, high priority overrides low priority)
    let config_files = find_config_files(working_dir);
    for config_path in config_files.iter().rev() {
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        // Parse and override directly (unspecified TOML fields will use Default)
        config = toml::from_str(&content)
            .with_context(|| format!("Config file format error: {:?}", config_path))?;
    }

    validate_config(&config)?;
    Ok(config)
}

/// Validate configuration value validity
fn validate_config(config: &MdFormatConfig) -> Result<()> {
    // Validate unordered list marker
    if !matches!(config.lists.unordered_marker.as_str(), "-" | "*" | "+") {
        anyhow::bail!(
            "Invalid config value: lists.unordered_marker = '{}' (must be '-', '*', or '+')",
            config.lists.unordered_marker
        );
    }

    // Validate heading numbering level
    if config.headings.numbering_start_level > 6 {
        anyhow::bail!(
            "Invalid config value: headings.numbering_start_level = {} (must be between 0-6)",
            config.headings.numbering_start_level
        );
    }

    // Validate indentation
    if config.lists.indent == 0 {
        anyhow::bail!("Invalid config value: lists.indent = 0 (must be greater than 0)");
    }

    Ok(())
}

/// Handle --init-config command
fn handle_init_config(path: Option<PathBuf>) -> Result<()> {
    let target_path = path.unwrap_or_else(|| PathBuf::from(".mdformat.toml"));

    if target_path.exists() {
        eprintln!("Warning: Config file already exists: {:?}", target_path);
        eprint!("Overwrite? [y/N] ");
        io::Write::flush(&mut io::stderr())?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Operation cancelled");
            return Ok(());
        }
    }

    std::fs::write(&target_path, DEFAULT_CONFIG_TEMPLATE)?;
    println!("Config file generated: {:?}", target_path);

    Ok(())
}

/// Build final configuration from CLI arguments and config file
fn build_final_config(args: &CliArgs) -> Result<MdFormatConfig> {
    // 1. Load config file
    let working_dir = std::env::current_dir()?;
    let mut config = load_config(&working_dir, args.config.as_deref())?;

    // 2. CLI arguments override config file
    if let Some(indent) = args.indent {
        config.lists.indent = indent;
    }

    if let Some(marker) = &args.unordered_marker {
        config.lists.unordered_marker = marker.clone();
    }

    if let Some(level) = args.heading_numbering {
        config.headings.numbering_start_level = level;
    }

    if args.no_format_tables {
        config.formatting.format_tables = false;
    }

    if args.no_format_lists {
        config.formatting.format_lists = false;
    }

    if args.no_cjk_spacing {
        config.spacing.cjk_ascii = false;
    }

    if args.no_code_span_spacing {
        config.spacing.around_code_spans = false;
    }

    if args.no_blank_lines {
        config.formatting.blank_lines = false;
    }

    // 3. Validate final configuration again
    validate_config(&config)?;

    Ok(config)
}

/// Command line arguments structure
#[derive(Parser)]
#[command(
    name = "mdformat",
    version,
    about = "Formats Markdown code with consistent empty lines and spacing"
)]
struct CliArgs {
    /// Input file (default: stdin)
    input: Option<PathBuf>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Config file path (if specified, other auto-searched config files will be ignored)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Generate example config file to specified path (default: ./.mdformat.toml)
    #[arg(long, value_name = "PATH")]
    init_config: Option<Option<PathBuf>>,

    // The following arguments will override corresponding values in config file

    /// Number of spaces for list indentation (overrides config file)
    #[arg(short, long)]
    indent: Option<usize>,

    /// Unordered list marker: "-", "*", "+" (overrides config file)
    #[arg(short = 'm', long)]
    unordered_marker: Option<String>,

    /// Heading numbering start level 0-6 (overrides config file)
    #[arg(short = 'n', long)]
    heading_numbering: Option<u8>,

    /// Disable table formatting
    #[arg(long)]
    no_format_tables: bool,

    /// Disable list formatting
    #[arg(long)]
    no_format_lists: bool,

    /// Disable CJK-ASCII spacing
    #[arg(long)]
    no_cjk_spacing: bool,

    /// Disable spacing around code spans
    #[arg(long)]
    no_code_span_spacing: bool,

    /// Disable blank lines between elements
    #[arg(long)]
    no_blank_lines: bool,
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Handle --init-config
    if let Some(path) = args.init_config {
        return handle_init_config(path);
    }

    // Load and build configuration
    let config = build_final_config(&args)?;

    // Read input content
    let mut content = String::new();
    match &args.input {
        Some(path) => File::open(path)?.read_to_string(&mut content)?,
        None => io::stdin().read_to_string(&mut content)?,
    };

    // Format code (with configuration)
    let formatted = format_markdown(&content, &config);

    // Write output
    match &args.output {
        Some(path) => File::create(path)?.write_all(formatted.as_bytes())?,
        None => io::stdout().write_all(formatted.as_bytes())?,
    };
    Ok(())
}

fn format_markdown(text: &str, config: &MdFormatConfig) -> String {
    // Convert string to a vector of lines
    // Remove empty lines at the beginning and end
    // And remove spaces at the end of each line
    let lines = text
        .trim()
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>();

    // Format all lines
    let new_lines = format_lines(lines, config);

    // Format lists (if enabled)
    let new_lines = if config.formatting.format_lists {
        format_lists(&new_lines, &config.lists)
    } else {
        new_lines
    };

    let mut ret = new_lines.join("\n");

    // Format tables (if enabled)
    if config.formatting.format_tables {
        ret = format_tables(&ret);
    }

    // End with "\n"
    if !ret.ends_with('\n') {
        ret.push('\n');
    }

    ret
}

#[derive(Debug, PartialEq, Clone)]
enum LineState {
    Normal,
    Table,
    CodeStart,
    CodeEnd,
    Code,
    Empty,
    Title,
    List,
    Blockquote,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ListType {
    Unordered,
    Ordered,
}

#[derive(Debug, Clone)]
struct ListContext {
    list_type: ListType,
    indent: usize,
    counter: usize,
    original_number: Option<usize>,  // Used to store original number (when renumber_ordered = false)
}

fn get_line_state(line: &str, prev_state: LineState) -> LineState {
    if prev_state == LineState::CodeStart || prev_state == LineState::Code {
        if line.starts_with("```") {
            return LineState::CodeEnd;
        } else {
            return LineState::Code;
        }
    }

    if line.is_empty() {
        return LineState::Empty;
    }
    if RE_LIST_ITEM.is_match(line).unwrap_or(false) {
        return LineState::List;
    }
    if line.starts_with("```") {
        return LineState::CodeStart;
    }
    if line.starts_with('#') {
        return LineState::Title;
    }
    if line.starts_with('>') {
        return LineState::Blockquote;
    }
    if line.starts_with('|') {
        return LineState::Table;
    }
    LineState::Normal
}

/// Heading counters (for multi-level numbering)
struct HeadingCounters {
    counters: [usize; 6], // H1-H6
}

impl HeadingCounters {
    fn new() -> Self {
        Self { counters: [0; 6] }
    }

    /// Increment counter at specified level and reset deeper levels
    fn increment(&mut self, level: usize) {
        if level > 0 && level <= 6 {
            self.counters[level - 1] += 1;
            // Reset deeper levels
            for i in level..6 {
                self.counters[i] = 0;
            }
        }
    }

    /// Get current numbering string (e.g., "1.2.3")
    fn get_numbering(&self, level: usize, start_level: u8) -> String {
        if level < start_level as usize || level > 6 {
            return String::new();
        }

        let start_idx = (start_level as usize).saturating_sub(1);
        let end_idx = level;

        self.counters[start_idx..end_idx]
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// Add numbering to headings
fn add_heading_numbering(
    line: &str,
    heading_config: &HeadingOptions,
    counters: &mut HeadingCounters,
    spacing_config: &SpacingOptions,
) -> String {
    // Extract heading level
    let level = line.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return format_line(line, spacing_config);
    }

    // Update counter
    counters.increment(level);

    // Generate numbering
    if level >= heading_config.numbering_start_level as usize {
        let numbering = counters.get_numbering(level, heading_config.numbering_start_level);
        let title_text = line[level..].trim_start();

        // Remove existing numbering (simple approach: remove leading digits, dots, and spaces)
        let title_text = title_text
            .trim_start_matches(|c: char| c.is_numeric() || c == '.' || c == ' ');

        // Format: "## 1.2 Title"
        let formatted = format!("{} {} {}", "#".repeat(level), numbering, title_text);
        format_line(&formatted, spacing_config)
    } else {
        format_line(line, spacing_config)
    }
}

fn format_lines(lines: Vec<&str>, config: &MdFormatConfig) -> Vec<String> {
    let mut ret = vec![];
    let mut prev_line_state = LineState::Empty;
    let mut prev_line = "";

    // Initialize heading counters (if heading numbering is enabled)
    let mut heading_counters = if config.headings.numbering_start_level > 0 {
        Some(HeadingCounters::new())
    } else {
        None
    };

    for line in lines.iter() {
        // insert space between CJK and ASCII
        let mut cur_state = get_line_state(line, prev_line_state.clone());
        debug!("{:?}: {}", cur_state, line);

        match cur_state {
            LineState::Normal => {
                // must be an empty line after a table, code block or blockquote (if enabled)
                if config.formatting.blank_lines
                    && (prev_line_state == LineState::Table
                        || prev_line_state == LineState::CodeEnd
                        || prev_line_state == LineState::Blockquote)
                {
                    ret.push(String::new());
                }

                // Normal line needs to be formatted
                ret.push(format_line(line, &config.spacing));
            }
            LineState::CodeStart => {
                // Must be an empty line before a code block (if enabled)
                if config.formatting.blank_lines && prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
                ret.push(line.to_string());
            }
            LineState::Blockquote => {
                // Must be an empty line before a blockquote (if enabled)
                if config.formatting.blank_lines
                    && prev_line_state != LineState::Empty
                    && prev_line_state != LineState::Blockquote
                {
                    ret.push(String::new());
                }
                ret.push(format_line(line, &config.spacing));
            }
            LineState::Code | LineState::CodeEnd => {
                ret.push(line.to_string());
            }
            LineState::Table => {
                // Must be an empty line before a table (if enabled)
                if config.formatting.blank_lines
                    && prev_line_state != LineState::Table
                    && prev_line_state != LineState::Empty
                {
                    ret.push(String::new());
                }

                // Table line needs to be formatted
                ret.push(format_line(line, &config.spacing));
            }
            LineState::Empty => {
                // Merge consecutive empty lines (if enabled)
                if !config.formatting.merge_blank_lines || prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
            }
            LineState::Title => {
                // Must be an empty line after a table, list or code block (if enabled)
                if config.formatting.blank_lines
                    && (prev_line_state == LineState::Table
                        || prev_line_state == LineState::CodeEnd
                        || prev_line_state == LineState::List
                        || prev_line_state == LineState::Blockquote)
                {
                    ret.push(String::new());
                }

                // Header line needs to be formatted (may add numbering)
                let formatted = if let Some(ref mut counters) = heading_counters {
                    add_heading_numbering(line, &config.headings, counters, &config.spacing)
                } else {
                    format_line(line, &config.spacing)
                };
                ret.push(formatted);
                // Must be an empty line after a header (if enabled)
                if config.headings.blank_line_after {
                    ret.push(String::new());
                    cur_state = LineState::Empty;
                }
            }
            LineState::List => {
                if config.formatting.blank_lines
                    && prev_line_state != LineState::List
                    && prev_line_state != LineState::Empty
                {
                    // Don't add blank line if previous line is indented content (part of list)
                    if !(prev_line_state == LineState::Normal && prev_line.starts_with(' ')) {
                        ret.push(String::new());
                    }
                }
                ret.push(format_line(line, &config.spacing));
            }
        }

        prev_line_state = cur_state;
        prev_line = line;
    }
    ret
}

fn format_line(line: &str, config: &SpacingOptions) -> String {
    format_text(line, config)
}

fn format_text(text: &str, config: &SpacingOptions) -> String {
    let mut text = text.to_string();

    // CJK-ASCII spacing (based on config)
    if config.cjk_ascii {
        text = add_spaces_between_cjk_ascii(&text);
        // sometimes we need to perform this twice to make it stable
        text = add_spaces_between_cjk_ascii(&text);
    }

    // Spacing around code spans (based on config)
    if config.around_code_spans {
        text = add_space_around_code_spans(&text);
        // sometimes we need to perform this twice to make it stable
        text = add_space_around_code_spans(&text);
    }

    text
}

fn format_lists(lines: &[String], config: &ListOptions) -> Vec<String> {
    lazy_static! {
        // Regular expression to capture list lines:
        // 1: Indentation (leading spaces)
        // 2: Unordered list marker (*, +, -)
        // 3: Ordered list number
        // 4: List item content
        static ref RE_LIST_ITEM: Regex =
            Regex::new(r"^(\s*)(?:([*+-])|(\d+)\.)\s+(.*)").unwrap();
    }

    let mut result = Vec::new();
    let mut list_stack: Vec<ListContext> = Vec::new();

    for line in lines {
        if let Some(caps) = RE_LIST_ITEM.captures(line).unwrap() {
            let indent = caps.get(1).unwrap().as_str().len();
            let content = caps.get(4).unwrap().as_str();

            // Determine list type and extract original number for ordered lists
            let (current_list_type, original_number) = if caps.get(2).is_some() {
                (ListType::Unordered, None)
            } else {
                // Extract original number
                let num = caps.get(3).unwrap().as_str().parse::<usize>().unwrap_or(1);
                (ListType::Ordered, Some(num))
            };

            // Adjust list level based on indentation
            while !list_stack.is_empty() && indent < list_stack.last().unwrap().indent {
                list_stack.pop();
            }

            if list_stack.is_empty() || indent > list_stack.last().unwrap().indent {
                // Enter a new sub-list
                let new_indent = if list_stack.is_empty() {
                    0
                } else {
                    // New indentation is based on the actual indentation captured by the regex
                    indent
                };
                list_stack.push(ListContext {
                    list_type: current_list_type,
                    indent: new_indent,
                    counter: 1,
                    original_number,
                });
            } else {
                // Same-level list item
                let last = list_stack.last_mut().unwrap();
                if last.list_type != current_list_type {
                    // list type changed, treat as a new list
                    list_stack.pop();
                    list_stack.push(ListContext {
                        list_type: current_list_type,
                        indent,
                        counter: 1,
                        original_number,
                    });
                } else {
                    // Update original number (if ordered list)
                    if current_list_type == ListType::Ordered {
                        last.original_number = original_number;
                    }
                    // Only increment counter when renumbering is enabled
                    if last.list_type == ListType::Ordered && config.renumber_ordered {
                        last.counter += 1;
                    }
                }
            }

            // Construct the new formatted line
            let current_context = list_stack.last().unwrap();
            let prefix_indent = " ".repeat(if list_stack.len() > 1 {
                config.indent * (list_stack.len() - 1)
            } else {
                0
            });

            let new_line = match current_context.list_type {
                ListType::Unordered => format!("{}{} {}", prefix_indent, config.unordered_marker, content),
                ListType::Ordered => {
                    // If renumbering is disabled, use original number; otherwise use counter
                    let number = if !config.renumber_ordered {
                        current_context.original_number.unwrap_or(current_context.counter)
                    } else {
                        current_context.counter
                    };
                    format!("{}{}. {}", prefix_indent, number, content)
                }
            };
            result.push(new_line);
        } else {
            // Non-list line
            if line.is_empty() {
                // Empty line: might be a separator within the same list, keep list_stack
                result.push(line.clone());
            } else if line.starts_with(' ') || line.starts_with('\t') {
                // Indented content: part of the list item (code blocks, continued text, etc.)
                // Keep list_stack intact
                result.push(line.clone());
            } else {
                // Real non-list content (text, heading, code, etc.): end the list
                list_stack.clear();
                result.push(line.clone());
            }
        }
    }

    result
}

lazy_static! {
    // Regular expression to capture list lines:
    // 1: Indentation (leading spaces)
    // 2: Unordered list marker (*, +, -)
    // 3: Ordered list number
    // 4: List item content
    static ref RE_LIST_ITEM: Regex =
        Regex::new(r"^(\s*)(?:([*+-])|(\d+)\.)\s+(.*)").unwrap();
    static ref RE_CJK: Regex =
        Regex::new(r"(\p{sc=Han})([a-zA-Z0-9])|([a-zA-Z0-9])(\p{sc=Han})").unwrap();
    static ref RE_CODE_SPAN: Regex = Regex::new(r"([^`\s]?)(`[^`]*`)([^`\s]?)").unwrap();
}
fn add_spaces_between_cjk_ascii(text: &str) -> String {
    RE_CJK
        .replace_all(text, |caps: &Captures| {
            if let Some(cjk) = caps.get(1) {
                format!("{} {}", cjk.as_str(), &caps[2])
            } else {
                format!("{} {}", caps.get(3).unwrap().as_str(), &caps[4])
            }
        })
        .to_string()
}

fn add_space_around_code_spans(text: &str) -> String {
    RE_CODE_SPAN
        .replace_all(text, |caps: &Captures| {
            let before = caps.get(1).unwrap().as_str();
            let code = caps.get(2).unwrap().as_str();
            let after = caps.get(3).unwrap().as_str();
            debug!("before: [{}], code: [{}], after: [{}]", before, code, after);
            if before.is_empty() && after.is_empty() {
                return format!("{}", code);
            } else if before.is_empty() {
                return format!("{} {}", code, after);
            } else if after.is_empty() {
                return format!("{} {}", before, code);
            } else {
                return format!("{} {} {}", before, code, after);
            }
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_table() {
        let fmt_md = format_markdown("|a|b|\n|---|---|\n| column 1 | column 2    |", &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            "| a        | b        |\n| -------- | -------- |\n| column 1 | column 2 |\n"
        );
    }

    #[test]
    fn test_insert_empty_line_for_title() {
        let fmt_md = format_markdown(
            "# title 1\n## title2\n### title3\n\n```c\n#define ABC\n```\n# title4\n| ---- | ---- |\n# title5",
            &MdFormatConfig::default(),
        );
        assert_eq!(
            fmt_md,
            "# title 1\n\n## title2\n\n### title3\n\n```c\n#define ABC\n```\n\n# title4\n\n| ---- | ---- |\n\n# title5\n"
        );
    }

    #[test]
    fn test_insert_empty_line_for_table() {
        let fmt_md = format_markdown(
            "# title 1\ntext\n| aaa | bbb |\n| --- | --- |\n| 123 | 456 |\nline text",
            &MdFormatConfig::default(),
        );
        assert_eq!(
            fmt_md,
            "# title 1\n\ntext\n\n| aaa | bbb |\n| --- | --- |\n| 123 | 456 |\n\nline text\n"
        );
    }

    #[test]
    fn test_insert_space() {
        let fmt_md = format_markdown("# 123你好2谢谢hello`你好call function()`$text谢谢$谢谢", &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            "# 123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );

        let fmt_md = format_markdown("123你好2谢谢hello`你好call function()`$text谢谢$谢谢", &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            "123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );

        let fmt_md = format_markdown("- 123你好2谢谢hello`你好call function()`$text谢谢$谢谢", &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            "- 123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );

        let fmt_md = format_markdown("1. 123你好2谢谢hello`你好call function()`$text谢谢$谢谢", &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            "1. 123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );
    }

    #[test]
    fn test_join_empty_lines() {
        let fmt_md = format_markdown("line1\n\n\nline2\n\n  \n  \nline3", &MdFormatConfig::default());
        assert_eq!(fmt_md, "line1\n\nline2\n\nline3\n");
    }

    #[test]
    fn test_code_block() {
        let input = r#"pre text
```
$ brew install ripgrep
```
after text
"#;
        let fmt_md = format_markdown(input, &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            r#"pre text

```
$ brew install ripgrep
```

after text
"#
        );
    }

    #[test]
    fn test_code_span() {
        env_logger::init();
        let input = "`start`ignored `by` your `.gitignore`/`.ignore`/`.rgignore` files`end`";
        let fmt_md = format_markdown(input, &MdFormatConfig::default());
        assert_eq!(
            fmt_md,
            "`start` ignored `by` your `.gitignore` / `.ignore` / `.rgignore` files `end`\n"
        );
    }

    #[test]
    fn test_format_lists() {
        // Test case 1: Unordered list with mixed markers
        let input1 = "* item 1
+ item 2
- item 3";
        let expected1 = "- item 1
- item 2
- item 3
";
        assert_eq!(format_markdown(input1, &MdFormatConfig::default()), expected1);

        // Test case 2: Ordered list with incorrect numbering
        let input2 = "1. item 1
3. item 2
2. item 3";
        let expected2 = "1. item 1
2. item 2
3. item 3
";
        assert_eq!(format_markdown(input2, &MdFormatConfig::default()), expected2);

        // Test case 3: Nested unordered list
        let input3 = "* level 1
  + level 2
    - level 3";
        let expected3 = "- level 1
  - level 2
    - level 3
";
        assert_eq!(format_markdown(input3, &MdFormatConfig::default()), expected3);

        // Test case 4: Nested ordered list
        let input4 = "1. level 1
   2. level 2
      3. level 3";
        let expected4 = "1. level 1
  1. level 2
    1. level 3
";
        assert_eq!(format_markdown(input4, &MdFormatConfig::default()), expected4);

        // Test case 5: Mixed nested list
        let input5 = "* level 1
  1. sub 1
  2. sub 2
* level 1";
        let expected5 = "- level 1
  1. sub 1
  2. sub 2
- level 1
";
        assert_eq!(format_markdown(input5, &MdFormatConfig::default()), expected5);

        // Test case 6: List with intermittent text
        let input6 = "1. item 1

not a list

2. item 2";
        let expected6 = "1. item 1

not a list

1. item 2
";
        assert_eq!(format_markdown(input6, &MdFormatConfig::default()), expected6);

        // Test case 7: Deeply nested list
        let input7 = "1. L1
    * L2
        3. L3
            + L4";
        let expected7 = "1. L1
  - L2
    1. L3
      - L4
";
        assert_eq!(format_markdown(input7, &MdFormatConfig::default()), expected7);

        // Test case 8: List with extra spacing
        let input8 = "*   item 1
1.    item 2";
        let expected8 = "- item 1
1. item 2
";
        assert_eq!(format_markdown(input8, &MdFormatConfig::default()), expected8);

        // Test case 9: List preceded by a normal line, should insert an empty line
        let input9 = "This is a normal line.
* List item 1
* List item 2";
        let expected9 = "This is a normal line.

- List item 1
- List item 2
";
        assert_eq!(format_markdown(input9, &MdFormatConfig::default()), expected9);

        // Test case 10: List preceded by a title, should have an empty line
        let input10 = "# My Title
* List item 1
* List item 2";
        let expected10 = "# My Title

- List item 1
- List item 2
";
        assert_eq!(format_markdown(input10, &MdFormatConfig::default()), expected10);

        // Test case 11: Ordered list with single empty line between items
        let input11 = "1. aaa

1. bbb

1. ccc";
        let expected11 = "1. aaa

2. bbb

3. ccc
";
        assert_eq!(format_markdown(input11, &MdFormatConfig::default()), expected11);

        // Test case 12: Ordered list with multiple empty lines (will be merged by format_lines)
        let input12 = "1. first


1. second";
        let expected12 = "1. first

2. second
";
        assert_eq!(format_markdown(input12, &MdFormatConfig::default()), expected12);

        // Test case 13: Unordered list with empty lines (should not be affected)
        let input13 = "- first

- second

- third";
        let expected13 = "- first

- second

- third
";
        assert_eq!(format_markdown(input13, &MdFormatConfig::default()), expected13);

        // Test case 14: Nested ordered list with empty lines
        let input14 = "1. level 1

  1. level 2

  1. level 2 item 2

1. level 1 item 2";
        let expected14 = "1. level 1

  1. level 2

  2. level 2 item 2

2. level 1 item 2
";
        assert_eq!(format_markdown(input14, &MdFormatConfig::default()), expected14);

        // Test case 15: Ordered list with nested unordered list and empty lines
        let input15 = "1. ordered 1

  - unordered sub

  - unordered sub 2

1. ordered 2";
        let expected15 = "1. ordered 1

  - unordered sub

  - unordered sub 2

2. ordered 2
";
        assert_eq!(format_markdown(input15, &MdFormatConfig::default()), expected15);

        // Test case 16: Ordered list + real text + ordered list (should reset numbering)
        let input16 = "1. first

Some paragraph text.

1. second";
        let expected16 = "1. first

Some paragraph text.

1. second
";
        assert_eq!(format_markdown(input16, &MdFormatConfig::default()), expected16);
    }

    #[test]
    fn test_blockquote() {
        let input = "text before\n> quote 1\n> quote 2\ntext after";
        let expected = "text before\n\n> quote 1\n> quote 2\n\ntext after\n";
        assert_eq!(format_markdown(input, &MdFormatConfig::default()), expected);

        let input2 = "> quote\n# title";
        let expected2 = "> quote\n\n# title\n";
        assert_eq!(format_markdown(input2, &MdFormatConfig::default()), expected2);

        // list before quote
        let input3 = "- list item\n> quote";
        let expected3 = "- list item\n\n> quote\n";
        assert_eq!(format_markdown(input3, &MdFormatConfig::default()), expected3);

        // quote before list
        let input4 = "> quote\n- list item";
        let expected4 = "> quote\n\n- list item\n";
        assert_eq!(format_markdown(input4, &MdFormatConfig::default()), expected4);

        // code block before quote
        let input5 = "```\ncode\n```\n> quote";
        let expected5 = "```\ncode\n```\n\n> quote\n";
        assert_eq!(format_markdown(input5, &MdFormatConfig::default()), expected5);

        // quote before code block
        let input6 = "> quote\n```\ncode\n```";
        let expected6 = "> quote\n\n```\ncode\n```\n";
        assert_eq!(format_markdown(input6, &MdFormatConfig::default()), expected6);
    }

    #[test]
    fn test_ordered_list_with_indented_code_block() {
        // Test case 1: Basic scenario - ordered list with indented code block
        let input1 = "1. aaa\n  ```c\n  int a = 0;\n  ```\n\n\n2. bbb\n\n3. ccc";
        let expected1 = "1. aaa\n  ```c\n  int a = 0;\n  ```\n\n2. bbb\n\n3. ccc\n";
        assert_eq!(format_markdown(input1, &MdFormatConfig::default()), expected1);

        // Test case 2: Ordered list with indented text
        let input2 = "1. first item\n  continued text\n  more text\n\n2. second item";
        let expected2 = "1. first item\n  continued text\n  more text\n\n2. second item\n";
        assert_eq!(format_markdown(input2, &MdFormatConfig::default()), expected2);

        // Test case 3: Ordered list with multiple indented elements
        let input3 = "1. item one\n  ```\n  code\n  ```\n  continued text\n\n2. item two";
        let expected3 = "1. item one\n  ```\n  code\n  ```\n  continued text\n\n2. item two\n";
        assert_eq!(format_markdown(input3, &MdFormatConfig::default()), expected3);

        // Test case 4: Nested list with indented code block
        let input4 = "1. outer\n  1. inner\n    ```\n    code\n    ```\n  2. inner two\n2. outer two";
        let expected4 = "1. outer\n  1. inner\n    ```\n    code\n    ```\n  2. inner two\n2. outer two\n";
        assert_eq!(format_markdown(input4, &MdFormatConfig::default()), expected4);

        // Test case 5: Ordered list with indented code block followed by unindented text (should reset)
        let input5 = "1. first\n  ```\n  code\n  ```\n\nNormal text here\n\n1. new list";
        let expected5 = "1. first\n  ```\n  code\n  ```\n\nNormal text here\n\n1. new list\n";
        assert_eq!(format_markdown(input5, &MdFormatConfig::default()), expected5);
    }

    // ===== Added: FormattingOptions tests (6 tests) =====
    #[test]
    fn test_disable_format_tables() {
        // Test disabling table formatting
        let mut config = MdFormatConfig::default();
        config.formatting.format_tables = false;

        let input = "|a|b|\n|---|---|\n| column 1 | column 2    |";
        let output = format_markdown(input, &config);

        // Table should not be formatted
        assert_eq!(output, "|a|b|\n|---|---|\n| column 1 | column 2    |\n");
    }

    #[test]
    fn test_disable_format_lists() {
        // Test disabling list formatting
        let mut config = MdFormatConfig::default();
        config.formatting.format_lists = false;

        let input = "1. first\n5. second\n+ item";
        let output = format_markdown(input, &config);

        // List numbers and markers should not be modified (not adding blank lines between list items is correct behavior)
        assert_eq!(output, "1. first\n5. second\n+ item\n");
    }

    #[test]
    fn test_disable_cjk_spacing() {
        // Test disabling CJK-ASCII spacing
        let mut config = MdFormatConfig::default();
        config.spacing.cjk_ascii = false;

        let input = "123你好world";
        let output = format_markdown(input, &config);

        // Should not add spaces
        assert_eq!(output, "123你好world\n");
    }

    #[test]
    fn test_disable_code_span_spacing() {
        // Test disabling spacing around code spans
        let mut config = MdFormatConfig::default();
        config.spacing.around_code_spans = false;

        let input = "text`code`text";
        let output = format_markdown(input, &config);

        // Should not add spaces
        assert_eq!(output, "text`code`text\n");
    }

    #[test]
    fn test_disable_blank_lines() {
        // Test disabling blank lines between elements
        let mut config = MdFormatConfig::default();
        config.formatting.blank_lines = false;

        let input = "# Title\n## Subtitle\ntext\n```\ncode\n```\nmore text";
        let output = format_markdown(input, &config);

        // Should not add blank lines (blank line after heading is still controlled by heading.blank_line_after)
        assert!(output.contains("```\ncode\n```\nmore text"));
    }

    #[test]
    fn test_disable_merge_blank_lines() {
        // Test disabling merging consecutive blank lines
        let mut config = MdFormatConfig::default();
        config.formatting.merge_blank_lines = false;

        let input = "line1\n\n\n\nline2";
        let output = format_markdown(input, &config);

        // Consecutive blank lines should be preserved (input has 3 blank lines, output should also have 3)
        assert_eq!(output, "line1\n\n\n\nline2\n");
    }

    // ===== Added: ListOptions tests (4 tests) =====
    #[test]
    fn test_custom_list_indent_4() {
        // Test custom indentation: 4 spaces
        let mut config = MdFormatConfig::default();
        config.lists.indent = 4;

        let input = "- level 1\n  - level 2\n    - level 3";
        let output = format_markdown(input, &config);

        let expected = "- level 1\n    - level 2\n        - level 3\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_unordered_marker_star() {
        // Test using * marker
        let mut config = MdFormatConfig::default();
        config.lists.unordered_marker = "*".to_string();

        let input = "- item 1\n+ item 2\n* item 3";
        let output = format_markdown(input, &config);

        let expected = "* item 1\n* item 2\n* item 3\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_unordered_marker_plus() {
        // Test using + marker
        let mut config = MdFormatConfig::default();
        config.lists.unordered_marker = "+".to_string();

        let input = "- item 1\n* item 2";
        let output = format_markdown(input, &config);

        let expected = "+ item 1\n+ item 2\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_disable_renumber_ordered() {
        // Test disabling renumbering of ordered lists
        let mut config = MdFormatConfig::default();
        config.lists.renumber_ordered = false;

        let input = "1. first\n5. second\n3. third";
        let output = format_markdown(input, &config);

        // Numbers should remain as-is
        assert_eq!(output, "1. first\n5. second\n3. third\n");
    }

    // ===== Added: HeadingOptions tests (5 tests) =====
    #[test]
    fn test_heading_numbering_from_h1() {
        // Test numbering starting from H1
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 1;

        let input = "# Title\n## Sub\n### SubSub\n## Sub2";
        let output = format_markdown(input, &config);

        let expected = "# 1 Title\n\n## 1.1 Sub\n\n### 1.1.1 SubSub\n\n## 1.2 Sub2\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_numbering_from_h2() {
        // Test numbering starting from H2
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 2;

        let input = "# Title\n## Sub1\n### SubSub\n## Sub2";
        let output = format_markdown(input, &config);

        let expected = "# Title\n\n## 1 Sub1\n\n### 1.1 SubSub\n\n## 2 Sub2\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_numbering_from_h3() {
        // Test numbering starting from H3
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 3;

        let input = "# Title\n## Sub\n### SubSub1\n### SubSub2";
        let output = format_markdown(input, &config);

        let expected = "# Title\n\n## Sub\n\n### 1 SubSub1\n\n### 2 SubSub2\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_no_blank_line_after() {
        // Test disabling blank line after headings
        let mut config = MdFormatConfig::default();
        config.headings.blank_line_after = false;

        let input = "# Title\ntext";
        let output = format_markdown(input, &config);

        let expected = "# Title\ntext\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_numbering_skip_levels() {
        // Test numbering with skipped heading levels
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 1;

        let input = "# H1\n### H3\n## H2";
        let output = format_markdown(input, &config);

        let expected = "# 1 H1\n\n### 1.0.1 H3\n\n## 1.1 H2\n";
        assert_eq!(output, expected);
    }

    // ===== Added: HeadingOptions advanced tests (existing numbering handling) (4 tests) =====
    #[test]
    fn test_heading_renumber_with_existing_jump_numbering() {
        // Test renumbering headings with existing jump numbering
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 1;

        let input = "# 1 First\n# 5 Second\n# 10 Third";
        let output = format_markdown(input, &config);

        let expected = "# 1 First\n\n# 2 Second\n\n# 3 Third\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_add_numbering_to_clean_titles() {
        // Test adding numbering to headings without numbering
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 1;

        let input = "# Introduction\n## Background\n### Details\n## Methodology";
        let output = format_markdown(input, &config);

        let expected = "# 1 Introduction\n\n## 1.1 Background\n\n### 1.1.1 Details\n\n## 1.2 Methodology\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_fix_incorrect_level_numbering() {
        // Test fixing incorrect level numbering in headings
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 1;

        // H2 incorrectly uses H1 format numbering "2", should be "1.1"
        // H3 incorrectly uses H1 format numbering "3", should be "1.1.1"
        let input = "# 1 Main Title\n## 2 Subtitle\n### 3 Detail\n## 4 Another Subtitle";
        let output = format_markdown(input, &config);

        let expected = "# 1 Main Title\n\n## 1.1 Subtitle\n\n### 1.1.1 Detail\n\n## 1.2 Another Subtitle\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_heading_mixed_numbered_and_clean_titles() {
        // Test headings with mixed numbering and no numbering
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 1;

        let input = "# 1.2.3 Title1\n## Clean Title\n### 5.6 Title3\n## 10 Title4";
        let output = format_markdown(input, &config);

        let expected = "# 1 Title1\n\n## 1.1 Clean Title\n\n### 1.1.1 Title3\n\n## 1.2 Title4\n";
        assert_eq!(output, expected);
    }

    // ===== Added: Configuration validation tests (3 tests) =====
    #[test]
    fn test_validate_invalid_unordered_marker() {
        // Test invalid unordered list marker
        let mut config = MdFormatConfig::default();
        config.lists.unordered_marker = "x".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unordered_marker"));
    }

    #[test]
    fn test_validate_invalid_numbering_level() {
        // Test invalid numbering level
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 7;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("numbering_start_level"));
    }

    #[test]
    fn test_validate_zero_indent() {
        // Test zero indentation
        let mut config = MdFormatConfig::default();
        config.lists.indent = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("indent"));
    }

    // ===== Added: Configuration combination tests (5 tests) =====
    #[test]
    fn test_complex_config_combination() {
        // Test complex configuration combination
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 2;
        config.lists.indent = 3;
        config.lists.unordered_marker = "+".to_string();
        config.spacing.cjk_ascii = false;

        let input = "# Main\n## Sub\n### Detail\n- item\n  - sub\n你好world";
        let output = format_markdown(input, &config);

        // Verify all configurations take effect
        assert!(output.contains("## 1 Sub"));
        assert!(output.contains("### 1.1 Detail"));
        assert!(output.contains("+ item\n   + sub"));
        assert!(output.contains("你好world"));  // No spaces
    }

    #[test]
    fn test_minimal_formatting_config() {
        // Test minimal formatting configuration
        let mut config = MdFormatConfig::default();
        config.formatting.format_tables = false;
        config.formatting.format_lists = false;
        config.formatting.blank_lines = false;
        config.spacing.cjk_ascii = false;
        config.spacing.around_code_spans = false;

        let input = "# Title\ntext\n|a|b|\n- item\n你好world";
        let output = format_markdown(input, &config);

        // Only keep basic structure, no formatting
        assert!(output.contains("你好world"));
    }

    #[test]
    fn test_boundary_indent_value() {
        // Test boundary indentation value
        let mut config = MdFormatConfig::default();
        config.lists.indent = 1;

        let input = "- item\n  - sub";
        let output = format_markdown(input, &config);

        let expected = "- item\n - sub\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_max_heading_numbering_level() {
        // Test maximum numbering level
        let mut config = MdFormatConfig::default();
        config.headings.numbering_start_level = 6;

        let input = "# H1\n###### H6";
        let output = format_markdown(input, &config);

        assert!(output.contains("# H1"));  // H1 not numbered
        assert!(output.contains("###### 1 H6"));  // H6 numbered
    }

    #[test]
    fn test_all_format_switches_off() {
        // Test all formatting switches off
        let mut config = MdFormatConfig::default();
        config.formatting.format_tables = false;
        config.formatting.format_lists = false;
        config.formatting.blank_lines = false;
        config.formatting.merge_blank_lines = false;
        config.headings.blank_line_after = false;
        config.spacing.cjk_ascii = false;
        config.spacing.around_code_spans = false;

        let input = "# Title\ntext\n\n\n|a|b|\n- item";
        let output = format_markdown(input, &config);

        // Should keep original format (except line ending handling)
        assert!(output.lines().count() >= input.lines().count());
    }

    // ===== Added: Configuration file upward search tests (7 tests) =====

    #[test]
    fn test_find_config_in_current_dir() {
        // Test finding config file in current directory
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".mdformat.toml");
        std::fs::write(&config_path, "[formatting]\n").unwrap();

        let result = find_project_config_upward(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), config_path);
    }

    #[test]
    fn test_find_config_in_parent_dir() {
        // Test finding config file in parent directory
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".mdformat.toml");
        std::fs::write(&config_path, "[formatting]\n").unwrap();

        // Create subdirectory
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        // Search from subdirectory, should find parent directory's config
        let result = find_project_config_upward(&sub_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), config_path);
    }

    #[test]
    fn test_find_config_multiple_levels_up() {
        // Test searching multiple levels upward
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".mdformat.toml");
        std::fs::write(&config_path, "[formatting]\n").unwrap();

        // Create deeply nested directory
        let nested_dir = temp_dir.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested_dir).unwrap();

        // Search from deep directory
        let result = find_project_config_upward(&nested_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), config_path);
    }

    #[test]
    fn test_find_config_stops_at_first_match() {
        // Test stopping at first config found
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create config in root directory
        let root_config = temp_dir.path().join(".mdformat.toml");
        std::fs::write(&root_config, "[formatting]\n").unwrap();

        // Create config in subdirectory
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        let sub_config = sub_dir.join(".mdformat.toml");
        std::fs::write(&sub_config, "[formatting]\n").unwrap();

        // Search from subdirectory, should only find subdirectory's config
        let result = find_project_config_upward(&sub_dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), sub_config);
    }

    #[test]
    fn test_find_config_not_found() {
        // Test config file not found
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        // Don't create any config file
        let result = find_project_config_upward(&sub_dir);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_config_ignores_directory_named_config() {
        // Test ignoring directory named .mdformat.toml
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create a directory named .mdformat.toml (not a file)
        let config_dir = temp_dir.path().join(".mdformat.toml");
        std::fs::create_dir(&config_dir).unwrap();

        // Should not be found (because it's a directory, not a file)
        let result = find_project_config_upward(temp_dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_find_config_files_prefers_project_over_global() {
        // Test project config preferred over global config
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_config = temp_dir.path().join(".mdformat.toml");
        std::fs::write(&project_config, "[formatting]\n").unwrap();

        // Call find_config_files
        let configs = find_config_files(temp_dir.path());

        // Should only return project config, not include global configs
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0], project_config);
    }
}
