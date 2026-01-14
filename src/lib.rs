mod settings;
mod spacemouse;

use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

use crate::{settings::InputMode, spacemouse::*};
use godot::{
    classes::{
        Camera3D, Control, EditorInterface, EditorPlugin, IEditorPlugin, InputEvent, Label,
        PhysicsRayQueryParameters3D, editor_plugin::DockSlot,
    },
    global::print,
    prelude::*,
};

const GRAB_MODE_MOVE_FLIP: Vector3 = Vector3::new(-1.0, -1.0, -1.0);
const GRAB_MODE_ROTATION_FLIP: Vector3 = Vector3::new(1.0, -1.0, 1.0);

struct SpaceMouse;
#[gdextension]
unsafe impl ExtensionLibrary for SpaceMouse {}

#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
struct SpaceMousePlugin {
    base: Base<EditorPlugin>,

    // state
    spacemouse: Option<SpaceMouseDevice>,
    focused: bool,
    input_mode: InputMode,
    move_speed: f64,
    rotation_speed: f64,
    grab_position: Option<Vector3>,
    end_polling: Option<mpsc::Sender<()>>,

    // ui
    control: Option<Gd<Control>>,
    type_label: Option<Gd<Label>>,
    translation_label: Option<Gd<Label>>,
    rotation_label: Option<Gd<Label>>,

    // 3d
    camera: Option<Gd<Camera3D>>,
}

#[godot_api]
impl SpaceMousePlugin {
    fn cache_path() -> PathBuf {
        std::env::current_dir()
            .unwrap()
            .join(".godot/spacemouse_cache.bin")
    }

    #[func]
    fn on_focus_entered(&mut self) {
        self.focused = true;
    }

    #[func]
    fn on_focus_exited(&mut self) {
        self.focused = false;
    }

    #[func]
    fn on_settings_changed(&mut self) {
        let editor = EditorInterface::singleton();
        let Some(settings) = editor.get_editor_settings() else {
            return;
        };

        let changed_settings = settings.get_changed_settings();
        if changed_settings.contains(settings::SETTING_INPUT_MODE) {
            self.input_mode = settings::get_input_mode();
        }
        if changed_settings.contains(settings::SETTING_MOVE_SPEED) {
            self.move_speed = settings::get_move_speed();
        }
        if changed_settings.contains(settings::SETTING_ROTATION_SPEED) {
            self.move_speed = settings::get_rotation_speed();
        }
    }
}

#[godot_api]
impl IEditorPlugin for SpaceMousePlugin {
    fn enter_tree(&mut self) {
        print(&["enter_tree".to_variant()]);

        settings::init();
        let editor = EditorInterface::singleton();
        if let Some(mut settings) = editor.get_editor_settings() {
            settings.connect(
                "settings_changed",
                &self.base().callable("on_settings_changed"),
            );
            self.input_mode = settings::get_input_mode();
            self.move_speed = settings::get_move_speed();
            self.rotation_speed = settings::get_rotation_speed();
        };

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

        if let Some(end_polling) = self.end_polling.as_ref() {
            end_polling.send(()).expect("could not kill polling thread");
            // this sleep somehow makes sure godot doesn't crash when reloading
            // the editor plugin after rebuilding it...
            thread::sleep(Duration::from_millis(50));
        }

        let editor = EditorInterface::singleton();
        if let Some(mut settings) = editor.get_editor_settings() {
            settings.disconnect(
                "settings_changed",
                &self.base().callable("on_settings_changed"),
            );
        };

        if let Some(control) = self.control.as_ref() {
            self.to_gd().remove_control_from_docks(control);
        }
    }

    fn ready(&mut self) {
        self.focused = true;
        self.camera = self.to_gd().get_viewport().unwrap().get_camera_3d(); // TODO: doesnt work

        if let Ok(mut spacemouse) = SpaceMouseDevice::find_with_cache(Self::cache_path()) {
            let channel = spacemouse.start_polling();
            self.end_polling = Some(channel);
            self.spacemouse = Some(spacemouse);
        }

        if let Some(type_label) = self.type_label.as_mut()
            && let Some(spacemouse) = &self.spacemouse
        {
            type_label.set_text(&spacemouse.format.to_string());
        }

        // window focus
        let mut window = self
            .to_gd()
            .get_editor_interface()
            .unwrap()
            .get_base_control()
            .unwrap()
            .get_window()
            .unwrap();
        let callable_enter = self.to_gd().callable("on_focus_entered");
        let callable_exit = self.to_gd().callable("on_focus_exited");

        window.connect("focus_entered", &callable_enter);
        window.connect("focus_exited", &callable_exit);
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
        }

        0
    }

    fn physics_process(&mut self, delta: f64) {
        if let Some(spacemouse) = self.spacemouse.as_ref()
            && self.focused
            && let Some(camera) = self.camera.as_mut()
        {
            let translation = spacemouse.translation.lock().unwrap();
            let rotation = spacemouse.rotation.lock().unwrap();

            self.translation_label
                .as_mut()
                .unwrap()
                .set_text(&translation.to_string());

            self.rotation_label
                .as_mut()
                .unwrap()
                .set_text(&rotation.to_string());

            if !translation.is_zero_approx() || !rotation.is_zero_approx() {
                if self.input_mode == InputMode::Fly {
                    camera.translate(*translation * 0.05 * delta as f32);
                    let new_rotation = camera.get_rotation() + (*rotation * 0.025 * delta as f32);
                    camera.set_rotation(new_rotation);
                } else {
                    let middle_of_screen =
                        camera.get_viewport().unwrap().get_visible_rect().center();
                    let from = camera.project_position(middle_of_screen, 0.0);
                    let to = from + camera.project_position(middle_of_screen, 15.0);

                    if let Some(query) = PhysicsRayQueryParameters3D::create(from, to) {
                        let mut space_state = camera
                            .get_world_3d()
                            .unwrap()
                            .get_direct_space_state()
                            .unwrap();
                        let result = space_state.intersect_ray(&query);
                        if self.grab_position.is_none() {
                            if !result.is_empty() {
                                let position = result.get("position").unwrap().to::<Vector3>();
                                self.grab_position = Some(position);
                                print(&[result.to_variant()]);
                            } else {
                                self.grab_position = Some(to);
                            }
                        }

                        if let Some(grab_position) = self.grab_position {
                            let mut camera_transform = camera.get_transform();
                            let camera_origin = camera_transform.origin;

                            let offset_speed =
                                (grab_position.distance_to(camera_origin) / 8.0) + 0.01;
                            let offset_speed = offset_speed.clamp(0.01, 8.0);

                            let space_trans = camera_transform * *translation * GRAB_MODE_MOVE_FLIP;
                            let space_trans = space_trans * 0.1 * offset_speed * delta as f32;
                            let space_rot = *rotation * 0.02 * GRAB_MODE_ROTATION_FLIP * delta as f32;

                            camera_transform.origin = space_trans + camera_origin - grab_position;

                            let camera_transform = camera_transform
                                .rotated(camera_transform.basis.col_a().normalized(), space_rot.x);
                            let camera_transform = camera_transform
                                .rotated(camera_transform.basis.col_b().normalized(), space_rot.y);
                            let mut camera_transform = camera_transform
                                .rotated(camera_transform.basis.col_c().normalized(), space_rot.z);

                            camera_transform.origin += grab_position;
                            camera.set_transform(camera_transform);
                        }
                    }
                }
            } else if self.grab_position.is_some() {
                self.grab_position = None;
            }
        }
    }
}
