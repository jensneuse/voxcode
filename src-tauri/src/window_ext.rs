#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::{c_char, c_void};

    pub type Id = *mut c_void;
    pub type Sel = *const c_void;
    pub type Class = *mut c_void;

    extern "C" {
        pub fn sel_registerName(name: *const c_char) -> Sel;
        pub fn objc_getClass(name: *const c_char) -> Class;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send();
    }

    pub unsafe fn msg_send_id(obj: Id, sel: Sel) -> Id {
        let typed: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msg_send as *const ());
        typed(obj, sel)
    }

    pub unsafe fn msg_send_void_i64(obj: Id, sel: Sel, val: i64) {
        let typed: unsafe extern "C" fn(Id, Sel, i64) = std::mem::transmute(objc_msg_send as *const ());
        typed(obj, sel, val)
    }

    pub unsafe fn msg_send_void_u64(obj: Id, sel: Sel, val: u64) {
        let typed: unsafe extern "C" fn(Id, Sel, u64) = std::mem::transmute(objc_msg_send as *const ());
        typed(obj, sel, val)
    }

    pub unsafe fn msg_send_void_u8(obj: Id, sel: Sel, val: u8) {
        let typed: unsafe extern "C" fn(Id, Sel, u8) = std::mem::transmute(objc_msg_send as *const ());
        typed(obj, sel, val)
    }

    pub unsafe fn msg_send_usize(obj: Id, sel: Sel) -> usize {
        let typed: unsafe extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msg_send as *const ());
        typed(obj, sel)
    }

    pub unsafe fn msg_send_id_usize(obj: Id, sel: Sel, idx: usize) -> Id {
        let typed: unsafe extern "C" fn(Id, Sel, usize) -> Id =
            std::mem::transmute(objc_msg_send as *const ());
        typed(obj, sel, idx)
    }

    extern "C" {
        pub fn CFStringGetCString(s: Id, buf: *mut u8, buf_size: isize, encoding: u32) -> bool;
    }
    pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
}

#[cfg(target_os = "macos")]
pub fn configure_non_activating_panel(window: &tauri::WebviewWindow) {
    use ffi::*;

    const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
    const CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
    const STATIONARY: u64 = 1 << 4;
    const IGNORES_CYCLE: u64 = 1 << 6;
    const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

    let ns_window = match window.ns_window() {
        Ok(ptr) => ptr as Id,
        Err(e) => {
            log::error!("Failed to get NSWindow: {}", e);
            return;
        }
    };

    unsafe {
        let set_level = sel_registerName(c"setLevel:".as_ptr());
        msg_send_void_i64(ns_window, set_level, NS_FLOATING_WINDOW_LEVEL);

        let behavior = CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY;
        let set_behavior = sel_registerName(c"setCollectionBehavior:".as_ptr());
        msg_send_void_u64(ns_window, set_behavior, behavior);

        let set_hides = sel_registerName(c"setHidesOnDeactivate:".as_ptr());
        msg_send_void_u8(ns_window, set_hides, 0); // NO
    }

    log::info!("Pill window configured as floating panel");
}

/// Get the real display names from NSScreen.localizedName.
/// Returns a Vec of names in the same order as NSScreen.screens.
#[cfg(target_os = "macos")]
pub fn get_monitor_names() -> Vec<String> {
    use ffi::*;

    let mut names = Vec::new();
    unsafe {
        let ns_screen_class = objc_getClass(c"NSScreen".as_ptr());
        let screens_sel = sel_registerName(c"screens".as_ptr());
        let screens = msg_send_id(ns_screen_class as Id, screens_sel); // NSArray

        let count_sel = sel_registerName(c"count".as_ptr());
        let count = msg_send_usize(screens, count_sel);

        let object_at_sel = sel_registerName(c"objectAtIndex:".as_ptr());
        let name_sel = sel_registerName(c"localizedName".as_ptr());

        for i in 0..count {
            let screen = msg_send_id_usize(screens, object_at_sel, i);
            let ns_string = msg_send_id(screen, name_sel); // NSString

            if ns_string.is_null() {
                names.push(format!("Display {}", i + 1));
                continue;
            }

            let mut buf = [0u8; 256];
            let ok = CFStringGetCString(
                ns_string,
                buf.as_mut_ptr(),
                buf.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
            );
            if ok {
                let name = std::ffi::CStr::from_ptr(buf.as_ptr() as *const i8)
                    .to_string_lossy()
                    .to_string();
                names.push(name);
            } else {
                names.push(format!("Display {}", i + 1));
            }
        }
    }
    names
}

/// Play a macOS system sound by name (e.g. "Tink", "Pop", "Purr", "Blow").
#[cfg(target_os = "macos")]
pub fn play_system_sound(name: &str) {
    use ffi::*;

    unsafe {
        let ns_sound_class = objc_getClass(c"NSSound".as_ptr());

        // Create NSString from the sound name
        let ns_string_class = objc_getClass(c"NSString".as_ptr());
        let alloc_sel = sel_registerName(c"alloc".as_ptr());
        let raw_string = msg_send_id(ns_string_class as Id, alloc_sel);

        let cstr = std::ffi::CString::new(name).unwrap();
        let init_sel = sel_registerName(c"initWithUTF8String:".as_ptr());
        let ns_name = msg_send_id_usize(raw_string, init_sel, cstr.as_ptr() as usize);

        // [NSSound soundNamed:name]
        let sound_named_sel = sel_registerName(c"soundNamed:".as_ptr());
        let sound = msg_send_id_usize(ns_sound_class as Id, sound_named_sel, ns_name as usize);

        if !sound.is_null() {
            let play_sel = sel_registerName(c"play".as_ptr());
            msg_send_id(sound, play_sel);
        }
    }
}

/// Get the PID of the current frontmost application.
#[cfg(target_os = "macos")]
pub fn get_frontmost_app_pid() -> Option<i32> {
    use ffi::*;

    unsafe {
        let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
        let shared_sel = sel_registerName(c"sharedWorkspace".as_ptr());
        let workspace = msg_send_id(workspace_class as Id, shared_sel);

        let frontmost_sel = sel_registerName(c"frontmostApplication".as_ptr());
        let frontmost = msg_send_id(workspace, frontmost_sel);

        if !frontmost.is_null() {
            let pid_sel = sel_registerName(c"processIdentifier".as_ptr());
            let pid = msg_send_usize(frontmost, pid_sel) as i32;
            if pid > 0 {
                return Some(pid);
            }
        }
    }
    None
}

/// Reactivate a specific app by PID so paste goes to the right window.
#[cfg(target_os = "macos")]
pub fn reactivate_app_by_pid(pid: i32) {
    use ffi::*;

    unsafe {
        let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
        let shared_sel = sel_registerName(c"sharedWorkspace".as_ptr());
        let workspace = msg_send_id(workspace_class as Id, shared_sel);

        let apps_sel = sel_registerName(c"runningApplications".as_ptr());
        let apps = msg_send_id(workspace, apps_sel);

        let count_sel = sel_registerName(c"count".as_ptr());
        let count = msg_send_usize(apps, count_sel);

        let object_at_sel = sel_registerName(c"objectAtIndex:".as_ptr());
        let pid_sel = sel_registerName(c"processIdentifier".as_ptr());
        let activate_sel = sel_registerName(c"activateWithOptions:".as_ptr());

        for i in 0..count {
            let app = msg_send_id_usize(apps, object_at_sel, i);
            let app_pid = msg_send_usize(app, pid_sel) as i32;
            if app_pid == pid {
                msg_send_void_u64(app, activate_sel, 0);
                return;
            }
        }
        log::warn!("Could not find app with PID {} to reactivate", pid);
    }
}
