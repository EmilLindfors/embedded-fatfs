//! PageBuffer domain service - core business logic for page buffering.
//!
//! This module contains the `PageBuffer` service which implements all the
//! business rules for managing a single-page buffer over block storage.

use crate::domain::{
    entities::{Page, PageState},
    error::DomainError,
    ports::BlockStorage,
    value_objects::{PageConfig, PageConfigError, PageNumber},
};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Domain service for managing a single-page buffer.
///
/// `PageBuffer` is the core domain service that implements all business rules
/// for page buffering:
/// - Enforces dirty page conflicts
/// - Manages page state transitions
/// - Coordinates with storage through the BlockStorage port
///
/// # Type Parameters
///
/// - `S`: The storage implementation (must implement `BlockStorage`)
/// - `T`: The page data storage type (`Vec<u8>` for heap, `[u8; N]` for stack)
/// - `BLOCK_SIZE`: The block size in bytes (must match the BlockStorage implementation)
///
/// # Examples
///
/// ```ignore
/// use fatrs_adapters::domain::{PageBuffer, PageConfig, PageNumber};
///
/// let storage = MyStorage::new();
/// let config = PageConfig::<512>::new(4096, 8);
/// let mut buffer = PageBuffer::new(storage, config);
///
/// // Load a page
/// buffer.load(PageNumber::new(0)).await?;
///
/// // Modify it
/// buffer.modify(|data| data[0] = 42)?;
///
/// // Flush changes
/// buffer.flush().await?;
/// ```
pub struct PageBuffer<S: BlockStorage, T, const BLOCK_SIZE: usize> {
    storage: S,
    config: PageConfig<BLOCK_SIZE>,
    current: Option<Page<T>>,
}

// Implementation for heap-allocated page buffers
#[cfg(feature = "alloc")]
impl<S: BlockStorage, const BLOCK_SIZE: usize> PageBuffer<S, Vec<u8>, BLOCK_SIZE> {
    /// Create a new heap-allocated page buffer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let storage = MyStorage::new();
    /// let config = PageConfig::<512>::new(4096, 8);
    /// let buffer = PageBuffer::new(storage, config);
    /// ```
    pub fn new(storage: S, config: PageConfig<BLOCK_SIZE>) -> Self {
        Self {
            storage,
            config,
            current: None,
        }
    }

    /// Load a page from storage into the buffer.
    ///
    /// # Business Rules
    ///
    /// - If the requested page is already loaded, this is a no-op
    /// - If a different dirty page is loaded, returns `DirtyPageConflict` error
    /// - Otherwise, loads the page from storage
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A dirty page conflict occurs
    /// - The storage read fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let page_ref = buffer.load(PageNumber::new(5)).await?;
    /// assert_eq!(page_ref.number().value(), 5);
    /// ```
    pub async fn load(&mut self, number: PageNumber) -> Result<(), DomainError<S::Error>> {
        // Fast path: page already loaded
        if let Some(ref page) = self.current {
            if page.number() == number {
                return Ok(());
            }

            // Business rule: cannot load different page while current is dirty
            if page.is_dirty() {
                return Err(DomainError::DirtyPageConflict {
                    current: page.number(),
                    requested: number,
                });
            }
        }

        // Load page from storage
        let block_addr = self.config.page_to_block(number);
        let page_size = self.config.page_size();

        let mut data = alloc::vec![0u8; page_size];
        self.storage
            .read_blocks(block_addr, &mut data)
            .await
            .map_err(DomainError::Storage)?;

        let page = Page::<Vec<u8>>::new(number, data, PageState::Clean);
        self.current = Some(page);

        Ok(())
    }

    /// Write the current page to storage.
    ///
    /// After writing, the page is marked as clean.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No page is loaded
    /// - The storage write fails
    pub async fn store(&mut self) -> Result<(), DomainError<S::Error>> {
        let page = self.current.as_mut().ok_or(DomainError::NoPageLoaded)?;

        let block_addr = self.config.page_to_block(page.number());

        self.storage
            .write_blocks(block_addr, page.data())
            .await
            .map_err(DomainError::Storage)?;

        page.mark_clean();
        Ok(())
    }

    /// Flush any uncommitted changes to storage.
    ///
    /// If the current page is dirty, writes it to storage.
    /// Otherwise, this is a no-op.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage write fails.
    pub async fn flush(&mut self) -> Result<(), DomainError<S::Error>> {
        if let Some(page) = &self.current {
            if page.is_dirty() {
                self.store().await?;
            }
        }
        Ok(())
    }

    /// Modify the current page with a closure.
    ///
    /// The closure receives mutable access to the page data.
    /// The page is automatically marked as dirty.
    ///
    /// # Errors
    ///
    /// Returns an error if no page is loaded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// buffer.load(PageNumber::new(0)).await?;
    /// buffer.modify(|data| {
    ///     data[0] = 42;
    ///     data[1] = 43;
    /// })?;
    /// ```
    pub fn modify<F>(&mut self, f: F) -> Result<(), DomainError<S::Error>>
    where
        F: FnOnce(&mut [u8]),
    {
        let page = self.current.as_mut().ok_or(DomainError::NoPageLoaded)?;
        f(page.data_mut());
        Ok(())
    }

    /// Get immutable access to the current page.
    ///
    /// # Errors
    ///
    /// Returns an error if no page is loaded.
    pub fn current(&self) -> Result<&Page<Vec<u8>>, DomainError<S::Error>> {
        self.current.as_ref().ok_or(DomainError::NoPageLoaded)
    }

    /// Get mutable access to the current page.
    ///
    /// # Errors
    ///
    /// Returns an error if no page is loaded.
    pub fn current_mut(&mut self) -> Result<&mut Page<Vec<u8>>, DomainError<S::Error>> {
        self.current.as_mut().ok_or(DomainError::NoPageLoaded)
    }

    /// Get the page configuration.
    pub const fn config(&self) -> &PageConfig<BLOCK_SIZE> {
        &self.config
    }

    /// Clear the buffer, discarding any loaded page.
    ///
    /// **Warning**: Any uncommitted changes are lost!
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// Get the storage size in bytes.
    pub async fn storage_size(&mut self) -> Result<u64, DomainError<S::Error>> {
        self.storage.size().await.map_err(DomainError::Storage)
    }

    /// Get the storage size in pages.
    pub async fn storage_size_in_pages(&mut self) -> Result<u64, DomainError<S::Error>> {
        let bytes = self.storage.size().await.map_err(DomainError::Storage)?;
        Ok(bytes / self.config.page_size() as u64)
    }

    /// Read multiple pages directly from storage without buffering.
    ///
    /// This bypasses the internal page buffer and reads directly into the
    /// destination slice. Useful for large sequential reads.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting page number
    /// * `dest` - Destination buffer (must be at least `num_pages * page_size`)
    /// * `num_pages` - Number of pages to read
    ///
    /// # Errors
    ///
    /// Returns an error if the storage read fails.
    pub async fn read_pages_direct(
        &mut self,
        start: PageNumber,
        dest: &mut [u8],
        num_pages: usize,
    ) -> Result<(), DomainError<S::Error>> {
        let start_block = self.config.page_to_block(start);
        let expected_size = num_pages * self.config.page_size();

        if dest.len() < expected_size {
            // For now, just read what we can
            self.storage.read_blocks(start_block, dest).await
                .map_err(DomainError::Storage)?;
        } else {
            self.storage
                .read_blocks(start_block, &mut dest[..expected_size])
                .await
                .map_err(DomainError::Storage)?;
        }

        Ok(())
    }

    /// Write multiple pages directly to storage without buffering.
    ///
    /// This bypasses the internal page buffer and writes directly from the
    /// source slice. Useful for large sequential writes.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting page number
    /// * `src` - Source buffer
    /// * `num_pages` - Number of pages to write
    ///
    /// # Errors
    ///
    /// Returns an error if the storage write fails.
    pub async fn write_pages_direct(
        &mut self,
        start: PageNumber,
        src: &[u8],
        num_pages: usize,
    ) -> Result<(), DomainError<S::Error>> {
        let start_block = self.config.page_to_block(start);
        let expected_size = num_pages * self.config.page_size();

        if src.len() < expected_size {
            self.storage.write_blocks(start_block, src).await
                .map_err(DomainError::Storage)?;
        } else {
            self.storage
                .write_blocks(start_block, &src[..expected_size])
                .await
                .map_err(DomainError::Storage)?;
        }

        // Invalidate buffer if we wrote to the current page
        if let Some(current) = &self.current {
            let end_page = start.value() + num_pages as u32;
            if current.number().value() >= start.value()
                && current.number().value() < end_page
            {
                self.current = None;
            }
        }

        Ok(())
    }
}

// Implementation for stack-allocated page buffers
impl<S: BlockStorage, const N: usize, const BLOCK_SIZE: usize> PageBuffer<S, [u8; N], BLOCK_SIZE> {
    /// Create a new stack-allocated page buffer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let storage = MyStorage::new();
    /// let config = PageConfig::<512>::new(4096, 8);
    /// let buffer = PageBuffer::<_, 4096, 512>::new_stack(storage, config);
    /// ```
    pub fn new_stack(storage: S, config: PageConfig<BLOCK_SIZE>) -> Self {
        Self {
            storage,
            config,
            current: None,
        }
    }

    /// Load a page from storage into the buffer.
    ///
    /// # Business Rules
    ///
    /// - If the requested page is already loaded, this is a no-op
    /// - If a different dirty page is loaded, returns `DirtyPageConflict` error
    /// - Otherwise, loads the page from storage
    pub async fn load(&mut self, number: PageNumber) -> Result<(), DomainError<S::Error>> {
        // Fast path: page already loaded
        if let Some(ref page) = self.current {
            if page.number() == number {
                return Ok(());
            }

            // Business rule: cannot load different page while current is dirty
            if page.is_dirty() {
                return Err(DomainError::DirtyPageConflict {
                    current: page.number(),
                    requested: number,
                });
            }
        }

        // Load page from storage
        let block_addr = self.config.page_to_block(number);
        let page_size = self.config.page_size();

        if page_size != N {
            return Err(DomainError::InvalidConfig(PageConfigError::InvalidPageSize {
                page_size: N,
                block_size: page_size,
            }));
        }

        let mut data = [0u8; N];
        self.storage
            .read_blocks(block_addr, &mut data[..page_size])
            .await
            .map_err(DomainError::Storage)?;

        let page = Page::<[u8; N]>::new(number, data, PageState::Clean);
        self.current = Some(page);

        Ok(())
    }

    /// Write the current page to storage.
    pub async fn store(&mut self) -> Result<(), DomainError<S::Error>> {
        let page = self.current.as_mut().ok_or(DomainError::NoPageLoaded)?;

        let block_addr = self.config.page_to_block(page.number());

        self.storage
            .write_blocks(block_addr, page.data())
            .await
            .map_err(DomainError::Storage)?;

        page.mark_clean();
        Ok(())
    }

    /// Flush any uncommitted changes to storage.
    pub async fn flush(&mut self) -> Result<(), DomainError<S::Error>> {
        if let Some(page) = &self.current {
            if page.is_dirty() {
                self.store().await?;
            }
        }
        Ok(())
    }

    /// Modify the current page with a closure.
    pub fn modify<F>(&mut self, f: F) -> Result<(), DomainError<S::Error>>
    where
        F: FnOnce(&mut [u8]),
    {
        let page = self.current.as_mut().ok_or(DomainError::NoPageLoaded)?;
        f(page.data_mut());
        Ok(())
    }

    /// Get immutable access to the current page.
    pub fn current(&self) -> Result<&Page<[u8; N]>, DomainError<S::Error>> {
        self.current.as_ref().ok_or(DomainError::NoPageLoaded)
    }

    /// Get mutable access to the current page.
    pub fn current_mut(&mut self) -> Result<&mut Page<[u8; N]>, DomainError<S::Error>> {
        self.current.as_mut().ok_or(DomainError::NoPageLoaded)
    }

    /// Get the page configuration.
    pub const fn config(&self) -> &PageConfig<BLOCK_SIZE> {
        &self.config
    }

    /// Clear the buffer, discarding any loaded page.
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// Get the storage size in bytes.
    pub async fn storage_size(&mut self) -> Result<u64, DomainError<S::Error>> {
        self.storage.size().await.map_err(DomainError::Storage)
    }

    /// Get the storage size in pages.
    pub async fn storage_size_in_pages(&mut self) -> Result<u64, DomainError<S::Error>> {
        let bytes = self.storage.size().await.map_err(DomainError::Storage)?;
        Ok(bytes / self.config.page_size() as u64)
    }

    /// Read multiple pages directly from storage without buffering.
    pub async fn read_pages_direct(
        &mut self,
        start: PageNumber,
        dest: &mut [u8],
        num_pages: usize,
    ) -> Result<(), DomainError<S::Error>> {
        let start_block = self.config.page_to_block(start);
        let expected_size = num_pages * self.config.page_size();

        if dest.len() < expected_size {
            self.storage.read_blocks(start_block, dest).await
                .map_err(DomainError::Storage)?;
        } else {
            self.storage
                .read_blocks(start_block, &mut dest[..expected_size])
                .await
                .map_err(DomainError::Storage)?;
        }

        Ok(())
    }

    /// Write multiple pages directly to storage without buffering.
    pub async fn write_pages_direct(
        &mut self,
        start: PageNumber,
        src: &[u8],
        num_pages: usize,
    ) -> Result<(), DomainError<S::Error>> {
        let start_block = self.config.page_to_block(start);
        let expected_size = num_pages * self.config.page_size();

        if src.len() < expected_size {
            self.storage.write_blocks(start_block, src).await
                .map_err(DomainError::Storage)?;
        } else {
            self.storage
                .write_blocks(start_block, &src[..expected_size])
                .await
                .map_err(DomainError::Storage)?;
        }

        // Invalidate buffer if we wrote to the current page
        if let Some(current) = &self.current {
            let end_page = start.value() + num_pages as u32;
            if current.number().value() >= start.value()
                && current.number().value() < end_page
            {
                self.current = None;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock storage for testing
    struct MockStorage {
        data: HashMap<u32, Vec<u8>>,
        block_size: usize,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
                block_size: 512,
            }
        }
    }

    impl BlockStorage for MockStorage {
        type Error = std::io::Error;

        async fn read_blocks(
            &self,
            start: crate::domain::value_objects::BlockAddress,
            dest: &mut [u8],
        ) -> Result<(), Self::Error> {
            let blocks_needed = (dest.len() + self.block_size - 1) / self.block_size;

            for i in 0..blocks_needed {
                let block_num = start.value() + i as u32;
                let block_data = self
                    .data
                    .get(&block_num)
                    .cloned()
                    .unwrap_or_else(|| vec![0u8; self.block_size]);

                let dest_offset = i * self.block_size;
                let copy_len = (dest.len() - dest_offset).min(self.block_size);
                dest[dest_offset..dest_offset + copy_len]
                    .copy_from_slice(&block_data[..copy_len]);
            }

            Ok(())
        }

        async fn write_blocks(
            &mut self,
            start: crate::domain::value_objects::BlockAddress,
            src: &[u8],
        ) -> Result<(), Self::Error> {
            let blocks_needed = (src.len() + self.block_size - 1) / self.block_size;

            for i in 0..blocks_needed {
                let block_num = start.value() + i as u32;
                let src_offset = i * self.block_size;
                let copy_len = (src.len() - src_offset).min(self.block_size);

                let mut block_data = vec![0u8; self.block_size];
                block_data[..copy_len]
                    .copy_from_slice(&src[src_offset..src_offset + copy_len]);

                self.data.insert(block_num, block_data);
            }

            Ok(())
        }

        async fn size(&self) -> Result<u64, Self::Error> {
            Ok(1024 * 1024) // 1MB
        }
    }

    #[tokio::test]
    async fn test_load_page() {
        let storage = MockStorage::new();
        let config = PageConfig::<512>::new(4096, 8);
        let mut buffer = PageBuffer::new(storage, config);

        buffer.load(PageNumber::new(0)).await.unwrap();
        let page = buffer.current().unwrap();
        assert_eq!(page.number().value(), 0);
        assert!(page.is_clean());
    }

    #[tokio::test]
    async fn test_dirty_page_conflict() {
        let storage = MockStorage::new();
        let config = PageConfig::<512>::new(4096, 8);
        let mut buffer = PageBuffer::new(storage, config);

        // Load page 0 and make it dirty
        buffer.load(PageNumber::new(0)).await.unwrap();
        buffer.modify(|data| data[0] = 42).unwrap();

        // Try to load page 1 - should fail
        let result = buffer.load(PageNumber::new(1)).await;
        assert!(matches!(result, Err(DomainError::DirtyPageConflict { .. })));
    }

    #[tokio::test]
    async fn test_modify_and_flush() {
        let storage = MockStorage::new();
        let config = PageConfig::<512>::new(4096, 8);
        let mut buffer = PageBuffer::new(storage, config);

        // Load, modify, and flush
        buffer.load(PageNumber::new(0)).await.unwrap();
        buffer.modify(|data| data[0] = 42).unwrap();

        assert!(buffer.current().unwrap().is_dirty());

        buffer.flush().await.unwrap();

        assert!(buffer.current().unwrap().is_clean());
    }

    #[tokio::test]
    async fn test_load_same_page_twice() {
        let storage = MockStorage::new();
        let config = PageConfig::<512>::new(4096, 8);
        let mut buffer = PageBuffer::new(storage, config);

        // Load same page twice - should be a no-op
        buffer.load(PageNumber::new(0)).await.unwrap();
        buffer.load(PageNumber::new(0)).await.unwrap();

        let page = buffer.current().unwrap();
        assert_eq!(page.number().value(), 0);
    }

    #[tokio::test]
    async fn test_clear() {
        let storage = MockStorage::new();
        let config = PageConfig::<512>::new(4096, 8);
        let mut buffer = PageBuffer::new(storage, config);

        buffer.load(PageNumber::new(0)).await.unwrap();
        buffer.clear();

        assert!(buffer.current().is_err());
    }
}
