use std::env;
use auto_launch::AutoLaunchBuilder;

pub fn set_autostart_enabled(enabled: bool) {
    let current_exe = env::current_exe().unwrap_or_default();
    let exe_str = current_exe.to_string_lossy().to_string();
    if exe_str.is_empty() {
        return;
    }

    #[cfg(target_os = "linux")]
    let args = vec!["--ozone-platform=x11".to_string(), "--hidden".to_string()];
    #[cfg(not(target_os = "linux"))]
    let args = vec!["--hidden".to_string()];

    let auto = AutoLaunchBuilder::new()
        .set_app_name("Flick")
        .set_app_path(&exe_str)
        .set_args(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .build();

    if let Ok(auto) = auto {
        if enabled {
            let _ = auto.enable();
        } else {
            let _ = auto.disable();
        }
    }
}

pub fn is_autostart_enabled() -> bool {
    let current_exe = env::current_exe().unwrap_or_default();
    let exe_str = current_exe.to_string_lossy().to_string();
    if exe_str.is_empty() {
        return false;
    }

    #[cfg(target_os = "linux")]
    let args = vec!["--ozone-platform=x11".to_string(), "--hidden".to_string()];
    #[cfg(not(target_os = "linux"))]
    let args = vec!["--hidden".to_string()];

    let auto = AutoLaunchBuilder::new()
        .set_app_name("Flick")
        .set_app_path(&exe_str)
        .set_args(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .build();

    if let Ok(auto) = auto {
        auto.is_enabled().unwrap_or(false)
    } else {
        false
    }
}

pub struct AutoLaunchManager;

impl crate::ports::AutostartManager for AutoLaunchManager {
    fn set_enabled(&self, enabled: bool) {
        set_autostart_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        is_autostart_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_checks() {
        let initial_state = is_autostart_enabled();
        // Toggle state and verify
        set_autostart_enabled(!initial_state);
        let toggled_state = is_autostart_enabled();
        assert_eq!(toggled_state, !initial_state);

        // Restore state
        set_autostart_enabled(initial_state);
        assert_eq!(is_autostart_enabled(), initial_state);
    }
}

