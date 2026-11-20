//! Finds the hook points by the shape of the code, not by fixed address, so they survive game
//! updates. Every search must yield exactly one result, otherwise installation is aborted.
//!
//! Hook points:
//! - entrada (join):   the `call` to the save reader, right before the message "[save2] Failed to read client savegame for"
//! - saida (save):    the two `lea r8, [rip+d]` pointing to the function that builds the save message
//! - sessao (session): the `call` to the logger, right after the message "[session] Remote player added. Player handle"

use core::slice;

pub struct Secao {
    pub base: usize,
    pub tam: usize,
}

pub struct Imagem {
    pub texto: Secao,
    pub todas: Secao, // whole image, to search for the messages in any data section
}

/// Prologue of the function that builds the save message (identical in builds 1269579, 1273043 and 1285601).
/// It stops before the relative `call`, whose displacement changes between builds.
const PROLOGO_MONTADOR: &[u8] = &[
    0x40, 0x55, 0x53, 0x41, 0x56, 0x48, 0x8d, 0xac, 0x24, 0xc0, 0xff, 0xfe, 0xff, 0xb8, 0x40, 0x01,
    0x01, 0x00,
];
const MSG_ENTRADA: &[u8] = b"[save2] Failed to read client savegame for";
const MSG_SESSAO: &[u8] = b"[session] Remote player added. Player handle";

pub struct Pontos {
    pub entrada_call: usize,
    pub entrada_alvo: usize,
    pub saida_lea: [usize; 2],
    pub saida_alvo: usize,
    pub sessao_call: usize,
    pub sessao_alvo: usize,
}

unsafe fn bytes(s: &Secao) -> &'static [u8] {
    slice::from_raw_parts(s.base as *const u8, s.tam)
}

fn acha_unico(agulha: &[u8], palheiro: &[u8], base: usize) -> Result<usize, String> {
    let mut achados = palheiro
        .windows(agulha.len())
        .enumerate()
        .filter(|(_, w)| *w == agulha)
        .map(|(i, _)| base + i);
    let primeiro = achados.next().ok_or("pattern not found")?;
    if achados.next().is_some() {
        return Err("pattern found more than once".into());
    }
    Ok(primeiro)
}

/// Addresses of the `lea reg, [rip+disp32]` instructions (48/4c 8d /r, mod=00 rm=101) that point to `alvo`.
fn leas_para(img: &Imagem, alvo: usize, so_r8: bool) -> Vec<usize> {
    let t = unsafe { bytes(&img.texto) };
    let mut out = Vec::new();
    for i in 0..t.len().saturating_sub(7) {
        let (p, m) = (t[i], t[i + 2]);
        let prefixo_ok = if so_r8 { p == 0x4c } else { p == 0x48 || p == 0x4c };
        let modrm_ok = if so_r8 { m == 0x05 } else { m & 0xc7 == 0x05 };
        if prefixo_ok && t[i + 1] == 0x8d && modrm_ok {
            let disp = i32::from_le_bytes([t[i + 3], t[i + 4], t[i + 5], t[i + 6]]) as isize;
            if (img.texto.base + i + 7).wrapping_add(disp as usize) == alvo {
                out.push(img.texto.base + i);
            }
        }
    }
    out
}

/// Absolute target of the `call rel32` at `ins`, if there is one and it points into the code.
fn destino_call(img: &Imagem, ins: usize) -> Option<usize> {
    let t = unsafe { bytes(&img.texto) };
    let i = ins.checked_sub(img.texto.base)?;
    if i + 5 > t.len() || t[i] != 0xe8 {
        return None;
    }
    let disp = i32::from_le_bytes([t[i + 1], t[i + 2], t[i + 3], t[i + 4]]) as isize;
    let dst = (ins + 5).wrapping_add(disp as usize);
    (img.texto.base..img.texto.base + img.texto.tam).contains(&dst).then_some(dst)
}

/// First `call rel32` in a window before (`para_tras`) or after the given address.
fn call_proximo(img: &Imagem, de: usize, janela: usize, para_tras: bool) -> Option<usize> {
    let passo: isize = if para_tras { -1 } else { 1 };
    let inicio: isize = if para_tras { -5 } else { 7 };
    for k in 0..janela as isize {
        let ins = (de as isize + inicio + passo * k) as usize;
        if destino_call(img, ins).is_some() {
            return Some(ins);
        }
    }
    None
}

fn acha_mensagem(img: &Imagem, msg: &[u8]) -> Result<usize, String> {
    let todas = unsafe { bytes(&img.todas) };
    acha_unico(msg, todas, img.todas.base).map_err(|e| format!("join message: {e}"))
}

pub fn achar(img: &Imagem) -> Result<Pontos, String> {
    // join: lea of the message -> first call before it
    let msg = acha_mensagem(img, MSG_ENTRADA)?;
    let refs = leas_para(img, msg, false);
    if refs.len() != 1 {
        return Err(format!("join message referenced {} times (expected 1)", refs.len()));
    }
    let entrada_call = call_proximo(img, refs[0], 80, true).ok_or("save reader call not found")?;
    let entrada_alvo = destino_call(img, entrada_call).unwrap();
    // The reader receives an output pointer in RCX. The caller compares its first
    // byte with zero and skips the failure log when it is zero.
    let t = unsafe { bytes(&img.texto) };
    let i = entrada_call - img.texto.base;
    let antes = i.checked_sub(11).and_then(|inicio| t.get(inicio..i));
    let depois = t.get(i + 5..i + 15);
    match (antes, depois) {
        (Some([0x48, 0x8d, 0x4d, saida, 0x48, 0x8b, 0xd7, 0xc6, 0x47, 0x0c, 0x02]),
         Some([0x44, 0x38, 0x6d, status, 0x0f, 0x84, salto @ ..]))
            if saida == status
                && ((refs[0] - img.texto.base + 7) as i64..t.len() as i64).contains(
                    &(i as i64 + 15 + i32::from_le_bytes(salto.try_into().unwrap()) as i64),
                ) => {}
        _ => return Err("client save reader status check not recognized".into()),
    }

    // save: builder prologue -> the two lea r8 pointing to it
    let saida_alvo =
        acha_unico(PROLOGO_MONTADOR, t, img.texto.base).map_err(|e| format!("save builder: {e}"))?;
    let leas = leas_para(img, saida_alvo, true);
    if leas.len() != 2 {
        return Err(format!("save builder referenced by {} lea r8 (expected 2)", leas.len()));
    }

    // session: lea of the message -> first call after it
    let msg = acha_mensagem(img, MSG_SESSAO)?;
    let refs = leas_para(img, msg, false);
    if refs.len() != 1 {
        return Err(format!("session message referenced {} times (expected 1)", refs.len()));
    }
    let sessao_call = call_proximo(img, refs[0], 90, false).ok_or("session logger call not found")?;
    let sessao_alvo = destino_call(img, sessao_call).unwrap();

    Ok(Pontos {
        entrada_call,
        entrada_alvo,
        saida_lea: [leas[0], leas[1]],
        saida_alvo,
        sessao_call,
        sessao_alvo,
    })
}

/// The call that chooses the join/leave localization tag comes before the
/// chat-post call. Other mods already hook the latter.
pub fn achar_chat(img: &Imagem) -> Option<(usize, usize, usize)> {
    const TAG: &str = "48 83 7e 60 00 74 ?? 41 8b 4e 0c e8 ?? ?? ?? ?? 49 8b 4e 48 4c 8d 05 ?? ?? ?? ?? 41 8b 16 85 c0 4c 89 85 ?? ?? ?? ?? 41 b9 df 0c ec 2f 41 b8 2d 70 53 ec";
    const POST: &[u8] = &[
        0x40, 0x55, 0x56, 0x41, 0x54, 0x41, 0x55, 0x41, 0x57, 0x48, 0x83, 0xec, 0x30,
        0x49, 0x8b, 0x41, 0x08, 0x33, 0xed, 0x4d, 0x8b, 0xf9, 0x45, 0x0f, 0xb6, 0xe0,
        0x48, 0x8b, 0xf2, 0x4c, 0x8b, 0xe9, 0x48, 0x3d, 0x00, 0x04, 0x00, 0x00, 0x76,
    ];
    let texto = unsafe { bytes(&img.texto) };
    let padrao: Vec<_> = TAG.split_ascii_whitespace().map(|b| u8::from_str_radix(b, 16).ok()).collect();
    let mut achados = texto.windows(padrao.len()).enumerate()
        .filter(|(_, janela)| janela.iter().zip(&padrao).all(|(b, p)| p.is_none_or(|p| *b == p)));
    let (posicao, _) = achados.next()?;
    if achados.next().is_some() {
        return None;
    }
    let ins = img.texto.base + posicao + 11;
    let tipo = destino_call(img, ins)?;
    let post = acha_unico(POST, texto, img.texto.base).ok()?;
    Some((ins, tipo, post))
}

/// Verified against EMF's ChatReceive and SessionRequestKick signatures. Only
/// used on the explicitly supported build; never patch ambiguous call sites.
pub fn achar_reset_chat(img: &Imagem) -> Option<(usize, usize, usize)> {
    let texto = unsafe { bytes(&img.texto) };
    let achar = |padrao: &str| -> Option<usize> {
        let p: Vec<_> = padrao.split_ascii_whitespace().map(|s| u8::from_str_radix(s, 16).ok()).collect();
        let mut hits = texto.windows(p.len()).enumerate()
            .filter(|(_, w)| w.iter().zip(&p).all(|(b, p)| p.is_none_or(|p| *b == p)));
        let (i, _) = hits.next()?;
        if hits.next().is_some() { return None; }
        Some(img.texto.base + i)
    };
    let receber = achar("40 55 53 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 ?? ?? FF FF 48 81 EC ?? ?? 00 00 4C 8B ?? 48 8B 49 30 E8 ?? ?? ?? ?? 49 8B ?? 10 48 8D 85 B8 00 00 00 48 89 85 90 00 00 00")?;
    let kick_call = achar("41 F6 86 F0 01 00 00 01 75 ?? 41 8B 96 E4 01 00 00 45 33 C0 49 8B 4F 10 E8")? + 0x18;
    let kick = destino_call(img, kick_call)?;
    let calls: Vec<_> = (0..texto.len().saturating_sub(5))
        .filter(|&i| destino_call(img, img.texto.base + i) == Some(receber))
        .map(|i| img.texto.base + i).collect();
    (calls.len() == 1).then(|| (calls[0], receber, kick))
}

pub fn achar_componente_nome(img: &Imagem) -> Option<usize> {
    let texto = unsafe { bytes(&img.texto) };
    let prefixo = [0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x74, 0x24, 0x10,
        0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8d, 0xba];
    let mut hits = texto.windows(24).enumerate().filter(|(_, w)|
        w[..18] == prefixo && w[21..24] == [0, 0x44, 0x89]);
    let (posicao, _) = hits.next()?;
    if hits.next().is_some() { return None; }
    Some(img.texto.base + posicao)
}
