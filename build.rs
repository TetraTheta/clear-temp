use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
  println!("cargo:rerun-if-changed=app.manifest");
  println!("cargo:rerun-if-changed=ClearTemp.txt");

  if env::var_os("CARGO_CFG_WINDOWS").is_some() && env::var("PROFILE").is_ok_and(|profile| profile == "release") {
    winresource::WindowsResource::new().set_manifest_file("app.manifest").compile()?;
  }

  let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"))
    .ancestors()
    .nth(3)
    .expect("Cargo output directory has the expected layout")
    .to_path_buf();
  fs::copy("ClearTemp.txt", output_directory.join("ClearTemp.txt"))?;

  Ok(())
}
