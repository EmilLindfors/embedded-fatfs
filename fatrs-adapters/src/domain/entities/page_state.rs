//! Page state for tracking buffer modifications.

/// The state of a page in the buffer.
///
/// Pages transition through these states:
/// - Clean: Page data matches storage
/// - Dirty: Page has been modified and needs to be written back
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
    /// Page data is synchronized with storage.
    Clean,
    /// Page data has been modified and needs to be written back.
    Dirty,
}

impl PageState {
    /// Check if the page is dirty.
    #[inline]
    pub const fn is_dirty(&self) -> bool {
        matches!(self, PageState::Dirty)
    }

    /// Check if the page is clean.
    #[inline]
    pub const fn is_clean(&self) -> bool {
        matches!(self, PageState::Clean)
    }
}

impl Default for PageState {
    fn default() -> Self {
        Self::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_state_checks() {
        let clean = PageState::Clean;
        assert!(clean.is_clean());
        assert!(!clean.is_dirty());

        let dirty = PageState::Dirty;
        assert!(dirty.is_dirty());
        assert!(!dirty.is_clean());
    }

    #[test]
    fn test_page_state_default() {
        assert_eq!(PageState::default(), PageState::Clean);
    }
}
