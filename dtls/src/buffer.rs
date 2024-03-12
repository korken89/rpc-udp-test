/// A buffer hander wrapping a mutable slice.
///
/// It allows for building DTLS records with look back for writing lengths.
pub struct TlsBuffer<'a> {
    buf: &'a mut [u8],
    offset: usize,
    len: usize,
}

impl<'a> TlsBuffer<'a> {
    /// Create a new buffer wrapper.
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            offset: 0,
            len: 0,
        }
    }

    // TODO: More to come.
}
