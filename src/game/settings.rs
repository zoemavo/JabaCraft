//! Application preferences; intentionally independent of world metadata.
use crate::{
    generation::GenerationSettings,
    player::{PlayerCamera, PlayerSettings},
};
use bevy::{
    audio::{GlobalVolume, Volume},
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future, poll_once},
    window::{PresentMode, PrimaryWindow},
};
use serde::{Deserialize, Serialize};

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub render_distance: i32,
    pub mouse_sensitivity: f32,
    pub fov: f32,
    pub master_volume: f32,
    pub vsync: bool,
}
impl Default for UserSettings {
    fn default() -> Self {
        Self {
            render_distance: 8,
            mouse_sensitivity: 0.0025,
            fov: 60.0,
            master_volume: 1.0,
            vsync: true,
        }
    }
}
impl UserSettings {
    fn sanitize(&mut self) {
        self.render_distance = self.render_distance.clamp(2, 16);
        self.mouse_sensitivity = finite_clamp(self.mouse_sensitivity, 0.0005, 0.01, 0.0025);
        self.fov = finite_clamp(self.fov, 40.0, 110.0, 60.0);
        self.master_volume = finite_clamp(self.master_volume, 0.0, 1.0, 1.0);
    }
}
fn finite_clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
#[derive(Resource)]
pub struct ConfigPath(pub std::path::PathBuf);
impl Default for ConfigPath {
    fn default() -> Self {
        Self("config/settings.ron".into())
    }
}
#[derive(Resource, Default)]
pub(super) struct ConfigWriter {
    task: Option<Task<Result<(), String>>>,
    pending: Option<UserSettings>,
    pub error: Option<String>,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<UserSettings>()
        .init_resource::<ConfigPath>()
        .init_resource::<ConfigWriter>()
        .add_systems(Startup, load)
        .add_systems(Update, (apply, apply_audio, persist).chain())
        .add_systems(Last, shutdown.after(super::pause::departure));
}
fn load(
    path: Res<ConfigPath>,
    mut settings: ResMut<UserSettings>,
    mut writer: ResMut<ConfigWriter>,
) {
    match std::fs::read_to_string(&path.0) {
        Ok(text) => match ron::from_str::<UserSettings>(&text) {
            Ok(mut loaded) => {
                loaded.sanitize();
                *settings = loaded;
            }
            Err(e) => {
                writer.error = Some(format!("Cannot read settings: {e}"));
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => writer.error = Some(format!("Cannot read settings: {e}")),
    }
}
fn apply(
    settings: Res<UserSettings>,
    mut generation: ResMut<GenerationSettings>,
    mut player: ResMut<PlayerSettings>,
    mut cameras: Query<&mut Projection, With<PlayerCamera>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut volume: ResMut<GlobalVolume>,
) {
    if !settings.is_changed() {
        return;
    }
    if generation.render_distance_chunks != settings.render_distance {
        generation.render_distance_chunks = settings.render_distance;
    }
    player.mouse_sensitivity = settings.mouse_sensitivity;
    for mut camera in &mut cameras {
        if let Projection::Perspective(p) = &mut *camera {
            p.fov = settings.fov.to_radians();
        }
    }
    for mut window in &mut windows {
        let desired = if settings.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        if window.present_mode != desired {
            window.present_mode = desired;
        }
    }
    volume.volume = Volume::Linear(settings.master_volume);
}
fn apply_audio(
    settings: Res<UserSettings>,
    mut sinks: Query<(
        &bevy::audio::PlaybackSettings,
        Option<&mut bevy::audio::AudioSink>,
        Option<&mut bevy::audio::SpatialAudioSink>,
    )>,
) {
    if !settings.is_changed() {
        return;
    }
    use bevy::audio::AudioSinkPlayback;
    for (playback, sink, spatial) in &mut sinks {
        let volume = playback.volume * Volume::Linear(settings.master_volume);
        if let Some(mut sink) = sink {
            sink.set_volume(volume);
        }
        if let Some(mut sink) = spatial {
            sink.set_volume(volume);
        }
    }
}

fn persist(
    settings: Res<UserSettings>,
    path: Res<ConfigPath>,
    mut writer: ResMut<ConfigWriter>,
    mut started: Local<bool>,
) {
    if !*started {
        *started = true;
        return;
    }
    if settings.is_changed() {
        writer.pending = Some(settings.clone());
    }
    if let Some(result) = writer
        .task
        .as_mut()
        .and_then(|task| future::block_on(poll_once(task)))
    {
        writer.task = None;
        writer.error = result.err();
    }
    if writer.task.is_none()
        && let Some(snapshot) = writer.pending.take()
    {
        let path = path.0.clone();
        writer.task = Some(
            IoTaskPool::get()
                .spawn(async move { write(&path, &snapshot).map_err(|e| e.to_string()) }),
        );
    }
}
fn shutdown(
    mut exits: MessageReader<bevy::app::AppExit>,
    path: Res<ConfigPath>,
    mut writer: ResMut<ConfigWriter>,
) {
    if exits.read().count() == 0 {
        return;
    }
    if let Some(task) = writer.task.take() {
        writer.error = future::block_on(task).err();
    }
    if let Some(snapshot) = writer.pending.take() {
        writer.error = write(&path.0, &snapshot).err().map(|e| e.to_string());
    }
}

fn write(path: &std::path::Path, settings: &UserSettings) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("config path needs a parent"))?;
    std::fs::create_dir_all(parent)?;
    let text = ron::ser::to_string_pretty(settings, ron::ser::PrettyConfig::default())
        .map_err(std::io::Error::other)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(text.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_round_trip_and_limits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config/settings.ron");
        let mut settings = UserSettings {
            render_distance: 999,
            fov: f32::NAN,
            ..default()
        };
        settings.sanitize();
        assert_eq!(settings.render_distance, 16);
        assert_eq!(settings.fov, 60.0);
        write(&path, &settings).unwrap();
        let loaded: UserSettings = ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(loaded, settings);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    #[test]
    fn changing_preferences_updates_live_world_camera_and_window() {
        let mut app = App::new();
        app.init_resource::<UserSettings>()
            .init_resource::<PlayerSettings>()
            .init_resource::<GenerationSettings>()
            .init_resource::<GlobalVolume>()
            .add_systems(Update, apply);
        let camera = app
            .world_mut()
            .spawn((
                PlayerCamera,
                Projection::from(PerspectiveProjection::default()),
            ))
            .id();
        let window = app
            .world_mut()
            .spawn((PrimaryWindow, Window::default()))
            .id();
        app.update();
        *app.world_mut().resource_mut::<UserSettings>() = UserSettings {
            render_distance: 12,
            mouse_sensitivity: 0.004,
            fov: 90.0,
            master_volume: 0.25,
            vsync: false,
        };
        app.update();
        assert_eq!(
            app.world()
                .resource::<GenerationSettings>()
                .render_distance_chunks,
            12
        );
        assert_eq!(
            app.world().resource::<PlayerSettings>().mouse_sensitivity,
            0.004
        );
        let Projection::Perspective(projection) = app.world().get::<Projection>(camera).unwrap()
        else {
            panic!()
        };
        assert_eq!(projection.fov, 90.0_f32.to_radians());
        assert_eq!(
            app.world().get::<Window>(window).unwrap().present_mode,
            PresentMode::AutoNoVsync
        );
        assert_eq!(
            app.world().resource::<GlobalVolume>().volume,
            Volume::Linear(0.25)
        );
    }
}
