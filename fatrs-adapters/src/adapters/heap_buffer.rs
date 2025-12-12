//! Heap-allocated page buffer adapter (runtime sizing).

#[cfg(feature = "alloc")]
use crate::{
    adapters::{BlockDeviceAdapter, error::HeapAdapterError},
    domain::{PageBuffer, PageConfig, PageNumber},
};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(feature = "alloc")]
use fatrs_block_device::BlockDevice;

/// Heap-allocated page buffer with runtime-determined size.
///
/// This adapter provides a page buffer where the page size is determined at
/// runtime. Requires the `alloc` feature for heap allocation.
///
/// Perfect for scenarios where:
/// - Page sizes are not known at compile-time
/// - Very large pages are needed (128KB, 1MB, etc.)
/// - Runtime flexibility is more important than zero allocation
///
/// # Type Parameters
///
/// - `D`: The block device type
/// - `BLOCK_SIZE`: The block size in bytes (must match the device's block size)
///
/// # Examples
///
/// ```ignore
/// use fatrs_adapters::adapters::{HeapBuffer, presets};
///
/// let device = MyBlockDevice::new();
/// let mut buffer = HeapBuffer::new(device, presets::PAGE_128K)?;
///
/// buffer.load(0).await?;
/// buffer.modify(|data| data[0] = 42)?;
/// buffer.flush().await?;
/// ```
#[cfg(feature = "alloc")]
pub struct HeapBuffer<D, const BLOCK_SIZE: usize>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    inner: PageBuffer<BlockDeviceAdapter<D, BLOCK_SIZE>, Vec<u8>, BLOCK_SIZE>,
}

#[cfg(feature = "alloc")]
impl<D, const BLOCK_SIZE: usize> HeapBuffer<D, BLOCK_SIZE>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    /// Create a new heap-allocated page buffer with the specified page size.
    ///
    /// # Arguments
    ///
    /// * `device` - The block device to use for storage
    /// * `page_size` - Size of each page in bytes (must be multiple of BLOCK_SIZE)
    ///
    /// # Errors
    ///
    /// Returns an error if `page_size` is not a multiple of `BLOCK_SIZE`.
    pub fn new(device: D, page_size: usize) -> Result<Self, HeapAdapterError<D::Error>> {
        extern crate alloc;
        use alloc::format;

        let adapter = BlockDeviceAdapter::new(device);
        let config = PageConfig::from_page_size(page_size)
            .map_err(|e| HeapAdapterError::Domain(format!("{}", e)))?;

        let inner = PageBuffer::new(adapter, config);

        Ok(Self { inner })
    }

    /// Load a page by number.
    pub async fn load(&mut self, page_num: u32) -> Result<(), HeapAdapterError<D::Error>> {
        self.inner
            .load(PageNumber::new(page_num))
            .await
            .map_err(HeapAdapterError::from_domain)
    }

    /// Get immutable access to the current page data.
    pub fn data(&self) -> Result<&[u8], HeapAdapterError<D::Error>> {
        self.inner
            .current()
            .map(|page| page.data())
            .map_err(HeapAdapterError::from_domain)
    }

    /// Get mutable access to the current page data (marks page as dirty).
    pub fn data_mut(&mut self) -> Result<&mut [u8], HeapAdapterError<D::Error>> {
        self.inner
            .current_mut()
            .map(|page| page.data_mut())
            .map_err(HeapAdapterError::from_domain)
    }

    /// Modify the current page with a closure.
    pub fn modify<F>(&mut self, f: F) -> Result<(), HeapAdapterError<D::Error>>
    where
        F: FnOnce(&mut [u8]),
    {
        self.inner.modify(f).map_err(HeapAdapterError::from_domain)
    }

    /// Flush any uncommitted changes.
    pub async fn flush(&mut self) -> Result<(), HeapAdapterError<D::Error>> {
        self.inner.flush().await.map_err(HeapAdapterError::from_domain)
    }

    /// Clear the buffer (discards loaded page).
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Get the currently loaded page number, if any.
    pub fn current_page(&self) -> Option<u32> {
        self.inner.current().ok().map(|p| p.number().value())
    }

    /// Check if the current page is dirty.
    pub fn is_dirty(&self) -> bool {
        self.inner.current().ok().map(|p| p.is_dirty()).unwrap_or(false)
    }

    /// Get the configured page size.
    pub fn page_size(&self) -> usize {
        self.inner.config().page_size()
    }

    /// Get storage size in bytes.
    pub async fn size(&mut self) -> Result<u64, HeapAdapterError<D::Error>> {
        self.inner
            .storage_size()
            .await
            .map_err(HeapAdapterError::from_domain)
    }

    /// Get storage size in pages.
    pub async fn size_in_pages(&mut self) -> Result<u64, HeapAdapterError<D::Error>> {
        self.inner
            .storage_size_in_pages()
            .await
            .map_err(HeapAdapterError::from_domain)
    }
}

/// Common page size presets for heap buffers.
#[cfg(feature = "alloc")]
pub mod presets {
    /// 4KB pages (typical for SD cards, flash drives).
    pub const PAGE_4K: usize = 4 * 1024;

    /// 8KB pages.
    pub const PAGE_8K: usize = 8 * 1024;

    /// 16KB pages.
    pub const PAGE_16K: usize = 16 * 1024;

    /// 32KB pages.
    pub const PAGE_32K: usize = 32 * 1024;

    /// 64KB pages.
    pub const PAGE_64K: usize = 64 * 1024;

    /// 128KB pages (optimal for many SSDs).
    pub const PAGE_128K: usize = 128 * 1024;

    /// 256KB pages.
    pub const PAGE_256K: usize = 256 * 1024;

    /// 512KB pages.
    pub const PAGE_512K: usize = 512 * 1024;

    /// 1MB pages (for very large sequential I/O).
    pub const PAGE_1M: usize = 1024 * 1024;
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use crate::adapters::block_device_adapter::tests::MockBlockDevice;

    #[tokio::test]
    async fn test_heap_buffer_creation() {
        let device = MockBlockDevice::<512>::new(1024 * 1024);
        let buffer = HeapBuffer::new(device, presets::PAGE_4K);
        assert!(buffer.is_ok());
    }

    #[tokio::test]
    async fn test_heap_buffer_invalid_size() {
        let device = MockBlockDevice::<512>::new(1024 * 1024);
        let buffer = HeapBuffer::new(device, 4000); // Not a multiple of 512
        assert!(buffer.is_err());
    }

    #[tokio::test]
    async fn test_heap_buffer_4k_block_size() {
        let device = MockBlockDevice::<4096>::new(1024 * 1024);
        let buffer = HeapBuffer::new(device, 4096);
        assert!(buffer.is_ok());
    }
}
