use crate::integers::{U24, U48};
use core::ops::{Deref, DerefMut};

pub mod embedded {
    use heapless::Vec;

    /// A buffer handler wrapping a mutable slice.
    ///
    /// It allows for building DTLS records with look back for writing lengths.
    pub struct HeaplessTlsBuffer {
        buf: Vec<u8, 1024>,
        offset: usize,
    }

    impl HeaplessTlsBuffer {
        /// Create a new buffer wrapper.
        #[inline]
        pub fn new() -> Self {
            Self {
                buf: Vec::new(),
                offset: 0,
            }
        }
    }
}

pub mod std {
    // TODO
}

pub trait DTlsBuffer: Deref<Target = [u8]> + DerefMut {
    /// The current length.
    fn len(&self) -> usize;

    /// Extend from a slice.
    fn extend_from_slice(&mut self, slice: &[u8]) -> Result<(), ()>;

    /// Push a byte.
    fn push_u8(&mut self, val: u8) -> Result<(), ()>;

    /// Push a u16 in big-endian.
    fn push_u16_be(&mut self, val: u16) -> Result<(), ()> {
        DTlsBuffer::extend_from_slice(self, &val.to_be_bytes())
    }

    /// Push a u24 in big-endian.
    fn push_u24_be(&mut self, val: U24) -> Result<(), ()> {
        DTlsBuffer::extend_from_slice(self, &val.to_be_bytes())
    }

    /// Push a u32 in big-endian.
    fn push_u32_be(&mut self, val: u32) -> Result<(), ()> {
        DTlsBuffer::extend_from_slice(self, &val.to_be_bytes())
    }

    /// Push a u48 in big-endian.
    fn push_u48_be(&mut self, val: U48) -> Result<(), ()> {
        DTlsBuffer::extend_from_slice(self, &val.to_be_bytes())
    }

    /// Allocate the space for a `u8` for later updating.
    fn alloc_u8(&mut self) -> Result<AllocU8Handle, ()> {
        let index = DTlsBuffer::len(self);
        DTlsBuffer::push_u8(self, 0)?;

        Ok(AllocU8Handle { index })
    }

    /// Allocate the space for a `u16` for later updating.
    fn alloc_u16(&mut self) -> Result<AllocU16Handle, ()> {
        let index = DTlsBuffer::len(self);
        DTlsBuffer::push_u16_be(self, 0)?;

        Ok(AllocU16Handle { index })
    }

    /// Allocate the space for a `u24` for later updating.
    fn alloc_u24(&mut self) -> Result<AllocU24Handle, ()> {
        let index = DTlsBuffer::len(self);
        DTlsBuffer::push_u24_be(self, U24::new(0))?;

        Ok(AllocU24Handle { index })
    }

    /// Allocate the space for a `u48` for later updating.
    fn alloc_u48(&mut self) -> Result<AllocU48Handle, ()> {
        let index = DTlsBuffer::len(self);
        DTlsBuffer::push_u48_be(self, U48::new(0))?;

        Ok(AllocU48Handle { index })
    }

    /// Allocate space for a slice for later updating.
    fn alloc_slice(&mut self, len: usize) -> Result<AllocSliceHandle, ()> {
        let index = DTlsBuffer::len(self);

        for _ in 0..len {
            DTlsBuffer::push_u8(self, 0)?;
        }

        Ok(AllocSliceHandle { index, len })
    }
}

/// Handle to an allocated `u8` spot in a `DTlsBuffer`.
pub struct AllocU8Handle {
    index: usize,
}

impl AllocU8Handle {
    /// Set the value.
    pub fn set(self, buf: &mut impl DTlsBuffer, val: u8) {
        buf[self.index] = val;
        core::mem::forget(self);
    }
}

impl Drop for AllocU8Handle {
    fn drop(&mut self) {
        panic!("Alloc handle dropped without being used!");
    }
}

/// Handle to an allocated `u16` spot in a `DTlsBuffer`.
pub struct AllocU16Handle {
    index: usize,
}

impl AllocU16Handle {
    /// Set the value.
    pub fn set(self, buf: &mut impl DTlsBuffer, val: u16) {
        buf[self.index..self.index + 2].copy_from_slice(&val.to_be_bytes());
        core::mem::forget(self);
    }
}

impl Drop for AllocU16Handle {
    fn drop(&mut self) {
        panic!("Alloc handle dropped without being used!");
    }
}

/// Handle to an allocated `u24` spot in a `DTlsBuffer`.
pub struct AllocU24Handle {
    index: usize,
}

impl AllocU24Handle {
    /// Set the value.
    pub fn set(self, buf: &mut impl DTlsBuffer, val: U24) {
        buf[self.index..self.index + 3].copy_from_slice(&val.to_be_bytes());
        core::mem::forget(self);
    }
}

impl Drop for AllocU24Handle {
    fn drop(&mut self) {
        panic!("Alloc handle dropped without being used!");
    }
}

/// Handle to an allocated `u48` spot in a `DTlsBuffer`.
pub struct AllocU48Handle {
    index: usize,
}

impl AllocU48Handle {
    /// Set the value.
    pub fn set(self, buf: &mut impl DTlsBuffer, val: U48) {
        buf[self.index..self.index + 6].copy_from_slice(&val.to_be_bytes());
        core::mem::forget(self);
    }
}

impl Drop for AllocU48Handle {
    fn drop(&mut self) {
        panic!("Alloc handle dropped without being used!");
    }
}

/// Handle to an allocated slice in a `DTlsBuffer`.
pub struct AllocSliceHandle {
    index: usize,
    len: usize,
}

impl AllocSliceHandle {
    /// Set the value.
    pub fn set(self, buf: &mut impl DTlsBuffer, val: &[u8]) {
        buf[self.index..self.index + self.len].copy_from_slice(val);
        core::mem::forget(self);
    }
}

impl Drop for AllocSliceHandle {
    fn drop(&mut self) {
        panic!("Alloc handle dropped without being used!");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn push() {}

    #[test]
    fn alloc() {}
}
