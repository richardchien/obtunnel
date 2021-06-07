use anyhow::Result;
use futures::{channel::mpsc, SinkExt, StreamExt};
use packet::ip;
use serde_json::{json, Value as JsonValue};
use tokio_tungstenite::{connect_async as ws_connect_async, tungstenite};
use tun::TunPacket;

use crate::Config;

/// OBLINK frame struct representation.
#[derive(Debug)]
pub struct Frame {
    source: String,
    destination: String,
    packet: TunPacket,
}

impl Frame {
    pub fn new(source: String, destination: String, packet: TunPacket) -> Self {
        Self {
            source,
            destination,
            packet,
        }
    }

    pub fn new_for_send(destination: String, packet: TunPacket) -> Self {
        Self::new("".to_string(), destination, packet)
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    pub fn packet(&self) -> &TunPacket {
        &self.packet
    }

    pub fn into_packet(self) -> TunPacket {
        self.packet
    }
}

/// Setup OneBot connection and forward OBLINK frame between
/// OneBot impl and OBLINK layer.
pub async fn oblink_task(
    config: &Config,
    oblink_send: mpsc::UnboundedReceiver<Frame>,
    oblink_recv: mpsc::UnboundedSender<Frame>,
) -> Result<()> {
    let url = url::Url::parse(
        config
            .ws_url_table
            .get(&config.self_user_id)
            .expect("cannot find entry for the given self_id in the WebSocket URL table"),
    )?;
    let (ob_stream, _) = ws_connect_async(url).await?;
    let (ob_tx, ob_rx) = ob_stream.split();

    tokio::try_join!(
        oblink_to_ob(config, oblink_send, ob_tx),
        ob_to_oblink(config, ob_rx, oblink_recv)
    )?;

    Ok(())
}

type ObLinkSource = mpsc::UnboundedReceiver<Frame>;
type ObLinkSink = mpsc::UnboundedSender<Frame>;
type ObSource = futures::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;
type ObSink = futures::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Message,
>;

/// Forward OBLINK frame from OBLINK layer to OneBot impl.
async fn oblink_to_ob(config: &Config, mut oblink: ObLinkSource, mut ob: ObSink) -> Result<()> {
    // TODO: maybe to process packets concurrently in the future
    while let Some(frame) = oblink.next().await {
        assert!(!frame.destination().is_empty()); // the frame must have a destination
        if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(frame.packet().get_bytes()) {
            assert_ne!(
                ip_pkt.source(),
                ip_pkt.destination(),
                "any packets that send to self should be handled in tun.rs"
            );
            let frame_text_form = format!(
                "{}{}",
                config.magic_prefix,
                base64::encode(frame.packet.get_bytes())
            );
            let action = json!({
                "action": "send_private_msg",
                "params": {
                    "user_id": frame.destination().parse::<i64>()?,
                    "message": {
                        "type": "text",
                        "data": { "text": frame_text_form }
                    }
                }
            });
            ob.send(tungstenite::Message::Text(action.to_string()))
                .await?;
        } else {
            continue;
        }
    }
    panic!("oblink_to_ob stopped unexpectedly");
}

/// Forward OBLINK frame from OneBot impl to OBLINK layer.
async fn ob_to_oblink(config: &Config, mut ob: ObSource, mut oblink: ObLinkSink) -> Result<()> {
    while let Some(Ok(ws_msg)) = ob.next().await {
        let payload = ws_msg.into_data();
        let event: JsonValue = serde_json::from_slice(&payload)?;
        if let (Some("message"), Some("private"), Some(user_id), Some(self_id), Some(message)) = (
            event["post_type"].as_str(),
            event["message_type"].as_str(),
            event["user_id"].as_i64(),
            event["self_id"].as_i64(),
            event["message"].as_str(),
        ) {
            println!("message from: {}, content: {}", user_id, message);
            if message.starts_with(&config.magic_prefix) {
                if let Ok(payload) = base64::decode(&message[config.magic_prefix.len()..]) {
                    if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(&payload) {
                        println!(
                            "receive packet src: {}, dst: {}, data: {}",
                            ip_pkt.source(),
                            ip_pkt.destination(),
                            hex::encode_upper(&payload)
                        );
                    }
                    oblink
                        .send(Frame::new(
                            user_id.to_string(),
                            self_id.to_string(),
                            TunPacket::new(payload),
                        ))
                        .await?;
                }
            }
        }
    }
    panic!("ob_to_oblink stopped unexpectedly");
}
