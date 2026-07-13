# Mouseless — Windows port

A keyboard-driven mouse controller for Windows. Press a global hotkey
(default **Alt+U**) to summon a full-screen grid overlay, type grid letters to
narrow down a target cell, then use shortcut keys to click, double-click,
right-click, drag, or scroll — all without touching the mouse.

## Project layout

```
windows/
├── Cargo.toml              # manifest (windows-rs 0.58, serde)
├── Cargo.lock
├── build.rs                # compiles resources/icon into the .exe
├── .cargo/config.toml      # default target: x86_64-pc-windows-gnu
├── resources/
│   ├── AppIcon.ico         # multi-size icon (16–256px) embedded in the .exe
│   └── resource.rc         # resource script (icon resource ID 1)
└── src/
    ├── main.rs             # entry point, message loop, key dispatch, tray
    ├── input.rs            # low-level keyboard hook (WH_KEYBOARD_LL)
    ├── core.rs             # platform-independent overlay state machine
    ├── mouse.rs            # synthetic mouse via SendInput
    ├── accessibility.rs    # UI Automation click-target snapping
    ├── overlay.rs          # layered overlay windows (GDI software rendering)
    ├── free_mode.rs        # direct cursor-movement mode
    ├── settings.rs         # settings schema + JSON persistence
    ├── monitor.rs          # multi-monitor enumeration
    ├── main_window.rs      # hidden message-only window (hosts tray + msgs)
    ├── tray.rs             # system tray icon + popup menu
    ├── toast.rs            # toast notifications
    ├── preferences.rs      # preferences dialog (raw Win32 controls)
    └── common.rs           # shared types (Rect, MouseButton, rgb helper)
```

## Prerequisites

- **Rust** (stable, 1.75+ recommended)
- The `x86_64-pc-windows-gnu` target:
  ```sh
  rustup target add x86_64-pc-windows-gnu
  ```
- A Windows resource compiler for embedding the icon:
  - **Native Windows (MSVC):** `rc.exe` ships with Visual Studio Build Tools.
  - **Native Windows (GNU):** `windres` ships with MinGW-w64.
  - **Cross-compilation from Linux:** see below.

## Building on Windows (native)

```sh
cargo build --release
```

The output binary is at:

```
target/x86_64-pc-windows-gnu/release/mouseless.exe
```

`build.rs` automatically finds `windres` (or `rc.exe`), compiles
`resources/resource.rc`, and links the icon into the `.exe`. If no resource
compiler is found the build still succeeds — it just won't have the embedded
icon.

## Cross-compiling from Linux (x86_64)

Because this project uses the `x86_64-pc-windows-gnu` target, you need a
MinGW-w64 toolchain. The easiest self-contained option is
[llvm-mingw](https://github.com/mstorsjo/llvm-mingw):

```sh
# 1. Download & extract llvm-mingw (ucrt, x86_64 host)
curl -sL -o llvm-mingw.tar.xz \
  "https://github.com/mstorsjo/llvm-mingw/releases/latest/download/llvm-mingw-<version>-ucrt-ubuntu-22.04-x86_64.tar.xz"
tar -xf llvm-mingw.tar.xz

# 2. Put it on PATH
export PATH="$PWD/llvm-mingw-<version>-ucrt-ubuntu-22.04-x86_64/bin:$PATH"

# 3. Tell cargo to use the MinGW linker
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

# 4. Build
cargo build --release
```

> **Note on `libgcc` / `libgcc_eh`:** Rust's `windows-gnu` std expects to link
> against `libgcc` and `libgcc_eh`. llvm-mingw replaces these with
> `compiler-rt` + `libunwind`. If the linker complains about missing
> `-lgcc` / `-lgcc_eh`, create shim libraries:
>
> ```sh
> MINGW="$PWD/llvm-mingw-<version>-ucrt-ubuntu-22.04-x86_64"
> mkdir -p mingw-stubs
> cp "$MINGW/lib/clang/<llvm-ver>/lib/windows/libclang_rt.builtins-x86_64.a" mingw-stubs/libgcc.a
> cp "$MINGW/x86_64-w64-mingw32/lib/libunwind.a"                  mingw-stubs/libgcc_eh.a
> export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-L $PWD/mingw-stubs"
> ```
>
> Then rebuild.

## Embedding the icon

The app icon (`resources/AppIcon.ico`) is embedded into the `.exe` at build
time via `build.rs` + `resources/resource.rc` (icon resource ID `1`). This makes
the icon appear in:

- **Explorer / taskbar** — as the executable's icon
- **System tray** — `tray::load_app_icon()` loads resource ID 1 at runtime
- **Preferences window** — set as the window class `hIcon`

The `.ico` was generated from the macOS `Resources/AppIcon.png` (multi-size:
256, 128, 64, 48, 32, 16 px):
```sh
convert Resources/AppIcon.png -define icon:auto-resize=256,128,64,48,32,16 \
  windows/resources/AppIcon.ico
```

## Is the binary self-contained?

**Yes.** The release `.exe` has no external runtime dependencies. It only
imports Windows system DLLs that ship with every Windows 10/11 install:

```
kernel32.dll, user32.dll, gdi32.dll, ole32.dll, oleaut32.dll,
shell32.dll, advapi32.dll, userenv.dll, ws2_32.dll, ntdll.dll, ...
```

There is **no** dependency on `VCRUNTIME140.dll`, `libgcc_s_seh-1.dll`,
`libwinpthread-1.dll`, or any other redistributable runtime. You can copy the
single `mouseless.exe` to any Windows 10/11 machine and run it directly.

## Code signing (SmartScreen)

The binary is **unsigned**. On first launch Windows SmartScreen may show
"Windows protected your PC" — click **More info → Run anyway**. This is normal
for any unsigned application. Permanently suppressing the warning requires a
purchased **Authenticode code-signing certificate**, which is a commercial
purchase, not a build-tooling change.

## How the keyboard hook works

A `WH_KEYBOARD_LL` low-level hook captures keystrokes globally. The hook
callback (`dispatch_key`) must return **fast** — Windows silently disables
hooks that exceed `LowLevelHooksTimeout` (~300 ms). To stay within that budget:

- **Hotkey toggle** and **overlay rendering** are deferred to the message loop
  via `PostMessageW` (`WM_TOGGLE_OVERLAY` / `WM_RENDER_OVERLAY`).
- **Click actions** run on a background `std::thread` (the accessibility scan
  + `SendInput` sleeps would blow the timeout).
- Only the cheap state-machine update (`OverlayState::handle_key`) runs inline
  in the hook callback.

## Settings

Stored as JSON in `%APPDATA%\Mouseless\config.json`. Defaults:

| Setting          | Default      |
|------------------|--------------|
| Overlay hotkey   | **Alt + U**  |
| Quit grid key    | Q            |
| Grid             | 5 × 5        |
| Overlay opacity  | 0.72         |
| Free-mode step   | 26 px        |
| Scroll step      | 18           |
| Continuous mode  | off          |

In the Preferences dialog, the default overlay hotkey shows **only Alt
checked** (Ctrl, Shift, Win are unchecked).
