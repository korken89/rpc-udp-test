#![no_main]
#![no_std]
#![allow(incomplete_features)]

use rtic_monotonics::{
    systick::{fugit::MicrosDurationU64, Systick},
    Monotonic,
};

pub mod command_handling;
pub mod ethernet;

defmt::timestamp!("{=u64:us}", {
    let time_us: MicrosDurationU64 = Systick::now().duration_since_epoch().convert();

    time_us.ticks()
});

#[rtic::app(device = embassy_stm32::pac, dispatchers = [I2C1_EV, I2C1_ER, I2C2_EV, I2C2_ER], peripherals = false)]
mod app {
    use crate::{
        command_handling::handle_sleep_command,
        ethernet::{handle_stack, run_comms},
    };
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
