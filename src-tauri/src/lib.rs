pub mod audio;
pub mod config;
pub mod editor_context;
pub mod ort_init;
pub mod parakeet;
pub mod parakeet_longform;
mod error;
mod hotkey;
pub mod paste;
pub mod repo_index;
pub mod state;
pub mod window_ext;

pub fn manifest_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Listener, Manager,
};

fn init_logger() {
    let mut logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    logger
        .filter_module("transcribe_rs::onnx::session", log::LevelFilter::Warn)
        .filter_module(
            "transcribe_rs::transcriber::energy_adaptive_chunked",
            log::LevelFilter::Warn,
        )
        .filter_module("ort", log::LevelFilter::Warn);

    let _ = logger.try_init();
}

fn save_config_from_state(state: &state::AppState) {
    config::save(&config::AppConfig {
        selected_monitor: state.selected_monitor,
    });
}

/// Build the tray menu, including the "Recent" submenu from transcript history.
fn build_tray_menu(app: &tauri::AppHandle) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // Enumerate monitors
    let monitors: Vec<tauri::Monitor> = if let Some(window) = app.get_webview_window("pill") {
        window.available_monitors()?.into_iter().collect()
    } else {
        vec![]
    };

    #[cfg(target_os = "macos")]
    let real_names = window_ext::get_monitor_names();
    #[cfg(not(target_os = "macos"))]
    let real_names: Vec<String> = Vec::new();

    let state = app.state::<std::sync::Mutex<state::AppState>>();
    let guard = state.lock().unwrap();
    let selected = guard.selected_monitor;
    let history: Vec<String> = guard.transcript_history.iter().cloned().collect();
    drop(guard);

    // Screen submenu
    let mut screen_items: Vec<CheckMenuItem<tauri::Wry>> = Vec::new();
    for (i, monitor) in monitors.iter().enumerate() {
        let name = real_names.get(i)
            .cloned()
            .unwrap_or_else(|| monitor.name().map(|n| n.to_string()).unwrap_or_else(|| format!("Display {}", i + 1)));
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let logical_w = (size.width as f64 / scale) as u32;
        let logical_h = (size.height as f64 / scale) as u32;
        let label = format!("{} ({}x{})", name, logical_w, logical_h);
        let item = CheckMenuItem::with_id(app, format!("screen_{}", i), &label, true, i == selected, None::<&str>)?;
        screen_items.push(item);
    }
    let screen_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = screen_items.iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let screen_submenu = Submenu::with_items(app, "Screen", true, &screen_refs)?;

    // Recent transcripts submenu
    let mut recent_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    if history.is_empty() {
        recent_items.push(MenuItem::with_id(app, "recent_empty", "(empty)", false, None::<&str>)?);
    } else {
        for (i, text) in history.iter().enumerate() {
            // Truncate label to ~60 chars for readability
            let label = if text.len() > 60 {
                format!("{}...", &text[..57])
            } else {
                text.clone()
            };
            recent_items.push(MenuItem::with_id(app, format!("recent_{}", i), &label, true, None::<&str>)?);
        }
    }
    let recent_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = recent_items.iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let recent_submenu = Submenu::with_items(app, "Recent", true, &recent_refs)?;

    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[
        &screen_submenu,
        &separator,
        &recent_submenu,
        &separator2,
        &quit,
    ]).map_err(Into::into)
}

/// Rebuild tray menu and apply it to the tray icon.
fn rebuild_tray_menu(app: &tauri::AppHandle) {
    match build_tray_menu(app) {
        Ok(menu) => {
            if let Some(tray) = app.tray_by_id("main_tray") {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    log::error!("Failed to update tray menu: {}", e);
                }
            }
        }
        Err(e) => log::error!("Failed to build tray menu: {}", e),
    }
}

fn drop_accumulator_sender(app: &tauri::AppHandle) {
    let session = app.state::<std::sync::Arc<state::SessionState>>();
    session.accum_chunk_sender.lock().unwrap().take();
}

fn discard_session(app: &tauri::AppHandle) {
    let session = app.state::<std::sync::Arc<state::SessionState>>();
    session.accum_chunk_sender.lock().unwrap().take();
    session.accum_handle.lock().unwrap().take();
    session.reference_text.lock().unwrap().clear();
}

fn finish_processing_as_idle(
    app: &tauri::AppHandle,
    state_guard: &std::sync::Mutex<state::AppState>,
) {
    if let Some(window) = app.get_webview_window("pill") {
        let _ = window.hide();
    }
    {
        let mut state = state_guard.lock().unwrap();
        state.recording_state = state::RecordingState::Idle;
    }
    let is_recording = app.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
    is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = app.emit("recording-state", state::RecordingState::Idle);
}

fn start_recording(handle: &tauri::AppHandle) {
    let state = handle.state::<std::sync::Mutex<state::AppState>>();
    let current_state = state.lock().unwrap().recording_state;
    if current_state != state::RecordingState::Idle {
        log::debug!("Ignoring start — not idle (state={:?})", current_state);
        return;
    }

    let mut s = state.lock().unwrap();
    if s.last_recording_start.elapsed().as_millis() < 300 {
        log::debug!("Ignoring start — debounce");
        return;
    }
    s.recording_state = state::RecordingState::Recording;
    s.last_recording_start = std::time::Instant::now();
    #[cfg(target_os = "macos")]
    {
        s.previous_app_pid = window_ext::get_frontmost_app_pid();
        log::info!("Saved previous app PID: {:?}", s.previous_app_pid);
    }
    let capture_pid = s.previous_app_pid;
    drop(s);

    // Set is_recording flag for hotkey callback
    let is_recording = handle.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
    is_recording.store(true, std::sync::atomic::Ordering::Relaxed);

    let raw_capture = capture_pid.and_then(editor_context::capture_editor_raw);
    let has_selected_text = raw_capture.as_ref()
        .map(|r| !r.selected_text.is_empty())
        .unwrap_or(false);
    let editor_resolve_rx = raw_capture.map(|raw| {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let ctx = editor_context::resolve_editor_context(raw);
            let _ = tx.send(ctx);
        });
        rx
    });

    let mut s = state.lock().unwrap();
    if s.recording_state != state::RecordingState::Recording {
        return;
    }
    s.session_id += 1;
    let current_session_id = s.session_id;
    let monitor_idx = s.selected_monitor;
    log::info!("Recording started (monitor {})", monitor_idx);
    #[cfg(target_os = "macos")]
    window_ext::play_system_sound("Tink");
    position_pill_on_monitor(handle, monitor_idx);
    if let Some(window) = handle.get_webview_window("pill") {
        let _ = window.show();
    }

    let session_state = handle
        .state::<std::sync::Arc<state::SessionState>>()
        .inner()
        .clone();
    session_state.begin_session();
    let (accum_tx, accum_rx) = std::sync::mpsc::channel::<Vec<f32>>();
    *session_state.accum_chunk_sender.lock().unwrap() = Some(accum_tx);
    let accum_join = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        while let Ok(chunk) = accum_rx.recv() {
            buffer.extend_from_slice(&chunk);
        }
        buffer
    });
    *session_state.accum_handle.lock().unwrap() = Some(accum_join);
    let _ = handle.emit("recording-state", s.recording_state);
    if has_selected_text {
        let _ = handle.emit("resolving-context", serde_json::json!({
            "session_id": s.session_id,
        }));
    }

    let resolve_start = std::time::Instant::now();
    let handle_clone = handle.clone();
    let session_for_async = session_state.clone();
    tauri::async_runtime::spawn(async move {
        let editor_context = editor_resolve_rx.and_then(|rx| {
            rx.recv_timeout(std::time::Duration::from_secs(30)).ok()
        });
        let resolve_ms = resolve_start.elapsed().as_millis();
        if let Some(ref editor_context) = editor_context {
            let reference = editor_context.format_markdown_reference();
            if !reference.is_empty() {
                session_for_async.set_reference_text(&reference);
                emit_reference_text(
                    &handle_clone,
                    current_session_id,
                    &reference,
                    resolve_ms as u64,
                );
            }
        }

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let state_guard = handle_clone.state::<std::sync::Mutex<state::AppState>>();
            if state_guard.lock().unwrap().recording_state != state::RecordingState::Recording {
                break;
            }
        }

        let state_guard = handle_clone.state::<std::sync::Mutex<state::AppState>>();
        if state_guard.lock().unwrap().recording_state != state::RecordingState::Processing {
            return;
        }

        // Join audio threads
        let session = handle_clone.state::<std::sync::Arc<state::SessionState>>();
        let capture_handle = session.worker_handle.lock().unwrap().take();
        if let Some(handle) = capture_handle {
            let _ = tokio::task::spawn_blocking(move || handle.join()).await;
        }
        let amplitude_bridge_handle =
            session.amplitude_bridge_handle.lock().unwrap().take();
        if let Some(handle) = amplitude_bridge_handle {
            let _ = tokio::task::spawn_blocking(move || handle.join()).await;
        }

        // Join the accumulator thread to get the full audio buffer
        let audio_buffer = {
            let accum_handle = session.accum_handle.lock().unwrap().take();
            if let Some(handle) = accum_handle {
                tokio::task::spawn_blocking(move || handle.join().unwrap_or_default())
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        };

        if state_guard.lock().unwrap().recording_state != state::RecordingState::Processing {
            log::info!("Transcription cancelled by user, discarding result");
            return;
        }

        // Run Parakeet final-pass transcription
        let transcript = if audio_buffer.is_empty() {
            log::info!("Empty audio buffer, nothing to transcribe");
            String::new()
        } else {
            log::info!(
                "Running Parakeet final pass on {:.1}s of audio",
                audio_buffer.len() as f64 / 16_000.0
            );
            match tokio::task::spawn_blocking(move || {
                crate::parakeet::transcribe(&audio_buffer)
            })
            .await
            {
                Ok(Ok(text)) => {
                    log::debug!("Parakeet transcript: {text}");
                    text
                }
                Ok(Err(e)) => {
                    log::error!("Parakeet transcription failed: {e}");
                    String::new()
                }
                Err(e) => {
                    log::error!("Parakeet task panicked: {e}");
                    String::new()
                }
            }
        };
        log::debug!("Final transcript: {}", transcript);
        if transcript.trim().is_empty() && editor_context.is_none() {
            log::info!("Empty transcript, no editor context, nothing to paste");
        } else {
            if state_guard.lock().unwrap().recording_state != state::RecordingState::Processing {
                log::info!("Cancelled before paste, discarding");
                return;
            }

            let paste_text = if let Some(ref editor_context) = editor_context {
                editor_context.format_with_transcript(&transcript)
            } else {
                transcript.clone()
            };

            {
                let s = state_guard.lock().unwrap();
                if s.recording_state != state::RecordingState::Processing {
                    log::info!("Cancelled before output, discarding");
                    return;
                }
            }

            if let Some(window) = handle_clone.get_webview_window("pill") {
                let _ = window.hide();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            #[cfg(target_os = "macos")]
            {
                let previous_pid = state_guard.lock().unwrap().previous_app_pid;
                if let Some(pid) = previous_pid {
                    window_ext::reactivate_app_by_pid(pid);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));

            if let Err(e) = paste::paste_text(&paste_text) {
                log::error!("Paste error: {}", e);
            }

            {
                let mut s = state_guard.lock().unwrap();
                s.push_transcript_history(paste_text);
            }
            rebuild_tray_menu(&handle_clone);
        }

        finish_processing_as_idle(&handle_clone, &state_guard);
    });
}

fn stop_recording(handle: &tauri::AppHandle) {
    let state = handle.state::<std::sync::Mutex<state::AppState>>();
    let current_state = state.lock().unwrap().recording_state;
    if current_state != state::RecordingState::Recording {
        log::debug!("Ignoring stop — not recording (state={:?})", current_state);
        return;
    }
    let mut s = state.lock().unwrap();
    s.recording_state = state::RecordingState::Processing;
    log::info!("Recording stopped, processing...");
    #[cfg(target_os = "macos")]
    window_ext::play_system_sound("Pop");
    let session = handle.state::<std::sync::Arc<state::SessionState>>();
    session.signal_stop();
    drop_accumulator_sender(handle);
    let _ = handle.emit("recording-state", s.recording_state);

    // Clear is_recording flag
    let is_recording = handle.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
    is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn emit_reference_text(
    app: &tauri::AppHandle,
    session_id: u64,
    reference_text: &str,
    resolve_ms: u64,
) {
    let _ = app.emit(
        "transcript-update",
        serde_json::json!({
            "reference_text": reference_text,
            "session_id": session_id,
            "resolve_ms": resolve_ms,
        }),
    );
}

#[tauri::command]
fn stop(
    state: tauri::State<'_, std::sync::Mutex<state::AppState>>,
    app: tauri::AppHandle,
    session: tauri::State<'_, std::sync::Arc<state::SessionState>>,
) -> Result<(), error::Error> {
    log::info!("Stop command received from frontend");
    let mut s = state.lock().unwrap();
    if s.recording_state == state::RecordingState::Recording {
        s.recording_state = state::RecordingState::Processing;
        session.signal_stop();
        drop_accumulator_sender(&app);
        let is_recording = app.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
        is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = app.emit("recording-state", s.recording_state);
    }
    Ok(())
}

#[tauri::command]
fn cancel(
    state: tauri::State<'_, std::sync::Mutex<state::AppState>>,
    app: tauri::AppHandle,
    session: tauri::State<'_, std::sync::Arc<state::SessionState>>,
) -> Result<(), error::Error> {
    log::info!("Cancel command received from frontend");
    let mut s = state.lock().unwrap();
    if matches!(
        s.recording_state,
        state::RecordingState::Recording | state::RecordingState::Processing
    ) {
        s.recording_state = state::RecordingState::Idle;
        session.signal_cancel();
        discard_session(&app);

        if let Some(window) = app.get_webview_window("pill") {
            let _ = window.hide();
        }
        let is_recording = app.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
        is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = app.emit("recording-state", s.recording_state);
    }
    Ok(())
}

#[tauri::command]
fn start_amplitude_stream(
    on_amplitude: tauri::ipc::Channel<f32>,
    session: tauri::State<'_, std::sync::Arc<state::SessionState>>,
) -> Result<(), error::Error> {
    log::info!("Amplitude stream requested");
    session.begin_session();

    let (amplitude_tx, amplitude_rx) = std::sync::mpsc::channel::<f32>();
    let amplitude_bridge = std::thread::spawn(move || {
        while let Ok(amplitude) = amplitude_rx.recv() {
            if on_amplitude.send(amplitude).is_err() {
                break;
            }
        }
    });

    let mut capture = audio::AudioInputWorker::new(session.stop_signal.clone());
    capture.set_amplitude_sender(amplitude_tx);
    if let Some(sender) = session.accum_chunk_sender.lock().unwrap().clone() {
        capture.set_accumulator_sender(sender);
    }

    let handle = capture.start();
    *session.worker_handle.lock().unwrap() = Some(handle);
    *session.amplitude_bridge_handle.lock().unwrap() = Some(amplitude_bridge);
    Ok(())
}

#[tauri::command]
fn get_state(state: tauri::State<'_, std::sync::Mutex<state::AppState>>) -> Result<state::RecordingState, error::Error> {
    let s = state.lock().unwrap();
    Ok(s.recording_state)
}

#[tauri::command]
fn resize_pill(height: f64, app: tauri::AppHandle) -> Result<(), error::Error> {
    const PILL_WIDTH: f64 = 400.0;
    const MIN_HEIGHT: f64 = 76.0;

    if let Some(window) = app.get_webview_window("pill") {
        let height = height.max(MIN_HEIGHT).ceil();
        window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(PILL_WIDTH, height)))?;
    }

    Ok(())
}

/// Position the pill window centered on the given monitor index.
fn position_pill_on_monitor(app: &tauri::AppHandle, monitor_idx: usize) {
    let Some(window) = app.get_webview_window("pill") else { return };
    let monitors: Vec<tauri::Monitor> = match window.available_monitors() {
        Ok(m) => m.into_iter().collect(),
        Err(_) => return,
    };
    let Some(monitor) = monitors.get(monitor_idx) else { return };

    let pos = monitor.position();
    let size = monitor.size();
    let scale = monitor.scale_factor();

    // Center horizontally on this monitor, 100 logical pixels from top
    let screen_width = size.width as f64 / scale;
    let x = pos.x as f64 / scale + (screen_width - 400.0) / 2.0;
    let y = pos.y as f64 / scale + 100.0;

    let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
}

pub fn run() {
    init_logger();
    let app_config = config::load();

    tauri::Builder::default()
        .manage(std::sync::Mutex::new(state::AppState {
            recording_state: state::RecordingState::Idle,
            session_id: 0,
            selected_monitor: app_config.selected_monitor,
            last_recording_start: std::time::Instant::now(),
            previous_app_pid: None,
            transcript_history: std::collections::VecDeque::new(),
        }))
        .manage(state::SessionState::new())
        .invoke_handler(tauri::generate_handler![stop, cancel, start_amplitude_stream, get_state, resize_pill])
        .setup(|app| {
            let menu = build_tray_menu(app.handle())?;

            // Load tray-specific icon (mic silhouette on transparent, not the full app icon)
            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                .expect("failed to load tray icon");

            TrayIconBuilder::with_id("main_tray")
                .icon(tray_icon.to_owned())
                .icon_as_template(true)
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    if id == "quit" {
                        app.exit(0);
                    } else if let Some(idx_str) = id.strip_prefix("recent_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            let state = app.state::<std::sync::Mutex<state::AppState>>();
                            let guard = state.lock().unwrap();
                            if let Some(text) = guard.transcript_history.get(idx) {
                                let text = text.clone();
                                drop(guard);
                                // Copy to clipboard
                                if let Err(e) = paste::copy_to_clipboard(&text) {
                                    log::error!("Failed to copy recent transcript: {}", e);
                                } else {
                                    log::info!("Copied recent transcript {} to clipboard", idx);
                                }
                            }
                        }
                    } else if let Some(idx_str) = id.strip_prefix("screen_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            let state = app.state::<std::sync::Mutex<state::AppState>>();
                            let mut guard = state.lock().unwrap();
                            guard.selected_monitor = idx;
                            save_config_from_state(&guard);
                            drop(guard);
                            log::info!("Selected monitor: {}", idx);
                            position_pill_on_monitor(app, idx);
                            // Update check marks on all screen items
                            for i in 0..10 {
                                let item_id = format!("screen_{}", i);
                                if let Some(item) = app.menu().and_then(|m| m.get(&item_id)) {
                                    if let Some(check) = item.as_check_menuitem() {
                                        check.set_checked(i == idx).ok();
                                    }
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            // Position pill on saved monitor and configure as floating
            let initial_monitor = app.state::<std::sync::Mutex<state::AppState>>().lock().unwrap().selected_monitor;
            if let Some(window) = app.get_webview_window("pill") {
                position_pill_on_monitor(app.handle(), initial_monitor);
                #[cfg(target_os = "macos")]
                window_ext::configure_non_activating_panel(&window);
            }

            let resource_dir = app.path().resource_dir().ok();
            let ort_dylib = ort_init::init(resource_dir.as_deref())
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            log::info!("ONNX Runtime initialized from {}", ort_dylib.display());
            let parakeet_model_dir = parakeet::find_model_dir(resource_dir.as_deref())
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            log::info!("Parakeet model found at {}", parakeet_model_dir.display());
            parakeet::init_from_dir(&parakeet_model_dir)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            std::thread::spawn(|| {
                if let Err(e) = parakeet::warm_up() {
                    log::error!("Parakeet warm-up failed: {e}");
                }
            });
            repo_index::start_background_index();
            match repo_index::start_fs_watcher() {
                Ok(watcher) => {
                    // Leak the watcher so it lives for the app's lifetime
                    std::mem::forget(watcher);
                }
                Err(e) => log::warn!("FS watcher failed to start: {}", e),
            }
            // Shared flag so hotkey callback knows when recording is active
            let is_recording = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            app.manage(is_recording.clone());

            // Start global hotkey listener
            let app_handle = app.handle().clone();
            let stop_signal = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            hotkey::start_hotkey_listener(app_handle, stop_signal.clone(), is_recording);

            // Handle hotkey-toggle: cycle Idle -> Recording -> Processing
            let handle = app.handle().clone();
            app.listen("hotkey-toggle", move |_event| {
                let state = handle.state::<std::sync::Mutex<state::AppState>>();
                let current_state = state.lock().unwrap().recording_state;
                match current_state {
                    state::RecordingState::Idle => start_recording(&handle),
                    state::RecordingState::Recording => stop_recording(&handle),
                    state::RecordingState::Processing => {
                        let mut s = state.lock().unwrap();
                        s.recording_state = state::RecordingState::Idle;
                        log::info!("Processing cancelled by user");
                        handle
                            .state::<std::sync::Arc<state::SessionState>>()
                            .signal_cancel();
                        discard_session(&handle);
                        if let Some(window) = handle.get_webview_window("pill") {
                            let _ = window.hide();
                        }
                        let is_recording = handle.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
                        is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
                        let _ = handle.emit("recording-state", s.recording_state);
                    }
                }
            });

            // Handle hotkey-start: Cmd+Option+C voice copy
            let handle_start = app.handle().clone();
            app.listen("hotkey-start", move |_event| {
                start_recording(&handle_start);
            });

            // Handle hotkey-stop: Cmd+V while recording
            let handle_stop = app.handle().clone();
            app.listen("hotkey-stop", move |_event| {
                stop_recording(&handle_stop);
            });

            // Handle hotkey-cancel: cancel recording if active
            let handle2 = app.handle().clone();
            app.listen("hotkey-cancel", move |_event| {
                let state = handle2.state::<std::sync::Mutex<state::AppState>>();
                let mut s = state.lock().unwrap();
                if matches!(
                    s.recording_state,
                    state::RecordingState::Recording | state::RecordingState::Processing
                ) {
                    s.recording_state = state::RecordingState::Idle;
                    log::info!("Recording cancelled");
                    if let Some(window) = handle2.get_webview_window("pill") {
                        let _ = window.hide();
                    }
                    let session = handle2.state::<std::sync::Arc<state::SessionState>>();
                    session.signal_cancel();
                    discard_session(&handle2);
                    let is_recording = handle2.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
                    is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
                    let _ = handle2.emit("recording-state", s.recording_state);
                }
            });

            log::info!("Voxcode started");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building Voxcode")
        .run(|_app, event| {
            // Prevent app from exiting when the pill window is hidden.
            // This is a menu bar app — it should keep running with just the tray icon.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
