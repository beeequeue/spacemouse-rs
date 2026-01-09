export def main [
  --release
  target_dir: path
] {
  mkdir $target_dir

  # addons/magicmouse/
  # MagicMouse.gdextension
  # lib/[all lib files]

  let base_path: path = ($target_dir | path join addons spacemouse2)
  let files = {
    "MagicMouse.gdextension": "MagicMouse.gdextension"
    "target/:MODE:/godot_magicmouse.dll": "lib/godot_magicmouse.dll"
    "target/:MODE:/libgodot_magicmouse.dylib": "lib/libgodot_magicmouse.dylib"
    "target/:MODE:/libgodot_magicmouse.so": "lib/libgodot_magicmouse.so"
  }

  let os: string = (sys host).name
  let output_dir = if $release { "release" } else { "debug" }

  $files | transpose from to | each { |it|
    let real_from = (
      $it.from
      | str replace -a ':MODE:' $output_dir
      | path expand
    )
    if not ($real_from | path exists -n) { return }
    #print $"($it.from) -> ($it.to)"

    let real_to: path = (
      $base_path
      | path join $it.to
      | path expand
    )
    mkdir ($real_to | path dirname)

    if $os == Windows {
      if ($real_to | path exists -n) { rm $real_to }
      cp $real_from $real_to
      #mklink /H $real_to $real_from
    }
  }
}
