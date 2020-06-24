use std::num;
use url::Url;

/// Identifying and authentication info for a Discord webhook
pub struct DiscordWebhookAuth {
    /// Discord webhook id
    pub id: u64,
    /// Discord webhook token
    pub token: String,
}
impl DiscordWebhookAuth {
    /// Constructor
    ///
    /// # Parameters
    /// * `id` - Discord webhook id
    /// * `token` - Discord webhook token
    pub fn new(id: u64, token: String) -> Self {
        Self { id, token }
    }

    /// Parse the relevant fields of out a Discord webhook url
    ///
    /// # Parameters
    /// * `url` - Discord webhook url
    pub fn from_url(url: &str) -> Result<Self, DiscordWebhookAuthUrlError> {
        use DiscordWebhookAuthUrlError::*;
        // Parse the url
        // As of 2020 06 23, the format is
        // https://discord.com/api/webhooks/ID/TOKEN
        let url = Url::parse(url).map_err(UrlParseError)?;
        // Skip schema but you really should be using https
        // Skip hostname since discord may change
        let mut path_segments = url.path_segments().ok_or_else(|| UrlMissingPath)?;
        if path_segments.next() != Some("api") {
            Err(UrlPathMissingApi)
        } else if path_segments.next() != Some("webhooks") {
            Err(UrlPathMissingWebhooks)
        } else {
            if let Some(id) = path_segments.next() {
                let id: u64 = id.parse().map_err(IdParseError)?;
                if let Some(token) = path_segments.next() {
                    Ok(Self::new(id, token.into()))
                } else {
                    Err(UrlPathMissingToken)
                }
            } else {
                Err(UrlPathMissingId)
            }
        }
    }
}

/// Error parsing a URL to get the Discord webhook auth info
#[derive(Debug)]
pub enum DiscordWebhookAuthUrlError {
    /// Failed to parse the URL at all
    UrlParseError(url::ParseError),
    /// Url has no path
    UrlMissingPath,
    /// Url has no /api
    UrlPathMissingApi,
    /// Url has no /api/webhooks
    UrlPathMissingWebhooks,
    /// Url is missing /api/webhooks/ID
    UrlPathMissingId,
    /// Url has an invalid /api/webhooks/ID
    IdParseError(num::ParseIntError),
    /// Url is missing /api/webhooks/ID/TOKEN
    UrlPathMissingToken,
}
