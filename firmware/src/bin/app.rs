#![no_main]
#![no_std]
#![allow(incomplete_features)]

use alloc::collections::btree_map::Entry;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    Ipv4Address,
};
use rtic_monotonics::{
    systick::{fugit::MicrosDurationU64, ExtU64, Systick},
    Monotonic,
};

defmt::timestamp!("{=u64:us}", {
    let time_us: MicrosDurationU64 = Systick::now().duration_since_epoch().convert();

    time_us.ticks() as u64
});

#[rtic::app(device = embassy_stm32::pac, dispatchers = [I2C1_EV, I2C1_ER, I2C2_EV, I2C2_ER], peripherals = false)]
mod app {
    use crate::{handle_stack, run_comms};
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
        run_comms::spawn().ok();

        (Shared { network_stack }, Local {})
    }

    extern "Rust" {
        #[task(shared = [&network_stack])]
        async fn handle_stack(_: handle_stack::Context);

        #[task(shared = [&network_stack])]
        async fn run_comms(_: run_comms::Context);

    }
}

pub async fn handle_stack(cx: app::handle_stack::Context<'_>) -> ! {
    cx.shared.network_stack.run().await
}

// pub async fn dispatcher(innie: Innie, outie: OutieRef, mut led: Output<'static, AnyPin>) {
//     loop {
//         let msg = innie.receive().await;

//         let Some(whbody) = WhBody::try_from(&msg) else {
//             continue;
//         };

//         match whbody.wh.key {
//             icd::Uptime::REQ_KEY => {
//                 // This has no request body
//                 handle_uptime(outie, whbody.wh.seq_no).await;
//             }
//             icd::ResetToBootloader::REQ_KEY => {
//                 handle_reset_bootload(outie, whbody.wh.seq_no).await;
//             }
//             icd::Ident::REQ_KEY => {
//                 if let Ok(time_ms) =
//                     postcard::from_bytes::<<icd::Ident as Endpoint>::Request>(whbody.body)
//                 {
//                     handle_ident(outie, whbody.wh.seq_no, time_ms, &mut led).await;
//                 }
//             }
//             _ => {
//                 // oops
//             }
//         }
//     }
// }

use rpc_definition::postcard_rpc::Endpoint;

/// Possible errors in dispatch handling.
pub enum DispatchError {
    /// The deserialization of the header failed.
    Header(postcard::Error),
    /// The  deserialization of the body failed.
    Body(postcard::Error),
}

macro_rules! postcard_rpc_dispatch {
    (
        $buf:ident,
        $unhandled:ident = _ => $unhandled_body:tt,
        $($request:pat = $endpoint:path => $body:tt),*
    ) => {
        match rpc_definition::postcard_rpc::headered::extract_header_from_bytes($buf) {
            Ok((hdr, body)) => {
                match hdr.key {
                $(
                    <$endpoint as Endpoint>::REQ_KEY => {
                        match postcard::take_from_bytes::<<$endpoint as Endpoint>::Request>(body) {
                            Ok((req, _rest)) => {
                                let $request = (&hdr, req);
                                $body

                                Ok(())
                            }
                            Err(e) => Err(DispatchError::Body(e))
                        }
                    }
                )*
                    _ => {
                        let $unhandled = (&hdr, body);

                        $unhandled_body

                        Ok(())
                    }
                }
            }
            Err(e) => Err(DispatchError::Header(e)),
        }

    };
}

async fn dispatch(buf: &[u8]) -> Result<(), ()> {
    use rpc_definition::endpoints::{pingpong::PingPongEndpoint, sleep::SleepEndpoint};

    let r = postcard_rpc_dispatch!(
        buf,
        unhandled = _ => {
            // Do something with unhandled requests, maybe log a warning.
        },
        (hdr, sleeping_req) = SleepEndpoint => {
            // Do something with `sleeping_req`
            // some_queue.try_send(sleeping_req);
        },
        pingpong_req = PingPongEndpoint => {
            // Do something with `pingpong_req`
        }
    );

    // postcard_rpc!(
    //     sleeping_req = SleepingEnpoint => {
    //       // Do something with `sleeping_req`
    //     }
    //     pingpong_req = PingPongEndpoint => {
    //       // Do something with `pingpong_req`
    //     }
    // );
    //
    // Expands to:

    // let (hdr, buf) =
    //     rpc_definition::postcard_rpc::headered::extract_header_from_bytes(buf).map_err(|_| ())?;

    // match hdr.key {
    //     SleepEndpoint::REQ_KEY => {
    //         let Ok((msg, _rest)) =
    //             postcard::take_from_bytes::<<SleepEndpoint as Endpoint>::Request>(buf)
    //         else {
    //             return Err(());
    //         };

    //         // Handle sleep command asynchronously.
    //     }
    //     PingPongEndpoint::REQ_KEY => {
    //         let Ok((msg, _rest)) =
    //             postcard::take_from_bytes::<<PingPongEndpoint as Endpoint>::Request>(buf)
    //         else {
    //             return Err(());
    //         };

    //         // Handle ping command asynchronously.
    //     }
    //     _ => {
    //         return Err(());
    //     }
    // }

    Ok(())
}

pub async fn run_comms(cx: app::run_comms::Context<'_>) -> ! {
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

        let remote_endpoint = (Ipv4Address::new(192, 168, 0, 200), 8321);

        loop {
            socket.send_to(b"hello!\n", remote_endpoint).await.unwrap();
            if let Ok(Ok((n, ep))) =
                Systick::timeout_after(3.secs(), socket.recv_from(&mut buf)).await
            {
                dispatch(&buf[..n]);
                if let Ok(s) = core::str::from_utf8(&buf[..n]) {
                    defmt::info!("rxd from {}: {}", ep, s);
                }
            } else {
                defmt::warn!("No response from {}", remote_endpoint);
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
