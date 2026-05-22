use std::{env, path::PathBuf};

use bindgen::RustTarget;

fn main() {
  let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
  assert_eq!(arch, "x86_64");

  println!("cargo::rustc-link-arg=-nostdlib");

  let bindings = bindgen::Builder::default()
    .use_core()
    .header("uacpi.h")
    .rust_target(RustTarget::nightly())
    .blocklist_function("uacpi_kernel_.*")
    .clang_arg("-Iuacpi/include")
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    .generate()
    .expect("Unable to generate uacpi bindings");

  let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
  bindings
    .write_to_file(out_path.join("uacpi_bindings.rs"))
    .expect("Couldn't write uacpi bindings");

  const UACPI_LIB: &str = "uacpi";
  cc::Build::new()
    .files(
      std::fs::read_dir("uacpi/source")
        .expect("Source directory for uacpi doesn't exist")
        .filter_map(|entry| {
          let entry = entry.ok()?;
          entry
            .file_name()
            .as_os_str()
            .to_str()?
            .ends_with(".c")
            .then_some(entry.path())
        }),
    )
    .flags(["-Iuacpi/include", "-Iinclude", "-nostdlib", "-pie", "-ffreestanding"])
    .compile(UACPI_LIB);
}
