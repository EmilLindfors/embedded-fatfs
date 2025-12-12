//! Generic stream block device adapter
//!
//! Provides a `BlockDevice<512>` implementation wrapping any async I/O stream.

use aligned::{A4, Aligned};
use core::cell::RefCell;
use embedded_io_async::{ErrorType, Read, Seek, SeekFrom, Write};
use fatrs_block_device::BlockDevice;

const BLOCK_SIZE: usize = 512;

/// Block device wrapper for async I/O streams
///
/// Wraps any type implementing `embedded_io_async::{Read, Write, Seek}`
/// and provides the `BlockDevice<512>` trait.
///
/// Uses `RefCell` internally for interior mutability to allow `&self` on read operations.
///
/// This is useful for wrapping file handles, in-memory buffers, or any other
/// stream-like interface to provide block device access.
///
/// # Example
///
/// ```ignore
/// use fatrs_block_platform::StreamBlockDevice;
/// use embedded_io_adapters::tokio_1::FromTokio;
///
/// let file = tokio::fs::File::open("disk.img").await?;
/// let stream = FromTokio::new(file);
/// let block_dev = StreamBlockDevice::new(stream);
/// ```
pub struct StreamBlockDevice<T>(RefCell<T>);

impl<T> StreamBlockDevice<T> {
    /// Create a new StreamBlockDevice wrapping the given stream.
    pub fn new(inner: T) -> Self {
        Self(RefCell::new(inner))
    }

    /// Get a reference to the inner stream.
    ///
    /// # Panics
    /// Panics if the inner stream is currently borrowed mutably.
    pub fn inner(&self) -> core::cell::Ref<'_, T> {
        self.0.borrow()
    }

    /// Get a mutable reference to the inner stream.
    ///
    /// # Panics
    /// Panics if the inner stream is currently borrowed.
    pub fn inner_mut(&self) -> core::cell::RefMut<'_, T> {
        self.0.borrow_mut()
    }

    /// Consume the wrapper and return the inner stream.
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: ErrorType> ErrorType for StreamBlockDevice<T> {
    type Error = T::Error;
}

impl<T> BlockDevice<BLOCK_SIZE> for StreamBlockDevice<T>
where
    T: Read + Write + Seek,
{
    type Error = T::Error;
    type Align = A4;

    async fn read(
        &self,
        block_address: u32,
        data: &mut [Aligned<Self::Align, [u8; BLOCK_SIZE]>],
    ) -> Result<(), Self::Error> {
        let mut inner = self.0.borrow_mut();
        inner
            .seek(SeekFrom::Start((block_address as u64) * BLOCK_SIZE as u64))
            .await?;
        for block in data {
            let mut offset = 0;
            while offset < BLOCK_SIZE {
                let n = inner.read(&mut block[offset..]).await?;
                if n == 0 {
                    break; // EOF
                }
                offset += n;
            }
        }
        Ok(())
    }

    async fn write(
        &mut self,
        block_address: u32,
        data: &[Aligned<Self::Align, [u8; BLOCK_SIZE]>],
    ) -> Result<(), Self::Error> {
        let mut inner = self.0.borrow_mut();
        inner
            .seek(SeekFrom::Start((block_address as u64) * BLOCK_SIZE as u64))
            .await?;
        for block in data {
            let mut offset = 0;
            while offset < BLOCK_SIZE {
                let n = inner.write(&block[offset..]).await?;
                if n == 0 {
                    break; // Can't write more
                }
                offset += n;
            }
        }
        Ok(())
    }

    async fn size(&self) -> Result<u64, Self::Error> {
        let mut inner = self.0.borrow_mut();
        // For files, seek to end to get size
        // For block devices, this may return u64::MAX (handled by caller)
        let size = inner.seek(SeekFrom::End(0)).await?;
        // Seek back to beginning
        inner.seek(SeekFrom::Start(0)).await?;
        Ok(size)
    }

    async fn sync(&mut self) -> Result<(), Self::Error> {
        let mut inner = self.0.borrow_mut();
        inner.flush().await
    }
}
