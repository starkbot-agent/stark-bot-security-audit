//! Gmail PubSub Integration
//!
//! This module provides integration with Gmail via Google Cloud Pub/Sub.
//!
//! ## Architecture
//! Gmail Watch → Pub/Sub Push → Webhook Endpoint → Agent Processing
//!
//! ## Setup Requirements
//! 1. Google Cloud Project with Gmail API and Pub/Sub API enabled
//! 2. OAuth2 credentials for Gmail access
//! 3. Pub/Sub topic with gmail-api-push@system.gserviceaccount.com as publisher
//! 4. Watch set up via Gmail API to monitor specific labels

mod client;
mod types;

pub use client::GmailClient;
pub use types::*;
