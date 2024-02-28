//! A small example ingress handling many concurrent connections to embedded devices connected via
//! UDP, where each device implementes `postcard-rpc` for RPCs and unsoliced messages (topics).
//!
//! Note: This app uses IP as identifier for each device, you should not do that when running UDP,
//! as UDP source addresses are trivial to spoof.

use ingress::subscriptions::Connection;
use log::*;
// use std::time::Duration;
// use tokio::time::interval;

// This is the "library"
pub mod ingress;

// This is the app using the library
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    info!("Starting ingress");
    tokio::spawn(ingress::run_ingress());

    // TODO: Use the API here.
    let mut connecton = ingress::subscriptions::connection();

    loop {
        let Ok(connection) = connecton.recv().await else {
            continue;
        };

        match connection {
            Connection::New(ip) => info!("{ip}: New connection established."),
            Connection::Closed(ip) => info!("{ip}: Connection lost."),
        }
    }
}
