//! metier-skins — extension Pumpkin : tenue de métier + tête du joueur conservée.
//!
//! Principe : le skin Minecraft est une texture unique 64x64. On fabrique une
//! texture composite (tête du joueur + corps de la tenue du métier), on
//! l'uploade sur mineskin.org (URL textures.minecraft.net + signature), puis
//! on l'applique via `set_skin`.
//!
//! Contrainte connue : le profil est envoyé aux autres clients uniquement à la
//! jointure. Ce plugin cible donc le CAS 1 : le métier est choisi dans le
//! lobby AVANT l'entrée dans le monde de jeu.
//!
//! NOTE : scaffolding non compilé ici (pas de toolchain Rust dans l'environnement
//! de dev). Les signatures exactes de l'API plugin (Context, register_command,
//! permissions) doivent être ajustées d'après crates/pumpkin-plugin-api du
//! repo Pumpkin correspondant à ta version serveur.

use pumpkin_plugin_api::{permissions, register_plugin, Context, Plugin, PluginMetadata, Result};
use pumpkin_macros::plugin_impl;

mod composite;
mod mineskin;

const METIERS: &[(&str, &str)] = &[
    // (id métier, fichier de tenue embarqué dans le plugin)
    ("fermier", "tenues/fermier.png"),
    ("mineur", "tenues/mineur.png"),
];

#[derive(Default)]
struct MetierSkins;

#[plugin_impl]
impl Plugin for MetierSkins {
    fn new() -> Self { MetierSkins }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "metier-skins".into(),
            version: "0.1.0".into(),
            authors: vec!["damien millet".into()],
            description: "Tenue de métier avec conservation de la tête du joueur".into(),
            dependencies: vec![],
            permissions: vec![
                permissions::PLAYER_GET_SKIN.into(),
                permissions::PLAYER_SET_SKIN.into(),
                permissions::COMMAND_REGISTER.into(),
                permissions::NETWORK_CONNECT.into(),
                permissions::NETWORK_DNS.into(),
            ],
        }
    }

    fn on_load(&self, context: Context) -> Result<()> {
        context.server.send_message("[metier-skins] chargé")?;

        // Commande /metier <id> — à appeler depuis le menu du lobby.
        context.command.register_command(
            commands_builder!("metier")
                .then(argument!("id", string()))
                .executes(Self::on_choose_metier),
        )?;
        Ok(())
    }
}

impl MetierSkins {
    fn on_choose_metier(sender: &dyn CommandSender, id: &str) -> Result<()> {
        let Some(player) = sender.as_player() else {
            return Ok(());
        };
        let Some((_, outfit_file)) = METIERS.iter().find(|(m, _)| *m == id) else {
            player.send_message(format!("Métier inconnu : {id}"));
            return Ok(());
        };

        // 1. Skin actuel du joueur (textures property signée)
        let Some(skin) = player.get_skin() else {
            player.send_message("Skin introuvable.".into());
            return Ok(());
        };

        // 2. Composite : tête du joueur + corps de la tenue
        let player_png = mineskin::download_player_skin(&skin)?;
        let outfit_png = composite::load_embedded(outfit_file)?;
        let merged = composite::merge_head_onto(player_png, outfit_png)?;

        // 3. Upload vers mineskin.org (async) -> value + signature
        let (value, signature) = mineskin::upload(&merged)?;

        // 4. Application
        player.set_skin(pumpkin_plugin_api::player::PlayerSkin { value: value.into(), signature: signature.map(Into::into) })?;
        player.send_message(format!("Tenue de métier « {id} » appliquée."));
        Ok(())
    }
}

register_plugin!(MetierSkins);
