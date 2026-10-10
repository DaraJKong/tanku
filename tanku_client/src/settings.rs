//! The player's settings and records, kept between sessions.

use std::collections::BTreeMap;

use bevy::{
    audio::{GlobalVolume, Volume},
    prelude::*,
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};
use tanku_core::seed::VERSION;

use crate::{audio::Muted, storage};

const SETTINGS_FILE: &str = "settings.txt";
const RECORDS_FILE: &str = "records.txt";

/// Something the player does with a key or a mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Shoot,
    Mine,
    Shield,
    RotateLeft,
    RotateRight,
}

impl Action {
    pub const ALL: [Action; 9] = [
        Action::Up,
        Action::Down,
        Action::Left,
        Action::Right,
        Action::Shoot,
        Action::Mine,
        Action::Shield,
        Action::RotateLeft,
        Action::RotateRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Up => "Move up",
            Action::Down => "Move down",
            Action::Left => "Move left",
            Action::Right => "Move right",
            Action::Shoot => "Shoot",
            Action::Mine => "Place a mine",
            Action::Shield => "Shield",
            Action::RotateLeft => "Turn the view left",
            Action::RotateRight => "Turn the view right",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Action::Up => "bind.up",
            Action::Down => "bind.down",
            Action::Left => "bind.left",
            Action::Right => "bind.right",
            Action::Shoot => "bind.shoot",
            Action::Mine => "bind.mine",
            Action::Shield => "bind.shield",
            Action::RotateLeft => "bind.rotate_left",
            Action::RotateRight => "bind.rotate_right",
        }
    }

    fn defaults(self) -> [Option<Binding>; 2] {
        use Binding::{Key, Mouse};
        match self {
            Action::Up => [Some(Key(KeyCode::KeyW)), Some(Key(KeyCode::ArrowUp))],
            Action::Down => [Some(Key(KeyCode::KeyS)), Some(Key(KeyCode::ArrowDown))],
            Action::Left => [Some(Key(KeyCode::KeyA)), Some(Key(KeyCode::ArrowLeft))],
            Action::Right => [Some(Key(KeyCode::KeyD)), Some(Key(KeyCode::ArrowRight))],
            Action::Shoot => [Some(Mouse(MouseButton::Left)), None],
            Action::Mine => [Some(Mouse(MouseButton::Right)), None],
            Action::Shield => [Some(Key(KeyCode::Space)), None],
            Action::RotateLeft => [Some(Key(KeyCode::KeyQ)), None],
            Action::RotateRight => [Some(Key(KeyCode::KeyE)), None],
        }
    }
}

macro_rules! keys {
    ($($key:ident),* $(,)?) => {
        /// Every key that can be bound, with the name it is saved under.
        const KEYS: &[(KeyCode, &str)] = &[$((KeyCode::$key, stringify!($key))),*];
    };
}

keys!(
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Space,
    Tab,
    Enter,
    Backspace,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    Comma,
    Period,
    Slash,
    Semicolon,
    Quote,
    BracketLeft,
    BracketRight,
    Backslash,
    Minus,
    Equal,
    Backquote,
);

const MOUSE_BUTTONS: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Left, "MouseLeft", "Left click"),
    (MouseButton::Right, "MouseRight", "Right click"),
    (MouseButton::Middle, "MouseMiddle", "Middle click"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl Binding {
    /// The name the binding is saved under.
    fn code(self) -> &'static str {
        match self {
            Binding::Key(key) => KEYS
                .iter()
                .find(|(known, _)| *known == key)
                .map(|(_, name)| *name),
            Binding::Mouse(button) => MOUSE_BUTTONS
                .iter()
                .find(|(known, ..)| *known == button)
                .map(|(_, name, _)| *name),
        }
        .unwrap_or("")
    }

    fn parse(code: &str) -> Option<Self> {
        let key = KEYS.iter().find(|(_, name)| *name == code);
        let button = MOUSE_BUTTONS.iter().find(|(_, name, _)| *name == code);
        key.map(|(key, _)| Binding::Key(*key))
            .or(button.map(|(button, ..)| Binding::Mouse(*button)))
    }

    /// The name shown to the player.
    pub fn name(self) -> String {
        if let Binding::Mouse(button) = self {
            let known = MOUSE_BUTTONS.iter().find(|(known, ..)| *known == button);
            return known.map_or("Mouse", |(.., name)| *name).to_string();
        }
        let code = self.code();
        for (prefix, side) in [("Shift", "Shift"), ("Control", "Ctrl"), ("Alt", "Alt")] {
            if let Some(which) = code.strip_prefix(prefix) {
                return format!("{which} {side}");
            }
        }
        ["Key", "Digit", "Arrow"]
            .iter()
            .find_map(|prefix| code.strip_prefix(prefix))
            .unwrap_or(code)
            .to_string()
    }

    /// The key or button that was pressed this frame, if it can be bound.
    pub fn pressed_now(
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> Option<Self> {
        let key = KEYS.iter().find(|(key, _)| keys.just_pressed(*key));
        let button = MOUSE_BUTTONS
            .iter()
            .find(|(button, ..)| mouse.just_pressed(*button));
        key.map(|(key, _)| Binding::Key(*key))
            .or(button.map(|(button, ..)| Binding::Mouse(*button)))
    }

    pub fn held(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Binding::Key(key) => keys.pressed(key),
            Binding::Mouse(button) => mouse.pressed(button),
        }
    }

    pub fn just_pressed(
        self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        match self {
            Binding::Key(key) => keys.just_pressed(key),
            Binding::Mouse(button) => mouse.just_pressed(button),
        }
    }
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Two bindings per action: WASD and the arrow keys both move the tank.
    bindings: Vec<(Action, [Option<Binding>; 2])>,
    /// How loud the game is, from 0 to 1.
    pub volume: f32,
    /// How loud the music is against the sound cues, from 0 to 1.
    pub music: f32,
    pub camera_shake: bool,
    pub ui_scale: f32,
    pub fullscreen: bool,
    pub vsync: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .map(|action| (action, action.defaults()))
                .to_vec(),
            volume: 0.7,
            music: 0.5,
            camera_shake: true,
            ui_scale: 1.0,
            // A browser only goes full screen when the player asks for it.
            fullscreen: !cfg!(target_arch = "wasm32"),
            vsync: true,
        }
    }
}

impl Settings {
    /// The settings as the player left them, or the defaults on the first launch.
    pub fn load() -> Self {
        Self::from_pairs(&storage::load(SETTINGS_FILE))
    }

    pub fn window_mode(&self) -> WindowMode {
        if self.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        }
    }

    pub fn present_mode(&self) -> PresentMode {
        if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        }
    }

    pub fn bindings(&self, action: Action) -> [Option<Binding>; 2] {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == action)
            .map_or([None, None], |(_, bindings)| *bindings)
    }

    /// Binds a key or a button to an action, and takes it away from any other action.
    pub fn bind(&mut self, action: Action, slot: usize, binding: Binding) {
        for (bound, bindings) in &mut self.bindings {
            for (index, current) in bindings.iter_mut().enumerate() {
                if *bound == action && index == slot {
                    *current = Some(binding);
                } else if *current == Some(binding) {
                    *current = None;
                }
            }
        }
    }

    pub fn reset_bindings(&mut self) {
        self.bindings = Settings::default().bindings;
    }

    pub fn held(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        let bindings = self.bindings(action);
        bindings
            .iter()
            .flatten()
            .any(|binding| binding.held(keys, mouse))
    }

    pub fn just_pressed(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        let bindings = self.bindings(action);
        bindings
            .iter()
            .flatten()
            .any(|binding| binding.just_pressed(keys, mouse))
    }

    fn from_pairs(pairs: &BTreeMap<String, String>) -> Self {
        let mut settings = Settings::default();
        let number = |key: &str, fallback: f32, low: f32, high: f32| {
            let value = pairs.get(key).and_then(|value| value.parse::<f32>().ok());
            value
                .filter(|value| value.is_finite())
                .unwrap_or(fallback)
                .clamp(low, high)
        };
        let flag = |key: &str, fallback: bool| {
            pairs
                .get(key)
                .and_then(|value| value.parse().ok())
                .unwrap_or(fallback)
        };
        settings.volume = number("volume", settings.volume, 0.0, 1.0);
        settings.music = number("music", settings.music, 0.0, 1.0);
        settings.ui_scale = number("ui_scale", settings.ui_scale, 0.75, 2.0);
        settings.camera_shake = flag("camera_shake", settings.camera_shake);
        settings.fullscreen = flag("fullscreen", settings.fullscreen);
        settings.vsync = flag("vsync", settings.vsync);
        for (action, bindings) in &mut settings.bindings {
            if let Some(saved) = pairs.get(action.key()) {
                let mut codes = saved.split(',').map(|code| Binding::parse(code.trim()));
                *bindings = [codes.next().flatten(), codes.next().flatten()];
            }
        }
        settings
    }

    fn to_pairs(&self) -> BTreeMap<String, String> {
        let mut pairs = BTreeMap::new();
        pairs.insert("volume".to_string(), self.volume.to_string());
        pairs.insert("music".to_string(), self.music.to_string());
        pairs.insert("ui_scale".to_string(), self.ui_scale.to_string());
        pairs.insert("camera_shake".to_string(), self.camera_shake.to_string());
        pairs.insert("fullscreen".to_string(), self.fullscreen.to_string());
        pairs.insert("vsync".to_string(), self.vsync.to_string());
        for (action, bindings) in &self.bindings {
            let codes = bindings.map(|binding| binding.map_or("", Binding::code));
            pairs.insert(action.key().to_string(), codes.join(", "));
        }
        pairs
    }
}

/// The furthest a run got.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub sector: u32,
    pub seed: String,
    pub version: String,
}

/// The best run per mode and number of players.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Records(BTreeMap<String, Record>);

impl Records {
    fn key(mode: &str, players: u32) -> String {
        format!("{mode}.{players}")
    }

    pub fn best(&self, mode: &str, players: u32) -> Option<&Record> {
        self.0.get(&Self::key(mode, players))
    }

    /// Keeps a run if it went further than the record. Returns whether it did.
    pub fn submit(&mut self, mode: &str, players: u32, sector: u32, seed: &str) -> bool {
        let best = self.best(mode, players).map_or(0, |record| record.sector);
        if sector <= best {
            return false;
        }
        let record = Record {
            sector,
            seed: seed.to_string(),
            version: VERSION.to_string(),
        };
        self.0.insert(Self::key(mode, players), record);
        storage::save(RECORDS_FILE, &self.to_pairs());
        true
    }

    fn from_pairs(pairs: &BTreeMap<String, String>) -> Self {
        let record = |value: &String| {
            let mut parts = value.split_whitespace();
            Some(Record {
                sector: parts.next()?.parse().ok()?,
                seed: parts.next()?.to_string(),
                version: parts.next()?.to_string(),
            })
        };
        Self(
            pairs
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), record(value)?)))
                .collect(),
        )
    }

    fn to_pairs(&self) -> BTreeMap<String, String> {
        self.0
            .iter()
            .map(|(key, record)| {
                let value = format!("{} {} {}", record.sector, record.seed, record.version);
                (key.clone(), value)
            })
            .collect()
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Settings::load())
        .insert_resource(Records::from_pairs(&storage::load(RECORDS_FILE)))
        .add_systems(Update, apply.run_if(resource_changed::<Settings>));
}

/// Applies the settings when they change, and saves them unless they were just loaded.
fn apply(
    settings: Res<Settings>,
    mut ui_scale: ResMut<UiScale>,
    mut volume: ResMut<GlobalVolume>,
    muted: Option<Res<Muted>>,
    window: Option<Single<&mut Window, With<PrimaryWindow>>>,
    mut loaded: Local<bool>,
) {
    ui_scale.0 = settings.ui_scale;
    let loudness = if muted.is_some() {
        0.0
    } else {
        settings.volume
    };
    *volume = GlobalVolume::new(Volume::Linear(loudness));
    // A scripted run has no window.
    if let Some(mut window) = window {
        window.mode = settings.window_mode();
        window.present_mode = settings.present_mode();
    }
    if *loaded {
        storage::save(SETTINGS_FILE, &settings.to_pairs());
    }
    *loaded = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_launch_is_full_screen_and_the_saved_choice_wins_afterward() {
        let saved = |text: &str| {
            let pairs = text
                .split_once('=')
                .map(|(key, value)| (key.into(), value.into()));
            Settings::from_pairs(&pairs.into_iter().collect())
        };
        assert!(saved("").fullscreen);
        assert!(!saved("fullscreen=false").fullscreen);
        assert!(saved("fullscreen=true").fullscreen);
        assert_eq!(
            saved("").window_mode(),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
        assert_eq!(
            saved("fullscreen=false").window_mode(),
            WindowMode::Windowed
        );
    }

    #[test]
    fn settings_survive_a_save_and_a_load() {
        let mut settings = Settings {
            volume: 0.25,
            music: 0.8,
            camera_shake: false,
            ..default()
        };
        settings.bind(Action::Shield, 0, Binding::Key(KeyCode::ShiftLeft));
        settings.bind(Action::Shoot, 1, Binding::Key(KeyCode::KeyF));
        assert_eq!(Settings::from_pairs(&settings.to_pairs()), settings);
    }

    #[test]
    fn broken_settings_fall_back_to_the_defaults() {
        let mut pairs = BTreeMap::new();
        pairs.insert("volume".to_string(), "loud".to_string());
        pairs.insert("music".to_string(), "7".to_string());
        pairs.insert("ui_scale".to_string(), "40".to_string());
        pairs.insert("bind.up".to_string(), "NoSuchKey".to_string());
        let settings = Settings::from_pairs(&pairs);
        assert_eq!(settings.volume, Settings::default().volume);
        assert_eq!(settings.music, 1.0);
        assert_eq!(settings.ui_scale, 2.0);
        assert_eq!(settings.bindings(Action::Up), [None, None]);
        assert_eq!(settings.bindings(Action::Down), Action::Down.defaults());
    }

    #[test]
    fn a_binding_belongs_to_one_action() {
        let mut settings = Settings::default();
        settings.bind(Action::Mine, 0, Binding::Key(KeyCode::Space));
        assert_eq!(
            settings.bindings(Action::Mine)[0],
            Some(Binding::Key(KeyCode::Space))
        );
        assert_eq!(settings.bindings(Action::Shield), [None, None]);
    }

    #[test]
    fn bindings_have_readable_names() {
        assert_eq!(Binding::Key(KeyCode::KeyW).name(), "W");
        assert_eq!(Binding::Key(KeyCode::ArrowUp).name(), "Up");
        assert_eq!(Binding::Key(KeyCode::ShiftLeft).name(), "Left Shift");
        assert_eq!(Binding::Mouse(MouseButton::Right).name(), "Right click");
        for (key, name) in KEYS {
            assert_eq!(Binding::parse(name), Some(Binding::Key(*key)));
        }
    }

    #[test]
    fn records_keep_the_best_run() {
        let mut records = Records::default();
        // `submit` saves to disk, so the test works on the pairs only.
        records.0.insert(
            Records::key("normal", 1),
            Record {
                sector: 7,
                seed: "ABC".into(),
                version: "0.1.0".into(),
            },
        );
        let reloaded = Records::from_pairs(&records.to_pairs());
        assert_eq!(reloaded, records);
        assert_eq!(reloaded.best("normal", 1).unwrap().sector, 7);
        assert!(reloaded.best("normal", 2).is_none());
    }
}
