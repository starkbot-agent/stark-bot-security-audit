/// Fields required to build a SIWA (Sign In With Agent) plaintext message.
///
/// When `agent_id` and `agent_registry` are `None`, produces a standard SIWE
/// message (no Agent ID / Agent Registry lines). This allows plain SIWE auth
/// against servers that don't require ERC-8004 agent identity.
pub struct SiwaMessageFields {
    pub domain: String,
    pub address: String,
    pub uri: String,
    pub agent_id: Option<String>,
    pub agent_registry: Option<String>,
    pub chain_id: u64,
    pub nonce: String,
    pub issued_at: String,
    pub expiration_time: String,
    pub statement: Option<String>,
}

/// Build a SIWA/SIWE plaintext message in the canonical format.
///
/// When `agent_id` and `agent_registry` are present, produces a full SIWA
/// message with "Agent account" header. Otherwise produces a standard SIWE
/// message with "Ethereum account" header.
///
/// The resulting string is intended to be signed with EIP-191 personal_sign.
pub fn build_siwa_message(f: &SiwaMessageFields) -> String {
    let has_agent = f.agent_id.is_some() && f.agent_registry.is_some();

    let account_type = if has_agent { "Agent" } else { "Ethereum" };

    let mut msg = format!(
        "{domain} wants you to sign in with your {account_type} account:\n\
         {address}",
        domain = f.domain,
        account_type = account_type,
        address = f.address,
    );

    // Optional statement block (blank line before and after)
    if let Some(ref stmt) = f.statement {
        msg.push_str(&format!("\n\n{}", stmt));
    }

    msg.push_str(&format!(
        "\n\n\
         URI: {uri}\n\
         Version: 1",
        uri = f.uri,
    ));

    // Agent fields (only in SIWA mode)
    if let (Some(agent_id), Some(agent_registry)) =
        (&f.agent_id, &f.agent_registry)
    {
        msg.push_str(&format!(
            "\n\
             Agent ID: {agent_id}\n\
             Agent Registry: {agent_registry}",
        ));
    }

    msg.push_str(&format!(
        "\n\
         Chain ID: {chain_id}\n\
         Nonce: {nonce}\n\
         Issued At: {issued_at}\n\
         Expiration Time: {expiration_time}",
        chain_id = f.chain_id,
        nonce = f.nonce,
        issued_at = f.issued_at,
        expiration_time = f.expiration_time,
    ));

    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_siwa_message_without_statement() {
        let fields = SiwaMessageFields {
            domain: "example.com".to_string(),
            address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            uri: "https://example.com".to_string(),
            agent_id: Some("42".to_string()),
            agent_registry: Some("0xRegistryAddress".to_string()),
            chain_id: 8453,
            nonce: "abc123".to_string(),
            issued_at: "2025-01-01T00:00:00Z".to_string(),
            expiration_time: "2025-01-01T01:00:00Z".to_string(),
            statement: None,
        };

        let msg = build_siwa_message(&fields);

        let expected = "\
example.com wants you to sign in with your Agent account:
0x1234567890abcdef1234567890abcdef12345678

URI: https://example.com
Version: 1
Agent ID: 42
Agent Registry: 0xRegistryAddress
Chain ID: 8453
Nonce: abc123
Issued At: 2025-01-01T00:00:00Z
Expiration Time: 2025-01-01T01:00:00Z";

        assert_eq!(msg, expected);
    }

    #[test]
    fn test_build_siwa_message_with_statement() {
        let fields = SiwaMessageFields {
            domain: "app.example.com".to_string(),
            address: "0xABCDEF".to_string(),
            uri: "https://app.example.com/auth".to_string(),
            agent_id: Some("7".to_string()),
            agent_registry: Some("0xReg".to_string()),
            chain_id: 1,
            nonce: "nonce456".to_string(),
            issued_at: "2025-06-15T12:00:00Z".to_string(),
            expiration_time: "2025-06-15T13:00:00Z".to_string(),
            statement: Some("I accept the Terms of Service.".to_string()),
        };

        let msg = build_siwa_message(&fields);

        assert!(msg.starts_with("app.example.com wants you to sign in with your Agent account:\n0xABCDEF"));
        assert!(msg.contains("\n\nI accept the Terms of Service.\n\n"));
        assert!(msg.contains("URI: https://app.example.com/auth\n"));
        assert!(msg.contains("Version: 1\n"));
        assert!(msg.contains("Agent ID: 7\n"));
        assert!(msg.contains("Chain ID: 1\n"));
        assert!(msg.contains("Nonce: nonce456\n"));
        assert!(msg.contains("Issued At: 2025-06-15T12:00:00Z\n"));
        assert!(msg.contains("Expiration Time: 2025-06-15T13:00:00Z"));
    }

    #[test]
    fn test_build_siwe_message_no_agent_identity() {
        let fields = SiwaMessageFields {
            domain: "hub.starkbot.ai".to_string(),
            address: "0xb3367C9a01d97f47d00932D93453B9b5c29929a0".to_string(),
            uri: "https://hub.starkbot.ai".to_string(),
            agent_id: None,
            agent_registry: None,
            chain_id: 8453,
            nonce: "abc123".to_string(),
            issued_at: "2025-01-01T00:00:00Z".to_string(),
            expiration_time: "2025-01-01T01:00:00Z".to_string(),
            statement: Some("Sign in to StarkHub".to_string()),
        };

        let msg = build_siwa_message(&fields);

        // Should say "Ethereum account", not "Agent account"
        assert!(msg.starts_with("hub.starkbot.ai wants you to sign in with your Ethereum account:"));
        // Should NOT contain agent fields
        assert!(!msg.contains("Agent ID:"));
        assert!(!msg.contains("Agent Registry:"));
        // Should still contain standard SIWE fields
        assert!(msg.contains("Version: 1\n"));
        assert!(msg.contains("Chain ID: 8453\n"));
        assert!(msg.contains("Nonce: abc123\n"));
        assert!(msg.contains("Sign in to StarkHub"));
    }
}
