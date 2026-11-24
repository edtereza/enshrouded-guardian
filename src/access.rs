//! Reloaded at each join so edits to guardian.json apply without a restart.
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

#[derive(Default)]
pub struct Acesso {
    whitelist_ativa: bool,
    whitelist: HashSet<u64>,
    admins: HashSet<u64>,
}

impl Acesso {
    pub fn exige_identidade(&self) -> bool { self.whitelist_ativa }
    pub fn permitido(&self, sid: u64) -> bool {
        !self.whitelist_ativa || self.whitelist.contains(&sid)
    }
    pub fn admin(&self, sid: u64) -> bool { self.admins.contains(&sid) }
}

fn lista(valor: Option<&Value>, campo: &str) -> Result<HashSet<u64>, String> {
    let Some(valor) = valor else { return Ok(HashSet::new()); };
    let lista = valor.as_array().ok_or_else(|| format!("{campo} must be a list of Steam IDs"))?;
    lista.iter().map(|valor| {
        let id = match valor {
            Value::String(texto) if texto.len() == 17 && texto.bytes().all(|b| b.is_ascii_digit()) => texto.parse::<u64>().ok(),
            Value::Number(numero) => numero.as_u64().filter(|id| id.to_string().len() == 17),
            _ => None,
        };
        id.filter(|id| *id != 0).ok_or_else(|| format!("Invalid Steam ID in {campo}; use the full SteamID64 in quotes"))
    }).collect()
}

fn analisar(texto: &str) -> Result<Acesso, String> {
    let config: Value = serde_json::from_str(texto).map_err(|erro| format!("Invalid guardian.json: {erro}"))?;
    if !config.is_object() { return Err("guardian.json must be a JSON object".into()); }
    let mut acesso = Acesso::default();
    if let Some(whitelist) = config.get("whitelist") {
        if !whitelist.is_object() { return Err("whitelist must be an object with enabled and players".into()); }
        acesso.whitelist_ativa = match whitelist.get("enabled") {
            None => false,
            Some(Value::Bool(ativo)) => *ativo,
            _ => return Err("whitelist.enabled must be true or false".into()),
        };
        acesso.whitelist = lista(whitelist.get("players").or_else(|| whitelist.get("steamIds")), "whitelist.players")?;
    }
    acesso.admins = lista(config.get("admins"), "admins")?;
    Ok(acesso)
}

pub fn carregar(arquivo: &Path) -> Result<Acesso, String> {
    match std::fs::read_to_string(arquivo) {
        Ok(texto) => analisar(&texto),
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => Ok(Acesso::default()),
        Err(erro) => Err(format!("Could not read guardian.json: {erro}")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    const ID: u64 = 76561199244298543;
    #[test]
    fn whitelist_opcional_e_admin_sao_regras_separadas() {
        assert!(analisar("{}").unwrap().permitido(ID));
        assert!(analisar(r#"{"whitelist":{"enabled":true,"steamIds":["76561199244298543"]}}"#).unwrap().permitido(ID));
        assert!(!analisar(r#"{"whitelist":{"enabled":true,"players":[],"steamIds":["76561199244298543"]}}"#).unwrap().permitido(ID));
        let acesso = analisar(r#"{"whitelist":{"enabled":true,"players":[]},"admins":["76561199244298543"]}"#).unwrap();
        assert!(acesso.admin(ID));
        assert!(!acesso.permitido(ID));
        let acesso = analisar(r#"{"whitelist":{"enabled":true,"players":["76561199244298543","76561199244298543"]}}"#).unwrap();
        assert!(acesso.permitido(ID));
        assert!(!acesso.permitido(ID + 1));
        assert!(!acesso.admin(ID));
        let acesso = analisar(r#"{"whitelist":{"enabled":false,"players":[]}}"#).unwrap();
        assert!(acesso.permitido(ID));
    }
    #[test]
    fn invalid_configuration_does_not_silently_disable_protection() {
        for texto in ["{", "[]", r#"{"whitelist":true}"#, r#"{"whitelist":{"enabled":"true"}}"#,
            r#"{"admins":"76561199244298543"}"#, r#"{"admins":["123"]}"#] {
            assert!(analisar(texto).is_err(), "{texto}");
        }
    }
}
