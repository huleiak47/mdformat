use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_file_input_output() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input_file = NamedTempFile::new()?;
    fs::write(input_file.path(), "## title\n\ntext")?;

    let output_file = NamedTempFile::new()?;

    cmd.arg(input_file.path())
        .arg("-o")
        .arg(output_file.path());

    cmd.assert().success();

    let output = fs::read_to_string(output_file.path())?;
    assert_eq!(output, "## title\n\ntext\n");

    Ok(())
}

#[test]
fn test_stdin_stdout() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.write_stdin("## title\n\ntext");
    cmd.assert()
        .success()
        .stdout("## title\n\ntext\n");
    Ok(())
}

#[test]
fn test_non_existent_input_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.arg("non_existent_file.md");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
    Ok(())
}

#[test]
fn test_empty_input() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.write_stdin("");
    cmd.assert().success().stdout("\n");
    Ok(())
}

#[test]
fn test_indent_argument() -> Result<(), Box<dyn std::error::Error>> {
    let _cmd = Command::cargo_bin("mdformat")?;
    let _input = "1. level 1\n  2. level 2";
    let _expected = "1. level 1\n    1. level 2\n"; // Default indent is 4, but the logic seems to be 2 * (level - 1)
    
    // The current implementation seems to have hardcoded indent logic (2 spaces per level).
    // Let's first test the existing behavior.
    let mut cmd_default = Command::cargo_bin("mdformat")?;
    cmd_default.write_stdin("1. level 1\n  2. level 2");
    cmd_default.assert().success().stdout("1. level 1\n  1. level 2\n");


    // If the indent argument were implemented, the test would look like this:
    /*
    cmd.write_stdin(input).arg("-i").arg("4");
    cmd.assert().success().stdout(expected);
    */

    Ok(())
}

#[test]
fn test_mixed_elements() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "# Title\n\nSome text.\n\n- list1\n- list2\n\n```rust\nlet a = 1;\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nAnother line.";
    let expected = "# Title\n\nSome text.\n\n- list1\n- list2\n\n```rust\nlet a = 1;\n```\n\n| a   | b   |\n| --- | --- |\n| 1   | 2   |\n\nAnother line.\n";
    cmd.write_stdin(input);
    cmd.assert().success().stdout(predicate::str::diff(expected));
    Ok(())
}

// ===== Added: CLI argument tests (6 tests) =====
#[test]
fn test_indent_argument_works() -> Result<(), Box<dyn std::error::Error>> {
    // Test --indent argument
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "- level 1\n  - level 2";

    cmd.write_stdin(input)
        .arg("--indent")
        .arg("4");

    cmd.assert()
        .success()
        .stdout("- level 1\n    - level 2\n");

    Ok(())
}

#[test]
fn test_unordered_marker_argument() -> Result<(), Box<dyn std::error::Error>> {
    // Test --unordered-marker argument
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "- item 1\n- item 2";

    cmd.write_stdin(input)
        .arg("--unordered-marker")
        .arg("*");

    cmd.assert()
        .success()
        .stdout("* item 1\n* item 2\n");

    Ok(())
}

#[test]
fn test_heading_numbering_argument() -> Result<(), Box<dyn std::error::Error>> {
    // Test --heading-numbering argument
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "# Title\n## Sub";

    cmd.write_stdin(input)
        .arg("--heading-numbering")
        .arg("1");

    cmd.assert()
        .success()
        .stdout("# 1 Title\n\n## 1.1 Sub\n");

    Ok(())
}

#[test]
fn test_no_format_tables_flag() -> Result<(), Box<dyn std::error::Error>> {
    // Test --no-format-tables flag
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "|a|b|\n|---|---|\n| column 1 | column 2    |";

    cmd.write_stdin(input)
        .arg("--no-format-tables");

    cmd.assert()
        .success()
        .stdout("|a|b|\n|---|---|\n| column 1 | column 2    |\n");

    Ok(())
}

#[test]
fn test_no_cjk_spacing_flag() -> Result<(), Box<dyn std::error::Error>> {
    // Test --no-cjk-spacing flag
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "123你好world";

    cmd.write_stdin(input)
        .arg("--no-cjk-spacing");

    cmd.assert()
        .success()
        .stdout("123你好world\n");

    Ok(())
}

#[test]
fn test_multiple_flags_combined() -> Result<(), Box<dyn std::error::Error>> {
    // Test multiple flags combined
    let mut cmd = Command::cargo_bin("mdformat")?;
    let input = "# Title\n你好world\n- item";

    cmd.write_stdin(input)
        .arg("--heading-numbering")
        .arg("1")
        .arg("--no-cjk-spacing")
        .arg("--unordered-marker")
        .arg("*");

    cmd.assert()
        .success()
        .stdout("# 1 Title\n\n你好world\n\n* item\n");

    Ok(())
}

// ===== Added: Configuration file loading tests (5 tests) =====
#[test]
fn test_init_config_command() -> Result<(), Box<dyn std::error::Error>> {
    // Test --init-config command
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test.toml");

    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.arg("--init-config")
        .arg(&config_path);

    cmd.assert().success();

    // Verify config file is created
    assert!(config_path.exists());

    // Verify config file content
    let content = fs::read_to_string(&config_path)?;
    assert!(content.contains("[formatting]"));
    assert!(content.contains("[lists]"));
    assert!(content.contains("[headings]"));
    assert!(content.contains("indent = 2"));

    Ok(())
}

#[test]
fn test_config_file_loading() -> Result<(), Box<dyn std::error::Error>> {
    // Test config file loading
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join(".mdformat.toml");

    // Create custom config
    fs::write(&config_path, r#"
[lists]
indent = 4
unordered_marker = "*"

[headings]
numbering_start_level = 1
"#)?;

    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(temp_dir.path())
        .write_stdin("# Title\n## Sub\n- item\n  - sub");

    cmd.assert()
        .success()
        .stdout("# 1 Title\n\n## 1.1 Sub\n\n* item\n    * sub\n");

    Ok(())
}

#[test]
fn test_explicit_config_file() -> Result<(), Box<dyn std::error::Error>> {
    // Test --config argument
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("custom.toml");

    fs::write(&config_path, r#"
[lists]
unordered_marker = "+"
"#)?;

    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.arg("--config")
        .arg(&config_path)
        .write_stdin("- item");

    cmd.assert()
        .success()
        .stdout("+ item\n");

    Ok(())
}

#[test]
fn test_cli_overrides_config_file() -> Result<(), Box<dyn std::error::Error>> {
    // Test CLI arguments override config file
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join(".mdformat.toml");

    // Config file sets indent = 4
    fs::write(&config_path, r#"
[lists]
indent = 4
"#)?;

    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(temp_dir.path())
        .arg("--indent")
        .arg("2")  // CLI overrides to 2
        .write_stdin("- item\n  - sub");

    cmd.assert()
        .success()
        .stdout("- item\n  - sub\n");

    Ok(())
}

#[test]
fn test_invalid_config_file() -> Result<(), Box<dyn std::error::Error>> {
    // Test invalid config file
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join(".mdformat.toml");

    // Create incorrectly formatted config
    fs::write(&config_path, "invalid toml content {")?;

    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(temp_dir.path())
        .write_stdin("text");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Config file format error"));

    Ok(())
}

// ===== Added: Configuration file upward search integration tests (6 tests) =====

#[test]
fn test_config_upward_search_from_subdir() -> Result<(), Box<dyn std::error::Error>> {
    // Test upward config search from subdirectory
    let temp_dir = TempDir::new()?;

    // Create config file in root directory
    let config_path = temp_dir.path().join(".mdformat.toml");
    fs::write(
        &config_path,
        r#"
[lists]
unordered_marker = "*"
"#,
    )?;

    // Create subdirectory and input file
    let sub_dir = temp_dir.path().join("docs");
    fs::create_dir(&sub_dir)?;
    let input_file = sub_dir.join("test.md");
    fs::write(&input_file, "- item\n")?;

    // Run mdformat from subdirectory
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(&sub_dir)
        .arg(&input_file)
        .arg("-o")
        .arg(&input_file);

    cmd.assert().success();

    // Verify parent directory's config was used
    let output = fs::read_to_string(&input_file)?;
    assert_eq!(output, "* item\n");

    Ok(())
}

#[test]
fn test_config_upward_search_multiple_levels() -> Result<(), Box<dyn std::error::Error>> {
    // Test upward search through multiple directory levels
    let temp_dir = TempDir::new()?;

    // Create config in root directory
    let config_path = temp_dir.path().join(".mdformat.toml");
    fs::write(
        &config_path,
        r#"
[lists]
indent = 4
"#,
    )?;

    // Create deeply nested directory
    let nested_dir = temp_dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested_dir)?;

    let input_file = nested_dir.join("test.md");
    fs::write(&input_file, "- item\n  - sub\n")?;

    // Run from deeply nested directory
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(&nested_dir)
        .arg(&input_file)
        .arg("-o")
        .arg(&input_file);

    cmd.assert().success();

    // Verify root directory's config was used (4 space indent)
    let output = fs::read_to_string(&input_file)?;
    assert_eq!(output, "- item\n    - sub\n");

    Ok(())
}

#[test]
fn test_config_upward_search_stops_at_first() -> Result<(), Box<dyn std::error::Error>> {
    // Test stopping at first config found
    let temp_dir = TempDir::new()?;

    // Create config in root directory (using - marker)
    let root_config = temp_dir.path().join(".mdformat.toml");
    fs::write(
        &root_config,
        r#"
[lists]
unordered_marker = "-"
"#,
    )?;

    // Create config in subdirectory (using * marker)
    let sub_dir = temp_dir.path().join("docs");
    fs::create_dir(&sub_dir)?;
    let sub_config = sub_dir.join(".mdformat.toml");
    fs::write(
        &sub_config,
        r#"
[lists]
unordered_marker = "*"
"#,
    )?;

    let input_file = sub_dir.join("test.md");
    fs::write(&input_file, "- item\n")?;

    // Run from subdirectory
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(&sub_dir)
        .arg(&input_file)
        .arg("-o")
        .arg(&input_file);

    cmd.assert().success();

    // Should use subdirectory's config (* marker), not root directory's
    let output = fs::read_to_string(&input_file)?;
    assert_eq!(output, "* item\n");

    Ok(())
}

#[test]
fn test_config_explicit_overrides_upward_search() -> Result<(), Box<dyn std::error::Error>> {
    // Test --config parameter overrides upward search
    let temp_dir = TempDir::new()?;

    // Create auto-discovered config in root (using - marker)
    let auto_config = temp_dir.path().join(".mdformat.toml");
    fs::write(
        &auto_config,
        r#"
[lists]
unordered_marker = "-"
"#,
    )?;

    // Create explicitly specified config file (using + marker)
    let explicit_config = temp_dir.path().join("custom.toml");
    fs::write(
        &explicit_config,
        r#"
[lists]
unordered_marker = "+"
"#,
    )?;

    let input_file = temp_dir.path().join("test.md");
    fs::write(&input_file, "- item\n")?;

    // Use --config to explicitly specify config
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg(&explicit_config)
        .arg(&input_file)
        .arg("-o")
        .arg(&input_file);

    cmd.assert().success();

    // Should use explicit config (+ marker), ignore auto-discovered config
    let output = fs::read_to_string(&input_file)?;
    assert_eq!(output, "+ item\n");

    Ok(())
}

#[test]
fn test_config_upward_search_not_found_uses_default() -> Result<(), Box<dyn std::error::Error>> {
    // Test using default config when no config file found
    let temp_dir = TempDir::new()?;

    // Don't create any config file
    let input_file = temp_dir.path().join("test.md");
    fs::write(&input_file, "+ item\n")?;

    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(temp_dir.path())
        .arg(&input_file)
        .arg("-o")
        .arg(&input_file);

    cmd.assert().success();

    // Should use default config (- marker)
    let output = fs::read_to_string(&input_file)?;
    assert_eq!(output, "- item\n");

    Ok(())
}

#[test]
fn test_config_upward_search_with_nested_project() -> Result<(), Box<dyn std::error::Error>> {
    // Test working correctly in a nested project structure
    let temp_dir = TempDir::new()?;

    // Create root project config (using - marker)
    let root_config = temp_dir.path().join(".mdformat.toml");
    fs::write(
        &root_config,
        r#"
[lists]
unordered_marker = "-"
"#,
    )?;

    // Create nested project in src/ directory (using * marker)
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir)?;
    let src_config = src_dir.join(".mdformat.toml");
    fs::write(
        &src_config,
        r#"
[lists]
unordered_marker = "*"
"#,
    )?;

    // Create deeply nested directory under src/
    let nested_dir = src_dir.join("components").join("ui");
    fs::create_dir_all(&nested_dir)?;
    let input_file = nested_dir.join("test.md");
    fs::write(&input_file, "+ item\n")?;

    // Run from deeply nested directory
    let mut cmd = Command::cargo_bin("mdformat")?;
    cmd.current_dir(&nested_dir)
        .arg(&input_file)
        .arg("-o")
        .arg(&input_file);

    cmd.assert().success();

    // Should find and use src/.mdformat.toml (* marker), not root's
    let output = fs::read_to_string(&input_file)?;
    assert_eq!(output, "* item\n");

    Ok(())
}
