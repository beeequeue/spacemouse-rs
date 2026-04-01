export def main [
  --release
  target_dir: path
] {
  mkdir $target_dir

  let base_path: path = ($target_dir | path join addons spacemouse)
  let files = {
    "godot/spacemouse.gdextension": "spacemouse.gdextension"
    "target/:MODE:/godot_spacemouse.dll": "lib/godot_spacemouse.dll"
    "target/:MODE:/libgodot_spacemouse.dylib": "lib/libgodot_spacemouse.dylib"
    "target/:MODE:/libgodot_spacemouse.so": "lib/libgodot_spacemouse.so"
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
      if ($real_to | path exists -n) { rm -t $real_to }
      #cp $real_from $real_to
      mklink $real_to $real_from
    }
  }
}
