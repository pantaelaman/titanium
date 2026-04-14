fn main() {
  let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
  assert_eq!(arch, "x86_64");

  println!("cargo:rustc-link-arg=-fuse-ld=ld");
  println!("cargo:rustc-link-arg=-Tlinker-x86_64.ld");
  println!("cargo:rerun-if-changed=linker-x86_64.ld");
}
