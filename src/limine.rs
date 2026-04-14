use core::slice;

const LIMINE_COMMON_MAGIC: [u64; 2] = [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b];

#[repr(C, align(8))]
struct RequestHeader {
  common: [u64; 2],
  subid: [u64; 2],
  revision: u64,
  response: *const (),
}

unsafe impl Sync for RequestHeader {}

impl RequestHeader {
  pub const fn id(&self) -> &[u64; 4] {
    return unsafe { &*self.common.as_ptr().cast_array() };
  }
}

#[repr(C)]
struct ResponseHeader {
  revision: u64,
}

#[repr(C)]
struct FramebufferRequest {
  header: RequestHeader,
}

impl FramebufferRequest {
  pub const fn new() -> Self {
    Self {
      header: RequestHeader {
        common: LIMINE_COMMON_MAGIC,
        subid: [0x9d5827dcd881dd75, 0xa3148604f6fab11b],
        revision: 0,
        response: core::ptr::null(),
      },
    }
  }
}

#[repr(C)]
pub struct Framebuffer {
  address: *mut (),
  pub width: u64,
  pub height: u64,
  pub pitch: u64,
  pub bpp: u16,
  pub memory_model: u8,
  pub red_mask_size: u8,
  pub red_mask_shift: u8,
  pub green_mask_size: u8,
  pub green_mask_shift: u8,
  pub blue_mask_size: u8,
  pub blue_mask_shift: u8,
  pub unused: [u8; 7],
  edid_size: u64,
  edid: *const core::ffi::c_void,
}

impl Framebuffer {
  pub const fn address(&self) -> *mut () {
    self.address
  }

  pub const fn byte_size(&self) -> usize {
    (self.height * self.pitch) as usize
  }

  pub const fn pixel_size(&self) -> usize {
    (self.width * self.height) as usize
  }
}

#[repr(C)]
struct FramebufferResponse {
  header: ResponseHeader,
  framebuffer_count: u64,
  framebuffers: *const *const Framebuffer,
}

impl FramebufferResponse {
  pub const fn framebuffers(&self) -> &[&Framebuffer] {
    unsafe {
      slice::from_raw_parts(
        self.framebuffers as _,
        self.framebuffer_count as usize,
      )
    }
  }
}

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static _START_MARKER: [u64; 4] = [
  0xf6b8f4b39de7d1ae,
  0xfab91a6940fcb9cf,
  0x785c6ed015d3e316,
  0x181e920a7852b9d9,
];

#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: [u64; 3] = [0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 6];

#[used]
#[unsafe(link_section = ".limine_requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests_end")]
static _END_MARKER: [u64; 2] = [0xadc0e0531bb10d03, 0x9572709f31764c62];

pub fn is_base_revision_supported() -> bool {
  BASE_REVISION[2] == 0
}

pub fn framebuffers() -> Option<&'static [&'static Framebuffer]> {
  (!FRAMEBUFFER_REQUEST.header.response.is_null()).then(|| {
    unsafe {
      &*FRAMEBUFFER_REQUEST
        .header
        .response
        .cast::<FramebufferResponse>()
    }
    .framebuffers()
  })
}
