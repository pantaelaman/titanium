pub struct AlignedTo<Align, Bytes: ?Sized> {
  pub _align: [Align; 0],
  pub bytes: Bytes,
}

pub trait Cast<T> {
  fn cast(self) -> T;
}

macro_rules! impl_cast {
  ($from:ty as $to:ty) => {
    impl Cast<$to> for $from {
      #[inline]
      fn cast(self) -> $to {
        self as $to
      }
    }
  };
  ($from:ty as $to:ty, $($froms:ty as $tos:ty),+) => {
    impl_cast!($from as $to);
    impl_cast!($($froms as $tos),+);
  };
}

impl_cast! {
  u8 as u16, u8 as u32, u8 as u64,
  u16 as u8, u16 as u32, u16 as u64,
  u32 as u8, u32 as u16, u32 as u64,
  u64 as u8, u64 as u16, u64 as u32
}

#[macro_export]
macro_rules! bitfield_volatile_bitrange {
  (struct $name:ident($t:ty)) => {
    impl<T> ::bitfield::BitRange<T> for $name
    where $t: crate::util::Cast<T> {
      fn bit_range(&self, msb: usize, lsb: usize) -> T {
        let value = unsafe {
          ::core::ptr::read_volatile(&self.0 as *const $t)
        };

        let width = msb - lsb + 1;
        let mask = (1 << width) - 1;
        crate::util::Cast::cast(value >> lsb & mask)
      }
    }

    impl<T: crate::util::Cast<$t>> ::bitfield::BitRangeMut<T> for $name {
      fn set_bit_range(&mut self, msb: usize, lsb: usize, value: T) {
        let width = msb - lsb + 1;
        let mask = ((1 << width) - 1) << lsb;
        let old_value = self.0;
        unsafe {
          ::core::ptr::write_volatile(&mut self.0 as *mut $t, (old_value & !mask) | (crate::util::Cast::cast(value) & mask))
        }
      }
    }
  }
}
