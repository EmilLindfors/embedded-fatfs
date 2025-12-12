//! Header rotating device wrapper for NOR flash wear-leveling.
//!
//! This module provides a wrapper that distributes writes to logical page 0 (the header)
//! across multiple physical pages to extend flash lifetime through wear leveling.
//!
//! # Problem
//!
//! NOR flash has limited erase cycles (typically 100K per sector). If a database commits
//! frequently and always writes to the same header page, the flash will wear out prematurely:
//!
//! - 100K commits = flash worn out (without rotation)
//! - 400K commits with 4-page rotation
//! - 800K commits with 8-page rotation
//!
//! # Architecture
//!
//! ```text
//! Logical View:           Physical Layout:
//! ┌─────────────┐         ┌─────────────┐ ← Physical 0 (header slot 0)
//! │ Page 0      │ ───────►│ Rotating    │ ← Physical 1 (header slot 1)
//! │ (header)    │         │ Header      │ ← Physical 2 (header slot 2)
//! ├─────────────┤         │ Slots       │ ← Physical 3 (header slot 3)
//! │ Page 1      │ ───────►├─────────────┤
//! │ (data)      │         │ Physical 4  │ ← Data starts here
//! ├─────────────┤         ├─────────────┤
//! │ Page 2      │ ───────►│ Physical 5  │
//! │ (data)      │         └─────────────┘
//! └─────────────┘
//! ```
//!
//! # Consumer Responsibilities
//!
//! The consumer is responsible for:
//! 1. Scanning header slots on startup to find the current valid header
//! 2. Calling `set_current_slot()` to initialize the adapter
//! 3. Managing sequence numbers within the header data
//!
//! # Example
//!
//! ```ignore
//! use fatrs_adapters::{NorFlashAdapter, NorFlashConfig, HeaderRotatingDevice, HeaderRotationConfig};
//!
//! // 1. Create underlying device
//! let flash = MyFlash::new();
//! let adapter = NorFlashAdapter::new(flash, NorFlashConfig::default_4mb());
//!
//! // 2. Wrap with rotation
//! let config = HeaderRotationConfig::new(4);
//! let mut device = HeaderRotatingDevice::new(adapter, config);
//!
//! // 3. Consumer scans header slots to find current valid header
//! let mut best_slot = 0u8;
//! let mut best_seq = 0u64;
//!
//! for slot in 0..device.rotation_pages() {
//!     let mut buf = Aligned([0u8; 4096]);
//!     device.read_header_slot(slot, core::slice::from_mut(&mut buf)).await?;
//!
//!     // Consumer-defined header format - parse sequence number
//!     let seq = u64::from_le_bytes(buf[0..8].try_into().unwrap());
//!     if is_valid_header(&buf) && seq > best_seq {
//!         best_seq = seq;
//!         best_slot = slot;
//!     }
//! }
//!
//! // 4. Initialize adapter with current slot
//! device.set_current_slot(best_slot);
//!
//! // 5. Use normally - header writes automatically rotate
//! // Logical page count = 64 - 3 = 61 pages available for data
//! ```

use core::cell::UnsafeCell;
use aligned::Aligned;
use fatrs_block_device::BlockDevice;

/// Block size for header rotation (must match wrapped device).
pub const HEADER_ROTATION_BLOCK_SIZE: usize = 4096;

/// Configuration for header rotation.
///
/// Header rotation distributes writes to logical page 0 (the header)
/// across multiple physical pages to extend flash lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderRotationConfig {
    /// Number of physical pages reserved for header rotation (1-8).
    rotation_pages: u8,
}

impl HeaderRotationConfig {
    /// Create a new header rotation configuration.
    ///
    /// # Arguments
    /// * `rotation_pages` - Number of pages for rotation (1-8)
    ///
    /// # Panics
    /// Panics if `rotation_pages` is 0 or greater than 8.
    pub const fn new(rotation_pages: u8) -> Self {
        assert!(
            rotation_pages >= 1 && rotation_pages <= 8,
            "rotation_pages must be between 1 and 8"
        );
        Self { rotation_pages }
    }

    /// Default configuration with 4-page rotation.
    ///
    /// This provides 4x the flash lifetime for header writes.
    pub const fn default_4_pages() -> Self {
        Self { rotation_pages: 4 }
    }

    /// Pass-through mode (no rotation).
    ///
    /// Useful when you want the wrapper for API consistency
    /// but don't need actual rotation.
    pub const fn no_rotation() -> Self {
        Self { rotation_pages: 1 }
    }

    /// Get the number of rotation pages.
    #[inline]
    pub const fn rotation_pages(&self) -> u8 {
        self.rotation_pages
    }
}

impl Default for HeaderRotationConfig {
    fn default() -> Self {
        Self::default_4_pages()
    }
}

/// A wrapper that provides header rotation for any BlockDevice<4096>.
///
/// This wrapper distributes writes to logical page 0 (the header page)
/// across multiple physical pages to extend flash lifetime through
/// wear leveling.
///
/// # Safety
///
/// This type uses `UnsafeCell` for interior mutability because
/// the `BlockDevice::read` trait method takes `&self`, but we need
/// to call `&mut self` methods on the inner device. This is safe in
/// single-threaded embedded contexts.
pub struct HeaderRotatingDevice<D> {
    inner: UnsafeCell<D>,
    config: HeaderRotationConfig,
    /// Current slot for header reads (0..rotation_pages).
    /// Writes advance to next slot before writing.
    current_slot: u8,
}

// SAFETY: HeaderRotatingDevice is Send if D is Send
// The UnsafeCell is only used for interior mutability in single-threaded contexts
unsafe impl<D: Send> Send for HeaderRotatingDevice<D> {}

// SAFETY: HeaderRotatingDevice is Sync if D is Sync
// Access must be externally synchronized in multi-threaded contexts
unsafe impl<D: Sync> Sync for HeaderRotatingDevice<D> {}

impl<D> HeaderRotatingDevice<D> {
    /// Create a new header rotating device wrapper.
    ///
    /// # Arguments
    /// * `inner` - The underlying block device to wrap
    /// * `config` - Header rotation configuration
    ///
    /// # Note
    /// After construction, call `set_current_slot()` after scanning
    /// header slots to find the current valid header.
    pub fn new(inner: D, config: HeaderRotationConfig) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
            config,
            current_slot: 0,
        }
    }

    /// Get the rotation configuration.
    pub fn config(&self) -> &HeaderRotationConfig {
        &self.config
    }

    /// Get the current header slot.
    ///
    /// This is the slot that will be READ for header operations.
    /// Writes advance to `(current_slot + 1) % rotation_pages` before writing.
    pub fn current_slot(&self) -> u8 {
        self.current_slot
    }

    /// Set the current header slot after initialization scan.
    ///
    /// The consumer should:
    /// 1. Read all header slots using `read_header_slot()`
    /// 2. Parse sequence numbers from header data
    /// 3. Call this with the slot containing the highest valid sequence
    ///
    /// # Panics
    /// Panics if `slot >= rotation_pages`.
    pub fn set_current_slot(&mut self, slot: u8) {
        assert!(
            slot < self.config.rotation_pages,
            "slot {} must be < rotation_pages {}",
            slot,
            self.config.rotation_pages
        );
        self.current_slot = slot;
    }

    /// Get the number of rotation pages.
    #[inline]
    pub fn rotation_pages(&self) -> u8 {
        self.config.rotation_pages
    }

    /// Consume the wrapper and return the inner device.
    pub fn into_inner(self) -> D {
        self.inner.into_inner()
    }

    /// Get a reference to the inner device.
    pub fn inner(&self) -> &D {
        // SAFETY: Only returns immutable reference
        unsafe { &*self.inner.get() }
    }

    /// Get mutable reference to inner device (internal use).
    #[inline]
    fn inner_mut(&self) -> &mut D {
        // SAFETY: Safe in single-threaded contexts
        unsafe { &mut *self.inner.get() }
    }

    /// Map a logical page number to a physical page number.
    ///
    /// - Logical page 0 → current_slot (0..rotation_pages)
    /// - Logical page N (N > 0) → physical page (N + rotation_pages - 1)
    #[inline]
    fn logical_to_physical(&self, logical_page: u32) -> u32 {
        if logical_page == 0 {
            // Header page maps to current rotation slot
            self.current_slot as u32
        } else {
            // Data pages are offset by (rotation_pages - 1)
            // because logical page 1 starts at physical page rotation_pages
            logical_page + (self.config.rotation_pages as u32 - 1)
        }
    }

    /// Get the next slot in the rotation sequence.
    #[inline]
    fn next_slot(&self) -> u8 {
        (self.current_slot + 1) % self.config.rotation_pages
    }
}

impl<D: BlockDevice<HEADER_ROTATION_BLOCK_SIZE>> HeaderRotatingDevice<D> {
    /// Read a specific header slot directly (for initialization scanning).
    ///
    /// This reads the raw physical page at the given slot index,
    /// allowing the consumer to parse sequence numbers and determine
    /// which slot contains the current valid header.
    ///
    /// # Arguments
    /// * `slot` - Slot index (0..rotation_pages)
    /// * `data` - Buffer to read into
    ///
    /// # Panics
    /// Panics if `slot >= rotation_pages`.
    pub async fn read_header_slot(
        &self,
        slot: u8,
        data: &mut [Aligned<D::Align, [u8; HEADER_ROTATION_BLOCK_SIZE]>],
    ) -> Result<(), D::Error> {
        assert!(
            slot < self.config.rotation_pages,
            "slot {} must be < rotation_pages {}",
            slot,
            self.config.rotation_pages
        );

        // Read directly from physical slot
        self.inner_mut().read(slot as u32, data).await
    }
}

impl<D> BlockDevice<HEADER_ROTATION_BLOCK_SIZE> for HeaderRotatingDevice<D>
where
    D: BlockDevice<HEADER_ROTATION_BLOCK_SIZE>,
{
    type Error = D::Error;
    type Align = D::Align;

    async fn read(
        &self,
        block_address: u32,
        data: &mut [Aligned<Self::Align, [u8; HEADER_ROTATION_BLOCK_SIZE]>],
    ) -> Result<(), Self::Error> {
        // Translate logical to physical addresses
        for (i, block) in data.iter_mut().enumerate() {
            let logical_page = block_address + i as u32;
            let physical_page = self.logical_to_physical(logical_page);

            self.inner_mut()
                .read(physical_page, core::slice::from_mut(block))
                .await?;
        }
        Ok(())
    }

    async fn write(
        &mut self,
        block_address: u32,
        data: &[Aligned<Self::Align, [u8; HEADER_ROTATION_BLOCK_SIZE]>],
    ) -> Result<(), Self::Error> {
        for (i, block) in data.iter().enumerate() {
            let logical_page = block_address + i as u32;

            let physical_page = if logical_page == 0 {
                // Header write: advance to next slot first
                let next = self.next_slot();
                self.current_slot = next;
                next as u32
            } else {
                self.logical_to_physical(logical_page)
            };

            self.inner_mut()
                .write(physical_page, core::slice::from_ref(block))
                .await?;
        }
        Ok(())
    }

    async fn size(&self) -> Result<u64, Self::Error> {
        // Report logical size: physical_size - (rotation_pages - 1) * block_size
        let physical_size = self.inner_mut().size().await?;
        let reserved_bytes =
            (self.config.rotation_pages as u64 - 1) * HEADER_ROTATION_BLOCK_SIZE as u64;
        Ok(physical_size.saturating_sub(reserved_bytes))
    }

    async fn sync(&mut self) -> Result<(), Self::Error> {
        self.inner_mut().sync().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock block device for testing
    struct MockDevice {
        pages: [[u8; HEADER_ROTATION_BLOCK_SIZE]; 16],
    }

    impl MockDevice {
        fn new() -> Self {
            Self {
                pages: [[0xFF; HEADER_ROTATION_BLOCK_SIZE]; 16],
            }
        }
    }

    impl BlockDevice<HEADER_ROTATION_BLOCK_SIZE> for MockDevice {
        type Error = core::convert::Infallible;
        type Align = aligned::A4;

        async fn read(
            &self,
            block_address: u32,
            data: &mut [Aligned<Self::Align, [u8; HEADER_ROTATION_BLOCK_SIZE]>],
        ) -> Result<(), Self::Error> {
            for (i, block) in data.iter_mut().enumerate() {
                let page = (block_address + i as u32) as usize;
                if page < self.pages.len() {
                    block.copy_from_slice(&self.pages[page]);
                }
            }
            Ok(())
        }

        async fn write(
            &mut self,
            block_address: u32,
            data: &[Aligned<Self::Align, [u8; HEADER_ROTATION_BLOCK_SIZE]>],
        ) -> Result<(), Self::Error> {
            for (i, block) in data.iter().enumerate() {
                let page = (block_address + i as u32) as usize;
                if page < self.pages.len() {
                    self.pages[page].copy_from_slice(&block[..]);
                }
            }
            Ok(())
        }

        async fn size(&self) -> Result<u64, Self::Error> {
            Ok((self.pages.len() * HEADER_ROTATION_BLOCK_SIZE) as u64)
        }

        async fn sync(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn block_on<F: core::future::Future>(f: F) -> F::Output {
        use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        fn raw_waker() -> RawWaker {
            fn no_op(_: *const ()) {}
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
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
    fn test_logical_to_physical_mapping() {
        let device = MockDevice::new();
        let config = HeaderRotationConfig::new(4);
        let rotating = HeaderRotatingDevice::new(device, config);

        // Header (page 0) maps to current slot (initially 0)
        assert_eq!(rotating.logical_to_physical(0), 0);

        // Data pages offset by rotation_pages - 1
        assert_eq!(rotating.logical_to_physical(1), 4); // 1 + 3 = 4
        assert_eq!(rotating.logical_to_physical(2), 5); // 2 + 3 = 5
        assert_eq!(rotating.logical_to_physical(10), 13); // 10 + 3 = 13
    }

    #[test]
    fn test_header_rotation_on_write() {
        block_on(async {
            let device = MockDevice::new();
            let config = HeaderRotationConfig::new(4);
            let mut rotating = HeaderRotatingDevice::new(device, config);

            // Initial slot is 0, so first write goes to slot 1
            let write_buf: Aligned<aligned::A4, [u8; HEADER_ROTATION_BLOCK_SIZE]> =
                Aligned([42u8; HEADER_ROTATION_BLOCK_SIZE]);

            rotating
                .write(0, core::slice::from_ref(&write_buf))
                .await
                .unwrap();
            assert_eq!(rotating.current_slot(), 1);

            rotating
                .write(0, core::slice::from_ref(&write_buf))
                .await
                .unwrap();
            assert_eq!(rotating.current_slot(), 2);

            rotating
                .write(0, core::slice::from_ref(&write_buf))
                .await
                .unwrap();
            assert_eq!(rotating.current_slot(), 3);

            // Wraps around
            rotating
                .write(0, core::slice::from_ref(&write_buf))
                .await
                .unwrap();
            assert_eq!(rotating.current_slot(), 0);
        });
    }

    #[test]
    fn test_size_reports_logical_size() {
        block_on(async {
            let device = MockDevice::new();
            let config = HeaderRotationConfig::new(4);
            let rotating = HeaderRotatingDevice::new(device, config);

            let physical_size = 16 * HEADER_ROTATION_BLOCK_SIZE as u64;
            let expected_logical = physical_size - (3 * HEADER_ROTATION_BLOCK_SIZE as u64);

            let size = rotating.size().await.unwrap();
            assert_eq!(size, expected_logical);
        });
    }

    #[test]
    fn test_set_current_slot() {
        let device = MockDevice::new();
        let config = HeaderRotationConfig::new(4);
        let mut rotating = HeaderRotatingDevice::new(device, config);

        rotating.set_current_slot(2);
        assert_eq!(rotating.current_slot(), 2);
        assert_eq!(rotating.logical_to_physical(0), 2);
    }

    #[test]
    #[should_panic(expected = "slot 5 must be < rotation_pages 4")]
    fn test_set_current_slot_invalid() {
        let device = MockDevice::new();
        let config = HeaderRotationConfig::new(4);
        let mut rotating = HeaderRotatingDevice::new(device, config);

        rotating.set_current_slot(5); // Panics
    }

    #[test]
    fn test_data_page_write_no_rotation() {
        block_on(async {
            let device = MockDevice::new();
            let config = HeaderRotationConfig::new(4);
            let mut rotating = HeaderRotatingDevice::new(device, config);

            let initial_slot = rotating.current_slot();

            // Write to data page (logical page 1)
            let write_buf: Aligned<aligned::A4, [u8; HEADER_ROTATION_BLOCK_SIZE]> =
                Aligned([42u8; HEADER_ROTATION_BLOCK_SIZE]);

            rotating
                .write(1, core::slice::from_ref(&write_buf))
                .await
                .unwrap();

            // Slot should not change for data page writes
            assert_eq!(rotating.current_slot(), initial_slot);
        });
    }

    #[test]
    fn test_read_header_slot() {
        block_on(async {
            let mut device = MockDevice::new();
            // Write marker to physical slot 2
            device.pages[2][0] = 0xAB;
            device.pages[2][1] = 0xCD;

            let config = HeaderRotationConfig::new(4);
            let rotating = HeaderRotatingDevice::new(device, config);

            let mut read_buf: Aligned<aligned::A4, [u8; HEADER_ROTATION_BLOCK_SIZE]> =
                Aligned([0u8; HEADER_ROTATION_BLOCK_SIZE]);

            rotating
                .read_header_slot(2, core::slice::from_mut(&mut read_buf))
                .await
                .unwrap();

            assert_eq!(read_buf[0], 0xAB);
            assert_eq!(read_buf[1], 0xCD);
        });
    }

    #[test]
    fn test_config_presets() {
        let default = HeaderRotationConfig::default();
        assert_eq!(default.rotation_pages(), 4);

        let no_rot = HeaderRotationConfig::no_rotation();
        assert_eq!(no_rot.rotation_pages(), 1);

        let custom = HeaderRotationConfig::new(8);
        assert_eq!(custom.rotation_pages(), 8);
    }

    #[test]
    #[should_panic(expected = "rotation_pages must be between 1 and 8")]
    fn test_config_invalid_zero() {
        let _ = HeaderRotationConfig::new(0);
    }

    #[test]
    #[should_panic(expected = "rotation_pages must be between 1 and 8")]
    fn test_config_invalid_too_large() {
        let _ = HeaderRotationConfig::new(9);
    }

    #[test]
    fn test_read_through_wrapper() {
        block_on(async {
            let mut device = MockDevice::new();
            // Write data to physical pages
            device.pages[0][0] = 0x11; // Header slot 0
            device.pages[4][0] = 0x22; // Data page (logical 1)
            device.pages[5][0] = 0x33; // Data page (logical 2)

            let config = HeaderRotationConfig::new(4);
            let rotating = HeaderRotatingDevice::new(device, config);

            let mut buf: Aligned<aligned::A4, [u8; HEADER_ROTATION_BLOCK_SIZE]> =
                Aligned([0u8; HEADER_ROTATION_BLOCK_SIZE]);

            // Read logical page 0 (header) - maps to slot 0
            rotating
                .read(0, core::slice::from_mut(&mut buf))
                .await
                .unwrap();
            assert_eq!(buf[0], 0x11);

            // Read logical page 1 (data) - maps to physical 4
            rotating
                .read(1, core::slice::from_mut(&mut buf))
                .await
                .unwrap();
            assert_eq!(buf[0], 0x22);

            // Read logical page 2 (data) - maps to physical 5
            rotating
                .read(2, core::slice::from_mut(&mut buf))
                .await
                .unwrap();
            assert_eq!(buf[0], 0x33);
        });
    }

    #[test]
    fn test_write_data_to_correct_physical_page() {
        block_on(async {
            let device = MockDevice::new();
            let config = HeaderRotationConfig::new(4);
            let mut rotating = HeaderRotatingDevice::new(device, config);

            let write_buf: Aligned<aligned::A4, [u8; HEADER_ROTATION_BLOCK_SIZE]> =
                Aligned([0x42u8; HEADER_ROTATION_BLOCK_SIZE]);

            // Write to logical page 1 (should go to physical page 4)
            rotating
                .write(1, core::slice::from_ref(&write_buf))
                .await
                .unwrap();

            // Verify by reading back
            let mut read_buf: Aligned<aligned::A4, [u8; HEADER_ROTATION_BLOCK_SIZE]> =
                Aligned([0u8; HEADER_ROTATION_BLOCK_SIZE]);

            rotating
                .read(1, core::slice::from_mut(&mut read_buf))
                .await
                .unwrap();

            assert_eq!(read_buf[0], 0x42);
        });
    }
}
