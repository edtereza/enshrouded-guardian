//! Lossless filename migration and English schemas. Original structured files
//! and historical logs are preserved verbatim in legacy-backup/.
use crate::logging;
use serde_json::Value;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

fn dictionary() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../data/storage-english.json")).expect("embedded migration dictionary"))
}

pub fn convert(value: &Value) -> Value {
    fn walk(value: &Value, field: &str) -> Value {
        match value {
            Value::Object(object) => Value::Object(object.iter().filter_map(|(original, value)| {
                let key = dictionary()["keys"][original].as_str().unwrap_or(original);
                if key != original.as_str() && object.contains_key(key) { return None; }
                Some((key.to_owned(), walk(value, key)))
            }).collect()),
            Value::Array(array) => Value::Array(array.iter().map(|v| walk(v, field)).collect()),
            Value::String(text) if matches!(field, "kind" | "decision" | "origin" | "reference" | "layout" | "format" | "mode") =>
                Value::String(dictionary()["keys"][text].as_str().unwrap_or(text).to_owned()),
            Value::String(text) if matches!(field, "issues" | "skillPointsNote" | "parser") =>
                Value::String(dictionary()["messages"][text].as_str().unwrap_or(text).to_owned()),
            _ => value.clone(),
        }
    }
    walk(value, "")
}

fn renamed(name: &str) -> String {
    if let Some(name) = dictionary()["files"][name].as_str() { return name.to_owned(); }
    let mut name = name.to_owned();
    for (old, new) in [("_primeira_entrada", "_first_join"), ("_referencia", "_reference"),
        ("_copia_desatualizada", "_outdated_copy"), ("_divergente_observado", "_difference_observed"),
        ("_substituido", "_replaced"), ("_versao_do_jogo_mudou", "_game_build_changed")] {
        name = name.replace(old, new);
    }
    name
}

fn archive_path(root: &Path, old: &Path) -> std::path::PathBuf {
    let mut path = root.join("legacy-backup");
    for component in old.strip_prefix(root).unwrap().components() {
        path.push(renamed(&component.as_os_str().to_string_lossy()));
    }
    path.set_file_name(format!("{}.original", path.file_name().unwrap().to_string_lossy()));
    path
}

fn walk(dir: &Path, root: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "legacy-backup" { continue; }
        if entry.file_type()?.is_symlink() { continue; }
        let old = entry.path();
        let destination = dir.join(renamed(&name));
        if entry.file_type()?.is_dir() {
            walk(&old, root)?;
        } else if name == "guardiao.log" || name == "registro.jsonl" {
            // Keep historical free-form Portuguese diagnostics intact. New log
            // files start in English instead of pretending to translate history.
            let archive = archive_path(root, &old);
            if let Some(parent) = archive.parent() { std::fs::create_dir_all(parent)?; }
            if archive.exists() { return Err(io::Error::new(io::ErrorKind::AlreadyExists, archive.display().to_string())); }
            std::fs::rename(&old, archive)?;
            continue;
        } else if old.extension().is_some_and(|e| e == "json") {
            let bytes = std::fs::read(&old)?;
            let value: Value = serde_json::from_slice(&bytes)?;
            let updated = convert(&value);
            if updated != value {
                let archive = archive_path(root, &old);
                if let Some(parent) = archive.parent() { std::fs::create_dir_all(parent)?; }
                if !archive.exists() { logging::gravar_atomico(&archive, &bytes)?; }
                logging::gravar_atomico(&old, &serde_json::to_vec_pretty(&updated)?)?;
            }
        }
        if destination != old {
            if destination.exists() { return Err(io::Error::new(io::ErrorKind::AlreadyExists, destination.display().to_string())); }
            std::fs::rename(old, destination)?;
        }
    }
    Ok(())
}

// Objects merge recursively. Explicit values (including empty lists, false and
// null) from the older file win; access-control lists are never unioned.
fn merge_config(newer: &Value, older: &Value) -> Value {
    match (newer, older) {
        (Value::Object(newer), Value::Object(older)) => {
            let mut result = newer.clone();
            for (key, value) in older {
                result.insert(key.clone(), result.get(key).map_or_else(|| value.clone(), |new| merge_config(new, value)));
            }
            Value::Object(result)
        }
        _ => older.clone(),
    }
}

fn read_config(bytes: &[u8]) -> io::Result<Value> {
    let value: Value = serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))?;
    if !value.is_object() { return Err(io::Error::new(io::ErrorKind::InvalidData, "Configuration must be a JSON object")); }
    let mut value = convert(&value);
    if let Some(Value::Object(list)) = value.get_mut("whitelist") {
        if let Some(ids) = list.remove("steamIds") { list.entry("players").or_insert(ids); }
    }
    Ok(value)
}

fn migrate_config(root: &Path, data: &Path) -> io::Result<()> {
    let old_config = root.join("guardiao.json");
    if !old_config.exists() { return Ok(()); }
    let config = root.join("guardian.json");
    let old_bytes = std::fs::read(&old_config)?;
    let old_value = read_config(&old_bytes)?;
    let current = if config.exists() {
        let bytes = std::fs::read(&config)?;
        let value = read_config(&bytes)?;
        Some((bytes, value, std::fs::metadata(&config)?.modified()?))
    } else { None };
    // Equal timestamps deterministically prefer the legacy file. Inaccessible
    // timestamps return an error instead of silently changing precedence.
    let old_time = std::fs::metadata(&old_config)?.modified()?;
    let legacy_older = current.as_ref().map_or(true, |(_, _, time)| old_time <= *time);
    let merged = match &current {
        Some((_, value, _)) if legacy_older => merge_config(value, &old_value),
        Some((_, value, _)) => merge_config(&old_value, value),
        None => old_value,
    };
    let backup_root = data.join("legacy-backup/config");
    std::fs::create_dir_all(&backup_root)?;
    let mut index = 0u64;
    let backup = loop {
        let candidate = backup_root.join(format!("merge-{index}"));
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => index += 1,
            Err(error) => return Err(error),
        }
    };
    logging::gravar_atomico(&backup.join("guardiao.json.original"), &old_bytes)?;
    if let Some((bytes, _, _)) = &current {
        logging::gravar_atomico(&backup.join("guardian.json.original"), bytes)?;
    }
    logging::gravar_atomico(&config, &serde_json::to_vec_pretty(&merged)?)?;
    std::fs::remove_file(&old_config)?;
    println!("[Guardian] Configuration migration completed; older values preferred from {}. Originals: {}",
        if legacy_older { "guardiao.json" } else { "guardian.json" }, backup.display());
    Ok(())
}

pub fn migrar(root: &Path) -> io::Result<()> {
    let data = root.join("guardian");
    std::fs::create_dir_all(&data)?;
    migrate_config(root, &data)?;
    walk(&data, &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("guardian-config-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(root.join("guardian")).unwrap();
        root
    }

    #[test]
    fn conflicting_configs_use_older_values_and_keep_both_originals() {
        for (legacy_time, current_time, legacy_wins) in [(10, 20, true), (20, 10, false), (10, 10, true)] {
            let root = config_root();
            let legacy = b"\xef\xbb\xbf{\"modo\":\"observar\",\"admins\":[],\"whitelist\":{\"enabled\":true,\"steamIds\":[\"76561198000000001\"]},\"legacyOnly\":7}";
            let current = br#"{"mode":"enforce","admins":["76561198000000002"],"whitelist":{"enabled":false,"players":[],"note":"kept"},"customSetting":false}"#;
            for (name, bytes, seconds) in [("guardiao.json", legacy.as_slice(), legacy_time), ("guardian.json", current.as_slice(), current_time)] {
                std::fs::write(root.join(name), bytes).unwrap();
                std::fs::File::options().write(true).open(root.join(name)).unwrap().set_times(
                    std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))).unwrap();
            }
            // Existing backups must not block migration or be overwritten.
            std::fs::create_dir_all(root.join("guardian/legacy-backup/config/merge-0")).unwrap();
            let sentinel = root.join("guardian/legacy-backup/config/merge-0/guardian.json.original");
            std::fs::write(&sentinel, b"previous backup").unwrap();
            migrar(&root).unwrap();
            let bytes = std::fs::read(root.join("guardian.json")).unwrap();
            let result: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(result["mode"], if legacy_wins { "observe" } else { "enforce" });
            assert_eq!(result["admins"], if legacy_wins { serde_json::json!([]) } else { serde_json::json!(["76561198000000002"]) });
            assert_eq!(result["whitelist"]["enabled"], legacy_wins);
            assert_eq!(result["whitelist"]["players"], if legacy_wins { serde_json::json!(["76561198000000001"]) } else { serde_json::json!([]) });
            assert!(result["whitelist"].get("steamIds").is_none());
            assert_eq!(result["whitelist"]["note"], "kept");
            assert_eq!(result["legacyOnly"], 7);
            assert_eq!(result["customSetting"], false);
            let backup = root.join("guardian/legacy-backup/config/merge-1");
            assert_eq!(std::fs::read(backup.join("guardiao.json.original")).unwrap(), legacy);
            assert_eq!(std::fs::read(backup.join("guardian.json.original")).unwrap(), current);
            assert_eq!(std::fs::read(&sentinel).unwrap(), b"previous backup");
            assert!(!root.join("guardiao.json").exists());
            migrar(&root).unwrap();
            assert_eq!(std::fs::read(root.join("guardian.json")).unwrap(), bytes);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn invalid_config_does_not_modify_either_original() {
        for invalid in [b"{".as_slice(), b"[]", b"null"] {
            for invalid_legacy in [false, true] {
                let root = config_root();
                let (old, current) = if invalid_legacy { (invalid, b"{}".as_slice()) } else { (b"{}".as_slice(), invalid) };
                std::fs::write(root.join("guardiao.json"), old).unwrap();
                std::fs::write(root.join("guardian.json"), current).unwrap();
                assert!(migrar(&root).is_err());
                assert_eq!(std::fs::read(root.join("guardiao.json")).unwrap(), old);
                assert_eq!(std::fs::read(root.join("guardian.json")).unwrap(), current);
                assert!(!root.join("guardian/legacy-backup").exists());
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn schemas_migrate_without_translating_player_names() {
        let value = serde_json::json!({"nome":"servidor", "origem":"servidor", "personagem":{"level":1}, "quando":"date"});
        let updated = convert(&value);
        assert_eq!(updated["name"], "servidor");
        assert_eq!(updated["origin"], "server");
        assert_eq!(updated["character"]["level"], 1);
        assert_eq!(updated["updatedAt"], "date");
        assert_eq!(convert(&updated), updated);
    }

    #[test]
    fn files_migrate_idempotently_and_keep_originals_and_binary_saves() {
        let root = std::env::temp_dir().join(format!("guardian-english-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let player = root.join("guardian/123");
        std::fs::create_dir_all(player.join("historico")).unwrap();
        std::fs::write(player.join("A.bin"), b"binary save").unwrap();
        std::fs::write(player.join("character.html"), b"existing report").unwrap();
        std::fs::write(player.join("meta.json"), br#"{"quando":"date","kEstado":1}"#).unwrap();
        std::fs::write(root.join("guardiao.json"), br#"{"mode":"enforce","admins":[],"whitelist":{"enabled":false,"players":[]}}"#).unwrap();
        std::fs::write(root.join("guardian/guardiao.log"), "Registro histórico intacto").unwrap();
        migrar(&root).unwrap();
        migrar(&root).unwrap();
        assert!(root.join("guardian.json").is_file());
        assert!(!root.join("guardiao.json").exists());
        assert!(player.join("history").is_dir());
        assert!(!player.join("character.html").exists());
        assert_eq!(std::fs::read(player.join("server_save.html")).unwrap(), b"existing report");
        assert_eq!(std::fs::read(player.join("A.bin")).unwrap(), b"binary save");
        let meta: Value = serde_json::from_slice(&std::fs::read(player.join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["updatedAt"], "date");
        assert_eq!(meta["knowledgeState"], 1);
        assert!(root.join("guardian/legacy-backup/123/meta.json.original").is_file());
        assert_eq!(std::fs::read_to_string(root.join("guardian/legacy-backup/guardian.log.original")).unwrap(), "Registro histórico intacto");
        std::fs::remove_dir_all(root).unwrap();
    }
}
