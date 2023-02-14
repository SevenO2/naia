use bevy::{ecs::system::Commands, log::info, prelude::Camera2dBundle};
use bevy::prelude::*;

use naia_bevy_client::Client;

use naia_bevy_demo_shared::{
    protocol::{Auth, Protocol},
    Channels,
};

use crate::resources::*;

pub fn init(mut commands: Commands, mut client: Client<Protocol, Channels>, asset_server: Res<AssetServer>) {
    info!("Naia Bevy Client Demo started");

    client.auth(Auth::new("charlie", "12345"));
    client.connect("http://127.0.0.1:14191");

    // Setup Camera
    commands.spawn(Camera2dBundle::default());

    // Setup Colors
    commands.init_resource::<Global>();

    // ...
    let font = asset_server.load("FiraMono-Medium.ttf");
    commands.spawn(
        TextBundle::from_section(
            "ping: --",
            TextStyle {
                font: font,
                font_size: 50.0,
                color: Color::WHITE,
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            position: UiRect {
                top: Val::Px(5.0),
                left: Val::Px(15.0),
                ..default()
            },
            ..default()
        }),
    ).insert(Ping { rtt: 0.0, jitter: 0.0 });
}
