extern crate obsdn;

use futures::channel::mpsc;
use obsdn::{link::oblink_task, tun::tun_task};

#[tokio::main]
async fn main() {
    let (oblink_send_chan_tx, oblink_send_chan_rx) = mpsc::unbounded(); // for sending packets through OneBot
    let (oblink_recv_chan_tx, oblink_recv_chan_rx) = mpsc::unbounded(); // for receiving packets from OneBot

    let oblink_task_fut = oblink_task(oblink_send_chan_rx, oblink_recv_chan_tx);
    let tun_task_fut = tun_task(oblink_send_chan_tx, oblink_recv_chan_rx);

    tokio::join!(oblink_task_fut, tun_task_fut);
}
