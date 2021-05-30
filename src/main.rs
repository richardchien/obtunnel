extern crate obtunnel;

use futures::channel::mpsc;
use obtunnel::{link::oblink_task, tun::tun_task};

#[tokio::main]
async fn main() {
    let (oblink_send_chan_tx, oblink_send_chan_rx) = mpsc::unbounded(); // for sending packets through OneBot
    let (oblink_recv_chan_tx, oblink_recv_chan_rx) = mpsc::unbounded(); // for receiving packets from OneBot

    tokio::join!(
        oblink_task(oblink_send_chan_rx, oblink_recv_chan_tx),
        tun_task(oblink_send_chan_tx, oblink_recv_chan_rx)
    );
}
