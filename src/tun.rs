use futures::{channel::mpsc, SinkExt, StreamExt};
use tun::Device;

use crate::link::Frame;

const MAGIC_PREFIX: &'static str = "(>_<)...";
const MAX_MSG_LEN: usize = 4500;
const MTU: usize = (MAX_MSG_LEN - MAGIC_PREFIX.len()) * 3 / 4;

pub async fn tun_task(
    oblink_send_chan: mpsc::UnboundedSender<Frame>,
    oblink_recv_chan: mpsc::UnboundedReceiver<Frame>,
) {
    let mut tun_config = tun::Configuration::default();
    tun_config
        .address((172, 29, 0, 1))
        .netmask((255, 255, 255, 0))
        .mtu(MTU as i32)
        .up();

    #[cfg(target_os = "linux")]
    tun_config.platform(|config| {
        config.packet_information(true);
    });

    let tun_dev = tun::create_as_async(&tun_config).unwrap();
    let tun_dev_ref = tun_dev.get_ref();
    println!("TUN device created:");
    println!("  Name: {}", tun_dev_ref.name());
    println!("  Address: {:?}", tun_dev_ref.address().unwrap());
    println!("  Netmask: {:?}", tun_dev_ref.netmask().unwrap());
    println!("  MTU: {:?}", tun_dev_ref.mtu().unwrap());

    let mut tun_stream = tun_dev.into_framed();
    // let (tun_tx, tun_rx) = tun_stream.split();
    // tun_rx
    //     .for_each(|packet| async {
    //         if let Ok(packet) = packet {
    //             oblink_send_chan_tx.send(Frame::new(0, 0, packet)).await;
    //         }
    //     })
    //     .await;
    while let Some(packet) = tun_stream.next().await {
        match packet {
            Ok(packet) => {
                println!("Packet: {}", hex::encode_upper(packet.get_bytes()));
                tun_stream.send(packet).await.unwrap();
            }
            Err(err) => panic!("Error: {:?}", err),
        }
    }
}
