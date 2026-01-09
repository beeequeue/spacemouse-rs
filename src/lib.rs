use godot::{
    classes::{Control, EditorPlugin, IEditorPlugin, editor_plugin::DockSlot},
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
}

#[godot_api]
impl IEditorPlugin for SpaceMousePlugin {
    fn enter_tree(&mut self) {
        print(&[Variant::from("Hello world?")]);
        let settings_scene = load::<PackedScene>("res://addons/spacemouse2/settings.tscn");
        let settings_control = settings_scene
            .instantiate()
            .unwrap()
            .try_cast::<Control>()
            .unwrap();
        println!("{:#?}", settings_control);
        self.to_gd()
            .add_control_to_dock(DockSlot::LEFT_UR, &settings_control);
    }

    fn exit_tree(&mut self) {
        // Perform typical plugin operations here.
    }
}
