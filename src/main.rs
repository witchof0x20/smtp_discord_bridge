// Copyright 2020 Jade
// This file is part of smtp_discord_bridge.

// smtp_discord_bridge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// smtp_discord_bridge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with smtp_discord_bridge.  If not, see <https://www.gnu.org/licenses/>.

use bytes::Bytes;
use futures::future::FutureResult;
use futures::sink::Sink;
use futures::{Async, AsyncSink, Poll, StartSend};
use samotop::model::command::{SmtpMail, SmtpPath};
use samotop::model::controll::{TlsConfig, TlsIdFile, TlsMode};
use samotop::model::mail::{AcceptRecipientRequest, AcceptRecipientResult, Envelope, QueueResult};
use samotop::service::session::StatefulSessionService;
use samotop::service::tcp::SamotopService;
use samotop::service::{Mail, MailGuard, MailQueue, NamedService};
use serenity::model::channel::Embed;
use std::io;
use std::sync::mpsc;
use std::thread;

/// Commands sent from the mailer to the dedicated Discord webhook thread
enum DiscordWebhookCommand {
    /// Send a message
    SendMessage {
        from: String,
        recipients: Vec<String>,
        body: String,
    },
    /// Kill the webhook thread
    Shutdown,
}

impl DiscordWebhookCommand {
    /// Constructor for the send message command
    ///
    /// # Parameters
    /// * `from` - The message's sender
    /// * `recipients` - The message's recipients
    /// * `body` - The message's body
    #[inline]
    fn send_message(from: String, recipients: Vec<String>, body: String) -> Self {
        Self::SendMessage {
            from,
            recipients,
            body,
        }
    }

    #[inline]
    fn shutdown() -> Self {
        Self::Shutdown
    }
}

/// Custom mail handler that sends messages to Discord via a webhook
#[derive(Clone)]
struct DiscordMailer {
    /// Used to send commands to the webhook thread
    sender: mpsc::Sender<DiscordWebhookCommand>,
}

impl DiscordMailer {
    /// Constructor
    ///
    /// # Parameter
    /// * `webhook_id` - ID of the Discord webhook
    /// * `webhook_token` - Token of the Discord webhook
    pub fn new(webhook_id: u64, webhook_token: String) -> Result<Self, serenity::Error> {
        // Create the Discord http client
        let http = serenity::http::client::Http::new_with_token("");
        // Get a reference to the webhook
        let webhook = http
            .as_ref()
            .get_webhook_with_token(webhook_id, &webhook_token)?;
        // Initialize the webhook thread
        let sender = Self::init_webhook_thread(http, webhook);
        // Store the command sender in
        Ok(Self { sender })
    }
    /// Initializes the thread that sends Discord messages
    ///
    /// # Parameter
    /// * `http` - Serenity HTTP client
    /// * `webhook` - Discord webhook handle
    fn init_webhook_thread(
        http: serenity::http::client::Http,
        webhook: serenity::model::webhook::Webhook,
    ) -> mpsc::Sender<DiscordWebhookCommand> {
        // Create the communication channel
        let (sender, receiver) = mpsc::channel();

        // Start the thread
        thread::spawn(move || {
            // Iterate over commands received
            for command in receiver.iter() {
                use DiscordWebhookCommand::*;
                match command {
                    SendMessage {
                        from,
                        recipients,
                        body,
                    } => {
                        // Create the fake embed
                        let embed = Embed::fake(|e| {
                            e.title("New Email")
                                .field("From", from, true)
                                .field("To", recipients.join("\n"), true)
                                .field("Body", body, false)
                        });
                        webhook
                            .execute(&http, true, |w| w.embeds(vec![embed]))
                            .unwrap();
                    }
                    Shutdown => break,
                }
            }
        });

        sender
    }
}

impl NamedService for DiscordMailer {
    fn name(&self) -> String {
        "discord".into()
    }
}

impl MailGuard for DiscordMailer {
    type Future = FutureResult<AcceptRecipientResult, io::Error>;

    fn accept(&self, request: AcceptRecipientRequest) -> Self::Future {
        // Accept the recipient as given
        Ok(AcceptRecipientResult::Accepted(request.rcpt)).into()
    }
}

impl MailQueue for DiscordMailer {
    type Mail = DiscordMailSink;
    type MailFuture = FutureResult<Option<Self::Mail>, io::Error>;

    fn mail(&self, envelope: Envelope) -> Self::MailFuture {
        // Get email sender
        // TODO: change unwrap to return error
        use SmtpMail::*;
        let sender = match envelope.mail.unwrap() {
            Mail(p) => p,
            Send(p) => p,
            Saml(p) => p,
            Soml(p) => p,
        };
        // Queue a new piece of mail with the given id
        Ok(Some(DiscordMailSink::new(
            envelope.id,
            sender,
            envelope.rcpts,
            self.sender.clone(),
        )))
        .into()
    }
}

/// Reads mail data over SMTP and sends it over Discord
struct DiscordMailSink {
    /// ID given for the message
    id: String,
    /// The message's sender
    from: SmtpPath,
    /// The message's recipients
    recipients: Vec<SmtpPath>,
    /// Buffer used to store the message body
    body: Vec<u8>,
    /// MPSC sender used to send the message to the Discord sink
    sender: mpsc::Sender<DiscordWebhookCommand>,
}

impl DiscordMailSink {
    /// Constructor
    ///
    /// # Parameters
    /// * `id` - ID of the SMTP message
    /// * `from` - The message's sender
    /// * `recipients` - The message's recipients
    /// * `sender` - MPSC sender used to send the message to the discord sink
    fn new(
        id: String,
        from: SmtpPath,
        recipients: Vec<SmtpPath>,
        sender: mpsc::Sender<DiscordWebhookCommand>,
    ) -> Self {
        Self {
            id,
            from,
            recipients,
            body: Vec::new(),
            sender,
        }
    }
}

impl Mail for DiscordMailSink {
    /// Sends the message to the Discord sink queue
    fn queue(self) -> QueueResult {
        // Parse the binary body as UTF-8
        let body = match String::from_utf8(self.body) {
            Ok(body) => body,
            Err(_) => return QueueResult::Failed,
        };
        // Construct a command for the Discord sink
        let command = DiscordWebhookCommand::send_message(
            self.from.to_string(),
            self.recipients.iter().map(SmtpPath::to_string).collect(),
            body,
        );
        // Return a result based on the result of the send operation
        // TODO: maybe have a receiver that detects whether there was a failure sending to the
        // Discord webhook
        match self.sender.send(command) {
            Ok(()) => QueueResult::QueuedWithId(self.id),
            Err(_) => QueueResult::Failed,
        }
    }
}

impl Sink for DiscordMailSink {
    type SinkItem = Bytes;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        // Consume the email bytes
        self.body.extend_from_slice(&item);
        // Return that the sink is ready for more
        Ok(AsyncSink::Ready)
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

fn main() {
    // Initialize a logger
    env_logger::init();
    // TODO: read command line arguments / config file
    // Initialize a discord-based mailer
    let custom_mail_svc = DiscordMailer::new(
        725087949108019290,
        "aMvaYAhvPZ5kmi0c1abZeuLZAc5MGM_1x-ZYo8OtsRB3heR9D89dSxwkN0UscGDwXYdy".into(),
    )
    .expect("Failed to create Discord mailer");

    // Wrap this in a stateful SMTP session
    let custom_session_svc = StatefulSessionService::new(custom_mail_svc);
    // Don't use TLS
    // TODO: allow the option for TLS
    let tls_conf = TlsConfig {
        mode: TlsMode::Disabled,
        id: TlsIdFile {
            file: "notafile.bin".into(),
            password: None,
        },
    };
    // Wrap the stateful SMTP session in a TCP service
    let custom_svc = SamotopService::new(custom_session_svc, tls_conf);
    // Build the
    let smtp_service = samotop::builder().with(custom_svc).on("localhost:2500");
    let smtp_task = smtp_service.build_task();
    tokio::run(smtp_task);
}
