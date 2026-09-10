# FXC Shader Compiler Drop-in Tool

A lightweight, standalone replacement for Microsoft's `fxc.exe` shader compiler.

## Motivation
`gpui-pre-windows` requires `fxc.exe` during release builds (`#[cfg(not(debug_assertions))]`) to compile its HLSL shaders (`shaders.hlsl` and `color_text_raster.hlsl`) into byte arrays.

Normally, `fxc.exe` is distributed as part of the multi-gigabyte Windows 10/11 SDK. Installing this SDK requires administrator privileges and several gigabytes of disk space.

## Solution
Windows includes `d3dcompiler_47.dll` out-of-the-box in `C:\Windows\System32`. This tool dynamically loads `d3dcompiler_47.dll` and invokes `D3DCompile` using `D3D_COMPILE_STANDARD_FILE_INCLUDE`, formatting the compiled bytecode into the C header files (`/Fh`) that `gpui-pre-windows` expects.

## Compilation
Can be compiled instantly with `rustc` without external crates:
```bash
rustc -O main.rs -o fxc.exe
```

The packaging scripts (`package-windows.ps1` and `package-windows.cmd`) automatically compile and use this tool if `GPUI_FXC_PATH` is not already configured.
