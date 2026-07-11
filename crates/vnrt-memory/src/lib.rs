//! Sparse, page-based 32-bit guest virtual memory.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fixed guest page size. Four KiB matches ordinary 32-bit x86 pages.
pub const PAGE_SIZE: usize = 4096;
/// Guest page size in address arithmetic form.
pub const PAGE_SIZE_U32: u32 = 4096;
const PAGE_MASK: u32 = !(PAGE_SIZE_U32 - 1);

/// A virtual address in the 32-bit guest address space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GuestAddress(pub u32);

impl GuestAddress {
    /// Returns the page-aligned base containing this address.
    #[must_use]
    pub const fn page_base(self) -> Self {
        Self(self.0 & PAGE_MASK)
    }

    /// Returns the byte offset within the containing page.
    #[must_use]
    pub const fn page_offset(self) -> usize {
        (self.0 & !PAGE_MASK) as usize
    }
}

/// Access permissions applied independently to every guest page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    /// Data may be read.
    pub read: bool,
    /// Data may be written.
    pub write: bool,
    /// Instructions may be fetched.
    pub execute: bool,
}

impl Permissions {
    /// No access.
    pub const NONE: Self = Self::new(false, false, false);
    /// Read-only data.
    pub const READ: Self = Self::new(true, false, false);
    /// Read/write data.
    pub const READ_WRITE: Self = Self::new(true, true, false);
    /// Readable executable code.
    pub const READ_EXECUTE: Self = Self::new(true, false, true);
    /// Loader-only unrestricted mapping.
    pub const ALL: Self = Self::new(true, true, true);

    /// Build a permission set.
    #[must_use]
    pub const fn new(read: bool, write: bool, execute: bool) -> Self {
        Self {
            read,
            write,
            execute,
        }
    }
}

/// Why a guest memory access was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Ordinary data read.
    Read,
    /// Data write.
    Write,
    /// Instruction fetch.
    Execute,
}

/// Guest memory mapping and access errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
    /// No page exists at the requested address.
    #[error("guest address {address:#010x} is not mapped")]
    NotMapped {
        /// First guest address that could not be resolved.
        address: u32,
    },
    /// The page exists but rejects this kind of access.
    #[error("guest {access:?} access denied at {address:#010x}")]
    Protection {
        /// First guest address rejected by the page permissions.
        address: u32,
        /// Attempted access kind.
        access: AccessKind,
    },
    /// A caller tried to map an occupied page.
    #[error("guest page {address:#010x} is already mapped")]
    AlreadyMapped {
        /// Base of the occupied page.
        address: u32,
    },
    /// A range crossed the end of the 32-bit address space.
    #[error("guest address range overflows 32-bit address space")]
    AddressOverflow,
    /// The requested operation is intentionally not implemented yet.
    #[error("unsupported memory operation: {0}")]
    Unsupported(&'static str),
}

#[derive(Clone)]
struct Page {
    bytes: Box<[u8; PAGE_SIZE]>,
    permissions: Permissions,
}

impl Page {
    fn zeroed(permissions: Permissions) -> Self {
        Self {
            bytes: Box::new([0; PAGE_SIZE]),
            permissions,
        }
    }
}

/// Sparse page table covering the complete 32-bit guest address space.
///
/// Pages use stable boxed storage. A future translated-block cache may safely
/// add short-lived host pointers, provided unmapping and permission changes
/// invalidate those cached translations.
#[derive(Clone, Default)]
pub struct GuestMemory {
    pages: BTreeMap<u32, Page>,
}

impl GuestMemory {
    /// Create an empty address space.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
        }
    }

    /// Map a page-aligned, page-sized range initialized to zero.
    pub fn map_range(
        &mut self,
        start: GuestAddress,
        len: u32,
        permissions: Permissions,
    ) -> Result<(), MemoryError> {
        validate_page_range(start, len)?;
        let end = start
            .0
            .checked_add(len)
            .ok_or(MemoryError::AddressOverflow)?;
        for address in (start.0..end).step_by(PAGE_SIZE) {
            if self.pages.contains_key(&address) {
                return Err(MemoryError::AlreadyMapped { address });
            }
        }
        for address in (start.0..end).step_by(PAGE_SIZE) {
            self.pages.insert(address, Page::zeroed(permissions));
        }
        Ok(())
    }

    /// Change access permissions on mapped whole pages.
    pub fn protect_range(
        &mut self,
        start: GuestAddress,
        len: u32,
        permissions: Permissions,
    ) -> Result<(), MemoryError> {
        validate_page_range(start, len)?;
        let end = start
            .0
            .checked_add(len)
            .ok_or(MemoryError::AddressOverflow)?;
        for address in (start.0..end).step_by(PAGE_SIZE) {
            let page = self
                .pages
                .get_mut(&address)
                .ok_or(MemoryError::NotMapped { address })?;
            page.permissions = permissions;
        }
        Ok(())
    }

    /// Remove a page-aligned range from the address space.
    ///
    /// Validation is transactional: if any page is absent, no pages are
    /// removed.
    pub fn unmap_range(&mut self, start: GuestAddress, len: u32) -> Result<(), MemoryError> {
        validate_page_range(start, len)?;
        let end = start
            .0
            .checked_add(len)
            .ok_or(MemoryError::AddressOverflow)?;
        for address in (start.0..end).step_by(PAGE_SIZE) {
            if !self.pages.contains_key(&address) {
                return Err(MemoryError::NotMapped { address });
            }
        }
        for address in (start.0..end).step_by(PAGE_SIZE) {
            self.pages.remove(&address);
        }
        Ok(())
    }

    /// Whether every page in an aligned range is currently unmapped.
    pub fn is_range_free(&self, start: GuestAddress, len: u32) -> Result<bool, MemoryError> {
        validate_page_range(start, len)?;
        let end = start
            .0
            .checked_add(len)
            .ok_or(MemoryError::AddressOverflow)?;
        Ok((start.0..end)
            .step_by(PAGE_SIZE)
            .all(|address| !self.pages.contains_key(&address)))
    }

    /// Read data using ordinary read permissions.
    pub fn read(&self, address: GuestAddress, output: &mut [u8]) -> Result<(), MemoryError> {
        self.read_for(address, output, AccessKind::Read)
    }

    /// Fetch instruction bytes using execute permissions.
    pub fn fetch(&self, address: GuestAddress, output: &mut [u8]) -> Result<(), MemoryError> {
        self.read_for(address, output, AccessKind::Execute)
    }

    /// Write bytes, transparently crossing page boundaries.
    pub fn write(&mut self, address: GuestAddress, input: &[u8]) -> Result<(), MemoryError> {
        walk_chunks(address, input.len(), |guest, source_offset, chunk_len| {
            let page = self
                .pages
                .get_mut(&guest.page_base().0)
                .ok_or(MemoryError::NotMapped { address: guest.0 })?;
            if !page.permissions.write {
                return Err(MemoryError::Protection {
                    address: guest.0,
                    access: AccessKind::Write,
                });
            }
            let page_offset = guest.page_offset();
            page.bytes[page_offset..page_offset + chunk_len]
                .copy_from_slice(&input[source_offset..source_offset + chunk_len]);
            Ok(())
        })
    }

    /// Read a little-endian 32-bit integer.
    pub fn read_u32(&self, address: GuestAddress) -> Result<u32, MemoryError> {
        let mut bytes = [0; 4];
        self.read(address, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Read one byte.
    pub fn read_u8(&self, address: GuestAddress) -> Result<u8, MemoryError> {
        let mut byte = [0];
        self.read(address, &mut byte)?;
        Ok(byte[0])
    }

    /// Read a little-endian 16-bit integer.
    pub fn read_u16(&self, address: GuestAddress) -> Result<u16, MemoryError> {
        let mut bytes = [0; 2];
        self.read(address, &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Write a little-endian 32-bit integer.
    pub fn write_u32(&mut self, address: GuestAddress, value: u32) -> Result<(), MemoryError> {
        self.write(address, &value.to_le_bytes())
    }

    /// Write one byte.
    pub fn write_u8(&mut self, address: GuestAddress, value: u8) -> Result<(), MemoryError> {
        self.write(address, &[value])
    }

    /// Write a little-endian 16-bit integer.
    pub fn write_u16(&mut self, address: GuestAddress, value: u16) -> Result<(), MemoryError> {
        self.write(address, &value.to_le_bytes())
    }

    fn read_for(
        &self,
        address: GuestAddress,
        output: &mut [u8],
        access: AccessKind,
    ) -> Result<(), MemoryError> {
        walk_chunks(address, output.len(), |guest, target_offset, chunk_len| {
            let page = self
                .pages
                .get(&guest.page_base().0)
                .ok_or(MemoryError::NotMapped { address: guest.0 })?;
            let permitted = match access {
                AccessKind::Read => page.permissions.read,
                AccessKind::Write => page.permissions.write,
                AccessKind::Execute => page.permissions.execute,
            };
            if !permitted {
                return Err(MemoryError::Protection {
                    address: guest.0,
                    access,
                });
            }
            let page_offset = guest.page_offset();
            output[target_offset..target_offset + chunk_len]
                .copy_from_slice(&page.bytes[page_offset..page_offset + chunk_len]);
            Ok(())
        })
    }
}

fn validate_page_range(start: GuestAddress, len: u32) -> Result<(), MemoryError> {
    if start.page_offset() != 0 || len == 0 || !len.is_multiple_of(PAGE_SIZE_U32) {
        return Err(MemoryError::Unsupported(
            "mapping and protection ranges must contain whole aligned pages",
        ));
    }
    start
        .0
        .checked_add(len)
        .ok_or(MemoryError::AddressOverflow)?;
    Ok(())
}

fn walk_chunks(
    start: GuestAddress,
    len: usize,
    mut visitor: impl FnMut(GuestAddress, usize, usize) -> Result<(), MemoryError>,
) -> Result<(), MemoryError> {
    let len_u32 = u32::try_from(len).map_err(|_| MemoryError::AddressOverflow)?;
    start
        .0
        .checked_add(len_u32)
        .ok_or(MemoryError::AddressOverflow)?;
    let mut completed = 0;
    while completed < len {
        let completed_u32 = u32::try_from(completed).map_err(|_| MemoryError::AddressOverflow)?;
        let address = GuestAddress(start.0 + completed_u32);
        let chunk_len = (PAGE_SIZE - address.page_offset()).min(len - completed);
        visitor(address, completed, chunk_len)?;
        completed += chunk_len;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_across_page_boundary() {
        let mut memory = GuestMemory::new();
        memory
            .map_range(GuestAddress(0x1000), 0x2000, Permissions::READ_WRITE)
            .expect("mapping should succeed");
        memory
            .write(GuestAddress(0x1ffe), &[1, 2, 3, 4])
            .expect("cross-page write should succeed");
        let mut output = [0; 4];
        memory
            .read(GuestAddress(0x1ffe), &mut output)
            .expect("cross-page read should succeed");
        assert_eq!(output, [1, 2, 3, 4]);
    }

    #[test]
    fn enforces_write_permission() {
        let mut memory = GuestMemory::new();
        memory
            .map_range(GuestAddress(0x4000), 0x1000, Permissions::READ)
            .expect("mapping should succeed");
        assert_eq!(
            memory.write(GuestAddress(0x4000), &[1]),
            Err(MemoryError::Protection {
                address: 0x4000,
                access: AccessKind::Write,
            })
        );
    }

    #[test]
    fn unmap_is_transactional_and_releases_pages() {
        let mut memory = GuestMemory::new();
        memory
            .map_range(GuestAddress(0x8000), 0x2000, Permissions::READ_WRITE)
            .unwrap();
        assert!(!memory.is_range_free(GuestAddress(0x8000), 0x2000).unwrap());
        memory.unmap_range(GuestAddress(0x8000), 0x2000).unwrap();
        assert!(memory.is_range_free(GuestAddress(0x8000), 0x2000).unwrap());

        memory
            .map_range(GuestAddress(0x8000), 0x1000, Permissions::READ_WRITE)
            .unwrap();
        assert!(memory.unmap_range(GuestAddress(0x8000), 0x2000).is_err());
        assert!(!memory.is_range_free(GuestAddress(0x8000), 0x1000).unwrap());
    }
}
