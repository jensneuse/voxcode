use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type CGEventMask = u64;
type CGEventType = u32;
type CGEventFlags = u64;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFBooleanRef = *const c_void;

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const K_CG_EVENT_FLAGS_CHANGED: CGEventType = 12;
const K_CG_EVENT_KEY_DOWN: CGEventType = 10;
const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = 0xFFFF_FFFE;
const K_CG_EVENT_TAP_DISABLED_BY_USER: CGEventType = 0xFFFF_FFFF;
const NX_DEVICERCMDKEYMASK: CGEventFlags = 0x0000_0010;
const K_CG_EVENT_FLAG_MASK_COMMAND: CGEventFlags = 0x0010_0000;
const K_VK_ESCAPE: u16 = 53;
const K_VK_C: u16 = 8;
const K_VK_V: u16 = 9;
const K_CG_EVENT_FLAG_MASK_ALTERNATE: CGEventFlags = 0x0008_0000;
const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

type CGEventTapCallBack = extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, events_of_interest: CGEventMask, callback: CGEventTapCallBack, user_info: *mut c_void) -> CFMachPortRef;
    fn CGEventGetFlags(event: CGEventRef) -> CGEventFlags;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CFMachPortCreateRunLoopSource(allocator: *const c_void, port: CFMachPortRef, order: i64) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source_handled: u8) -> i32;
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    fn IOHIDCheckAccess(request_type: u32) -> u32;
    fn IOHIDRequestAccess(request_type: u32) -> bool;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    fn CFRelease(cf: *mut c_void);
}

extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
    static kAXTrustedCheckOptionPrompt: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    static kCFBooleanTrue: CFBooleanRef;
}

struct HotkeyContext {
    app_handle: tauri::AppHandle,
    right_cmd_was_down: bool,
    tap: CFMachPortRef,
    is_recording: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    let ctx = unsafe { &mut *(user_info as *mut HotkeyContext) };

    match event_type {
        K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT | K_CG_EVENT_TAP_DISABLED_BY_USER => {
            log::warn!("Event tap was disabled (type=0x{:08X}), re-enabling", event_type);
            unsafe { CGEventTapEnable(ctx.tap, true); }
            return event;
        }
        K_CG_EVENT_FLAGS_CHANGED => {
            let flags = unsafe { CGEventGetFlags(event) };
            let right_cmd_down = (flags & NX_DEVICERCMDKEYMASK) != 0
                && (flags & K_CG_EVENT_FLAG_MASK_COMMAND) != 0;
            if right_cmd_down && !ctx.right_cmd_was_down {
                log::info!("Right Command pressed");
                let _ = ctx.app_handle.emit("hotkey-toggle", "paste");
            }
            ctx.right_cmd_was_down = right_cmd_down;
        }
        K_CG_EVENT_KEY_DOWN => {
            let flags = unsafe { CGEventGetFlags(event) };
            let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
            let cmd_down = (flags & K_CG_EVENT_FLAG_MASK_COMMAND) != 0;
            let option_down = (flags & K_CG_EVENT_FLAG_MASK_ALTERNATE) != 0;

            // Cmd+Option+C → voice copy (start recording)
            if cmd_down && option_down && keycode == K_VK_C {
                log::info!("Cmd+Option+C pressed — start voice copy");
                let _ = ctx.app_handle.emit("hotkey-start", ());
                return event;
            }

            // Cmd+V while recording → stop and paste
            if cmd_down && !option_down && keycode == K_VK_V
                && ctx.is_recording.load(std::sync::atomic::Ordering::Relaxed)
            {
                // Clear clipboard immediately so the passthrough Cmd+V
                // (which we can't block in listen-only mode) pastes nothing.
                // The real paste happens after transcription completes.
                let _ = crate::paste::copy_to_clipboard("");
                log::info!("Cmd+V pressed while recording — stop and paste");
                let _ = ctx.app_handle.emit("hotkey-stop", ());
                return event;
            }

            if keycode == K_VK_ESCAPE {
                log::info!("Escape pressed");
                let _ = ctx.app_handle.emit("hotkey-cancel", ());
            }
        }
        _ => {}
    }
    event
}

pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Check accessibility and show the macOS system prompt if not yet granted.
fn prompt_accessibility() -> bool {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue as *const c_void];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const _ as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const _ as *const c_void,
        );
        let trusted = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict as *mut c_void);
        trusted
    }
}

/// IOHIDRequestType
const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
/// IOHIDAccessType
const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;

/// Check Input Monitoring status without prompting.
fn is_input_monitoring_granted() -> bool {
    unsafe { IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) == K_IOHID_ACCESS_TYPE_GRANTED }
}

/// Prompt for Input Monitoring permission via IOHIDRequestAccess.
/// This shows the system dialog only when the status is "unknown" (first time).
/// If status is "denied", opens System Settings directly.
fn prompt_input_monitoring() -> bool {
    unsafe {
        let status = IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
        if status == K_IOHID_ACCESS_TYPE_GRANTED {
            return true;
        }
        log::warn!("Input Monitoring not granted (status={})", status);
        // status 2 = unknown (first time), IOHIDRequestAccess shows system prompt
        // status 1 = denied, IOHIDRequestAccess won't show anything
        IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
        // Re-check
        IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) == K_IOHID_ACCESS_TYPE_GRANTED
    }
}

pub fn start_hotkey_listener(
    app_handle: tauri::AppHandle,
    stop: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("hotkey-listener".into())
        .spawn(move || {
            // Prompt for Accessibility permission
            if !prompt_accessibility() {
                log::warn!("Accessibility permission not granted — waiting for user to enable it…");
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if check_accessibility() {
                        log::info!("Accessibility permission granted");
                        break;
                    }
                }
            }
            // Prompt for Input Monitoring permission
            if !stop.load(Ordering::Relaxed) && !prompt_input_monitoring() {
                log::warn!("Input Monitoring not granted — waiting for user to enable it…");
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if is_input_monitoring_granted() {
                        log::info!("Input Monitoring permission granted");
                        break;
                    }
                }
            }
            if !stop.load(Ordering::Relaxed) {
                run_event_tap(app_handle, stop, is_recording);
            }
        })
        .expect("failed to spawn hotkey thread")
}

fn run_event_tap(app_handle: tauri::AppHandle, stop: Arc<AtomicBool>, is_recording: Arc<AtomicBool>) {
    let event_mask: CGEventMask = (1 << K_CG_EVENT_FLAGS_CHANGED) | (1 << K_CG_EVENT_KEY_DOWN);
    // Create context with a null tap initially; we set it after CGEventTapCreate.
    let ctx = Box::new(HotkeyContext { app_handle, right_cmd_was_down: false, tap: std::ptr::null_mut(), is_recording });
    let ctx_ptr = Box::into_raw(ctx);

    let tap = unsafe {
        CGEventTapCreate(K_CG_HID_EVENT_TAP, K_CG_HEAD_INSERT_EVENT_TAP, K_CG_EVENT_TAP_OPTION_LISTEN_ONLY, event_mask, event_tap_callback, ctx_ptr as *mut c_void)
    };

    if tap.is_null() {
        log::error!("Failed to create CGEventTap. Accessibility permission is likely missing.");
        unsafe { drop(Box::from_raw(ctx_ptr)); }
        return;
    }

    // Store the tap handle so the callback can re-enable it.
    unsafe { (*ctx_ptr).tap = tap; }

    let source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0) };
    let run_loop = unsafe { CFRunLoopGetCurrent() };
    unsafe {
        CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
    }

    log::info!("Hotkey listener started (CGEventTap active)");

    loop {
        if stop.load(Ordering::Relaxed) { break; }
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.5, 0); }
    }

    unsafe {
        CGEventTapEnable(tap, false);
        CFRunLoopStop(run_loop);
        drop(Box::from_raw(ctx_ptr));
    }
    log::info!("Hotkey listener stopped");
}
