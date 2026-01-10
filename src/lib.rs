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

    pub fn read_data(format: Format, buf: &[u8]) -> (Vector3, Vector3) {
        let mut translation = Vector3::ZERO;
        let mut rotation = Vector3::ZERO;

        let first = *buf.first().unwrap();
        match format {
            Format::Original => {
                if first == 1 {
                    translation.x = *buf.get(1).unwrap() as f32;
                    translation.y = *buf.get(5).unwrap() as f32;
                    translation.x = *buf.get(3).unwrap() as f32;
                } else if first == 2 {
                    rotation.x = *buf.get(1).unwrap() as f32;
                    rotation.y = *buf.get(5).unwrap() as f32;
                    rotation.x = *buf.get(3).unwrap() as f32;
                }
            }

            Format::Current => {
                if first == 1 {
                    translation.x = *buf.get(1).unwrap() as f32;
                    translation.y = *buf.get(5).unwrap() as f32;
                    translation.x = *buf.get(3).unwrap() as f32;
                    rotation.x = *buf.get(7).unwrap() as f32;
                    rotation.y = *buf.get(11).unwrap() as f32;
                    rotation.x = *buf.get(9).unwrap() as f32;
                }
            }
        }

        (translation, rotation)
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

    control: Option<Gd<Control>>,
}

#[godot_api]
impl IEditorPlugin for SpaceMousePlugin {
    fn ready(&mut self) {
        let hidapi = HidApi::new().unwrap();

        if let Some(spacemouse) = SpaceMouse::find_spacemouse(&hidapi) {
            let device = hidapi.open(spacemouse.vid, spacemouse.pid).unwrap();
            device.set_blocking_mode(false).unwrap();
            self.spacemouse = Some(spacemouse);
            self.hid_device = Some(device);
        }

        self.hidapi = Some(hidapi);
    }

    fn physics_process(&mut self, _delta: f64) {
        if self.hid_device.is_none() {
            return;
        }

        let device = self.hid_device.as_ref().unwrap();
        let translation = Vector3::ZERO;
        let rotation = Vector3::ZERO;

        let buffer: &mut [u8; 12] = &mut [0; 12];
        let result = device.read(buffer);
        if result.is_ok() {
            let (translation, rotation) =
                SpaceMouse::read_data(self.spacemouse.unwrap().format, buffer);
            if (translation + rotation).length() != 0.0 {
                print(&[Variant::from(format!(
                    "{:#?}, {:#?}",
                    translation, rotation
                ))]);
            }
        }
    }

    fn enter_tree(&mut self) {
        print(&[Variant::from("enter_tree")]);

        let settings_scene = load::<PackedScene>("res://addons/spacemouse2/settings.tscn");
        let control = settings_scene
            .instantiate()
            .unwrap()
            .try_cast::<Control>()
            .unwrap();

        self.to_gd()
            .add_control_to_dock(DockSlot::LEFT_UR, &control);
        self.control = Some(control);
    }

    fn exit_tree(&mut self) {
        print(&[Variant::from("exit_tree")]);

        if let Some(control) = self.control.as_ref() {
            self.to_gd().remove_control_from_docks(control);
        }
    }
}
