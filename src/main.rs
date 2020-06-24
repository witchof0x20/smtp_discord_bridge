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

use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use samotop::model::command::SmtpMail;
use samotop::model::mail::Envelope;
use samotop::service::session::StatefulSessionService;
use samotop::service::tcp::SamotopService;
use serenity::builder::ExecuteWebhook;
use serenity::model::channel::Embed;
use smtp_discord_bridge::config::Config;
use smtp_discord_bridge::smtp::wrap_mailer_service;
use smtp_discord_bridge::{DiscordMailerBuilder, MailToDiscord};
use std::fs;
use std::net::SocketAddr;

#[derive(Clone)]
struct MMTD;

impl MailToDiscord for MMTD {
    fn handle(&mut self, envelope: Envelope, body: Vec<u8>, webhook_builder: &mut ExecuteWebhook) {
        use SmtpMail::*;
        let sender = match envelope.mail.unwrap() {
            Mail(p) => p,
            Send(p) => p,
            Saml(p) => p,
            Soml(p) => p,
        };
        let rcpt = envelope.rcpts.get(0).unwrap();
        let embed = Embed::fake(|e| {
            e.title("New Message")
                .field("From", sender.to_string(), true)
                .field("To", rcpt.to_string(), true)
                .field("Body", String::from_utf8(body).unwrap(), false)
        });
        webhook_builder.embeds(vec![embed]);
    }
}

/// Configuration path
const ARG_CONFIG_PATH: &str = "config_path";

fn main() {
    // Initialize a logger
    env_logger::init();

    // Parse command line arguments
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name(ARG_CONFIG_PATH)
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Path to the config file.")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    // Get the path to the config file
    let config_path = matches
        .value_of(ARG_CONFIG_PATH)
        .expect("Missing config file");

    // Read the config file as bytes
    let config_contents = fs::read(config_path).expect("Failed to read config file");
    // Parse the config file
    let config: Config = toml::from_slice(&config_contents).expect("Failed to parse config file");

    // Get the listen address
    let listen_addr: SocketAddr = (&config.smtp).into();

    // Get the Discord webhook id and token
    let discord_webhook_auth = config
        .discord
        .get_auth()
        .expect("Failed to get Discord auth from config");

    // Build a Discord-based mailer
    let mailer_builder = DiscordMailerBuilder::new();
    // Add name if specified in the config
    let mailer_builder = if let Some(name) = config.smtp.service_name {
        mailer_builder.with_name(&name)
    } else {
        mailer_builder
    };
    // Build mailer
    let mailer = mailer_builder
        .build(&discord_webhook_auth, MMTD)
        .expect("Failed to create Discord mailer");
    // Wrap the mailer service
    let smtp_service = wrap_mailer_service(mailer).on(listen_addr);
    // Run the service
    tokio::run(smtp_service.build_task());
}
