
use std::sync::Arc;

use anyhow::{Result, Context, anyhow as err};
use log::*;
// use tokio::time::Duration;


pub use webrtc::api::setting_engine::SettingEngine as RTCSettingEngine;
pub use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
// use webrtc::data_channel::data_channel_message::DataChannelMessage;
pub use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
pub use webrtc::peer_connection::RTCPeerConnection;
// use webrtc::peer_connection::math_rand_alpha;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
pub use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;





// struct SignalingServer {}
// impl SignalingServer {
//     async fn listen() -> SignalingChannel
// }
// struct SignalingClient {}
// impl SignalingClient {
//     async fn connect() -> SignalingChannel
// }
// impl HttpSignalingClient {}
// impl StdIOSignalingClient {}


mod util {
    /// must_read_stdin blocks until input is received from stdin
    pub fn must_read_stdin() -> anyhow::Result<String> {
        let mut line = String::new();

        std::io::stdin().read_line(&mut line)?;
        line = line.trim().to_owned();
        println!();

        Ok(line)
    }

    // Allows compressing offer/answer to bypass terminal input limits.
    // const COMPRESS: bool = false;

    /// encode encodes the input in base64
    /// It can optionally zip the input before encoding
    pub fn encode(b: &str) -> String {
        //if COMPRESS {
        //    b = zip(b)
        //}

        base64::encode(b)
    }

    /// decode decodes the input from base64
    /// It can optionally unzip the input after decoding
    pub fn decode(s: &str) -> anyhow::Result<String> {
        let b = base64::decode(s)?;

        //if COMPRESS {
        //    b = unzip(b)
        //}

        let s = String::from_utf8(b)?;
        Ok(s)
    }
}


pub struct AsyncTwoWay<T> {
    tx: async_channel::Sender<T>,
    rx: async_channel::Receiver<T>,
}
impl<T> AsyncTwoWay<T>
where T: std::fmt::Debug {
    pub fn pair(buffer_size: usize) -> (Self, Self) {
        let (send1, recv1) = async_channel::bounded(buffer_size);
        let (send2, recv2) = async_channel::bounded(buffer_size);

        (Self {tx: send1, rx: recv2}, Self {tx: send2, rx: recv1})
    }
    // pub fn once(func: FnOnce(RTCSessionDescription) -> RTCSessionDescription) -> (SignalingChannel)
    // pub fn once(func: impl Future<RTCSessionDescription>) -> (SignalingChannel)

    pub fn send(&self, value: T) -> Result<()> {
        self.tx.try_send(value).map_err(|e| err!("{}", e))
    }
    pub async fn recv(&mut self) -> Result<T> {
        self.rx.recv().await.context("") //.ok_or(err!("channel closed"))
    }
}
pub type SignalingChannel = AsyncTwoWay<RTCSessionDescription>;



// TODO: perfect negotiation? polite/impolite, onnegotiationneeded
//       https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Perfect_negotiation
pub type TokioSignal = async_channel::Receiver<()>;
pub async fn new_peer(ice_servers: Vec<RTCIceServer>, settings: Option<RTCSettingEngine>)
-> Result<(Arc<RTCPeerConnection>, TokioSignal)> {
    // TODO: config api (registry) (settings engine)?
    //       rtcp (control protocol)
    //       twcc (congestion control)

    // Create the API object
    let api = match settings {
        Some(engine) => webrtc::api::APIBuilder::new().with_setting_engine(engine),
        None => webrtc::api::APIBuilder::new()
    }.build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers,
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    let (done_tx, done_rx) = async_channel::bounded::<()>(1);

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        info!("Peer Connection State has changed: {}", s);

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            warn!("Peer Connection has gone to failed exiting");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }));

    Ok((peer_connection, done_rx))
}


async fn set_local_desc(peer_connection: &RTCPeerConnection, desc: RTCSessionDescription) -> Result<()> {
    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(desc).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    Ok(())
}

// TODO: trickle ICE, onicecandidate || signaler.send()
pub async fn create_channel(
    peer_connection: &RTCPeerConnection,
    mut signaling: SignalingChannel,
    config: Option<RTCDataChannelInit>,
    setup: impl FnOnce(Arc<RTCDataChannel>)
) -> Result<()> {
    // Create a datachannel with label 'data'
    let data_channel = peer_connection.create_data_channel("data", config).await?;

    setup(data_channel);

    // Create an offer to send to the browser
    let offer = peer_connection.create_offer(None).await?;

    set_local_desc(peer_connection, offer).await?;

    // Output the answer in base64 so we can paste it in browser
    if let Some(local_desc) = peer_connection.local_description().await {
        signaling.send(local_desc)?;
    } else {
        error!("generate local_description failed!");
    }

    // Wait for the answer to be pasted
    let answer = signaling.recv().await?;

    // Apply the answer as the remote description
    peer_connection.set_remote_description(answer).await.context("remote_desc")?;

    Ok(())
}

// TODO: trickle ICE, onicecandidate || signaler.send()
// TODO: cleanup `setup` type. Maybe we can stash in Arc to avoid constraints
pub async fn join_channel(
    peer_connection: &RTCPeerConnection,
    mut signaling: SignalingChannel,
    setup: impl FnOnce(Arc<RTCDataChannel>) + Send + Sync + Clone + 'static
) -> Result<()> {
    // Register data channel creation handling
    peer_connection
        .on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
            let setup = setup.clone();
            Box::pin(async move {
                setup(Arc::clone(&d));
            })
        }));

    // Wait for the offer to be pasted
    let offer = signaling.recv().await?;

    // Set the remote SessionDescription
    peer_connection.set_remote_description(offer).await?;

    // Create an answer
    let answer = peer_connection.create_answer(None).await?;

    set_local_desc(peer_connection, answer).await?;

    // Output the answer in base64 so we can paste it in browser
    if let Some(local_desc) = peer_connection.local_description().await {
        signaling.send(local_desc)?;
    } else {
        error!("generate local_description failed!");
    }

    Ok(())
}


pub async fn stdio_signal_listener(signaling: SignalingChannel) -> Result<()> {
    let (line_tx, line_rx) = async_channel::bounded::<Result<String>>(1);
    std::thread::Builder::new().name("stdio_signal_listener".into()).spawn(move || {
        loop {
            // TODO: figure out how to poison this thread when the outer function returns
            // (As is, if the outer function returns, this will fail after newline read)
            println!("Awaiting next offer... Copy-paste here:");
            if line_tx.try_send(util::must_read_stdin()).is_err() { return }
        }
    })?;
    loop {
        let line = line_rx.recv().await??;
        let desc_data = util::decode(line.as_str())?;
        let desc = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;
        signaling.tx.try_send(desc).context("RTCSessionDescription from stdin")?;
        // anyhow::Ok(())

        let desc = signaling.rx.recv().await.context("RTCSessionDescription to stdout")?;
        debug!("offer: {:?}", desc);
        let json_str = serde_json::to_string(&desc)?;
        let b64 = util::encode(&json_str);
        println!("{}", b64);
        // anyhow::Ok(())
    }
}



// pub async fn stdio_signaling_once(signaling: SignalingChannel) -> Result<()> {
pub async fn stdio_signal_once(offer: RTCSessionDescription) -> Result<RTCSessionDescription> {
    // let offer = signaling.rx.recv().await.context("RTCSessionDescription to stdout")?;

    debug!("offer: {:?}", offer);
    let json_str = serde_json::to_string(&offer)?;
    let b64 = util::encode(&json_str);
    println!("{}", b64);

    println!();
    println!("Awaiting answer... Copy-paste here:");
    let line = util::must_read_stdin()?;
    let desc_data = util::decode(line.as_str())?;
    let answer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // signaling.tx.try_send(answer).context("RTCSessionDescription from stdin")?;

    Ok(answer)
}
