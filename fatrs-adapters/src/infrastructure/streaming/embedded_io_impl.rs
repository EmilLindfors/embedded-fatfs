//! Implementations of embedded_io_async traits for streaming buffers.
//!
//! These implementations bridge our custom streams to the embedded_io_async ecosystem.
//! The Send/Sync properties are automatically inherited from the device - no manual
//! bounds needed!

use crate::{
    domain::BLOCK_SIZE,
    infrastructure::streaming::{HeapPageStream, StackPageStream, StreamError},
};
use embedded_io_async::{ErrorType, Read, Seek, Write};
use fatrs_block_device::BlockDevice;

// Convert our SeekFrom to embedded_io_async's SeekFrom
fn convert_seek_from(from: embedded_io_async::SeekFrom) -> crate::infrastructure::streaming::SeekFrom {
    match from {
        embedded_io_async::SeekFrom::Start(n) => crate::infrastructure::streaming::SeekFrom::Start(n),
        embedded_io_async::SeekFrom::End(n) => crate::infrastructure::streaming::SeekFrom::End(n),
        embedded_io_async::SeekFrom::Current(n) => crate::infrastructure::streaming::SeekFrom::Current(n),
    }
}

// Implement for StackPageStream
impl<D, const N: usize> ErrorType for StackPageStream<D, N>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    type Error = StreamError<D::Error>;
}

#[cfg(feature = "alloc")]
impl<D, const N: usize> Read for StackPageStream<D, N>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        StackPageStream::read(self, buf).await
    }
}

#[cfg(feature = "alloc")]
impl<D, const N: usize> Write for StackPageStream<D, N>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        StackPageStream::write(self, buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        StackPageStream::flush(self).await
    }
}

#[cfg(feature = "alloc")]
impl<D, const N: usize> Seek for StackPageStream<D, N>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    async fn seek(&mut self, pos: embedded_io_async::SeekFrom) -> Result<u64, Self::Error> {
        StackPageStream::seek(self, convert_seek_from(pos)).await
    }
}

// Implement for HeapPageStream
#[cfg(feature = "alloc")]
impl<D> ErrorType for HeapPageStream<D>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    type Error = StreamError<D::Error>;
}

#[cfg(feature = "alloc")]
impl<D> Read for HeapPageStream<D>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        HeapPageStream::read(self, buf).await
    }
}

#[cfg(feature = "alloc")]
impl<D> Write for HeapPageStream<D>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        HeapPageStream::write(self, buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        HeapPageStream::flush(self).await
    }
}

#[cfg(feature = "alloc")]
impl<D> Seek for HeapPageStream<D>
where
    D: BlockDevice<BLOCK_SIZE> + Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    async fn seek(&mut self, pos: embedded_io_async::SeekFrom) -> Result<u64, Self::Error> {
        HeapPageStream::seek(self, convert_seek_from(pos)).await
    }
}
