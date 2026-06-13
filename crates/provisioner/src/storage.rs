/// Backend-independent storage trait used by the provisioner to persist
/// configuration bytes across reboots.
pub trait Storage {
    type Error;

    /// Load previously stored bytes into `buf`.
    ///
    /// Returns the number of bytes written to `buf`. If `buf` is larger than
    /// the stored payload the implementation may leave the tail unchanged.
    fn load(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;

    /// Persist `data` so that a future `load` call can retrieve it.
    fn store(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}

/// In-memory [`Storage`] implementation for tests, examples, and host-side
/// simulations.
///
/// `N` is the maximum number of bytes that can be stored. The internal buffer
/// is stack-allocated and the type is `no_std`/`no_alloc` compatible.
///
/// # Example
///
/// ```
/// use provisioner::storage::{MockStorage, Storage};
///
/// let mut storage: MockStorage<64> = MockStorage::new();
/// storage.store(b"ssid=MyWiFi").unwrap();
///
/// let mut buf = [0u8; 64];
/// let len = storage.load(&mut buf).unwrap();
/// assert_eq!(&buf[..len], b"ssid=MyWiFi");
/// ```
#[derive(Debug, Clone)]
pub struct MockStorage<const N: usize> {
    buf: [u8; N],
    len: usize,
}

/// Errors returned by [`MockStorage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockStorageError {
    /// The payload exceeded the fixed capacity `N`.
    BufferTooSmall,
}

impl<const N: usize> MockStorage<N> {
    /// Create a new empty [`MockStorage`].
    pub const fn new() -> Self {
        Self {
            buf: [0u8; N],
            len: 0,
        }
    }

    /// Return the currently stored payload.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Return the configured capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Return the length of the currently stored payload.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no payload is currently stored.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear the stored payload.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> Default for MockStorage<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Storage for MockStorage<N> {
    type Error = MockStorageError;

    fn load(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let len = self.len.min(buf.len());
        buf[..len].copy_from_slice(&self.buf[..len]);
        Ok(len)
    }

    fn store(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        if data.len() > N {
            return Err(MockStorageError::BufferTooSmall);
        }
        self.buf[..data.len()].copy_from_slice(data);
        self.len = data.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_storage_is_empty() {
        let storage: MockStorage<32> = MockStorage::new();
        assert!(storage.is_empty());
        assert_eq!(storage.len(), 0);
        assert_eq!(storage.capacity(), 32);
    }

    #[test]
    fn store_and_load_roundtrip() {
        let mut storage: MockStorage<64> = MockStorage::new();
        storage.store(b"hello world").unwrap();

        let mut buf = [0u8; 64];
        let len = storage.load(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"hello world");
        assert_eq!(storage.as_slice(), b"hello world");
    }

    #[test]
    fn load_into_smaller_buffer_is_truncated() {
        let mut storage: MockStorage<64> = MockStorage::new();
        storage.store(b"long payload").unwrap();

        let mut buf = [0u8; 4];
        let len = storage.load(&mut buf).unwrap();
        assert_eq!(len, 4);
        assert_eq!(&buf, b"long");
    }

    #[test]
    fn store_too_large_fails() {
        let mut storage: MockStorage<4> = MockStorage::new();
        assert_eq!(
            storage.store(b"hello"),
            Err(MockStorageError::BufferTooSmall)
        );
    }

    #[test]
    fn clear_removes_payload() {
        let mut storage: MockStorage<32> = MockStorage::new();
        storage.store(b"data").unwrap();
        assert!(!storage.is_empty());

        storage.clear();
        assert!(storage.is_empty());
        assert_eq!(storage.as_slice(), b"");
    }

    #[test]
    fn store_overwrites_previous_payload() {
        let mut storage: MockStorage<32> = MockStorage::new();
        storage.store(b"first").unwrap();
        storage.store(b"second").unwrap();

        assert_eq!(storage.as_slice(), b"second");
    }

    #[test]
    fn default_is_empty() {
        let storage: MockStorage<16> = MockStorage::default();
        assert!(storage.is_empty());
    }
}
