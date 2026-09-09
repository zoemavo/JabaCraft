use bevy::prelude::*;

/// The title used by the native game window.
pub const APP_TITLE: &str = "rustcraft";

/// App-wide metadata shared by future diagnostics and UI systems.
#[derive(Debug, Resource)]
pub struct AppMetadata {
    pub title: &'static str,
}

impl Default for AppMetadata {
    fn default() -> Self {
        Self { title: APP_TITLE }
    }
}

/// Registers resources owned by the application shell.
pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppMetadata>()
            // Matches the procedural skybox horizon and remains as a graceful
            // fallback while its generated cubemap is uploaded.
            .insert_resource(ClearColor(crate::scene::SKY_COLOR))
            .add_systems(Startup, log_startup);
    }
}

fn log_startup(metadata: Res<AppMetadata>) {
    info!("Starting {}", metadata.title);
}
