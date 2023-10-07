use crate::{game::Game, profiles, profiles::Profiles};
use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use slint::SharedString;
use std::{fs, io, path::PathBuf, time::SystemTime};

const TAG_LENGTH: usize = 8;
const TAG: [u8; TAG_LENGTH] = [0xd6, 0xe1, 0x59, 0xdb, 0xa4, 0xd2, 0xf5, 0xd4];

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
        let turn = game.next_turn_index();
        let score = game.score_in_turn(turn);
        format!(
            "{teams}, end {end} stone {stone}, {score}, {datetime}.sav",
            teams = format_args!("{} - {}", game.setup().teams.a.name, game.setup().teams.b.name),
            end = turn.end(),
            stone = turn.stone(),
            score = format_args!("{}-{}", score.a, score.b),
            datetime = datetime.format("%F %T"),
        )
    }
    pub fn save_game(&mut self, game: &Game) -> Result<&SaveEntry> {
        let filename = self.save_filename(game, chrono::Local::now());
        let save_dir = self.save_dir()?;
        fs::create_dir_all(&save_dir)?;
        let path = save_dir.join(filename);
        let file = fs::File::create(&path)?;
        Self::serialize_game(&mut &file, &self.current_version, game)?;
        file.sync_all()?;
        let timestamp = SystemTime::now();
        self.known_saves.push(SaveEntry::new(path, Some(timestamp)));
        Ok(self.known_saves.last().unwrap())
    }

    fn serialize_game(
        mut writer: impl io::Write,
        version: &semver::Version,
        game: &Game,
    ) -> Result<()> {
        writer.write_all(&TAG)?;
        bincode2::serialize_into(&mut writer, version)?;
        bincode2::serialize_into(writer, game)?;
        Ok(())
    }

    pub fn load_game(&self, save_index: usize) -> Result<Game> {
        let save = self
            .known_saves
            .get(save_index)
            .ok_or_else(|| anyhow!("Wrong save index: {}", save_index))?;
        let file = fs::File::open(&save.path)?;
        Self::deserialize_game(&mut &file, &self.current_version)
    }

    fn deserialize_game(mut reader: impl io::Read, version: &semver::Version) -> Result<Game> {
        let mut read_tag = [0; TAG_LENGTH];
        reader.read_exact(&mut read_tag)?;
        if read_tag != TAG {
            bail!("Magic string mismatch. The save is not a valid save file");
        }
        let read_version: semver::Version = bincode2::deserialize_from(&mut reader)?;
        if &read_version != version {
            bail!("Save version mismatch: {read_version} while application is {}", version);
        }
        Ok(bincode2::deserialize_from(reader)?)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game,
        game::{
            team,
            team::{PerTeam, Team},
        },
    };

    #[test]
    fn serialize_and_deserialize() {
        let setup = game::Setup {
            teams: PerTeam {
                a: team::Info { name: "Test Team A".into(), ..Default::default() },
                b: team::Info { name: "Test Team B".into(), ..Default::default() },
            },
            starting_situation: game::situation::Situation {
                hammer: Team::B,
                ..game::situation::Situation::default()
            },
            ..game::Setup::default()
        };
        let game = Game::new(setup);
        let version = semver::Version::new(1, 12, 1);
        let mut buf: Vec<u8> = vec![];
        SaveLoad::serialize_game(&mut buf, &version, &game).expect("Failed to serialize game");
        let loaded = SaveLoad::deserialize_game(&mut buf.as_slice(), &version)
            .expect("Failed to deserialize game");
        assert_eq!(loaded.setup().teams.a.name, "Test Team A");
        assert_eq!(loaded.setup().teams.b.name, "Test Team B");
    }

    #[test]
    fn deserialize_version_mismatch() {
        let game = Game::new(game::Setup::default());
        let saved_version = semver::Version::new(1, 12, 1);
        let loaded_version = semver::Version::new(2, 0, 0);
        let mut buf: Vec<u8> = vec![];
        SaveLoad::serialize_game(&mut buf, &saved_version, &game)
            .expect("Failed to serialize game");
        assert!(SaveLoad::deserialize_game(&mut buf.as_slice(), &loaded_version).is_err());
    }

    #[test]
    fn deserialize_format_mismatch() {
        let game = Game::new(game::Setup::default());
        let version = semver::Version::new(1, 12, 1);
        let mut buf: Vec<u8> = vec![];
        SaveLoad::serialize_game(&mut buf, &version, &game).expect("Failed to serialize game");
        buf[3] = 0x00;
        assert!(SaveLoad::deserialize_game(&mut buf.as_slice(), &version).is_err());
    }
}
