//! Page configuration value object.

use super::{BlockAddress, PageNumber};

/// Configuration for page buffer operations.
///
/// Defines the relationship between pages and blocks, including:
/// - Page size in bytes
/// - Number of blocks per page
/// - Block size (configurable via const generic, typically 512 or 4096 bytes)
///
/// # Type Parameters
///
/// - `BLOCK_SIZE`: The block size in bytes (e.g., 512 for traditional storage, 4096 for modern SSDs/databases)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageConfig<const BLOCK_SIZE: usize> {
    page_size: usize,
    blocks_per_page: usize,
}

/// Standard block size used by most storage devices (512 bytes).
pub const BLOCK_SIZE_512: usize = 512;

/// Modern block size for SSDs and databases (4096 bytes).
pub const BLOCK_SIZE_4096: usize = 4096;

/// Large block size for high-performance SSDs (128KB).
pub const BLOCK_SIZE_128K: usize = 128 * 1024;

/// Very large block size for modern NVMe SSDs (256KB).
pub const BLOCK_SIZE_256K: usize = 256 * 1024;

impl<const BLOCK_SIZE: usize> PageConfig<BLOCK_SIZE> {
    /// Create a new page configuration.
    ///
    /// # Arguments
    ///
    /// * `page_size` - Size of each page in bytes
    /// * `blocks_per_page` - Number of blocks that make up one page
    ///
    /// # Panics
    ///
    /// Panics if `page_size` is not equal to `blocks_per_page * BLOCK_SIZE`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::PageConfig;
    ///
    /// // 4KB pages (8 blocks of 512 bytes)
    /// let config = PageConfig::<512>::new(4096, 8);
    /// assert_eq!(config.page_size(), 4096);
    /// ```
    pub const fn new(page_size: usize, blocks_per_page: usize) -> Self {
        assert!(
            page_size == blocks_per_page * BLOCK_SIZE,
            "page_size must equal blocks_per_page * BLOCK_SIZE"
        );

        Self {
            page_size,
            blocks_per_page,
        }
    }

    /// Create configuration from page size, calculating blocks_per_page.
    ///
    /// # Errors
    ///
    /// Returns an error if page_size is not a multiple of BLOCK_SIZE.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::PageConfig;
    ///
    /// let config = PageConfig::<512>::from_page_size(4096).unwrap();
    /// assert_eq!(config.blocks_per_page(), 8);
    /// ```
    pub const fn from_page_size(page_size: usize) -> Result<Self, PageConfigError> {
        if page_size == 0 {
            return Err(PageConfigError::ZeroPageSize);
        }

        if page_size % BLOCK_SIZE != 0 {
            return Err(PageConfigError::InvalidPageSize {
                page_size,
                block_size: BLOCK_SIZE,
            });
        }

        let blocks_per_page = page_size / BLOCK_SIZE;

        Ok(Self {
            page_size,
            blocks_per_page,
        })
    }

    /// Get the page size in bytes.
    #[inline]
    pub const fn page_size(&self) -> usize {
        self.page_size
    }

    /// Get the number of blocks per page.
    #[inline]
    pub const fn blocks_per_page(&self) -> usize {
        self.blocks_per_page
    }

    /// Get the block size in bytes.
    #[inline]
    pub const fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    /// Convert a page number to its starting block address.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::{PageConfig, PageNumber, BlockAddress};
    ///
    /// let config = PageConfig::<512>::new(4096, 8);
    /// let page = PageNumber::new(2);
    /// let block = config.page_to_block(page);
    /// assert_eq!(block.value(), 16); // 2 pages * 8 blocks/page
    /// ```
    #[inline]
    pub const fn page_to_block(&self, page: PageNumber) -> BlockAddress {
        BlockAddress::new(page.value() * self.blocks_per_page as u32)
    }

    /// Convert a block address to its containing page number.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::{PageConfig, BlockAddress, PageNumber};
    ///
    /// let config = PageConfig::<512>::new(4096, 8);
    /// let block = BlockAddress::new(16);
    /// let page = config.block_to_page(block);
    /// assert_eq!(page.value(), 2);
    /// ```
    #[inline]
    pub const fn block_to_page(&self, block: BlockAddress) -> PageNumber {
        PageNumber::new(block.value() / self.blocks_per_page as u32)
    }

    /// Calculate the offset within a page for a given byte offset.
    ///
    /// Returns (page_number, offset_in_page).
    #[inline]
    pub const fn byte_offset_to_page(&self, offset: u64) -> (PageNumber, usize) {
        let page_num = (offset / self.page_size as u64) as u32;
        let page_offset = (offset % self.page_size as u64) as usize;
        (PageNumber::new(page_num), page_offset)
    }
}

/// Errors that can occur when creating a PageConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageConfigError {
    /// Page size is zero.
    ZeroPageSize,
    /// Page size is not a multiple of block size.
    InvalidPageSize {
        /// The requested page size.
        page_size: usize,
        /// The block size.
        block_size: usize,
    },
}

impl core::fmt::Display for PageConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ZeroPageSize => write!(f, "Page size cannot be zero"),
            Self::InvalidPageSize {
                page_size,
                block_size,
            } => write!(
                f,
                "Page size {} must be a multiple of block size {}",
                page_size, block_size
            ),
        }
    }
}

impl core::error::Error for PageConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_config_creation() {
        let config = PageConfig::<512>::new(4096, 8);
        assert_eq!(config.page_size(), 4096);
        assert_eq!(config.blocks_per_page(), 8);
        assert_eq!(config.block_size(), 512);
    }

    #[test]
    fn test_page_config_from_page_size() {
        let config = PageConfig::<512>::from_page_size(4096).unwrap();
        assert_eq!(config.page_size(), 4096);
        assert_eq!(config.blocks_per_page(), 8);
    }

    #[test]
    fn test_page_config_invalid_page_size() {
        let result = PageConfig::<512>::from_page_size(4000); // Not multiple of 512
        assert!(result.is_err());
    }

    #[test]
    fn test_page_to_block_conversion() {
        let config = PageConfig::<512>::new(4096, 8);
        let page = PageNumber::new(2);
        let block = config.page_to_block(page);
        assert_eq!(block.value(), 16);
    }

    #[test]
    fn test_block_to_page_conversion() {
        let config = PageConfig::<512>::new(4096, 8);
        let block = BlockAddress::new(16);
        let page = config.block_to_page(block);
        assert_eq!(page.value(), 2);
    }

    #[test]
    fn test_byte_offset_to_page() {
        let config = PageConfig::<512>::new(4096, 8);

        let (page, offset) = config.byte_offset_to_page(0);
        assert_eq!(page.value(), 0);
        assert_eq!(offset, 0);

        let (page, offset) = config.byte_offset_to_page(4096);
        assert_eq!(page.value(), 1);
        assert_eq!(offset, 0);

        let (page, offset) = config.byte_offset_to_page(5000);
        assert_eq!(page.value(), 1);
        assert_eq!(offset, 904);
    }

    #[test]
    fn test_page_config_4096_block_size() {
        // Test with 4096 byte block size
        let config = PageConfig::<4096>::new(4096, 1);
        assert_eq!(config.page_size(), 4096);
        assert_eq!(config.blocks_per_page(), 1);
        assert_eq!(config.block_size(), 4096);
    }

    #[test]
    fn test_page_config_large_blocks() {
        // Test with 128KB block size
        let config = PageConfig::<{128 * 1024}>::new(128 * 1024, 1);
        assert_eq!(config.page_size(), 128 * 1024);
        assert_eq!(config.blocks_per_page(), 1);
        assert_eq!(config.block_size(), 128 * 1024);
    }
}
