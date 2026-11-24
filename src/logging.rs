//! Guardian files next to the executable: guardian.json, guardian/ (copies, events.jsonl, guardian.log).
//! Also the configuration (mode and language) and the messages, which go to guardian.log and to the server console.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static RAIZ: OnceLock<PathBuf> = OnceLock::new();
static VERIFICAR: AtomicBool = AtomicBool::new(true);

/// Whether the new-version check is on (the default) in guardian.json.
pub fn verificar_atualizacao() -> bool {
    VERIFICAR.load(Ordering::Relaxed)
}


pub fn definir_raiz(dir_exe: &Path) -> bool {
    let _ = RAIZ.set(dir_exe.to_path_buf());
    if let Err(erro) = migrar_dados(dir_exe) {
        let _ = writeln!(std::io::stdout(), "[Guardian] Could not migrate guardiao-dados to guardian: {erro}. No gameplay changes will be made.");
        return false;
    }
    let _ = fs::create_dir_all(dados());
    if let Err(erro) = crate::storage_migration::migrar(dir_exe) {
        let _ = writeln!(std::io::stdout(), "[Guardian] Storage migration failed: {erro}. Guardian remains disabled.");
        return false;
    }
    // BisectHosting's DLL loader writes DLLMods/loader.log and its panel follows that file live, while the
    // server's own stdout never reaches the panel console. When the file is there, mirror our lines into it.
    let loader = dir_exe.join("DLLMods").join("loader.log");
    let _ = ESPELHO.set(loader.is_file().then_some(loader));
    true
}

fn migrar_dados(raiz: &Path) -> std::io::Result<()> {
    let antiga = raiz.join("guardiao-dados");
    let nova = raiz.join("guardian");
    if antiga.exists() {
        if nova.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists,
                "Both directories exist; preserve and reconcile the data before starting"));
        }
        fs::rename(antiga, nova)?;
    }
    Ok(())
}

#[cfg(test)]
mod testes_migracao {
    use super::*;
    #[test]
    fn migracao_preserva_arquivos_e_nao_sobrescreve_destino_existente() {
        let raiz = std::env::temp_dir().join(format!("guardian-migracao-{}-{}", std::process::id(), carimbo()));
        fs::create_dir_all(raiz.join("guardiao-dados/123")).unwrap();
        fs::write(raiz.join("guardiao-dados/123/A.bin"), b"save-original").unwrap();
        migrar_dados(&raiz).unwrap();
        assert_eq!(fs::read(raiz.join("guardian/123/A.bin")).unwrap(), b"save-original");
        assert!(!raiz.join("guardiao-dados").exists());
        fs::create_dir_all(raiz.join("guardiao-dados")).unwrap();
        assert!(migrar_dados(&raiz).is_err());
        assert_eq!(fs::read(raiz.join("guardian/123/A.bin")).unwrap(), b"save-original");
        fs::remove_dir_all(raiz).unwrap();
    }
}

/// Extra file that receives a copy of every log line (only on hosts whose panel shows it, see definir_raiz).
static ESPELHO: OnceLock<Option<PathBuf>> = OnceLock::new();
static LOG_SERVIDOR: OnceLock<PathBuf> = OnceLock::new();

pub fn raiz() -> PathBuf {
    RAIZ.get().cloned().unwrap_or_else(|| PathBuf::from("."))
}

pub fn dados() -> PathBuf {
    raiz().join("guardian")
}

/// "2026-10-20T14:01:46Z" without depending on date libraries.
pub fn agora() -> String {
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (dias, resto) = (s.div_euclid(86400), s.rem_euclid(86400));
    // civil_from_days (Howard Hinnant)
    let z = dias + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", resto / 3600, resto / 60 % 60, resto % 60)
}

pub fn carimbo() -> String {
    agora().replace(['-', ':'], "").replace('T', "_").trim_end_matches('Z').to_string()
}

/// Console lines held back while the server is still loading (None once released): printed at boot they get
/// buried under hundreds of game lines, so the guardian's block shows up after the world is loaded instead.
static RETIDO: Mutex<Option<Vec<String>>> = Mutex::new(Some(Vec::new()));

/// Prints to the server console and to the host panel's mirror file.
fn emitir(linhas: &[String]) {
    // writeln! and not println!: without a console println! panics, and with panic=abort that would take the server down
    let mut saida = std::io::stdout().lock();
    for l in linhas {
        let _ = writeln!(saida, "[Guardian] {l}");
    }
    let _ = saida.flush();
    if let Some(Some(espelho)) = ESPELHO.get() {
        // append only: the panel reads the file from where it stopped, so rewriting it would garble the console
        if let Ok(mut f) = OpenOptions::new().append(true).open(espelho) {
            for l in linhas {
                let _ = write!(f, "[Guardian] {l}\r\n");
            }
        }
    }
}

/// Lines shown only where a person watches the server (console and the panel's mirror), not in guardian.log:
/// the boot banner would otherwise repeat in the history on every restart.
pub fn console(linhas: &[&str]) {
    let linhas: Vec<String> = linhas.iter().map(|l| l.to_string()).collect();
    let mut retido = RETIDO.lock().unwrap_or_else(|e| e.into_inner());
    match retido.as_mut() {
        Some(fila) => fila.extend(linhas),
        None => {
            drop(retido);
            emitir(&linhas);
        }
    }
}

/// One line in guardian.log (right away: it's the history) and in the server console (held back during boot).
pub fn log(msg: &str) {
    log_arquivo(msg);
    console(&[msg]);
}

/// Entry denials are also reported in the game's log, alongside its save error.
pub fn log_entrada(msg: &str) {
    log_servidor(msg);
}

/// Append warnings to the console, Guardian log and native server log.
pub fn log_servidor(msg: &str) {
    log(msg);
    let arquivo = LOG_SERVIDOR.get_or_init(caminho_log_servidor);
    let linha = format!("[Guardian] {} {msg}\r\n", agora());
    if let Err(erro) = OpenOptions::new().append(true).open(arquivo)
        .and_then(|mut f| f.write_all(linha.as_bytes()))
    {
        log_arquivo(&format!("Could not append the Guardian message to enshrouded_server.log. {erro}"));
    }
}

/// One line in guardian.log only: technical details that mean nothing in a server console.
pub fn log_arquivo(msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dados().join("guardian.log")) {
        let _ = writeln!(f, "{} {}", agora(), msg);
    }
}

/// Prints everything held back and from then on prints right away.
fn liberar_console() {
    let fila = RETIDO.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(fila) = fila {
        emitir(&fila);
    }
}

/// Seconds since 1970 of a "YYYY.MM.DD HH:MM:SS" UTC date (days_from_civil, Howard Hinnant).
fn segundos_utc(data: &str) -> Option<i64> {
    let n: Vec<i64> = data.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(-1)).collect();
    let [a, m, d, h, mi, s] = n[..] else { return None };
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let yoe = a - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146097 + doe - 719468) * 86400 + h * 3600 + mi * 60 + s)
}

fn caminho_log_servidor() -> PathBuf {
    let json = fs::read_to_string(raiz().join("enshrouded_server.json")).unwrap_or_default();
    let pasta = campo_texto(&json, "logDirectory").unwrap_or("./logs");
    raiz().join(pasta).join("enshrouded_server.log")
}

/// Releases the held-back console lines once this boot's game log reports the world is ready,
/// or after three minutes.
pub fn liberar_quando_pronto() {
    let inicio = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let arquivo = LOG_SERVIDOR.get_or_init(caminho_log_servidor).clone();
    std::thread::spawn(move || {
        for _ in 0..90 {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let Ok(b) = fs::read(&arquivo) else { continue };
            let texto = String::from_utf8_lossy(&b);
            let desta_sessao = texto.split("Current UTC date: ").nth(1)
                .and_then(|r| segundos_utc(r.lines().next()?))
                .is_some_and(|t| (t - inicio).abs() < 600);
            if desta_sessao && texto.contains("Switching state from LoadEcsScene to Run") {
                break;
            }
        }
        liberar_console();
    });
}

/// One line of events.jsonl. `campos` already comes as "key": JSON value pairs.
pub fn registrar(tipo: &str, campos: &[(&str, String)]) {
    let mut linha = format!("{{\"kind\": \"{tipo}\", \"updatedAt\": \"{}\"", agora());
    for (k, v) in campos {
        linha.push_str(&format!(", \"{k}\": {v}"));
    }
    linha.push('}');
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dados().join("events.jsonl")) {
        let _ = writeln!(f, "{linha}");
    }
}

pub fn texto(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn gravar_atomico(caminho: &Path, dados: &[u8]) -> std::io::Result<()> {
    let tmp = caminho.with_extension("tmp");
    fs::write(&tmp, dados)?;
    fs::rename(&tmp, caminho)
}

/// Text value of a top-level key in a simple JSON like guardian.json.
fn campo_texto<'a>(json: &'a str, chave: &str) -> Option<&'a str> {
    let resto = &json[json.find(&format!("\"{chave}\""))? + chave.len() + 2..];
    let resto = resto.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(&resto[..resto.find('"')?])
}

fn campo(json: &str, chave: &str) -> Option<String> {
    campo_texto(json, chave).map(|valor| valor.trim().to_lowercase())
}

/// Reads guardian.json, accepting both English and Portuguese names:
/// {"mode": "observe"} or {"modo": "observar"}; {"language": "pt-BR"} or {"idioma": "pt-BR"}.
/// No file or no mode means enforce. Returns whether to enforce. {"checkUpdates": false} turns off the version check.
pub fn ler_config() -> bool {
    let json = fs::read_to_string(raiz().join("guardian.json")).unwrap_or_default();
    let desligada = |chave: &str| json.find(&format!("\"{chave}\"")).is_some_and(|i| {
        json[i + chave.len() + 2..].trim_start().strip_prefix(':').is_some_and(|r| r.trim_start().starts_with("false"))
    });
    VERIFICAR.store(!(desligada("checkUpdates") || desligada("verificarAtualizacao")), Ordering::Relaxed);
    let modo = campo(&json, "mode").or_else(|| campo(&json, "modo")).unwrap_or_default();
    !matches!(modo.as_str(), "observe" | "observar")
}
