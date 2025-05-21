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
    // string to line vector
    // remove empty lines at the beginning and end
    // and remove spaces at the end of each line
    let lines = text
        .trim()
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>();

    // format all lines
    let new_lines = format_lines(lines);

    let mut ret = new_lines.join("\n");

    // format tables
    ret = format_tables(&ret);

    // end with "\n"
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
    Header,
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
    if line.starts_with("```") {
        return LineState::CodeStart;
    }
    if line.starts_with('#') {
        return LineState::Header;
    }
    if line.starts_with('|') {
        return LineState::Table;
    }
    LineState::Normal
}

fn format_lines(lines: Vec<&str>) -> Vec<String> {
    let mut ret = vec![];
    let mut prev_line_state = LineState::Normal;

    for line in lines.iter() {
        // insert space between CJK and ASCII
        let mut cur_state = get_line_state(&line, prev_line_state.clone());
        debug!("{:?}: {}", cur_state, line);

        match cur_state {
            LineState::Normal => {
                // must be an empty line after a table or code block
                if prev_line_state == LineState::Table || prev_line_state == LineState::CodeEnd {
                    ret.push(String::new());
                }

                // normal line needs to be formated
                ret.push(format_line(line));
            }
            LineState::CodeStart => {
                // must be an empty line before a code block
                if prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
                ret.push(line.to_string());
            }
            LineState::Code | LineState::CodeEnd => {
                ret.push(line.to_string());
            }
            LineState::Table => {
                // must be an empty line before a table
                if prev_line_state != LineState::Table && prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }

                // table line needs to be formated
                ret.push(format_line(line));
            }
            LineState::Empty => {
                // merge consecutive empty lines
                if prev_line_state != LineState::Empty {
                    ret.push(String::new());
                }
            }
            LineState::Header => {
                // header line needs to be formated
                ret.push(format_line(line));
                // must be an empty line after a header
                ret.push(String::new());
                cur_state = LineState::Empty;
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

lazy_static! {
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
            "\n| a        | b        |\n| -------- | -------- |\n| column 1 | column 2 |\n"
        );
    }

    #[test]
    fn test_insert_empty_line_for_title() {
        let fmt_md = format_markdown("# title 1\n## title2\n### title3");
        assert_eq!(fmt_md, "# title 1\n\n## title2\n\n### title3\n");
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
}
