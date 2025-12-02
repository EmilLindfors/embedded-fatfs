//! Type-safe block address value object.

use core::fmt;

/// A validated block address.
///
/// This value object provides type safety to prevent mixing block addresses
/// with other integer types like page numbers or byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockAddress(u32);

impl BlockAddress {
    /// Create a new block address.
    ///
    /// # Examples
    ///
    /// ```
    /// use fatrs_adapters::domain::BlockAddress;
    ///
    /// let addr = BlockAddress::new(0);
    /// assert_eq!(addr.value(), 0);
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

    /// Add an offset to this block address.
    #[inline]
    pub const fn add(self, offset: u32) -> Self {
        Self(self.0.saturating_add(offset))
    }

    /// Calculate the offset between two block addresses.
    #[inline]
    pub const fn offset_from(self, other: Self) -> u32 {
        self.0.saturating_sub(other.0)
    }
}

impl fmt::Display for BlockAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Block({})", self.0)
    }
}

impl From<u32> for BlockAddress {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<BlockAddress> for u32 {
    fn from(addr: BlockAddress) -> Self {
        addr.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_address_creation() {
        let addr = BlockAddress::new(100);
        assert_eq!(addr.value(), 100);
    }

    #[test]
    fn test_block_address_add() {
        let addr = BlockAddress::new(10);
        let new_addr = addr.add(5);
        assert_eq!(new_addr.value(), 15);

        let max_addr = BlockAddress::new(u32::MAX);
        assert_eq!(max_addr.add(1).value(), u32::MAX); // saturating
    }

    #[test]
    fn test_block_address_offset() {
        let addr1 = BlockAddress::new(100);
        let addr2 = BlockAddress::new(50);
        assert_eq!(addr1.offset_from(addr2), 50);
        assert_eq!(addr2.offset_from(addr1), 0); // saturating
    }

    #[test]
    fn test_block_address_display() {
        let addr = BlockAddress::new(512);
        assert_eq!(format!("{}", addr), "Block(512)");
    }
}
