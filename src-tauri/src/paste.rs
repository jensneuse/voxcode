use crate::error::{Error, Result};

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| Error::Paste(format!("Failed to open clipboard: {}", e)))?;
    clipboard.set_text(text)
        .map_err(|e| Error::Paste(format!("Failed to set clipboard: {}", e)))?;
    Ok(())
}

pub fn paste_text(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| Error::Paste(format!("Failed to open clipboard: {}", e)))?;

    let saved_text = clipboard.get_text().ok();

    clipboard.set_text(text)
        .map_err(|e| Error::Paste(format!("Failed to set clipboard: {}", e)))?;

    simulate_cmd_v()?;

    std::thread::sleep(std::time::Duration::from_millis(200));

    if let Some(original) = saved_text {
        let _ = clipboard.set_text(original);
    } else {
        let _ = clipboard.clear();
    }

    Ok(())
}

fn simulate_cmd_v() -> Result<()> {
    use std::ffi::c_void;

    type CGEventRef = *mut c_void;
    type CGEventSourceRef = *mut c_void;

    const K_CG_EVENT_SOURCE_STATE_HID: u32 = 1;
    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;
    const K_VK_V: u16 = 9;

    extern "C" {
        fn CGEventSourceCreate(state: u32) -> CGEventSourceRef;
        fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, key_down: bool) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventPost(tap: u32, event: CGEventRef);
        fn CFRelease(cf: *mut c_void);
    }

    unsafe {
        let source = CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_HID);
        if source.is_null() {
            return Err(Error::Paste("Failed to create CGEventSource".into()));
        }

        let key_down = CGEventCreateKeyboardEvent(source, K_VK_V, true);
        CGEventSetFlags(key_down, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_HID_EVENT_TAP, key_down);

        std::thread::sleep(std::time::Duration::from_millis(20));

        let key_up = CGEventCreateKeyboardEvent(source, K_VK_V, false);
        CGEventSetFlags(key_up, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_HID_EVENT_TAP, key_up);

        CFRelease(key_down);
        CFRelease(key_up);
        CFRelease(source);
    }

    log::info!("Simulated Cmd+V");
    Ok(())
}
