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
            .insert_resource(ClearColor(Color::srgb(0.53, 0.72, 0.9)))
            .add_systems(Startup, log_startup);
    }
}

fn log_startup(metadata: Res<AppMetadata>) {
    info!("Starting {}", metadata.title);
}
