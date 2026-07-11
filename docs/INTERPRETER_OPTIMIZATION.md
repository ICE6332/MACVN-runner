# Interpreter optimization

VNRT keeps `iced-x86` as its decoder. The evaluated replacements either require a full execution-layer rewrite, provide another non-JIT interpreter, introduce GPL licensing, or are not mature cross-architecture backends for this runtime.

## Current safe fast path

- Guest memory uses a sparse two-level 32-bit page table instead of a `BTreeMap` lookup per access.
- Every mapped page carries a monotonically assigned generation.
- Writes, protection changes, unmap/remap cycles, and loader initialization invalidate decoded instructions through that generation.
- The x86 interpreter caches decoded instructions by EIP and the generation of every code page they span.
- Self-modifying code is covered by a regression test.
- Runtime batches consecutive CPU steps when instruction-level tracing is disabled.
- Single-page `u8`, `u16`, and `u32` accesses bypass the generic cross-page walker.

On the local Chinese-launcher checkpoint, 20 million interpreted steps fell from roughly 3.0 seconds of CPU time to roughly 1.5 seconds. The 350-million-step target run reaches the same NTDLL-resolution boundary in about 15 seconds.

## Unsafe assessment

No unsafe code is needed for the current twofold improvement. A future direct-pointer path is allowed only after the safe block cache is exhausted as an optimization source.

Any unsafe memory path must keep these invariants local and documented next to the unsafe block:

1. The pointed-to page is boxed and cannot move during the borrow.
2. No pointer survives a GuestMemory mutation, unmap, remap, or permission change.
3. Cross-page accesses always return to the checked safe path.
4. Decode and translated-block caches validate every spanned page generation.
5. Executable writes invalidate code before the next Guest instruction.

The next optimization stage is a decoded basic-block cache. A JIT backend remains a later option after block boundaries, flags, memory exits, and invalidation behavior are represented explicitly.
