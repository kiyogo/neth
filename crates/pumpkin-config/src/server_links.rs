use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for server-related links.
///
/// Controls default URLs for bug reports, support, community, and other resources,
/// as well as allowing custom links.
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ServerLinksConfig {
    /// Whether server links are enabled.
    pub enabled: bool,
    /// URL for reporting bugs.
    pub bug_report: String,
    /// URL for support resources.
    pub support: String,
    /// URL for server status.
    pub status: String,
    /// URL for player feedback.
    pub feedback: String,
    /// URL for the community page.
    pub community: String,
    /// URL for the official website.
    pub website: String,
    /// URL for forums.
    pub forums: String,
    /// URL for news updates.
    pub news: String,
    /// URL for announcements.
    pub announcements: String,
    /// Custom key-value links.
    pub custom: HashMap<String, String>,
}

impl Default for ServerLinksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bug_report: "https://kiyomc.com".to_string(),
            support: "https://kiyomc.com".to_string(),
            status: "https://kiyomc.com".to_string(),
            feedback: "https://kiyomc.com".to_string(),
            community: "https://kiyomc.com".to_string(),
            website: "https://kiyomc.com".to_string(),
            forums: "https://kiyomc.com".to_string(),
            news: "https://kiyomc.com".to_string(),
            announcements: "https://kiyomc.com".to_string(),
            custom: HashMap::default(),
        }
    }
}
