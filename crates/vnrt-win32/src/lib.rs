//! Win32-facing state and the host-call registry.
//!
//! API handlers work through [`HostCallContext`], so this crate does not need to
//! own or depend on the x86 interpreter.

use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;
pub use vnrt_gfx::{TextureDescriptor, TextureId};
pub use vnrt_memory::GuestAddress;

/// Default safety limit for NUL-terminated guest strings.
pub const MAX_GUEST_STRING_BYTES: usize = 32 * 1024;
/// Stable pseudo handle used for the initial process heap.
pub const PROCESS_HEAP_HANDLE: u32 = 0x0001_0000;

/// Opaque 32-bit value visible to guest code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Handle(pub u32);

/// Generic owner for host resources referenced by guest handles.
#[derive(Debug)]
pub struct HandleTable<T> {
    next: u32,
    entries: HashMap<Handle, T>,
}

impl<T> Default for HandleTable<T> {
    fn default() -> Self {
        Self {
            next: 4,
            entries: HashMap::new(),
        }
    }
}

impl<T> HandleTable<T> {
    /// Insert a value and allocate a non-null handle.
    pub fn insert(&mut self, value: T) -> Result<Handle, Win32Error> {
        let handle = Handle(self.next);
        self.next = self
            .next
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        self.entries.insert(handle, value);
        Ok(handle)
    }

    /// Borrow the resource referenced by a handle.
    #[must_use]
    pub fn get(&self, handle: Handle) -> Option<&T> {
        self.entries.get(&handle)
    }

    /// Mutably borrow the resource referenced by a handle.
    pub fn get_mut(&mut self, handle: Handle) -> Option<&mut T> {
        self.entries.get_mut(&handle)
    }

    /// Remove and return a resource.
    pub fn remove(&mut self, handle: Handle) -> Option<T> {
        self.entries.remove(&handle)
    }

    /// Number of live handles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no handles are live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A guest-loaded module tracked by the compatibility layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Module {
    /// Normalized module name.
    pub name: String,
    /// Guest load base.
    pub base: GuestAddress,
    /// Mapped image size.
    pub size: u32,
}

/// Loaded-module lookup by normalized ASCII name.
#[derive(Debug, Default)]
pub struct ModuleTable {
    modules: HashMap<String, Module>,
}

impl ModuleTable {
    /// Add or replace a module record.
    pub fn insert(&mut self, mut module: Module) {
        module.name.make_ascii_lowercase();
        self.modules.insert(module.name.clone(), module);
    }

    /// Find a module case-insensitively.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Module> {
        self.modules.get(&name.to_ascii_lowercase())
    }
}

/// One seekable read-only file owned by the virtual filesystem boundary.
pub trait VirtualReadFile: Send {
    /// Read at most `length` bytes from the current cursor.
    fn read(&mut self, length: usize) -> Result<Vec<u8>, Win32Error>;
    /// Move the cursor relative to start, current position, or end.
    fn seek(&mut self, distance: i64, origin: u32) -> Result<u64, Win32Error>;
    /// Exact byte length of the file.
    fn len(&self) -> u64;
    /// Whether the file has no bytes.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Minimal filesystem boundary used by file-oriented Win32 APIs.
pub trait VirtualFileSystem: Send + Sync {
    /// Open a guest-visible path without loading its complete contents.
    fn open_read(&self, path: &str) -> Result<Box<dyn VirtualReadFile>, Win32Error>;
    /// Read the complete contents of a guest-visible path.
    fn read(&self, path: &str) -> Result<Vec<u8>, Win32Error>;
    /// Replace a guest-visible file.
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), Win32Error>;
    /// Create one directory below the configured Guest root.
    fn create_directory(&self, path: &str) -> Result<(), Win32Error>;
    /// Remove one empty directory below the configured Guest root.
    fn remove_directory(&self, path: &str) -> Result<(), Win32Error>;
    /// Remove one file below the configured Guest root.
    fn remove_file(&self, path: &str) -> Result<(), Win32Error>;
    /// Query metadata for one existing guest-visible path.
    fn metadata(&self, path: &str) -> Result<FileMetadata, Win32Error>;
    /// Enumerate entries matching one guest-relative wildcard pattern.
    fn list(&self, pattern: &str) -> Result<Vec<FileEntry>, Win32Error>;
}

/// Metadata for one guest-visible filesystem path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadata {
    /// Exact byte size for regular files.
    pub size: u64,
    /// Whether the path refers to a directory.
    pub is_directory: bool,
}

/// Metadata returned by the VFS directory enumeration boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Final path component visible to the Guest.
    pub name: String,
    /// Exact byte size for regular files.
    pub size: u64,
    /// Whether this entry is a directory.
    pub is_directory: bool,
}

/// Filesystem implementation used until sandbox path mapping is configured.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedFileSystem;

impl VirtualFileSystem for UnsupportedFileSystem {
    fn open_read(&self, _path: &str) -> Result<Box<dyn VirtualReadFile>, Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem streaming reads",
        })
    }

    fn read(&self, _path: &str) -> Result<Vec<u8>, Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem reads",
        })
    }

    fn write(&self, _path: &str, _bytes: &[u8]) -> Result<(), Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem writes",
        })
    }

    fn create_directory(&self, _path: &str) -> Result<(), Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem directory creation",
        })
    }

    fn remove_directory(&self, _path: &str) -> Result<(), Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem directory removal",
        })
    }

    fn remove_file(&self, _path: &str) -> Result<(), Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem file removal",
        })
    }

    fn metadata(&self, _path: &str) -> Result<FileMetadata, Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem metadata",
        })
    }

    fn list(&self, _pattern: &str) -> Result<Vec<FileEntry>, Win32Error> {
        Err(Win32Error::Unsupported {
            feature: "virtual filesystem enumeration",
        })
    }
}

struct HostReadFile {
    file: File,
    length: u64,
    guest_path: String,
}

impl VirtualReadFile for HostReadFile {
    fn read(&mut self, length: usize) -> Result<Vec<u8>, Win32Error> {
        let mut bytes = vec![0; length];
        let read = self.file.read(&mut bytes).map_err(|error| Win32Error::Io {
            operation: "read",
            path: self.guest_path.clone(),
            message: error.to_string(),
        })?;
        bytes.truncate(read);
        Ok(bytes)
    }

    fn seek(&mut self, distance: i64, origin: u32) -> Result<u64, Win32Error> {
        let position = match origin {
            0 => u64::try_from(distance)
                .map(SeekFrom::Start)
                .map_err(|_| Win32Error::InvalidArgument("negative file seek"))?,
            1 => SeekFrom::Current(distance),
            2 => SeekFrom::End(distance),
            _ => return Err(Win32Error::InvalidArgument("invalid file seek origin")),
        };
        self.file.seek(position).map_err(|error| Win32Error::Io {
            operation: "seek",
            path: self.guest_path.clone(),
            message: error.to_string(),
        })
    }

    fn len(&self) -> u64 {
        self.length
    }
}

/// Host-backed filesystem constrained to one explicit root directory.
#[derive(Debug, Clone)]
pub struct SandboxFileSystem {
    root: PathBuf,
    guest_root: String,
}

impl SandboxFileSystem {
    /// Create a filesystem rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, guest_root: impl Into<String>) -> Self {
        let guest_root = guest_root.into().replace('\\', "/");
        Self {
            root: root.into(),
            guest_root: guest_root.trim_end_matches('/').to_ascii_lowercase(),
        }
    }

    fn resolve(&self, guest_path: &str) -> Result<PathBuf, Win32Error> {
        let normalized = self.relative_guest_path(guest_path)?;
        let mut relative = PathBuf::new();
        for component in Path::new(&normalized).components() {
            match component {
                Component::Normal(part) => relative.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(Win32Error::InvalidArgument(
                        "guest path escapes the filesystem root",
                    ));
                }
            }
        }
        if relative.as_os_str().is_empty() {
            return Err(Win32Error::InvalidArgument("empty guest path"));
        }
        Ok(self.root.join(relative))
    }

    fn relative_guest_path(&self, guest_path: &str) -> Result<String, Win32Error> {
        let normalized = guest_path.replace('\\', "/");
        let normalized = normalized
            .split('/')
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if guest_path.starts_with(['/', '\\']) {
            return Err(Win32Error::InvalidArgument("absolute guest path"));
        }
        if normalized.contains(':') {
            let lower = normalized.to_ascii_lowercase();
            if lower == self.guest_root {
                return Err(Win32Error::InvalidArgument("empty guest path"));
            }
            let prefix = format!("{}/", self.guest_root);
            return lower
                .starts_with(&prefix)
                .then(|| normalized[prefix.len()..].to_owned())
                .ok_or(Win32Error::InvalidArgument(
                    "drive-qualified guest path is outside the Guest root",
                ));
        }
        Ok(normalized)
    }

    fn resolve_existing(&self, guest_path: &str) -> Result<PathBuf, Win32Error> {
        let normalized = guest_path
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase();
        if normalized == self.guest_root {
            return Ok(self.root.clone());
        }
        let resolved = self.resolve(guest_path)?;
        let relative = resolved.strip_prefix(&self.root).map_err(|_| {
            Win32Error::InvalidArgument("resolved guest path is outside filesystem root")
        })?;
        let mut current = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(Win32Error::InvalidArgument(
                    "invalid resolved guest path component",
                ));
            };
            let exact = current.join(name);
            if exact.exists() {
                current = exact;
                continue;
            }
            let requested = name.to_string_lossy();
            let matched = fs::read_dir(&current)
                .ok()
                .and_then(|entries| {
                    entries.filter_map(Result::ok).find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&requested)
                    })
                })
                .map(|entry| entry.path())
                .ok_or_else(|| Win32Error::Io {
                    operation: "resolve",
                    path: guest_path.to_owned(),
                    message: "path does not exist".to_owned(),
                })?;
            current = matched;
        }
        Ok(current)
    }
}

impl VirtualFileSystem for SandboxFileSystem {
    fn open_read(&self, path: &str) -> Result<Box<dyn VirtualReadFile>, Win32Error> {
        let resolved = self.resolve_existing(path)?;
        let file = File::open(&resolved).map_err(|error| Win32Error::Io {
            operation: "open",
            path: path.to_owned(),
            message: error.to_string(),
        })?;
        let length = file
            .metadata()
            .map_err(|error| Win32Error::Io {
                operation: "metadata",
                path: path.to_owned(),
                message: error.to_string(),
            })?
            .len();
        Ok(Box::new(HostReadFile {
            file,
            length,
            guest_path: path.to_owned(),
        }))
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, Win32Error> {
        let resolved = self.resolve_existing(path)?;
        fs::read(&resolved).map_err(|error| Win32Error::Io {
            operation: "read",
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), Win32Error> {
        let resolved = self.resolve(path)?;
        fs::write(&resolved, bytes).map_err(|error| Win32Error::Io {
            operation: "write",
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    fn create_directory(&self, path: &str) -> Result<(), Win32Error> {
        let resolved = self.resolve(path)?;
        fs::create_dir(&resolved).map_err(|error| Win32Error::Io {
            operation: "create directory",
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    fn remove_directory(&self, path: &str) -> Result<(), Win32Error> {
        let resolved = self.resolve_existing(path)?;
        fs::remove_dir(&resolved).map_err(|error| Win32Error::Io {
            operation: "remove directory",
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    fn remove_file(&self, path: &str) -> Result<(), Win32Error> {
        let resolved = self.resolve_existing(path)?;
        fs::remove_file(&resolved).map_err(|error| Win32Error::Io {
            operation: "remove file",
            path: path.to_owned(),
            message: error.to_string(),
        })
    }

    fn metadata(&self, path: &str) -> Result<FileMetadata, Win32Error> {
        let resolved = self.resolve_existing(path)?;
        let metadata = fs::metadata(&resolved).map_err(|error| Win32Error::Io {
            operation: "metadata",
            path: path.to_owned(),
            message: error.to_string(),
        })?;
        Ok(FileMetadata {
            size: metadata.len(),
            is_directory: metadata.is_dir(),
        })
    }

    fn list(&self, pattern: &str) -> Result<Vec<FileEntry>, Win32Error> {
        let normalized = self.relative_guest_path(pattern)?;
        let (directory, wildcard) = normalized
            .rsplit_once('/')
            .map_or(("", normalized.as_str()), |(directory, wildcard)| {
                (directory, wildcard)
            });
        if wildcard.is_empty() {
            return Err(Win32Error::InvalidArgument("empty search wildcard"));
        }
        let resolved = if directory.is_empty() || directory == "." {
            self.root.clone()
        } else {
            self.resolve_existing(directory)?
        };
        let entries = fs::read_dir(&resolved).map_err(|error| Win32Error::Io {
            operation: "enumerate",
            path: pattern.to_owned(),
            message: error.to_string(),
        })?;
        let mut matches = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| Win32Error::Io {
                operation: "enumerate",
                path: pattern.to_owned(),
                message: error.to_string(),
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !wildcard_matches(wildcard, &name) {
                continue;
            }
            let metadata = entry.metadata().map_err(|error| Win32Error::Io {
                operation: "metadata",
                path: name.clone(),
                message: error.to_string(),
            })?;
            matches.push(FileEntry {
                name,
                size: metadata.len(),
                is_directory: metadata.is_dir(),
            });
        }
        matches.sort_by_key(|entry| entry.name.to_ascii_lowercase());
        Ok(matches)
    }
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    // Win32/DOS wildcard rules preserve `*.*` as the historical spelling for
    // "all directory entries", including names with no dot at all.
    if pattern.eq_ignore_ascii_case("*.*") {
        return true;
    }
    let pattern = pattern.to_ascii_lowercase().into_bytes();
    let value = value.to_ascii_lowercase().into_bytes();
    let (mut pattern_index, mut value_index) = (0, 0);
    let (mut star, mut retry_value) = (None, 0);
    while value_index < value.len() {
        if pattern
            .get(pattern_index)
            .is_some_and(|byte| *byte == b'?' || Some(byte) == value.get(value_index))
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern.get(pattern_index) == Some(&b'*') {
            star = Some(pattern_index);
            pattern_index += 1;
            retry_value = value_index;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            retry_value += 1;
            value_index = retry_value;
        } else {
            return false;
        }
    }
    while pattern.get(pattern_index) == Some(&b'*') {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

struct OpenFile {
    path: String,
    storage: OpenFileStorage,
    readable: bool,
    writable: bool,
    dirty: bool,
    directory: bool,
}

enum OpenFileStorage {
    Stream(Box<dyn VirtualReadFile>),
    Buffered { bytes: Vec<u8>, cursor: usize },
}

struct DirectorySearch {
    remaining: VecDeque<FileEntry>,
}

/// Per-process file handles and read cursors over the configured VFS.
pub struct ProcessIo {
    filesystem: Arc<dyn VirtualFileSystem>,
    files: HandleTable<OpenFile>,
    searches: HashMap<Handle, DirectorySearch>,
    next_search_handle: u32,
}

impl ProcessIo {
    /// Create process I/O using a sandboxed host directory.
    #[must_use]
    pub fn sandboxed(root: impl Into<PathBuf>, guest_root: impl Into<String>) -> Self {
        Self {
            filesystem: Arc::new(SandboxFileSystem::new(root, guest_root)),
            files: HandleTable::default(),
            searches: HashMap::new(),
            next_search_handle: 0x0004_0000,
        }
    }

    /// Open a guest path for sequential reading.
    pub fn open_read(&mut self, path: &str) -> Result<Handle, Win32Error> {
        self.open(path, true, false, 3).map(|(handle, _)| handle)
    }

    /// Open or create a Guest file with Win32 creation-disposition semantics.
    pub fn open(
        &mut self,
        path: &str,
        readable: bool,
        writable: bool,
        disposition: u32,
    ) -> Result<(Handle, bool), Win32Error> {
        let existed = self.filesystem.metadata(path).is_ok();
        let create_empty = match disposition {
            1 if existed => {
                return Err(Win32Error::Io {
                    operation: "create",
                    path: path.to_owned(),
                    message: "file already exists".to_owned(),
                });
            }
            1 | 2 => true, // CREATE_NEW / CREATE_ALWAYS
            3 if !existed => {
                return Err(Win32Error::Io {
                    operation: "open",
                    path: path.to_owned(),
                    message: "file does not exist".to_owned(),
                });
            }
            3 => false,                       // OPEN_EXISTING
            4 => !existed,                    // OPEN_ALWAYS
            5 if existed && writable => true, // TRUNCATE_EXISTING
            5 => {
                return Err(Win32Error::Io {
                    operation: "truncate",
                    path: path.to_owned(),
                    message: "file does not exist or is not writable".to_owned(),
                });
            }
            _ => {
                return Err(Win32Error::InvalidArgument(
                    "invalid file creation disposition",
                ));
            }
        };
        if create_empty {
            self.filesystem.write(path, &[])?;
        }
        let storage = if !writable && !create_empty {
            OpenFileStorage::Stream(self.filesystem.open_read(path)?)
        } else {
            let bytes = if create_empty {
                Vec::new()
            } else {
                self.filesystem.read(path)?
            };
            OpenFileStorage::Buffered { bytes, cursor: 0 }
        };
        let handle = self.files.insert(OpenFile {
            path: path.to_owned(),
            storage,
            readable,
            writable,
            dirty: false,
            directory: false,
        })?;
        Ok((handle, existed))
    }

    /// Open an existing Guest directory as a closeable kernel handle.
    pub fn open_directory(&mut self, path: &str) -> Result<Handle, Win32Error> {
        let metadata = self.filesystem.metadata(path)?;
        if !metadata.is_directory {
            return Err(Win32Error::Io {
                operation: "open directory",
                path: path.to_owned(),
                message: "path is not a directory".to_owned(),
            });
        }
        self.files.insert(OpenFile {
            path: path.to_owned(),
            storage: OpenFileStorage::Buffered {
                bytes: Vec::new(),
                cursor: 0,
            },
            readable: false,
            writable: false,
            dirty: false,
            directory: true,
        })
    }

    /// Read at most `length` bytes and advance the file cursor.
    pub fn read(&mut self, handle: Handle, length: usize) -> Result<Vec<u8>, Win32Error> {
        let file = self
            .files
            .get_mut(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if file.directory || !file.readable {
            return Err(Win32Error::InvalidHandle(handle.0));
        }
        match &mut file.storage {
            OpenFileStorage::Stream(stream) => stream.read(length),
            OpenFileStorage::Buffered { bytes, cursor } => {
                if *cursor >= bytes.len() {
                    return Ok(Vec::new());
                }
                let end = cursor.saturating_add(length).min(bytes.len());
                let output = bytes[*cursor..end].to_vec();
                *cursor = end;
                Ok(output)
            }
        }
    }

    /// Create one Guest directory within the configured filesystem root.
    pub fn create_directory(&self, path: &str) -> Result<(), Win32Error> {
        self.filesystem.create_directory(path)
    }

    /// Remove one empty Guest directory within the filesystem root.
    pub fn remove_directory(&self, path: &str) -> Result<(), Win32Error> {
        self.filesystem.remove_directory(path)
    }

    /// Remove one Guest file within the filesystem root.
    pub fn remove_file(&self, path: &str) -> Result<(), Win32Error> {
        self.filesystem.remove_file(path)
    }

    /// Return Win32 file attributes for one Guest path.
    pub fn file_attributes(&self, path: &str) -> Result<u32, Win32Error> {
        let metadata = self.filesystem.metadata(path)?;
        Ok(if metadata.is_directory { 0x10 } else { 0x20 })
    }

    /// Copy one Guest file within the configured filesystem root.
    pub fn copy_file(
        &self,
        source: &str,
        destination: &str,
        fail_if_exists: bool,
    ) -> Result<(), Win32Error> {
        if fail_if_exists && self.filesystem.metadata(destination).is_ok() {
            return Err(Win32Error::Io {
                operation: "copy",
                path: destination.to_owned(),
                message: "destination already exists".to_owned(),
            });
        }
        let bytes = self.filesystem.read(source)?;
        self.filesystem.write(destination, &bytes)
    }

    /// Close a file handle.
    pub fn close(&mut self, handle: Handle) -> Result<(), Win32Error> {
        let file = self
            .files
            .remove(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if file.dirty {
            let OpenFileStorage::Buffered { bytes, .. } = &file.storage else {
                return Err(Win32Error::InvalidHandle(handle.0));
            };
            self.filesystem.write(&file.path, bytes)?;
        }
        Ok(())
    }

    /// Write bytes at the current cursor and advance it.
    pub fn write(&mut self, handle: Handle, bytes: &[u8]) -> Result<usize, Win32Error> {
        let file = self
            .files
            .get_mut(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if file.directory || !file.writable {
            return Err(Win32Error::InvalidHandle(handle.0));
        }
        let OpenFileStorage::Buffered {
            bytes: data,
            cursor,
        } = &mut file.storage
        else {
            return Err(Win32Error::InvalidHandle(handle.0));
        };
        let end = cursor
            .checked_add(bytes.len())
            .ok_or(Win32Error::OutOfMemory)?;
        if data.len() < end {
            data.resize(end, 0);
        }
        data[*cursor..end].copy_from_slice(bytes);
        *cursor = end;
        file.dirty = true;
        Ok(bytes.len())
    }

    /// Resize a writable file to its current cursor position.
    pub fn set_end(&mut self, handle: Handle) -> Result<(), Win32Error> {
        let file = self
            .files
            .get_mut(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if !file.writable {
            return Err(Win32Error::InvalidHandle(handle.0));
        }
        let OpenFileStorage::Buffered { bytes, cursor } = &mut file.storage else {
            return Err(Win32Error::InvalidHandle(handle.0));
        };
        bytes.resize(*cursor, 0);
        file.dirty = true;
        Ok(())
    }

    /// Persist pending writes for one file handle.
    pub fn flush(&mut self, handle: Handle) -> Result<(), Win32Error> {
        let file = self
            .files
            .get_mut(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if file.dirty {
            let OpenFileStorage::Buffered { bytes, .. } = &file.storage else {
                return Err(Win32Error::InvalidHandle(handle.0));
            };
            self.filesystem.write(&file.path, bytes)?;
            file.dirty = false;
        }
        Ok(())
    }

    /// Start a wildcard directory search and return its first result.
    pub fn find_first(&mut self, pattern: &str) -> Result<(Handle, FileEntry), Win32Error> {
        debug!(pattern, "guest wildcard search requested");
        let mut entries = VecDeque::from(self.filesystem.list(pattern)?);
        debug!(pattern, matches = entries.len(), "guest wildcard search");
        let first = entries.pop_front().ok_or_else(|| Win32Error::Io {
            operation: "enumerate",
            path: pattern.to_owned(),
            message: "no matching files".to_owned(),
        })?;
        let handle = Handle(self.next_search_handle);
        self.next_search_handle = self
            .next_search_handle
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        self.searches
            .insert(handle, DirectorySearch { remaining: entries });
        Ok((handle, first))
    }

    /// Advance an existing wildcard directory search.
    pub fn find_next(&mut self, handle: Handle) -> Result<Option<FileEntry>, Win32Error> {
        self.searches
            .get_mut(&handle)
            .map(|search| search.remaining.pop_front())
            .ok_or(Win32Error::InvalidHandle(handle.0))
    }

    /// Close a wildcard directory search.
    pub fn close_search(&mut self, handle: Handle) -> Result<(), Win32Error> {
        self.searches
            .remove(&handle)
            .map(|_| ())
            .ok_or(Win32Error::InvalidHandle(handle.0))
    }

    /// Total byte length of an open file.
    pub fn file_size(&self, handle: Handle) -> Result<u64, Win32Error> {
        let file = self
            .files
            .get(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        match &file.storage {
            OpenFileStorage::Stream(stream) => Ok(stream.len()),
            OpenFileStorage::Buffered { bytes, .. } => u64::try_from(bytes.len())
                .map_err(|_| Win32Error::InvalidArgument("file size exceeds u64")),
        }
    }

    /// Move an open file cursor relative to start, current position, or end.
    pub fn seek(&mut self, handle: Handle, distance: i64, origin: u32) -> Result<u64, Win32Error> {
        let file = self
            .files
            .get_mut(handle)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        match &mut file.storage {
            OpenFileStorage::Stream(stream) => stream.seek(distance, origin),
            OpenFileStorage::Buffered { bytes, cursor } => {
                let base = match origin {
                    0 => 0,
                    1 => i64::try_from(*cursor)
                        .map_err(|_| Win32Error::InvalidArgument("file cursor exceeds i64"))?,
                    2 => i64::try_from(bytes.len())
                        .map_err(|_| Win32Error::InvalidArgument("file size exceeds i64"))?,
                    _ => return Err(Win32Error::InvalidArgument("invalid file seek origin")),
                };
                let position = base
                    .checked_add(distance)
                    .filter(|position| *position >= 0)
                    .ok_or(Win32Error::InvalidArgument(
                        "negative or overflowing file seek",
                    ))?;
                *cursor = usize::try_from(position)
                    .map_err(|_| Win32Error::InvalidArgument("file seek exceeds host usize"))?;
                Ok(position as u64)
            }
        }
    }

    /// Whether a handle refers to an open disk file.
    #[must_use]
    pub fn contains(&self, handle: Handle) -> bool {
        self.files.get(handle).is_some()
    }

    /// Number of currently open file handles.
    #[must_use]
    pub fn open_handle_count(&self) -> usize {
        self.files.len()
    }
}

/// Stable name of a host-implemented imported API.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiKey {
    /// Lowercase module name without path assumptions.
    pub module: String,
    /// Export name.
    pub name: String,
}

impl ApiKey {
    /// Construct and normalize an API key.
    #[must_use]
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into().to_ascii_lowercase(),
            name: name.into(),
        }
    }
}

/// Narrow machine interface exposed to Win32 host-call handlers.
pub trait HostCallContext {
    /// Read a 32-bit stdcall argument, excluding the return address.
    fn argument_u32(&self, index: usize) -> Result<u32, Win32Error>;
    /// Store the conventional 32-bit return value in EAX.
    fn set_return_u32(&mut self, value: u32);
    /// Declare how many argument bytes the stdcall callee removes on return.
    fn set_stdcall_cleanup(&mut self, argument_bytes: u32);
    /// Enter a Guest stdcall callback before the current Host call returns.
    fn request_guest_callback(
        &mut self,
        callback: GuestAddress,
        arguments: &[u32],
    ) -> Result<(), Win32Error>;
    /// Use the final queued Guest callback's EAX as the suspended Host return value.
    fn use_guest_callback_return_value(&mut self);
    /// Finish the suspended Host call after the active Guest callback returns.
    fn complete_suspended_host_call(&mut self, return_value: u32) -> Result<(), Win32Error>;
    /// Associate a Guest callback with an opaque Win32 object handle.
    fn register_guest_callback_target(&mut self, object: u32, callback: GuestAddress);
    /// Look up the Guest callback associated with an opaque Win32 object handle.
    fn guest_callback_target(&self, object: u32) -> Option<GuestAddress>;
    /// Replace the focused Guest window and return the previous handle.
    fn replace_focus_window(&mut self, window: u32) -> u32;
    /// Return the current focused/foreground Guest window.
    fn focused_window(&self) -> u32;
    /// Replace one 32-bit window-class attribute and return its previous value.
    fn replace_window_class_long(&mut self, window: u32, index: i32, value: u32) -> u32;
    /// Read one previously modeled 32-bit window-class attribute.
    fn window_class_long(&self, window: u32, index: i32) -> u32;
    /// Allocate an opaque Guest icon object.
    fn create_icon(&mut self) -> u32;
    /// Destroy a process-owned Guest icon object.
    fn destroy_icon(&mut self, icon: u32) -> bool;
    /// Register a named Guest window class and return its process-local atom.
    fn register_window_class(&mut self, name: &str, callback: GuestAddress) -> Option<u16>;
    /// Resolve a registered window class callback by name.
    fn window_class_callback_by_name(&self, name: &str) -> Option<GuestAddress>;
    /// Resolve a registered window class callback by atom.
    fn window_class_callback_by_atom(&self, atom: u16) -> Option<GuestAddress>;
    /// Resolve a registered window class name by atom.
    fn window_class_name_by_atom(&self, atom: u16) -> Option<String>;
    /// Allocate a Guest window record and return its opaque handle.
    fn create_window(&mut self, class_name: &str, title: &str, visible: bool) -> u32;
    /// Return the registered class name for a Guest window.
    fn window_class_name(&self, window: u32) -> Option<String>;
    /// Return the current title of a Guest window.
    fn window_title(&self, window: u32) -> Option<String>;
    /// Replace the title of a Guest window, returning whether it exists.
    fn set_window_title(&mut self, window: u32, title: &str) -> bool;
    /// Remove a Guest window record.
    fn remove_window(&mut self, window: u32) -> bool;
    /// Whether a Guest window handle is currently alive.
    fn is_window(&self, window: u32) -> bool;
    /// Query the modeled visibility of a Guest window.
    fn is_window_visible(&self, window: u32) -> bool;
    /// Change a Guest window's visibility and return its previous state.
    fn set_window_visible(&mut self, window: u32, visible: bool) -> bool;
    /// Store a 32-bit WINDOWPLACEMENT record for a Guest window.
    fn set_window_placement(&mut self, window: u32, placement: &[u8]) -> bool;
    /// Read the last 32-bit WINDOWPLACEMENT record for a Guest window.
    fn window_placement(&self, window: u32) -> Option<Vec<u8>>;
    /// Change a Guest window's enabled state and return its previous state.
    fn set_window_enabled(&mut self, window: u32, enabled: bool) -> bool;
    /// Query whether a Guest window is enabled.
    fn is_window_enabled(&self, window: u32) -> bool;
    /// Post one message to the initial Guest thread's queue.
    fn post_thread_message(&mut self, window: u32, message: u32, wparam: u32, lparam: u32);
    /// Peek or remove the first queued message matching the requested range.
    fn next_thread_message(
        &mut self,
        remove: bool,
        minimum: u32,
        maximum: u32,
    ) -> Option<(u32, u32, u32, u32)>;
    /// Return the logical primary display size exposed to the Guest.
    fn primary_display_size(&self) -> (u32, u32);
    /// Change the logical primary display mode.
    fn set_primary_display_size(&mut self, width: u32, height: u32);
    /// Allocate a process-owned Guest menu object.
    fn create_menu(&mut self) -> u32;
    /// Destroy a process-owned Guest menu object.
    fn destroy_menu(&mut self, menu: u32) -> bool;
    /// Whether a handle identifies a process-owned Guest menu.
    fn is_menu(&self, menu: u32) -> bool;
    /// Insert one submenu reference into a Guest menu.
    fn insert_submenu(&mut self, menu: u32, position: usize, submenu: u32) -> bool;
    /// Return a submenu reference by position.
    fn submenu(&self, menu: u32, position: usize) -> Option<u32>;
    /// Snapshot all live top-level Guest window handles.
    fn window_handles(&self) -> Vec<u32>;
    /// Return the logical screen-space cursor position.
    fn cursor_position(&self) -> (i32, i32);
    /// Change the logical screen-space cursor position.
    fn set_cursor_position(&mut self, x: i32, y: i32);
    /// Attach or clear a menu on a Guest window.
    fn set_window_menu(&mut self, window: u32, menu: u32) -> bool;
    /// Return the menu attached to a Guest window.
    fn window_menu(&self, window: u32) -> Option<u32>;
    /// Open or close the process clipboard model.
    fn set_clipboard_open(&mut self, open: bool) -> bool;
    /// Clear all modeled clipboard formats.
    fn clear_clipboard(&mut self);
    /// Read a clipboard data handle for one format.
    fn clipboard_data(&self, format: u32) -> Option<u32>;
    /// Store a clipboard data handle for one format.
    fn set_clipboard_data(&mut self, format: u32, handle: u32);
    /// Replace one 32-bit per-window attribute and return its previous value.
    fn replace_window_long(&mut self, window: u32, index: i32, value: u32) -> Option<u32>;
    /// Read one 32-bit per-window attribute.
    fn window_long(&self, window: u32, index: i32) -> Option<u32>;
    /// Mark a live Guest window as needing paint.
    fn invalidate_window(&mut self, window: u32) -> bool;
    /// Clear a live Guest window's pending paint state.
    fn validate_window(&mut self, window: u32) -> bool;
    /// Whether a live Guest window currently needs paint.
    fn window_needs_paint(&self, window: u32) -> bool;
    /// Return the stable display DC associated with a live Guest window.
    fn window_dc(&mut self, window: u32) -> Option<u32>;
    /// Whether a handle is one of the live Guest window display DCs.
    fn is_window_dc(&self, dc: u32) -> bool;
    /// Snapshot the Win32 256-byte keyboard state table.
    fn keyboard_state(&self) -> [u8; 256];
    /// Replace the Win32 256-byte keyboard state table.
    fn set_keyboard_state(&mut self, state: &[u8; 256]);
    /// Whether a handle identifies a screen, window, or memory display DC.
    fn is_gdi_dc(&self, dc: u32) -> bool;
    /// Allocate a memory DC compatible with an existing display DC.
    fn create_memory_dc(&mut self, source: u32) -> Option<u32>;
    /// Destroy a process-owned memory DC.
    fn delete_memory_dc(&mut self, dc: u32) -> bool;
    /// Select an opaque GDI object into a DC and return the previous selection.
    fn select_gdi_object(&mut self, dc: u32, object: u32) -> Option<u32>;
    /// Return the object currently selected into a DC.
    fn selected_gdi_object(&self, dc: u32) -> Option<u32>;
    /// Allocate a process-owned GDI object with Guest-visible descriptor bytes.
    fn create_gdi_object(&mut self, descriptor: &[u8]) -> u32;
    /// Read the descriptor of a process-owned GDI object.
    fn gdi_object(&self, object: u32) -> Option<Vec<u8>>;
    /// Destroy a process-owned GDI object.
    fn delete_gdi_object(&mut self, object: u32) -> bool;
    /// Replace one scalar DC attribute and return its previous/default value.
    fn replace_gdi_dc_attribute(
        &mut self,
        dc: u32,
        attribute: u32,
        value: u32,
        default: u32,
    ) -> Option<u32>;
    /// Snapshot a selected DIB from a source DC into a destination window DC.
    fn present_selected_bitmap(
        &mut self,
        destination: u32,
        source: u32,
    ) -> Result<bool, Win32Error>;
    /// Snapshot direct DIB pixels into a destination window DC.
    fn present_dib(
        &mut self,
        destination: u32,
        width: u32,
        height: i32,
        stride: u32,
        bits_per_pixel: u16,
        pixels: GuestAddress,
    ) -> Result<bool, Win32Error>;
    /// Human-readable name of the attached Host GPU adapter.
    fn graphics_adapter_name(&self) -> Option<&str>;
    /// Allocate a backend-neutral GPU texture.
    fn create_graphics_texture(
        &mut self,
        descriptor: TextureDescriptor,
    ) -> Result<TextureId, Win32Error>;
    /// Upload a complete tightly packed GPU texture.
    fn write_graphics_texture(
        &mut self,
        texture: TextureId,
        bytes: &[u8],
    ) -> Result<(), Win32Error>;
    /// Destroy a backend-neutral GPU texture.
    fn destroy_graphics_texture(&mut self, texture: TextureId) -> bool;
    /// Attach an owned region handle to a Guest window.
    fn set_window_region(&mut self, window: u32, region: u32);
    /// Read guest bytes for string and structure arguments.
    fn read_memory(&self, address: GuestAddress, output: &mut [u8]) -> Result<(), Win32Error>;
    /// Write guest output data.
    fn write_memory(&mut self, address: GuestAddress, bytes: &[u8]) -> Result<(), Win32Error>;
    /// Request clean process termination.
    fn request_exit(&mut self, code: u32);
    /// Raise software exception through Guest SEH after this Host call returns.
    fn raise_guest_exception(
        &mut self,
        code: u32,
        flags: u32,
        information: &[u32],
    ) -> Result<(), Win32Error>;
    /// Milliseconds elapsed on the runtime's monotonic process clock.
    fn tick_count(&self) -> u32;
    /// Nanosecond-frequency monotonic performance-counter value.
    fn performance_counter(&self) -> u64;
    /// Number of performance-counter ticks per second.
    fn performance_frequency(&self) -> u64;
    /// Current UTC time as 100-nanosecond intervals since 1601-01-01.
    fn system_time_filetime(&self) -> u64;
    /// Create a private process heap and return its opaque guest handle.
    fn create_heap(
        &mut self,
        initial_size: u32,
        maximum_size: u32,
        executable: bool,
    ) -> Result<Handle, Win32Error>;
    /// Destroy a private heap and all allocations it still owns.
    fn destroy_heap(&mut self, heap: Handle) -> Result<(), Win32Error>;
    /// Allocate zero-initialized, read/write memory owned by `heap`.
    fn allocate_heap_memory(&mut self, heap: Handle, size: u32)
    -> Result<GuestAddress, Win32Error>;
    /// Replace one heap allocation, preserving its existing prefix.
    fn reallocate_heap_memory(
        &mut self,
        heap: Handle,
        address: GuestAddress,
        size: u32,
    ) -> Result<GuestAddress, Win32Error>;
    /// Release an allocation owned by `heap`.
    fn free_heap_memory(&mut self, heap: Handle, address: GuestAddress) -> Result<(), Win32Error>;
    /// Return the requested byte size of an allocation owned by `heap`.
    fn heap_memory_size(&self, heap: Handle, address: GuestAddress) -> Result<u32, Win32Error>;
    /// Allocate and track one legacy global-memory object.
    fn allocate_global_memory(&mut self, size: u32) -> Result<Handle, Win32Error>;
    /// Lock a global-memory object and return its Guest pointer.
    fn lock_global_memory(&mut self, handle: Handle) -> Result<GuestAddress, Win32Error>;
    /// Unlock a global-memory object and return whether it remains locked.
    fn unlock_global_memory(&mut self, handle: Handle) -> Result<bool, Win32Error>;
    /// Free one global-memory object.
    fn free_global_memory(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Allocate one process-wide TLS index for the current thread's slot array.
    fn allocate_tls_index(&mut self) -> Result<u32, Win32Error>;
    /// Release a dynamically allocated TLS index.
    fn free_tls_index(&mut self, index: u32) -> Result<(), Win32Error>;
    /// Read the current thread's value for an allocated TLS index.
    fn tls_value(&self, index: u32) -> Result<u32, Win32Error>;
    /// Set the current thread's value for an allocated TLS index.
    fn set_tls_value(&mut self, index: u32, value: u32) -> Result<(), Win32Error>;
    /// Replace the process top-level exception filter and return its old pointer.
    fn replace_unhandled_exception_filter(&mut self, filter: u32) -> u32;
    /// Return the process top-level exception filter, or null when none is installed.
    fn unhandled_exception_filter(&self) -> GuestAddress;
    /// Enter the initial thread's simplified COM apartment and return HRESULT.
    fn initialize_com(&mut self) -> u32;
    /// Balance one successful COM apartment initialization.
    fn uninitialize_com(&mut self);
    /// Adjust and return the current thread's ShowCursor display count.
    fn adjust_cursor_display_count(&mut self, show: bool) -> i32;
    /// Preferred base of the main executable image.
    fn main_module_base(&self) -> GuestAddress;
    /// Mapped PE resource directory root and byte size, when present.
    fn resource_directory(&self) -> Option<(GuestAddress, u32)>;
    /// Allocate a committed virtual-memory region outside the process heap.
    fn allocate_virtual_memory(
        &mut self,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<GuestAddress, Win32Error>;
    /// Reserve an address range without committing backing Guest pages.
    fn reserve_virtual_memory(&mut self, size: u32) -> Result<GuestAddress, Win32Error>;
    /// Commit pages inside an existing virtual-memory reservation.
    fn commit_virtual_memory(
        &mut self,
        address: GuestAddress,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<(), Win32Error>;
    /// Change protection on mapped Guest pages and return the previous flags.
    fn protect_virtual_memory(
        &mut self,
        address: GuestAddress,
        size: u32,
        read: bool,
        write: bool,
        execute: bool,
    ) -> Result<(bool, bool, bool), Win32Error>;
    /// Test whether every page in a Guest range permits writes.
    fn is_memory_writable(&self, address: GuestAddress, size: u32) -> bool;
    /// Test whether every page in a Guest range permits reads.
    fn is_memory_readable(&self, address: GuestAddress, size: u32) -> bool;
    /// Test whether a Guest address is mapped executable code.
    fn is_memory_executable(&self, address: GuestAddress) -> bool;
    /// Release a complete virtual-memory allocation.
    fn free_virtual_memory(&mut self, address: GuestAddress) -> Result<(), Win32Error>;
    /// Look up a loaded Host module by normalized DLL name.
    fn loaded_module_handle(&self, name: &str) -> Option<GuestAddress>;
    /// Resolve a loaded Host module handle back to its normalized DLL name.
    fn loaded_module_name(&self, module: GuestAddress) -> Option<String>;
    /// Resolve a named export and return an executable Host thunk address.
    fn resolve_host_api(
        &mut self,
        module: GuestAddress,
        name: &str,
    ) -> Result<GuestAddress, Win32Error>;
    /// Persistent ANSI command-line buffer owned by the guest process.
    fn command_line_ansi(&self) -> GuestAddress;
    /// Persistent UTF-16 command-line buffer owned by the guest process.
    fn command_line_utf16(&self) -> GuestAddress;
    /// Win32-visible path of the main executable.
    fn main_module_path(&self) -> &str;
    /// Current thread's simplified Win32 last-error value.
    fn last_error(&self) -> u32;
    /// Replace the current thread's simplified Win32 last-error value.
    fn set_last_error(&mut self, value: u32);
    /// Replace the process error-mode flags and return the previous value.
    fn replace_process_error_mode(&mut self, mode: u32) -> u32;
    /// Open a VFS path for sequential reading.
    fn open_file_read(&mut self, path: &str) -> Result<Handle, Win32Error>;
    /// Open or create a VFS file and report whether it previously existed.
    fn open_file(
        &mut self,
        path: &str,
        readable: bool,
        writable: bool,
        disposition: u32,
    ) -> Result<(Handle, bool), Win32Error>;
    /// Open an existing VFS directory as a closeable kernel handle.
    fn open_directory_handle(&mut self, path: &str) -> Result<Handle, Win32Error>;
    /// Read bytes from an open file and advance its cursor.
    fn read_file(&mut self, handle: Handle, length: usize) -> Result<Vec<u8>, Win32Error>;
    /// Resize a writable file to its current cursor.
    fn set_end_of_file(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Persist pending writes for one file handle.
    fn flush_file(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Close an open file handle.
    fn close_file(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Create one directory inside the configured Guest filesystem root.
    fn create_directory(&mut self, path: &str) -> Result<(), Win32Error>;
    /// Remove one empty directory inside the Guest filesystem root.
    fn remove_directory(&mut self, path: &str) -> Result<(), Win32Error>;
    /// Remove one file inside the Guest filesystem root.
    fn remove_file(&mut self, path: &str) -> Result<(), Win32Error>;
    /// Return Win32 file-attribute flags for one guest-visible path.
    fn file_attributes(&self, path: &str) -> Result<u32, Win32Error>;
    /// Copy one guest-visible file within the filesystem sandbox.
    fn copy_file(
        &mut self,
        source: &str,
        destination: &str,
        fail_if_exists: bool,
    ) -> Result<(), Win32Error>;
    /// Close a file or synchronization-object handle.
    fn close_kernel_handle(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Open a token handle for the current process pseudo handle.
    fn open_process_token(
        &mut self,
        process: Handle,
        desired_access: u32,
    ) -> Result<Handle, Win32Error>;
    /// Whether a token handle is currently open.
    fn token_is_open(&self, token: Handle) -> bool;
    /// Create or reopen a named mutex, returning whether it already existed.
    fn create_mutex(
        &mut self,
        name: Option<&str>,
        initial_owner: bool,
    ) -> Result<(Handle, bool), Win32Error>;
    /// Release one recursive ownership level of a mutex.
    fn release_mutex(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Create or reopen a named event object.
    fn create_event(
        &mut self,
        name: Option<&str>,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<(Handle, bool), Win32Error>;
    /// Set or reset an event's signaled state.
    fn set_event_state(&mut self, handle: Handle, signaled: bool) -> Result<(), Win32Error>;
    /// Try to satisfy a wait immediately; `None` means valid but not signaled.
    fn try_wait_for_objects(
        &mut self,
        handles: &[Handle],
        wait_all: bool,
    ) -> Result<Option<u32>, Win32Error>;
    /// Park the current Guest thread on a blocking wait and schedule another.
    ///
    /// On success the cooperative scheduler has switched Guest contexts; the
    /// Host call must not complete the original stdcall return frame.
    fn park_wait_and_schedule(
        &mut self,
        handles: &[Handle],
        wait_all: bool,
        cleanup: u32,
    ) -> Result<(), Win32Error>;
    /// Create a cooperative Guest worker thread.
    fn create_guest_thread(
        &mut self,
        start_address: GuestAddress,
        parameter: u32,
        stack_size: u32,
        creation_flags: u32,
    ) -> Result<(Handle, u32), Win32Error>;
    /// Decrement a Guest thread's suspend count and return the previous count.
    fn resume_guest_thread(&mut self, handle: Handle) -> Result<u32, Win32Error>;
    /// Terminate the current Guest thread. The initial thread exits the process.
    fn exit_guest_thread(&mut self, exit_code: u32) -> Result<(), Win32Error>;
    /// Begin one wildcard filesystem search.
    fn find_first_file(&mut self, pattern: &str) -> Result<(Handle, FileEntry), Win32Error>;
    /// Advance one wildcard filesystem search.
    fn find_next_file(&mut self, handle: Handle) -> Result<Option<FileEntry>, Win32Error>;
    /// Close one wildcard filesystem search.
    fn close_file_search(&mut self, handle: Handle) -> Result<(), Win32Error>;
    /// Query an open file's total byte length.
    fn file_size(&self, handle: Handle) -> Result<u64, Win32Error>;
    /// Return a pseudo handle for stdin, stdout, or stderr selectors.
    fn standard_handle(&self, selector: i32) -> Option<Handle>;
    /// Replace stdin, stdout, or stderr and return whether the selector was valid.
    fn set_standard_handle(&mut self, selector: i32, handle: Handle) -> Result<bool, Win32Error>;
    /// Write bytes to a supported output handle.
    fn write_handle(&mut self, handle: Handle, bytes: &[u8]) -> Result<usize, Win32Error>;
    /// Move a disk-file cursor and return the absolute byte position.
    fn seek_file(&mut self, handle: Handle, distance: i64, origin: u32) -> Result<u64, Win32Error>;
    /// Return Win32 FILE_TYPE_* for a known handle.
    fn file_type(&self, handle: Handle) -> Option<u32>;
    /// Look up one case-insensitive process environment value.
    fn environment_variable(&self, name: &str) -> Option<&str>;
    /// Set or delete one case-insensitive process environment value.
    fn set_environment_variable(
        &mut self,
        name: &str,
        value: Option<&str>,
    ) -> Result<(), Win32Error>;
    /// Runtime-owned double-NUL ANSI environment block.
    fn environment_block_ansi(&self) -> GuestAddress;
    /// Runtime-owned double-NUL UTF-16 environment block.
    fn environment_block_utf16(&self) -> GuestAddress;
    /// Win32-visible current directory for relative path resolution.
    fn current_directory(&self) -> &str;
    /// Replace the Win32-visible current directory without changing the Host sandbox root.
    fn set_current_directory(&mut self, path: &str) -> Result<(), Win32Error>;
    /// Stable identifier of the single emulated process.
    fn current_process_id(&self) -> u32;
    /// Stable identifier of the initial emulated thread.
    fn current_thread_id(&self) -> u32;
}

/// Encode a NUL-terminated Windows Japanese ANSI string.
#[must_use]
pub fn encode_ansi_z(value: &str) -> Vec<u8> {
    let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(value);
    let mut bytes = encoded.into_owned();
    bytes.push(0);
    bytes
}

/// Encode a NUL-terminated UTF-16LE string.
#[must_use]
pub fn encode_utf16_z(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Read a Japanese Windows ANSI string from bounded guest memory.
///
/// `encoding_rs::SHIFT_JIS` follows the Windows-compatible mapping needed by
/// the initial Japanese visual-novel target set.
pub fn read_ansi_z(
    context: &dyn HostCallContext,
    address: GuestAddress,
) -> Result<String, Win32Error> {
    let mut bytes = Vec::new();
    for offset in 0..MAX_GUEST_STRING_BYTES {
        let offset = u32::try_from(offset)
            .map_err(|_| Win32Error::InvalidArgument("ANSI string offset overflow"))?;
        let current = address
            .0
            .checked_add(offset)
            .ok_or(Win32Error::InvalidArgument("ANSI string address overflow"))?;
        let mut byte = [0];
        context.read_memory(GuestAddress(current), &mut byte)?;
        if byte[0] == 0 {
            return Ok(decode_ansi(&bytes));
        }
        bytes.push(byte[0]);
    }
    Err(Win32Error::InvalidArgument(
        "unterminated ANSI guest string",
    ))
}

fn decode_ansi(bytes: &[u8]) -> String {
    // Win32 "A" APIs do not make arbitrary application byte strings a Rust
    // validity boundary. Without MB_ERR_INVALID_CHARS, Windows conversion
    // replaces malformed sequences and continues. This is especially
    // important for legacy games that mix locale-specific bytes in captions.
    encoding_rs::SHIFT_JIS
        .decode_without_bom_handling(bytes)
        .0
        .into_owned()
}

/// Read a bounded NUL-terminated UTF-16LE string from guest memory.
pub fn read_utf16_z(
    context: &dyn HostCallContext,
    address: GuestAddress,
) -> Result<String, Win32Error> {
    let mut units = Vec::new();
    for index in 0..MAX_GUEST_STRING_BYTES / 2 {
        let byte_offset = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(2))
            .ok_or(Win32Error::InvalidArgument("UTF-16 string offset overflow"))?;
        let current = address
            .0
            .checked_add(byte_offset)
            .ok_or(Win32Error::InvalidArgument(
                "UTF-16 string address overflow",
            ))?;
        let mut bytes = [0; 2];
        context.read_memory(GuestAddress(current), &mut bytes)?;
        let unit = u16::from_le_bytes(bytes);
        if unit == 0 {
            return String::from_utf16(&units)
                .map_err(|_| Win32Error::InvalidArgument("invalid UTF-16 guest string"));
        }
        units.push(unit);
    }
    Err(Win32Error::InvalidArgument(
        "unterminated UTF-16 guest string",
    ))
}

/// Object-safe implementation of one imported API.
pub trait HostCallHandler: Send + Sync {
    /// Execute the host call against the current guest machine state.
    fn invoke(&self, context: &mut dyn HostCallContext) -> Result<(), Win32Error>;
}

/// Registry mapping imported API names to host implementations.
#[derive(Default)]
pub struct ApiRegistry {
    handlers: HashMap<ApiKey, Arc<dyn HostCallHandler>>,
}

impl ApiRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or replace one API implementation.
    pub fn register<H>(&mut self, key: ApiKey, handler: H)
    where
        H: HostCallHandler + 'static,
    {
        self.handlers.insert(key, Arc::new(handler));
    }

    /// Resolve a handler. Cloning the `Arc` lets the runtime invoke it while
    /// mutably borrowing the machine context.
    #[must_use]
    pub fn resolve(&self, key: &ApiKey) -> Option<Arc<dyn HostCallHandler>> {
        self.handlers.get(key).cloned().or_else(|| {
            let undecorated = undecorate_stdcall(&key.name)?;
            self.handlers
                .get(&ApiKey::new(key.module.clone(), undecorated))
                .cloned()
        })
    }

    /// Number of registered names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Whether no APIs are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// Snapshot all normalized API keys registered in this process.
    #[must_use]
    pub fn registered_keys(&self) -> Vec<ApiKey> {
        self.handlers.keys().cloned().collect()
    }
}

fn undecorate_stdcall(name: &str) -> Option<&str> {
    let without_prefix = name.strip_prefix('_')?;
    let (base, argument_bytes) = without_prefix.rsplit_once('@')?;
    if base.is_empty()
        || argument_bytes.is_empty()
        || !argument_bytes.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(base)
}

/// A registered placeholder that fails explicitly when called.
#[derive(Debug, Clone)]
pub struct UnsupportedApi {
    feature: &'static str,
}

impl UnsupportedApi {
    /// Describe the missing implementation.
    #[must_use]
    pub const fn new(feature: &'static str) -> Self {
        Self { feature }
    }
}

impl HostCallHandler for UnsupportedApi {
    fn invoke(&self, _context: &mut dyn HostCallContext) -> Result<(), Win32Error> {
        Err(Win32Error::Unsupported {
            feature: self.feature,
        })
    }
}

/// Win32 state, registry, or host-call errors.
#[derive(Debug, Error)]
pub enum Win32Error {
    /// An imported API does not have a registered implementation.
    #[error("Win32 API is not registered: {module}!{name}")]
    ApiNotRegistered {
        /// Normalized DLL name.
        module: String,
        /// Imported export name.
        name: String,
    },
    /// A known facility is beyond the current project milestone.
    #[error("unsupported Win32 feature: {feature}")]
    Unsupported {
        /// Stable description of the missing facility.
        feature: &'static str,
    },
    /// Handle allocation wrapped the 32-bit namespace.
    #[error("Win32 handle table exhausted")]
    HandleExhausted,
    /// Guest memory could not satisfy an API access.
    #[error("Win32 guest memory access failed: {0}")]
    GuestMemory(String),
    /// An API received an invalid guest value.
    #[error("invalid Win32 argument: {0}")]
    InvalidArgument(&'static str),
    /// A guest allocation could not be satisfied.
    #[error("Win32 guest allocation failed: out of memory")]
    OutOfMemory,
    /// A guest address is not owned by the requested allocator.
    #[error("Win32 allocation address is invalid: {address:#010x}")]
    InvalidAllocation {
        /// Guest pointer supplied by the API caller.
        address: u32,
    },
    /// A requested Host-provided DLL is not loaded.
    #[error("Win32 module is not loaded: {0}")]
    ModuleNotFound(String),
    /// A requested Host API is not registered.
    #[error("Win32 export is not available: {module}!{name}")]
    ProcedureNotFound {
        /// Normalized DLL name.
        module: String,
        /// Requested export name.
        name: String,
    },
    /// An opaque handle was not present in the relevant table.
    #[error("invalid Win32 handle: {0:#010x}")]
    InvalidHandle(u32),
    /// Host filesystem operation failed within the sandbox.
    #[error("Win32 filesystem {operation} failed for {path}: {message}")]
    Io {
        /// Operation name.
        operation: &'static str,
        /// Guest-visible path.
        path: String,
        /// Host error text.
        message: String,
    },
    /// The selected Host GPU backend rejected an operation.
    #[error("Win32 graphics backend failed: {0}")]
    Graphics(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_table_owns_values() {
        let mut table = HandleTable::default();
        let handle = table.insert("file").expect("handle should allocate");
        assert_eq!(table.get(handle), Some(&"file"));
        assert_eq!(table.remove(handle), Some("file"));
    }

    #[test]
    fn api_keys_normalize_module_case() {
        assert_eq!(
            ApiKey::new("KERNEL32.DLL", "ExitProcess").module,
            "kernel32.dll"
        );
    }

    #[test]
    fn registry_resolves_i686_stdcall_decoration() {
        let mut registry = ApiRegistry::new();
        registry.register(
            ApiKey::new("kernel32.dll", "HeapAlloc"),
            UnsupportedApi::new("test handler"),
        );
        assert!(
            registry
                .resolve(&ApiKey::new("KERNEL32.dll", "_HeapAlloc@12"))
                .is_some()
        );
    }

    #[test]
    fn ansi_decoding_replaces_malformed_sequences() {
        assert_eq!(decode_ansi(&[0x82]), "\u{fffd}");
    }

    #[test]
    fn sandbox_maps_only_its_configured_guest_root() {
        let filesystem = SandboxFileSystem::new("/tmp/vnrt-sandbox-test", r"C:\VNRT");
        assert!(filesystem.resolve("../secret.txt").is_err());
        assert_eq!(
            filesystem.resolve(r"C:\VNRT\\assets\script.dat").unwrap(),
            PathBuf::from("/tmp/vnrt-sandbox-test/assets/script.dat")
        );
        assert!(filesystem.resolve("D:\\secret.txt").is_err());
        assert!(filesystem.resolve("C:\\Windows\\secret.txt").is_err());
        assert!(filesystem.resolve("/etc/passwd").is_err());
        assert!(filesystem.resolve("assets\\script.dat").is_ok());
    }

    #[test]
    fn wildcard_matching_is_case_insensitive() {
        assert!(wildcard_matches("*.ypf", "CG.YPF"));
        assert!(wildcard_matches("update?.ypf", "update3.ypf"));
        assert!(wildcard_matches("*.*", "pac"));
        assert!(!wildcard_matches("*.ypf", "cg.ypf_old"));
    }

    #[test]
    fn writable_file_handles_flush_and_truncate_at_the_cursor() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("vnrt-process-io-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mut io = ProcessIo::sandboxed(&root, r"C:\GAME");

        let (handle, existed) = io.open("save.dat", true, true, 2).unwrap();
        assert!(!existed);
        assert_eq!(io.write(handle, b"abcdef").unwrap(), 6);
        assert_eq!(io.seek(handle, 3, 0).unwrap(), 3);
        io.set_end(handle).unwrap();
        io.flush(handle).unwrap();
        assert_eq!(fs::read(root.join("save.dat")).unwrap(), b"abc");
        io.close(handle).unwrap();

        let handle = io.open_read("SAVE.DAT").unwrap();
        assert_eq!(io.read(handle, 8).unwrap(), b"abc");
        assert_eq!(io.seek(handle, 1, 0).unwrap(), 1);
        assert_eq!(io.read(handle, 1).unwrap(), b"b");
        io.close(handle).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sandbox_opens_its_guest_root_as_a_directory_handle() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("vnrt-directory-io-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let mut io = ProcessIo::sandboxed(&root, r"C:\GAME");

        let handle = io.open_directory(r"C:\GAME").unwrap();
        assert!(io.contains(handle));
        assert!(io.read(handle, 1).is_err());
        io.close(handle).unwrap();

        fs::remove_dir_all(root).unwrap();
    }
}
