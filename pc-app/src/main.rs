//! A small example ingress handling many concurrent connections to embedded devices connected via
//! UDP, where each device implementes `postcard-rpc` for RPCs and unsoliced messages (topics).
//!
//! Note: This app uses IP as identifier for each device, you should not do that when running UDP.
//! as UDP source addresses are trivial to spoof.

use log::*;
use std::{net::IpAddr, time::Duration};
use tokio::time::{interval, timeout};

mod ingress;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    info!("Starting ingress");
    tokio::spawn(ingress::udp_listener());

    // TODO: Use the API here.
    let mut interval = interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
    }
}

//
// ---------------------- PUBLIC API ----------------------
//

use rpc_definition::{
    endpoints::sleep::{Sleep, SleepDone, SleepEndpoint},
    postcard_rpc::host_client::HostErr,
    wire_error::FatalError,
};

/// Example public API endpoint.
///
/// This will make the MCU server wait the requested time before answering.
pub async fn sleep(device: IpAddr, sleep: &Sleep) -> Result<SleepDone, ApiError> {
    // Hold the read lock to the global state as short as possible.
    let api = {
        ingress::API_CLIENT
            .read()
            .await
            .get(&device)
            .ok_or(ApiError::IpNotFound)?
            .clone()
    };

    // TODO: Settable timeout, always have in public API? Seems not nice...
    timeout(
        Duration::from_secs(1),
        api.send_resp::<SleepEndpoint>(sleep),
    )
    .await
    .map_err(|_| ApiError::NoResponse)?
    .map_err(Into::into)
}

/// Errors of the public API.
pub enum ApiError {
    IpNotFound,
    NoResponse,
    // Unsure if the ones below should be log::warn/error instead of be given to the user.
    // Not sure if a user really can do anything with them.
    BadResponse,
    Malformed,
    TooManyConcurrentApiCalls,
    Unimplemented,
}

impl From<HostErr<FatalError>> for ApiError {
    fn from(value: HostErr<FatalError>) -> Self {
        match value {
            HostErr::Wire(we) => match we {
                FatalError::UnknownEndpoint => ApiError::Unimplemented,
                FatalError::NotEnoughSenders => ApiError::TooManyConcurrentApiCalls,
                FatalError::WireFailure => ApiError::Malformed,
            },
            HostErr::BadResponse => ApiError::BadResponse,
            HostErr::Postcard(_) => ApiError::Malformed,
            HostErr::Closed => ApiError::NoResponse,
        }
    }
}
