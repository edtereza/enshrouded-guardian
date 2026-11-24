//! Read-only port of the Save Editor's shared/character.mjs,
//! shared/knowledge.mjs, src/treecheck.js and checkModel in src/tauri-api.js.
//! Source: Nicolas Ferreira, MIT (the repository LICENSE also covers this port).

use crate::logging;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{mpsc, OnceLock};

const LIMITE: usize = 64 * 1024 * 1024;
const EQUIP_MAX: usize = 24;
const MIN_EQUIP: usize = 4;
const TAIL_COMPACTO: usize = 22;

struct Catalogo {
    itens: Value,
    habilidades: Value,
    conhecimentos: Value,
}

fn catalogo() -> Result<&'static Catalogo, String> {
    static DADOS: OnceLock<Result<Catalogo, String>> = OnceLock::new();
    DADOS.get_or_init(|| {
        Ok(Catalogo {
            itens: serde_json::from_str(include_str!("../data/save-editor/items.json")).map_err(|e| e.to_string())?,
            habilidades: serde_json::from_str(include_str!("../data/save-editor/skills.json")).map_err(|e| e.to_string())?,
            conhecimentos: serde_json::from_str(include_str!("../data/save-editor/knowledge.json")).map_err(|e| e.to_string())?,
        })
    }).as_ref().map_err(Clone::clone)
}

fn u32_em(p: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(p.get(off..off.checked_add(4)?)?.try_into().ok()?))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn nome(dados: &Value, reserva: String) -> String {
    dados.get("en").and_then(Value::as_str)
        .or_else(|| dados.get("en").and_then(Value::as_str))
        
        .or_else(|| dados.get("debug").and_then(Value::as_str))
        .map(str::to_owned).unwrap_or(reserva)
}

fn item(dados: &Catalogo, id: u32) -> Option<&Value> {
    dados.itens.get(id.to_string())
}

fn equipavel(dados: &Catalogo, id: u32) -> bool {
    matches!(item(dados, id).and_then(|i| i["slot"].as_str()),
        Some("Armour_Head" | "Armour_UpperBody" | "Armour_Arms" | "Armour_LowerBody" |
            "Armour_Feet" | "Armour_Ring" | "Shield" | "HookShot" | "Glider" | "Backpack"))
}

fn ler_item(p: &[u8], off: usize, dados: &Catalogo) -> Option<(Value, usize)> {
    let cab = p.get(off..off.checked_add(8)?)?;
    let id = u32_em(cab, 1)?;
    let info = item(dados, id)?;
    let flags = cab[7];
    if flags & !0x3f != 0 {
        return None;
    }
    let quantidade = u16::from_le_bytes([cab[5], cab[6]]);
    let mut e = json!({
        "slot": cab[0], "itemId": id, "name": nome(info, id.to_string()),
        "count": quantidade, "flags": flags,
    });
    let mut q = off + 8;
    for (bit, campos, tamanho) in [
        (0x01, &["level"][..], 1),
        (0x02, &["durability", "maxDurability"][..], 8),
        (0x04, &["liquid", "maxLiquid"][..], 8),
        (0x08, &["appearance"][..], 4),
        (0x10, &["maxUpgrades", "upgrades"][..], 2),
        (0x20, &["socket"][..], 5),
    ] {
        if flags & bit == 0 {
            continue;
        }
        let bytes = p.get(q..q.checked_add(tamanho)?)?;
        match bit {
            0x01 => e[campos[0]] = json!(bytes[0]),
            0x02 | 0x04 => {
                e[campos[0]] = json!(u32_em(bytes, 0)?);
                e[campos[1]] = json!(u32_em(bytes, 4)?);
            }
            0x10 => {
                e[campos[0]] = json!(bytes[0]);
                e[campos[1]] = json!(bytes[1]);
            }
            _ => e[campos[0]] = json!(hex(bytes)),
        }
        q += tamanho;
    }
    Some((e, q))
}

#[derive(Clone)]
struct Candidato {
    inicio: usize,
    fim_equip: usize,
    inicio_inv: usize,
    mascara: u32,
    livres: Option<u16>,
    equipamento: Vec<Value>,
    principal: Vec<Value>,
    extra: Vec<Value>,
    fim: usize,
}

fn inventario(p: &[u8], inicio: usize, principal: usize, extra: usize,
    ordenado: bool, dados: &Catalogo) -> Option<(Vec<Value>, Vec<Value>, usize)> {
    let mut q = inicio;
    let mut lista = Vec::new();
    let mut anterior = None;
    for i in 0..principal + extra {
        if i == principal {
            anterior = None;
        }
        let (e, fim) = ler_item(p, q, dados)?;
        let slot = e["slot"].as_u64()?;
        if ordenado && anterior.is_some_and(|a| slot <= a) {
            return None;
        }
        anterior = Some(slot);
        lista.push(e);
        q = fim;
    }
    let extra = lista.split_off(principal);
    Some((lista, extra, q))
}

fn ler_personagem(p: &[u8], dados: &Catalogo) -> Result<Value, String> {
    let nivel = u32_em(p, 0x3a).ok_or("Missing level")?;
    let xp = u32_em(p, 0x3e).ok_or("Missing XP")?;
    let n = u32_em(p, 0x42).ok_or("Missing skill count")? as usize;
    let fim_habilidades = 0x46usize.checked_add(n.checked_mul(5).ok_or("Skill count exceeds the limit")?)
        .filter(|fim| *fim <= p.len()).ok_or("Truncated skill list")?;
    let mut habilidades = Vec::new();
    for s in p[0x46..fim_habilidades].chunks_exact(5) {
        let id = u32_em(s, 0).unwrap();
        let no = dados.habilidades["nodes"].get(id.to_string()).ok_or_else(|| format!("Unknown skill: {id}"))?;
        habilidades.push(json!({"id": id, "level": s[4], "name": nome(no, id.to_string())}));
    }
    let mut classico = None;
    let mut compacto = None;
    let mut n_compactos = 0;
    for inicio in fim_habilidades + 4..p.len().saturating_sub(8) {
        let mut equipamento = Vec::new();
        let mut q = inicio;
        while let Some((e, fim)) = ler_item(p, q, dados) {
            if e["slot"].as_u64() != Some(equipamento.len() as u64) || equipamento.len() >= EQUIP_MAX {
                break;
            }
            equipamento.push(e);
            q = fim;
        }
        if !equipamento.iter().all(|e| equipavel(dados, e["itemId"].as_u64().unwrap() as u32)) {
            continue;
        }
        let mascara = u32_em(p, inicio - 4).unwrap();
        let bits = mascara.count_ones() as usize;
        if bits < equipamento.len() || bits > equipamento.len() + 4 {
            continue;
        }
        if let Some(cab) = p.get(q..q + 4) {
            let principal = cab[1] as usize;
            let extra = cab[3] as usize;
            let cab_valido = cab[0] == 5 && cab[2] <= 8 && (cab[2] > 0 || extra == 0);
            if bits == equipamento.len() && (cab_valido || cab == [0, 0, 0, 0]) {
                if let Some((principal, extra, fim)) = inventario(p, q + 4, principal, extra, true, dados) {
                    if p.len() - fim == TAIL_COMPACTO {
                        n_compactos += 1;
                        compacto = Some(Candidato { inicio, fim_equip: q, inicio_inv: q + 4,
                            mascara, livres: None, equipamento: equipamento.clone(), principal, extra, fim });
                    }
                }
            }
        }
        if equipamento.len() < MIN_EQUIP || q + 2 > p.len() || classico.is_some() {
            continue;
        }
        let livres = u16::from_le_bytes([p[q], p[q + 1]]);
        // Same first valid candidate rule as the editor's classic layout.
        for inicio_inv in q + 6..p.len() {
            let cab = &p[inicio_inv - 4..inicio_inv];
            if cab[0] != 5 || cab[2] > 8 {
                continue;
            }
            if let Some((principal, extra, fim)) = inventario(p, inicio_inv, cab[1] as usize, cab[3] as usize, false, dados) {
                classico = Some(Candidato { inicio, fim_equip: q, inicio_inv, mascara,
                    livres: Some(livres), equipamento: equipamento.clone(), principal, extra, fim });
                break;
            }
        }
    }
    let c = if n_compactos == 1 { compacto } else { classico }
        .ok_or("Equipment/inventory not found in save")?;
    Ok(json!({
        "level": nivel, "xp": xp, "skills": habilidades,
        "equipment": c.equipamento, "main": c.principal, "extra": c.extra,
        "equipMask": c.mascara, "equipFree": c.livres,
        "layout": if c.livres.is_some() { "classic" } else { "compact" },
        "opaque": {
            "appearanceAndScalesHex": hex(&p[..0x3a]),
            "beforeEquipmentHex": hex(&p[fim_habilidades..c.inicio]),
            "betweenEquipmentAndInventoryHex": hex(&p[c.fim_equip..c.inicio_inv]),
            "tailHex": hex(&p[c.fim..]),
        }
    }))
}

fn conhecimento(k: &[u8], estado: u8, dados: &Catalogo) -> Result<Value, String> {
    if k.len() % 8 != 0 {
        return Err("Unexpected K.bin size".into());
    }
    let meio = k.len() / 2;
    let mut vistos = HashSet::new();
    let mut duplicados = Vec::new();
    let entradas: Vec<_> = k[..meio].chunks_exact(4).zip(k[meio..].chunks_exact(4)).map(|(id, valor)| {
        let id = u32_em(id, 0).unwrap();
        let valor = u32_em(valor, 0).unwrap();
        if !vistos.insert(id) {
            duplicados.push(id);
        }
        let info = &dados.conhecimentos[id.to_string()];
        let nome = info["i"].as_u64().and_then(|id| item(dados, id as u32))
            .map(|i| nome(i, id.to_string()))
            .or_else(|| info["n"].as_str().map(str::to_owned));
        json!({"id": id, "value": valor, "name": nome, "type": info["t"]})
    }).collect();
    Ok(json!({"containerState": estado, "entries": entradas, "duplicateIds": duplicados}))
}

fn validar_modelo(c: &Value, dados: &Catalogo) -> Vec<String> {
    let mut problemas = Vec::new();
    if c["level"].as_u64().is_none_or(|v| v > 1000) {
        problemas.push("Level outside editor range (0 to 1000)".into());
    }
    let principal = c["main"].as_array().unwrap();
    let mut slots = HashSet::new();
    if principal.iter().any(|e| !slots.insert(e["slot"].as_u64().unwrap())) {
        problemas.push("Two items occupy the same main inventory slot".into());
    }
    let nos = dados.habilidades["nodes"].as_object().unwrap();
    let mut comprados = HashSet::new();
    for s in c["skills"].as_array().unwrap() {
        let id = s["id"].as_u64().unwrap() as u32;
        let nivel = s["level"].as_u64().unwrap();
        let max = nos[&id.to_string()]["maxLevel"].as_u64().unwrap_or(1).max(1);
        if nivel > max {
            problemas.push(format!("Skill {id}: level {nivel} exceeds maximum {max}"));
        }
        if nivel > 0 {
            comprados.insert(id);
        }
    }
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for ligacao in dados.habilidades["links"].as_array().unwrap() {
        if let (Some(a), Some(b)) = (ligacao[0].as_u64(), ligacao[1].as_u64()) {
            adj.entry(a as u32).or_default().push(b as u32);
            adj.entry(b as u32).or_default().push(a as u32);
        }
    }
    let mut vistos: HashSet<u32> = nos.iter().filter(|(_, n)| n["type"] == "Root")
        .filter_map(|(id, _)| id.parse().ok()).collect();
    let mut fila: Vec<_> = vistos.iter().copied().collect();
    while let Some(id) = fila.pop() {
        for &vizinho in adj.get(&id).into_iter().flatten() {
            if comprados.contains(&vizinho) && vistos.insert(vizinho) {
                fila.push(vizinho);
            }
        }
    }
    let mut orfaos: Vec<_> = comprados.difference(&vistos).copied().collect();
    orfaos.sort_unstable();
    if !orfaos.is_empty() {
        problemas.push(format!("Skills disconnected from the tree: {orfaos:?}"));
    }
    problemas
}

fn pontos_em_uso(c: &Value, dados: &Catalogo) -> u64 {
    c["skills"].as_array().into_iter().flatten().map(|s| {
        let id = s["id"].as_u64().unwrap_or(0).to_string();
        dados.habilidades["nodes"][id]["cost"].as_u64().unwrap_or(0)
            * s["level"].as_u64().unwrap_or(0)
    }).sum()
}

fn descompactar(a: &[u8]) -> Result<Vec<u8>, String> {
    if a.len() < 14 || !a.starts_with(b"S2BH") || a[8] != 9 {
        return Err("Received copy is not an S2BH character block".into());
    }
    match a[9] {
        0 => Ok(a[14..].to_vec()),
        1 => {
            let mut corpo = Vec::new();
            let leitor = ruzstd::decoding::StreamingDecoder::new(&a[14..]).map_err(|e| e.to_string())?;
            leitor.take(LIMITE as u64 + 1).read_to_end(&mut corpo).map_err(|e| e.to_string())?;
            if corpo.len() > LIMITE {
                return Err("Decompressed block exceeds 64 MiB".into());
            }
            Ok(corpo)
        }
        _ => Err("Unknown S2BH compression type".into()),
    }
}

pub fn analisar(a: &[u8], k: Option<&[u8]>, estado: u8) -> Value {
    let mut relatorio = json!({
        "parser": "Save Editor: character.mjs, knowledge.mjs, checkModel and treecheck",
        "character": null, "knowledge": null, "issues": [],
        "validation": {
            "characterReadable": false, "knowledgeReadable": false,
            "totalSkillPoints": null,
            "skillPointsNote": "The editor requires a user-supplied total; Guardian has not verified that total."
        },
        "blockA": {"sizeBytes": a.len(), "compressed": a.get(9).copied().filter(|_| a.starts_with(b"S2BH")).map(|v| v == 1)}
    });
    let mut problemas = Vec::new();
    let dados = match catalogo() {
        Ok(dados) => dados,
        Err(erro) => {
            relatorio["issues"] = json!([format!("Save Editor catalog unavailable: {erro}")]);
            return relatorio;
        }
    };
    let inicial = crate::state::aparencia_inicial(a, k);
    if inicial {
        relatorio["character"] = json!({"format": "initial_appearance", "appearanceHex": hex(a),
            "level": null, "xp": null, "gameInitializesProgress": true});
        relatorio["validation"]["characterReadable"] = json!(true);
    } else {
        match descompactar(a).and_then(|p| ler_personagem(&p, dados)) {
            Ok(c) => {
                problemas.extend(validar_modelo(&c, dados));
                relatorio["validation"]["usedSkillPoints"] = json!(pontos_em_uso(&c, dados));
                relatorio["character"] = c;
                relatorio["validation"]["characterReadable"] = json!(true);
            }
            Err(erro) => problemas.push(erro),
        }
    }
    match k.ok_or_else(|| "Unreadable knowledge list".to_string()).and_then(|k| conhecimento(k, estado, dados)) {
        Ok(k) => {
            if k["duplicateIds"].as_array().is_some_and(|lista| !lista.is_empty()) {
                problemas.push("Duplicate knowledge ID".into());
            }
            relatorio["knowledge"] = k;
            relatorio["validation"]["knowledgeReadable"] = json!(true);
        }
        Err(erro) => problemas.push(erro),
    }
    relatorio["issues"] = json!(problemas);
    relatorio
}

/// Reset all progression using a game-created fresh save, keeping only the
/// incoming appearance/scales prefix documented by the Save Editor.
pub(crate) fn personagem_inicial(a: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let recebido = descompactar(a)?;
    let aparencia = recebido.get(..0x3a).ok_or("Truncated character appearance")?;
    let modelo = include_bytes!("../templates/initial-character/A.bin");
    let mut corpo = descompactar(modelo)?;
    corpo[..0x3a].copy_from_slice(aparencia);
    let mut novo = modelo[..14].to_vec();
    novo[9] = 0;
    novo.extend_from_slice(&corpo);
    Ok((novo, include_bytes!("../templates/initial-character/K.bin").to_vec()))
}

struct Captura {
    sid: u64,
    handle: u32,
    a: Vec<u8>,
    k: Option<Vec<u8>>,
    estado: u8,
    origem: &'static str,
    quando: String,
}

static FILA: OnceLock<Option<mpsc::SyncSender<Captura>>> = OnceLock::new();

fn persistir(raiz: &std::path::Path, c: &Captura) -> std::io::Result<std::path::PathBuf> {
    let mut valor = analisar(&c.a, c.k.as_deref(), c.estado);
    valor["steam_id_64"] = json!(c.sid.to_string());
    valor["connectionHandle"] = json!(c.handle);
    valor["origin"] = json!(c.origem);
    valor["updatedAtUtc"] = json!(c.quando);
    let dir = raiz.join(c.sid.to_string());
    let arquivo = if c.origem == "client" { "client_save.json" } else { "server_save.json" };
    std::fs::create_dir_all(&dir)?;
    let bytes = serde_json::to_vec_pretty(&valor)?;
    let destino = dir.join(arquivo);
    logging::gravar_atomico(&destino, &bytes)?;
    if c.origem != "client" {
        // Keep the last server view when a client submits another entry save.
        // HTML failures must not prevent the JSON report from being saved.
        if let Err(erro) = crate::save_html::gerar(raiz, &valor)
            .and_then(|html| logging::gravar_atomico(&dir.join("server_save.html"), &html))
        {
            logging::log_arquivo(&format!("Save Editor HTML {}: {erro}", c.sid));
        }
    }
    Ok(destino)
}

fn exportar_copias_salvas() {
    let raiz = logging::dados();
    let Ok(pastas) = std::fs::read_dir(&raiz) else { return };
    for pasta in pastas.flatten() {
        let Ok(sid) = pasta.file_name().to_string_lossy().parse::<u64>() else { continue };
        let Ok(a) = std::fs::read(pasta.path().join("A.bin")) else { continue };
        let k = std::fs::read(pasta.path().join("K.bin")).ok();
        let meta: Value = std::fs::read(pasta.path().join("meta.json")).ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or(Value::Null);
        let c = Captura { sid, a, k, handle: meta["handle"].as_u64().unwrap_or(0) as u32,
            estado: meta["knowledgeState"].as_u64().unwrap_or(1) as u8,
            origem: "disk_copy", quando: meta["updatedAt"].as_str()
                .map(str::to_owned).unwrap_or_else(logging::agora) };
        if let Err(erro) = persistir(&raiz, &c) {
            logging::log_arquivo(&format!("Save Editor JSON {sid}/disk_copy: {erro}"));
        }
    }
}

fn fila() -> Option<&'static mpsc::SyncSender<Captura>> {
    FILA.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<Captura>(4);
        std::thread::Builder::new().name("guardian-save-reader".into()).spawn(move || {
            exportar_copias_salvas();
            while let Ok(c) = rx.recv() {
                if let Err(erro) = persistir(&logging::dados(), &c) {
                    logging::log_arquivo(&format!("Save Editor JSON {}/{}: {erro}", c.sid, c.origem));
                }
            }
        }).map(|_| tx).map_err(|erro| {
            logging::log_arquivo(&format!("Save Editor reader thread: {erro}"));
        }).ok()
    }).as_ref()
}

pub fn iniciar() {
    let _ = fila();
}

/// Catalog parsing and decoding run off the server thread. A bounded queue
/// prevents slow disk I/O from blocking game callbacks or retaining every save.
pub fn capturar(sid: u64, handle: u32, a: &[u8], k: Option<&[u8]>, estado: u8, origem: &'static str) {
    if let Some(fila) = fila() {
        if fila.try_send(Captura { sid, handle, a: a.to_vec(), k: k.map(<[u8]>::to_vec),
            estado, origem, quando: logging::agora() }).is_err()
        {
            logging::log_arquivo("Save Editor JSON: capture queue full or unavailable.");
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn reinicia_personagem_externo_preservando_apenas_aparencia() {
        for caso in fixtures().into_iter().filter(|caso| !caso["expected"].is_null()) {
            let original = bytes(&caso);
            let (a, k) = personagem_inicial(&original).unwrap();
            assert_eq!(&descompactar(&a).unwrap()[..0x3a], &descompactar(&original).unwrap()[..0x3a]);
            let relatorio = analisar(&a, Some(&k), 1);
            let personagem = &relatorio["character"];
            assert_eq!(personagem["level"], 1);
            assert_eq!(personagem["xp"], 0);
            for campo in ["skills", "main", "extra"] {
                assert!(personagem[campo].as_array().unwrap().is_empty());
            }
            assert_eq!(personagem["equipment"].as_array().unwrap().len(), 1);
            assert_eq!(personagem["equipment"][0]["itemId"], 3972122124u64);
            assert_eq!(relatorio["knowledge"]["entries"].as_array().unwrap().len(), 3);
        }
        assert!(personagem_inicial(&[0; 20]).is_err());
    }

    fn fixtures() -> Vec<Value> {
        serde_json::from_str(include_str!("../tests/fixtures/save-editor.json")).unwrap()
    }

    fn bytes(valor: &Value) -> Vec<u8> {
        let hex = valor["A_hex"].as_str().unwrap();
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
    }

    fn tirar_nomes(valor: &mut Value) {
        match valor {
            Value::Object(objeto) => {
                objeto.remove("name");
                for valor in objeto.values_mut() {
                    tirar_nomes(valor);
                }
            }
            Value::Array(lista) => lista.iter_mut().for_each(tirar_nomes),
            _ => {}
        }
    }

    #[test]
    fn mesmos_dados_do_save_editor_em_layouts_reais_e_sinteticos() {
        for caso in fixtures() {
            let mut relatorio = analisar(&bytes(&caso), Some(&[]), 1);
            if caso["expected"].is_null() {
                assert_eq!(relatorio["validation"]["characterReadable"], false, "{}", caso["name"]);
                continue;
            }
            assert_eq!(relatorio["validation"]["characterReadable"], true,
                "{}: {}", caso["name"], relatorio["issues"]);
            tirar_nomes(&mut relatorio["character"]);
            for (chave, esperado) in caso["expected"].as_object().unwrap() {
                assert_eq!(&relatorio["character"][chave], esperado, "{}: {chave}", caso["name"]);
            }
        }
    }

    #[test]
    fn dados_truncados_e_flags_desconhecidas_nao_causam_panico() {
        let a = bytes(&fixtures()[0]);
        for fim in 0..a.len() {
            let _ = analisar(&a[..fim], Some(&[]), 1);
        }
        let dados = catalogo().unwrap();
        let p = descompactar(&a).unwrap();
        let modelo = ler_personagem(&p, dados).unwrap();
        let id = modelo["main"][0]["itemId"].as_u64().unwrap() as u32;
        let mut entrada = vec![0; 8];
        entrada[1..5].copy_from_slice(&id.to_le_bytes());
        entrada[7] = 0x40;
        assert!(ler_item(&entrada, 0, dados).is_none());
    }

    #[test]
    fn valida_duplicatas_e_arvore_de_habilidades_como_o_editor() {
        let mut c = analisar(&bytes(&fixtures()[0]), Some(&[]), 1)["character"].take();
        let duplicado = c["main"][0].clone();
        c["main"].as_array_mut().unwrap().push(duplicado);
        c["skills"][0]["level"] = json!(255);
        let erros = validar_modelo(&c, catalogo().unwrap());
        assert!(erros.iter().any(|e| e.contains("same main inventory slot")));
        assert!(erros.iter().any(|e| e.contains("exceeds maximum")));
        let k = [1u32, 1, 2, 3].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
        let k = conhecimento(&k, 1, catalogo().unwrap()).unwrap();
        assert_eq!(k["duplicateIds"], json!([1]));
    }

    #[test]
    fn criacao_e_conhecimento_invalido_sao_reportados_sem_dados_fabricados() {
        let mut visual = vec![0; 38];
        visual[0] = 1;
        visual[37] = 1;
        let relatorio = analisar(&visual, Some(&[]), 1);
        assert_eq!(relatorio["validation"]["characterReadable"], true);
        assert_eq!(relatorio["character"]["format"], "initial_appearance");
        assert!(relatorio["character"]["level"].is_null());
        assert!(relatorio["character"]["xp"].is_null());
        let k = conhecimento(&[1], 1, catalogo().unwrap());
        assert!(k.is_err());
    }

    #[test]
    fn json_persistido_separa_entrada_e_save_do_servidor() {
        let raiz = std::env::temp_dir().join(format!("guardian-editor-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut c = Captura { sid: 123, handle: 64, a: bytes(&fixtures()[0]), k: Some(vec![]),
            estado: 1, origem: "client", quando: "2026-11-18T00:00:00Z".into() };
        let recebido = persistir(&raiz, &c).unwrap();
        let pagina = raiz.join("123/server_save.html");
        assert!(!pagina.exists());
        let original = std::fs::read(&recebido).unwrap();
        let valor: Value = serde_json::from_slice(&original).unwrap();
        assert_eq!(valor["steam_id_64"], "123");
        assert_eq!(valor["origin"], "client");
        assert!(valor["character"]["equipment"].as_array().unwrap().len() >= 4);
        c.origem = "server";
        let enviado = persistir(&raiz, &c).unwrap();
        assert_ne!(recebido, enviado);
        assert_eq!(std::fs::read(&recebido).unwrap(), original);
        let html = std::fs::read(&pagina).unwrap();
        c.origem = "client";
        persistir(&raiz, &c).unwrap();
        assert_eq!(std::fs::read(&pagina).unwrap(), html);
        c.origem = "disk_copy";
        persistir(&raiz, &c).unwrap();
        assert!(std::fs::read_to_string(&pagina).unwrap().contains("disk_copy"));
        std::fs::remove_dir_all(raiz).unwrap();
    }
}
