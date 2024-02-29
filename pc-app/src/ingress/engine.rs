//! The engine drives all communication.

use log::*;
use once_cell::sync::{Lazy, OnceCell};
use rustc_hash::FxHashMap;
use std::{net::IpAddr, time::Duration};
use tokio::{
    net::UdpSocket,
    sync::{
        broadcast,
        mpsc::{channel, error::TrySendError, Receiver, Sender},
        RwLock,
    },
    time::timeout,
};

use rpc_definition::{
    postcard_rpc::{
        headered::extract_header_from_bytes,
        host_client::{HostClient, ProcessError, RpcFrame, WireContext},
    },
    wire_error::{FatalError, ERROR_PATH},
};

/// The new state of a connection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    New(IpAddr),
    Closed(IpAddr),
}

/// Global singleton for the UDP socket.
///
/// RX happens in `udp_listener`, TX in `communication_worker`.
static SOCKET: OnceCell<UdpSocket> = OnceCell::new();

/// Core UDP socket listener, creates the socket and handles all incoming packets.
///
/// This should run until the app closes.
pub async fn udp_listener() -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:8321").await?;
    let socket = SOCKET.get_or_init(|| socket);

    // Wire workers are handling RX/TX packets, one worker per IP connected.
    let mut wire_workers = FxHashMap::default();
    wire_workers.reserve(1000);

    debug!("Waiting for connections...");

    loop {
        let mut rx_buf = Vec::with_capacity(2048);

        let (len, from) = socket.recv_buf_from(&mut rx_buf).await?;
        assert_eq!(rx_buf.len(), len); // Assumption: We don't need `len`.

        let ip = from.ip();

        // Find existing RX/TX worker or create a new one.
        let worker = wire_workers
            .entry(ip)
            .or_insert_with(|| create_communication_worker(ip));

        // Send packet to the worker, or create it again if it has closed its connection.
        if let Err(e) = worker.try_send(rx_buf) {
            match e {
                TrySendError::Full(_) => {
                    error!("{ip}: Can't keep up with incoming packets");
                }
                TrySendError::Closed(retry_payload) => {
                    // Recreate the worker if the old one has shut down.
                    wire_workers.insert(ip, create_communication_worker(ip));

                    if let Err(e) = wire_workers.get_mut(&ip).unwrap().try_send(retry_payload) {
                        error!(
                            "{}: Retry worker failed to start with error {e:?}",
                            from.ip()
                        );
                    }
                }
            }
        }
    }
}

// Helper to create a new worker for a specific IP.
fn create_communication_worker(from: IpAddr) -> Sender<Vec<u8>> {
    let (rx_packet_sender, rx_packet_recv) = channel(10);
    tokio::spawn(communication_worker(from, rx_packet_recv));
    rx_packet_sender
}

/// Global state of the active API clients for use by public API.
pub(crate) static API_CLIENTS: Lazy<RwLock<FxHashMap<IpAddr, HostClient<FatalError>>>> =
    Lazy::new(|| {
        RwLock::new({
            let mut m = FxHashMap::default();
            m.reserve(1000);
            m
        })
    });

/// Global subscription to signal a new connection is available.
pub(crate) static CONNECTION_SUBSCRIBER: Lazy<broadcast::Sender<Connection>> =
    Lazy::new(|| broadcast::channel(1000).0);

/// This handles incoming packets from a specific IP.
async fn communication_worker(ip: IpAddr, mut packet_recv: Receiver<Vec<u8>>) {
    debug!("{ip}: Registered new connection, starting handshake");

    // TODO: This is where we should perform ECDH handshake & authenticity verification of a device.
    //
    // let secure_channel = match perform_handshake(ip, packet_recv).await {
    //     Ok(ch) => ch,
    //     Err(e) => {
    //         error!("{ip}: Failed handshake, error = {e:?}");
    //         return;
    //     }
    // };

    // TODO: This is where we should perform version checks and firmware update devices before
    // accepting them as active. Most likely they will restart, and this connection will be closed
    // and recreated as soon as the device comes back updated and can pass this check.
    //
    // match check_version_and_maybe_update(&mut packet_recv) {
    //     FirmwareUpdateStatus::NeedsUpdating => {
    //         debug!("{ip}: Firmware needs updating, performing firmware update");
    //
    //         start_firmware_update(&ip).await;
    //
    //         // Close the worker and await the reconnection after updates.
    //         return;
    //     }
    //     FirmwareUpdateStatus::Valid => {
    //         debug!("{ip}: Firmware valid, continuing");
    //     }
    // }

    debug!("{ip}: Connection active");

    // We have one host client per connection.
    let (hostclient, wirecontext) = HostClient::<FatalError>::new_manual(ERROR_PATH, 10);

    // Store the API client for access by public APIs
    {
        API_CLIENTS.write().await.insert(ip, hostclient);
    }

    let _ = CONNECTION_SUBSCRIBER.send(Connection::New(ip));

    // Start handling of all I/O.
    let WireContext {
        mut outgoing,
        incoming,
        mut new_subs,
    } = wirecontext;

    let mut subs = FxHashMap::default();

    loop {
        // Adapted from `cobs_wire_worker`.
        // Wait for EITHER a serialized request, OR some data from the embedded device.
        tokio::select! {
            sub = new_subs.recv() => {
                let Some(si) = sub else {
                    break;
                };

                subs.insert(si.key, si.tx);
            }
            out = outgoing.recv() => {
                // Receiver returns None when all Senders have hung up.
                let (Some(msg), Some(socket)) = (out, SOCKET.get()) else {
                    break;
                };

                // Send message via the UDP socket.
                if let Err(e) = socket.send_to(&msg.to_bytes(), (ip, 8321)).await {
                    error!("{ip}: Socket send error = {e:?}");
                    break;
                }
            }
            packet = timeout(Duration::from_secs(5), packet_recv.recv()) => {
                // Make sure the UDP RX worker is still alive.
                let Ok(packet) = packet else {
                    debug!("{ip}: Connection closed.");
                    let _ = CONNECTION_SUBSCRIBER.send(Connection::Closed(ip));
                    break;
                };

                let Some(packet) = packet else {
                    break;
                };

                // Attempt to extract a header so we can get the sequence number.
                // Since UDP is already full packets, we don't need to use COBS or similar, a
                // packet is a full message.
                if let Ok((hdr, body)) = extract_header_from_bytes(&packet) {
                    // Got a header, turn it into a frame.
                    let frame = RpcFrame { header: hdr.clone(), body: body.to_vec() };

                    // Give priority to subscriptions. TBH I only do this because I know a hashmap
                    // lookup is cheaper than a waitmap search.
                    if let Some(tx) = subs.get_mut(&hdr.key) {
                        // Yup, we have a subscription.
                        if tx.send(frame).await.is_err() {
                            // But if sending failed, the listener is gone, so drop it.
                            subs.remove(&hdr.key);
                        }
                    } else {
                        // Wake the given sequence number. If the WaitMap is closed, we're done here
                        if let Err(ProcessError::Closed) = incoming.process(frame) {
                            break;
                        }
                    }
                }

            }
        }
    }

    // cleanup of global state
    API_CLIENTS.write().await.remove(&ip);
}
