# Interpreter optimization

VNRT keeps `iced-x86` as its decoder. The evaluated replacements either require a full execution-layer rewrite, provide another non-JIT interpreter, introduce GPL licensing, or are not mature cross-architecture backends for this runtime.

## Current safe fast path

- Guest memory uses a sparse two-level 32-bit page table instead of a `BTreeMap` lookup per access.
- Every mapped page carries a monotonically assigned generation.
- Writes, protection changes, unmap/remap cycles, and loader initialization invalidate decoded instructions through that generation.
- The x86 interpreter caches decoded instructions by EIP and the generation of every code page they span.
- Self-modifying code is covered by a regression test.
- Runtime batches consecutive CPU steps when instruction-level tracing is disabled.
- Decoded basic blocks execute up to 64 straight-line instructions after one page-generation validation.
- A process-wide executable epoch makes per-instruction block validation a
  scalar comparison. Full page-generation validation runs only after an
  executable mapping, protection change, unmap, or write; ordinary archive
  output writes do not invalidate code.
- Single-page `u8`, `u16`, and `u32` accesses bypass the generic cross-page walker.
- Release builds use thin LTO with one codegen unit so the interpreter's
  cross-crate memory fast paths can inline.

On the local Chinese-launcher checkpoint, 20 million interpreted steps fell from roughly 3.0 seconds of CPU time to roughly 1.0 second. The 350-million-step target run reaches the same NTDLL-resolution boundary in roughly 9 seconds of CPU time; observed wall time varies with host scheduling.

## Unsafe assessment

No unsafe code is needed for the current threefold microbenchmark improvement. A future direct-pointer path is allowed only after the safe block cache is exhausted as an optimization source.

Any unsafe memory path must keep these invariants local and documented next to the unsafe block:

1. The pointed-to page is boxed and cannot move during the borrow.
2. No pointer survives a GuestMemory mutation, unmap, remap, or permission change.
3. Cross-page accesses always return to the checked safe path.
4. Decode and translated-block caches validate every spanned page generation.
5. Executable writes invalidate code before the next Guest instruction.

The target now justifies returning to performance work: after the compatibility
layer exposes the `pac` directory, a one-billion-instruction run reaches its
limit while indexing/decrypting the real YPF archives. Profile this loop before
choosing between wider safe block execution, a compact translated-block IR, or
a narrowly scoped direct-pointer path. A JIT remains a later option rather than
the default next step.

On the current Apple M2 Pro checkpoint, adding thin LTO reduced the same
one-billion-instruction target run from 66.45 to 59.87 seconds wall time. A
five-second sample after the executable-epoch change reduced
`block_is_valid` top-of-stack samples from 246 to 7; the next dominant safe
targets are operand dispatch and repeated read-only archive opens.
