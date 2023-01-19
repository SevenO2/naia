//! # Naia Server Socket
//! Provides an abstraction of a Socket capable of sending/receiving to many
//! clients, using either an underlying UdpSocket or a service that can
//! communicate via unreliable WebRTC datachannels

#![deny(
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

extern crate log;

#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(all(feature = "webrtc-lite"))] {
        mod async_socket_lite;
        use async_socket_lite::AsyncSocket;
        mod session_lite;
    }
    else if #[cfg(all(feature = "webrtc-full"))] {
        mod async_socket_full;
        use async_socket_full::AsyncSocket;
        mod session_full;
    }
    else {
         compile_error!("Naia Server Socket on Native requires either the 'webrtc-lite' OR 'webrtc-full' feature to be enabled, you must pick one.");
    }
}

mod conditioned_packet_receiver;
mod error;
mod io;
mod packet_receiver;
mod packet_sender;
mod server_addrs;
mod socket;

/// Executor for Server
pub mod executor;

pub use error::NaiaServerSocketError;
pub use naia_socket_shared as shared;
pub use packet_receiver::PacketReceiver;
pub use packet_sender::PacketSender;
pub use server_addrs::ServerAddrs;
pub use socket::Socket;
