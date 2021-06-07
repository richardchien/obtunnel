use anyhow::Result;
use futures::{channel::mpsc, SinkExt, StreamExt};
use packet::ip;
use tun::Device;

use crate::{link::Frame, Config};

/// Setup TUN device and forward TUN packets between
/// TUN device and OBLINK layer.
pub async fn tun_task(
    config: &Config,
    oblink_send: mpsc::UnboundedSender<Frame>,
    oblink_recv: mpsc::UnboundedReceiver<Frame>,
) -> Result<()> {
    let mut tun_cfg = tun::Configuration::default();
    tun_cfg
        .address(&config.self_ip)
        .netmask(&config.netmask)
        .mtu(config.mtu as i32)
        .up();

    #[cfg(target_os = "linux")]
    tun_cfg.platform(|cfg| {
        cfg.packet_information(true);
    });

    let tun_dev = tun::create_as_async(&tun_cfg)?;
    {
        let tun_dev_ref = tun_dev.get_ref();
        println!("TUN device created:");
        println!("  Name: {}", tun_dev_ref.name());
        println!("  Address: {:?}", tun_dev_ref.address()?);
        println!("  Netmask: {:?}", tun_dev_ref.netmask()?);
        println!("  MTU: {:?}", tun_dev_ref.mtu()?);
    }

    let (tun_tx, tun_rx) = tun_dev.into_framed().split();

    tokio::try_join!(
        tun_to_oblink(config, tun_rx, oblink_send),
        oblink_to_tun(config, oblink_recv, tun_tx)
    )?;

    Ok(())
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
async fn tun_to_oblink(config: &Config, mut tun: TunSource, mut oblink: ObLinkSink) -> Result<()> {
    // TODO: maybe to process packets concurrently in the future
    while let Some(Ok(packet)) = tun.next().await {
        if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(packet.get_bytes()) {
            // println!(
            //     "send packet src: {}, dst: {}, data: {}",
            //     ip_pkt.source(),
            //     ip_pkt.destination(),
            //     hex::encode_upper(packet.get_bytes())
            // );

            if ip_pkt.source() == ip_pkt.destination() {
                println!("packet for self");
                continue;
            }

            if let Some(dest_user_id) = config.route_table.get(&ip_pkt.destination().to_string()) {
                oblink
                    .send(Frame::new_for_send(dest_user_id.to_owned(), packet))
                    .await?
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
async fn oblink_to_tun(_: &Config, mut oblink: ObLinkSource, mut tun: TunSink) -> Result<()> {
    while let Some(frame) = oblink.next().await {
        // println!("oblink frame to tun: {:#x?}", frame);
        tun.send(frame.into_packet()).await?;
    }
    panic!("oblink_to_tun stopped unexpectedly");
}
