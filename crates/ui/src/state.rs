use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f32::is_finite)
            && self.width >= 100.0
            && self.height >= 100.0
    }

    fn fit(self, display: Self) -> Self {
        let width = self
            .width
            .clamp(640.0_f32.min(display.width), display.width);
        let height = self
            .height
            .clamp(400.0_f32.min(display.height), display.height);
        Self {
            x: self.x.clamp(display.x, display.x + display.width - width),
            y: self.y.clamp(display.y, display.y + display.height - height),
            width,
            height,
        }
    }

    fn overlap(self, other: Self) -> f32 {
        let width = (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
        let height = (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
        width.max(0.0) * height.max(0.0)
    }
}

pub struct Display {
    pub id: Option<String>,
    pub bounds: Rectangle,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub version: u32,
    pub appearance: Appearance,
    pub bounds: Option<Rectangle>,
    pub display: Option<String>,
    pub maximized: bool,
    pub recent: Vec<PathBuf>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            appearance: Appearance::System,
            bounds: None,
            display: None,
            maximized: false,
            recent: Vec::new(),
        }
    }
}

impl State {
    pub fn placement(&self, displays: &[Display]) -> Rectangle {
        let saved = self.bounds.filter(|bounds| bounds.valid());
        let primary = displays.first().filter(|display| display.bounds.valid());
        let default = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1100.0,
            height: 720.0,
        };
        let Some(primary) = primary else {
            let bounds = saved.unwrap_or(default);
            return Rectangle {
                width: bounds.width.min(4096.0),
                height: bounds.height.min(4096.0),
                ..bounds
            };
        };
        let target = displays
            .iter()
            .filter(|display| display.bounds.valid())
            .find(|display| self.display.is_some() && display.id == self.display)
            .or_else(|| {
                saved.and_then(|bounds| {
                    displays
                        .iter()
                        .filter(|display| display.bounds.valid())
                        .max_by(|a, b| {
                            bounds
                                .overlap(a.bounds)
                                .total_cmp(&bounds.overlap(b.bounds))
                        })
                        .filter(|display| bounds.overlap(display.bounds) > 0.0)
                })
            })
            .unwrap_or(primary)
            .bounds;
        saved
            .unwrap_or(Rectangle {
                x: target.x + (target.width - default.width) / 2.0,
                y: target.y + (target.height - default.height) / 2.0,
                ..default
            })
            .fit(target)
    }

    pub fn remember(&mut self, bounds: Rectangle, maximized: bool, display: Option<String>) {
        self.maximized = maximized;
        if !maximized && bounds.valid() {
            self.bounds = Some(bounds);
            self.display = display;
        }
    }
}

pub struct Store {
    path: Option<PathBuf>,
    writable: bool,
}

impl Store {
    pub fn load() -> (Self, State, Option<String>) {
        let path = dirs::config_dir().map(|dir| dir.join("wireroom/session.json"));
        let mut store = Self {
            path,
            writable: true,
        };
        match store.read() {
            Ok(state) => (store, state, None),
            Err(error) => {
                let message = format!("Could not restore window settings: {error:#}");
                eprintln!("{message}");
                (store, State::default(), Some(message))
            }
        }
    }

    fn read(&mut self) -> anyhow::Result<State> {
        let Some(path) = &self.path else {
            return Ok(State::default());
        };
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(State::default());
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            bail!("settings file exceeds 64 KiB")
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
            self.writable = false;
            bail!("unsupported settings version; file will be left unchanged")
        }
        let mut state: State = serde_json::from_value(value)?;
        state.recent.truncate(10);
        Ok(state)
    }

    pub fn save(&self, state: &State) -> anyhow::Result<()> {
        if !self.writable {
            return Ok(());
        }
        let path = self
            .path
            .as_deref()
            .context("no per-user configuration directory")?;
        save_to(path, state)
    }
}

fn save_to(path: &Path, state: &State) -> anyhow::Result<()> {
    let directory = path.parent().context("settings path has no parent")?;
    std::fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).context("replace settings file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Appearance, Display, Rectangle, State, Store, save_to};

    fn screen() -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    #[test]
    fn missing_display_and_invalid_geometry_restore_inside_visible_screen() {
        let displays = [Display {
            id: Some("primary".into()),
            bounds: screen(),
        }];
        let state = State {
            bounds: Some(Rectangle {
                x: 9000.0,
                y: -800.0,
                width: 3000.0,
                height: 1500.0,
            }),
            display: Some("gone".into()),
            ..State::default()
        };
        assert_eq!(state.placement(&displays), screen());
        let invalid = State {
            bounds: Some(Rectangle {
                width: f32::NAN,
                ..screen()
            }),
            ..state
        };
        let restored = invalid.placement(&displays);
        assert_eq!((restored.width, restored.height), (1100.0, 720.0));
        assert!(restored.x > 0.0 && restored.y > 0.0);
    }

    #[test]
    fn maximizing_preserves_last_normal_rectangle() {
        let normal = Rectangle {
            width: 900.0,
            height: 600.0,
            ..screen()
        };
        let mut state = State::default();
        state.remember(normal, false, Some("display".into()));
        state.remember(screen(), true, None);
        assert_eq!(state.bounds, Some(normal));
        assert!(state.maximized);
        assert_eq!(state.display.as_deref(), Some("display"));
    }

    #[test]
    fn atomic_replacement_keeps_appearance_and_real_history() {
        let directory = tempfile::tempdir().expect("temporary settings directory");
        let path = directory.path().join("session.json");
        let mut state = State::default();
        save_to(&path, &state).expect("initial settings");
        state.appearance = Appearance::Dark;
        state.recent.push(directory.path().join("chosen.wav"));
        save_to(&path, &state).expect("replace settings");
        let restored: State =
            serde_json::from_slice(&std::fs::read(&path).expect("settings bytes"))
                .expect("settings data");
        assert_eq!(restored.appearance, Appearance::Dark);
        assert_eq!(restored.recent, state.recent);
        let future = br#"{"version":2,"future":"preserve"}"#;
        std::fs::write(&path, future).expect("future settings");
        let mut store = Store {
            path: Some(path.clone()),
            writable: true,
        };
        assert!(store.read().is_err());
        store
            .save(&state)
            .expect("unknown version is not overwritten");
        assert_eq!(
            std::fs::read(&path).expect("original future settings"),
            future
        );
        std::fs::write(&path, b"{").expect("malformed settings");
        let mut store = Store {
            path: Some(path),
            writable: true,
        };
        assert!(store.read().is_err());
    }
}
