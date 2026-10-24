//! Client mineskin.org : upload d'un PNG -> textures property (value + signature).
//! Les skins générés sont servis depuis textures.minecraft.net, donc acceptés
//! par la validation de domaines de Pumpkin.

use serde::Deserialize;

#[derive(Deserialize)]
struct MineSkinData { texture: MineSkinTexture }
#[derive(Deserialize)]
struct MineSkinTexture { value: String, #[serde(default)] signature: Option<String> }
#[derive(Deserialize)]
struct MineSkinResp { data: MineSkinData }

pub fn download_player_skin(skin: &pumpkin_plugin_api::player::PlayerSkin) -> Result<Vec<u8>, String> {
    // value = base64 JSON {"textures":{"SKIN":{"url":"https://..."}}}
    let decoded = base64_decode(&skin.value)?;
    let url = extract_skin_url(&decoded)?;
    http_get_bytes(&url)
}

pub fn upload(png: &[u8]) -> Result<(String, Option<String>), String> {
    // POST multipart/form-data sur https://api.mineskin.org/v2/generate/upload
    // Réponse : {"data":{"texture":{"value":"...","signature":"..."}}}
    // TODO: à implémenter avec reqwest (multipart). La version 0 de ce
    // scaffolding expose la structure de réponse attendue.
    todo!("multipart upload via reqwest")
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> { todo!("base64") }
fn extract_skin_url(json: &[u8]) -> Result<String, String> { todo!("parse textures JSON") }
fn http_get_bytes(url: &str) -> Result<Vec<u8>, String> { todo!("reqwest get") }
