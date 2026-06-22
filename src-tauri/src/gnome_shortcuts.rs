use std::process::Command;
use std::env;
use std::path::PathBuf;

const SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const LIST_KEY: &str = "custom-keybindings";
const BASE_PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings";
const OWNED_PREFIX: &str = "flick-";

pub fn is_gnome_session() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
    #[cfg(target_os = "linux")]
    {
        let desktop = env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| env::var("XDG_SESSION_DESKTOP"))
            .unwrap_or_default()
            .to_lowercase();
        
        ["gnome", "unity", "pop", "ubuntu", "cinnamon"]
            .iter()
            .any(|d| desktop.contains(d))
    }
}

fn dconf_path_for(key: &str) -> String {
    format!("{}/{}{}/", BASE_PATH, OWNED_PREFIX, key)
}

fn relocatable_schema(dconf_path: &str) -> String {
    format!("{}.custom-keybinding:{}", SCHEMA, dconf_path)
}

fn trigger_command(key: &str) -> String {
    let exe = env::var("APPIMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::current_exe().unwrap_or_default());
    let exe_str = exe.to_string_lossy().to_string();
    // Quote single quotes inside path
    let escaped_exe = exe_str.replace('\'', "'\\''");
    format!("'{}' --trigger {}", escaped_exe, key)
}

fn gvariant_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn gsettings_get(schema: &str, key: &str) -> Result<String, std::io::Error> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn gsettings_set(schema: &str, key: &str, value: &str) -> Result<(), std::io::Error> {
    Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()?;
    Ok(())
}

fn parse_list(raw: &str) -> Vec<String> {
    let mut results = Vec::new();
    let re = regex::Regex::new(r"'([^']*)'").unwrap();
    for cap in re.captures_iter(raw) {
        results.push(cap[1].to_string());
    }
    results
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        "@as []".to_string()
    } else {
        let quoted: Vec<String> = items.iter().map(|i| format!("'{}'", i)).collect();
        format!("[{}]", quoted.join(", "))
    }
}

fn is_owned(dconf_path: &str) -> bool {
    dconf_path.contains(&format!("/{}", OWNED_PREFIX))
}

fn reset_entry(dconf_path: &str) {
    let schema = relocatable_schema(dconf_path);
    for k in ["name", "command", "binding"] {
        let _ = Command::new("gsettings")
            .args(["reset", &schema, k])
            .status();
    }
}

pub fn sync_gnome_shortcuts(keys: &[String]) -> Result<(), std::io::Error> {
    if !is_gnome_session() {
        return Ok(());
    }

    let wanted: Vec<String> = keys.iter().map(|k| k.to_uppercase()).collect();
    let wanted_paths: std::collections::HashSet<String> = wanted.iter().map(|k| dconf_path_for(k)).collect();

    let raw_list = gsettings_get(SCHEMA, LIST_KEY)?;
    let current = parse_list(&raw_list);
    let foreign: Vec<String> = current.iter().filter(|p| !is_owned(p)).cloned().collect();
    let stale_owned: Vec<String> = current.iter().filter(|p| is_owned(p) && !wanted_paths.contains(*p)).cloned().collect();

    for p in stale_owned {
        reset_entry(&p);
    }

    for key in &wanted {
        let path = dconf_path_for(key);
        let schema = relocatable_schema(&path);
        let _ = gsettings_set(&schema, "name", &gvariant_string(&format!("Flick {}", key)));
        let _ = gsettings_set(&schema, "command", &gvariant_string(&trigger_command(key)));
        let _ = gsettings_set(&schema, "binding", &gvariant_string(key));
    }

    let mut new_list = foreign;
    new_list.extend(wanted.iter().map(|k| dconf_path_for(k)));

    gsettings_set(SCHEMA, LIST_KEY, &format_list(&new_list))?;
    println!("[gnome] global shortcuts synced: {:?}", wanted);
    Ok(())
}

pub fn clear_gnome_shortcuts_sync() {
    if !is_gnome_session() {
        return;
    }
    if let Ok(raw_list) = gsettings_get(SCHEMA, LIST_KEY) {
        let current = parse_list(&raw_list);
        let owned: Vec<String> = current.iter().filter(|p| is_owned(p)).cloned().collect();
        if owned.is_empty() {
            return;
        }
        let foreign: Vec<String> = current.iter().filter(|p| !is_owned(p)).cloned().collect();
        for p in owned {
            reset_entry(&p);
        }
        let _ = gsettings_set(SCHEMA, LIST_KEY, &format_list(&foreign));
        println!("[gnome] global shortcuts cleared");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helpers() {
        assert_eq!(dconf_path_for("F17"), "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/flick-F17/");
        assert_eq!(relocatable_schema("/path"), "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/path");
        assert_eq!(gvariant_string("Flick F17"), "\"Flick F17\"");
        assert_eq!(gvariant_string("a\\b\"c"), "\"a\\\\b\\\"c\"");
    }

    #[test]
    fn test_list_parse_and_format() {
        let list_str = "['/path/a/', '/path/b/']";
        let parsed = parse_list(list_str);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], "/path/a/");
        assert_eq!(parsed[1], "/path/b/");

        let empty_list = "@as []";
        assert_eq!(parse_list(empty_list).len(), 0);

        let formatted = format_list(&parsed);
        assert_eq!(formatted, "['/path/a/', '/path/b/']");

        let formatted_empty = format_list(&[]);
        assert_eq!(formatted_empty, "@as []");
    }

    #[test]
    fn test_is_gnome_session() {
        // Mock XDG env variable
        std::env::set_var("XDG_CURRENT_DESKTOP", "Ubuntu:GNOME");
        assert!(is_gnome_session());

        std::env::set_var("XDG_CURRENT_DESKTOP", "XFCE");
        assert!(!is_gnome_session());
    }

    #[test]
    fn test_trigger_command() {
        let cmd = trigger_command("F17");
        assert!(cmd.contains("--trigger F17"));
    }
}

