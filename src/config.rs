//! Configuration des fonctionnalités du plugin.
//!
//! Chaque fonctionnalité peut être activée ou désactivée via
//! `plugins/data/rpumpkin/config.json`. Si le fichier est absent, il est
//! créé avec les valeurs par défaut.

use std::collections::BTreeMap;

/// Identifiants des fonctionnalités configurables.
pub const FEATURE_SHEETS: &str = "fiches";
pub const FEATURE_CHAT_RP: &str = "chat_rp";
pub const FEATURE_RP_COMMANDS: &str = "commandes_rp";
pub const FEATURE_METIER: &str = "metier";
pub const FEATURE_BOOKS: &str = "livres";

/// Valeurs par défaut : (id, description, activé).
pub const FEATURES: &[(&str, &str, bool)] = &[
    (
        FEATURE_SHEETS,
        "Fiches de personnage persistantes (/fiche)",
        true,
    ),
    (
        FEATURE_CHAT_RP,
        "Formatage RP du chat ([RP] Nom : message)",
        true,
    ),
    (
        FEATURE_RP_COMMANDS,
        "Commandes RolePlay (/roll, /me, /do, /hrp, /rp)",
        true,
    ),
    (
        FEATURE_METIER,
        "Tenues de metier (/metier)",
        true,
    ),
    (
        FEATURE_BOOKS,
        "Livres configurables cote serveur (/livre)",
        true,
    ),
];

/// Configuration chargee du fichier `config.json`.
#[derive(Default, Clone)]
pub struct Config {
    /// Activation par identifiant de fonctionnalite.
    pub features: BTreeMap<String, bool>,
}

impl Config {
    /// Charge la configuration depuis `<data_folder>/config.json`.
    /// Cree le fichier avec les valeurs par defaut s'il est absent ou invalide.
    pub fn load(data_folder: &str) -> Config {
        let mut config = Config::default();
        for (id, _, enabled) in FEATURES {
            config.features.insert((*id).to_string(), *enabled);
        }
        let path = format!("{}/config.json", data_folder);
        if data_folder.is_empty() {
            return config;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&content) {
                if let Some(serde_json::Value::Object(features)) =
                    map.get("fonctionnalites").or_else(|| map.get("features"))
                {
                    for (id, value) in features {
                        if let Some(enabled) = value.as_bool() {
                            config.features.insert(id.clone(), enabled);
                        }
                    }
                }
            }
        } else {
            let _ = std::fs::write(&path, config.to_json());
        }
        config
    }

    /// Indique si une fonctionnalite est activee.
    pub fn enabled(&self, feature: &str) -> bool {
        self.features.get(feature).copied().unwrap_or(true)
    }

    /// Serialise la configuration courante en JSON lisible.
    pub fn to_json(&self) -> String {
        let mut features = serde_json::Map::new();
        for (id, _, _) in FEATURES {
            let enabled = self.enabled(id);
            features.insert((*id).to_string(), enabled.into());
        }
        let mut root = serde_json::Map::new();
        root.insert("fonctionnalites".into(), serde_json::Value::Object(features));
        serde_json::Value::Object(root).to_string()
    }
}
