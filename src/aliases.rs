//! Fonctionnalite "alias" : alias de commandes style Linux, stockes dans un
//! fichier de configuration serveur.
//!
//! Les alias sont definis dans `<data_folder>/aliases.json` :
//!
//! ```json
//! {
//!   "aliases": {
//!     "salut": "me salue tout le monde",
//!     "home": "tp @s 0 64 0"
//!   }
//! }
//! ```
//!
//! Chaque cle devient une commande `/<cle>` en jeu : les arguments passes apres
//! l'alias sont ajoutes a la commande cible, qui est executee via le dispatcher
//! du serveur avec l'expediteur d'origine (les permissions de la commande cible
//! s'appliquent donc normalement, aucun escalade de privilege). La gestion se
//! fait en jeu via `/alias` (admin) : `add`, `remove`, `list`, `reload`.
//!
//! Les commandes d'alias sont enregistrees au chargement du plugin : un alias
//! ajoute dans le fichier devient une commande utilisable au prochain demarrage
//! du serveur, tandis que la cible d'un alias existant est relue a chaque
//! execution (donc mise a jour immediatement par `reload` ou `add`).

use std::cell::Cell;
use std::collections::BTreeMap;

use pumpkin_plugin_api::command::{
    ArgumentType, Command, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType,
};
use pumpkin_plugin_api::commands::CommandHandler;
use pumpkin_plugin_api::server::CommandSender as ExecSender;
use pumpkin_plugin_api::text::TextComponent;
use pumpkin_plugin_api::Server;

/// Profondeur max d'imbrication des alias (protection contre les boucles).
const MAX_DEPTH: u32 = 16;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Noms reserves : commandes du plugin lui-meme, un alias ne peut pas les
/// remplacer.
pub const RESERVED: &[&str] = &[
    "sheet",
    "roll",
    "me",
    "do",
    "hrp",
    "rp",
    "job",
    "book",
    "sightline",
    "alias",
];

/// Indique si `name` est un nom d'alias valide (literal de commande : pas de
/// slash initial, pas d'espace, non reserve).
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.chars().any(char::is_whitespace)
        && !RESERVED.contains(&name)
}

/// Charge les alias depuis `<data_folder>/aliases.json`. Cree le fichier avec
/// une structure vide s'il est absent. Accepte un objet racine direct
/// (`{"nom": "commande"}`) ou enveloppe dans `"aliases"`. Les entrees invalides
/// (nom reserve/invalide, commande vide) sont ignorees.
pub fn load_aliases(data_folder: &str) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    if data_folder.is_empty() {
        return aliases;
    }
    let path = format!("{}/aliases.json", data_folder);
    let Ok(content) = std::fs::read_to_string(&path) else {
        let _ = std::fs::write(&path, to_json(&aliases));
        return aliases;
    };
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&content) {
        let entries = map
            .get("aliases")
            .and_then(serde_json::Value::as_object)
            .unwrap_or(&map);
        for (name, value) in entries {
            if let Some(command) = value.as_str() {
                insert_valid(&mut aliases, name, command);
            }
        }
    }
    aliases
}

/// Insere une entree valide (nom normalise en minuscules) dans la map.
fn insert_valid(aliases: &mut BTreeMap<String, String>, name: &str, command: &str) {
    let name = name.to_ascii_lowercase();
    if is_valid_name(&name) && !command.trim().is_empty() {
        aliases.insert(name, command.trim().to_string());
    }
}

/// Serialise les alias en JSON (`{"aliases": {...}}`).
pub fn to_json(aliases: &BTreeMap<String, String>) -> String {
    let mut inner = serde_json::Map::new();
    for (name, command) in aliases {
        inner.insert(name.clone(), serde_json::Value::String(command.clone()));
    }
    let mut root = serde_json::Map::new();
    root.insert("aliases".into(), serde_json::Value::Object(inner));
    serde_json::Value::Object(root).to_string()
}

/// Persiste les alias dans `<data_folder>/aliases.json`.
pub fn save_aliases(data_folder: &str, aliases: &BTreeMap<String, String>) -> Result<(), String> {
    if data_folder.is_empty() {
        return Err("Dossier de donnees indisponible.".to_string());
    }
    let path = format!("{}/aliases.json", data_folder);
    std::fs::write(&path, to_json(aliases)).map_err(|e| e.to_string())
}

/// Execute `command` via le dispatcher du serveur, comme si l'expediteur
/// l'avait tapee lui-meme. Protege contre les alias recursifs.
pub fn execute_as(
    sender: &CommandSender,
    server: &Server,
    command: &str,
) -> Result<(), CommandError> {
    DEPTH.with(|depth| {
        let current = depth.get();
        if current >= MAX_DEPTH {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Alias trop imbriques (boucle detectee), execution annulee.",
            )));
        }
        depth.set(current + 1);
        let exec_sender = match sender.as_player() {
            Some(player) => ExecSender::Player(player),
            None => ExecSender::Console,
        };
        server.execute_command(command, exec_sender);
        depth.set(current);
        Ok(())
    })
}

/// Construit la commande `/<name>` d'un alias, avec un argument greedy
/// `args` ajoute a la commande cible.
pub fn alias_command(name: &str, target: &str) -> Command {
    let description = format!("Alias vers /{}", target);
    Command::new(&[name.to_string()], &description)
        .then(
            CommandNode::argument("args", &ArgumentType::String(StringType::Greedy)).execute(
                AliasHandler {
                    alias: name.to_string(),
                },
            ),
        )
        .execute(AliasHandler {
            alias: name.to_string(),
        })
}

/// Construit la commande admin `/alias` (list, add, remove, reload).
pub fn admin_command() -> Command {
    Command::new(&["alias".to_string()], "Gerer les alias de commandes")
        .then(CommandNode::literal("list").execute(AliasListHandler))
        .then(
            CommandNode::literal("add").then(
                CommandNode::argument("name", &ArgumentType::String(StringType::SingleWord)).then(
                    CommandNode::argument("command", &ArgumentType::String(StringType::Greedy))
                        .execute(AliasAddHandler),
                ),
            ),
        )
        .then(
            CommandNode::literal("remove").then(
                CommandNode::argument("name", &ArgumentType::String(StringType::SingleWord))
                    .execute(AliasRemoveHandler),
            ),
        )
        .then(CommandNode::literal("reload").execute(AliasReloadHandler))
        .execute(AliasListHandler)
}

/// Handler d'une commande d'alias : `/<alias> <args...>`.
pub struct AliasHandler {
    /// Nom de l'alias (cle dans l'etat).
    pub alias: String,
}

impl CommandHandler for AliasHandler {
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let target = crate::with_state(|s| s.aliases.get(&self.alias).cloned());
        let Some(target) = target else {
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "L'alias «{}» n'existe plus (voir /alias list).",
                self.alias
            ))));
        };
        let extra = crate::arg_string(&args, "args");
        let command = if extra.is_empty() {
            target
        } else {
            format!("{} {}", target, extra)
        };
        execute_as(&sender, &server, &command)?;
        Ok(1)
    }
}

/// `/alias` et `/alias list` : liste les alias et leur commande cible.
pub struct AliasListHandler;

impl CommandHandler for AliasListHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let aliases = crate::with_state(|s| s.aliases.clone());
        if aliases.is_empty() {
            sender.send_message(TextComponent::from_legacy_string(
                "&7Aucun alias defini. Editez &fplugins/data/rpumpkin/aliases.json&7 \
                 ou utilisez &f/alias add <nom> <commande>&7.",
            ));
            return Ok(0);
        }
        let mut lines = vec!["&6--- Alias ---".to_string()];
        for (name, command) in &aliases {
            lines.push(format!("&e/{} &7-> &f/{}", name, command));
        }
        sender.send_message(TextComponent::from_legacy_string(&lines.join("\n")));
        Ok(aliases.len() as i32)
    }
}

/// `/alias add <nom> <commande...>` : cree ou remplace un alias, persistant.
pub struct AliasAddHandler;

impl CommandHandler for AliasAddHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let raw_name = crate::arg_string(&args, "name");
        let name = raw_name.to_ascii_lowercase();
        if !is_valid_name(&name) {
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "Nom «{}» invalide ou reserve. Noms reserves : {}.",
                raw_name,
                RESERVED.join(", ")
            ))));
        }
        let command = crate::arg_string(&args, "command");
        let command = command.trim().to_string();
        if command.is_empty() {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Usage : /alias add <nom> <commande...>",
            )));
        }
        let (existed, data_folder, snapshot) = crate::with_state(|s| {
            let existed = s.aliases.insert(name.clone(), command.clone()).is_some();
            (existed, s.data_folder.clone(), s.aliases.clone())
        });
        if let Err(e) = save_aliases(&data_folder, &snapshot) {
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "Alias enregistre en memoire mais sauvegarde impossible : {}",
                e
            ))));
        }
        let message = if existed {
            format!(
                "&aAlias «/{}» mis a jour -> &f/{}&a (effectif immediatement).",
                name, command
            )
        } else {
            format!(
                "&aAlias «/{}» cree -> &f/{}&a. La commande sera enregistree \
                 au prochain demarrage du serveur.",
                name, command
            )
        };
        sender.send_message(TextComponent::from_legacy_string(&message));
        Ok(1)
    }
}

/// `/alias remove <nom>` : supprime un alias, persistant.
pub struct AliasRemoveHandler;

impl CommandHandler for AliasRemoveHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let name = crate::arg_string(&args, "name").to_ascii_lowercase();
        let (removed, data_folder, snapshot) = crate::with_state(|s| {
            let removed = s.aliases.remove(&name).is_some();
            (removed, s.data_folder.clone(), s.aliases.clone())
        });
        if !removed {
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "Alias «/{}» introuvable (voir /alias list).",
                name
            ))));
        }
        if let Err(e) = save_aliases(&data_folder, &snapshot) {
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "Alias supprime en memoire mais sauvegarde impossible : {}",
                e
            ))));
        }
        sender.send_message(TextComponent::from_legacy_string(&format!(
            "&aAlias «/{}» supprime. La commande /{} restera enregistree \
             jusqu'au prochain demarrage (elle renverra alors une erreur).",
            name, name
        )));
        Ok(1)
    }
}

/// `/alias reload` : recharge `aliases.json` depuis le disque.
pub struct AliasReloadHandler;

impl CommandHandler for AliasReloadHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let data_folder = crate::with_state(|s| s.data_folder.clone());
        let loaded = load_aliases(&data_folder);
        let count = loaded.len();
        crate::with_state(|s| s.aliases = loaded);
        sender.send_message(TextComponent::from_legacy_string(&format!(
            "&aConfiguration des alias rechargee ({} alias). Les nouveaux noms seront \
             enregistres comme commandes au prochain demarrage du serveur.",
            count
        )));
        Ok(count as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wrapped_and_bare() {
        let dir = std::env::temp_dir().join(format!("rpumpkin-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let df = dir.to_str().unwrap().to_string();

        std::fs::write(
            format!("{}/aliases.json", df),
            r#"{"aliases": {"Salut": "me salue", "home": "tp @s 0 64 0"}}"#,
        )
        .unwrap();
        let a = load_aliases(&df);
        assert_eq!(a.get("salut").unwrap(), "me salue");
        assert_eq!(a.get("home").unwrap(), "tp @s 0 64 0");

        std::fs::write(
            format!("{}/aliases.json", df),
            r#"{"roll": "me triche", "bad name": "x", "/slash": "y", "empty": "  "}"#,
        )
        .unwrap();
        let a = load_aliases(&df);
        assert!(
            a.is_empty(),
            "reserved/invalid entries must be skipped: {a:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_is_created() {
        let dir = std::env::temp_dir().join(format!("rpumpkin-test2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let df = dir.to_str().unwrap().to_string();
        let a = load_aliases(&df);
        assert!(a.is_empty());
        let content = std::fs::read_to_string(format!("{}/aliases.json", df)).unwrap();
        assert_eq!(content, r#"{"aliases":{}}"#);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip() {
        let mut a = BTreeMap::new();
        a.insert("salut".to_string(), "me salue".to_string());
        let json = to_json(&a);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["aliases"]["salut"], "me salue");
        assert!(is_valid_name("salut"));
        assert!(!is_valid_name("alias"));
        assert!(!is_valid_name("a b"));
        assert!(!is_valid_name("/x"));
    }
}
