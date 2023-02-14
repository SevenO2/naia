use std::default::Default;

use bevy::ecs::{entity::Entity, prelude::Resource, prelude::Component};

use naia_bevy_client::CommandHistory;

use naia_bevy_demo_shared::protocol::KeyCommand;

#[derive(Component)]
pub struct Ping {
    pub rtt: f32,
    pub jitter: f32,
}

pub struct OwnedEntity {
    pub confirmed: Entity,
    pub predicted: Entity,
}

impl OwnedEntity {
    pub fn new(confirmed_entity: Entity, predicted_entity: Entity) -> Self {
        OwnedEntity {
            confirmed: confirmed_entity,
            predicted: predicted_entity,
        }
    }
}

#[derive(Default, Resource)]
pub struct Global {
    pub owned_entity: Option<OwnedEntity>,
    pub queued_command: Option<KeyCommand>,
    pub command_history: CommandHistory<KeyCommand>,
}
