use anyhow::Result;
use clap::Parser;
use fancy_regex::{Captures, Regex};
use lazy_static::lazy_static;
use log::debug;
use markdown_table_formatter::format_tables;
use std::{
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};

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

    /// Number of spaces for indentation
    #[arg(short, long, default_value_t = 4, value_parser = clap::value_parser!(usize))]
    indent: usize,
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Read input content
    let mut content = String::new();
    match &args.input {
        Some(path) => File::open(path)?.read_to_string(&mut content)?,
        None => io::stdin().read_to_string(&mut content)?,
    };

    // Format code
    let formatted = format_markdown(&content);

    // Write output
    match &args.output {
        Some(path) => File::create(path)?.write_all(formatted.as_bytes())?,
        None => io::stdout().write_all(formatted.as_bytes())?,
    };
    Ok(())
}

fn format_markdown(text: &str) -> String {
    // Convert string to a vector of lines
    // Remove empty lines at the beginning and end
    // And remove spaces at the end of each line
    let lines = text
        .trim()
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>();

    // Format all lines
    let new_lines = format_lines(lines);

    // Format lists
    let new_lines = format_lists(&new_lines);

    let mut ret = new_lines.join("\n");

    // Format tables
    ret = format_tables(&ret);

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

fn format_lines(lines: Vec<&str>) -> Vec<String> {
    let mut ret = vec![];
    let mut prev_line_state = LineState::Empty;

    for line in lines.iter() {
        // insert space between CJK and ASCII
        let mut cur_state = get_line_state(line, prev_line_state.clone());
        debug!("{:?}: {}", cur_state, line);

        match cur_state {
            LineState::Normal => {
                // must be an empty line after a table, code block or blockquote
                if prev_line_state == LineState::Table
                    || prev_line_state == LineState::CodeEnd
                    || prev_line_state == LineState::Blockquote
                {
                    ret.push(String::new());
                }

                // Normal line needs to be formatted
                ret.push(format_line(line));
            }
            LineState::CodeStart => {
                // Must be an empty line before a code block
                if prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
                ret.push(line.to_string());
            }
            LineState::Blockquote => {
                // Must be an empty line before a blockquote
                if prev_line_state != LineState::Empty && prev_line_state != LineState::Blockquote {
                    ret.push(String::new());
                }
                ret.push(format_line(line));
            }
            LineState::Code | LineState::CodeEnd => {
                ret.push(line.to_string());
            }
            LineState::Table => {
                // Must be an empty line before a table
                if prev_line_state != LineState::Table && prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }

                // Table line needs to be formatted
                ret.push(format_line(line));
            }
            LineState::Empty => {
                // Merge consecutive empty lines
                if prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
            }
            LineState::Title => {
                // Must be an empty line after a table, list or code block
                if prev_line_state == LineState::Table
                    || prev_line_state == LineState::CodeEnd
                    || prev_line_state == LineState::List
                    || prev_line_state == LineState::Blockquote
                {
                    ret.push(String::new());
                }

                // Header line needs to be formatted
                ret.push(format_line(line));
                // Must be an empty line after a header
                ret.push(String::new());
                cur_state = LineState::Empty;
            }
            LineState::List => {
                if prev_line_state != LineState::List && prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
                ret.push(format_line(line));
            }
        }

        prev_line_state = cur_state;
    }
    ret
}

fn format_line(line: &str) -> String {
    format_text(line)
}

fn format_text(text: &str) -> String {
    let mut text = add_spaces_between_cjk_ascii(text);
    // sometimes we need to perform this twice to make it stable
    text = add_spaces_between_cjk_ascii(&text);

    text = add_space_around_code_spans(&text);
    // sometimes we need to perform this twice to make it stable
    text = add_space_around_code_spans(&text);
    text
}

fn format_lists(lines: &[String]) -> Vec<String> {
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

            // Determine list type
            let current_list_type = if caps.get(2).is_some() {
                ListType::Unordered
            } else {
                ListType::Ordered
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
                    });
                } else if last.list_type == ListType::Ordered {
                    last.counter += 1;
                }
            }

            // Construct the new formatted line
            let current_context = list_stack.last().unwrap();
            let prefix_indent = " ".repeat(if list_stack.len() > 1 {
                2 * (list_stack.len() - 1)
            } else {
                0
            });

            let new_line = match current_context.list_type {
                ListType::Unordered => format!("{}- {}", prefix_indent, content),
                ListType::Ordered => {
                    format!("{}{}. {}", prefix_indent, current_context.counter, content)
                }
            };
            result.push(new_line);
        } else {
            // Non-list line, clear list state
            list_stack.clear();
            result.push(line.clone());
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
        let fmt_md = format_markdown("|a|b|\n|---|---|\n| column 1 | column 2    |");
        assert_eq!(
            fmt_md,
            "| a        | b        |\n| -------- | -------- |\n| column 1 | column 2 |\n"
        );
    }

    #[test]
    fn test_insert_empty_line_for_title() {
        let fmt_md = format_markdown(
            "# title 1\n## title2\n### title3\n\n```c\n#define ABC\n```\n# title4\n| ---- | ---- |\n# title5",
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
        );
        assert_eq!(
            fmt_md,
            "# title 1\n\ntext\n\n| aaa | bbb |\n| --- | --- |\n| 123 | 456 |\n\nline text\n"
        );
    }

    #[test]
    fn test_insert_space() {
        let fmt_md = format_markdown("# 123你好2谢谢hello`你好call function()`$text谢谢$谢谢");
        assert_eq!(
            fmt_md,
            "# 123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );

        let fmt_md = format_markdown("123你好2谢谢hello`你好call function()`$text谢谢$谢谢");
        assert_eq!(
            fmt_md,
            "123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );

        let fmt_md = format_markdown("- 123你好2谢谢hello`你好call function()`$text谢谢$谢谢");
        assert_eq!(
            fmt_md,
            "- 123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );

        let fmt_md = format_markdown("1. 123你好2谢谢hello`你好call function()`$text谢谢$谢谢");
        assert_eq!(
            fmt_md,
            "1. 123 你好 2 谢谢 hello `你好 call function()` $text 谢谢$谢谢\n"
        );
    }

    #[test]
    fn test_join_empty_lines() {
        let fmt_md = format_markdown("line1\n\n\nline2\n\n  \n  \nline3");
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
        let fmt_md = format_markdown(input);
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
        let fmt_md = format_markdown(input);
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
        assert_eq!(format_markdown(input1), expected1);

        // Test case 2: Ordered list with incorrect numbering
        let input2 = "1. item 1
3. item 2
2. item 3";
        let expected2 = "1. item 1
2. item 2
3. item 3
";
        assert_eq!(format_markdown(input2), expected2);

        // Test case 3: Nested unordered list
        let input3 = "* level 1
  + level 2
    - level 3";
        let expected3 = "- level 1
  - level 2
    - level 3
";
        assert_eq!(format_markdown(input3), expected3);

        // Test case 4: Nested ordered list
        let input4 = "1. level 1
   2. level 2
      3. level 3";
        let expected4 = "1. level 1
  1. level 2
    1. level 3
";
        assert_eq!(format_markdown(input4), expected4);

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
        assert_eq!(format_markdown(input5), expected5);

        // Test case 6: List with intermittent text
        let input6 = "1. item 1

not a list

2. item 2";
        let expected6 = "1. item 1

not a list

1. item 2
";
        assert_eq!(format_markdown(input6), expected6);

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
        assert_eq!(format_markdown(input7), expected7);

        // Test case 8: List with extra spacing
        let input8 = "*   item 1
1.    item 2";
        let expected8 = "- item 1
1. item 2
";
        assert_eq!(format_markdown(input8), expected8);

        // Test case 9: List preceded by a normal line, should insert an empty line
        let input9 = "This is a normal line.
* List item 1
* List item 2";
        let expected9 = "This is a normal line.

- List item 1
- List item 2
";
        assert_eq!(format_markdown(input9), expected9);

        // Test case 10: List preceded by a title, should have an empty line
        let input10 = "# My Title
* List item 1
* List item 2";
        let expected10 = "# My Title

- List item 1
- List item 2
";
        assert_eq!(format_markdown(input10), expected10);
    }

    #[test]
    fn test_blockquote() {
        let input = "text before\n> quote 1\n> quote 2\ntext after";
        let expected = "text before\n\n> quote 1\n> quote 2\n\ntext after\n";
        assert_eq!(format_markdown(input), expected);

        let input2 = "> quote\n# title";
        let expected2 = "> quote\n\n# title\n";
        assert_eq!(format_markdown(input2), expected2);

        // list before quote
        let input3 = "- list item\n> quote";
        let expected3 = "- list item\n\n> quote\n";
        assert_eq!(format_markdown(input3), expected3);

        // quote before list
        let input4 = "> quote\n- list item";
        let expected4 = "> quote\n\n- list item\n";
        assert_eq!(format_markdown(input4), expected4);

        // code block before quote
        let input5 = "```\ncode\n```\n> quote";
        let expected5 = "```\ncode\n```\n\n> quote\n";
        assert_eq!(format_markdown(input5), expected5);

        // quote before code block
        let input6 = "> quote\n```\ncode\n```";
        let expected6 = "> quote\n\n```\ncode\n```\n";
        assert_eq!(format_markdown(input6), expected6);
    }
}
