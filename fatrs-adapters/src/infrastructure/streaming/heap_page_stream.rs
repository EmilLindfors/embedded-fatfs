//! Heap-allocated streaming page buffer with runtime-configurable size.

use crate::{
    adapters::{HeapAdapterError, HeapBuffer},
    domain::BLOCK_SIZE,
    infrastructure::streaming::{SeekFrom, StreamError},
};
use fatrs_block_device::BlockDevice;

#[cfg(feature = "alloc")]
extern crate alloc;

/// Heap-allocated streaming page buffer with runtime-configurable size.
///
/// This wraps `HeapBuffer` and adds async Read/Write/Seek capabilities.
/// Perfect for systems that need flexible page sizes determined at runtime.
///
/// # Type Parameters
///
/// - `D`: The block device type
///
/// # Send/Sync Properties
///
/// The Send/Sync properties are automatically inherited from the underlying
/// HeapBuffer and device. No manual bounds needed!
///
/// # Examples
///
/// ```ignore
/// use fatrs_adapters::infrastructure::{HeapPageStream, presets};
///
/// let device = MyBlockDevice::new();
/// let mut stream = HeapPageStream::new(device, presets::PAGE_128K);
///
/// // Use async read/write/seek
/// stream.write(&data).await?;
/// stream.seek(SeekFrom::Start(0)).await?;
/// stream.read(&mut buffer).await?;
/// ```
#[cfg(feature = "alloc")]
pub struct HeapPageStream<D>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    buffer: HeapBuffer<D>,
    position: u64,
    page_size: usize,
}

#[cfg(feature = "alloc")]
impl<D> HeapPageStream<D>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    /// Create a new large page stream with the specified page size.
    ///
    /// # Arguments
    ///
    /// * `device` - The block device to buffer
    /// * `page_size` - Size of each page in bytes (must be a multiple of 512)
    ///
    /// # Errors
    ///
    /// Returns an error if the page size is invalid (not a multiple of 512 or zero).
    pub fn new(device: D, page_size: usize) -> Result<Self, HeapAdapterError<D::Error>> {
        if page_size == 0 {
            return Err(HeapAdapterError::Domain("Page size must be non-zero".into()));
        }
        if page_size % BLOCK_SIZE != 0 {
            return Err(HeapAdapterError::Domain(
                alloc::format!("Page size {} must be a multiple of block size (512)", page_size)
            ));
        }

        Ok(Self {
            buffer: HeapBuffer::new(device, page_size)?,
            position: 0,
            page_size,
        })
    }

    /// Create a new large page stream, panicking on error.
    ///
    /// This is a convenience method for cases where the page size is known to be valid.
    ///
    /// # Panics
    ///
    /// Panics if `page_size` is not a multiple of 512 or is zero.
    pub fn new_unwrap(device: D, page_size: usize) -> Self {
        Self::new(device, page_size).expect("Failed to create HeapPageStream with valid page size")
    }

    /// Read data from the stream.
    ///
    /// Reads up to `buf.len()` bytes from the current position.
    /// Returns the number of bytes read.
    ///
    /// Note: This method is internal. Users should use the `embedded_io_async::Read` trait.
    pub(crate) async fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError<D::Error>> {
        if buf.is_empty() {
            return Ok(0);
        }

        let page_num = (self.position / self.page_size as u64) as u32;
        let page_offset = (self.position % self.page_size as u64) as usize;

        // Load the page containing current position
        self.buffer
            .load(page_num)
            .await
            .map_err(|e| match e {
                HeapAdapterError::Storage(s) => StreamError::Storage(s),
                _ => StreamError::OutOfBounds,
            })?;

        // Read from current page
        let data = self.buffer.data().map_err(|e| match e {
            HeapAdapterError::Storage(s) => StreamError::Storage(s),
            _ => StreamError::OutOfBounds,
        })?;

        let available = data.len().saturating_sub(page_offset);
        let to_read = buf.len().min(available);

        if to_read > 0 {
            buf[..to_read].copy_from_slice(&data[page_offset..page_offset + to_read]);
            self.position += to_read as u64;
        }

        Ok(to_read)
    }

    /// Write data to the stream.
    ///
    /// Writes `buf.len()` bytes to the current position.
    /// Returns the number of bytes written.
    ///
    /// Note: This method is internal. Users should use the `embedded_io_async::Write` trait.
    pub(crate) async fn write(&mut self, buf: &[u8]) -> Result<usize, StreamError<D::Error>> {
        if buf.is_empty() {
            return Ok(0);
        }

        let page_num = (self.position / self.page_size as u64) as u32;
        let page_offset = (self.position % self.page_size as u64) as usize;

        // Load the page containing current position
        self.buffer
            .load(page_num)
            .await
            .map_err(|e| match e {
                HeapAdapterError::Storage(s) => StreamError::Storage(s),
                _ => StreamError::OutOfBounds,
            })?;

        // Write to current page
        let data = self.buffer.data_mut().map_err(|e| match e {
            HeapAdapterError::Storage(s) => StreamError::Storage(s),
            _ => StreamError::OutOfBounds,
        })?;

        let available = data.len().saturating_sub(page_offset);
        let to_write = buf.len().min(available);

        if to_write > 0 {
            data[page_offset..page_offset + to_write].copy_from_slice(&buf[..to_write]);
            self.position += to_write as u64;
        }

        Ok(to_write)
    }

    /// Flush any uncommitted changes to storage.
    ///
    /// Note: This method is internal. Users should use the `embedded_io_async::Write` trait.
    pub(crate) async fn flush(&mut self) -> Result<(), StreamError<D::Error>> {
        self.buffer.flush().await.map_err(|e| match e {
            HeapAdapterError::Storage(s) => StreamError::Storage(s),
            _ => StreamError::OutOfBounds,
        })
    }

    /// Seek to a new position in the stream.
    ///
    /// Returns the new position from the start of the stream.
    ///
    /// Note: This method is internal. Users should use the `embedded_io_async::Seek` trait.
    pub(crate) async fn seek(&mut self, pos: SeekFrom) -> Result<u64, StreamError<D::Error>> {
        // Flush before seeking to ensure data consistency
        self.flush().await?;

        let size = self.buffer.size().await.map_err(|e| match e {
            HeapAdapterError::Storage(s) => StreamError::Storage(s),
            _ => StreamError::OutOfBounds,
        })?;

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(offset) => size as i64 + offset,
        };

        if new_pos < 0 {
            return Err(StreamError::InvalidSeek);
        }

        let old_page = self.position / self.page_size as u64;
        let new_page = new_pos as u64 / self.page_size as u64;

        // If seeking to a different page, clear the buffer to avoid corruption
        // This ensures the next read/write loads the correct page
        if old_page != new_page {
            self.buffer.clear();
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }

    /// Get the current position in the stream.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the size of the underlying storage in bytes.
    pub async fn size(&mut self) -> Result<u64, StreamError<D::Error>> {
        self.buffer.size().await.map_err(|e| match e {
            HeapAdapterError::Storage(s) => StreamError::Storage(s),
            _ => StreamError::OutOfBounds,
        })
    }

    /// Get the page size in bytes.
    pub fn page_size(&self) -> usize {
        self.page_size
    }
}
