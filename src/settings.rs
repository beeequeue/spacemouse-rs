use core::fmt;

use godot::{classes::EditorInterface, global::PropertyHint, prelude::*};

const SETTING_HID: &str = "spacemouse/hid";
pub const SETTING_INPUT_MODE: &str = "spacemouse/input_mode";
pub const SETTING_MOVE_SPEED: &str = "spacemouse/move_speed";
const DEFAULT_MOVE_SPEED: f64 = 10.0;
pub const SETTING_ROTATION_SPEED: &str = "spacemouse/rotation_speed";
const DEFAULT_ROTATION_SPEED: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Fly,
    Grab,
}

impl TryFrom<String> for InputMode {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "fly" => Ok(InputMode::Fly),
            "grab" => Ok(InputMode::Grab),
            _ => Err(()),
        }
    }
}

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fly => write!(f, "Fly"),
            Self::Grab => write!(f, "Grab"),
        }
    }
}

pub fn init() {
    let editor = EditorInterface::singleton();
    let Some(mut settings) = editor.get_editor_settings() else {
        godot_error!("[godot-spacemouse] Failed to get EditorSettings");
        return;
    };

    if !settings.has_setting(SETTING_INPUT_MODE) {
        settings.set_setting(
            SETTING_INPUT_MODE,
            &Variant::from(InputMode::Fly.to_string()),
        );
    }
    settings.add_property_info(&vdict! {
        "name": SETTING_INPUT_MODE,
        "type": VariantType::STRING,
        "hint": PropertyHint::ENUM,
        "hint_string": "Fly,Grab",
    });

    if !settings.has_setting(SETTING_MOVE_SPEED) {
        settings.set_setting(SETTING_MOVE_SPEED, &Variant::from(DEFAULT_MOVE_SPEED));
    }
    settings.add_property_info(&vdict! {
        "name": SETTING_MOVE_SPEED,
        "type": VariantType::FLOAT,
        "hint": PropertyHint::RANGE,
        "hint_string": "0,20",
    });

    if !settings.has_setting(SETTING_ROTATION_SPEED) {
        settings.set_setting(
            SETTING_ROTATION_SPEED,
            &Variant::from(DEFAULT_ROTATION_SPEED),
        );
    }
    settings.add_property_info(&vdict! {
        "name": SETTING_ROTATION_SPEED,
        "type": VariantType::FLOAT,
        "hint": PropertyHint::RANGE,
        "hint_string": "0,20",
    });
}

pub fn get_input_mode() -> InputMode {
    let editor = EditorInterface::singleton();
    if let Some(settings) = editor.get_editor_settings() {
        let raw = settings.get_setting(SETTING_INPUT_MODE);
        raw.to_string().try_into().unwrap_or(InputMode::default())
    } else {
        InputMode::default()
    }
}

pub fn get_move_speed() -> f64 {
    let editor = EditorInterface::singleton();
    if let Some(settings) = editor.get_editor_settings() {
        let raw = settings.get_setting(SETTING_INPUT_MODE);
        raw.try_to().unwrap_or(DEFAULT_MOVE_SPEED)
    } else {
        DEFAULT_MOVE_SPEED
    }
}

pub fn get_rotation_speed() -> f64 {
    let editor = EditorInterface::singleton();
    if let Some(settings) = editor.get_editor_settings() {
        let raw = settings.get_setting(SETTING_ROTATION_SPEED);
        raw.try_to().unwrap_or(DEFAULT_ROTATION_SPEED)
    } else {
        DEFAULT_ROTATION_SPEED
    }
}
