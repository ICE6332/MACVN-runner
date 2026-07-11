use super::*;

pub(super) struct GuestMutex {
    name: Option<String>,
    owner: Option<u32>,
    recursion: u32,
    references: u32,
}

pub(super) struct GuestEvent {
    name: Option<String>,
    manual_reset: bool,
    signaled: bool,
    references: u32,
}

pub(super) struct EventManager {
    next_handle: u32,
    objects: BTreeMap<u32, GuestEvent>,
    names: HashMap<String, u32>,
}

impl EventManager {
    pub(super) fn new() -> Self {
        Self {
            next_handle: 0x0005_0000,
            objects: BTreeMap::new(),
            names: HashMap::new(),
        }
    }

    pub(super) fn create(
        &mut self,
        name: Option<&str>,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<(Handle, bool), Win32Error> {
        if let Some(handle) = name.and_then(|name| self.names.get(name)).copied() {
            let event = self
                .objects
                .get_mut(&handle)
                .ok_or(Win32Error::InvalidHandle(handle))?;
            event.references = event
                .references
                .checked_add(1)
                .ok_or(Win32Error::HandleExhausted)?;
            return Ok((Handle(handle), true));
        }
        let handle = self.next_handle;
        self.next_handle = self
            .next_handle
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        let name = name.map(str::to_owned);
        if let Some(name) = &name {
            self.names.insert(name.clone(), handle);
        }
        self.objects.insert(
            handle,
            GuestEvent {
                name,
                manual_reset,
                signaled: initial_state,
                references: 1,
            },
        );
        Ok((Handle(handle), false))
    }

    pub(super) fn set_state(&mut self, handle: Handle, signaled: bool) -> Result<(), Win32Error> {
        self.objects
            .get_mut(&handle.0)
            .map(|event| event.signaled = signaled)
            .ok_or(Win32Error::InvalidHandle(handle.0))
    }

    pub(super) fn is_signaled(&self, handle: Handle) -> Option<bool> {
        self.objects.get(&handle.0).map(|event| event.signaled)
    }

    pub(super) fn consume(&mut self, handle: Handle) -> Option<()> {
        let event = self.objects.get_mut(&handle.0)?;
        if event.signaled && !event.manual_reset {
            event.signaled = false;
        }
        Some(())
    }

    pub(super) fn close(&mut self, handle: Handle) -> Result<(), Win32Error> {
        let event = self
            .objects
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        event.references -= 1;
        if event.references != 0 {
            return Ok(());
        }
        let name = event.name.clone();
        self.objects.remove(&handle.0);
        if let Some(name) = name {
            self.names.remove(&name);
        }
        Ok(())
    }
}

pub(super) struct MutexManager {
    next_handle: u32,
    objects: BTreeMap<u32, GuestMutex>,
    names: HashMap<String, u32>,
}

impl MutexManager {
    pub(super) fn new() -> Self {
        Self {
            next_handle: 0x0002_0000,
            objects: BTreeMap::new(),
            names: HashMap::new(),
        }
    }

    pub(super) fn create(
        &mut self,
        name: Option<&str>,
        initial_owner: bool,
        thread_id: u32,
    ) -> Result<(Handle, bool), Win32Error> {
        if let Some(handle) = name.and_then(|name| self.names.get(name)).copied() {
            let object = self
                .objects
                .get_mut(&handle)
                .ok_or(Win32Error::InvalidHandle(handle))?;
            object.references = object
                .references
                .checked_add(1)
                .ok_or(Win32Error::HandleExhausted)?;
            return Ok((Handle(handle), true));
        }
        let handle = self.next_handle;
        self.next_handle = self
            .next_handle
            .checked_add(4)
            .ok_or(Win32Error::HandleExhausted)?;
        let name = name.map(str::to_owned);
        if let Some(name) = &name {
            self.names.insert(name.clone(), handle);
        }
        self.objects.insert(
            handle,
            GuestMutex {
                name,
                owner: initial_owner.then_some(thread_id),
                recursion: u32::from(initial_owner),
                references: 1,
            },
        );
        Ok((Handle(handle), false))
    }

    pub(super) fn release(&mut self, handle: Handle, thread_id: u32) -> Result<(), Win32Error> {
        let object = self
            .objects
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        if object.owner != Some(thread_id) || object.recursion == 0 {
            return Err(Win32Error::InvalidArgument(
                "mutex is not owned by current thread",
            ));
        }
        object.recursion -= 1;
        if object.recursion == 0 {
            object.owner = None;
        }
        Ok(())
    }

    pub(super) fn is_available(&self, handle: Handle, thread_id: u32) -> Option<bool> {
        self.objects
            .get(&handle.0)
            .map(|mutex| mutex.owner.is_none() || mutex.owner == Some(thread_id))
    }

    pub(super) fn acquire(&mut self, handle: Handle, thread_id: u32) -> Option<()> {
        let mutex = self.objects.get_mut(&handle.0)?;
        if mutex.owner.is_some() && mutex.owner != Some(thread_id) {
            return Some(());
        }
        mutex.owner = Some(thread_id);
        mutex.recursion = mutex.recursion.saturating_add(1);
        Some(())
    }

    pub(super) fn close(&mut self, handle: Handle) -> Result<(), Win32Error> {
        let object = self
            .objects
            .get_mut(&handle.0)
            .ok_or(Win32Error::InvalidHandle(handle.0))?;
        object.references -= 1;
        if object.references != 0 {
            return Ok(());
        }
        let name = object.name.clone();
        self.objects.remove(&handle.0);
        if let Some(name) = name {
            self.names.remove(&name);
        }
        Ok(())
    }
}

pub(super) struct TlsSlotManager {
    allocated: Vec<bool>,
    static_slot_count: usize,
}

impl TlsSlotManager {
    pub(super) fn new(has_static_tls: bool) -> Self {
        const SLOT_COUNT: usize = PAGE_SIZE_U32 as usize / 4;
        let static_slot_count = usize::from(has_static_tls);
        let mut allocated = vec![false; SLOT_COUNT];
        if has_static_tls {
            allocated[0] = true;
        }
        Self {
            allocated,
            static_slot_count,
        }
    }

    pub(super) fn allocate(&mut self, memory: &mut GuestMemory) -> Result<u32, Win32Error> {
        let index = self
            .allocated
            .iter()
            .position(|allocated| !allocated)
            .ok_or(Win32Error::OutOfMemory)?;
        self.allocated[index] = true;
        let index = u32::try_from(index).map_err(|_| Win32Error::OutOfMemory)?;
        memory
            .write_u32(self.address(index)?, 0)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))?;
        Ok(index)
    }

    pub(super) fn free(&mut self, memory: &mut GuestMemory, index: u32) -> Result<(), Win32Error> {
        let host_index = self.validate_dynamic(index)?;
        self.allocated[host_index] = false;
        memory
            .write_u32(self.address(index)?, 0)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    pub(super) fn get(&self, memory: &GuestMemory, index: u32) -> Result<u32, Win32Error> {
        self.validate(index)?;
        memory
            .read_u32(self.address(index)?)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    pub(super) fn set(
        &self,
        memory: &mut GuestMemory,
        index: u32,
        value: u32,
    ) -> Result<(), Win32Error> {
        self.validate(index)?;
        memory
            .write_u32(self.address(index)?, value)
            .map_err(|error| Win32Error::GuestMemory(error.to_string()))
    }

    pub(super) fn validate(&self, index: u32) -> Result<usize, Win32Error> {
        let index = usize::try_from(index).map_err(|_| Win32Error::InvalidArgument("TLS index"))?;
        if !self.allocated.get(index).copied().unwrap_or(false) {
            return Err(Win32Error::InvalidArgument("unallocated TLS index"));
        }
        Ok(index)
    }

    pub(super) fn validate_dynamic(&self, index: u32) -> Result<usize, Win32Error> {
        let index = self.validate(index)?;
        if index < self.static_slot_count {
            return Err(Win32Error::InvalidArgument("static TLS index"));
        }
        Ok(index)
    }

    pub(super) fn address(&self, index: u32) -> Result<GuestAddress, Win32Error> {
        index
            .checked_mul(4)
            .and_then(|offset| GUEST_TLS_BASE.checked_add(offset))
            .map(GuestAddress)
            .ok_or(Win32Error::InvalidArgument("TLS slot address overflow"))
    }
}
