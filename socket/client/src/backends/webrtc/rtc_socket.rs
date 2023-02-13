use std::{sync::Arc, time::Duration};
use std::net::{SocketAddr, Ipv4Addr};

use anyhow::{Error, Result};
use bytes::Bytes;
use log::*;
use reqwest::{Client as HttpClient, Response};
use tinyjson::JsonValue;
use tokio::{sync::mpsc, sync::oneshot, time::sleep};

use webrtc::data::data_channel::DataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::{sdp::session_description::RTCSessionDescription, RTCPeerConnection};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;

use naia_socket_shared::rtc_peer;
use rtc_peer::*;
// use rtc_peer::{
//     SignalingChannel, RTCPeerConnection, RTCDataChannel, RTCIceServer, RTCSessionDescription, RTCSettingEngine
// };

use crate::backends::webrtc::addr_cell::AddrCell;
use crate::backends::webrtc::runtime;

const MESSAGE_SIZE: usize = 1500;
const CLIENT_CHANNEL_SIZE: usize = 8;

pub struct Socket;

impl Socket {
    pub async fn connect(
        server_url: &str,
    ) -> (AddrCell, mpsc::Sender<Box<[u8]>>, mpsc::Receiver<Box<[u8]>>) {
        let addr_cell = AddrCell::default();
        let (channel_setup, channel_ready, to_server_sender, to_client_receiver)
            = channel_setup_fn();

        let mut settings = RTCSettingEngine::default();
        settings.detach_data_channels();

        // create a new RTCPeerConnection
        let (peer_connection, _) = new_peer(fallback_ice_servers(), Some(settings))
            .await.expect("configuring peer_connection");

        let (mut sig1, sig2) = SignalingChannel::pair(1);

        let signaling_join = runtime::get_runtime().spawn(async move {
            let offer = sig1.recv().await?;
            let answer = stdio_signal_once(offer).await?;
            sig1.send(answer)?;
            anyhow::Ok(())
        });

        create_channel(
            Arc::clone(&peer_connection),
            sig2,
            Some(UNRELIABLE_CONFIG),
            channel_setup
        ).await.expect("creating data_channel");
        signaling_join.await.expect("stdio signaling complete");
        channel_ready.await.expect("data_channel ready");

        // FIXME: data_channel isn't finalized at this point.
        //        need to await a signal from on_open()

        let remote_candidate = get_nominated_candidate(&peer_connection).await.expect("get_nominated_candidate").remote_addr();
        // let remote_candidate = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        info!("Connected to {}", remote_candidate);

        addr_cell
            .set(remote_candidate)
            .await;

        debug!("debug enabled");


        // TODO: save or return peer_connection
        (addr_cell, to_server_sender, to_client_receiver)
    }

    // pub async fn connect(
    //     server_url: &str,
    // ) -> (AddrCell, mpsc::Sender<Box<[u8]>>, mpsc::Receiver<Box<[u8]>>) {
    //     let (to_server_sender, to_server_receiver) =
    //         mpsc::channel::<Box<[u8]>>(CLIENT_CHANNEL_SIZE);
    //     let (to_client_sender, to_client_receiver) =
    //         mpsc::channel::<Box<[u8]>>(CLIENT_CHANNEL_SIZE);

    //     let addr_cell = AddrCell::default();

    //     // create a new RTCPeerConnection
    //     let peer_connection = new_peer(fallback_ice_servers())
    //         .await.expect("configuring peer_connection");

    //     let label = "data";
    //     // TODO: config unreliable: Some(RTCDataChannelInit {})
    //     let protocol = None;

    //     // create a datachannel with label 'data'
    //     let data_channel = peer_connection
    //         .create_data_channel(label, protocol)
    //         .await
    //         .expect("cannot create data channel");

    //     // datachannel on_error callback
    //     data_channel
    //         .on_error(Box::new(move |error| {
    //             error!("data channel error: {:?}", error);
    //             Box::pin(async {})
    //         }));

    //     // datachannel on_open callback
    //     let data_channel_ref = Arc::clone(&data_channel);
    //     data_channel
    //         .on_open(Box::new(move || {
    //             let data_channel_ref_2 = Arc::clone(&data_channel_ref);
    //             Box::pin(async move {
    //                 let detached_data_channel = data_channel_ref_2
    //                     .detach()
    //                     .await
    //                     .expect("data channel detach got error");

    //                 // Handle reading from the data channel
    //                 let detached_data_channel_1 = Arc::clone(&detached_data_channel);
    //                 let detached_data_channel_2 = Arc::clone(&detached_data_channel);
    //                 tokio::spawn(async move {
    //                     let _loop_result =
    //                         read_loop(detached_data_channel_1, to_client_sender).await;
    //                     // do nothing with result, just close thread
    //                 });

    //                 // Handle writing to the data channel
    //                 tokio::spawn(async move {
    //                     let _loop_result =
    //                         write_loop(detached_data_channel_2, to_server_receiver).await;
    //                     // do nothing with result, just close thread
    //                 });
    //             })
    //         }));

    //     // create an offer to send to the server
    //     let offer = peer_connection
    //         .create_offer(None)
    //         .await
    //         .expect("cannot create offer");

    //     // sets the LocalDescription, and starts our UDP listeners
    //     peer_connection
    //         .set_local_description(offer)
    //         .await
    //         .expect("cannot set local description");

    //     let offer = peer_connection.local_description().await.unwrap();

    //     // let (sig1, sig2) = SignalingChannel::pair(1);

    //     // Send a request to server to initiate connection (signaling, essentially)
    //     let (answer, candidate) = http_signal_once(server_url, offer)
    //         .await
    //         .expect("offer/answer signaling");
    //     // TODO
    //     // let answer = stdio_signal_once(offer)
    //     //     .await
    //     //     .expect("offer/answer signaling");

    //     // apply the server's response as the remote description
    //     peer_connection
    //         .set_remote_description(answer)
    //         .await
    //         .expect("cannot set remote description");

    //     // TODO: optional? fake? maybe just replace entirely with shared code
    //     addr_cell
    //         .receive_candidate(candidate.candidate.as_str())
    //         .await;

    //     // add ice candidate to connection
    //     if let Err(error) = peer_connection.add_ice_candidate(candidate).await {
    //         panic!("Error during add_ice_candidate: {:?}", error);
    //     }

    //     (addr_cell, to_server_sender, to_client_receiver)
    // }
}

fn fallback_ice_servers() -> Vec<RTCIceServer> {
    vec![RTCIceServer { // backup
        urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        ..Default::default()
    }]
}


pub fn channel_setup_fn() -> (
    impl FnOnce(Arc<RTCPeerConnection>, Arc<RTCDataChannel>),
    oneshot::Receiver<()>,
    mpsc::Sender<Box<[u8]>>,
    mpsc::Receiver<Box<[u8]>>
) {
    let (to_server_sender, to_server_receiver) =
            mpsc::channel::<Box<[u8]>>(CLIENT_CHANNEL_SIZE);
    let (to_client_sender, to_client_receiver) =
        mpsc::channel::<Box<[u8]>>(CLIENT_CHANNEL_SIZE);

    let (ready_tx, ready_rx) = oneshot::channel();

    // let func = |_peer, data_channel| {
    let func = |peer: Arc<RTCPeerConnection>, data_channel: Arc<RTCDataChannel>| {
        // datachannel on_error callback
        data_channel
            .on_error(Box::new(move |error| {
                error!("data channel error: {:?}", error);
                Box::pin(async {})
            }));

        // datachannel on_open callback
        let data_channel_ref = Arc::clone(&data_channel);
        data_channel
            .on_open(Box::new(move || {
                debug_data_channel(&data_channel_ref);

                ready_tx.send(()).unwrap();

                let data_channel_ref_2 = Arc::clone(&data_channel_ref);
                Box::pin(async move {
                    let detached_data_channel = data_channel_ref_2
                        .detach()
                        .await
                        .expect("data channel detach got error");

                    // Handle reading from the data channel
                    let detached_data_channel_1 = Arc::clone(&detached_data_channel);
                    let detached_data_channel_2 = Arc::clone(&detached_data_channel);
                    tokio::spawn(async move {
                        let _loop_result =
                            read_loop(detached_data_channel_1, to_client_sender).await;
                        // do nothing with result, just close thread
                    });

                    // Handle writing to the data channel
                    tokio::spawn(async move {
                        let _loop_result =
                            write_loop(detached_data_channel_2, to_server_receiver).await;
                        // do nothing with result, just close thread
                    });

                    // tokio::spawn(async move {
                    //     loop {
                    //         sleep(Duration::from_secs(1)).await;
                    //         if let Some(pair) = get_nominated_candidate(&peer).await {
                    //             debug!("{:?}", pair.net_stats());
                    //         }
                    //     }
                    // });
                })
            }));
    };

    (func, ready_rx, to_server_sender, to_client_receiver)
}

// pub async fn http_signaling_once(signaling: SignalingChannel) -> Result<()> {
pub async fn http_signal_once(server_url: &str, offer: RTCSessionDescription)
-> Result<(RTCSessionDescription, RTCIceCandidateInit)> {
    let http_client = HttpClient::new();

    // let offer = signaling.rx.recv().await.context("RTCSessionDescription to stdout")?;

    let sdp = offer.sdp;
    let sdp_len = sdp.len();

    // wait to receive a response from server
    let response: Response = loop {
        let request = http_client
            .post(server_url)
            .header("Content-Length", sdp_len)
            .body(sdp.clone());

        match request.send().await {
            Ok(resp) => {
                break resp;
            }
            Err(err) => {
                warn!("Could not send request, original error: {:?}", err);
                sleep(Duration::from_secs(1)).await;
            }
        };
    };
    let response_string = response.text().await?;

    // parse session from server response
    let session_response: JsSessionResponse = get_session_response(response_string.as_str());
    let answer = RTCSessionDescription::answer(session_response.answer.sdp)?;

    // signaling.tx.try_send(answer).context("RTCSessionDescription from stdin")?;
    Ok((answer, session_response.candidate.candidate))
}


// TODO: use shared code
// async fn new_peer(ice_servers: Vec<RTCIceServer>)
// -> Result<Arc<RTCPeerConnection>> {
//     // TODO: config api (registry) (settings engine)?
//     //       rtcp (control protocol)
//     //       twcc (congestion control)

//     // Create the API object with the MediaEngine
//     let api = webrtc::api::APIBuilder::new()
//         .build();

//     // Prepare the configuration
//     let config = RTCConfiguration {
//         ice_servers,
//         ..Default::default()
//     };

//     // Create a new RTCPeerConnection
//     let peer_connection = Arc::new(api.new_peer_connection(config).await?);


//     // Set the handler for Peer connection state
//     // This will notify you when the peer has connected/disconnected
//     peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
//         info!("Peer Connection State has changed: {}", s);

//         if s == RTCPeerConnectionState::Failed {
//             // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
//             // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
//             // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
//             warn!("Peer Connection has gone to failed exiting");
//         }

//         Box::pin(async {})
//     }));

//     Ok(peer_connection)
// }

// read_loop shows how to read from the datachannel directly
async fn read_loop(
    data_channel: Arc<DataChannel>,
    to_client_sender: mpsc::Sender<Box<[u8]>>,
) -> Result<()> {
    let mut buffer = vec![0u8; MESSAGE_SIZE];
    loop {
        let message_length = match data_channel.read(&mut buffer).await {
            Ok(length) => length,
            Err(err) => {
                warn!("Datachannel closed; Exit the read_loop: {}", err);
                return Ok(());
            }
        };

        match to_client_sender.send(buffer[..message_length].into()).await {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::new(e));
            }
        }
    }
}

// write_loop shows how to write to the datachannel directly
async fn write_loop(
    data_channel: Arc<DataChannel>,
    mut to_server_receiver: mpsc::Receiver<Box<[u8]>>,
) -> Result<()> {
    loop {
        if let Some(write_message) = to_server_receiver.recv().await {
            match data_channel.write(&Bytes::from(write_message)).await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Error::new(e));
                }
            }
        } else {
            return Ok(());
        }
    }
}

#[derive(Clone)]
pub(crate) struct SessionAnswer {
    pub(crate) sdp: String,
}

pub(crate) struct SessionCandidate {
    pub(crate) candidate: RTCIceCandidateInit,
}

pub(crate) struct JsSessionResponse {
    pub(crate) answer: SessionAnswer,
    pub(crate) candidate: SessionCandidate,
}

fn get_session_response(input: &str) -> JsSessionResponse {
    let json_obj: JsonValue = input.parse().unwrap();

    let sdp_opt: Option<&String> = json_obj["answer"]["sdp"].get();
    let sdp: String = sdp_opt.unwrap().clone();

    let candidate_opt: Option<&String> = json_obj["candidate"]["candidate"].get();
    let candidate: String = candidate_opt.unwrap().clone();

    JsSessionResponse {
        answer: SessionAnswer { sdp },
        candidate: SessionCandidate {
            candidate: RTCIceCandidateInit {
                candidate,
                ..Default::default()
            }
        },
    }
}
