use crate::{profiles, profiles::Profiles, Game};
use anyhow::{anyhow, Result};
use itertools::Itertools;
use slint::SharedString;
use std::{fs, io, path::PathBuf, time::SystemTime};

pub fn is_file_not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<io::Error>().map_or(false, |io_err| io_err.kind() == io::ErrorKind::NotFound)
}
pub struct SaveEntry {
    pub name: SharedString,
    pub path: PathBuf,
    pub timestamp: Option<SystemTime>,
}

impl SaveEntry {
    fn new(path: PathBuf, timestamp: Option<SystemTime>) -> Self {
        let name = path
            .file_name()
            .map_or("INVALID".into(), |name| name.to_string_lossy().as_ref().into());
        SaveEntry { name, path, timestamp }
    }

    fn from_dir_entry(entry: fs::DirEntry) -> Self {
        let path = entry.path();
        let timestamp = entry
            .metadata()
            .and_then(|md| md.modified())
            .map_err(|err| {
                log::warn!(
                    "Cannot read timestamp of save {}: {err}. Will be treated as the oldest.",
                    path.to_string_lossy()
                );
            })
            .ok();
        Self::new(path, timestamp)
    }
}
pub struct SaveLoad {
    pub current_version: semver::Version,
    pub project_dirs: Option<directories::ProjectDirs>,
    /// Kept from oldest to newest.
    pub known_saves: Vec<SaveEntry>,
}

impl SaveLoad {
    pub fn new() -> Self {
        let mut this = Self {
            current_version: semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap(),
            project_dirs: directories::ProjectDirs::from("pl", "Capricornus", "Gielo"),
            known_saves: vec![],
        };
        match this.reload_saves_list() {
            Err(err) if !is_file_not_found(&err) => {
                log::error!("Failed to load initial saves list: {err}")
            }
            _ => {}
        }
        this
    }

    pub fn project_dirs(&self) -> Result<&directories::ProjectDirs> {
        self.project_dirs.as_ref().ok_or(anyhow!("Cannot retrieve home directory from the system"))
    }
    fn save_dir(&self) -> Result<PathBuf> {
        Ok(self.project_dirs()?.data_local_dir().join("save"))
    }

    fn save_filename(&self, game: &Game, datetime: chrono::DateTime<chrono::Local>) -> String {
        format!(
            "{teams}, {situation}, {score}, {datetime}.sav",
            teams = format_args!("{} - {}", game.teams.a.name, game.teams.b.name),
            situation = match (game.current_end_number(), game.current_turn_number()) {
                (Some(end), Some(turn)) => format!("end {end} stone {turn}"),
                (Some(end), None) => format!("end {end} finished"),
                (None, _) => format!("finished"),
            },
            score = format_args!("{}-{}", game.score.a, game.score.b),
            datetime = datetime.format("%F %T"),
        )
    }
    pub fn save_game(&mut self, game: &Game) -> Result<&SaveEntry> {
        let filename = self.save_filename(game, chrono::Local::now());
        let save_dir = self.save_dir()?;
        fs::create_dir_all(&save_dir)?;
        let path = save_dir.join(filename);
        let file = fs::File::create(&path)?;
        bincode2::serialize_into(&file, game)?;
        file.sync_all()?;
        let timestamp = SystemTime::now();
        self.known_saves.push(SaveEntry::new(path, Some(timestamp)));
        Ok(self.known_saves.last().unwrap())
    }

    pub fn load_game(&self, save_index: usize) -> Result<Game> {
        let save = self
            .known_saves
            .get(save_index)
            .ok_or_else(|| anyhow!("Wrong save index: {}", save_index))?;
        let file = fs::File::open(&save.path)?;
        Ok(bincode2::deserialize_from(file)?)
    }

    pub fn reload_saves_list(&mut self) -> Result<()> {
        self.known_saves = fs::read_dir(self.save_dir()?)?
            .filter_map(|save| match save {
                Ok(save) => Some(SaveEntry::from_dir_entry(save)),
                Err(err) => {
                    log::error!("Could not read info about a save: {err}!");
                    None
                }
            })
            .sorted_by_key(|save| save.timestamp)
            .collect();
        Ok(())
    }

    fn profiles_path(&self) -> Result<PathBuf> {
        Ok(self.project_dirs()?.config_dir().join("profiles.yaml"))
    }
    pub fn load_profiles(&self) -> Profiles {
        match self.profiles_path() {
            Ok(path) => Profiles::load_from_or_create_file(&path),
            Err(err) => {
                log::error!("Failed to discover path for profiles file: {err}.");
                profiles::builtin::create()
            }
        }
    }
}

impl Default for SaveLoad {
    fn default() -> Self {
        Self::new()
    }
}
