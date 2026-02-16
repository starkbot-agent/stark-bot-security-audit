//! Content-aware token estimation for context management
//!
//! Provides more accurate token estimation than simple character counting
//! by considering content type (JSON, code, prose) and message role.

use crate::models::session_message::MessageRole;

/// Token estimator strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenEstimator {
    /// Simple heuristic: chars / 3.5
    Heuristic,
    /// Content-aware estimation based on text type
    ContentAware,
}

impl Default for TokenEstimator {
    fn default() -> Self {
        TokenEstimator::ContentAware
    }
}

impl TokenEstimator {
    /// Estimate tokens for a message with role context
    pub fn estimate_message(&self, content: &str, role: &MessageRole) -> i32 {
        match self {
            TokenEstimator::Heuristic => heuristic_estimate(content),
            TokenEstimator::ContentAware => content_aware_estimate(content, role),
        }
    }

    /// Estimate tokens for raw text (no role context)
    pub fn estimate_text(&self, text: &str) -> i32 {
        match self {
            TokenEstimator::Heuristic => heuristic_estimate(text),
            TokenEstimator::ContentAware => content_aware_text_estimate(text),
        }
    }
}

/// Simple heuristic: ~3.5 characters per token for English text
fn heuristic_estimate(text: &str) -> i32 {
    let chars = text.chars().count();
    ((chars as f64) / 3.5).ceil() as i32
}

/// Content-aware estimation considering text type
fn content_aware_text_estimate(text: &str) -> i32 {
    let chars = text.chars().count();
    if chars == 0 {
        return 0;
    }

    // Determine content type and appropriate multiplier
    let multiplier = if is_json_content(text) {
        2.5  // JSON has more tokens per char (punctuation, short keys)
    } else if is_code_content(text) {
        3.0  // Code has symbols, keywords, indentation
    } else {
        3.5  // Standard prose
    };

    ((chars as f64) / multiplier).ceil() as i32
}

/// Content-aware estimation with role overhead
fn content_aware_estimate(text: &str, role: &MessageRole) -> i32 {
    let base = content_aware_text_estimate(text);

    // Role overhead (message framing tokens)
    let overhead = match role {
        MessageRole::ToolCall | MessageRole::ToolResult => 8,  // More structured
        MessageRole::System => 6,   // System messages have role prefix
        MessageRole::User | MessageRole::Assistant => 4,  // Basic role prefix
    };

    base + overhead
}

/// Check if text appears to be JSON content
fn is_json_content(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Quick checks for JSON indicators
    let starts_json = trimmed.starts_with('{') || trimmed.starts_with('[');
    let ends_json = trimmed.ends_with('}') || trimmed.ends_with(']');

    if starts_json && ends_json {
        return true;
    }

    // Check for high density of JSON-like characters
    let json_chars = text.chars().filter(|c| matches!(c, '{' | '}' | '[' | ']' | ':' | '"')).count();
    let total_chars = text.chars().count();

    if total_chars > 20 {
        // If more than 10% JSON-like characters, probably JSON
        (json_chars as f64 / total_chars as f64) > 0.10
    } else {
        false
    }
}

/// Check if text appears to be code content
fn is_code_content(text: &str) -> bool {
    // Check for code block markers
    if text.contains("```") {
        return true;
    }

    // Check for common code patterns
    let code_indicators = [
        "fn ", "def ", "function ", "class ", "impl ", "pub ", "const ",
        "let ", "var ", "import ", "from ", "require(", "async ", "await ",
        "return ", "if (", "for (", "while (", "match ", "struct ", "enum ",
        "interface ", "type ", "export ", "#include", "#define", "package ",
    ];

    for indicator in code_indicators {
        if text.contains(indicator) {
            return true;
        }
    }

    // Check for high density of programming characters
    let code_chars = text.chars().filter(|c| matches!(c, '{' | '}' | '(' | ')' | ';' | '=' | '<' | '>')).count();
    let total_chars = text.chars().count();

    if total_chars > 50 {
        // If more than 5% code-like characters, probably code
        (code_chars as f64 / total_chars as f64) > 0.05
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_estimate() {
        assert_eq!(heuristic_estimate("hello"), 2); // 5 chars / 3.5 = 1.43 -> 2
        assert_eq!(heuristic_estimate(""), 0);
    }

    #[test]
    fn test_json_detection() {
        assert!(is_json_content(r#"{"key": "value"}"#));
        assert!(is_json_content(r#"[1, 2, 3]"#));
        assert!(is_json_content(r#"  { "nested": { "data": true } }  "#));
        assert!(!is_json_content("Hello, world!"));
        assert!(!is_json_content(""));
    }

    #[test]
    fn test_code_detection() {
        assert!(is_code_content("fn main() { println!(\"hello\"); }"));
        assert!(is_code_content("```rust\nlet x = 5;\n```"));
        assert!(is_code_content("def hello():\n    return 'hi'"));
        assert!(!is_code_content("Hello, this is a normal sentence."));
        assert!(!is_code_content(""));
    }

    #[test]
    fn test_content_aware_estimate() {
        // JSON should have lower multiplier (more tokens per char)
        let json = r#"{"key": "value", "nested": {"a": 1, "b": 2}}"#;
        let prose = "The quick brown fox jumps over the lazy dog.";

        let json_tokens = content_aware_text_estimate(json);
        let prose_tokens = content_aware_text_estimate(prose);

        // JSON with same length should have more tokens
        // (using ratio check since exact values depend on implementation)
        let json_ratio = json.len() as f64 / json_tokens as f64;
        let prose_ratio = prose.len() as f64 / prose_tokens as f64;

        assert!(json_ratio < prose_ratio, "JSON should have higher token density");
    }

    #[test]
    fn test_role_overhead() {
        let text = "Hello";
        let base = content_aware_text_estimate(text);

        let user_estimate = content_aware_estimate(text, &MessageRole::User);
        let tool_estimate = content_aware_estimate(text, &MessageRole::ToolCall);

        assert_eq!(user_estimate, base + 4);
        assert_eq!(tool_estimate, base + 8);
    }
}
