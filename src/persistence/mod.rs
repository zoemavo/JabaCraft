//! Versioned world-save boundaries for persistent runtime state.

use std::{
    fs,
    path::{Path, PathBuf},
};

use bevy::{app::AppExit, prelude::*};

use crate::game_time::GameTime;

const WORLD_SAVE_VERSION: u32 = 1;
const AUTOSAVE_INTERVAL_SECONDS: f32 = 30.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) struct PersistenceLoadSet;

/// Identifies the save slot used by persistence systems.
#[derive(Debug, Resource)]
pub struct PersistenceSettings {
    pub world_name: String,
    pub save_directory: PathBuf,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            world_name: "default-world".to_owned(),
            save_directory: PathBuf::from("saves"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WorldSave {
    version: u32,
    game_time_elapsed_days: f64,
}

impl WorldSave {
    fn capture(game_time: &GameTime) -> Self {
        Self {
            version: WORLD_SAVE_VERSION,
            game_time_elapsed_days: game_time.elapsed_days(),
        }
    }

    fn encode(self) -> String {
        format!(
            "version={}\ngame_time_elapsed_days={:.15}\n",
            self.version, self.game_time_elapsed_days
        )
    }

    fn decode(contents: &str) -> Result<Self, String> {
        let mut version = None;
        let mut game_time_elapsed_days = None;
        for line in contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid world-save line: {line}"));
            };
            match key.trim() {
                "version" => {
                    version = Some(
                        value
                            .trim()
                            .parse::<u32>()
                            .map_err(|error| format!("invalid save version: {error}"))?,
                    );
                }
                "game_time_elapsed_days" => {
                    let value = value
                        .trim()
                        .parse::<f64>()
                        .map_err(|error| format!("invalid game time: {error}"))?;
                    if !value.is_finite() || value < 0.0 {
                        return Err("game time must be finite and non-negative".to_owned());
                    }
                    game_time_elapsed_days = Some(value);
                }
                _ => {}
            }
        }

        let version = version.ok_or_else(|| "world save has no version".to_owned())?;
        if version != WORLD_SAVE_VERSION {
            return Err(format!(
                "unsupported world-save version {version}; expected {WORLD_SAVE_VERSION}"
            ));
        }
        Ok(Self {
            version,
            game_time_elapsed_days: game_time_elapsed_days
                .ok_or_else(|| "world save has no game time".to_owned())?,
        })
    }
}

/// Owns world serialization, loading, autosaving, and shutdown saves.
pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PersistenceSettings>()
            .add_systems(Startup, load_world_time.in_set(PersistenceLoadSet))
            .add_systems(Last, (autosave_world_time, save_world_time_on_exit));
    }
}

fn load_world_time(settings: Res<PersistenceSettings>, mut game_time: ResMut<GameTime>) {
    let path = match world_save_path(&settings) {
        Ok(path) => path,
        Err(error) => {
            error!("Cannot resolve world save: {error}");
            return;
        }
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            info!("Starting new world save at {}", path.display());
            return;
        }
        Err(error) => {
            error!("Failed to read {}: {error}", path.display());
            return;
        }
    };
    match WorldSave::decode(&contents) {
        Ok(save) => {
            *game_time = GameTime::from_elapsed_days(save.game_time_elapsed_days);
            info!(
                "Loaded day {} at {:.1}% from {}",
                game_time.day_number(),
                game_time.day_fraction() * 100.0,
                path.display()
            );
        }
        Err(error) => error!("Failed to parse {}: {error}", path.display()),
    }
}

fn autosave_world_time(
    time: Res<Time>,
    settings: Res<PersistenceSettings>,
    game_time: Res<GameTime>,
    mut elapsed: Local<f32>,
) {
    *elapsed += time.delta_secs().max(0.0);
    if *elapsed < AUTOSAVE_INTERVAL_SECONDS {
        return;
    }
    *elapsed %= AUTOSAVE_INTERVAL_SECONDS;
    save_world_time(&settings, &game_time);
}

fn save_world_time_on_exit(
    mut exits: MessageReader<AppExit>,
    settings: Res<PersistenceSettings>,
    game_time: Res<GameTime>,
) {
    if exits.read().next().is_some() {
        save_world_time(&settings, &game_time);
    }
}

fn save_world_time(settings: &PersistenceSettings, game_time: &GameTime) {
    let path = match world_save_path(settings) {
        Ok(path) => path,
        Err(error) => {
            error!("Cannot resolve world save: {error}");
            return;
        }
    };
    if let Err(error) = write_world_save(&path, WorldSave::capture(game_time)) {
        error!("Failed to save {}: {error}", path.display());
    }
}

fn world_save_path(settings: &PersistenceSettings) -> Result<PathBuf, String> {
    let valid_name = !settings.world_name.is_empty()
        && settings
            .world_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if !valid_name {
        return Err("world name may contain only ASCII letters, digits, '-' and '_'".to_owned());
    }
    Ok(settings
        .save_directory
        .join(&settings.world_name)
        .join("world.save"))
}

fn write_world_save(path: &Path, save: WorldSave) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "save path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("save.tmp");
    fs::write(&temporary, save.encode())?;
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_save_round_trips_game_time() {
        let save = WorldSave::capture(&GameTime::from_elapsed_days(42.625));

        assert_eq!(WorldSave::decode(&save.encode()), Ok(save));
    }

    #[test]
    fn malformed_or_future_world_saves_are_rejected() {
        assert!(WorldSave::decode("not a save").is_err());
        assert!(WorldSave::decode("version=999\ngame_time_elapsed_days=1.0\n").is_err());
        assert!(WorldSave::decode("version=1\ngame_time_elapsed_days=-1\n").is_err());
    }

    #[test]
    fn world_names_cannot_escape_the_save_directory() {
        let settings = PersistenceSettings {
            world_name: "../outside".to_owned(),
            save_directory: PathBuf::from("saves"),
        };

        assert!(world_save_path(&settings).is_err());
    }
}
