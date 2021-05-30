use std::collections::HashMap;

use futures::{channel::mpsc, StreamExt};
use packet::ip;
use serde_json::Value as JsonValue;
use tokio_tungstenite::connect_async as ws_connect_async;
use tun::TunPacket;

use crate::config;

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
    let url = url::Url::parse(config::ONEBOT_WS_URL).unwrap();
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
    // TODO: move to Config struct
    use maplit::hashmap;
    let route_table: HashMap<&str, i64> = hashmap! {
        "172.29.0.1" => 3281334718,
        "172.29.0.2" => 2910007356,
    };

    // TODO: maybe to process packets concurrently in the future
    while let Some(frame) = oblink.next().await {
        if let Ok(ip::Packet::V4(ip_pkt)) = ip::Packet::new(frame.packet().get_bytes()) {
            // TODO: lookup route table for dest QQ, convert ip_pkt.payload() to oblink frame
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
        if let (Some("message"), Some(user_id), Some(self_id), Some(message)) = (
            event["post_type"].as_str(),
            event["user_id"].as_i64(),
            event["self_id"].as_i64(),
            event["message"].as_str(),
        ) {
            println!("{}", message);
            // TODO: check the message prefix, convert oblink frame to ip packet
        }
    }
    panic!("ob_to_oblink stopped unexpectedly");
}
