use futures::{
    channel::mpsc,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use packet::{ip, Packet};
use tun::{AsyncDevice, Device, TunPacket, TunPacketCodec};

use crate::{config, link::Frame};

/// Setup TUN device and forward TUN packets between
/// TUN device and OBLINK layer.
pub async fn tun_task(
    oblink_send: mpsc::UnboundedSender<Frame>,
    oblink_recv: mpsc::UnboundedReceiver<Frame>,
) {
    let mut tun_cfg = tun::Configuration::default();
    tun_cfg
        .address((172, 29, 0, 1))
        .netmask((255, 255, 255, 0))
        .mtu(config::MTU as i32)
        .up();

    #[cfg(target_os = "linux")]
    tun_cfg.platform(|cfg| {
        cfg.packet_information(true);
    });

    let tun_dev = tun::create_as_async(&tun_cfg).unwrap();
    {
        let tun_dev_ref = tun_dev.get_ref();
        println!("TUN device created:");
        println!("  Name: {}", tun_dev_ref.name());
        println!("  Address: {:?}", tun_dev_ref.address().unwrap());
        println!("  Netmask: {:?}", tun_dev_ref.netmask().unwrap());
        println!("  MTU: {:?}", tun_dev_ref.mtu().unwrap());
    }

    let (tun_tx, tun_rx) = tun_dev.into_framed().split();

    tokio::join!(
        tun_to_oblink(tun_rx, oblink_send),
        oblink_to_tun(oblink_recv, tun_tx)
    );
}

/// Forward TUN packets from TUN device to OBLINK layer.
async fn tun_to_oblink(
    mut tun: SplitStream<tokio_util::codec::Framed<AsyncDevice, TunPacketCodec>>,
    mut oblink: mpsc::UnboundedSender<Frame>,
) {
    // TODO: maybe to process packets concurrently in the future
    while let Some(Ok(packet)) = tun.next().await {
        if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(packet.get_bytes()) {
            println!(
                "Packet src: {}, dst: {}, data: {}",
                ip_pkt.source(),
                ip_pkt.destination(),
                hex::encode_upper(ip_pkt.payload())
            );
            // tun_tx.send(packet).await.unwrap();
            oblink
                .send(Frame::new_for_send("".to_string(), packet))
                .await
                .unwrap()
        } else {
            panic!("failed to parse TUN packet");
        }
    }
    panic!("tun_to_oblink stopped unexpectedly");
}

/// Forward TUN packets from OBLINK layer to TUN device.
async fn oblink_to_tun(
    mut oblink: mpsc::UnboundedReceiver<Frame>,
    mut tun: SplitSink<tokio_util::codec::Framed<AsyncDevice, TunPacketCodec>, TunPacket>,
) {
    while let Some(frame) = oblink.next().await {
        tun.send(frame.into_packet()).await.unwrap();
    }
    panic!("oblink_to_tun stopped unexpectedly");
}
