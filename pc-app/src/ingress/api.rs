use super::api_handle;
use rpc_definition::{
    endpoints::{
        pingpong::{Ping, PingPongEndpoint, Pong},
        sleep::{Sleep, SleepDone, SleepEndpoint},
    },
    postcard_rpc::host_client::HostErr,
    wire_error::FatalError,
};
use std::{future::Future, net::IpAddr, time::Duration};
use tokio::time::timeout;

// TODO: Do retries.

/// Example public API endpoint.
///
/// This will make the MCU server wait the requested time before answering.
pub async fn sleep(device: IpAddr, sleep: &Sleep) -> Result<SleepDone, ApiError> {
    let api = api_handle(&device).await?;

    timeout_helper(api.send_resp::<SleepEndpoint>(sleep)).await
}

/// Example public API endpoint.
///
/// This will perform a ping/pong exchange with the device.
pub async fn ping(device: IpAddr) -> Result<Pong, ApiError> {
    let api = api_handle(&device).await?;

    timeout_helper(api.send_resp::<PingPongEndpoint>(&Ping {})).await
}

async fn timeout_helper<F, T>(f: F) -> Result<T, ApiError>
where
    F: Future<Output = Result<T, HostErr<FatalError>>>,
{
    // TODO: Settable timeout, always have in public API? Seems not nice...
    timeout(Duration::from_secs(1), f)
        .await
        .map_err(|_timeout| ApiError::NoResponse)?
        .map_err(Into::into)
}

/// Errors of the public API.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

/// Auto-convert from internal communication errors to user understandable errors.
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
