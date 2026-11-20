//! Standalone viewer generated from the same static HTML edited in the project.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io;
use std::sync::OnceLock;

const MARCADOR: &str = "<script id=\"guardian-data\" type=\"application/json\">";

struct Modelo {
    inicio: &'static str,
    fim: &'static str,
    metadados: String,
    icones: HashMap<String, String>,
}

fn erro(mensagem: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, mensagem.to_string())
}

// Decode catalogs once on the existing save worker, never on the game thread.
fn modelo() -> io::Result<&'static Modelo> {
    static MODELO: OnceLock<Result<Modelo, String>> = OnceLock::new();
    MODELO.get_or_init(|| {
        let html = include_str!("../templates/save-viewer.html");
        let (inicio, resto) = html.split_once(MARCADOR).ok_or("Missing viewer data marker")?;
        let (dados, fim) = resto.split_once("</script>").ok_or("Incomplete viewer data")?;
        let mut dados: Value = serde_json::from_str(dados).map_err(|e| e.to_string())?;
        let objeto = dados.as_object_mut().ok_or("Invalid viewer catalogs")?;
        // No snapshot from the developer's account is shipped in generated pages.
        objeto.remove("reports");
        objeto.remove("icons");
        let metadados = serde_json::to_string(&dados).map_err(|e| e.to_string())?;
        let icones = serde_json::from_str(include_str!("../data/save-editor/icons.json"))
            .map_err(|e| e.to_string())?;
        Ok(Modelo { inicio, fim, metadados, icones })
    }).as_ref().map_err(erro)
}

// JSON is inside an HTML script element: escaping '<' prevents a name or other
// received field from closing that element, including a literal </script>.
fn json_seguro(texto: &str) -> String {
    texto.replace('<', "\\u003c").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

pub fn gerar(relatorio: &Value) -> io::Result<Vec<u8>> {
    let modelo = modelo()?;
    let mut icones = serde_json::Map::new();
    let mut adicionar = |id: u64| {
        let chave = id.to_string();
        if let Some(imagem) = modelo.icones.get(&chave) {
            icones.insert(chave, json!(imagem));
        }
    };
    for campo in ["equipment", "main", "extra"] {
        for item in relatorio["character"][campo].as_array().into_iter().flatten() {
            if let Some(id) = item["itemId"].as_u64() { adicionar(id); }
            if let Some(socket) = item["socket"].as_str().filter(|s| s.len() == 10 && s.is_ascii()) {
                let bytes: Option<Vec<u8>> = (0..8).step_by(2)
                    .map(|i| u8::from_str_radix(&socket[i..i + 2], 16).ok()).collect();
                if let Some(bytes) = bytes {
                    adicionar(u32::from_le_bytes(bytes.try_into().unwrap()) as u64);
                }
            }
        }
    }
    let dinamicos = json!({"reports": [{"label": "Server save", "data": relatorio}], "icons": icones});
    let dinamicos = serde_json::to_string(&dinamicos)?;
    // Join the dynamic report and icons with immutable editor catalogs without
    // cloning the entire icon library for every save.
    let dados = format!("{},{}", &dinamicos[..dinamicos.len() - 1], &modelo.metadados[1..]);
    let mut html = Vec::with_capacity(modelo.inicio.len() + modelo.fim.len() + dados.len());
    html.extend_from_slice(modelo.inicio.as_bytes());
    html.extend_from_slice(MARCADOR.as_bytes());
    html.extend_from_slice(json_seguro(&dados).as_bytes());
    html.extend_from_slice(b"</script>");
    html.extend_from_slice(modelo.fim.as_bytes());
    Ok(html)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pagina_contem_apenas_o_save_atual_e_escapa_dados_recebidos() {
        let relatorio = json!({"steam_id_64": "123", "steam_account_name": "</script><script>teste</script>",
            "origin": "server", "character": {"level": 1, "equipment": [{"itemId": 3972122124u64}]}});
        let html = String::from_utf8(gerar(&relatorio).unwrap()).unwrap();
        let dados = html.split_once(MARCADOR).unwrap().1.split_once("</script>").unwrap().0;
        assert!(!dados.contains('<'));
        let dados: Value = serde_json::from_str(dados).unwrap();
        assert_eq!(dados["reports"].as_array().unwrap().len(), 1);
        assert_eq!(dados["reports"][0]["data"], relatorio);
        assert!(dados["icons"]["3972122124"].as_str().unwrap().starts_with("data:image/webp;base64,"));
        assert!(!html.contains("fetch("));
        assert!(!html.contains("setInterval("));
        assert!(!html.contains("76561199244298543"));
    }
}
