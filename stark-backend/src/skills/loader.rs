use crate::skills::types::{Skill, SkillMetadata, SkillSource};
use std::path::Path;

/// Parse a SKILL.md file content into a Skill
pub fn parse_skill_file(content: &str, path: &str, source: SkillSource) -> Result<Skill, String> {
    // SKILL.md format:
    // ---
    // YAML frontmatter
    // ---
    // Prompt template content

    let content = content.trim();

    // Check for frontmatter delimiters
    if !content.starts_with("---") {
        return Err("SKILL.md must start with YAML frontmatter (---)".to_string());
    }

    // Find the end of frontmatter
    let rest = &content[3..]; // Skip first ---
    let end_idx = rest.find("---").ok_or("Missing closing --- for frontmatter")?;

    let frontmatter = rest[..end_idx].trim();
    let prompt_template = rest[end_idx + 3..].trim().to_string();

    // Parse YAML frontmatter
    let metadata: SkillMetadata =
        serde_yaml_parse(frontmatter).map_err(|e| format!("Failed to parse frontmatter: {}", e))?;

    if metadata.name.is_empty() {
        return Err("Skill name is required in frontmatter".to_string());
    }

    if metadata.description.is_empty() {
        return Err("Skill description is required in frontmatter".to_string());
    }

    Ok(Skill {
        metadata,
        prompt_template,
        source,
        path: path.to_string(),
        enabled: true,
    })
}

/// Load a skill from a SKILL.md file path
pub async fn load_skill_from_file(path: &Path, source: SkillSource) -> Result<Skill, String> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    parse_skill_file(&content, &path.to_string_lossy(), source)
}

/// Load all skills from a directory
pub async fn load_skills_from_directory(
    dir: &Path,
    source: SkillSource,
) -> Result<Vec<Skill>, String> {
    let mut skills = Vec::new();

    if !dir.exists() {
        return Ok(skills);
    }

    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| e.to_string())?
    {
        let path = entry.path();

        // Check for .md files (SKILL.md or any *.md files that contain frontmatter)
        if path.is_file() {
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                // Accept SKILL.md or any .md file (e.g., github.md, weather.md)
                if name_str.to_uppercase() == "SKILL.MD" || name_str.ends_with(".md") {
                    match load_skill_from_file(&path, source.clone()).await {
                        Ok(skill) => {
                            log::info!("Loaded skill '{}' from {}", skill.metadata.name, path.display());
                            skills.push(skill);
                        }
                        Err(e) => {
                            // Only warn if it's a SKILL.md file (expected to be a skill)
                            // For other .md files, they may just be README or other docs
                            if name_str.to_uppercase() == "SKILL.MD" {
                                log::warn!("Failed to load skill from {}: {}", path.display(), e);
                            } else {
                                log::debug!("Skipping {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }
        // Check for subdirectories with SKILL.md
        else if path.is_dir() {
            // Skip inactive/disabled directories
            if let Some(dir_name) = path.file_name() {
                let dir_name_str = dir_name.to_string_lossy();
                if dir_name_str == "inactive" || dir_name_str == "disabled" || dir_name_str.starts_with('_') {
                    log::debug!("Skipping inactive skills directory: {}", path.display());
                    continue;
                }
            }

            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                match load_skill_from_file(&skill_file, source.clone()).await {
                    Ok(skill) => {
                        log::info!("Loaded skill '{}' from {}", skill.metadata.name, skill_file.display());
                        skills.push(skill);
                    }
                    Err(e) => {
                        log::warn!("Failed to load skill from {}: {}", skill_file.display(), e);
                    }
                }
            }
        }
    }

    Ok(skills)
}

/// Simple YAML parser for skill metadata
/// This is a minimal implementation that handles the specific YAML format we use
fn serde_yaml_parse(yaml: &str) -> Result<SkillMetadata, String> {
    use std::collections::HashMap;

    let mut metadata = SkillMetadata::default();
    let mut current_key = String::new();
    let mut in_arguments = false;
    let mut in_api_keys = false;
    let mut current_arg_name = String::new();
    let mut current_arg = crate::skills::types::SkillArgument {
        description: String::new(),
        required: false,
        default: None,
    };
    let mut current_api_key_name = String::new();
    let mut current_api_key = crate::skills::types::SkillApiKey {
        description: String::new(),
        secret: true,
    };

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check indentation level
        let indent = line.len() - line.trim_start().len();

        if indent == 0 {
            // Flush pending argument/api_key before switching sections
            if in_arguments && !current_arg_name.is_empty() {
                metadata.arguments.insert(current_arg_name.clone(), current_arg.clone());
                current_arg_name.clear();
            }
            if in_api_keys && !current_api_key_name.is_empty() {
                metadata.requires_api_keys.insert(current_api_key_name.clone(), current_api_key.clone());
                current_api_key_name.clear();
            }

            // Top-level key
            if let Some((key, value)) = trimmed.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                current_key = key.to_string();
                in_arguments = key == "arguments";
                in_api_keys = key == "requires_api_keys";

                match key {
                    "name" => metadata.name = unquote(value),
                    "description" => metadata.description = unquote(value),
                    "version" => metadata.version = unquote(value),
                    "author" => metadata.author = Some(unquote(value)),
                    "homepage" => metadata.homepage = Some(unquote(value)),
                    "metadata" => {
                        // metadata can be a JSON object, preserve it as-is
                        let value_str = unquote(value);
                        if !value_str.is_empty() {
                            metadata.metadata = Some(value_str);
                        }
                    }
                    "requires_tools" => {
                        if value.starts_with('[') {
                            metadata.requires_tools = parse_inline_list(value);
                        }
                    }
                    "requires_binaries" => {
                        if value.starts_with('[') {
                            metadata.requires_binaries = parse_inline_list(value);
                        }
                    }
                    "tags" => {
                        if value.starts_with('[') {
                            metadata.tags = parse_inline_list(value);
                        }
                    }
                    _ => {}
                }
            }
        } else if indent == 2 {
            // Second-level (list items or argument/api_key names)
            if trimmed.starts_with("- ") {
                let value = trimmed[2..].trim();
                match current_key.as_str() {
                    "requires_tools" => metadata.requires_tools.push(unquote(value)),
                    "requires_binaries" => metadata.requires_binaries.push(unquote(value)),
                    "tags" => metadata.tags.push(unquote(value)),
                    _ => {}
                }
            } else if in_arguments {
                // Argument name
                if let Some((arg_name, _)) = trimmed.split_once(':') {
                    if !current_arg_name.is_empty() {
                        metadata
                            .arguments
                            .insert(current_arg_name.clone(), current_arg.clone());
                    }
                    current_arg_name = arg_name.trim().to_string();
                    current_arg = crate::skills::types::SkillArgument {
                        description: String::new(),
                        required: false,
                        default: None,
                    };
                }
            } else if in_api_keys {
                // API key name
                if let Some((key_name, _)) = trimmed.split_once(':') {
                    if !current_api_key_name.is_empty() {
                        metadata.requires_api_keys.insert(current_api_key_name.clone(), current_api_key.clone());
                    }
                    current_api_key_name = key_name.trim().to_string();
                    current_api_key = crate::skills::types::SkillApiKey {
                        description: String::new(),
                        secret: true,
                    };
                }
            }
        } else if indent >= 4 {
            if in_arguments {
                // Argument properties
                if let Some((key, value)) = trimmed.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    match key {
                        "description" => current_arg.description = unquote(value),
                        "required" => current_arg.required = value == "true",
                        "default" => current_arg.default = Some(unquote(value)),
                        _ => {}
                    }
                }
            } else if in_api_keys {
                // API key properties
                if let Some((key, value)) = trimmed.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    match key {
                        "description" => current_api_key.description = unquote(value),
                        "secret" => current_api_key.secret = value != "false",
                        _ => {}
                    }
                }
            }
        }
    }

    // Don't forget the last argument/api_key
    if in_arguments && !current_arg_name.is_empty() {
        metadata.arguments.insert(current_arg_name, current_arg);
    }
    if in_api_keys && !current_api_key_name.is_empty() {
        metadata.requires_api_keys.insert(current_api_key_name, current_api_key);
    }

    Ok(metadata)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_inline_list(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        s[1..s.len() - 1]
            .split(',')
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect()
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_file() {
        let content = r#"---
name: code-review
description: Review code and provide feedback
version: 1.0.0
requires_tools: [read_file, exec]
requires_binaries: [git]
arguments:
  path:
    description: "Path to review"
    default: "."
---
You are a code reviewer. Review the code at {{path}} and provide feedback.
"#;

        let skill = parse_skill_file(content, "/test/SKILL.md", SkillSource::Bundled).unwrap();
        assert_eq!(skill.metadata.name, "code-review");
        assert_eq!(skill.metadata.description, "Review code and provide feedback");
        assert_eq!(skill.metadata.version, "1.0.0");
        assert_eq!(skill.metadata.requires_tools, vec!["read_file", "exec"]);
        assert_eq!(skill.metadata.requires_binaries, vec!["git"]);
        assert!(skill.metadata.arguments.contains_key("path"));
        assert!(skill.prompt_template.contains("You are a code reviewer"));
    }

    #[test]
    fn test_parse_skill_missing_frontmatter() {
        let content = "Just some text without frontmatter";
        let result = parse_skill_file(content, "/test/SKILL.md", SkillSource::Bundled);
        assert!(result.is_err());
    }
}
