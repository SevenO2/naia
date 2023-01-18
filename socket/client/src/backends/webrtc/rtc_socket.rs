use std::{sync::Arc, time::Duration};

use anyhow::{Error, Result};
use bytes::Bytes;
use log::{warn, info};
use reqwest::{Client as HttpClient, Response};
use tinyjson::JsonValue;
use tokio::{sync::mpsc, time::sleep};

use webrtc::data::data_channel::DataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::{sdp::session_description::RTCSessionDescription, RTCPeerConnection};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;

use crate::backends::webrtc::addr_cell::AddrCell;

const MESSAGE_SIZE: usize = 1500;
const CLIENT_CHANNEL_SIZE: usize = 8;

pub struct Socket;

impl Socket {
    pub async fn connect(
        server_url: &str,
    ) -> (AddrCell, mpsc::Sender<Box<[u8]>>, mpsc::Receiver<Box<[u8]>>) {
        let (to_server_sender, to_server_receiver) =
            mpsc::channel::<Box<[u8]>>(CLIENT_CHANNEL_SIZE);
        let (to_client_sender, to_client_receiver) =
            mpsc::channel::<Box<[u8]>>(CLIENT_CHANNEL_SIZE);

        let addr_cell = AddrCell::default();

        // create a new RTCPeerConnection
        let peer_connection = new_peer(fallback_ice_servers())
            .await.expect("configuring peer_connection");

        let label = "data";
        // TODO: config unreliable: Some(RTCDataChannelInit {})
        let protocol = None;

        // create a datachannel with label 'data'
        let data_channel = peer_connection
            .create_data_channel(label, protocol)
            .await
            .expect("cannot create data channel");

        // datachannel on_error callback
        data_channel
            .on_error(Box::new(move |error| {
                println!("data channel error: {:?}", error);
                Box::pin(async {})
            }));

        // datachannel on_open callback
        let data_channel_ref = Arc::clone(&data_channel);
        data_channel
            .on_open(Box::new(move || {
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
                })
            }));

        // create an offer to send to the server
        let offer = peer_connection
            .create_offer(None)
            .await
            .expect("cannot create offer");

        // sets the LocalDescription, and starts our UDP listeners
        peer_connection
            .set_local_description(offer)
            .await
            .expect("cannot set local description");

        // send a request to server to initiate connection (signaling, essentially)
        let http_client = HttpClient::new();

        let sdp = peer_connection.local_description().await.unwrap().sdp;

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
        let response_string = response.text().await.unwrap();

        // parse session from server response
        let session_response: JsSessionResponse = get_session_response(response_string.as_str());

        // apply the server's response as the remote description
        let session_description =
            RTCSessionDescription::answer(session_response.answer.sdp).unwrap();

        peer_connection
            .set_remote_description(session_description)
            .await
            .expect("cannot set remote description");

        addr_cell
            .receive_candidate(session_response.candidate.candidate.candidate.as_str())
            .await;

        // add ice candidate to connection
        if let Err(error) = peer_connection
            .add_ice_candidate(session_response.candidate.candidate)
            .await
        {
            panic!("Error during add_ice_candidate: {:?}", error);
        }

        (addr_cell, to_server_sender, to_client_receiver)
    }
}

fn fallback_ice_servers() -> Vec<RTCIceServer> {
    vec![RTCIceServer { // backup
        urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        ..Default::default()
    }]
}

async fn new_peer(ice_servers: Vec<RTCIceServer>)
-> Result<Arc<RTCPeerConnection>> {
    // TODO: config api (registry) (settings engine)?
    //       rtcp (control protocol)
    //       twcc (congestion control)

    // Create the API object with the MediaEngine
    let api = webrtc::api::APIBuilder::new()
        .build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers,
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);


    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        info!("Peer Connection State has changed: {}", s);

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            warn!("Peer Connection has gone to failed exiting");
        }

        Box::pin(async {})
    }));

    Ok(peer_connection)
}

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
                println!("Datachannel closed; Exit the read_loop: {}", err);
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
