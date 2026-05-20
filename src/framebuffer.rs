pub use crate::limine::Framebuffer;
use crate::{serial_println, util::AlignedTo};

mod font;

const ALIGNED_ICON: &'static AlignedTo<u32, [u8]> = &AlignedTo {
  _align: [],
  bytes: *include_bytes!("../titanium.data"),
};

const ICON_HEIGHT: usize = 256;
const ICON_WIDTH: usize = 256;
const ICON_DATA: *const u32 = ALIGNED_ICON.bytes.as_ptr().cast();

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Point {
  pub x: usize,
  pub y: usize,
}

impl core::ops::Add for Point {
  type Output = Point;

  fn add(self, rhs: Self) -> Self::Output {
    Self { x: self.x + rhs.x, y: self.y + rhs.y }
  }
}

impl core::ops::Sub for Point {
  type Output = Point;

  fn sub(self, rhs: Self) -> Self::Output {
    Self { x: self.x - rhs.x, y: self.y - rhs.y }
  }
}

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
      let [r,g,b,a] = unsafe { *ICON_DATA.add(x + y * ICON_HEIGHT) }.to_le_bytes();
      let px_data = u32::from_le_bytes([b,g,r,a]);
      unsafe {
        row_ptr
          .add(sx + x)
          .write_volatile(px_data);
      }
    }
  }
}

pub fn print_str_at(framebuffer: &Framebuffer, point: Point, string: &str) {
  let galley = font::Galley {
    text: string,
    width: font::Dim::Unbounded,
    height: font::Dim::Unbounded,
  };
  const STYLE: font::Style = font::Style {
    fg: 0xff00ff00,
    bg: 0x00000000,
  };

  font::draw_galley_at(framebuffer, &galley, &STYLE, point);
}
