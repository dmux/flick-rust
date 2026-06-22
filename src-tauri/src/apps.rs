use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::OnceLock;
use base64::Engine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: String, // Data URL or icon name
}

static ICON_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn get_icon_cache() -> &'static Mutex<HashMap<String, String>> {
    ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// Data URL generator
fn file_to_data_url(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        _ => return None,
    };
    let data = fs::read(path).ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    Some(format!("data:{};base64,{}", mime, encoded))
}

// ---------------- LINUX ----------------

#[cfg(target_os = "linux")]
fn get_linux_icon_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    if !home.is_empty() {
        dirs.push(PathBuf::from(&home).join(".icons"));
        let data_home = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&home).join(".local/share"));
        dirs.push(data_home.join("icons"));
    }

    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for d in data_dirs.split(':') {
        if !d.is_empty() {
            dirs.push(PathBuf::from(d).join("icons"));
        }
    }
    dirs.push(PathBuf::from("/usr/share/pixmaps"));

    dirs.into_iter().filter(|d| d.exists()).collect()
}

#[cfg(target_os = "linux")]
fn find_linux_icon_file(name: &str, base_dirs: &[PathBuf]) -> Option<PathBuf> {
    let renderable = ["png", "svg", "jpg", "jpeg", "gif"];
    let mut matches = Vec::new();

    fn walk(dir: &Path, name: &str, renderable: &[&str], matches: &mut Vec<PathBuf>, depth: usize) {
        if depth > 5 || matches.len() > 200 {
            return;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, name, renderable, matches, depth + 1);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if renderable.contains(&ext_lower.as_str()) {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if stem == name {
                                matches.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    for base in base_dirs {
        walk(base, name, &renderable, &mut matches, 0);
    }

    if matches.is_empty() {
        return None;
    }

    // Score and sort matches
    // Scoring: PNGs win (3), SVGs (2), Others (1)
    // Secondary score: largest size based on (\d+)x\d+ pattern
    let size_of = |p: &Path| -> u32 {
        if let Some(p_str) = p.to_str() {
            // Find size like 256x256
            if let Some(idx) = p_str.find('x') {
                let before = &p_str[..idx];
                // scan backward for digits
                let digits: String = before.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    let normal: String = digits.chars().rev().collect();
                    if let Ok(sz) = normal.parse::<u32>() {
                        return sz;
                    }
                }
            }
        }
        0
    };

    matches.sort_by(|a, b| {
        let ext_a = a.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
        let ext_b = b.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
        let rank = |e: &str| match e {
            "png" => 3,
            "svg" => 2,
            _ => 1,
        };
        let r_a = rank(&ext_a);
        let r_b = rank(&ext_b);
        if r_a != r_b {
            r_b.cmp(&r_a) // Descending
        } else {
            size_of(b).cmp(&size_of(a)) // Descending
        }
    });

    matches.first().cloned()
}

#[cfg(target_os = "linux")]
fn get_linux_apps() -> Vec<AppInfo> {
    let mut apps = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
    }

    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg.split(':') {
            let app_dir = PathBuf::from(dir).join("applications");
            if !dirs.contains(&app_dir) {
                dirs.push(app_dir);
            }
        }
    }

    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if !file_name_str.ends_with(".desktop") {
                    continue;
                }
                if seen_ids.contains(&*file_name_str) {
                    continue;
                }

                let path = entry.path();
                if let Ok(content) = fs::read_to_string(&path) {
                    let mut is_desktop_entry = false;
                    let mut name = String::new();
                    let mut exec = String::new();
                    let mut icon = String::new();
                    let mut no_display = false;
                    let mut hidden = false;

                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed == "[Desktop Entry]" {
                            is_desktop_entry = true;
                            continue;
                        }
                        if trimmed.starts_with('[') && trimmed != "[Desktop Entry]" {
                            is_desktop_entry = false;
                        }

                        if is_desktop_entry {
                            if let Some(val) = trimmed.strip_prefix("Name=") {
                                name = val.trim().to_string();
                            } else if let Some(val) = trimmed.strip_prefix("Exec=") {
                                exec = val.trim().to_string();
                            } else if let Some(val) = trimmed.strip_prefix("Icon=") {
                                icon = val.trim().to_string();
                            } else if let Some(val) = trimmed.strip_prefix("NoDisplay=") {
                                no_display = val.trim().to_lowercase() == "true";
                            } else if let Some(val) = trimmed.strip_prefix("Hidden=") {
                                hidden = val.trim().to_lowercase() == "true";
                            }
                        }
                    }

                    if !name.is_empty() && !exec.is_empty() && !no_display && !hidden {
                        // clean exec params like %f, %F, %u, %U, %i, %d, %D, %n, %N, %v, %m, %k, %s
                        let re = regex::Regex::new(r"%[fFuiUdDnNvmks]").unwrap();
                        let clean_exec = re.replace_all(&exec, "").trim().to_string();

                        apps.push(AppInfo {
                            id: file_name_str.to_string(),
                            name,
                            exec: clean_exec,
                            icon: if icon.is_empty() { "system-run".to_string() } else { icon },
                        });
                        seen_ids.insert(file_name_str.to_string());
                    }
                }
            }
        }
    }

    apps
}

// ---------------- MACOS ----------------

#[cfg(target_os = "macos")]
fn get_mac_apps() -> Vec<AppInfo> {
    let mut apps = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let search_dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from(&home).join("Applications"),
    ];

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    apps.push(AppInfo {
                        id: path.file_name().unwrap().to_string_lossy().to_string(),
                        name,
                        exec: path.to_string_lossy().to_string(),
                        icon: "app".to_string(),
                    });
                }
            }
        }
    }
    apps
}

#[cfg(target_os = "macos")]
fn resolve_mac_icon(app_path: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let app_path_buf = PathBuf::from(app_path);
    let res_dir = app_path_buf.join("Contents/Resources");
    if !res_dir.exists() {
        return "".to_string();
    }

    let mut icns = PathBuf::new();

    // Try reading Info.plist
    let plist_path = app_path_buf.join("Contents/Info.plist");
    if plist_path.exists() {
        if let Ok(output) = Command::new("plutil")
            .args(["-extract", "CFBundleIconFile", "raw", "-o", "-", plist_path.to_str().unwrap()])
            .output()
        {
            let mut name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                if !name.to_lowercase().ends_with(".icns") {
                    name.push_str(".icns");
                }
                let candidate = res_dir.join(&name);
                if candidate.exists() {
                    icns = candidate;
                }
            }
        }
    }

    // Fallback: search Resources for any .icns file
    if icns.as_os_str().is_empty() {
        if let Ok(entries) = fs::read_dir(&res_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("icns") {
                    icns = p;
                    break;
                }
            }
        }
    }

    if icns.as_os_str().is_empty() || !icns.exists() {
        return "".to_string();
    }

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp_png = std::env::temp_dir().join(format!("flick-icon-{}-{}.png", std::process::id(), id));

    // run sips to convert icns to png
    let sips_status = Command::new("sips")
        .args([
            "-s",
            "format",
            "png",
            icns.to_str().unwrap(),
            "--out",
            tmp_png.to_str().unwrap(),
            "-Z",
            "128",
        ])
        .status();

    if let Ok(status) = sips_status {
        if status.success() {
            if let Some(data_url) = file_to_data_url(&tmp_png) {
                let _ = fs::remove_file(tmp_png);
                return data_url;
            }
        }
    }

    let _ = fs::remove_file(tmp_png);
    "".to_string()
}

// ---------------- WINDOWS ----------------

#[cfg(target_os = "windows")]
fn get_files_recursively(dir: &Path, ext: &str, max_depth: usize, current_depth: usize) -> Vec<PathBuf> {
    if current_depth > max_depth {
        return Vec::new();
    }
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(get_files_recursively(&path, ext, max_depth, current_depth + 1));
            } else if let Some(e) = path.extension().and_then(|x| x.to_str()) {
                if e.eq_ignore_ascii_case(ext) {
                    results.push(path);
                }
            }
        }
    }
    results
}

#[cfg(target_os = "windows")]
fn get_windows_apps() -> Vec<AppInfo> {
    let mut apps = Vec::new();
    let mut search_dirs = Vec::new();

    // Global start menu
    let common_start = PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs");
    if common_start.exists() {
        search_dirs.push(common_start);
    }

    // User start menu
    let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
    if !user_profile.is_empty() {
        let user_start = PathBuf::from(user_profile)
            .join("AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs");
        if user_start.exists() {
            search_dirs.push(user_start);
        }
    }

    for dir in search_dirs {
        let lnk_files = get_files_recursively(&dir, "lnk", 3, 0);
        for lnk in lnk_files {
            if let Some(stem) = lnk.file_stem().and_then(|s| s.to_str()) {
                apps.push(AppInfo {
                    id: lnk.to_string_lossy().to_string(),
                    name: stem.to_string(),
                    exec: lnk.to_string_lossy().to_string(),
                    icon: "shortcut".to_string(),
                });
            }
        }
    }

    apps
}

// ---------------- CROSS PLATFORM WRAPPERS ----------------

pub fn list_installed_apps() -> Vec<AppInfo> {
    #[cfg(target_os = "linux")]
    let mut apps = get_linux_apps();
    #[cfg(target_os = "macos")]
    let mut apps = get_mac_apps();
    #[cfg(target_os = "windows")]
    let mut apps = get_windows_apps();
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let mut apps = Vec::new();
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

pub fn resolve_app_icon(app_info: &AppInfo) -> String {
    let cache_key = format!("{}:{}", app_info.id, app_info.icon);
    {
        let cache = get_icon_cache().lock().unwrap();
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let mut result = "".to_string();

    #[cfg(target_os = "linux")]
    {
        let raw = &app_info.icon;
        let raw_path = Path::new(raw);
        if !raw.is_empty() && raw_path.is_absolute() && raw_path.exists() {
            result = file_to_data_url(raw_path).unwrap_or_default();
        } else if !raw.is_empty() {
            // Strip extensions like .png, .svg
            let name = regex::Regex::new(r"\.(png|svg|xpm|jpg|jpeg|gif|ico)$")
                .unwrap()
                .replace(raw, "")
                .to_string();
            let icon_dirs = get_linux_icon_dirs();
            if let Some(file) = find_linux_icon_file(&name, &icon_dirs) {
                result = file_to_data_url(&file).unwrap_or_default();
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        result = resolve_mac_icon(&app_info.exec);
    }

    #[cfg(target_os = "windows")]
    {
        // For Windows icons, we can either extract using a custom library,
        // or since we are in Tauri, we could let Tauri front-end or another tool load it.
        // For completeness, we can implement extraction or return placeholder.
        // Let's use standard Tauri/Windows icon extraction if we can, or just empty.
    }

    let mut cache = get_icon_cache().lock().unwrap();
    cache.insert(cache_key, result.clone());
    result
}

pub fn list_installed_apps_with_icons() -> Vec<AppInfo> {
    let apps = list_installed_apps();
    apps.into_iter()
        .map(|mut app| {
            app.icon = resolve_app_icon(&app);
            app
        })
        .collect()
}

pub fn resolve_path_icon(target_path: &str) -> String {
    let cache_key = format!("path:{}", target_path);
    {
        let cache = get_icon_cache().lock().unwrap();
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    #[cfg(target_os = "macos")]
    let result = resolve_mac_icon(target_path);
    #[cfg(not(target_os = "macos"))]
    let result = "".to_string();
    // For other platforms, we can also extract or use file icon resolver.
    // E.g. on Linux, if it's an app path, it may not have a simple resolution
    // unless mapped, but we can do a best effort.

    let mut cache = get_icon_cache().lock().unwrap();
    cache.insert(cache_key, result.clone());
    result
}

pub fn launch_app(exec_cmd: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").args(["-a", exec_cmd]).spawn();
    }

    #[cfg(target_os = "windows")]
    {
        // open the lnk or exe
        let _ = open::that(exec_cmd);
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: use sh -c to correctly handle arguments, env prefixes, snap/flatpak wrappers, etc.
        let _ = Command::new("sh").args(["-c", exec_cmd]).spawn();
    }
}

pub fn launch_url(url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = open::that(url);
    }
}

pub struct ProcessAppLauncher;

impl crate::ports::AppLauncher for ProcessAppLauncher {
    fn launch_app(&self, exec_cmd: &str) {
        launch_app(exec_cmd);
    }
    fn launch_url(&self, url: &str) {
        launch_url(url);
    }
    fn list_apps(&self) -> Vec<AppInfo> {
        list_installed_apps_with_icons()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_apps() {
        let apps = list_installed_apps_with_icons();
        // Since we are running on a Linux test container/system, it might have apps or not,
        // but we verify that the call executes and returns a vector without panic.
        let _ = apps.len();
    }

    #[test]
    fn test_launch_url_no_panic() {
        // Verify launch_url doesn't panic when given a URL
        launch_url("https://localhost");
    }

    #[test]
    fn test_launch_app_no_panic() {
        // Verify launch_app doesn't panic
        launch_app("echo 1");
    }
}

