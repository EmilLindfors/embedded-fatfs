//! Path parsing utilities for CLI commands
//!
//! This module provides utilities for parsing path specifications that distinguish
//! between host filesystem paths and paths within FAT images.
//!
//! # Path Notation
//!
//! The CLI uses a clear notation to distinguish between host and image paths:
//!
//! - `image.img:path/to/file` - Path within the FAT image
//! - `./path/to/file` or `/path/to/file` - Path on host filesystem
//!
//! This is similar to the familiar `host:path` syntax used by tools like scp and rsync.

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

/// A parsed path specification that can be either a host path or an image path
#[derive(Debug, Clone, PartialEq)]
pub enum PathSpec {
    /// Path within a FAT image (e.g., "test.img:file.txt")
    ImagePath { image: PathBuf, path: String },
    /// Path on the host filesystem (e.g., "./file.txt" or "/usr/local/file.txt")
    HostPath(PathBuf),
}

impl PathSpec {
    /// Parse a path specification string into a PathSpec
    ///
    /// # Syntax
    ///
    /// - `image:path` - Explicit image path (e.g., "test.img:dir/file.txt")
    /// - Anything else - Host filesystem path
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use fatrs_cli::path_parser::PathSpec;
    ///
    /// // Image path with explicit notation
    /// let spec = PathSpec::parse("test.img:file.txt").unwrap();
    /// assert!(matches!(spec, PathSpec::ImagePath { .. }));
    ///
    /// // Host path
    /// let spec = PathSpec::parse("./file.txt").unwrap();
    /// assert!(matches!(spec, PathSpec::HostPath(_)));
    /// ```
    pub fn parse(spec: &str) -> Result<Self> {
        // Check for image:path notation
        if let Some((img, path)) = spec.split_once(':') {
            // On Windows, we need to handle drive letters like "C:"
            // If the first part is a single letter, it's likely a Windows drive
            if cfg!(windows) && img.len() == 1 && img.chars().next().unwrap().is_alphabetic() {
                // This is a Windows path like "C:\file.txt", treat as host path
                return Ok(PathSpec::HostPath(PathBuf::from(spec)));
            }

            // Explicit image:path notation
            Ok(PathSpec::ImagePath {
                image: PathBuf::from(img),
                path: path.to_string(),
            })
        } else {
            // No colon, treat as host path
            Ok(PathSpec::HostPath(PathBuf::from(spec)))
        }
    }

    /// Parse a path specification with a default image
    ///
    /// This is useful for commands like `rm` and `cat` where the image is specified
    /// separately and the path can be either in the image or on the host.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use fatrs_cli::path_parser::PathSpec;
    ///
    /// // If spec contains image:path, use that
    /// let spec = PathSpec::parse_with_default_image("other.img:file.txt", Path::new("default.img")).unwrap();
    /// if let PathSpec::ImagePath { image, .. } = spec {
    ///     assert_eq!(image, Path::new("other.img"));
    /// }
    ///
    /// // Otherwise, use default image
    /// let spec = PathSpec::parse_with_default_image("file.txt", Path::new("default.img")).unwrap();
    /// if let PathSpec::ImagePath { image, .. } = spec {
    ///     assert_eq!(image, Path::new("default.img"));
    /// }
    /// ```
    pub fn parse_with_default_image(spec: &str, default_image: &Path) -> Result<Self> {
        if let Some((img, path)) = spec.split_once(':') {
            // On Windows, handle drive letters
            if cfg!(windows) && img.len() == 1 && img.chars().next().unwrap().is_alphabetic() {
                // This is a Windows path, but we're expecting image paths
                // This is likely an error
                anyhow::bail!(
                    "Path '{}' looks like a Windows drive path. For image paths, use 'image.img:path' notation.",
                    spec
                );
            }

            // Explicit image:path notation
            Ok(PathSpec::ImagePath {
                image: PathBuf::from(img),
                path: path.to_string(),
            })
        } else {
            // No colon, assume it's a path in the default image
            Ok(PathSpec::ImagePath {
                image: default_image.to_path_buf(),
                path: spec.to_string(),
            })
        }
    }

    /// Get the image path if this is an ImagePath
    pub fn image_path(&self) -> Option<&Path> {
        match self {
            PathSpec::ImagePath { image, .. } => Some(image),
            PathSpec::HostPath(_) => None,
        }
    }

    /// Get the path within the image if this is an ImagePath
    pub fn inner_path(&self) -> Option<&str> {
        match self {
            PathSpec::ImagePath { path, .. } => Some(path),
            PathSpec::HostPath(_) => None,
        }
    }

    /// Get the host path if this is a HostPath
    pub fn host_path(&self) -> Option<&Path> {
        match self {
            PathSpec::HostPath(path) => Some(path),
            PathSpec::ImagePath { .. } => None,
        }
    }

    /// Returns true if this is an image path
    pub fn is_image_path(&self) -> bool {
        matches!(self, PathSpec::ImagePath { .. })
    }

    /// Returns true if this is a host path
    pub fn is_host_path(&self) -> bool {
        matches!(self, PathSpec::HostPath(_))
    }
}

/// Parse a copy operation with source and destination paths
///
/// This validates that the copy operation makes sense (i.e., not copying
/// within the same location).
pub fn parse_copy_operation(
    source: &str,
    dest: &str,
) -> Result<(PathSpec, PathSpec)> {
    let src = PathSpec::parse(source)
        .with_context(|| format!("Failed to parse source path: {}", source))?;
    let dst = PathSpec::parse(dest)
        .with_context(|| format!("Failed to parse destination path: {}", dest))?;

    // Validate the operation makes sense
    match (&src, &dst) {
        (PathSpec::ImagePath { image: img1, .. }, PathSpec::ImagePath { image: img2, .. }) => {
            if img1 == img2 {
                anyhow::bail!(
                    "Cannot copy within the same image '{}'. Use host filesystem for intermediate storage.",
                    img1.display()
                );
            }
        }
        (PathSpec::HostPath(_), PathSpec::HostPath(_)) => {
            anyhow::bail!(
                "Cannot copy within host filesystem. Use standard tools like 'cp' instead.\n\
                To copy to/from images, use notation like 'fatrs cp source.txt image.img:dest.txt'"
            );
        }
        _ => {
            // Valid: copying between host and image
        }
    }

    Ok((src, dst))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_image_path() {
        let spec = PathSpec::parse("test.img:file.txt").unwrap();
        match spec {
            PathSpec::ImagePath { image, path } => {
                assert_eq!(image, PathBuf::from("test.img"));
                assert_eq!(path, "file.txt");
            }
            _ => panic!("Expected ImagePath"),
        }
    }

    #[test]
    fn test_parse_image_path_with_subdirs() {
        let spec = PathSpec::parse("test.img:dir/subdir/file.txt").unwrap();
        match spec {
            PathSpec::ImagePath { image, path } => {
                assert_eq!(image, PathBuf::from("test.img"));
                assert_eq!(path, "dir/subdir/file.txt");
            }
            _ => panic!("Expected ImagePath"),
        }
    }

    #[test]
    fn test_parse_host_path() {
        let spec = PathSpec::parse("./file.txt").unwrap();
        assert!(matches!(spec, PathSpec::HostPath(_)));

        let spec = PathSpec::parse("/usr/local/file.txt").unwrap();
        assert!(matches!(spec, PathSpec::HostPath(_)));
    }

    #[test]
    fn test_parse_with_default_image() {
        let spec = PathSpec::parse_with_default_image("file.txt", Path::new("default.img")).unwrap();
        match spec {
            PathSpec::ImagePath { image, path } => {
                assert_eq!(image, PathBuf::from("default.img"));
                assert_eq!(path, "file.txt");
            }
            _ => panic!("Expected ImagePath"),
        }
    }

    #[test]
    fn test_parse_with_default_image_explicit() {
        let spec = PathSpec::parse_with_default_image("other.img:file.txt", Path::new("default.img")).unwrap();
        match spec {
            PathSpec::ImagePath { image, path } => {
                assert_eq!(image, PathBuf::from("other.img"));
                assert_eq!(path, "file.txt");
            }
            _ => panic!("Expected ImagePath"),
        }
    }

    #[test]
    fn test_parse_copy_operation_valid() {
        let (src, dst) = parse_copy_operation("host.txt", "image.img:dest.txt").unwrap();
        assert!(src.is_host_path());
        assert!(dst.is_image_path());
    }

    #[test]
    fn test_parse_copy_operation_same_image() {
        let result = parse_copy_operation("test.img:src.txt", "test.img:dest.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("same image"));
    }

    #[test]
    fn test_parse_copy_operation_both_host() {
        let result = parse_copy_operation("./src.txt", "./dest.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("host filesystem"));
    }
}
