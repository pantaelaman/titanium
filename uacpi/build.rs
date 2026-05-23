pub fn main() {
  println!("cargo::rustc-link-arg=-nostdlib");

  let bindings = bindgen::Builder::default()
    .use_core()
    .header("uacpi.h")
    .rust_target(bindgen::RustTarget::nightly())
    .prepend_enum_name(false)
    .blocklist_file("uacpi/include/uacpi/internal/.*")
    .blocklist_item("uacpi_kernel_.*")
    .clang_arg("-Iuacpi/include")
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    .generate()
    .expect("Unable to generate uacpi bindings");

  let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
  bindings
    .write_to_file(out_path.join("uacpi_bindings.rs"))
    .expect("Couldn't write uacpi bindings");

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
    .flags([
      "-Iuacpi/include",
      "-Iinclude",
      "-nostdlib",
      "-pie",
      "-ffreestanding",
      "-fno-stack-protector",
    ])
    .compile("uacpi");
}
