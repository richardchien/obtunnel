use futures::{channel::mpsc, StreamExt};
use serde_json::Value as JsonValue;
use tokio_tungstenite::connect_async as ws_connect_async;
use tun::TunPacket;

#[derive(Debug)]
pub struct Frame {
    source: String,
    destination: String,
    packet: TunPacket,
}

impl Frame {
    pub fn new(src_user_id: i64, dst_user_id: i64, packet: TunPacket) -> Self {
        Self {
            source: src_user_id.to_string(),
            destination: dst_user_id.to_string(),
            packet,
        }
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
}

const ONEBOT_WS_URL: &'static str = "ws://127.0.0.1:6701/"; // TODO: put in config file

pub async fn oblink_task(
    send_chan: mpsc::UnboundedReceiver<Frame>,
    recv_chan: mpsc::UnboundedSender<Frame>,
) {
    let url = url::Url::parse(ONEBOT_WS_URL).unwrap();
    let (onebot_stream, _) = ws_connect_async(url).await.unwrap();
    let (onebot_tx, onebot_rx) = onebot_stream.split();

    let onebot_rx_fut = onebot_rx.for_each(|ws_message| async move {
        let payload = ws_message.unwrap().into_data();
        let event: JsonValue = serde_json::from_slice(&payload).unwrap();
        if let (Some("message"), Some(user_id), Some(self_id), Some(message)) = (
            event["post_type"].as_str(),
            event["user_id"].as_i64(),
            event["self_id"].as_i64(),
            event["message"].as_str(),
        ) {
            // println!(
            //     "{:?}",
            //     Frame {
            //         src_user_id: user_id.to_string(),
            //         dst_user_id: self_id.to_string(),
            //         packet: TunPacket::new(bytes),
            //     }
            // );
            println!("{}", message);
        }
    });

    let send_chan_fut = send_chan.for_each(|frame| async move {
        println!("{:?}", frame);
    });

    tokio::join!(onebot_rx_fut, send_chan_fut);
}
