//! Audit log for tracking filesystem operations
//!
//! This module provides a persistent audit trail of all filesystem operations,
//! useful for security, compliance, forensics, and debugging.
//!
//! # Architecture
//!
//! ## Storage
//! - Stored as a hidden file `.audit.log` in the root directory
//! - Uses `postcard` binary serialization for compact storage
//! - Circular buffer with configurable maximum size
//! - Automatically rotates when full
//!
//! ## Entry Types
//! - File operations: open, read, write, truncate, close, delete
//! - Directory operations: create, delete, list
//! - Metadata operations: stat, rename, chmod
//!
//! ## Format
//! Each audit entry contains:
//! - Timestamp (milliseconds since epoch)
//! - Operation type
//! - Path(s) involved
//! - Result (success/error)
//! - Optional: size, offset, or other operation-specific data
//!
//! # no_std Compatibility
//! - Uses `postcard` for serialization (no_std compatible)
//! - Fixed-size buffers for paths
//! - Optional alloc feature for unbounded logs

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]

use core::fmt::Debug;

#[cfg(feature = "defmt")]
use defmt;

/// Maximum path length in audit entries (stack-allocated)
const MAX_PATH_LEN: usize = 256;

/// Maximum audit log size in bytes (default: 1MB)
const DEFAULT_MAX_LOG_SIZE: usize = 1024 * 1024;

/// Default number of sectors for audit log (8 sectors = 4KB with 512-byte sectors)
pub const DEFAULT_AUDIT_LOG_SECTORS: u32 = 8;

/// Maximum entries per sector (512 bytes / ~100 bytes per entry ≈ 5 entries)
/// With 8 sectors, we can store ~40 entries on disk
const ENTRIES_PER_SECTOR: usize = 5;

/// Audit logging level - controls which operations are logged
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AuditLevel {
    /// No audit logging
    None = 0,
    /// Only log file/directory creation and deletion (recommended for production)
    Minimal = 1,
    /// Log creates, deletes, and renames (good balance)
    Standard = 2,
    /// Log all operations including reads and writes (verbose, for debugging)
    Full = 3,
}

impl Default for AuditLevel {
    fn default() -> Self {
        AuditLevel::Standard
    }
}

/// Type of filesystem operation being audited
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "audit-log", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum AuditOperation {
    /// File opened for reading
    FileOpenRead = 0,
    /// File opened for writing
    FileOpenWrite = 1,
    /// File created
    FileCreate = 2,
    /// File read operation
    FileRead = 3,
    /// File write operation
    FileWrite = 4,
    /// File truncated
    FileTruncate = 5,
    /// File deleted
    FileDelete = 6,
    /// File closed
    FileClose = 7,
    /// Directory created
    DirCreate = 8,
    /// Directory deleted
    DirDelete = 9,
    /// Directory listed
    DirList = 10,
    /// File/directory renamed
    Rename = 11,
    /// File/directory stat
    Stat = 12,
    /// File/directory metadata changed
    MetadataUpdate = 13,
}

impl AuditOperation {
    /// Check if this operation should be logged at the given audit level
    pub const fn should_log(&self, level: AuditLevel) -> bool {
        match level {
            AuditLevel::None => false,
            AuditLevel::Minimal => matches!(
                self,
                AuditOperation::FileCreate
                    | AuditOperation::FileDelete
                    | AuditOperation::DirCreate
                    | AuditOperation::DirDelete
            ),
            AuditLevel::Standard => matches!(
                self,
                AuditOperation::FileCreate
                    | AuditOperation::FileDelete
                    | AuditOperation::DirCreate
                    | AuditOperation::DirDelete
                    | AuditOperation::Rename
                    | AuditOperation::FileTruncate
            ),
            AuditLevel::Full => true, // Log everything
        }
    }
}

/// Result of an operation
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "audit-log", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum AuditResult {
    /// Operation succeeded
    Success = 0,
    /// Operation failed
    Error = 1,
}

/// A single audit log entry
///
/// Optimized for size using postcard serialization.
/// Uses fixed-size arrays for no_std compatibility.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "audit-log", derive(serde::Serialize, serde::Deserialize))]
pub struct AuditEntry {
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
    /// Type of operation
    pub operation: AuditOperation,
    /// Result of operation
    pub result: AuditResult,
    /// Primary path (e.g., file being accessed)
    #[cfg_attr(feature = "audit-log", serde(with = "serde_big_array::BigArray"))]
    pub path: [u8; MAX_PATH_LEN],
    /// Length of valid path data
    pub path_len: u16,
    /// Optional secondary path (e.g., rename destination)
    #[cfg_attr(feature = "audit-log", serde(with = "serde_big_array::BigArray"))]
    pub path2: [u8; MAX_PATH_LEN],
    /// Length of valid path2 data
    pub path2_len: u16,
    /// Optional size/offset/count parameter
    pub data: u64,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(
        timestamp: u64,
        operation: AuditOperation,
        result: AuditResult,
        path: &str,
    ) -> Self {
        let mut entry = Self {
            timestamp,
            operation,
            result,
            path: [0; MAX_PATH_LEN],
            path_len: 0,
            path2: [0; MAX_PATH_LEN],
            path2_len: 0,
            data: 0,
        };
        entry.set_path(path);
        entry
    }

    /// Set the primary path
    pub fn set_path(&mut self, path: &str) {
        let bytes = path.as_bytes();
        let len = bytes.len().min(MAX_PATH_LEN);
        self.path[..len].copy_from_slice(&bytes[..len]);
        self.path_len = len as u16;
    }

    /// Set the secondary path (for rename operations)
    pub fn set_path2(&mut self, path: &str) {
        let bytes = path.as_bytes();
        let len = bytes.len().min(MAX_PATH_LEN);
        self.path2[..len].copy_from_slice(&bytes[..len]);
        self.path2_len = len as u16;
    }

    /// Get the primary path as a string
    pub fn get_path(&self) -> &str {
        core::str::from_utf8(&self.path[..self.path_len as usize]).unwrap_or("<invalid>")
    }

    /// Get the secondary path as a string
    pub fn get_path2(&self) -> Option<&str> {
        if self.path2_len > 0 {
            Some(core::str::from_utf8(&self.path2[..self.path2_len as usize]).unwrap_or("<invalid>"))
        } else {
            None
        }
    }

    /// Set operation-specific data (size, offset, etc.)
    pub fn with_data(mut self, data: u64) -> Self {
        self.data = data;
        self
    }
}

/// Audit log configuration
#[derive(Debug, Clone, Copy)]
pub struct AuditConfig {
    /// Starting sector for audit log storage
    pub log_start_sector: u32,
    /// Number of sectors allocated for audit log
    pub log_sector_count: u32,
    /// Enable audit logging
    pub enabled: bool,
    /// Audit level - controls which operations are logged
    pub level: AuditLevel,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_start_sector: 0, // Will be set automatically during mount
            log_sector_count: DEFAULT_AUDIT_LOG_SECTORS,
            enabled: true,
            level: AuditLevel::default(),
        }
    }
}

impl AuditConfig {
    /// Create a new audit configuration with automatic sector placement
    pub fn new() -> Self {
        Self::default()
    }

    /// Create configuration with automatic sector allocation based on filesystem size
    ///
    /// Allocates sectors proportional to filesystem size:
    /// - < 10MB: 8 sectors (4KB)
    /// - 10MB - 100MB: 16 sectors (8KB)
    /// - 100MB - 1GB: 32 sectors (16KB)
    /// - 1GB - 10GB: 64 sectors (32KB)
    /// - > 10GB: 128 sectors (64KB)
    pub fn automatic(total_sectors: u32) -> Self {
        let sector_count = if total_sectors < 20_480 {
            // < 10MB
            8
        } else if total_sectors < 204_800 {
            // 10MB - 100MB
            16
        } else if total_sectors < 2_097_152 {
            // 100MB - 1GB
            32
        } else if total_sectors < 20_971_520 {
            // 1GB - 10GB
            64
        } else {
            // > 10GB
            128
        };

        Self {
            log_start_sector: 0, // Will be set automatically during mount
            log_sector_count: sector_count,
            enabled: true,
            level: AuditLevel::default(),
        }
    }

    /// Create configuration with explicit sector location
    pub fn at_sector(log_start_sector: u32, log_sector_count: u32) -> Self {
        Self {
            log_start_sector,
            log_sector_count,
            enabled: true,
            level: AuditLevel::default(),
        }
    }

    /// Set the number of sectors for the audit log
    pub fn sector_count(mut self, count: u32) -> Self {
        self.log_sector_count = count;
        self
    }

    /// Enable or disable audit logging
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the audit logging level
    pub fn level(mut self, level: AuditLevel) -> Self {
        self.level = level;
        self
    }
}

/// In-memory audit log buffer
///
/// Holds audit entries before they're written to disk.
/// Uses fixed-size buffer for no_std compatibility.
pub struct AuditLog {
    /// Configuration
    config: AuditConfig,
    /// Number of entries in buffer
    count: usize,
    /// Buffer of pending entries (up to 16 entries)
    buffer: [Option<AuditEntry>; 16],
    /// Whether the buffer has unsaved changes
    dirty: bool,
}

impl AuditLog {
    /// Create a new audit log with default configuration
    pub fn new(config: AuditConfig) -> Self {
        Self {
            config,
            count: 0,
            buffer: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            dirty: false,
        }
    }

    /// Add an entry to the audit log
    pub fn log(&mut self, entry: AuditEntry) {
        if !self.config.enabled {
            return;
        }

        // Check if this operation should be logged at the configured level
        if !entry.operation.should_log(self.config.level) {
            return;
        }

        if self.count < self.buffer.len() {
            self.buffer[self.count] = Some(entry);
            self.count += 1;
        } else {
            // Buffer full - shift left and add new entry at end
            for i in 0..self.buffer.len() - 1 {
                self.buffer[i] = self.buffer[i + 1].take();
            }
            self.buffer[self.buffer.len() - 1] = Some(entry);
        }
        self.dirty = true;
    }

    /// Helper: log a file operation
    pub fn log_file_op(&mut self, timestamp: u64, operation: AuditOperation, path: &str, result: AuditResult) {
        // Early return if this operation won't be logged
        if !operation.should_log(self.config.level) {
            return;
        }
        self.log(AuditEntry::new(timestamp, operation, result, path));
    }

    /// Helper: log a file operation with data (size, offset, etc.)
    pub fn log_file_op_with_data(&mut self, timestamp: u64, operation: AuditOperation, path: &str, result: AuditResult, data: u64) {
        // Early return if this operation won't be logged
        if !operation.should_log(self.config.level) {
            return;
        }
        self.log(AuditEntry::new(timestamp, operation, result, path).with_data(data));
    }

    /// Helper: log a rename operation
    pub fn log_rename(&mut self, timestamp: u64, old_path: &str, new_path: &str, result: AuditResult) {
        // Early return if rename won't be logged
        if !AuditOperation::Rename.should_log(self.config.level) {
            return;
        }
        let mut entry = AuditEntry::new(timestamp, AuditOperation::Rename, result, old_path);
        entry.set_path2(new_path);
        self.log(entry);
    }

    /// Get all entries in the buffer
    pub fn entries(&self) -> impl Iterator<Item = &AuditEntry> {
        self.buffer[..self.count].iter().filter_map(|e| e.as_ref())
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.count = 0;
        for entry in &mut self.buffer {
            *entry = None;
        }
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        self.count >= self.buffer.len()
    }

    /// Get number of entries in buffer
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Check if log has unsaved changes
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Write audit log to reserved disk sectors
    pub async fn flush<IO: crate::io::Write + crate::io::Seek>(&mut self, disk: &mut IO) -> Result<(), IO::Error> {
        use crate::io::{WriteLeExt, SeekFrom};

        if !self.dirty || !self.config.enabled {
            return Ok(());
        }

        let sector_size = 512u64; // Standard sector size
        let start_offset = u64::from(self.config.log_start_sector) * sector_size;

        // Seek to audit log area
        disk.seek(SeekFrom::Start(start_offset)).await?;

        // Write entries using postcard serialization
        #[cfg(all(feature = "alloc", not(feature = "std")))]
        {
            let entries: alloc::vec::Vec<_> = self.entries().cloned().collect();
            if let Ok(data) = postcard::to_allocvec(&entries) {
                // Write length prefix
                disk.write_u32_le(data.len() as u32).await?;
                // Write serialized data
                disk.write_all(&data).await?;
            }
        }

        #[cfg(feature = "std")]
        {
            let entries: Vec<_> = self.entries().cloned().collect();
            if let Ok(data) = postcard::to_allocvec(&entries) {
                // Write length prefix
                disk.write_u32_le(data.len() as u32).await?;
                // Write serialized data
                disk.write_all(&data).await?;
            }
        }

        disk.flush().await?;
        self.dirty = false;
        Ok(())
    }

    /// Load audit log from reserved disk sectors
    pub async fn load<IO: crate::io::Read + crate::io::Seek>(&mut self, disk: &mut IO) -> Result<(), IO::Error> {
        use crate::io::{ReadLeExt, SeekFrom};

        if !self.config.enabled {
            return Ok(());
        }

        let sector_size = 512u64;
        let start_offset = u64::from(self.config.log_start_sector) * sector_size;

        // Seek to audit log area
        disk.seek(SeekFrom::Start(start_offset)).await?;

        // Read length prefix
        let data_len = match disk.read_u32_le().await {
            Ok(len) => len as usize,
            Err(_) => return Ok(()), // No data or error, start fresh
        };

        if data_len == 0 || data_len > (self.config.log_sector_count as usize * 512) {
            return Ok(()); // Invalid or no data
        }

        // Read serialized data
        #[cfg(all(feature = "alloc", not(feature = "std")))]
        {
            let mut data = alloc::vec![0u8; data_len];
            if disk.read_exact(&mut data).await.is_ok() {
                if let Ok(entries) = postcard::from_bytes::<alloc::vec::Vec<AuditEntry>>(&data) {
                    // Load entries into buffer
                    self.count = 0;
                    for entry in entries.into_iter().take(self.buffer.len()) {
                        self.buffer[self.count] = Some(entry);
                        self.count += 1;
                    }
                }
            }
        }

        #[cfg(feature = "std")]
        {
            let mut data = vec![0u8; data_len];
            if disk.read_exact(&mut data).await.is_ok() {
                if let Ok(entries) = postcard::from_bytes::<Vec<AuditEntry>>(&data) {
                    // Load entries into buffer
                    self.count = 0;
                    for entry in entries.into_iter().take(self.buffer.len()) {
                        self.buffer[self.count] = Some(entry);
                        self.count += 1;
                    }
                }
            }
        }

        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_paths() {
        let mut entry = AuditEntry::new(
            1000,
            AuditOperation::FileCreate,
            AuditResult::Success,
            "/test/file.txt",
        );

        assert_eq!(entry.get_path(), "/test/file.txt");
        assert_eq!(entry.get_path2(), None);

        entry.set_path2("/test/renamed.txt");
        assert_eq!(entry.get_path2(), Some("/test/renamed.txt"));
    }

    #[test]
    fn test_audit_log_buffer() {
        let mut log = AuditLog::new(AuditConfig::default());
        assert!(log.is_empty());

        log.log(AuditEntry::new(
            1000,
            AuditOperation::FileCreate,
            AuditResult::Success,
            "/test.txt",
        ));

        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());

        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn test_audit_log_overflow() {
        let mut log = AuditLog::new(AuditConfig::default());

        // Fill buffer beyond capacity
        for i in 0..20 {
            log.log(AuditEntry::new(
                i as u64,
                AuditOperation::FileCreate,
                AuditResult::Success,
                "/test.txt",
            ));
        }

        // Should have exactly 16 entries (buffer size)
        assert_eq!(log.len(), 16);

        // Oldest entries should have been dropped
        let entries: Vec<_> = log.entries().collect();
        assert_eq!(entries[0].timestamp, 4); // Entry 0-3 were dropped
    }
}
