//! Verify Changes tool - auto-detects project type and runs build+test
//!
//! This is the core of the "agentic coding loop" — after writing code,
//! the agent can verify it compiles and passes tests in one call.

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Detected project type with associated build/test commands
#[derive(Debug, Clone)]
pub struct ProjectType {
    pub name: &'static str,
    pub build_cmd: Option<&'static str>,
    pub test_cmd: Option<&'static str>,
    pub lint_cmd: Option<&'static str>,
    pub indicator_file: &'static str,
}

const PROJECT_TYPES: &[ProjectType] = &[
    ProjectType {
        name: "rust",
        build_cmd: Some("cargo check 2>&1"),
        test_cmd: Some("cargo test 2>&1"),
        lint_cmd: Some("cargo clippy 2>&1"),
        indicator_file: "Cargo.toml",
    },
    ProjectType {
        name: "node_typescript",
        build_cmd: Some("npx tsc --noEmit 2>&1"),
        test_cmd: Some("npm test 2>&1"),
        lint_cmd: Some("npm run lint 2>&1"),
        indicator_file: "tsconfig.json",
    },
    ProjectType {
        name: "node_javascript",
        build_cmd: None,
        test_cmd: Some("npm test 2>&1"),
        lint_cmd: Some("npm run lint 2>&1"),
        indicator_file: "package.json",
    },
    ProjectType {
        name: "python",
        build_cmd: Some("python -m py_compile 2>&1"),
        test_cmd: Some("python -m pytest -x --tb=short 2>&1"),
        lint_cmd: Some("python -m ruff check . 2>&1"),
        indicator_file: "pyproject.toml",
    },
    ProjectType {
        name: "python_legacy",
        build_cmd: None,
        test_cmd: Some("python -m pytest -x --tb=short 2>&1"),
        lint_cmd: None,
        indicator_file: "setup.py",
    },
    ProjectType {
        name: "go",
        build_cmd: Some("go build ./... 2>&1"),
        test_cmd: Some("go test ./... 2>&1"),
        lint_cmd: Some("go vet ./... 2>&1"),
        indicator_file: "go.mod",
    },
];

/// Tool for verifying code changes compile and pass tests
pub struct VerifyChangesTool {
    definition: ToolDefinition,
}

impl VerifyChangesTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "checks".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Which checks to run: 'build' (compile only), 'test' (build + tests), 'full' (build + lint + tests). Default: 'build'.".to_string(),
                default: Some(json!("build")),
                items: None,
                enum_values: Some(vec![
                    "build".to_string(),
                    "test".to_string(),
                    "full".to_string(),
                ]),
            },
        );

        properties.insert(
            "workdir".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Working directory (defaults to workspace). Useful for monorepos.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "custom_command".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Override auto-detected commands with a custom verification command.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        VerifyChangesTool {
            definition: ToolDefinition {
                name: "verify_changes".to_string(),
                description: "Verify code changes by auto-detecting project type and running build/test/lint. Use after editing code to confirm changes compile and tests pass. Returns structured pass/fail with error output.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec![],
                },
                group: ToolGroup::Development,
                hidden: false,
            },
        }
    }

    /// Detect project type from workspace contents
    fn detect_project_type(workdir: &Path) -> Option<&'static ProjectType> {
        for pt in PROJECT_TYPES {
            if workdir.join(pt.indicator_file).exists() {
                return Some(pt);
            }
        }
        None
    }

    /// Run a single command and capture output
    async fn run_command(cmd: &str, workdir: &Path, timeout_secs: u64) -> CommandResult {
        let result = timeout(
            Duration::from_secs(timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout.clone()
                } else if stdout.is_empty() {
                    stderr.clone()
                } else {
                    format!("{}\n{}", stdout, stderr)
                };

                // Truncate to avoid overwhelming the context
                let truncated = if combined.len() > 4000 {
                    let tail = &combined[combined.len() - 3800..];
                    format!("... (truncated, showing last 3800 chars) ...\n{}", tail)
                } else {
                    combined
                };

                CommandResult {
                    success: output.status.success(),
                    exit_code: output.status.code(),
                    output: truncated,
                    timed_out: false,
                }
            }
            Ok(Err(e)) => CommandResult {
                success: false,
                exit_code: None,
                output: format!("Failed to execute command: {}", e),
                timed_out: false,
            },
            Err(_) => CommandResult {
                success: false,
                exit_code: None,
                output: format!("Command timed out after {}s", timeout_secs),
                timed_out: true,
            },
        }
    }

    /// Parse compiler errors to extract actionable file:line info
    fn extract_error_locations(output: &str) -> Vec<String> {
        let mut locations = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();
            // Rust: --> src/main.rs:42:5
            if trimmed.starts_with("-->") {
                if let Some(loc) = trimmed.strip_prefix("--> ") {
                    locations.push(format!("  - {}", loc.trim()));
                }
            }
            // TypeScript/ESLint: src/App.tsx(42,5): error TS...
            // or: src/App.tsx:42:5 - error TS...
            else if (trimmed.contains(": error ") || trimmed.contains(": warning "))
                && (trimmed.contains(".ts") || trimmed.contains(".js") || trimmed.contains(".tsx") || trimmed.contains(".jsx"))
            {
                let short = if trimmed.len() > 120 {
                    format!("{}...", &trimmed[..120])
                } else {
                    trimmed.to_string()
                };
                locations.push(format!("  - {}", short));
            }
            // Python: File "path.py", line 42
            else if trimmed.starts_with("File \"") && trimmed.contains("line ") {
                locations.push(format!("  - {}", trimmed));
            }
            // Go: ./main.go:42:5: ...
            else if trimmed.contains(".go:") && trimmed.contains(": ") {
                let short = if trimmed.len() > 120 {
                    format!("{}...", &trimmed[..120])
                } else {
                    trimmed.to_string()
                };
                locations.push(format!("  - {}", short));
            }
        }

        // Deduplicate and limit
        locations.dedup();
        locations.truncate(15);
        locations
    }
}

impl Default for VerifyChangesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct CommandResult {
    success: bool,
    exit_code: Option<i32>,
    output: String,
    #[allow(dead_code)]
    timed_out: bool,
}

#[derive(Debug, Deserialize)]
struct VerifyChangesParams {
    checks: Option<String>,
    workdir: Option<String>,
    custom_command: Option<String>,
}

#[async_trait]
impl Tool for VerifyChangesTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: VerifyChangesParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let workspace = context
            .workspace_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let workdir = if let Some(ref wd) = params.workdir {
            let wd_path = PathBuf::from(wd);
            if wd_path.is_absolute() {
                wd_path
            } else {
                workspace.join(wd_path)
            }
        } else {
            workspace
        };

        if !workdir.exists() {
            return ToolResult::error(format!("Working directory does not exist: {}", workdir.display()));
        }

        let checks = params.checks.as_deref().unwrap_or("build");

        // If custom command provided, just run it
        if let Some(ref custom_cmd) = params.custom_command {
            let result = Self::run_command(custom_cmd, &workdir, 120).await;
            let status = if result.success { "PASS" } else { "FAIL" };
            let mut output = format!("## Verification: {}\n\nCommand: `{}`\n\n", status, custom_cmd);
            if !result.output.is_empty() {
                output.push_str(&format!("```\n{}\n```", result.output));
            }
            if result.success {
                return ToolResult::success(output);
            } else {
                let locations = Self::extract_error_locations(&result.output);
                if !locations.is_empty() {
                    output.push_str("\n\n**Error locations:**\n");
                    for loc in &locations {
                        output.push_str(&format!("{}\n", loc));
                    }
                    output.push_str("\nUse `read_file` to inspect these locations, then fix the issues.");
                }
                return ToolResult::error(output);
            }
        }

        // Auto-detect project type
        let project = match Self::detect_project_type(&workdir) {
            Some(p) => p,
            None => {
                return ToolResult::error(
                    "Could not auto-detect project type. No Cargo.toml, package.json, pyproject.toml, setup.py, or go.mod found.\n\n\
                     Use `custom_command` parameter to specify your build/test command."
                );
            }
        };

        let mut report = format!("## Verification Report\n**Project type**: {}\n**Checks**: {}\n\n", project.name, checks);
        let mut all_passed = true;
        let mut all_locations = Vec::new();

        // Run build check
        if let Some(build_cmd) = project.build_cmd {
            report.push_str(&format!("### Build: `{}`\n", build_cmd));
            let result = Self::run_command(build_cmd, &workdir, 120).await;
            if result.success {
                report.push_str("**PASS**\n\n");
            } else {
                all_passed = false;
                report.push_str(&format!("**FAIL** (exit code: {:?})\n```\n{}\n```\n\n", result.exit_code, result.output));
                let locs = Self::extract_error_locations(&result.output);
                all_locations.extend(locs);
            }
        }

        // Run lint check (only for 'full')
        if checks == "full" {
            if let Some(lint_cmd) = project.lint_cmd {
                report.push_str(&format!("### Lint: `{}`\n", lint_cmd));
                let result = Self::run_command(lint_cmd, &workdir, 60).await;
                if result.success {
                    report.push_str("**PASS**\n\n");
                } else {
                    all_passed = false;
                    report.push_str(&format!("**FAIL** (exit code: {:?})\n```\n{}\n```\n\n", result.exit_code, result.output));
                    let locs = Self::extract_error_locations(&result.output);
                    all_locations.extend(locs);
                }
            }
        }

        // Run test check (for 'test' and 'full')
        if checks == "test" || checks == "full" {
            if let Some(test_cmd) = project.test_cmd {
                report.push_str(&format!("### Tests: `{}`\n", test_cmd));
                let result = Self::run_command(test_cmd, &workdir, 180).await;
                if result.success {
                    report.push_str("**PASS**\n\n");
                } else {
                    all_passed = false;
                    report.push_str(&format!("**FAIL** (exit code: {:?})\n```\n{}\n```\n\n", result.exit_code, result.output));
                    let locs = Self::extract_error_locations(&result.output);
                    all_locations.extend(locs);
                }
            }
        }

        // Add error location summary
        if !all_locations.is_empty() {
            all_locations.dedup();
            all_locations.truncate(15);
            report.push_str("### Error Locations\n");
            for loc in &all_locations {
                report.push_str(&format!("{}\n", loc));
            }
            report.push_str("\nUse `read_file` to inspect these locations, then fix with `edit_file`.\n");
        }

        if all_passed {
            report.push_str("---\n**All checks passed.**");
            ToolResult::success(report).with_metadata(json!({
                "project_type": project.name,
                "checks": checks,
                "passed": true,
            }))
        } else {
            report.push_str("---\n**Some checks failed.** Fix the issues above and run `verify_changes` again.");
            ToolResult::error(report).with_metadata(json!({
                "project_type": project.name,
                "checks": checks,
                "passed": false,
                "error_count": all_locations.len(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust_project() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        let pt = VerifyChangesTool::detect_project_type(temp.path()).unwrap();
        assert_eq!(pt.name, "rust");
    }

    #[test]
    fn test_detect_node_project() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        let pt = VerifyChangesTool::detect_project_type(temp.path()).unwrap();
        assert_eq!(pt.name, "node_javascript");
    }

    #[test]
    fn test_detect_typescript_project() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        let pt = VerifyChangesTool::detect_project_type(temp.path()).unwrap();
        assert_eq!(pt.name, "node_typescript");
    }

    #[test]
    fn test_detect_no_project() {
        let temp = tempfile::TempDir::new().unwrap();
        let pt = VerifyChangesTool::detect_project_type(temp.path());
        assert!(pt.is_none());
    }

    #[test]
    fn test_extract_rust_error_locations() {
        let output = r#"error[E0308]: mismatched types
 --> src/main.rs:42:5
  |
42 |     let x: u32 = "hello";
  |                  ^^^^^^^ expected `u32`, found `&str`

error[E0425]: cannot find value `foo` in this scope
 --> src/lib.rs:10:12
  |
10 |     return foo;
  |            ^^^ not found in this scope"#;

        let locations = VerifyChangesTool::extract_error_locations(output);
        assert_eq!(locations.len(), 2);
        assert!(locations[0].contains("src/main.rs:42:5"));
        assert!(locations[1].contains("src/lib.rs:10:12"));
    }

    #[test]
    fn test_extract_python_error_locations() {
        let output = r#"Traceback (most recent call last):
  File "app.py", line 42, in main
    result = process(data)
TypeError: 'NoneType' object is not callable"#;

        let locations = VerifyChangesTool::extract_error_locations(output);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].contains("app.py"));
    }
}
