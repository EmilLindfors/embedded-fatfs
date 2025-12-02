//! Type-safe page number value object.

use core::fmt;

/// A validated page number.
///
/// This value object ensures that page numbers are always valid and provides
/// type safety to prevent mixing page numbers with other integer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageNumber(u32);

impl PageNumber {
    /// Create a new page number.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::PageNumber;
    ///
    /// let page = PageNumber::new(0);
    /// assert_eq!(page.value(), 0);
    /// ```
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the underlying u32 value.
    #[inline]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Get the next page number.
    #[inline]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Get the previous page number, or None if this is page 0.
    #[inline]
    pub const fn prev(self) -> Option<Self> {
        if self.0 == 0 {
            None
        } else {
            Some(Self(self.0 - 1))
        }
    }
}

impl fmt::Display for PageNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Page({})", self.0)
    }
}

impl From<u32> for PageNumber {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<PageNumber> for u32 {
    fn from(page: PageNumber) -> Self {
        page.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_number_creation() {
        let page = PageNumber::new(42);
        assert_eq!(page.value(), 42);
    }

    #[test]
    fn test_page_number_next() {
        let page = PageNumber::new(0);
        assert_eq!(page.next().value(), 1);

        let max_page = PageNumber::new(u32::MAX);
        assert_eq!(max_page.next().value(), u32::MAX); // saturating
    }

    #[test]
    fn test_page_number_prev() {
        let page = PageNumber::new(1);
        assert_eq!(page.prev(), Some(PageNumber::new(0)));

        let first_page = PageNumber::new(0);
        assert_eq!(first_page.prev(), None);
    }

    #[test]
    fn test_page_number_display() {
        let page = PageNumber::new(123);
        assert_eq!(format!("{}", page), "Page(123)");
    }
}
