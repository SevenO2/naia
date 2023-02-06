use std::{time::Duration, net::SocketAddr, sync::Arc};
// use std::{fmt::Display, io::Error as IoError};
use std::collections::HashMap;

use log::*;

use futures_channel::mpsc;
use futures_util::{pin_mut, select, join, FutureExt, StreamExt};
// use hashbrown::HashMap;
// use concurrent_queue::ConcurrentQueue;

use webrtc::data_channel::{data_channel_message::DataChannelMessage, data_channel_init::RTCDataChannelInit};

use naia_socket_shared::rtc_peer;
use rtc_peer::{
    SignalingChannel, RTCPeerConnection, RTCDataChannel, RTCIceServer, RTCSessionDescription, RTCSettingEngine
};

use naia_socket_shared::SocketConfig;
// use naia_socket_shared::{parse_server_url, url_to_socket_addr};
// use naia_socket_shared::signaling::{Signal, SignalingChannel};

use crate::{error::NaiaServerSocketError, server_addrs::ServerAddrs};

use anyhow::anyhow as err;
use webrtc::error::Error as RTCError;




const CLIENT_CHANNEL_SIZE: usize = 8;

/// A socket which communicates with clients using an underlying
/// unordered & unreliable network protocol
pub struct AsyncSocket {
    rtc_server: RtcServer,
    to_client_sender: mpsc::Sender<(SocketAddr, Box<[u8]>)>,
    to_client_receiver: mpsc::Receiver<(SocketAddr, Box<[u8]>)>,
}

impl AsyncSocket {
    /// Returns a new ServerSocket, listening at the given socket address
    pub async fn listen(server_addrs: ServerAddrs, config: SocketConfig) -> Self {
        let (to_client_sender, to_client_receiver) = mpsc::channel(CLIENT_CHANNEL_SIZE);

        let (rtc_server, incoming_sessions) = RtcServer::new(
            // server_addrs.webrtc_listen_addr,
            // url_to_socket_addr(&parse_server_url(&server_addrs.public_webrtc_url)),
        );

        // start_session_server(server_addrs, config, socket.rtc_server.session_endpoint());

        crate::executor::spawn(async {
            rtc_peer::stdio_signal_listener(incoming_sessions).await.expect("stdio_signal_listener");
        }).detach();
        // let server_clone = Arc::clone(&rtc_server);
        // crate::executor::spawn(async {
        //     session_accept_loop(server_clone, s2).await.expect("session_accept_loop");
        // }).detach();


        let socket = AsyncSocket {
            rtc_server,
            to_client_sender,
            to_client_receiver,
        };

        socket
    }

    pub async fn receive(&mut self) -> Result<(SocketAddr, Box<[u8]>), NaiaServerSocketError> {
        enum Next {
            FromClientMessage(Result<(SocketAddr, Box<[u8]>), RtcPeerError>),
            ToClientMessage((SocketAddr, Box<[u8]>)),
        }

        loop {
            let next = {
                let to_client_receiver_next = self.to_client_receiver.next().fuse();
                pin_mut!(to_client_receiver_next);

                let rtc_server = &mut self.rtc_server;
                let from_client_message_receiver_next = rtc_server.recv().fuse();
                pin_mut!(from_client_message_receiver_next);

                select! {
                    from_client_result = from_client_message_receiver_next => {
                        Next::FromClientMessage(
                            match from_client_result {
                                Ok(packet) => {
                                    Ok((packet.remote_addr, packet.copy_data()))
                                }
                                Err(err) => { Err(err) }
                            }
                        )
                    }
                    to_client_message = to_client_receiver_next => {
                        Next::ToClientMessage(
                            to_client_message.expect("to server message receiver closed")
                        )
                    }
                }
            };

            match next {
                Next::FromClientMessage(from_client_message) => match from_client_message {
                    Ok((address, payload)) => {
                        return Ok((address, payload));
                    }
                    Err(err) => {
                        return Err(NaiaServerSocketError::Wrapped(Box::new(err)));
                    }
                },
                Next::ToClientMessage((address, payload)) => {
                    let rtc_server = &mut self.rtc_server;
                    if rtc_server.send(RtcPacket {
                        is_string: false,
                        data: payload.into(),
                        remote_addr: address,
                    }).await.is_err() {
                        return Err(NaiaServerSocketError::SendError(address));
                    }
                }
            }
        }
    }

    pub fn sender(&self) -> mpsc::Sender<(SocketAddr, Box<[u8]>)> {
        self.to_client_sender.clone()
    }
}

// async fn session_accept_loop(server: Arc<RtcServer>, mut signaling: SignalingChannel)
// -> anyhow::Result<()> {
//     loop {
//         let offer = signaling.recv().await?;
//         let answer = server.accept_session(offer).await?;
//         signaling.send(answer)?;
//     }
//     // Ok(())
// }



#[derive(thiserror::Error, Debug)]
enum RtcPeerError {
    #[error("RtcPeer initialization failed: {0}")]
    WebRTC(#[from] RTCError),
    #[error("RtcPeer initialization failed: channel closed")]
    ChannelInit,
    #[error("RtcPeer recv() failed: channel closed")]
    ChannelReceive,
    #[error("RtcPeer not found for address: {0}")]
    Missing(SocketAddr),
    #[error("RtcPeer closed")]
    Closed,
    #[error(transparent)]
    Cloned(anyhow::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error)
}
// impl std::error::Error for RtcPeerError {}
type RtcPeerResult<T> = Result<T, RtcPeerError>;

impl Clone for RtcPeerError {
    fn clone(&self) -> Self {
        Self::Cloned(err!("cloned: {}", &self))
    }
}


struct RtcPeer {
    remote_addr: SocketAddr,
    connection: Arc<RTCPeerConnection>,
    channel: Arc<RTCDataChannel>,
}
// enum RtcPeer {
//     Ready {
//         remote_addr: SocketAddr,
//         connection: Arc<RTCPeerConnection>,
//         channel: Arc<DataChannel>,
//     },
//     Pending {
//         connection: Arc<RTCPeerConnection>,
//         channel: oneshot::Receiver<RTCResult<Arc<DataChannel>>>,
//     },
//     Failed(RtcPeerError),
//     Closed
// }
// impl RtcPeer {
//     fn data_channel(&self) -> RtcPeerResult<Arc<DataChannel>> {
//         match self {
//             Self::Ready { channel, .. } => Ok(Arc::clone(channel)),
//             Self::Pending => Err(RtcPeerError::Pending),
//             Self::Failed(err) => Err(err),
//             Self::Closed => Err(RtcPeerError::Closed),
//         }
//     }
// }

// #[derive(thiserror::Error, Debug)]
// enum ChannelQueuePeekErr {
//     #[error("ChannelQueue has no pending items (generated with peak())")]
//     NotReady,
//     // PrevPeekUnused,
//     #[error("ChannelQueue's underlying channel has closed. (how???)")]
//     ChannelClosed,
// }
// // impl std::error::Error for RtcPeerError {}
// type ChannelQueueResult<T> = Result<T, ChannelQueuePeekErr>;

type PeekReceiver<T> = futures_util::stream::Peekable<mpsc::Receiver<T>>;
struct ChannelQueue<T> {
    // next: Option<Option<T>>,
    recv: PeekReceiver<T>,
    // recv: mpsc::Receiver<T>,
    send: mpsc::Sender<T>,
}
impl<T> ChannelQueue<T> {
    fn bounded(buffer: usize) -> Self {
        let (send, recv) = mpsc::channel(buffer);
        Self {
            recv: recv.peekable(),
            send,
            // next: None
        }
    }
    // fn is_ready(&self) -> bool { self.recv.poll_peek().is_ready() }
    // fn take_next(&self) -> Option<T> { self.recv. }
    fn peek(&mut self) -> futures_util::stream::Peek<'_, mpsc::Receiver<T>> {
        std::pin::Pin::new(&mut self.recv).peek()
    }
    fn next(&mut self) -> futures_util::stream::Next<'_, PeekReceiver<T>> {
        self.recv.next()
    }

    // fn ready(&self) -> bool { self.next.is_some() }
    // fn take_next(&mut self) -> ChannelQueueResult<T> {
    //     match self.next.take() {
    //         Some(Some(val)) => Ok(val),
    //         Some(None) => Err(ChannelQueuePeekErr::ChannelClosed),
    //         None => Err(ChannelQueuePeekErr::NotReady),
    //     }
    // }
    // async fn next(&mut self) -> Option<T> {
    //     self.recv.next().await
    // }
    fn sender(&self) -> mpsc::Sender<T> { self.send.clone() }
}
// impl<T: Clone> ChannelQueue<T> {
//     async fn peek(&mut self) -> Option<T> {
//         let peek = self.next.get_or_insert(None);

//         if peek.is_none() {
//             *peek = self.recv.next().await;
//         }

//         peek.as_ref().cloned()
//     }
// }

#[derive(Clone)]
struct RtcPacket {
    is_string: bool,
    data: bytes::Bytes,
    remote_addr: SocketAddr,
}
impl RtcPacket {
    fn copy_data(&self) -> Box<[u8]> {
        // Vec::from(self.data).into_boxed_slice() // TODO: does this take? alias?
        self.data.to_vec().into_boxed_slice()
    }
    // fn fake_socket(&self) -> SocketAddr {
    //     SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), self.remote_id.get())
    // }
}

// struct RtcSessionListener {}

struct RtcServer {
    // inner: Vec<RtcPeer>
    active_sessions: HashMap<SocketAddr, RtcPeerResult<RtcPeer>>,
    new_sessions: ChannelQueue<RtcPeerResult<RtcPeer>>,
    periodic_timer: async_io::Timer, // TODO: see if this starves other events under load
    incoming_offers: SignalingChannel,
    incoming_packets: ChannelQueue<RtcPacket>,
}

const PERIODIC_TIMER_INTERVAL: Duration = Duration::from_secs(1);
const MAX_UDP_PAYLOAD_SIZE: usize = 1200;
impl RtcServer {
    fn new() -> (Self, SignalingChannel) {
        let (s1, s2) = SignalingChannel::pair(1);
        (
            Self { // async?
                active_sessions: HashMap::new(),
                new_sessions: ChannelQueue::bounded(3),
                periodic_timer: async_io::Timer::interval(PERIODIC_TIMER_INTERVAL),
                incoming_offers: s1,
                incoming_packets: ChannelQueue::bounded(12),
            },
            s2
        )
    }

    async fn accept_session(&self, offer: RTCSessionDescription) -> RtcPeerResult<RTCSessionDescription> {
        let settings = RTCSettingEngine::default();
        // settings.detatch_data_channels();

        let (peer, done_rx) = rtc_peer::new_peer(
            fallback_ice_servers(),
            Some(settings)
        ).await?;

        let (mut sig1, sig2) = SignalingChannel::pair(1);
        let (setup_fn, mut channel_rx) = data_channel_setup_fn(self.incoming_packets.sender());
        let peer_clone = Arc::clone(&peer);

        sig1.send(offer);
        let answer = match join!(
            rtc_peer::join_channel(peer_clone, sig2, setup_fn),
            sig1.recv(),
        ) {
            (Ok(_), Ok(answer)) => answer,
            (Err(err), _) => return Err(err.into()), // FIXME
            (_, Err(err)) => return Err(err.into()),
        };

        let mut new_sessions = self.new_sessions.sender();
        crate::executor::spawn(async move {
            new_sessions.try_send(match channel_rx.next().await {
                Some((data_channel, remote_addr)) => {
                    info!("Connected to {}", remote_addr);

                    Ok(RtcPeer {
                        remote_addr,
                        connection: peer,
                        channel: data_channel,
                    })
                }
                None => Err(RtcPeerError::ChannelInit),
            }).expect("trigger newly established");
        }).detach();

        Ok(answer)
    }

    pub async fn send(
        &mut self,
        packet: RtcPacket,
    ) -> RtcPeerResult<()> {
        let peer = self.active_sessions.get(&packet.remote_addr)
            .ok_or(RtcPeerError::Missing(packet.remote_addr))?
            .as_ref().map_err(Clone::clone)?;
        if !packet.is_string {
            peer.channel.send(&packet.data).await?;
        } else {
            unimplemented!()
        }
        Ok(())
    }

    pub async fn recv(&mut self) -> RtcPeerResult<RtcPacket> {
        while !futures_util::poll!(self.incoming_packets.peek()).is_ready() {
            self.process().await?;
        }

        let std::task::Poll::Ready(res) = futures_util::poll!(self.incoming_packets.next()) else {
            panic!("next pending after peek???");
        };
        res.ok_or(RtcPeerError::ChannelReceive)
    }

    async fn process(&mut self) -> RtcPeerResult<()> {
        enum Next {
            IncomingOffer(RTCSessionDescription),
            NewlyEstablished(RtcPeerResult<RtcPeer>),
            IncomingPacket,//(RtcPacket),
            PeriodicTimer,
        }

        let next = {
            let offer_next = self.incoming_offers.recv().fuse();
            pin_mut!(offer_next);
            let established_next = self.new_sessions.next().fuse();
            pin_mut!(established_next);
            let packet_peek = self.incoming_packets.peek().fuse();
            pin_mut!(packet_peek);
            let timer_next = self.periodic_timer.next().fuse();
            pin_mut!(timer_next);

            select! {
                offer = offer_next => {
                    Next::IncomingOffer(offer.expect("next incoming offer."))
                }
                newly_established = established_next => {
                    Next::NewlyEstablished(
                        newly_established.expect("next newly established.")
                    )
                }
                packet = packet_peek => {
                    packet.expect("next incoming packet.");
                    Next::IncomingPacket
                }
                _ = timer_next => {
                    Next::PeriodicTimer
                }
            }
        };

        match next {
            Next::IncomingOffer(offer) => {
                self.incoming_offers.send(self.accept_session(offer).await?).expect("incoming offer: send answer.");
            }
            Next::NewlyEstablished(newly_established) => {
                match newly_established {
                    Ok(peer) => {
                        let addr = peer.remote_addr;
                        if self.active_sessions.insert(addr, Ok(peer)).is_some() {
                            warn!("new connection has same address as existing connection: {}", addr);
                        }
                    }
                    Err(err) => warn!("connection establishment failed: {}", err),
                }
            }
            Next::IncomingPacket => {
                // NOTE: this will get processed by the recv fn which called this fn

                // TODO: ??
                // if len > MAX_UDP_PAYLOAD_SIZE {
                //     return Err(IoError::new(
                //         std::io::ErrorKind::Other,
                //         "failed to read entire datagram from socket",
                //     ));
                // }

                // // self.send_outgoing().await?;
            }
            Next::PeriodicTimer => {
                // TODO: do I actually need this or does webrtc take care of it already?

                // self.timeout_clients();
                // self.send_periodic_packets();
                // // self.send_outgoing().await?;
            }
        }

        Ok(())
    }

    fn timeout_clients(&mut self) { todo!() }
    fn send_periodic_packets(&mut self) { todo!() }
}


fn data_channel_init() -> Option<RTCDataChannelInit> { None }
fn data_channel_setup_fn(mut message_queue: mpsc::Sender<RtcPacket>) -> (
    impl FnOnce(Arc<RTCPeerConnection>, Arc<RTCDataChannel>) + Clone,
    mpsc::Receiver<(Arc<RTCDataChannel>, SocketAddr)>
) {
    let (mut tx, rx) = mpsc::channel(1);
    return (
        |peer_connection, data_channel| {
            let d_label = data_channel.label().to_owned();
            let d_id = data_channel.id();
            info!("New DataChannel {} {}", d_label, d_id);

            let d2 = Arc::clone(&data_channel);
            let d_label2 = d_label.clone();
            // Register channel opening handling
            data_channel.on_open(Box::new(move || {
                info!("Data channel '{}'-'{}' open.",
                    d_label, d_id);
                debug!("protocol: {}, ordered: {}, pkt_life: {}, re_tx: {}",
                    d2.protocol(),
                    d2.ordered(),
                    d2.max_packet_lifetime(),
                    d2.max_retransmits());

                Box::pin(async move {
                    let remote_addr = rtc_peer::get_nominated_candidate(&peer_connection)
                        .await.expect("get_nominated_candidate")
                        .remote_addr();

                    d2.on_message(Box::new(move |msg: DataChannelMessage| {
                        debug!("Received Message from DataChannel '{}'", d_label2);
                        if let Err(e) = message_queue.try_send(RtcPacket {
                            is_string: msg.is_string,
                            remote_addr,
                            data: msg.data, // TODO: copy audit
                        }) {
                            warn!("Message Queue full... dropping packet!");
                        }
                        Box::pin(async {})
                    }));

                    tx.try_send((d2, remote_addr)).unwrap();
                    // tx.send(match d2.detach().await).unwrap() // TODO: log err
                })
            }));

            // Register text message handling
            // ...
        },
        rx
    );
}
fn fallback_ice_servers() -> Vec<RTCIceServer> {
    vec![RTCIceServer { // backup
        urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        ..Default::default()
    }]
}
