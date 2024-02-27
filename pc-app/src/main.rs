use log::*;
use once_cell::sync::{Lazy, OnceCell};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::{
    net::UdpSocket,
    sync::{
        mpsc::{channel, error::TrySendError, Receiver, Sender},
        RwLock,
    },
    time::timeout,
};

use rpc_definition::wire_error::{FatalError, ERROR_PATH};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    println!("Starting socket server");

    start_udp_listener().await
}

static SOCKET: OnceCell<UdpSocket> = OnceCell::new();

/// Core UDP socket listener, creates the socket and handles all incoming packets.
async fn start_udp_listener() -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:8321").await?;
    let socket = SOCKET.get_or_init(|| socket);

    let mut wire_workers = HashMap::new();

    info!("Waiting for connections...");

    loop {
        let mut rx_buf = Vec::with_capacity(2048);

        let (len, from) = socket.recv_buf_from(&mut rx_buf).await?;
        assert_eq!(rx_buf.len(), len);

        // Find existing RX worker or create a new one.
        let worker = wire_workers
            .entry(from.ip())
            .or_insert_with(|| create_rx_worker(from));

        // Send packet to the correct worker.
        if let Err(e) = worker.try_send((from, rx_buf)) {
            match e {
                TrySendError::Full(_) => {
                    error!("{}: Can't keep up with incoming packets", from.ip());
                }
                TrySendError::Closed(retry_payload) => {
                    // Recreate the worker if the old one has shut down.
                    wire_workers.insert(from.ip(), create_rx_worker(from));

                    if let Err(e) = wire_workers
                        .get_mut(&from.ip())
                        .unwrap()
                        .try_send(retry_payload)
                    {
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

// Create a new worker for a specific IP.
fn create_rx_worker(from: SocketAddr) -> Sender<(SocketAddr, Vec<u8>)> {
    let (rx_packet_sender, rx_packet_recv) = channel(10);

    tokio::spawn(rx_worker(from.ip(), rx_packet_recv));

    rx_packet_sender
}

/// Global state of the active API clients for use by public API.
static API_CLIENT: Lazy<RwLock<HashMap<IpAddr, HostClient<FatalError>>>> =
    Lazy::new(|| RwLock::new(HashMap::with_capacity(1000)));

/// This handles incoming packets from a specific IP.
async fn rx_worker(ip: IpAddr, mut recv: Receiver<(SocketAddr, Vec<u8>)>) {
    info!("{ip}: Registered new connection");

    // We have one host client per connection.
    let (hostclient, wirecontext) = HostClient::<FatalError>::new_manual(ERROR_PATH, 10);

    {
        // Store the API client for access by public APIs
        API_CLIENT.write().await.insert(ip, hostclient);
    }

    // Host client has the async user API, wire context does the actual I/O
    // TODO: handle wire context below to send/receive correctly

    loop {
        // TODO: Do we need `from`?
        let (from, pkt) = match timeout(Duration::from_secs(5), recv.recv()).await {
            Ok(o) => match o {
                Some(s) => s,
                None => {
                    error!("{ip}: RX pipe closed, stoping worker");
                    break;
                }
            },
            Err(_) => {
                error!("{ip}: Connection timeout, stoping worker");
                break;
            }
        };

        info!("{}: Got packet with len = {}", from.ip(), pkt.len());
    }

    // cleanup of global state
    API_CLIENT.write().await.remove(&ip);
}

//
// ---------------------- PUBLIC API ----------------------
//

use rpc_definition::{
    endpoints::sleep::{Sleep, SleepDone, SleepEndpoint},
    postcard_rpc::host_client::{HostClient, HostErr},
};

pub enum ApiError {
    IpNotFound,
    NoResponse,
    BadResponse,
    TooManyConcurrentApiCalls,
}

/// Example public API endpoint.
pub async fn sleep(unit: IpAddr, sleep: &Sleep) -> Result<SleepDone, ApiError> {
    let api = {
        // Hold the read lock to the global state as short as possible.
        API_CLIENT
            .read()
            .await
            .get(&unit)
            .ok_or(ApiError::IpNotFound)?
            .clone()
    };

    // TODO: Handle timeout, `send_resp` will wait forever.
    api.send_resp::<SleepEndpoint>(sleep)
        .await
        .map_err(|e| match e {
            HostErr::Wire(we) => match we {
                FatalError::UnknownEndpoint => todo!(),
                FatalError::NotEnoughSenders => todo!(),
                FatalError::WireFailure => todo!(),
            },
            HostErr::BadResponse => todo!(),
            HostErr::Postcard(_) => todo!(),
            HostErr::Closed => todo!(),
        })
}
