const PORTABLE_UPDATE_HELPER_ARG: &str = "--coolapk-apply-portable-update";

/// 单文件版更新时，新版本可执行文件先作为极小的更新助手启动。
/// 它在旧进程退出、目标文件解除锁定后替换原文件并重新启动应用。
pub fn try_run_portable_update_helper() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(PORTABLE_UPDATE_HELPER_ARG)) {
        return false;
    }

    #[cfg(target_os = "windows")]
    if let (Some(parent_pid), Some(target)) = (args.next(), args.next()) {
        let parent_pid = parent_pid.to_string_lossy().parse::<u32>();
        if let Ok(parent_pid) = parent_pid {
            let target = std::path::PathBuf::from(target);
            if let Err(error) = apply_portable_update(parent_pid, target.clone()) {
                let log_path = std::env::temp_dir().join("coolapk-desktop-portable-update.log");
                let _ = std::fs::write(log_path, error);
                let _ = std::process::Command::new(target).spawn();
            }
        }
    }
    true
}

#[cfg(target_os = "windows")]
fn apply_portable_update(parent_pid: u32, target: std::path::PathBuf) -> Result<(), String> {
    use std::time::Duration;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};

    let staged = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| format!("无法定位便携版更新文件：{error}"))?;
    let update_dir = std::env::temp_dir().join("coolapk-desktop-update");
    let update_dir = update_dir
        .canonicalize()
        .map_err(|error| format!("更新目录不存在：{error}"))?;
    if !staged.starts_with(&update_dir) {
        return Err("拒绝从应用更新目录外执行便携版替换".to_string());
    }

    let target = target
        .canonicalize()
        .map_err(|error| format!("无法定位待更新程序：{error}"))?;
    if target == staged
        || !target
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("exe"))
    {
        return Err("便携版更新目标无效".to_string());
    }

    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "便携版更新目标文件名无效".to_string())?;
    let nonce = std::process::id();
    let replacement = target.with_file_name(format!(".{file_name}.update-{nonce}"));
    let backup = target.with_file_name(format!(".{file_name}.backup-{nonce}"));
    std::fs::copy(&staged, &replacement)
        .map_err(|error| format!("准备便携版更新失败：{error}"))?;

    // 等待旧进程彻底退出，避免重启后的新进程被旧版单实例插件拦截。
    unsafe {
        if let Ok(process) = OpenProcess(PROCESS_SYNCHRONIZE, false, parent_pid) {
            let wait_result = WaitForSingleObject(process, 120_000);
            let _ = CloseHandle(process);
            if wait_result != WAIT_OBJECT_0 {
                let _ = std::fs::remove_file(&replacement);
                return Err("等待旧版退出超时".to_string());
            }
        }
    }

    let mut last_error = None;
    for _ in 0..40 {
        match std::fs::rename(&target, &backup) {
            Ok(()) => {
                if let Err(error) = std::fs::rename(&replacement, &target) {
                    let _ = std::fs::rename(&backup, &target);
                    let _ = std::fs::remove_file(&replacement);
                    return Err(format!("替换便携版失败：{error}"));
                }
                if let Err(error) = std::process::Command::new(&target).spawn() {
                    let _ = std::fs::remove_file(&target);
                    let _ = std::fs::rename(&backup, &target);
                    return Err(format!("重新启动新版失败，已回滚：{error}"));
                }
                let _ = std::fs::remove_file(&backup);
                return Ok(());
            }
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    let _ = std::fs::remove_file(&replacement);
    Err(format!(
        "等待旧版退出超时：{}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "目标文件仍被占用".to_string())
    ))
}
