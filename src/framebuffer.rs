pub use crate::limine::Framebuffer;
use crate::serial_println;

struct AlignedTo<Align, Bytes: ?Sized> {
  _align: [Align; 0],
  bytes: Bytes,
}

const ALIGNED_ICON: &'static AlignedTo<u32, [u8]> = &AlignedTo {
  _align: [],
  bytes: *include_bytes!("../titanium.data"),
};

const ICON_HEIGHT: usize = 256;
const ICON_WIDTH: usize = 256;
const ICON_DATA: *const u32 = ALIGNED_ICON.bytes.as_ptr().cast();

pub fn draw_icon(framebuffer: &Framebuffer) {
  let (cx, cy) = (
    framebuffer.width as usize / 2,
    framebuffer.height as usize / 2,
  );
  let (sx, sy) = (cx - ICON_WIDTH / 2, cy - ICON_HEIGHT / 2);

  for y in 0..ICON_HEIGHT {
    let row_ptr = unsafe {
      framebuffer
        .address()
        .byte_add((sy + y) * framebuffer.pitch as usize)
        .cast::<u32>()
    };
    for x in 0..ICON_WIDTH {
      let px_data = unsafe { *ICON_DATA.add(x + y * ICON_HEIGHT) };
      unsafe {
        row_ptr
          .add(sx + x)
          .write_volatile(px_data);
      }
    }
  }
}

