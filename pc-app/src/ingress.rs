//! Embedded device ingress.
//!
//! Here the public APIs of the ingress are exposed.

mod engine;

/// Start the device ingress.
pub async fn start_ingress() -> anyhow::Result<()> {
    engine::udp_listener().await
}

//
// ---------------------- EXAMPLE PUBLIC API ----------------------
//

/// Subscriptions to data are handled here.
pub mod subscriptions {
    use super::engine;
    use rpc_definition::topics::heartbeat::Heartbeat;
    use std::net::IpAddr;
    use tokio::sync::broadcast;

    pub use engine::Connection;

    /// Subscription handle.
    pub struct Subscription<T>(broadcast::Receiver<T>);

    impl<T> Subscription<T>
    where
        T: Clone,
    {
        /// Receive a value from a subscription.
        pub async fn recv(&mut self) -> Result<T, SubscriptionError> {
            self.0.recv().await.map_err(|e| match e {
                broadcast::error::RecvError::Closed => unreachable!("We don't close the channel"),
                broadcast::error::RecvError::Lagged(_) => SubscriptionError::MessagesDropped,
            })
        }
    }

    /// Example of state tracking.
    ///
    /// Get an event on connection change.
    pub fn connection() -> Subscription<Connection> {
        Subscription(engine::CONNECTION_SUBSCRIBER.subscribe())
    }

    /// Example public topic subscription (unsolicited messages).
    ///
    ///
    pub async fn heartbeat(device: IpAddr) -> Subscription<(IpAddr, Heartbeat)> {
        // TODO: How to subscribe to ALL?
        // We can add a worker that auto-subscribes to a single device as soon as a connection is made.

        // let api = api_handle(&device)
        //     .await
        //     .map_err(|_notfound| SubscriptionError::IpNotFound)?;

        // api.subscribe::<TopicHeartbeat>(10) // TODO: What depth?
        //     .await
        //     .map_err(|_closed| SubscriptionError::IpNotFound)

        todo!()
    }

    /// Errors on subscription.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum SubscriptionError {
        IpNotFound,
        MessagesDropped,
    }
}

/// RPC APIs are handled here.
pub mod api {
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
}

use rpc_definition::{postcard_rpc::host_client::HostClient, wire_error::FatalError};
use std::net::IpAddr;

async fn api_handle(device: &IpAddr) -> Result<HostClient<FatalError>, api::ApiError> {
    // Hold the read lock to the global state as short as possible.
    engine::API_CLIENT
        .read()
        .await
        .get(&device)
        .ok_or(api::ApiError::IpNotFound)
        .cloned()
}
