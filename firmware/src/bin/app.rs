#![no_main]
#![no_std]
#![allow(incomplete_features)]

use embassy_futures::join::join;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    Ipv4Address,
};
use heapless::{binary_heap::Min, BinaryHeap};
use rtic_monotonics::{
    systick::{
        fugit::{ExtU64, MicrosDurationU64},
        Systick,
    },
    Monotonic,
};

defmt::timestamp!("{=u64:us}", {
    let time_us: MicrosDurationU64 = Systick::now().duration_since_epoch().convert();

    time_us.ticks()
});

#[rtic::app(device = embassy_stm32::pac, dispatchers = [I2C1_EV, I2C1_ER, I2C2_EV, I2C2_ER], peripherals = false)]
mod app {
    use crate::{handle_sleep_command, handle_stack, run_comms};
    use rpc_definition::endpoints::sleep::Sleep;
    use rpc_testing::bsp::{self, NetworkStack};
    use rtic_sync::{
        channel::{Receiver, Sender},
        make_channel,
    };

    #[shared]
    struct Shared {
        network_stack: NetworkStack,
        ethernet_tx_sender: Sender<'static, [u8; 128], 1>,
    }

    #[local]
    struct Local {}

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        defmt::info!("pre init");

        let network_stack = bsp::init(cx.core);

        let (ethernet_tx_sender, ethernet_tx_receiver) = make_channel!([u8; 128], 1);
        let (sleep_request_sender, sleep_request_receiver) = make_channel!((u32, Sleep), 8);

        handle_stack::spawn().ok();
        run_comms::spawn(ethernet_tx_receiver, sleep_request_sender).ok();
        handle_sleep_command::spawn(sleep_request_receiver).ok();

        (
            Shared {
                network_stack,
                ethernet_tx_sender,
            },
            Local {},
        )
    }

    extern "Rust" {
        #[task(shared = [&network_stack])]
        async fn handle_stack(_: handle_stack::Context);

        #[task(shared = [&network_stack, &ethernet_tx_sender])]
        async fn run_comms(
            _: run_comms::Context,
            _: Receiver<'static, [u8; 128], 1>,
            _: Sender<'static, (u32, Sleep), 8>,
        );

        #[task(shared = [&ethernet_tx_sender])]
        async fn handle_sleep_command(
            _: handle_sleep_command::Context,
            _: Receiver<'static, (u32, Sleep), 8>,
        );
    }
}

pub async fn handle_stack(cx: app::handle_stack::Context<'_>) -> ! {
    cx.shared.network_stack.run().await
}

use rpc_definition::{
    endpoints::{
        pingpong::{PingPongEndpoint, Pong},
        sleep::{Sleep, SleepDone, SleepEndpoint},
    },
    postcard_rpc::{self, Endpoint},
    wire_error::{FatalError, ERROR_KEY},
};
use rtic_sync::channel::{Receiver, Sender};

async fn dispatch(
    buf: &[u8],
    ethernet_tx: &mut Sender<'static, [u8; 128], 1>,
    sleep_command_sender: &mut Sender<'static, (u32, Sleep), 8>,
) {
    if let Err(e) = postcard_rpc::dispatch!(
        buf,
        (hdr, _buf) = _ => {
            // Do something with unhandled requests, maybe log a warning.
            defmt::error!("Got unhandled endpoint/topic with key = {:x}", hdr.key);
            unhandled_error(hdr.seq_no, ethernet_tx).await;
        },
        EP: (hdr, sleeping_req) = SleepEndpoint => {
            // Do something with `sleeping_req`
            sleep_command_sender.try_send((hdr.seq_no, sleeping_req)).ok();

        },
        EP: (hdr, _pingpong_req) = PingPongEndpoint => {
            // Do something with `pingpong_req`
            ping_response(hdr.seq_no, ethernet_tx).await;
        }
    ) {
        // Dispatch deserialization failure
        defmt::error!("Failed to do dispatch: {}", e);
    }
}

async fn unhandled_error(seq_no: u32, ethernet_tx: &mut Sender<'static, [u8; 128], 1>) {
    let mut buf = [0; 128];
    if let Ok(_) = postcard_rpc::headered::to_slice_keyed(
        seq_no,
        ERROR_KEY,
        &FatalError::UnknownEndpoint,
        &mut buf,
    ) {
        ethernet_tx.send(buf).await.ok();
    }
}

async fn ping_response(seq_no: u32, ethernet_tx: &mut Sender<'static, [u8; 128], 1>) {
    let mut buf = [0; 128];
    if let Ok(_) = postcard_rpc::headered::to_slice_keyed(
        seq_no,
        PingPongEndpoint::RESP_KEY,
        &Pong {},
        &mut buf,
    ) {
        ethernet_tx.send(buf).await.ok();
    }
}

async fn sleep_response(
    seq_no: u32,
    sleep: Sleep,
    ethernet_tx: &mut Sender<'static, [u8; 128], 1>,
) {
    let mut buf = [0; 128];
    if let Ok(_) = postcard_rpc::headered::to_slice_keyed(
        seq_no,
        SleepEndpoint::RESP_KEY,
        &SleepDone { slept_for: sleep },
        &mut buf,
    ) {
        ethernet_tx.send(buf).await.ok();
    }
}

#[derive(Clone)]
struct SortedSleepHandler {
    sleep_until: <Systick as Monotonic>::Instant,
    sleep: Sleep,
    seq_no: u32,
}

impl core::cmp::PartialEq for SortedSleepHandler {
    fn eq(&self, other: &Self) -> bool {
        self.sleep_until.eq(&other.sleep_until)
    }
}

impl core::cmp::Eq for SortedSleepHandler {}

impl core::cmp::PartialOrd for SortedSleepHandler {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.sleep_until.partial_cmp(&other.sleep_until)
    }
}

impl core::cmp::Ord for SortedSleepHandler {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.sleep_until.cmp(&other.sleep_until)
    }
}

pub async fn handle_sleep_command(
    cx: app::handle_sleep_command::Context<'_>,
    mut sleep_command_receiver: Receiver<'static, (u32, Sleep), 8>,
) {
    let mut eth_tx = cx.shared.ethernet_tx_sender.clone();
    let mut queue = BinaryHeap::<SortedSleepHandler, Min, 8>::new();

    loop {
        let next_wakeup = queue.peek().map(|next| next.sleep_until);

        if let Some(next_wakeup) = next_wakeup {
            if Systick::now() >= next_wakeup {
                let next = queue.pop().unwrap();

                sleep_response(next.seq_no, next.sleep, &mut eth_tx).await;

                continue;
            }
        }

        let (seq_no, sleep_command) = match next_wakeup {
            Some(next) => match Systick::timeout_at(next, sleep_command_receiver.recv()).await {
                Ok(o) => o.unwrap(),
                Err(_timeout) => continue,
            },
            None => sleep_command_receiver.recv().await.unwrap(),
        };

        queue
            .push(SortedSleepHandler {
                sleep_until: Systick::now()
                    + (sleep_command.seconds as u64).secs()
                    + (sleep_command.micros as u64).micros(),
                sleep: sleep_command,
                seq_no,
            })
            .ok();
    }
}

pub async fn run_comms(
    cx: app::run_comms::Context<'_>,
    mut ethernet_tx_receiver: Receiver<'static, [u8; 128], 1>,
    mut sleep_command_sender: Sender<'static, (u32, Sleep), 8>,
) -> ! {
    let stack = *cx.shared.network_stack;

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    defmt::info!("Network task initialized");

    // Then we can use it!
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];

    let mut buf = [0; 1024];

    // let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(8321).unwrap();

    let remote_endpoint = (Ipv4Address::new(192, 168, 0, 200), 8321);
    let mut ethernet_tx_sender = cx.shared.ethernet_tx_sender.clone();

    join(
        async {
            // Send worker.
            loop {
                socket
                    .send_to(
                        &ethernet_tx_receiver
                            .recv()
                            .await
                            .expect("We don't drop all senders"),
                        remote_endpoint,
                    )
                    .await
                    .unwrap();
            }
        },
        async {
            // Receive worker.
            loop {
                if let Ok((n, _ep)) = socket.recv_from(&mut buf).await {
                    dispatch(
                        &buf[..n],
                        &mut ethernet_tx_sender,
                        &mut sleep_command_sender,
                    )
                    .await;
                } else {
                    defmt::error!("UDP: incoming packet truncated");
                }
            }
        },
    )
    .await
    .0
}
