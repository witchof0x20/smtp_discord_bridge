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

pub mod config;
pub mod discord;
pub mod smtp;

use crate::discord::DiscordWebhookAuth;
use bytes::Bytes;
use futures::future::{self, FutureResult};
use futures::sink::Sink;
use futures::{Async, AsyncSink, Poll, StartSend};
use samotop::model::mail::{AcceptRecipientRequest, AcceptRecipientResult, Envelope, QueueResult};
use samotop::service::{Mail, MailGuard, MailQueue, NamedService};
use serenity::builder::ExecuteWebhook;
use serenity::model::channel::Message;
use serenity::model::webhook::Webhook;
use std::io;
use std::sync::{Arc, Mutex};

/// This trait defines the conversion between received mail and discord webhook messages
pub trait MailToDiscord {
    /// This function handles an incoming mail, and performs actions on a discord webhook messages
    ///
    /// # Parameters
    /// * `envelope` - contains information such as sender, recipients, IP addresses, and SMTP
    /// handshake information
    /// * `body` - contains the binary body of the mail
    /// * `webhook_builder` - Serenity `ExecuteWebhook` that allows for controlling the content of
    /// a webhook message
    fn handle(&mut self, envelope: Envelope, body: Vec<u8>, webhook_builder: &mut ExecuteWebhook);
}

/// Custom mail handler that sends messages to Discord via a webhook
#[derive(Clone)]
pub struct DiscordMailer<T> {
    /// SMTP service name
    name: String,
    /// Stores webhook connector and message handler
    webhook_sender: Arc<Mutex<WebhookSender<T>>>,
}

impl<T> DiscordMailer<T>
where
    T: Clone + MailToDiscord,
{
    /// Constructor
    ///
    /// # Parameter
    /// * `name` - SMTP service name
    /// * `webhook_auth` - Discord webhook id and auth info
    /// * `handler` - Object used to generate messages from email
    pub fn new(
        name: &str,
        webhook_auth: &DiscordWebhookAuth,
        handler: T,
    ) -> Result<Self, serenity::Error> {
        // Create the webhook sender
        let webhook_sender = WebhookSender::new(webhook_auth, handler)?;

        Ok(Self {
            name: name.into(),
            webhook_sender: Arc::new(Mutex::new(webhook_sender)),
        })
    }
}

impl<T> NamedService for DiscordMailer<T>
where
    T: MailToDiscord,
{
    /// Returns the service name
    fn name(&self) -> String {
        self.name.clone()
    }
}

impl<T> MailGuard for DiscordMailer<T> {
    /// The future type returned by the accept handler
    type Future = FutureResult<AcceptRecipientResult, io::Error>;

    /// Determines whether we should reject the mail
    ///
    /// # Parameters
    /// * `request` - request to send mail containing information such as sender, recipient, and IP
    /// addresses
    fn accept(&self, request: AcceptRecipientRequest) -> Self::Future {
        // Accept the recipient as given
        future::ok(AcceptRecipientResult::Accepted(request.rcpt))
    }
}

impl<T> MailQueue for DiscordMailer<T> {
    /// The sink used to
    type Mail = DiscordMailSink<T>;
    type MailFuture = FutureResult<Option<Self::Mail>, io::Error>;

    /// Begins queueing a piece of mail
    ///
    /// # Parameters
    /// `envelope` - the message's envelope
    fn mail(&self, envelope: Envelope) -> Self::MailFuture {
        // Queue a new piece of mail with the given id
        future::ok(Some(Self::Mail::new(envelope, self.webhook_sender.clone())))
    }
}

/// Sends a message using a webhook
struct WebhookSender<T> {
    /// Serenity HTTP client
    http: serenity::http::client::Http,
    /// Discord webhook handle
    webhook: Webhook,
    /// Object that can convert emails to discord webhook messages
    /// Mutexed because the function that does this takes a mutable reference to itself
    handler: T,
}

impl<T> WebhookSender<T>
where
    T: MailToDiscord,
{
    /// Constructor
    ///
    /// # Parameters
    /// * `webhook_auth` - Discord webhook id and auth info
    /// * `handler` - Object that converts mail to Discord webhook messages
    fn new(webhook_auth: &DiscordWebhookAuth, handler: T) -> Result<Self, serenity::Error> {
        // Create the Discord http client
        let http = serenity::http::client::Http::new_with_token("");
        // Get a reference to the webhook
        let webhook = http
            .as_ref()
            .get_webhook_with_token(webhook_auth.id, &webhook_auth.token)?;

        Ok(Self {
            http,
            webhook,
            handler,
        })
    }

    /// Sends a message based on a given envelope and body
    ///
    /// # Parameters
    /// * `envelope`
    /// * `body`
    fn send_messsage(
        &mut self,
        envelope: Envelope,
        body: Vec<u8>,
    ) -> Result<Option<Message>, serenity::Error> {
        // Get a mutable reference to the handler so we don't double borrow self
        let handler = &mut self.handler;
        // Run the webhook handler and produce a message
        self.webhook.execute(&self.http, true, |w| {
            handler.handle(envelope, body, w);
            w
        })
    }
}

/// Builder constructor for the Discord mailer
pub struct DiscordMailerBuilder {
    name: Option<String>,
}

impl DiscordMailerBuilder {
    /// Constructor
    pub fn new() -> Self {
        Self { name: None }
    }

    /// Adds an SMTP service name to the service
    ///
    /// # Parameters
    /// * `name` - the service name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Constructs the Discord mailer
    ///
    /// # Parameters
    /// * `webhook_auth` - Discord webhook id and auth info
    /// * `handler` - Object used to generate messages from email
    pub fn build<T>(
        self,
        webhook_auth: &DiscordWebhookAuth,
        handler: T,
    ) -> Result<DiscordMailer<T>, serenity::Error>
    where
        T: Clone + MailToDiscord,
    {
        let name = self.name.unwrap_or_else(|| "DiscordMailer".into());
        DiscordMailer::new(&name, webhook_auth, handler)
    }
}

/// Reads mail data over SMTP and sends it over Discord
pub struct DiscordMailSink<T> {
    /// The message's envelope
    envelope: Envelope,
    /// Buffer used to store the message body
    body: Vec<u8>,
    /// MPSC sender used to send the message to the Discord sink
    sink: Arc<Mutex<WebhookSender<T>>>,
}

impl<T> DiscordMailSink<T> {
    /// Constructor
    ///
    /// # Parameters
    /// * `envelope` - The message's envelope
    /// * `sink` - MPSC sender used to send the message to the discord sink
    fn new(envelope: Envelope, sink: Arc<Mutex<WebhookSender<T>>>) -> Self {
        Self {
            envelope,
            body: Vec::new(),
            sink,
        }
    }
}

impl<T> Mail for DiscordMailSink<T>
where
    T: MailToDiscord,
{
    /// Sends the message to the Discord sink queue
    fn queue(self) -> QueueResult {
        // Copy id out of the envelope
        let id = self.envelope.id.clone();

        // Return a result based on the result of the send operation
        // TODO: maybe have a receiver that detects whether there was a failure sending to the
        // Discord webhook so we can get feedback
        if let Ok(mut sink) = self.sink.lock() {
            match sink.send_messsage(self.envelope, self.body) {
                Ok(_) => QueueResult::QueuedWithId(id),
                Err(_) => QueueResult::Failed,
            }
        } else {
            QueueResult::Failed
        }
    }
}

impl<T> Sink for DiscordMailSink<T> {
    /// Type fed into the sink
    type SinkItem = Bytes;
    /// Error that occurs if sending or polling fails
    type SinkError = io::Error;

    /// Adds bytes to the mail body buffer
    ///
    /// # Parameters
    /// * `item` - Bytes to feed into the buffer
    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        // Consume the email bytes
        self.body.extend_from_slice(&item);
        // Return that the sink is ready for more
        Ok(AsyncSink::Ready)
    }

    /// Indicates that the poll is complete
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        // Shrink the internal buffer
        self.body.shrink_to_fit();
        // We are ready because none of this is actually asynchronous
        Ok(Async::Ready(()))
    }
}
