//! NOR Flash adapter for embedded-storage traits
//!
//! This module provides an adapter that wraps types implementing
//! `embedded-storage` NOR flash traits and exposes them as a `BlockDevice`.
//!
//! # Example
//!
//! ```ignore
//! use esp_storage::FlashStorage as EspFlash;
//! use fatrs_adapters::adapters::NorFlashAdapter;
//!
//! let esp_flash = EspFlash::new();
//! let config = NorFlashConfig::new(0x3C_0000, 64); // 256KB at offset
//! let device = NorFlashAdapter::new(esp_flash, config);
//!
//! // Now use with any fatrs-based storage
//! let storage = BlockDeviceStorage::new(device, config.page_count);
//! ```

use core::cell::UnsafeCell;
use aligned::Aligned;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use fatrs_block_device::BlockDevice;

/// Block size for NOR flash adapter (4KB pages)
pub const NOR_FLASH_BLOCK_SIZE: usize = 4096;

/// Configuration for NOR flash storage region
///
/// Defines where in flash the storage is located and how many pages to use.
#[derive(Debug, Clone, Copy)]
pub struct NorFlashConfig {
    /// Start offset in flash (must be 4KB sector-aligned)
    pub start_offset: u32,
    /// Number of 4KB pages to use
    pub page_count: u32,
}

impl NorFlashConfig {
    /// Create a new flash configuration
    ///
    /// # Arguments
    /// * `start_offset` - Byte offset in flash (must be 4KB aligned)
    /// * `page_count` - Number of 4KB pages
    ///
    /// # Panics
    /// Panics if `start_offset` is not 4KB aligned
    pub fn new(start_offset: u32, page_count: u32) -> Self {
        assert!(
            start_offset % NOR_FLASH_BLOCK_SIZE as u32 == 0,
            "start_offset must be 4KB aligned"
        );
        Self {
            start_offset,
            page_count,
        }
    }

    /// Default config using last 256KB of a 4MB flash
    ///
    /// Places the storage at offset 0x3C0000 (3.75MB) with 64 pages (256KB).
    pub fn default_4mb() -> Self {
        Self::new(0x3C_0000, 64)
    }

    /// Default config using last 1MB of a 16MB flash
    ///
    /// Places the storage at offset 0xF00000 (15MB) with 256 pages (1MB).
    pub fn default_16mb() -> Self {
        Self::new(0xF0_0000, 256)
    }

    /// Get the total flash size in bytes
    #[inline]
    pub fn total_size(&self) -> u32 {
        self.page_count * NOR_FLASH_BLOCK_SIZE as u32
    }
}

impl Default for NorFlashConfig {
    fn default() -> Self {
        Self::default_4mb()
    }
}

/// Error type for NOR flash operations
#[derive(Debug, Clone, Copy)]
pub struct NorFlashError;

impl core::fmt::Display for NorFlashError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NOR flash error")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NorFlashError {}

/// Adapter that wraps embedded-storage NOR flash as a BlockDevice
///
/// This adapter allows using ESP32 internal flash, external SPI flash,
/// or any other `embedded-storage` compatible flash with the fatrs ecosystem.
///
/// # Safety
///
/// This type uses `UnsafeCell` for interior mutability because
/// `embedded-storage` traits require `&mut self` for reads, but
/// `BlockDevice::read` takes `&self`. This is safe in single-threaded
/// embedded contexts.
///
/// # Example
///
/// ```ignore
/// use esp_storage::FlashStorage as EspFlash;
/// use fatrs_adapters::adapters::{NorFlashAdapter, NorFlashConfig};
///
/// let flash = EspFlash::new();
/// let config = NorFlashConfig::default_4mb();
/// let adapter = NorFlashAdapter::new(flash, config);
/// ```
pub struct NorFlashAdapter<F> {
    flash: UnsafeCell<F>,
    config: NorFlashConfig,
}

// SAFETY: NorFlashAdapter is Send if F is Send
// The UnsafeCell is only used for interior mutability in single-threaded contexts
unsafe impl<F: Send> Send for NorFlashAdapter<F> {}

// SAFETY: NorFlashAdapter is Sync if F is Sync
// Access must be externally synchronized in multi-threaded contexts
unsafe impl<F: Sync> Sync for NorFlashAdapter<F> {}

impl<F> NorFlashAdapter<F> {
    /// Create a new NOR flash adapter
    ///
    /// # Arguments
    /// * `flash` - The underlying flash implementation
    /// * `config` - Configuration for the flash region
    pub fn new(flash: F, config: NorFlashConfig) -> Self {
        Self {
            flash: UnsafeCell::new(flash),
            config,
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &NorFlashConfig {
        &self.config
    }

    /// Consume the adapter and return the underlying flash
    pub fn into_inner(self) -> F {
        self.flash.into_inner()
    }

    /// Get mutable access to the flash (internal use)
    #[inline]
    fn flash_mut(&self) -> &mut F {
        // SAFETY: Safe in single-threaded embedded contexts
        unsafe { &mut *self.flash.get() }
    }

    /// Convert block address to flash offset
    #[inline]
    fn block_to_offset(&self, block: u32) -> u32 {
        self.config.start_offset + block * NOR_FLASH_BLOCK_SIZE as u32
    }
}

impl<F> BlockDevice<NOR_FLASH_BLOCK_SIZE> for NorFlashAdapter<F>
where
    F: NorFlash + ReadNorFlash,
{
    type Error = NorFlashError;
    type Align = aligned::A4; // 4-byte alignment typical for flash

    async fn read(
        &self,
        block_address: u32,
        data: &mut [Aligned<Self::Align, [u8; NOR_FLASH_BLOCK_SIZE]>],
    ) -> Result<(), Self::Error> {
        for (i, block) in data.iter_mut().enumerate() {
            let offset = self.block_to_offset(block_address + i as u32);
            self.flash_mut()
                .read(offset, &mut block[..])
                .map_err(|_| NorFlashError)?;
        }
        Ok(())
    }

    async fn write(
        &mut self,
        block_address: u32,
        data: &[Aligned<Self::Align, [u8; NOR_FLASH_BLOCK_SIZE]>],
    ) -> Result<(), Self::Error> {
        for (i, block) in data.iter().enumerate() {
            let offset = self.block_to_offset(block_address + i as u32);

            // Erase before write (required for NOR flash)
            self.flash_mut()
                .erase(offset, offset + NOR_FLASH_BLOCK_SIZE as u32)
                .map_err(|_| NorFlashError)?;

            // Write the data
            self.flash_mut()
                .write(offset, &block[..])
                .map_err(|_| NorFlashError)?;
        }
        Ok(())
    }

    async fn size(&self) -> Result<u64, Self::Error> {
        Ok(self.config.total_size() as u64)
    }

    async fn sync(&mut self) -> Result<(), Self::Error> {
        // NOR flash writes are typically synchronous
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock NOR flash for testing
    struct MockFlash {
        data: [[u8; NOR_FLASH_BLOCK_SIZE]; 16],
    }

    impl MockFlash {
        fn new() -> Self {
            Self {
                data: [[0xFF; NOR_FLASH_BLOCK_SIZE]; 16],
            }
        }
    }

    impl embedded_storage::nor_flash::ErrorType for MockFlash {
        type Error = MockFlashError;
    }

    #[derive(Debug)]
    struct MockFlashError;

    impl embedded_storage::nor_flash::NorFlashError for MockFlashError {
        fn kind(&self) -> embedded_storage::nor_flash::NorFlashErrorKind {
            embedded_storage::nor_flash::NorFlashErrorKind::Other
        }
    }

    impl ReadNorFlash for MockFlash {
        const READ_SIZE: usize = 1;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            let page = (offset / NOR_FLASH_BLOCK_SIZE as u32) as usize;
            let page_offset = (offset % NOR_FLASH_BLOCK_SIZE as u32) as usize;
            if page < self.data.len() && page_offset + bytes.len() <= NOR_FLASH_BLOCK_SIZE {
                bytes.copy_from_slice(&self.data[page][page_offset..page_offset + bytes.len()]);
                Ok(())
            } else {
                Err(MockFlashError)
            }
        }

        fn capacity(&self) -> usize {
            self.data.len() * NOR_FLASH_BLOCK_SIZE
        }
    }

    impl NorFlash for MockFlash {
        const WRITE_SIZE: usize = 1;
        const ERASE_SIZE: usize = NOR_FLASH_BLOCK_SIZE;

        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            let start_page = (from / NOR_FLASH_BLOCK_SIZE as u32) as usize;
            let end_page = ((to + NOR_FLASH_BLOCK_SIZE as u32 - 1) / NOR_FLASH_BLOCK_SIZE as u32) as usize;
            for page in start_page..end_page.min(self.data.len()) {
                self.data[page] = [0xFF; NOR_FLASH_BLOCK_SIZE];
            }
            Ok(())
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            let page = (offset / NOR_FLASH_BLOCK_SIZE as u32) as usize;
            let page_offset = (offset % NOR_FLASH_BLOCK_SIZE as u32) as usize;
            if page < self.data.len() && page_offset + bytes.len() <= NOR_FLASH_BLOCK_SIZE {
                self.data[page][page_offset..page_offset + bytes.len()].copy_from_slice(bytes);
                Ok(())
            } else {
                Err(MockFlashError)
            }
        }
    }

    fn block_on<F: core::future::Future>(f: F) -> F::Output {
        use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        fn raw_waker() -> RawWaker {
            fn no_op(_: *const ()) {}
            fn clone(_: *const ()) -> RawWaker { raw_waker() }
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
            RawWaker::new(core::ptr::null(), &VTABLE)
        }

        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut cx = Context::from_waker(&waker);
        let mut f = core::pin::pin!(f);

        loop {
            match f.as_mut().poll(&mut cx) {
                Poll::Ready(val) => return val,
                Poll::Pending => {}
            }
        }
    }

    #[test]
    fn test_nor_flash_adapter_read_write() {
        block_on(async {
            let flash = MockFlash::new();
            let config = NorFlashConfig::new(0, 16);
            let mut adapter = NorFlashAdapter::new(flash, config);

            // Write data
            let write_buf: Aligned<aligned::A4, [u8; NOR_FLASH_BLOCK_SIZE]> =
                Aligned([42u8; NOR_FLASH_BLOCK_SIZE]);
            adapter.write(1, core::slice::from_ref(&write_buf)).await.unwrap();

            // Read it back
            let mut read_buf: Aligned<aligned::A4, [u8; NOR_FLASH_BLOCK_SIZE]> =
                Aligned([0u8; NOR_FLASH_BLOCK_SIZE]);
            adapter.read(1, core::slice::from_mut(&mut read_buf)).await.unwrap();

            assert_eq!(read_buf[0], 42);
            assert_eq!(read_buf[NOR_FLASH_BLOCK_SIZE - 1], 42);
        });
    }

    #[test]
    fn test_nor_flash_adapter_size() {
        block_on(async {
            let flash = MockFlash::new();
            let config = NorFlashConfig::new(0, 64);
            let adapter = NorFlashAdapter::new(flash, config);

            let size = adapter.size().await.unwrap();
            assert_eq!(size, 64 * NOR_FLASH_BLOCK_SIZE as u64);
        });
    }

    #[test]
    fn test_config_presets() {
        let config_4mb = NorFlashConfig::default_4mb();
        assert_eq!(config_4mb.start_offset, 0x3C_0000);
        assert_eq!(config_4mb.page_count, 64);

        let config_16mb = NorFlashConfig::default_16mb();
        assert_eq!(config_16mb.start_offset, 0xF0_0000);
        assert_eq!(config_16mb.page_count, 256);
    }

    #[test]
    #[should_panic(expected = "4KB aligned")]
    fn test_config_unaligned() {
        let _ = NorFlashConfig::new(0x100, 64);
    }
}
