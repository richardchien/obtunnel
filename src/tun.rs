use std::collections::HashMap;

use futures::{channel::mpsc, SinkExt, StreamExt};
use maplit::hashmap;
use packet::{ip, Packet};
use tun::{Device, TunPacket};

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
        // .address((172, 29, 0, 2))
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

    let (mut tun_tx, mut tun_rx) = tun_dev.into_framed().split();

    // while let Some(Ok(packet)) = tun_rx.next().await {
    //     if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(packet.get_bytes()) {
    //         println!(
    //             "send packet src: {}, dst: {}, data: {}",
    //             ip_pkt.source(),
    //             ip_pkt.destination(),
    //             hex::encode_upper(packet.get_bytes())
    //         );

    //         if ip_pkt.source() == ip_pkt.destination() {
    //             println!("packet for self");
    //             tun_tx
    //                 .send(TunPacket::new(packet.get_bytes().into()))
    //                 .await
    //                 .unwrap();
    //         }
    //     } else {
    //         panic!("failed to parse TUN packet");
    //     }
    // }

    tokio::join!(
        tun_to_oblink(tun_rx, oblink_send),
        oblink_to_tun(oblink_recv, tun_tx)
    );
}

type ObLinkSource = mpsc::UnboundedReceiver<Frame>;
type ObLinkSink = mpsc::UnboundedSender<Frame>;
type TunSource =
    futures::stream::SplitStream<tokio_util::codec::Framed<tun::AsyncDevice, tun::TunPacketCodec>>;
type TunSink = futures::stream::SplitSink<
    tokio_util::codec::Framed<tun::AsyncDevice, tun::TunPacketCodec>,
    tun::TunPacket,
>;

/// Forward TUN packets from TUN device to OBLINK layer.
async fn tun_to_oblink(mut tun: TunSource, mut oblink: ObLinkSink) {
    // TODO: move to config file
    let route_table: HashMap<String, String> = hashmap! {
        "172.29.0.1".to_string() => "3281334718".to_string(),
        "172.29.0.2".to_string() => "2910007356".to_string(),
    };

    // TODO: maybe to process packets concurrently in the future
    while let Some(Ok(packet)) = tun.next().await {
        if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(packet.get_bytes()) {
            println!(
                "send packet src: {}, dst: {}, data: {}",
                ip_pkt.source(),
                ip_pkt.destination(),
                hex::encode_upper(packet.get_bytes())
            );

            if ip_pkt.source() == ip_pkt.destination() {
                println!("packet for self");
                // TODO: send to self
                // tun_tx.send(packet).await.unwrap();
            }

            if let Some(dest_user_id) = route_table.get(&ip_pkt.destination().to_string()) {
                oblink
                    .send(Frame::new_for_send(dest_user_id.to_owned(), packet))
                    .await
                    .unwrap();
            } else {
                println!("cannot find entry for the given dest IP in the route table");
            }
        } else {
            panic!("failed to parse TUN packet");
        }
    }
    panic!("tun_to_oblink stopped unexpectedly");
}

/// Forward TUN packets from OBLINK layer to TUN device.
async fn oblink_to_tun(mut oblink: ObLinkSource, mut tun: TunSink) {
    while let Some(frame) = oblink.next().await {
        println!("oblink frame to tun: {:#x?}", frame);
        tun.send(frame.into_packet()).await.unwrap();
    }
    panic!("oblink_to_tun stopped unexpectedly");
}
