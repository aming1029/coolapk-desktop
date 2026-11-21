use super::*;

pub(crate) static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(false);
pub(crate) static START_MINIMIZED: AtomicBool = AtomicBool::new(false);
pub(crate) static REMEMBER_WINDOW_STATE: AtomicBool = AtomicBool::new(true);
pub(crate) static ALWAYS_ON_TOP: AtomicBool = AtomicBool::new(false);

/// 恢复主窗口，供托盘、单实例和 macOS Dock 重新打开事件共用。
#[cfg(desktop)]
pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WindowState {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(desktop)]
pub(crate) struct ScreenRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

pub(crate) static WINDOW_STATE: Mutex<Option<WindowState>> = Mutex::new(None);
// 最大化状态独立保存，WINDOW_STATE 始终保留可还原的普通窗口几何信息。
pub(crate) static WINDOW_MAXIMIZED: AtomicBool = AtomicBool::new(false);
// 拖动窗口会连续触发 Moved/Resized；用修订号合并事件，在最后一次变化后再落盘。
#[cfg(desktop)]
pub(crate) static WINDOW_STATE_REVISION: AtomicU64 = AtomicU64::new(0);
#[cfg(desktop)]
pub(crate) static WINDOW_STATE_SAVE_SCHEDULED: AtomicBool = AtomicBool::new(false);
pub(crate) static STARTUP_STATE_LOCK: Mutex<()> = Mutex::new(());
pub(crate) static STARTUP_STATE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) const STARTUP_STATE_FILE: &str = "startup_state.json";
pub(crate) const MIN_WINDOW_W: u32 = 800;
pub(crate) const MIN_WINDOW_H: u32 = 600;

pub(crate) fn startup_state_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    Some(dir.join(STARTUP_STATE_FILE))
}

pub(crate) fn update_startup_state_file<F>(
    path: &std::path::Path,
    fallback: serde_json::Value,
    update: F,
) -> Result<(), String>
where
    F: FnOnce(&mut serde_json::Value),
{
    let _lock = STARTUP_STATE_LOCK
        .lock()
        .map_err(|_| "启动状态锁已损坏".to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let mut state: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(fallback);
    if !state.is_object() {
        state = serde_json::json!({});
    }
    update(&mut state);
    let raw = serde_json::to_string_pretty(&state).map_err(|error| error.to_string())?;
    atomic_write_startup_state(path, raw.as_bytes())
}

pub(crate) fn atomic_write_startup_state(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let file_name = path
        .file_name()
        .ok_or_else(|| "启动状态文件路径无效".to_string())?;
    let sequence = STARTUP_STATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut temp_name = file_name.to_os_string();
    temp_name.push(format!(".{}-{nonce}-{sequence}.tmp", std::process::id()));
    let temp_path = path.with_file_name(temp_name);
    let mut temp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| format!("创建启动状态临时文件失败：{error}"))?;
    if let Err(error) = temp_file
        .write_all(bytes)
        .and_then(|_| temp_file.flush())
        .and_then(|_| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("写入启动状态临时文件失败：{error}"));
    }
    drop(temp_file);

    if let Err(error) = replace_startup_state_file(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn replace_startup_state_file(temp_path: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    std::fs::rename(temp_path, path).map_err(|error| format!("替换启动状态文件失败：{error}"))
}

#[cfg(target_os = "windows")]
pub(crate) fn replace_startup_state_file(temp_path: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let source: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| format!("替换启动状态文件失败：{error}"))
}

/// 合并写入窗口几何信息，保留已有的启动参数（静默启动/记忆窗口/置顶）
pub(crate) fn persist_window_geometry(app: &tauri::AppHandle, state: WindowState) {
    let path = match startup_state_path(app) {
        Some(p) => p,
        None => return,
    };
    let fallback = serde_json::json!({
        "start_minimized": START_MINIMIZED.load(Ordering::SeqCst),
        "remember_window_state": REMEMBER_WINDOW_STATE.load(Ordering::SeqCst),
        "always_on_top": ALWAYS_ON_TOP.load(Ordering::SeqCst)
    });
    if let Err(error) = update_startup_state_file(&path, fallback, |flags| {
        write_saved_window_state(flags, state, WINDOW_MAXIMIZED.load(Ordering::SeqCst))
    }) {
        log::warn!("保存窗口状态失败：{error}");
    }
}

// 最大化标记与普通窗口几何信息一起写入，保留同一文件里的其他启动设置。
pub(crate) fn write_saved_window_state(flags: &mut serde_json::Value, state: WindowState, maximized: bool) {
    flags["x"] = serde_json::json!(state.x);
    flags["y"] = serde_json::json!(state.y);
    flags["w"] = serde_json::json!(state.w);
    flags["h"] = serde_json::json!(state.h);
    flags["maximized"] = serde_json::json!(maximized);
}

pub(crate) fn cached_window_state() -> Option<WindowState> {
    WINDOW_STATE
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .filter(|state| state.w >= MIN_WINDOW_W && state.h >= MIN_WINDOW_H)
}

#[cfg(desktop)]
pub(crate) fn parse_saved_window_state(flags: &serde_json::Value) -> Option<WindowState> {
    let x: i32 = flags.get("x")?.as_i64()?.try_into().ok()?;
    let y: i32 = flags.get("y")?.as_i64()?.try_into().ok()?;
    let w: u32 = flags.get("w")?.as_u64()?.try_into().ok()?;
    let h: u32 = flags.get("h")?.as_u64()?.try_into().ok()?;
    if w < MIN_WINDOW_W || h < MIN_WINDOW_H {
        return None;
    }
    Some(WindowState { x, y, w, h })
}

#[cfg(desktop)]
pub(crate) fn intersection_size(window: WindowState, screen: ScreenRect) -> (u32, u32) {
    let left = i64::from(window.x).max(i64::from(screen.x));
    let top = i64::from(window.y).max(i64::from(screen.y));
    let right = (i64::from(window.x) + i64::from(window.w))
        .min(i64::from(screen.x) + i64::from(screen.w));
    let bottom = (i64::from(window.y) + i64::from(window.h))
        .min(i64::from(screen.y) + i64::from(screen.h));
    (
        right.saturating_sub(left).max(0) as u32,
        bottom.saturating_sub(top).max(0) as u32,
    )
}

#[cfg(desktop)]
pub(crate) fn restore_window_rect(
    saved: WindowState,
    screens: &[ScreenRect],
    primary: Option<ScreenRect>,
) -> WindowState {
    const MIN_VISIBLE: u32 = 64;
    if screens.is_empty() {
        return saved;
    }

    let visible_screen = screens
        .iter()
        .copied()
        .filter_map(|screen| {
            let (visible_w, visible_h) = intersection_size(saved, screen);
            (visible_w >= MIN_VISIBLE && visible_h >= MIN_VISIBLE)
                .then_some((u64::from(visible_w) * u64::from(visible_h), screen))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, screen)| screen);
    let screen = visible_screen.or(primary).unwrap_or(screens[0]);
    let width = saved.w.clamp(MIN_WINDOW_W, screen.w.max(MIN_WINDOW_W));
    let height = saved.h.clamp(MIN_WINDOW_H, screen.h.max(MIN_WINDOW_H));

    let (x, y) = if visible_screen.is_some() {
        let min_x = i64::from(screen.x) - i64::from(width) + i64::from(MIN_VISIBLE);
        let max_x = i64::from(screen.x) + i64::from(screen.w) - i64::from(MIN_VISIBLE);
        let min_y = i64::from(screen.y) - i64::from(height) + i64::from(MIN_VISIBLE);
        let max_y = i64::from(screen.y) + i64::from(screen.h) - i64::from(MIN_VISIBLE);
        (
            i64::from(saved.x).clamp(min_x, max_x) as i32,
            i64::from(saved.y).clamp(min_y, max_y) as i32,
        )
    } else {
        (
            (i64::from(screen.x) + (i64::from(screen.w) - i64::from(width)) / 2) as i32,
            (i64::from(screen.y) + (i64::from(screen.h) - i64::from(height)) / 2) as i32,
        )
    };

    WindowState {
        x,
        y,
        w: width,
        h: height,
    }
}

pub(crate) fn persist_current_window_geometry(app: &tauri::AppHandle) {
    if !REMEMBER_WINDOW_STATE.load(Ordering::SeqCst) {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // 最小化到任务栏时保留此前状态，避免将最小化尺寸或最大化尺寸写入普通窗口记录。
    let minimized = window.is_minimized().unwrap_or(false);
    if !minimized {
        if let Ok(maximized) = window.is_maximized() { WINDOW_MAXIMIZED.store(maximized, Ordering::SeqCst); }
    }
    let state = if minimized || WINDOW_MAXIMIZED.load(Ordering::SeqCst) {
        cached_window_state()
    } else { match (window.outer_position(), window.inner_size()) {
        (Ok(pos), Ok(size)) if size.width >= MIN_WINDOW_W && size.height >= MIN_WINDOW_H => {
            Some(WindowState {
                x: pos.x,
                y: pos.y,
                w: size.width,
                h: size.height,
            })
        }
        _ => cached_window_state(),
    } };
    if let Some(state) = state {
        persist_window_geometry(app, state);
    }
}

#[cfg(desktop)]
pub(crate) fn schedule_window_state_persistence(app: tauri::AppHandle) {
    if !REMEMBER_WINDOW_STATE.load(Ordering::SeqCst) {
        return;
    }

    WINDOW_STATE_REVISION.fetch_add(1, Ordering::SeqCst);
    if WINDOW_STATE_SAVE_SCHEDULED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    tauri::async_runtime::spawn(async move {
        loop {
            let revision = WINDOW_STATE_REVISION.load(Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            if WINDOW_STATE_REVISION.load(Ordering::SeqCst) != revision {
                continue;
            }

            // 先解除排队标记，避免保存期间恰好到来的最后一次事件丢失。
            WINDOW_STATE_SAVE_SCHEDULED.store(false, Ordering::SeqCst);
            persist_current_window_geometry(&app);

            if WINDOW_STATE_REVISION.load(Ordering::SeqCst) == revision {
                return;
            }
            if WINDOW_STATE_SAVE_SCHEDULED
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return;
            }
        }
    });
}

#[cfg(test)]
mod window_state_tests {
    use super::{
        MIN_WINDOW_H, MIN_WINDOW_W, ScreenRect, WindowState, parse_saved_window_state,
        restore_window_rect, update_startup_state_file, write_saved_window_state,
    };

    #[test]
    fn saves_maximized_state_without_losing_normal_geometry_or_preferences() {
        let normal = WindowState { x: 120, y: 80, w: 1200, h: 700 };
        let mut flags = serde_json::json!({"remember_window_state": true, "start_minimized": true});
        write_saved_window_state(&mut flags, normal, true);
        let mut restored: serde_json::Value = serde_json::from_str(&flags.to_string()).unwrap();
        assert_eq!(restored["maximized"], true);
        assert_eq!(parse_saved_window_state(&restored), Some(normal));
        assert_eq!(restored["start_minimized"], true);
        write_saved_window_state(&mut restored, normal, false);
        assert_eq!(restored["maximized"], false);
        assert_eq!(parse_saved_window_state(&restored), Some(normal));
    }

    #[test]
    fn parses_valid_window_geometry() {
        let value = serde_json::json!({"x": -120, "y": 45, "w": 1200, "h": 700});
        assert_eq!(parse_saved_window_state(&value), Some(WindowState { x: -120, y: 45, w: 1200, h: 700 }));
    }

    #[test]
    fn rejects_invalid_or_too_small_window_geometry() {
        let too_small = serde_json::json!({"x": 0, "y": 0, "w": MIN_WINDOW_W - 1, "h": MIN_WINDOW_H});
        let wrong_type = serde_json::json!({"x": 0.5, "y": 0, "w": MIN_WINDOW_W, "h": MIN_WINDOW_H});
        assert!(parse_saved_window_state(&too_small).is_none());
        assert!(parse_saved_window_state(&wrong_type).is_none());
    }

    #[test]
    fn preserves_visible_window_geometry() {
        let screen = ScreenRect { x: 0, y: 0, w: 1920, h: 1080 };
        let saved = WindowState { x: 120, y: 80, w: 1200, h: 700 };
        assert_eq!(restore_window_rect(saved, &[screen], Some(screen)), saved);
    }

    #[test]
    fn recenters_window_that_is_completely_off_screen() {
        let screen = ScreenRect { x: 0, y: 0, w: 1920, h: 1080 };
        let saved = WindowState { x: 3000, y: 200, w: 1200, h: 700 };
        assert_eq!(
            restore_window_rect(saved, &[screen], Some(screen)),
            WindowState { x: 360, y: 190, w: 1200, h: 700 }
        );
    }

    #[test]
    fn startup_state_updates_preserve_geometry_and_flags() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-startup-state-{unique}"));
        let path = root.join("startup_state.json");

        update_startup_state_file(&path, serde_json::json!({}), |state| {
            state["x"] = serde_json::json!(100);
            state["y"] = serde_json::json!(80);
            state["w"] = serde_json::json!(1200);
            state["h"] = serde_json::json!(700);
        })
        .unwrap();
        update_startup_state_file(&path, serde_json::json!({}), |state| {
            state["start_minimized"] = serde_json::json!(true);
            state["remember_window_state"] = serde_json::json!(true);
            state["always_on_top"] = serde_json::json!(false);
        })
        .unwrap();

        let state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(state["x"], 100);
        assert_eq!(state["w"], 1200);
        assert_eq!(state["start_minimized"], true);
        assert_eq!(state["remember_window_state"], true);
        let _ = std::fs::remove_dir_all(root);
    }
}
