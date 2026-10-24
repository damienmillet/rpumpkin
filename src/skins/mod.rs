//! Tenues de métier : application d'un skin composite (tête du joueur
//! conservée, corps remplacé par la tenue du métier).
//!
//! Le skin Minecraft étant une texture unique 64x64, la tenue est un PNG
//! embarqué dont la zone tête est transparente. Le composite final copie
//! la zone tête (base + overlay) du skin du joueur sur la tenue.
//!
//! Contrainte API : le plugin API 0.2 n'expose pas de client HTTP, donc
//! l'upload mineskin.org (value + signature signées) ne peut pas être fait
//! depuis le plugin. Deux modes :
//! - `Tenue::pre générée` : les composites sont pré-générés hors-ligne
//!   (voir `skins/gen_skins.py`) et le plugin lit le PNG depuis son
//!   dossier de données ;
//! - TODO futur : upload mineskin quand l'API plugin exposera HTTP.

use image::{GenericImageView, ImageBuffer, Rgba};
use pumpkin_plugin_api::player::PlayerSkin;

/// Zones de la texture correspondant à la tête (base + overlay/chapeau).
const HEAD_ZONES: [(u32, u32, u32, u32); 2] = [(0, 0, 32, 16), (32, 0, 32, 16)];

/// Métiers disponibles : (id, fichier de tenue dans le dossier de données).
pub const METIERS: &[(&str, &str)] = &[
    ("fermier", "tenues/metier_fermier.png"),
    ("mineur", "tenues/metier_mineur.png"),
];

pub fn metier_exists(id: &str) -> bool {
    METIERS.iter().any(|(m, _)| *m == id)
}

/// Retourne le fichier de tenue pour un métier, si connu.
pub fn outfit_file(id: &str) -> Option<&'static str> {
    METIERS
        .iter()
        .find(|(m, _)| *m == id)
        .map(|(_, file)| *file)
}

/// Copie la zone tête du PNG du joueur dans le PNG de la tenue.
/// La tenue doit avoir une zone tête transparente.
pub fn merge_head_onto(player: &[u8], outfit: &[u8]) -> Result<Vec<u8>, String> {
    let p = image::load_from_memory(player).map_err(|e| e.to_string())?;
    let o = image::load_from_memory(outfit).map_err(|e| e.to_string())?;
    if p.dimensions() != (64, 64) || o.dimensions() != (64, 64) {
        return Err("textures 64x64 attendues".into());
    }
    let (p, o) = (p.to_rgba8(), o.to_rgba8());
    let mut merged: ImageBuffer<Rgba<u8>, Vec<u8>> = o.clone();
    for (x0, y0, w, h) in HEAD_ZONES {
        for dy in 0..h {
            for dx in 0..w {
                let px = p.get_pixel(x0 + dx, y0 + dy);
                merged.put_pixel(x0 + dx, y0 + dy, *px);
            }
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    merged
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Extrait l'URL du skin depuis la propriété `value` d'un `PlayerSkin`
/// (base64 JSON `{"textures":{"SKIN":{"url":"https://..."}}}`).
pub fn extract_skin_url(skin: &PlayerSkin) -> Result<String, String> {
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&skin.value)
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).map_err(|e| e.to_string())?;
    json.pointer("/textures/SKIN/url")
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .ok_or_else(|| "URL du skin introuvable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_zones_are_the_expected_layout() {
        assert_eq!(HEAD_ZONES[0], (0, 0, 32, 16));
        assert_eq!(HEAD_ZONES[1], (32, 0, 32, 16));
    }

    #[test]
    fn metiers_are_known() {
        assert!(metier_exists("fermier"));
        assert!(metier_exists("mineur"));
        assert!(!metier_exists("forgeron"));
        assert_eq!(outfit_file("fermier"), Some("tenues/metier_fermier.png"));
    }

    #[test]
    fn extracts_skin_url_from_base64_value() {
        let payload = br#"{"textures":{"SKIN":{"url":"https://textures.minecraft.net/abc"}}}"#;
        use base64::Engine as _;
        let value = base64::engine::general_purpose::STANDARD.encode(payload);
        let skin = PlayerSkin {
            value: value.into(),
            signature: None,
        };
        assert_eq!(
            extract_skin_url(&skin).unwrap(),
            "https://textures.minecraft.net/abc"
        );
    }
}
