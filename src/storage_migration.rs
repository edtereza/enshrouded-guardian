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

pub fn migrar(root: &Path) -> io::Result<()> {
    let data = root.join("guardian");
    std::fs::create_dir_all(&data)?;
    let old_config = root.join("guardiao.json");
    let config = root.join("guardian.json");
    if old_config.exists() {
        let value: Value = serde_json::from_slice(&std::fs::read(&old_config)?)?;
        let updated = convert(&value);
        if config.exists() {
            let existing: Value = serde_json::from_slice(&std::fs::read(&config)?)?;
            if existing != updated { return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Conflicting guardian.json and legacy configuration")); }
        } else {
            logging::gravar_atomico(&config, &serde_json::to_vec_pretty(&updated)?)?;
        }
        let backup = data.join("legacy-backup/config");
        std::fs::create_dir_all(&backup)?;
        std::fs::rename(old_config, backup.join("guardian.json.original"))?;
    }
    walk(&data, &data)
}

#[cfg(test)]
mod tests {
    use super::*;
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
