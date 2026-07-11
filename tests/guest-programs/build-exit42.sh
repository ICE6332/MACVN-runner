#!/bin/sh
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
host=$(rustc -vV | sed -n 's/^host: //p')
sysroot=$(rustc --print sysroot)
rust_lld="$sysroot/lib/rustlib/$host/bin/rust-lld"

if [ ! -x "$rust_lld" ]; then
    echo "rust-lld was not found at $rust_lld" >&2
    exit 1
fi

clang --target=i686-pc-windows-msvc \
    -ffreestanding -fno-builtin -fno-stack-protector -O0 \
    -c "$here/exit42.c" -o "$here/exit42.obj"
clang --target=i686-pc-windows-msvc \
    -ffreestanding -fno-builtin -fno-stack-protector -mno-sse -mno-sse2 -O1 \
    -c "$here/exit42.c" -o "$here/exit42-opt.obj"
clang --target=i686-pc-windows-msvc \
    -ffreestanding -fno-builtin -fno-stack-protector -O0 \
    -DBUILD_KERNEL32_STUB \
    -c "$here/exit42.c" -o "$here/kernel32-stub.obj"
"$rust_lld" -flavor link \
    /dll /noentry /machine:x86 /nodefaultlib /fixed /safeseh:no /timestamp:0 \
    "/out:$here/kernel32.dll" "/implib:$here/kernel32.lib" \
    "/def:$here/kernel32.def" "$here/kernel32-stub.obj"

"$rust_lld" -flavor link \
    /machine:x86 /entry:mainCRTStartup /subsystem:console /nodefaultlib /fixed /safeseh:no \
    /timestamp:0 \
    "/out:$here/exit42.exe" \
    "$here/exit42.obj" "$here/kernel32.lib"
"$rust_lld" -flavor link \
    /machine:x86 /entry:mainCRTStartup /subsystem:console /nodefaultlib /fixed /safeseh:no \
    /timestamp:0 \
    "/out:$here/exit42-opt.exe" \
    "$here/exit42-opt.obj" "$here/kernel32.lib"

rm -f \
    "$here/exit42.obj" "$here/exit42-opt.obj" "$here/kernel32-stub.obj" \
    "$here/kernel32.dll" "$here/kernel32.lib"
echo "built $here/exit42.exe and $here/exit42-opt.exe"
