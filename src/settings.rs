use std::env;
use std::fs;
use std::path::PathBuf;

pub const SMALL_FUSHI_SCALE: f32 = 0.48;
pub const DEFAULT_FUSHI_SCALE: f32 = SMALL_FUSHI_SCALE;
pub const NORMAL_FUSHI_SCALE: f32 = 0.62;
pub const LARGE_FUSHI_SCALE: f32 = 0.80;
pub const HUGE_FUSHI_SCALE: f32 = 0.98;
pub const DEFAULT_INTERACT_WITH_WINDOWS: bool = true;

const SETTINGS_FILE_NAME: &str = "settings.ini";
const SETTINGS_DIR_NAME: &str = "Desktop Fushi";

#[derive(Clone, Copy, Debug)]
pub struct AppSettings {
    pub fushi_scale: f32,
    pub interact_with_windows: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            fushi_scale: DEFAULT_FUSHI_SCALE,
            interact_with_windows: DEFAULT_INTERACT_WITH_WINDOWS,
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };

        let mut settings = Self::default();
        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "fushi_scale" => {
                    if let Ok(scale) = value.trim().parse::<f32>() {
                        settings.fushi_scale = clamp_scale(scale);
                    }
                }
                "interact_with_windows" => {
                    if let Some(enabled) = parse_bool(value.trim()) {
                        settings.interact_with_windows = enabled;
                    }
                }
                _ => {}
            }
        }
        settings
    }

    pub fn save(self) -> Result<(), String> {
        let Some(path) = settings_path() else {
            return Err("settings directory is unavailable".to_string());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create settings directory: {err}"))?;
        }

        let contents = format!(
            "fushi_scale={:.2}\ninteract_with_windows={}\n",
            clamp_scale(self.fushi_scale),
            self.interact_with_windows
        );
        fs::write(path, contents).map_err(|err| format!("failed to save settings: {err}"))
    }
}

pub fn clamp_scale(scale: f32) -> f32 {
    normalize_legacy_scale(scale).clamp(SMALL_FUSHI_SCALE, HUGE_FUSHI_SCALE)
}

fn normalize_legacy_scale(scale: f32) -> f32 {
    for (legacy, current) in [
        (0.56, SMALL_FUSHI_SCALE),
        (0.72, NORMAL_FUSHI_SCALE),
        (0.92, LARGE_FUSHI_SCALE),
        (1.14, HUGE_FUSHI_SCALE),
    ] {
        if (scale - legacy).abs() <= 0.005 {
            return current;
        }
    }
    scale
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let base = env::var_os("APPDATA")
            .or_else(|| env::var_os("LOCALAPPDATA"))
            .map(PathBuf::from)?;
        return Some(base.join(SETTINGS_DIR_NAME).join(SETTINGS_FILE_NAME));
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").map(PathBuf::from)?;
        return Some(
            home.join("Library")
                .join("Application Support")
                .join(SETTINGS_DIR_NAME)
                .join(SETTINGS_FILE_NAME),
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(base.join(SETTINGS_DIR_NAME).join(SETTINGS_FILE_NAME))
    }
}
