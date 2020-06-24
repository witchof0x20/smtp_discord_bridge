// Copyright 2020 Jade
// This file is part of smtp_discord_bridge.
//
// smtp_discord_bridge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// smtp_discord_bridge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with smtp_discord_bridge.  If not, see <https://www.gnu.org/licenses/>.

use crate::discord::{DiscordWebhookAuth, DiscordWebhookAuthUrlError};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use url::Url;

/// Overall config file
#[derive(Debug, Deserialize)]
pub struct Config {
    /// SMTP section. Used to configure the SMTP server
    pub smtp: SmtpConfig,
    /// Discord section. Used to configure the Discord webhook
    pub discord: DiscordConfig,
}

/// SMTP section. Used to configure the SMTP server
#[derive(Debug, Deserialize)]
pub struct SmtpConfig {
    /// IP address to listen on
    listen_addr: IpAddr,
    /// Port to listen on
    listen_port: u16,
    /// Server name
    /// Returned to the SMTP client
    pub service_name: Option<String>,
}
impl Into<SocketAddr> for &SmtpConfig {
    fn into(self) -> SocketAddr {
        SocketAddr::new(self.listen_addr, self.listen_port)
    }
}
impl Into<SocketAddr> for SmtpConfig {
    fn into(self) -> SocketAddr {
        (&self).into()
    }
}

/// Discord section. Used to configure the Discord webhook
#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    webhook_url: Option<String>,
    webhook_id: Option<u64>,
    webhook_token: Option<String>,
}

impl DiscordConfig {
    pub fn get_auth(&self) -> Result<DiscordWebhookAuth, DiscordConfigError> {
        use DiscordConfigError::*;
        match (&self.webhook_url, self.webhook_id, &self.webhook_token) {
            (None, None, None) => Err(NeitherUrlNorPartsSpecified),
            (None, None, Some(_)) => Err(ConfigMissingWebhookId),
            (None, Some(_), None) => Err(ConfigMissingWebhookToken),
            (None, Some(id), Some(token)) => Ok(DiscordWebhookAuth::new(id, token.into())),
            (Some(url), None, None) => DiscordWebhookAuth::from_url(url).map_err(UrlError),
            (Some(_), Some(_), None) | (Some(_), None, Some(_)) | (Some(_), Some(_), Some(_)) => {
                Err(InvalidParamCombination)
            }
        }
    }
}
#[derive(Debug)]
pub enum DiscordConfigError {
    NeitherUrlNorPartsSpecified,
    ConfigMissingWebhookId,
    ConfigMissingWebhookToken,
    InvalidParamCombination,
    UrlError(DiscordWebhookAuthUrlError),
}
