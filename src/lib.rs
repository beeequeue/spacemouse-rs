mod spacemouse;

use crate::spacemouse::*;
use godot::{
    classes::{
        Camera3D, Control, EditorPlugin, IEditorPlugin, InputEvent, Label, editor_plugin::DockSlot,
    },
    global::print,
    prelude::*,
};

struct SpaceMouse;
#[gdextension]
unsafe impl ExtensionLibrary for SpaceMouse {}

#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
struct SpaceMousePlugin {
    base: Base<EditorPlugin>,
    spacemouse: Option<SpaceMouseDevice>,

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

        if let Ok(spacemouse) = SpaceMouseDevice::find() {
            self.spacemouse = Some(spacemouse);
        }

        if let Some(type_label) = self.type_label.as_mut()
            && let Some(spacemouse) = &self.spacemouse
        {
            type_label.set_text(&spacemouse.format.to_string());
        }
    }

    // Required to trigger `forward_3d_gui_input`
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
        if let Some(spacemouse) = self.spacemouse.as_ref() {
            let (translation, rotation) = spacemouse.read_data();
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
