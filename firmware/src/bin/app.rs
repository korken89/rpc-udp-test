#![no_main]
#![no_std]
#![allow(incomplete_features)]

use embassy_net::{
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
    Ipv4Address,
};
use embedded_io_async::Write;
use rtic_monotonics::{
    systick::{fugit::MicrosDurationU64, Systick},
    Monotonic,
};

defmt::timestamp!("{=u64:us}", {
    let time_us: MicrosDurationU64 = Systick::now().duration_since_epoch().convert();

    time_us.ticks() as u64
});

#[rtic::app(device = embassy_stm32::pac, dispatchers = [I2C1_EV, I2C1_ER, I2C2_EV, I2C2_ER], peripherals = false)]
mod app {
    use crate::{block_ethernet_high_prio, block_ethernet_same_prio, handle_stack, run_tcp};
    use rpc_testing::bsp::{self, NetworkStack};

    #[shared]
    struct Shared {
        network_stack: NetworkStack,
    }

    #[local]
    struct Local {}

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        defmt::info!("pre init");

        let network_stack = bsp::init(cx.core);

        handle_stack::spawn().ok();
        run_tcp::spawn().ok();
        // block_ethernet_high_prio::spawn().ok();
        block_ethernet_same_prio::spawn().ok();

        (Shared { network_stack }, Local {})
    }

    extern "Rust" {
        #[task(shared = [&network_stack])]
        async fn handle_stack(_: handle_stack::Context);

        #[task(shared = [&network_stack])]
        async fn run_tcp(_: run_tcp::Context);

        #[task(priority = 1)]
        async fn block_ethernet_high_prio(_: block_ethernet_high_prio::Context);

        #[task]
        async fn block_ethernet_same_prio(_: block_ethernet_same_prio::Context);
    }
}

pub async fn block_ethernet_high_prio(_: app::block_ethernet_high_prio::Context<'_>) -> ! {
    loop {
        Systick::delay(2.millis()).await;

        // Block for 20% of CPU time.
        let start = Systick::now();
        while start + 6.millis() > Systick::now() {}
    }
}

pub async fn block_ethernet_same_prio(_: app::block_ethernet_same_prio::Context<'_>) -> ! {
    loop {
        Systick::delay(1.millis()).await;

        let start = Systick::now();
        while start + 5.millis() > Systick::now() {}
    }
}

pub async fn handle_stack(cx: app::handle_stack::Context<'_>) -> ! {
    cx.shared.network_stack.run().await
}

use rtic_monotonics::systick::ExtU64;

pub async fn run_tcp(cx: app::run_tcp::Context<'_>) -> ! {
    let stack = cx.shared.network_stack;

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    defmt::info!("Network task initialized");

    // Then we can use it!
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];

    let mut buf = [0; 1024];

    loop {
        // let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
        let mut socket = UdpSocket::new(
            stack,
            &mut rx_meta,
            &mut rx_buffer,
            &mut tx_meta,
            &mut tx_buffer,
        );
        socket.bind(8321).unwrap();

        // socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        let remote_endpoint = (Ipv4Address::new(192, 168, 0, 200), 8321);

        loop {
            socket.send_to(b"hello!\n", remote_endpoint).await.unwrap();
            let (n, ep) = socket.recv_from(&mut buf).await.unwrap();
            if let Ok(s) = core::str::from_utf8(&buf[..n]) {
                defmt::info!("rxd from {}: {}", ep, s);
            }
        }

        // defmt::info!("connecting...");

        // let r = socket.connect(remote_endpoint).await;

        // if let Err(e) = r {
        //     defmt::info!("connect error: {:?}", e);

        //     Systick::delay(1.secs()).await;

        //     continue;
        // }

        // defmt::info!("connected!");

        // // let buf = [0; 1024];
        // let buf = b"pong";

        // let mut rx = [0; 1024];

        // loop {
        //     match socket.read(&mut rx).await {
        //         Ok(len) => {
        //             if len == 4 {
        //                 if &rx[0..4] == b"ping" {
        //                     let r = socket.write_all(buf).await;
        //                     if let Err(e) = r {
        //                         defmt::info!("write error: {:?}", e);
        //                         break;
        //                     }
        //                 } else {
        //                     defmt::info!(
        //                         "Did not get ping, len = {}, str = {}",
        //                         len,
        //                         core::str::from_utf8(&rx[0..4]).unwrap()
        //                     );
        //                 }
        //             } else if len == 0 {
        //                 defmt::info!("connection closed, len = 0");
        //                 break;
        //             } else {
        //                 defmt::info!("Did not get ping, len = {}", len);
        //             }
        //         }
        //         Err(e) => {
        //             defmt::info!("read error: {:?}", e);
        //             break;
        //         }
        //     }

        //     let r = socket.flush().await;
        //     if let Err(e) = r {
        //         defmt::info!("flush error: {:?}", e);
        //         break;
        //     }
        // }
    }
}
