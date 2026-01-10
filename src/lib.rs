use godot::{
    classes::{Control, EditorPlugin, IEditorPlugin, editor_plugin::DockSlot},
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
    pub fn find_spacemouse(hidapi: &HidApi) -> Option<SpaceMouseDevice> {
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

fn read_data(spacemouse: SpaceMouseDevice, hid: &HidDevice) -> (Vector3, Vector3) {
    match spacemouse.format {
        Format::Original => {
            let mut translation = Vector3::ZERO;
            let mut rotation = Vector3::ZERO;

            for _ in 0..1 {
                let buffer: &mut [u8; 7] = &mut [0; 7];
                let result = hid.read(buffer);
                if result.is_err() {
                    return (translation, rotation);
                }

                let first = *buffer.first().unwrap();
                if first == 1 {
                    translation.x = *buffer.get(1).unwrap() as f32;
                    translation.y = *buffer.get(5).unwrap() as f32;
                    translation.x = *buffer.get(3).unwrap() as f32;
                } else if first == 2 {
                    rotation.x = *buffer.get(1).unwrap() as f32;
                    rotation.y = *buffer.get(5).unwrap() as f32;
                    rotation.x = *buffer.get(3).unwrap() as f32;
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
                translation.x = *buffer.get(1).unwrap() as f32;
                translation.y = *buffer.get(5).unwrap() as f32;
                translation.x = *buffer.get(3).unwrap() as f32;
                rotation.x = *buffer.get(7).unwrap() as f32;
                rotation.y = *buffer.get(11).unwrap() as f32;
                rotation.x = *buffer.get(9).unwrap() as f32;
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
}

#[godot_api]
impl IEditorPlugin for SpaceMousePlugin {
    fn ready(&mut self) {

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
    }

    fn physics_process(&mut self, _delta: f64) {
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
                camera.translate(translation);
            } else {
                self.camera = self.to_gd().get_viewport().unwrap().get_camera_3d();
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
