# Multi-Platform Porting Guide

Voxcode is currently macOS-only.
This document outlines what it would take to port it to Windows and Linux,
for anyone interested in contributing.

## macOS-Specific Components

The frontend (`dist/`) is fully platform-agnostic.
All platform-specific code lives in the Rust backend (`src-tauri/src/`).
Here's what needs replacing on each platform:

### 1. Global Hotkey (`hotkey.rs`)

Uses `CGEventTap` + `CFRunLoop` to monitor the Right Command key globally.

- **Windows:** Use `SetWindowsHookEx` with `WH_KEYBOARD_LL`,
  or `RegisterHotKey`.
  The `windows` crate provides safe bindings.
  Map Right Command to Right Win key or a configurable alternative.
- **Linux X11:** Use `XGrabKey` via `x11` or `xcb` crate.
- **Linux Wayland:** Use the `org.freedesktop.portal.GlobalShortcuts` DBus portal.
  Not all compositors support it.
  Consider falling back to X11 compatibility via XWayland.

### 2. Paste Simulation (`paste.rs`)

Uses `CGEventCreateKeyboardEvent` + `CGEventPost` to synthesize Cmd+V.

- **Windows:** Use `SendInput` to synthesize Ctrl+V.
  The `windows` crate has `SendInput` bindings.
- **Linux X11:** Use `XSendEvent` or shell out to `xdotool key ctrl+v`.
  The `enigo` crate wraps this.
- **Linux Wayland:** Use `wtype` or `ydotool` (requires root).
  No reliable universal solution exists yet.
  The `enigo` crate has experimental Wayland support.

### 3. Editor Context / Accessibility (`editor_context.rs`)

Uses macOS Accessibility APIs (`AXUIElementCopyAttributeValue`)
to read the active window title and selected text from the frontmost app.

- **Windows:** Use the UI Automation API (`IUIAutomation`).
  The `uiautomation` crate provides Rust bindings.
  Well-supported — most Windows apps expose UI Automation elements.
- **Linux X11:** Use AT-SPI2 via the `atspi` crate.
  Coverage varies by app and toolkit.
- **Linux Wayland:** AT-SPI2 still works for accessibility,
  but there's no standard way to get the frontmost window's PID.

### 4. Window Management (`window_ext.rs`)

Uses `objc2` FFI to configure NSWindow (floating panel, non-activating)
and NSWorkspace (get/reactivate frontmost app).

- **Windows:** Use `SetWindowPos` with `HWND_TOPMOST` for floating.
  `SetForegroundWindow` to reactivate a previous app.
  Tauri v2 handles most of this cross-platform via window configuration.
- **Linux:** Tauri handles window management.
  On X11, `_NET_WM_WINDOW_TYPE_DOCK` or `_NET_WM_STATE_ABOVE` for floating.
  On Wayland, layer-shell protocol (compositor-dependent).

### 5. System Sounds (`window_ext.rs` — `play_system_sound`)

Uses `NSSound` to play system sounds ("Tink", "Pop") for audio feedback.

- **Windows:** Use `PlaySound` from `winmm.dll`,
  or the `windows` crate's `PlaySoundW`.
  Windows has built-in system sounds.
- **Linux:** Use `libcanberra` or shell out to `paplay` / `aplay`.
  Alternatively, bundle small WAV files and play them via `rodio`.

### 6. ONNX Runtime (`ort_init.rs`)

Loads `libonnxruntime.dylib` from the resources directory.

- **Windows:** Bundle and load `onnxruntime.dll` instead.
- **Linux:** Bundle and load `libonnxruntime.so` instead.
- The `ort` crate handles platform differences.
  The main work is updating the download script (`scripts/download-models.sh`)
  to fetch the right binary for each platform.

### 7. Audio Capture (`audio.rs`)

Uses `cpal` which is already cross-platform.
CoreAudio on macOS, WASAPI on Windows, ALSA/PulseAudio on Linux.
**No changes needed** — just test that it works.

## Cargo.toml Changes

Platform-specific dependencies should use target-conditional sections:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
objc2 = "0.6"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Input_KeyboardAndMouse",  # SendInput
    "Win32_UI_Accessibility",            # UI Automation
    "Win32_UI_WindowsAndMessaging",      # SetWindowPos, SetForegroundWindow
    "Win32_Media_Audio",                 # PlaySound
] }

[target.'cfg(target_os = "linux")'.dependencies]
# x11 = "2"      # for X11 hotkey/paste
# atspi = "0.5"   # for accessibility
```

## Tauri Configuration

In `tauri.conf.json`, add Windows and Linux bundle targets:

```json
{
  "bundle": {
    "targets": ["dmg", "app", "msi", "nsis", "deb", "appimage"],
    "resources": {
      "macos": ["resources/libonnxruntime.dylib"],
      "windows": ["resources/onnxruntime.dll"],
      "linux": ["resources/libonnxruntime.so"]
    }
  }
}
```

Remove `"macOSPrivateApi": true` or gate it behind a platform check.

## Suggested Architecture

Introduce a `PlatformBridge` trait and implement it per platform:

```rust
pub trait PlatformBridge {
    fn start_hotkey_listener(&self, tx: Sender<HotkeyEvent>) -> Result<()>;
    fn paste_text(&self, text: &str) -> Result<()>;
    fn get_frontmost_app(&self) -> Option<AppHandle>;
    fn reactivate_app(&self, handle: &AppHandle) -> Result<()>;
    fn capture_editor_context(&self, handle: &AppHandle) -> Option<EditorContext>;
    fn play_feedback_sound(&self, sound: FeedbackSound) -> Result<()>;
}
```

Each platform module (`platform_macos.rs`, `platform_windows.rs`, `platform_linux.rs`)
implements this trait,
and `lib.rs` uses `#[cfg]` to select the right one at compile time.

## Permissions

| Capability | macOS | Windows | Linux |
|---|---|---|---|
| Global hotkey | Accessibility permission required | No permission needed | No permission on X11; portal on Wayland |
| Microphone | System prompt on first use | Settings > Privacy > Microphone | No prompt (PulseAudio allows by default) |
| Accessibility (read other apps) | Accessibility permission required | No permission needed (UI Automation works) | No permission (AT-SPI2 is on by default) |
| Clipboard | No permission needed | No permission needed | No permission needed |

## Platform Difficulty Assessment

**Windows** — straightforward.
Every macOS API has a well-documented Windows equivalent.
All features are achievable.

**Linux X11** — moderate.
Most features work via X11 APIs and existing crates.
Accessibility coverage varies by app/toolkit.

**Linux Wayland** — hard.
No standardized global hotkey, keystroke injection, or window management APIs.
Solutions are compositor-dependent (GNOME vs KDE vs Sway).
Consider supporting X11 first and adding Wayland best-effort.

## Getting Started

1. Pick a platform (Windows recommended as first port)
2. Add the `PlatformBridge` trait and move macOS code behind it
3. Implement the trait for your platform,
   starting with hotkey + paste (the core loop)
4. Update the build script to download the right ONNX runtime binary
5. Test on CI — `cargo check` on `windows-latest` / `ubuntu-latest` catches most issues
6. Open a PR

## Known Challenges

- **Right Command key** doesn't exist on Windows/Linux keyboards.
  You'll need a sensible default (Right Alt? Right Ctrl? Configurable?)
  and a way to configure it.
- **Wayland keystroke injection** is intentionally restricted for security.
  There is no clean solution.
  `ydotool` requires root, `wtype` only works on wlroots-based compositors.
- **Editor context on Linux** is best-effort.
  Many terminal emulators and Electron apps don't expose selected text via AT-SPI2.
  The ripgrep-based file search still works — you just won't get selected text.
