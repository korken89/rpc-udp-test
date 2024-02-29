use super::{api_handle, engine};
use once_cell::sync::Lazy;
use rpc_definition::topics::{
    heartbeat::{Heartbeat, TopicHeartbeat},
    some_data::{SomeData, TopicSomeData},
};
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

/// Get an event on connection change.
pub fn connection() -> Subscription<Connection> {
    Subscription(engine::CONNECTION_SUBSCRIBER.subscribe())
}

/// Example public topic subscription (unsolicited messages).
///
/// Get heartbeats from a device.
pub async fn heartbeat() -> Subscription<(IpAddr, Heartbeat)> {
    Subscription(HEARTBEAT_SUBSCRIBER.subscribe())
}

/// Example public topic subscription (unsolicited messages).
///
/// Get some data from a device.
pub async fn some_data() -> Subscription<(IpAddr, SomeData)> {
    Subscription(SOMEDATA_SUBSCRIBER.subscribe())
}

/// Errors on subscription.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SubscriptionError {
    IpNotFound,
    MessagesDropped,
}

//
// --------------- Subscription consolidation
//

/// Global subscription for heartbeats.
pub(crate) static HEARTBEAT_SUBSCRIBER: Lazy<broadcast::Sender<(IpAddr, Heartbeat)>> =
    Lazy::new(|| broadcast::channel(100).0);

/// Global subscription for some data.
pub(crate) static SOMEDATA_SUBSCRIBER: Lazy<broadcast::Sender<(IpAddr, SomeData)>> =
    Lazy::new(|| broadcast::channel(100).0);

/// This tracks unsolicited messages and sends them on the correct endpoint, in the end
/// consolidating all messages of the same type into one stream of `(source, message)`.
pub(crate) async fn subscription_consolidation() {
    loop {
        // On every new connection, subscribe to data for that device.
        if let Ok(Connection::New(ip)) = connection().recv().await {
            let Ok(api) = api_handle(&ip).await else {
                continue;
            };

            // Get subscriptions to all topic
            let Ok(mut hb_sub) = api.subscribe::<TopicHeartbeat>(10).await else {
                continue;
            };

            let Ok(mut sd_sub) = api.subscribe::<TopicSomeData>(10).await else {
                continue;
            };

            // TODO: Add next subscription here.

            tokio::spawn(async move {
                tokio::select! {
                    _ = async {
                        while let Some(s) = hb_sub.recv().await {
                            let _ = HEARTBEAT_SUBSCRIBER.send((ip, s));
                        }
                    } => {}
                    _ = async {
                        while let Some(s) = sd_sub.recv().await {
                            let _ = SOMEDATA_SUBSCRIBER.send((ip, s));
                        }
                    } => {}
                    // TODO: Add next subscription forwarder here.
                }
            });
        }
    }
}
