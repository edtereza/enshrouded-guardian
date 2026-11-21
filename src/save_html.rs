//! Static reports with immutable, versioned assets shared by all accounts.
use serde_json::{json, Value};
use std::{io, path::Path, sync::OnceLock};
const MARKER: &str = "<script id=\"guardian-data\" type=\"application/json\">";
const VERSION: &str = env!("CARGO_PKG_VERSION");
struct Model { files: Vec<(String, Vec<u8>)> }
fn error(message: impl ToString) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message.to_string()) }
fn safe_json(text: &str) -> String { text.replace('<', "\\u003c").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029") }
fn decode_image(uri: &str) -> io::Result<Vec<u8>> {
    let data = uri.strip_prefix("data:image/webp;base64,").ok_or_else(|| error("Unsupported embedded image"))?;
    if data.len() % 4 != 0 { return Err(error("Invalid base64 length")); }
    let mut output = Vec::with_capacity(data.len() * 3 / 4);
    for (index, chunk) in data.as_bytes().chunks_exact(4).enumerate() {
        let mut values = [0u32; 4];
        for (i, byte) in chunk.iter().enumerate() {
            values[i] = match byte {
                b'A'..=b'Z' => (byte - b'A') as u32, b'a'..=b'z' => (byte - b'a' + 26) as u32,
                b'0'..=b'9' => (byte - b'0' + 52) as u32, b'+' => 62, b'/' => 63,
                b'=' if i >= 2 && index + 1 == data.len() / 4 => 0,
                _ => return Err(error("Invalid base64 image")),
            };
        }
        if chunk[2] == b'=' && chunk[3] != b'=' { return Err(error("Invalid base64 padding")); }
        let bits = values[0] << 18 | values[1] << 12 | values[2] << 6 | values[3];
        output.push((bits >> 16) as u8);
        if chunk[2] != b'=' { output.push((bits >> 8) as u8); }
        if chunk[3] != b'=' { output.push(bits as u8); }
    }
    Ok(output)
}
fn model() -> io::Result<&'static Model> {
    static MODEL: OnceLock<Result<Model, String>> = OnceLock::new();
    MODEL.get_or_init(|| prepare().map_err(|e| e.to_string())).as_ref().map_err(error)
}
fn prepare() -> io::Result<Model> {
    let mut data: Value = serde_json::from_str(include_str!("../templates/assets/catalogs.json"))?;
    let mut files = vec![("viewer.css".into(), include_bytes!("../templates/assets/viewer.css").to_vec()),
        ("viewer.js".into(), include_bytes!("../templates/assets/viewer.js").to_vec())];
    let icons: Value = serde_json::from_str(include_str!("../data/save-editor/icons.json"))?;
    let mut urls = serde_json::Map::new();
    for (key, uri) in icons.as_object().ok_or_else(|| error("Invalid icon catalog"))? {
        key.parse::<u64>().map_err(error)?;
        let path = format!("images/item-{key}.webp");
        files.push((path.clone(), decode_image(uri.as_str().ok_or_else(|| error("Invalid icon"))?)?));
        urls.insert(key.clone(), json!(path));
    }
    data["icons"] = Value::Object(urls);
    for (key, uri) in data["ui"].as_object_mut().ok_or_else(|| error("Invalid UI catalog"))? {
        let hex: String = key.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        let path = format!("images/ui-{hex}.webp");
        files.push((path.clone(), decode_image(uri.as_str().ok_or_else(|| error("Invalid UI image"))?)?));
        *uri = json!(path);
    }
    let css = data.as_object_mut().unwrap().remove("cssImages").ok_or_else(|| error("Missing CSS images"))?;
    for (key, uri) in css.as_object().ok_or_else(|| error("Invalid CSS images"))? {
        if !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') { return Err(error("Invalid CSS image name")); }
        files.push((format!("images/{key}.webp"), decode_image(uri.as_str().ok_or_else(|| error("Invalid CSS image"))?)?));
    }
    let catalogs = format!("window.guardianCatalogs={};\n{}", safe_json(&serde_json::to_string(&data)?), include_str!("../templates/assets/catalog-paths.js"));
    files.push(("catalogs.js".into(), catalogs.into_bytes()));
    Ok(Model { files })
}
fn assets(root: &Path) -> io::Result<()> {
    let directory = root.join("assets").join(VERSION);
    if std::fs::read(directory.join("ready")).is_ok_and(|v| v == VERSION.as_bytes()) { return Ok(()); }
    std::fs::create_dir_all(directory.join("images"))?;
    for (name, bytes) in &model()?.files { crate::logging::gravar_atomico(&directory.join(name), bytes)?; }
    crate::logging::gravar_atomico(&directory.join("ready"), VERSION.as_bytes())
}
pub fn gerar(root: &Path, report: &Value) -> io::Result<Vec<u8>> {
    assets(root)?;
    let template = include_str!("../templates/save-viewer.html");
    let (start, rest) = template.split_once(MARKER).ok_or_else(|| error("Missing report marker"))?;
    let (_, end) = rest.split_once("</script>").ok_or_else(|| error("Incomplete report marker"))?;
    let data = safe_json(&serde_json::to_string(&json!({"reports":[{"label":"Server save","data":report}]}))?);
    Ok(format!("{start}{MARKER}{data}</script>{end}").replace("__GUARDIAN_ASSET_BASE__", &format!("../assets/{VERSION}")).into_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_assets_are_external_reused_and_reports_remain_safe() {
        let root = std::env::temp_dir().join(format!("guardian-assets-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let report = json!({"steam_id_64":"123","steam_account_name":"</script><script>bad</script>","character":{"equipment":[{"itemId":3972122124u64}]}});
        let html = String::from_utf8(gerar(&root, &report).unwrap()).unwrap();
        let payload = html.split_once(MARKER).unwrap().1.split_once("</script>").unwrap().0;
        assert!(!payload.contains('<'));
        assert_eq!(serde_json::from_str::<Value>(payload).unwrap()["reports"][0]["data"], report);
        assert!(!html.contains("data:image"));
        assert!(!html.contains("<style>"));
        assert!(html.contains(&format!("../assets/{VERSION}/viewer.js")));
        let directory = root.join("assets").join(VERSION);
        let icon = std::fs::read(directory.join("images/item-3972122124.webp")).unwrap();
        assert_eq!(&icon[..4], b"RIFF");
        let js = std::fs::read_to_string(directory.join("catalogs.js")).unwrap();
        assert!(!js.contains("data:image"));
        assert!(!js.contains("steam_account_name"));
        let css = directory.join("viewer.css");
        let modified = std::fs::metadata(&css).unwrap().modified().unwrap();
        gerar(&root, &json!({"steam_id_64":"456"})).unwrap();
        assert_eq!(std::fs::metadata(&css).unwrap().modified().unwrap(), modified);
        assert!(html.len() < 10000);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bad_embedded_images_fail_without_silent_corruption() {
        for data in ["garbage", "data:image/webp;base64,=AAA", "data:image/webp;base64,AA=A", "data:image/webp;base64,AA!"] { assert!(decode_image(data).is_err()); }
    }
}
