use godot::{
    classes::{
        Camera3D, Control, EditorPlugin, IEditorPlugin, InputEvent, Label, editor_plugin::DockSlot,
    },
    global::print,
    prelude::*,
};
use hidapi::{HidApi, HidDevice};

#[derive(PartialEq, Eq, Clone, Copy)]
enum Format {
    Original,
    Current,
}

impl ToString for Format {
    fn to_string(&self) -> String {
        match self {
            Self::Original => "Original".to_string(),
            Self::Current => "Current".to_string(),
        }
    }
}

#[derive(Clone, Copy)]
struct SpaceMouseDevice {
    vid: u16,
    pid: u16,
    format: Format,
}

impl SpaceMouseDevice {
    pub const fn from(vid: u16, pid: u16, format: Format) -> Self {
        Self { vid, pid, format }
    }
}

const SPACE_MOUSE_HIDS: &[SpaceMouseDevice; 7] = &[
    SpaceMouseDevice::from(0x046d, 0xc626, Format::Original), // 3Dconnexion Space Navigator 3D Mouse
    SpaceMouseDevice::from(0x256f, 0xc635, Format::Original), // SpaceMouse Compact
    SpaceMouseDevice::from(0x256f, 0xc632, Format::Current),  // SpaceMouse Pro Wireless Receiver
    SpaceMouseDevice::from(0x046d, 0xc62b, Format::Original), // 3Dconnexion Space Mouse Pro
    SpaceMouseDevice::from(0x256f, 0xc62e, Format::Current),  // SpaceMouse Wireless (cabled)
    SpaceMouseDevice::from(0x256f, 0xc652, Format::Current),  // Universal Receiver
    SpaceMouseDevice::from(0x046d, 0xc629, Format::Original), // 3Dconnexion SpacePilot Pro 3D Mouse
];

struct SpaceMouse;

impl SpaceMouse {
    pub fn find(hidapi: &HidApi) -> Option<SpaceMouseDevice> {
        for device in hidapi.device_list() {
            for spacemouse in SPACE_MOUSE_HIDS {
                if device.vendor_id() == spacemouse.vid && device.product_id() == spacemouse.pid {
                    return Some(*spacemouse);
                }
            }
        }

        None
    }
}

fn to_i16(slice: &[u8]) -> i16 {
    i16::from_le_bytes(slice.try_into().unwrap())
}

fn read_data(spacemouse: SpaceMouseDevice, hid: &HidDevice) -> (Vector3, Vector3) {
    match spacemouse.format {
        Format::Original => {
            let mut translation = Vector3::ZERO;
            let mut rotation = Vector3::ZERO;

            for _ in 0..4 {
                let buffer: &mut [u8; 7] = &mut [0; 7];
                let result = hid.read(buffer);
                if result.is_err() {
                    return (translation, rotation);
                }

                let first = *buffer.first().unwrap();
                if first == 1 {
                    translation.x = to_i16(&buffer[1..=2]) as f32;
                    translation.y = -to_i16(&buffer[5..=6]) as f32;
                    translation.z = to_i16(&buffer[3..=4]) as f32;
                } else if first == 2 {
                    rotation.x = to_i16(&buffer[1..=2]) as f32;
                    rotation.y = -to_i16(&buffer[5..=6]) as f32;
                    rotation.z = to_i16(&buffer[3..=4]) as f32;
                }
            }

            (translation, rotation)
        }

        Format::Current => {
            let buffer: &mut [u8; 12] = &mut [0; 12];
            let result = hid.read(buffer);
            if result.is_err() {
                return (Vector3::ZERO, Vector3::ZERO);
            }

            let mut translation = Vector3::ZERO;
            let mut rotation = Vector3::ZERO;

            let first = *buffer.first().unwrap();
            if first == 1 {
                translation.x = to_i16(&buffer[1..=2]) as f32;
                translation.y = -to_i16(&buffer[5..=6]) as f32;
                translation.z = to_i16(&buffer[3..=4]) as f32;
                rotation.x = to_i16(&buffer[7..=8]) as f32;
                rotation.y = -to_i16(&buffer[1..=2]) as f32;
                rotation.z = to_i16(&buffer[9..=10]) as f32;
            }

            (translation, rotation)
        }
    }
}

#[gdextension]
unsafe impl ExtensionLibrary for SpaceMouse {}

#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
struct SpaceMousePlugin {
    base: Base<EditorPlugin>,
    hidapi: Option<HidApi>,
    spacemouse: Option<SpaceMouseDevice>,
    hid_device: Option<HidDevice>,

    // ui
    control: Option<Gd<Control>>,
    type_label: Option<Gd<Label>>,
    translation_label: Option<Gd<Label>>,
    rotation_label: Option<Gd<Label>>,

    // 3d
    camera: Option<Gd<Camera3D>>,
}

#[godot_api]
impl IEditorPlugin for SpaceMousePlugin {
    fn ready(&mut self) {
        // self.to_gd().get_viewport().unwrap().print_tree_pretty();
        self.camera = self.to_gd().get_viewport().unwrap().get_camera_3d();

        let hidapi = HidApi::new().unwrap();
        if let Some(spacemouse) = SpaceMouse::find(&hidapi) {
            let device = hidapi.open(spacemouse.vid, spacemouse.pid).unwrap();
            device.set_blocking_mode(false).unwrap();
            self.spacemouse = Some(spacemouse);
            self.hid_device = Some(device);
        }

        self.hidapi = Some(hidapi);
        if let Some(type_label) = self.type_label.as_mut()
            && let Some(spacemouse) = self.spacemouse
        {
            type_label.set_text(&spacemouse.format.to_string());
        }
    }

    fn handles(&self, _o: Gd<Object>) -> bool {
        true
    }

    fn forward_3d_gui_input(
        &mut self,
        camera: Option<Gd<Camera3D>>,
        _event: Option<Gd<InputEvent>>,
    ) -> i32 {
        if camera.is_some()
            && self.camera.as_ref().is_none_or(|current| {
                current.get_camera_rid() != camera.as_ref().unwrap().get_camera_rid()
            })
        {
            self.camera = camera;
            print(&["Set camera".to_variant()]);
        }

        0
    }

    fn physics_process(&mut self, delta: f64) {
        if self.hid_device.is_none() {
            return;
        }

        let device = self.hid_device.as_ref().unwrap();

        let (translation, rotation) = read_data(self.spacemouse.unwrap(), device);
        if (translation + rotation).length() != 0.0 {
            self.translation_label
                .as_mut()
                .unwrap()
                .set_text(&translation.to_string());

            self.rotation_label
                .as_mut()
                .unwrap()
                .set_text(&rotation.to_string());

            if let Some(camera) = self.camera.as_mut() {
                camera.translate(translation * 0.25 * delta as f32);
                let new_rotation = camera.get_rotation() + (rotation * 0.05 * delta as f32);
                camera.set_rotation(new_rotation);
            }
        }
    }

    fn enter_tree(&mut self) {
        print(&["enter_tree".to_variant()]);

        let settings_scene = load::<PackedScene>("res://addons/spacemouse2/settings.tscn");
        let control = settings_scene
            .instantiate()
            .unwrap()
            .try_cast::<Control>()
            .unwrap();

        self.type_label = Some(control.get_node_as("Bottom/DebugInfo/TypeLabel"));
        self.translation_label = Some(control.get_node_as("Bottom/DebugInfo/TransformLabel"));
        self.rotation_label = Some(control.get_node_as("Bottom/DebugInfo/RotationLabel"));

        self.to_gd()
            .add_control_to_dock(DockSlot::LEFT_UR, &control);
        self.control = Some(control);
    }

    fn exit_tree(&mut self) {
        print(&["exit_tree".to_variant()]);

        if let Some(control) = self.control.as_ref() {
            self.to_gd().remove_control_from_docks(control);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_int16() {
        let buffer: &[u8] = &[1, 0x00, 0x10, 0xff, 0x00, 0xff, 0xff];

        assert_eq!(to_i16(&buffer[1..=2]), 4096);
        assert_eq!(to_i16(&buffer[3..=4]), 255);
        assert_eq!(to_i16(&buffer[5..=6]), -1);
    }
}
