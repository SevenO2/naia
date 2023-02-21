use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::SinkExt;
use smol::channel;

use naia_socket_shared::SocketConfig;

use crate::{executor, io::Io};
use crate::AsyncSocket;

use super::{
    conditioned_packet_receiver::ConditionedPacketReceiverImpl,
    packet_receiver::{PacketReceiver, PacketReceiverImpl, PacketReceiverTrait},
    packet_sender::PacketSender,
    server_addrs::ServerAddrs,
};

/// Socket is able to send and receive messages from remote Clients
pub struct Socket {
    config: SocketConfig,
    io: Option<Io>,
}

impl Socket {
    /// Create a new Socket
    pub fn new(config: &SocketConfig) -> Self {
        Socket {
            config: config.clone(),
            io: None,
        }
    }

    /// Listens on the Socket for incoming communication from Clients
    pub fn listen(&mut self, server_addrs: &ServerAddrs) {
        if self.io.is_some() {
            panic!("Socket already listening!");
        }

        // Set up receiver loop
        let (from_client_sender, from_client_receiver) = channel::unbounded();
        let (sender_sender, sender_receiver) = channel::bounded(1);

        let server_addrs_clone = server_addrs.clone();
        let config_clone = self.config.clone();

        executor::spawn(async move {
            // unsafe {
            //     signal_hook_registry::register(
            //         signal_hook::consts::SIGINT,
            //         || panic!()
            //     ).unwrap();
            // }


            // Create async socket
            let mut async_socket = AsyncSocket::listen(server_addrs_clone, config_clone).await;

            sender_sender.send(async_socket.sender()).await.unwrap();
            //TODO: handle result..

            let term = Arc::new(AtomicBool::new(false));
            // signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term)).unwrap();
            while !term.load(Ordering::Relaxed) {
            // loop {
                let out_message = async_socket.receive().await;
                from_client_sender.send(out_message).await.unwrap();
                //TODO: handle result..
            }

            // drop(async_socket);
        })
        .detach();

        // Set up sender loop
        let (to_client_sender, to_client_receiver) = channel::unbounded();

        executor::spawn(async move {
            // Create async socket
            let mut async_sender = sender_receiver.recv().await.unwrap();

            loop {
                if let Ok(msg) = to_client_receiver.recv().await {
                    async_sender.send(msg).await.unwrap();
                    //TODO: handle result..
                }
            }
        })
        .detach();

        let conditioner_config = self.config.link_condition.clone();

        let receiver: Box<dyn PacketReceiverTrait> = match &conditioner_config {
            Some(config) => Box::new(ConditionedPacketReceiverImpl::new(
                from_client_receiver,
                config,
            )),
            None => Box::new(PacketReceiverImpl::new(from_client_receiver)),
        };
        let sender = PacketSender::new(to_client_sender);

        self.io = Some(Io {
            packet_sender: sender,
            packet_receiver: PacketReceiver::new(receiver),
        });
    }

    /// Gets a PacketSender which can be used to send packets through the Socket
    pub fn packet_sender(&self) -> PacketSender {
        return self
            .io
            .as_ref()
            .expect("Socket is not listening yet! Call Socket.listen() before this.")
            .packet_sender
            .clone();
    }

    /// Gets a PacketReceiver which can be used to receive packets from the
    /// Socket
    pub fn packet_receiver(&self) -> PacketReceiver {
        return self
            .io
            .as_ref()
            .expect("Socket is not listening yet! Call Socket.listen() before this.")
            .packet_receiver
            .clone();
    }
}
