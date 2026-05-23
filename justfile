build:
  cargo build
  make -C limine

  mkdir -p iso_root/boot
  cp -v target/x86_64-unknown-none/debug/titanium-rune.elf iso_root/boot/rune
  mkdir -p iso_root/boot/limine
  cp -v limine.conf limine/limine-bios.sys limine/limine-bios-cd.bin \
        limine/limine-uefi-cd.bin iso_root/boot/limine/

  mkdir -p iso_root/EFI/BOOT
  cp -v limine/BOOTX64.EFI iso_root/EFI/BOOT
  cp -v limine/BOOTIA32.EFI iso_root/EFI/BOOT

  xorriso -as mkisofs -R -r -J -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus \
    -apm-block-size 2048 --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    iso_root -o image.iso

  ./limine/limine bios-install image.iso

run:
  qemu-system-x86_64 -boot d -cdrom image.iso -m 8G -serial stdio -enable-kvm

fresh: build run
