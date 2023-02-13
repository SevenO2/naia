use std::time::Duration;

use naia_shared::{LinkConditionerConfig, SharedConfig, SocketConfig, ConnectionConfig, PingConfig};

use crate::channels::{Channels, CHANNEL_CONFIG};

pub fn shared_config() -> SharedConfig<Channels> {
    // Set tick rate to ~60 FPS
    let tick_interval = Some(Duration::from_millis(20));

     // let link_condition = None;
    let link_condition = Some(LinkConditionerConfig::good_condition());
    //  let link_condition = Some(LinkConditionerConfig {
    //      incoming_latency: 500,
    //      incoming_jitter: 1,
    //      incoming_loss: 0.0,
    //  });
    SharedConfig::new(
        SocketConfig::new(link_condition, None),
        CHANNEL_CONFIG,
        tick_interval,
        None,
    )
}

pub fn shared_connection_config() -> ConnectionConfig {
    ConnectionConfig {
        ping: PingConfig {
            rtt_initial_estimate: Duration::from_millis(10),
            jitter_initial_estimate: Duration::from_millis(3),
            rtt_smoothing_factor: 0.1,
            ..Default::default()
        },
        ..Default::default()
    }
}
