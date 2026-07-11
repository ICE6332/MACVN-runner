use super::*;

#[derive(Debug, Default)]
pub(super) struct GuestHeap {
    maximum_size: Option<u32>,
    allocations: BTreeMap<u32, u32>,
    executable: bool,
}

pub(super) struct GuestHeapManager {
    next_handle: u32,
    heaps: BTreeMap<u32, GuestHeap>,
    arena: GuestHeapArena,
}

pub(super) struct GuestHeapArena {
    cursor: u32,
    mapped_end: u32,
}

impl GuestHeapArena {
    pub(super) fn new() -> Self {
        Self {
            // Leave mapped header space before the first user allocation, as a
            // real NT heap does for its block metadata.
            cursor: GUEST_HEAP_BASE + 16,
            mapped_end: GUEST_HEAP_BASE,
        }
    }

    pub(super) fn allocate(
        &mut self,
        memory: &mut GuestMemory,
        requested_size: u32,
        executable: bool,
    ) -> Result<GuestAddress, Win32Error> {
        let allocation_size = align_up(requested_size.max(1), 16).ok_or(Win32Error::OutOfMemory)?;
        let address = GuestAddress(self.cursor);
        let end = self
            .cursor
            .checked_add(allocation_size)
            .filter(|end| *end <= GUEST_HEAP_LIMIT)
            .ok_or(Win32Error::OutOfMemory)?;
        let required_end = align_up(end, PAGE_SIZE_U32).ok_or(Win32Error::OutOfMemory)?;
        if required_end > self.mapped_end {
            let map_start = GuestAddress(self.mapped_end);
            let map_size = required_end - self.mapped_end;
            if !memory
                .is_range_free(map_start, map_size)
                .map_err(|error| Win32Error::GuestMemory(error.to_string()))?
            {
                return Err(Win32Error::OutOfMemory);
            }
            memory
                .map_range(
                    map_start,
                    map_size,
                    if executable {
                        Permissions::ALL
                    } else {
                        Permissions::READ_WRITE
                    },
                )
                .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
            self.mapped_end = required_end;
        }
        if executable {
            let page_start = GuestAddress(address.0 & !(PAGE_SIZE_U32 - 1));
            memory
                .protect_range(page_start, required_end - page_start.0, Permissions::ALL)
                .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        }
        self.cursor = end;
        Ok(address)
    }
}

impl GuestHeapManager {
    pub(super) fn new() -> Self {
        Self {
            next_handle: PROCESS_HEAP_HANDLE + 4,
            heaps: BTreeMap::from([(PROCESS_HEAP_HANDLE, GuestHeap::default())]),
            arena: GuestHeapArena::new(),
        }
    }

    pub(super) fn create(
        &mut self,
        initial_size: u32,
        maximum_size: u32,
        executable: bool,
    ) -> Result<Handle, Win32Error> {
        if maximum_size != 0 && initial_size > maximum_size {
            return Err(Win32Error::InvalidArgument(
                "HeapCreate initial size exceeds maximum size",
            ));
        }
        let handle = self.next_handle;
        self.next_handle = self
            .next_handle
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        self.heaps.insert(
            handle,
            GuestHeap {
                maximum_size: (maximum_size != 0).then_some(maximum_size),
                allocations: BTreeMap::new(),
                executable,
            },
        );
        Ok(Handle(handle))
    }

    pub(super) fn destroy(
        &mut self,
        _memory: &mut GuestMemory,
        heap: Handle,
    ) -> Result<(), Win32Error> {
        if heap.0 == PROCESS_HEAP_HANDLE {
            return Err(Win32Error::InvalidHandle(heap.0));
        }
        self.heaps
            .get(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?;
        self.heaps.remove(&heap.0);
        Ok(())
    }

    pub(super) fn allocate(
        &mut self,
        memory: &mut GuestMemory,
        heap: Handle,
        size: u32,
    ) -> Result<GuestAddress, Win32Error> {
        self.ensure_capacity(heap, 0, size)?;
        let executable = self
            .heaps
            .get(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?
            .executable;
        let address = self.arena.allocate(memory, size, executable)?;
        self.heaps
            .get_mut(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?
            .allocations
            .insert(address.0, size);
        Ok(address)
    }

    pub(super) fn reallocate(
        &mut self,
        memory: &mut GuestMemory,
        heap: Handle,
        address: GuestAddress,
        size: u32,
    ) -> Result<GuestAddress, Win32Error> {
        let old_size = self.size(heap, address)?;
        self.ensure_capacity(heap, old_size, size)?;
        let executable = self
            .heaps
            .get(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?
            .executable;
        let replacement = self.arena.allocate(memory, size, executable)?;
        let copy_length =
            usize::try_from(old_size.min(size)).map_err(|_| Win32Error::OutOfMemory)?;
        let mut bytes = vec![0; copy_length];
        if let Err(error) = memory
            .read(address, &mut bytes)
            .and_then(|()| memory.write(replacement, &bytes))
        {
            return Err(Win32Error::GuestMemory(error.to_string()));
        }
        let allocations = &mut self
            .heaps
            .get_mut(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?
            .allocations;
        allocations.remove(&address.0);
        allocations.insert(replacement.0, size);
        Ok(replacement)
    }

    pub(super) fn free(
        &mut self,
        _memory: &mut GuestMemory,
        heap: Handle,
        address: GuestAddress,
    ) -> Result<(), Win32Error> {
        self.size(heap, address)?;
        self.heaps
            .get_mut(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?
            .allocations
            .remove(&address.0);
        Ok(())
    }

    pub(super) fn size(&self, heap: Handle, address: GuestAddress) -> Result<u32, Win32Error> {
        self.heaps
            .get(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?
            .allocations
            .get(&address.0)
            .copied()
            .ok_or(Win32Error::InvalidAllocation { address: address.0 })
    }

    pub(super) fn ensure_capacity(
        &self,
        heap: Handle,
        replaced_size: u32,
        requested_size: u32,
    ) -> Result<(), Win32Error> {
        let heap_state = self
            .heaps
            .get(&heap.0)
            .ok_or(Win32Error::InvalidHandle(heap.0))?;
        let Some(maximum_size) = heap_state.maximum_size else {
            return Ok(());
        };
        let live_size = heap_state
            .allocations
            .values()
            .try_fold(0_u32, |total, size| total.checked_add(*size))
            .ok_or(Win32Error::OutOfMemory)?;
        let updated_size = live_size
            .checked_sub(replaced_size)
            .and_then(|size| size.checked_add(requested_size))
            .ok_or(Win32Error::OutOfMemory)?;
        if updated_size > maximum_size {
            return Err(Win32Error::OutOfMemory);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn live_allocation_count(&self) -> usize {
        self.heaps.values().map(|heap| heap.allocations.len()).sum()
    }
}

pub(super) struct GuestRegionAllocator {
    next: u32,
    limit: u32,
    allocations: BTreeMap<u32, u32>,
}

impl GuestRegionAllocator {
    pub(super) fn new(base: u32, limit: u32) -> Self {
        Self {
            next: base,
            limit,
            allocations: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.allocations.is_empty()
    }

    pub(super) fn allocate(
        &mut self,
        memory: &mut GuestMemory,
        requested_size: u32,
        permissions: Permissions,
    ) -> Result<GuestAddress, Win32Error> {
        let mapped_size =
            align_up(requested_size.max(1), PAGE_SIZE_U32).ok_or(Win32Error::OutOfMemory)?;
        let end = self
            .next
            .checked_add(mapped_size)
            .filter(|end| *end <= self.limit)
            .ok_or(Win32Error::OutOfMemory)?;
        let address = GuestAddress(self.next);
        if !memory
            .is_range_free(address, mapped_size)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?
        {
            return Err(Win32Error::OutOfMemory);
        }
        memory
            .map_range(address, mapped_size, permissions)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        self.allocations.insert(address.0, mapped_size);
        self.next = end;
        Ok(address)
    }

    pub(super) fn free(
        &mut self,
        memory: &mut GuestMemory,
        address: GuestAddress,
    ) -> Result<(), Win32Error> {
        let mapped_size = self
            .allocations
            .get(&address.0)
            .copied()
            .ok_or(Win32Error::InvalidAllocation { address: address.0 })?;
        memory
            .unmap_range(address, mapped_size)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        self.allocations.remove(&address.0);
        Ok(())
    }
}
