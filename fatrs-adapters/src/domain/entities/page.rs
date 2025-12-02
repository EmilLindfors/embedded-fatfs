//! Page entity - the core domain entity representing a buffered page.

use super::PageState;
use crate::domain::value_objects::PageNumber;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// A page of data loaded into a buffer.
///
/// The `Page` entity encapsulates a page number, its data, and its state
/// (clean or dirty). This is the core domain entity that represents a
/// buffered page of storage.
///
/// # Type Parameters
///
/// The page can store data in different ways:
/// - Stack-allocated: `Page<[u8; N]>` for no_std environments
/// - Heap-allocated: `Page<Vec<u8>>` when `alloc` feature is enabled
pub struct Page<T> {
    number: PageNumber,
    data: T,
    state: PageState,
}

// Implementation for heap-allocated pages
#[cfg(feature = "alloc")]
impl Page<Vec<u8>> {
    /// Create a new page with heap-allocated data.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::{Page, PageNumber, PageState};
    ///
    /// let data = vec![0u8; 4096];
    /// let page = Page::<Vec<u8>>::new(PageNumber::new(0), data, PageState::Clean);
    /// ```
    pub fn new(number: PageNumber, data: Vec<u8>, state: PageState) -> Self {
        Self {
            number,
            data,
            state,
        }
    }

    /// Create a new clean page with zeroed data of the specified size.
    pub fn new_zeroed(number: PageNumber, size: usize) -> Self {
        Self {
            number,
            data: alloc::vec![0u8; size],
            state: PageState::Clean,
        }
    }
}

// Implementation for stack-allocated pages
impl<const N: usize> Page<[u8; N]> {
    /// Create a new page with stack-allocated data.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::{Page, PageNumber, PageState};
    ///
    /// let data = [0u8; 4096];
    /// let page = Page::<[u8; 4096]>::new(PageNumber::new(0), data, PageState::Clean);
    /// ```
    pub const fn new(number: PageNumber, data: [u8; N], state: PageState) -> Self {
        Self {
            number,
            data,
            state,
        }
    }

    /// Create a new clean page with zeroed data.
    pub const fn new_zeroed(number: PageNumber) -> Self {
        Self {
            number,
            data: [0u8; N],
            state: PageState::Clean,
        }
    }
}

// Common implementation for all page types
impl<T: AsRef<[u8]> + AsMut<[u8]>> Page<T> {
    /// Get the page number.
    #[inline]
    pub const fn number(&self) -> PageNumber {
        self.number
    }

    /// Get the page state.
    #[inline]
    pub const fn state(&self) -> PageState {
        self.state
    }

    /// Check if the page is dirty (has uncommitted changes).
    #[inline]
    pub const fn is_dirty(&self) -> bool {
        self.state.is_dirty()
    }

    /// Check if the page is clean (synchronized with storage).
    #[inline]
    pub const fn is_clean(&self) -> bool {
        self.state.is_clean()
    }

    /// Get immutable access to the page data.
    #[inline]
    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// Get mutable access to the page data.
    ///
    /// **Important**: This method automatically marks the page as dirty,
    /// as we assume any mutable access will result in modifications.
    /// This is an explicit design decision to prevent accidental data loss.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::{Page, PageNumber, PageState};
    ///
    /// let mut page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![0u8; 4096], PageState::Clean);
    /// assert!(page.is_clean());
    ///
    /// let data = page.data_mut();  // Automatically marks dirty
    /// data[0] = 42;
    ///
    /// assert!(page.is_dirty());
    /// ```
    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.state = PageState::Dirty;
        self.data.as_mut()
    }

    /// Get mutable access to the page data without marking it dirty.
    ///
    /// Use this method when you need to modify the data but don't want
    /// to trigger a dirty state (e.g., when loading data from storage).
    #[inline]
    pub fn data_mut_no_dirty(&mut self) -> &mut [u8] {
        self.data.as_mut()
    }

    /// Mark the page as dirty.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.state = PageState::Dirty;
    }

    /// Mark the page as clean.
    ///
    /// This should only be called after successfully writing the page to storage.
    #[inline]
    pub fn mark_clean(&mut self) {
        self.state = PageState::Clean;
    }

    /// Get the size of the page data in bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.data.as_ref().len()
    }

    /// Copy data from a source slice into this page.
    ///
    /// Copies up to `source.len()` bytes or the page size, whichever is smaller.
    /// Marks the page as dirty after copying.
    ///
    /// Returns the number of bytes copied.
    pub fn copy_from_slice(&mut self, source: &[u8]) -> usize {
        let data = self.data.as_mut();
        let len = source.len().min(data.len());
        data[..len].copy_from_slice(&source[..len]);
        self.mark_dirty();
        len
    }

    /// Copy data from this page to a destination slice.
    ///
    /// Copies up to `dest.len()` bytes or the page size, whichever is smaller.
    ///
    /// Returns the number of bytes copied.
    pub fn copy_to_slice(&self, dest: &mut [u8]) -> usize {
        let data = self.data.as_ref();
        let len = dest.len().min(data.len());
        dest[..len].copy_from_slice(&data[..len]);
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "alloc")]
    fn test_heap_page_creation() {
        let data = vec![1, 2, 3, 4];
        let page = Page::<Vec<u8>>::new(PageNumber::new(5), data, PageState::Clean);

        assert_eq!(page.number().value(), 5);
        assert_eq!(page.data(), &[1, 2, 3, 4]);
        assert!(page.is_clean());
    }

    #[test]
    fn test_stack_page_creation() {
        let data = [1u8, 2, 3, 4];
        let page = Page::<[u8; 4]>::new(PageNumber::new(3), data, PageState::Clean);

        assert_eq!(page.number().value(), 3);
        assert_eq!(page.data(), &[1, 2, 3, 4]);
        assert!(page.is_clean());
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_data_mut_marks_dirty() {
        let mut page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![0u8; 4], PageState::Clean);
        assert!(page.is_clean());

        let data = page.data_mut();
        data[0] = 42;

        assert!(page.is_dirty());
        assert_eq!(page.data()[0], 42);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_data_mut_no_dirty() {
        let mut page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![0u8; 4], PageState::Clean);
        assert!(page.is_clean());

        let data = page.data_mut_no_dirty();
        data[0] = 42;

        assert!(page.is_clean()); // Still clean!
        assert_eq!(page.data()[0], 42);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_mark_dirty_and_clean() {
        let mut page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![0u8; 4], PageState::Clean);

        page.mark_dirty();
        assert!(page.is_dirty());

        page.mark_clean();
        assert!(page.is_clean());
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_copy_from_slice() {
        let mut page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![0u8; 4], PageState::Clean);

        let source = [1, 2, 3, 4];
        let copied = page.copy_from_slice(&source);

        assert_eq!(copied, 4);
        assert_eq!(page.data(), &[1, 2, 3, 4]);
        assert!(page.is_dirty());
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_copy_to_slice() {
        let page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![1, 2, 3, 4], PageState::Clean);

        let mut dest = [0u8; 4];
        let copied = page.copy_to_slice(&mut dest);

        assert_eq!(copied, 4);
        assert_eq!(dest, [1, 2, 3, 4]);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_partial_copy() {
        let mut page = Page::<Vec<u8>>::new(PageNumber::new(0), vec![0u8; 4], PageState::Clean);

        // Copy less data than page size
        let source = [1, 2];
        let copied = page.copy_from_slice(&source);

        assert_eq!(copied, 2);
        assert_eq!(page.data(), &[1, 2, 0, 0]);
    }
}
