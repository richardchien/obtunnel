use std::collections::HashMap;

use futures::{channel::mpsc, SinkExt, StreamExt};
use maplit::hashmap;
use packet::ip;
use serde_json::{json, Value as JsonValue};
use tokio_tungstenite::{connect_async as ws_connect_async, tungstenite};
use tun::TunPacket;

use crate::config;

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
    oblink_send: mpsc::UnboundedReceiver<Frame>,
    oblink_recv: mpsc::UnboundedSender<Frame>,
) {
    // TODO: move to config file
    let self_id = "3281334718".to_string();
    // let self_id = "2910007356".to_string();
    let ws_url_table: HashMap<String, String> = hashmap! {
        "3281334718".to_string() => "ws://127.0.0.1:6701/".to_string(),
        "2910007356".to_string() => "ws://127.0.0.1:6702/".to_string(),
    };

    let url = url::Url::parse(
        ws_url_table
            .get(&self_id)
            .expect("cannot find entry for the given self_id in the WebSocket URL table"),
    )
    .unwrap();
    let (ob_stream, _) = ws_connect_async(url).await.unwrap();
    let (ob_tx, ob_rx) = ob_stream.split();

    tokio::join!(
        oblink_to_ob(oblink_send, ob_tx),
        ob_to_oblink(ob_rx, oblink_recv)
    );
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
async fn oblink_to_ob(mut oblink: ObLinkSource, mut ob: ObSink) {
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
                config::MAGIC_PREFIX,
                base64::encode(frame.packet.get_bytes())
            );
            let action = json!({
                "action": "send_private_msg",
                "params": {
                    "user_id": frame.destination().parse::<i64>().unwrap(),
                    "message": {
                        "type": "text",
                        "data": { "text": frame_text_form }
                    }
                }
            });
            ob.send(tungstenite::Message::Text(action.to_string()))
                .await
                .unwrap();
        } else {
            continue;
        }
    }
    panic!("oblink_to_ob stopped unexpectedly");
}

/// Forward OBLINK frame from OneBot impl to OBLINK layer.
async fn ob_to_oblink(mut ob: ObSource, mut oblink: ObLinkSink) {
    while let Some(Ok(ws_msg)) = ob.next().await {
        let payload = ws_msg.into_data();
        let event: JsonValue = serde_json::from_slice(&payload).unwrap();
        if let (Some("message"), Some("private"), Some(user_id), Some(self_id), Some(message)) = (
            event["post_type"].as_str(),
            event["message_type"].as_str(),
            event["user_id"].as_i64(),
            event["self_id"].as_i64(),
            event["message"].as_str(),
        ) {
            println!("message from: {}, content: {}", user_id, message);
            if message.starts_with(config::MAGIC_PREFIX) {
                if let Ok(payload) = base64::decode(&message[config::MAGIC_PREFIX.len()..]) {
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
                        .await
                        .unwrap();
                }
            }
        }
    }
    panic!("ob_to_oblink stopped unexpectedly");
}
