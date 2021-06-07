pub mod link;
pub mod tun;

use std::collections::HashMap;

use anyhow::Result;
use futures::channel::mpsc;
use serde::Deserialize;

use crate::{link::oblink_task, tun::tun_task};

#[derive(Debug, Deserialize)]
pub struct Config {
    magic_prefix: String,
    max_msg_len: u32,
    mtu: u32,
    netmask: String,
    self_ip: String,
    self_user_id: String,
    route_table: HashMap<String, String>,
    ws_url_table: HashMap<String, String>,
}

pub async fn main() -> Result<()> {
    let config: Config = toml::from_str(&std::fs::read_to_string(
        std::env::current_dir()?.join("obtunnel.toml"),
    )?)?;
    println!("{:#?}", config);

    let (oblink_send_chan_tx, oblink_send_chan_rx) = mpsc::unbounded(); // for sending packets through OneBot
    let (oblink_recv_chan_tx, oblink_recv_chan_rx) = mpsc::unbounded(); // for receiving packets from OneBot

    tokio::try_join!(
        oblink_task(&config, oblink_send_chan_rx, oblink_recv_chan_tx),
        tun_task(&config, oblink_send_chan_tx, oblink_recv_chan_rx)
    )?;

    Ok(())
}
