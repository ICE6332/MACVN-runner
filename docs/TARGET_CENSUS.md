# Target capability census

`vnrt-inspect --census` decodes every executable PE section before a deep
runtime run. It reports:

- every distinct x86 operand form and its static frequency;
- x87 forms not covered by the current target-driven interpreter subset;
- indirect calls through small vtable offsets, annotated with possible D3D9
  interface methods.

The D3D annotations are candidates, not a data-flow proof: the executable can
contain other COM objects with the same offsets. Dynamic execution remains the
authority for deciding which methods to implement.

For the local YU-RIS comparison image:

```bash
cargo run -p vnrt-inspect -- \
  tests/targets/euphoria/inspect/euphoriaHD.exe --census
```

The first census decoded about 486,000 instructions. Batch implementation of
the target-relevant register loads, rounding, constants, integer widths, and
power sequence reduced the static x87 gap from 59 forms to 28. The remaining
set is dominated by 80-bit extended precision, trigonometry, and FPU-state
save/restore code that may belong to linked runtime libraries. Those forms are
implemented only after the dynamic path proves they execute.

The D3D candidates confirm a compact fixed-function 2D path centered on
`CreateDevice`, `CreateTexture`, `LockRect`, `SetFVF`, render/texture/sampler
state, `DrawPrimitiveUP`, and `Present`. That observed subset is the boundary
for the first-frame milestone; input, audio, and the native window are later
layers.
