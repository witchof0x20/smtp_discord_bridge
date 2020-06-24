# smtp_discord_bridge

This program hosts an SMTP server that will forward all messages to a Discord bridge. This is useful for setting up servers when you don't particularly want to set up an SMTP server for alerts, but would still like to receive notifications if anything goes wrong.


## How to run

* Install rust
* `cargo run --release`
