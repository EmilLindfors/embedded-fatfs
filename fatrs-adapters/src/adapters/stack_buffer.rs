//! Stack-allocated page buffer adapter (const generic sizing).

use crate::{
    adapters::{BlockDeviceAdapter, error::AdapterError},
    domain::{PageBuffer, PageConfig, PageNumber},
};
use fatrs_block_device::BlockDevice;

/// Stack-allocated page buffer with compile-time size.
///
/// This adapter provides a page buffer with a size determined at compile-time
/// using const generics. Perfect for `no_std` environments where heap allocation
/// is not available.
///
/// # Type Parameters
///
/// - `D`: The block device type
/// - `N`: Page size in bytes (must be a multiple of BLOCK_SIZE)
/// - `BLOCK_SIZE`: The block size in bytes (must match the device's block size)
///
/// # Examples
///
/// ```ignore
/// use fatrs_adapters::adapters::StackBuffer;
///
/// // 4KB pages with 512-byte blocks
/// let device = MyBlockDevice::new();
/// let mut buffer = StackBuffer::<_, 4096, 512>::new(device);
///
/// buffer.load(0).await?;
/// buffer.modify(|data| data[0] = 42)?;
/// buffer.flush().await?;
/// ```
pub struct StackBuffer<D, const N: usize, const BLOCK_SIZE: usize>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    inner: PageBuffer<BlockDeviceAdapter<D, BLOCK_SIZE>, [u8; N], BLOCK_SIZE>,
}

impl<D, const N: usize, const BLOCK_SIZE: usize> StackBuffer<D, N, BLOCK_SIZE>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    /// Create a new stack-allocated page buffer.
    ///
    /// The page size is `N` bytes (must be a multiple of BLOCK_SIZE).
    pub fn new(device: D) -> Self {
        let adapter = BlockDeviceAdapter::new(device);
        let blocks_per_page = N / BLOCK_SIZE;
        let config = PageConfig::new(N, blocks_per_page);
        let inner = PageBuffer::new_stack(adapter, config);

        Self { inner }
    }

    /// Load a page by number.
    pub async fn load(&mut self, page_num: u32) -> Result<(), AdapterError<D::Error>> {
        self.inner
            .load(PageNumber::new(page_num))
            .await
            .map_err(AdapterError::from_domain)
    }

    /// Get immutable access to the current page data.
    pub fn data(&self) -> Result<&[u8], AdapterError<D::Error>> {
        self.inner
            .current()
            .map(|page| page.data())
            .map_err(AdapterError::from_domain)
    }

    /// Get mutable access to the current page data (marks page as dirty).
    pub fn data_mut(&mut self) -> Result<&mut [u8], AdapterError<D::Error>> {
        self.inner
            .current_mut()
            .map(|page| page.data_mut())
            .map_err(AdapterError::from_domain)
    }

    /// Modify the current page with a closure.
    pub fn modify<F>(&mut self, f: F) -> Result<(), AdapterError<D::Error>>
    where
        F: FnOnce(&mut [u8]),
    {
        self.inner.modify(f).map_err(AdapterError::from_domain)
    }

    /// Flush any uncommitted changes.
    pub async fn flush(&mut self) -> Result<(), AdapterError<D::Error>> {
        self.inner.flush().await.map_err(AdapterError::from_domain)
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

    /// Get storage size in bytes.
    pub async fn size(&mut self) -> Result<u64, AdapterError<D::Error>> {
        self.inner
            .storage_size()
            .await
            .map_err(AdapterError::from_domain)
    }

    /// Get storage size in pages.
    pub async fn size_in_pages(&mut self) -> Result<u64, AdapterError<D::Error>> {
        self.inner
            .storage_size_in_pages()
            .await
            .map_err(AdapterError::from_domain)
    }

    /// Get the page configuration.
    pub const fn config(&self) -> &PageConfig<BLOCK_SIZE> {
        self.inner.config()
    }
}

/// Type alias for 4KB page buffer with 512-byte blocks.
pub type StackBuffer4K<D> = StackBuffer<D, 4096, 512>;

/// Type alias for 8KB page buffer with 512-byte blocks.
pub type StackBuffer8K<D> = StackBuffer<D, 8192, 512>;

/// Type alias for 2KB page buffer with 512-byte blocks.
pub type StackBuffer2K<D> = StackBuffer<D, 2048, 512>;

/// Type alias for 4KB page buffer with 4096-byte blocks.
pub type StackBuffer4KBlock4K<D> = StackBuffer<D, 4096, 4096>;

/// Type alias for 128KB page buffer with 128KB blocks.
pub type StackBuffer128KBlock128K<D> = StackBuffer<D, { 128 * 1024 }, { 128 * 1024 }>;
