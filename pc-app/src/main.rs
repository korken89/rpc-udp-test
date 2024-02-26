use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    let socket = UdpSocket::bind("0.0.0.0:8321").await?;

    loop {
        let mut buf = Vec::new();
        if let Ok((_len, from)) = socket.recv_buf_from(&mut buf).await {
            if let Ok(s) = core::str::from_utf8(&buf) {
                println!("Got ({from}): {s}");
            }

            socket.send_to(&buf, from).await?;
        }

        // let (stream, addr) = listener.accept().await?;
        // // stream.set_nodelay(true)?;
        // tokio::spawn(process_socket(addr, stream));
    }

    // Ok(())
}

// async fn process_socket(addr: SocketAddr, mut socket: TcpStream) {
//     let mut buf = Vec::with_capacity(1024 * 1024 * 1024);
//     let mut last_update = Instant::now();
//     let mut last_num_packets = 0;
//     let mut num_packets = 0;
//     let mut last_len = 0;

//     println!("Accepted connection from {addr}");

//     'outer: loop {
//         if let Ok(Ok(())) = timeout(Duration::from_secs(2), socket.write_all(b"ping")).await {
//             // socket.flush().await;
//         } else {
//             println!("TCP stream with {} ended", addr);
//             break;
//         }

//         if let Ok(Ok(size)) = timeout(Duration::from_secs(2), socket.read_buf(&mut buf)).await {
//             let len = buf.len();

//             if &buf[len - size..] != b"pong" {
//                 println!("no pong");
//                 break 'outer;
//             }

//             num_packets += 1;

//             // println!("got pong");

//             // Print speed every 1s
//             if last_update.elapsed() >= Duration::from_secs(1) {
//                 println!(
//                     "Total ({addr}): {:.3} MBit/s - {} packets/s",
//                     (buf.len() - last_len) as f64 / 1024. / 1024. * 8.,
//                     num_packets - last_num_packets,
//                 );

//                 last_update = Instant::now();
//                 last_len = buf.len();
//                 last_num_packets = num_packets;
//             }
//         } else {
//             println!("TCP stream with {} ended", addr);
//             break 'outer;
//         }
//     }
// }
