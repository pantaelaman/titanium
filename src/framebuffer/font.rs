use crate::{serial_println, util::AlignedTo};

#[repr(C)]
struct PSFFontHeader {
  magic: u32,
  version: u32,
  headersize: u32,
  flags: u32,
  numglyph: u32,
  bytesperglyph: u32,
  height: u32,
  width: u32,
}

static ALIGNED_FONT: &'static AlignedTo<u32, [u8]> = &AlignedTo {
  _align: [],
  bytes: *include_bytes!("../../lat9-12.psf"),
};

static FONT_HEADER: &'static PSFFontHeader =
  unsafe { &*ALIGNED_FONT.bytes.as_ptr().cast() };

pub enum Dim {
  Unbounded,
  Strict(usize),
}

pub struct Galley<'g> {
  pub text: &'g str,
  pub width: Dim,
  pub height: Dim,
}

pub struct Style {
  pub fg: u32,
  pub bg: u32,
}

pub fn draw_galley_at(
  framebuffer: &super::Framebuffer,
  galley: &Galley,
  style: &Style,
  point: super::Point,
) {
  let mut offset = super::Point { x: 0, y: 0 };
  for c in galley.text.chars() {
    let c = c as u32;
    // check if we're too wide, wrap if so
    if let Dim::Strict(max_width) = galley.width
      && (offset.x + FONT_HEADER.width as usize > max_width
        || point.x + offset.x + FONT_HEADER.width as usize
          > framebuffer.width as usize)
    {
      // check if we're too tall
      if let Dim::Strict(max_height) = galley.height
        && (offset.y + FONT_HEADER.height as usize > max_height
          || point.y + offset.y + FONT_HEADER.height as usize
            > framebuffer.height as usize)
      {
        serial_println!("galley overflowed");
        return;
      }

      offset.x = 0;
      offset.y += FONT_HEADER.height as usize;
    }

    let glyph_idx = if c > FONT_HEADER.numglyph { 0 } else { c };
    let glyph_ptr = unsafe {
      ALIGNED_FONT.bytes.as_ptr().byte_add(
        (FONT_HEADER.headersize + glyph_idx * FONT_HEADER.bytesperglyph)
          as usize,
      )
    };

    // draw glyph
    let mut glyph_row_ptr = glyph_ptr;
    for y in 0..FONT_HEADER.height as usize {
      let row_ptr = unsafe {
        framebuffer
          .address()
          .byte_add((point.y + offset.y + y) * framebuffer.pitch as usize)
          .cast::<u32>()
      };
      for x in 0..FONT_HEADER.width as usize {
        let glyph_byte = unsafe { *glyph_row_ptr.byte_add(x / 8) };
        let px_data = if glyph_byte & (1 << (7 - (x % 8))) != 0 {
          style.fg
        } else {
          style.bg
        };

        unsafe {
          row_ptr.add(point.x + offset.x + x).write_volatile(px_data);
        }
      }
      glyph_row_ptr =
        unsafe { glyph_row_ptr.byte_add(FONT_HEADER.width as usize / 8) };
    }

    offset.x += FONT_HEADER.width as usize;
  }
}
