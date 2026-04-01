mod settings;

use std::path::PathBuf;

use godot::{
    classes::{
        Camera3D, EditorInterface, EditorPlugin, IEditorPlugin, MeshInstance3D,
        PhysicsRayQueryParameters3D, SphereMesh,
    },
    prelude::*,
};
use spacemouse::SpaceMouseDevice;

use crate::settings::InputMode;

const GRAB_MODE_MOVE_FLIP: Vector3 = Vector3::new(-1.0, -1.0, -1.0);
const GRAB_MODE_ROTATION_FLIP: Vector3 = Vector3::new(1.0, -1.0, 1.0);

struct SpaceMouse;
#[gdextension]
unsafe impl ExtensionLibrary for SpaceMouse {}

#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
struct SpaceMousePlugin {
    base: Base<EditorPlugin>,
    grab_gizmo: Option<Gd<MeshInstance3D>>,

    // state
    spacemouse: Option<SpaceMouseDevice>,

    focused: bool,
    input_mode: InputMode,
    move_speed: f64,
    rotation_speed: f64,
    grab_position: Option<Vector3>,

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

    fn redraw_grab_ball(&mut self) {
        let Some(grab_position) = self.grab_position else {
            if let Some(mut gizmo) = self.grab_gizmo.take() {
                gizmo.queue_free();
            }
            return;
        };

        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.25);
        sphere.set_height(0.5);

        // Create MeshInstance3D
        let mut mesh_instance = MeshInstance3D::new_alloc();
        mesh_instance.set_mesh(&sphere);
        mesh_instance.set_position(grab_position);

        let editor_interface = EditorInterface::singleton();
        if let Some(mut scene_root) = editor_interface.get_edited_scene_root() {
            scene_root.add_child(&mesh_instance);
            self.grab_gizmo = Some(mesh_instance);
        }
    }
}

#[godot_api]
impl IEditorPlugin for SpaceMousePlugin {
    fn enter_tree(&mut self) {
        #[cfg(debug_assertions)]
        godot_print!("enter_tree");

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
    }

    fn exit_tree(&mut self) {
        #[cfg(debug_assertions)]
        godot_print!("exit_tree");

        if let Some(spacemouse) = self.spacemouse.as_mut() {
            let res = spacemouse.stop_polling();
            if let Err(error) = res {
                godot_error!("SpaceMouse polling thread crashed: {}", error);
            };
            // this sleep somehow makes sure godot doesn't crash when reloading
            // the editor plugin after rebuilding it...
            #[cfg(debug_assertions)]
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let editor = EditorInterface::singleton();
        if let Some(mut settings) = editor.get_editor_settings() {
            settings.disconnect(
                "settings_changed",
                &self.base().callable("on_settings_changed"),
            );
        };
    }

    fn ready(&mut self) {
        self.focused = true;
        self.camera = self
            .to_gd()
            .get_editor_interface()
            .unwrap()
            .get_editor_viewport_3d()
            .unwrap()
            .get_camera_3d();

        if let Ok(mut spacemouse) = SpaceMouseDevice::find_with_cache(Self::cache_path())
            && !spacemouse.is_polling()
        {
            spacemouse.start_polling();
            self.spacemouse = Some(spacemouse);
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

    fn process(&mut self, delta: f64) {
        if let Some(spacemouse) = self.spacemouse.as_ref()
            && self.focused
            && let Some(mut camera) = self.camera.take()
        {
            // handle polling thread stopping
            if !spacemouse.is_polling() {
                self.exit_tree();
                return;
            }

            let translation = *spacemouse.translation.lock();
            let rotation = *spacemouse.rotation.lock();

            if !translation.is_zero_approx() || !rotation.is_zero_approx() {
                if self.input_mode == InputMode::Fly {
                    camera.translate(translation * 0.05 * delta as f32);
                    let new_rotation = camera.get_rotation() + (rotation * 0.025 * delta as f32);
                    camera.set_rotation(new_rotation);
                } else {
                    let middle_of_screen =
                        camera.get_viewport().unwrap().get_visible_rect().center();
                    let from = camera.get_position();
                    let to = from + camera.project_ray_normal(middle_of_screen) * 10.0;

                    if let Some(query) = PhysicsRayQueryParameters3D::create(from, to) {
                        let mut space_state = camera
                            .get_world_3d()
                            .unwrap()
                            .get_direct_space_state()
                            .unwrap();
                        let result = space_state.intersect_ray(&query);

                        if self.grab_position.is_none() {
                            let new_grab_pos: Vector3 =
                                result.get("position").map_or(to, |p| p.to());
                            self.grab_position = Some(new_grab_pos);
                            self.redraw_grab_ball();
                        }

                        if let Some(grab_position) = self.grab_position {
                            let mut camera_transform = camera.get_transform();
                            let camera_origin = camera_transform.origin;

                            let offset_speed =
                                (grab_position.distance_to(camera_origin) / 8.0) + 0.01;
                            let offset_speed = offset_speed.clamp(0.01, 8.0);

                            let space_trans = camera_transform * translation * GRAB_MODE_MOVE_FLIP;
                            let space_trans = space_trans * 0.1 * offset_speed * delta as f32;
                            let space_rot =
                                rotation * 0.02 * GRAB_MODE_ROTATION_FLIP * delta as f32;

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
                self.redraw_grab_ball();
            }

            self.camera = Some(camera);
        }
    }
}
