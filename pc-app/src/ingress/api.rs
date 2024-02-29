use super::api_handle;
use rpc_definition::{
    endpoints::sleep::{Sleep, SleepDone, SleepEndpoint},
    postcard_rpc::host_client::HostErr,
    wire_error::FatalError,
};
use std::{net::IpAddr, time::Duration};
use tokio::time::timeout;

/// Example public API endpoint.
///
/// This will make the MCU server wait the requested time before answering.
pub async fn sleep(device: IpAddr, sleep: &Sleep) -> Result<SleepDone, ApiError> {
    let api = api_handle(&device).await?;

    // TODO: Settable timeout, always have in public API? Seems not nice...
    // TODO: Do retries.
    timeout(
        Duration::from_secs(1),
        api.send_resp::<SleepEndpoint>(sleep),
    )
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
