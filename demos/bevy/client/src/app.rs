use bevy::{app::*, DefaultPlugins};
use bevy::prelude::*;

use naia_bevy_client::{Client, ClientConfig, Plugin as ClientPlugin, Stage};

use naia_bevy_demo_shared::{protocol::Protocol, shared_config, shared_connection_config, Channels};

use crate::systems::{events, init, input, sync, tick};
use crate::resources::*;

pub fn run() {
    App::default()
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugin(ClientPlugin::<Protocol, Channels>::new(
            ClientConfig {
                connection: shared_connection_config(),
                ..default()
            },
            shared_config(),
        ))
        // Startup System
        .add_startup_system(init)
        // Shutdown Systems
        .add_system_to_stage(Stage::Frame, close_on_esc)
        .add_system_to_stage(Stage::Tick, shutdown.after(close_on_esc).after(sync))
        // Realtime Gameplay Loop
        .add_system_to_stage(Stage::Connection, events::connect_event)
        .add_system_to_stage(Stage::Disconnection, events::disconnect_event)
        .add_system_to_stage(Stage::Rejection, events::reject_event)
        .add_system_to_stage(Stage::ReceiveEvents, events::spawn_entity_event)
        .add_system_to_stage(Stage::ReceiveEvents, events::insert_component_event)
        .add_system_to_stage(Stage::ReceiveEvents, events::update_component_event)
        .add_system_to_stage(Stage::ReceiveEvents, events::receive_message_event)
        .add_system_to_stage(Stage::Frame, input)
        .add_system_to_stage(Stage::PostFrame, sync)
        // Gameplay Loop on Tick
        .add_system_to_stage(Stage::Tick, tick)
        .add_system_to_stage(Stage::Tick, ping)
        // Run App
        .run();

    // std::thread::sleep(std::time::Duration::from_millis(200));
}


fn close_on_esc(keys: Res<Input<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if keys.pressed(KeyCode::Escape) {
        exit.send(AppExit);
    }
}
fn shutdown(mut client: Client<Protocol, Channels>, exit: EventReader<AppExit>) {
    if !exit.is_empty() {
        exit.clear();
        client.disconnect();
    }
}

fn ping(client: Client<Protocol, Channels>, mut ping: Query<(&mut Text, &mut Ping)>) {
    if !client.is_connected() { return }
    let (mut text, _ping) = ping.single_mut();
    let rtt = client.rtt() as u32;
    let jitter = client.jitter() as u32;
    text.sections[0].value = format!("ping: {rtt} +/- {jitter}");
}