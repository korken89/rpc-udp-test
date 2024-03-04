use crate::app;
use heapless::{binary_heap::Min, BinaryHeap, Vec};
use rpc_definition::{
    endpoints::{
        pingpong::{PingPongEndpoint, Pong},
        sleep::{Sleep, SleepDone, SleepEndpoint},
    },
    postcard_rpc::{self, Endpoint},
    wire_error::{FatalError, ERROR_KEY},
};
use rtic_monotonics::{
    systick::{fugit::ExtU64, Systick},
    Monotonic,
};
use rtic_sync::channel::{Receiver, Sender};

/// Main command dispatch helper, this is called on all incoming packets.
pub async fn dispatch(
    buf: &[u8],
    ethernet_tx: &mut Sender<'static, Vec<u8, 128>, 1>,
    sleep_command_sender: &mut Sender<'static, (u32, Sleep), 8>,
) {
    // Do handling of each command, some synchronously and some asynchronously.
    if let Err(e) = postcard_rpc::dispatch!(
        buf,
        (hdr, _buf) = _ => {
            defmt::error!("Got unhandled endpoint/topic with key = {:x}", hdr.key);
            unhandled_error(hdr.seq_no, ethernet_tx, FatalError::UnknownEndpoint).await;
        },
        EP: (hdr, sleeping_req) = SleepEndpoint => {
            defmt::trace!("Got Sleep request {}", sleeping_req);
            if sleep_command_sender.try_send((hdr.seq_no, sleeping_req)).is_err() {
                // If all queues are full, tell the backend that we are over capacity.
                unhandled_error(hdr.seq_no, ethernet_tx, FatalError::NotEnoughSenders).await;
            }
        },
        EP: (hdr, _pingpong_req) = PingPongEndpoint => {
            defmt::trace!("Got Ping request");
            ping_response(hdr.seq_no, ethernet_tx).await;
        }
    ) {
        // Dispatch deserialization failure
        defmt::error!("Failed to do dispatch: {}", e);
    }
}

/// Helper to generate an unhandled packet error.
async fn unhandled_error(
    seq_no: u32,
    ethernet_tx: &mut Sender<'static, Vec<u8, 128>, 1>,
    error: FatalError,
) {
    let mut buf = [0; 128];
    if let Ok(used) = postcard_rpc::headered::to_slice_keyed(seq_no, ERROR_KEY, &error, &mut buf) {
        ethernet_tx.send(Vec::from_slice(used).unwrap()).await.ok();
    }
}

/// Helper to generate a response to a `Ping` call.
async fn ping_response(seq_no: u32, ethernet_tx: &mut Sender<'static, Vec<u8, 128>, 1>) {
    let mut buf = [0; 128];
    if let Ok(used) = postcard_rpc::headered::to_slice_keyed(
        seq_no,
        PingPongEndpoint::RESP_KEY,
        &Pong {},
        &mut buf,
    ) {
        ethernet_tx.send(Vec::from_slice(used).unwrap()).await.ok();
    }
}

/// Helper to generate a response to a `Sleep` call.
async fn sleep_response(
    seq_no: u32,
    sleep: Sleep,
    ethernet_tx: &mut Sender<'static, Vec<u8, 128>, 1>,
) {
    let mut buf = [0; 128];
    if let Ok(used) = postcard_rpc::headered::to_slice_keyed(
        seq_no,
        SleepEndpoint::RESP_KEY,
        &SleepDone { slept_for: sleep },
        &mut buf,
    ) {
        ethernet_tx.send(Vec::from_slice(used).unwrap()).await.ok();
    }
}

/// Task to executing `Sleep` commands.
///
/// It looks a bit complex, but basically it:
/// 1. Takes `Sleep` commands from a queue and calculate the time at which a
///    response should be sent.
/// 2. Puts this in a sorted heap, with the next to execute at the top.
/// 3. Wait for the next one to dequeue, and generate a response over Ethernet.
pub async fn handle_sleep_command(
    _: app::handle_sleep_command::Context<'_>,
    mut sleep_command_receiver: Receiver<'static, (u32, Sleep), 8>,
    mut ethernet_tx_sender: Sender<'static, Vec<u8, 128>, 1>,
) {
    let mut queue = BinaryHeap::<SortedSleepHandler, Min, 8>::new();

    loop {
        // Always get the head of the queue in case last iteration replaced it.
        let next_wakeup = queue.peek().map(|next| next.sleep_until);

        // Check if the time has come to send a response.
        if let Some(next_wakeup) = next_wakeup {
            if Systick::now() >= next_wakeup {
                let next = queue.pop().unwrap();

                defmt::debug!("Sleep {} finished", next.seq_no);
                sleep_response(next.seq_no, next.sleep, &mut ethernet_tx_sender).await;

                continue;
            }
        }

        // Check if there is a new command to add to the queue.
        let (seq_no, sleep_command) = match next_wakeup {
            Some(next) => match Systick::timeout_at(next, async {
                if queue.len() == queue.capacity() {
                    // The queue is full, wait for timeout.
                    core::future::pending().await
                } else {
                    // The queue is not full, we can add more data into the queue.
                    sleep_command_receiver.recv().await.unwrap()
                }
            })
            .await
            {
                Ok(o) => o,
                Err(_timeout) => continue,
            },
            None => sleep_command_receiver.recv().await.unwrap(),
        };

        defmt::debug!("Sleep {} requested", seq_no);
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

/// Boiler-plate to make a `Sleep` command sortable on when it should run in a
/// `heapless::BinaryHeap`.
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
