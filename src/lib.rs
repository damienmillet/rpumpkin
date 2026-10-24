//! RPumpkin — RolePlay unifié pour serveurs Pumpkin.
//!
//! Fournit des fiches de personnage persistantes, un chat RP (HRP/RP),
//! des commandes RolePlay (/roll, /me, /do, /hrp, /fiche) et des tenues
//! de métier avec conservation de la tête du joueur (/metier).

pub mod books;
pub mod config;
pub mod skins;

use std::collections::HashMap;
use std::sync::Mutex;

use pumpkin_plugin_api::command::{
    Arg, ArgumentType, Command, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType,
};
use pumpkin_plugin_api::commands::CommandHandler;
use pumpkin_plugin_api::events::player::PlayerChatEvent;
use pumpkin_plugin_api::events::{EventHandler, EventPriority};
use pumpkin_plugin_api::permission::{Permission, PermissionDefault};
use pumpkin_plugin_api::permissions::{FS_READ_DATA, FS_WRITE_DATA};
use pumpkin_plugin_api::text::TextComponent;
use pumpkin_plugin_api::wit::pumpkin::plugin::data_components::DataComponent;
use pumpkin_plugin_api::ItemStack;
use pumpkin_plugin_api::{register_plugin, Context, Plugin, PluginMetadata, Server};

/// Fiche de personnage d'un joueur.
#[derive(Default, Clone)]
struct CharacterSheet {
    /// Nom du personnage (nom RP). Vide = non défini.
    name: String,
    /// Description / histoire du personnage.
    description: String,
    /// Métier du personnage.
    job: String,
    /// Mode RP actif (`true` = messages formatés RP, `false` = chat normal).
    rp_mode: bool,
}

/// État global du plugin, protégé par un mutex.
struct State {
    /// Fiches indexées par nom de compte Minecraft.
    sheets: HashMap<String, CharacterSheet>,
    /// Chemin du dossier de données persistantes.
    data_folder: String,
    /// Configuration des fonctionnalités.
    plugin_config: config::Config,
    /// Livres chargés depuis `<data_folder>/livres/`.
    books: std::collections::BTreeMap<String, books::Book>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let plugin_config = config::Config::default();
        *guard = Some(State {
            sheets: HashMap::new(),
            data_folder: String::new(),
            plugin_config,
            books: std::collections::BTreeMap::new(),
        });
    }
    f(guard.as_mut().unwrap())
}

/// Accès en lecture à la configuration des fonctionnalités.
pub fn with_config<R>(f: impl FnOnce(&config::Config) -> R) -> R {
    with_state(|s| f(&s.plugin_config))
}

fn sheet_of(player_name: &str) -> CharacterSheet {
    with_state(|s| s.sheets.get(player_name).cloned().unwrap_or_default())
}

fn set_sheet(player_name: &str, sheet: CharacterSheet) {
    with_state(|s| s.sheets.insert(player_name.to_string(), sheet));
}

fn save_sheets() -> Result<(), String> {
    let (path, json) = with_state(|s| {
        let path = format!("{}/sheets.json", s.data_folder);
        let mut map = serde_json::Map::new();
        for (player, sheet) in &s.sheets {
            let mut entry = serde_json::Map::new();
            entry.insert("name".into(), sheet.name.clone().into());
            entry.insert("description".into(), sheet.description.clone().into());
            entry.insert("job".into(), sheet.job.clone().into());
            entry.insert("rp_mode".into(), sheet.rp_mode.into());
            map.insert(player.clone(), serde_json::Value::Object(entry));
        }
        (path, serde_json::Value::Object(map).to_string())
    });
    if path.is_empty() {
        return Ok(());
    }
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn load_sheets() {
    let (path, _) = with_state(|s| (format!("{}/sheets.json", s.data_folder), ()));
    if path.is_empty() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&content) else {
        return;
    };
    with_state(|s| {
        for (player, entry) in map {
            let get = |key: &str| {
                entry
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let rp_mode = entry
                .get("rp_mode")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            s.sheets.insert(
                player,
                CharacterSheet {
                    name: get("name"),
                    description: get("description"),
                    job: get("job"),
                    rp_mode,
                },
            );
        }
    });
}

fn rp_display_name(player_name: &str) -> String {
    let sheet = sheet_of(player_name);
    if sheet.name.is_empty() {
        player_name.to_string()
    } else {
        sheet.name
    }
}

fn arg_string(args: &ConsumedArgs, key: &str) -> String {
    match args.get_value(key) {
        Arg::Simple(s) | Arg::Msg(s) => s,
        _ => String::new(),
    }
}

fn legacy(component: &TextComponent, message: &str) -> TextComponent {
    let _ = component;
    TextComponent::from_legacy_string(message)
}

// --- Commandes ---

struct RollHandler;

impl CommandHandler for RollHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let faces = arg_string(&args, "faces").parse::<u32>().unwrap_or(100);
        let faces = faces.clamp(2, 1000);
        let result = pseudo_random(1, faces);
        let name = sender.get_name();
        let message = format!(
            "&e{} &7lance un dé à &e{}&7 faces : &6{}",
            name, faces, result
        );
        broadcast_message(_server, &message);
        Ok(0)
    }
}

fn pseudo_random(min: u32, max: u32) -> u32 {
    if max <= min {
        return min;
    }
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    min + t.wrapping_mul(2654435761).wrapping_add(ticks) % (max - min + 1)
}

fn broadcast_message(server: Server, message: &str) {
    for player in server.get_all_players() {
        let component = TextComponent::from_legacy_string(message);
        player.send_system_message(component, false);
    }
}

struct MeHandler;

impl CommandHandler for MeHandler {
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let action = arg_string(&args, "action");
        if action.is_empty() {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Usage : /me <action>",
            )));
        }
        let display = rp_display_name(&sender.get_name());
        broadcast_message(server, &format!("&7* {} {} &7*", display, action));
        Ok(0)
    }
}

struct DoHandler;

impl CommandHandler for DoHandler {
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let action = arg_string(&args, "action");
        if action.is_empty() {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Usage : /do <action>",
            )));
        }
        let display = rp_display_name(&sender.get_name());
        broadcast_message(server, &format!("&7&o[RP] &f{} &7&o: {}", display, action));
        Ok(0)
    }
}

struct HrpHandler;

impl CommandHandler for HrpHandler {
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let message = arg_string(&args, "message");
        if message.is_empty() {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Usage : /hrp <message>",
            )));
        }
        broadcast_message(
            server,
            &format!("&8&o[HRP] &7{} : &8{}", sender.get_name(), message),
        );
        Ok(0)
    }
}

struct FicheSetHandler;

impl CommandHandler for FicheSetHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let player = sender.get_name();
        let mut sheet = sheet_of(&player);
        match args.get_value("field") {
            Arg::Simple(field) => {
                let value = arg_string(&args, "value");
                match field.as_str() {
                    "nom" => sheet.name = value,
                    "metier" => sheet.job = value,
                    "description" => sheet.description = value,
                    _ => {
                        return Err(CommandError::CommandFailed(TextComponent::text(
                            "Champ inconnu. Champs : nom, metier, description",
                        )))
                    }
                }
                set_sheet(&player, sheet);
                let _ = save_sheets();
                sender.send_message(TextComponent::text("Fiche mise à jour."));
                Ok(0)
            }
            _ => Err(CommandError::CommandFailed(TextComponent::text(
                "Champ invalide.",
            ))),
        }
    }
}

struct FicheShowHandler;

impl CommandHandler for FicheShowHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let target = match args.get_value("joueur") {
            Arg::Simple(name) => name,
            _ => sender.get_name(),
        };
        let sheet = sheet_of(&target);
        let name = if sheet.name.is_empty() {
            "(non défini)"
        } else {
            &sheet.name
        };
        let job = if sheet.job.is_empty() {
            "(non défini)"
        } else {
            &sheet.job
        };
        let description = if sheet.description.is_empty() {
            "(non définie)"
        } else {
            &sheet.description
        };
        let text = format!(
            "&6--- Fiche de {} ---\n&7Nom : &f{}\n&7Métier : &f{}\n&7Description : &f{}",
            target, name, job, description
        );
        sender.send_message(legacy(&TextComponent::text(""), &text));
        Ok(0)
    }
}

struct MetierHandler;

impl CommandHandler for MetierHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let Some(player) = sender.as_player() else {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Cette commande est réservée aux joueurs.",
            )));
        };
        let metier = arg_string(&args, "id");
        let Some(outfit_file) = skins::outfit_file(&metier) else {
            let known = skins::METIERS
                .iter()
                .map(|(m, _)| *m)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "Métier inconnu. Métiers disponibles : {}",
                known
            ))));
        };

        let (data_folder, outfit_png) = with_state(|s| {
            (
                s.data_folder.clone(),
                std::fs::read(format!("{}/{}", s.data_folder, outfit_file)),
            )
        });
        let _outfit_png: Vec<u8> = outfit_png.map_err(|e| {
            CommandError::CommandFailed(TextComponent::text(&format!(
                "Tenue « {} » introuvable dans le dossier de données ({}) — \
                 placez-y le PNG généré par skins/gen_skins.py",
                metier, e
            )))
        })?;

        let Some(skin) = player.get_skin() else {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Skin du joueur introuvable.",
            )));
        };
        let _ = data_folder;
        let skin_url = skins::extract_skin_url(&skin).map_err(|e| {
            CommandError::CommandFailed(TextComponent::text(&format!(
                "Impossible de lire le skin actuel : {}",
                e
            )))
        })?;
        // Le téléchargement du PNG du joueur et l'upload mineskin nécessitent
        // un client HTTP, non exposé par le plugin API 0.2. Le composite doit
        // être pré-généré hors-ligne : voir skins/gen_skins.py.
        let _ = skin_url;
        Err(CommandError::CommandFailed(TextComponent::text(
            "L'application de tenue nécessite un composite pré-généré \
             (skins/gen_skins.py) — l'upload automatique sera disponible \
             quand le plugin API exposera un client HTTP",
        )))
    }
}

struct RpModeHandler;

impl CommandHandler for RpModeHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let player = sender.get_name();
        let mut sheet = sheet_of(&player);
        let enable = match args.get_value("etat") {
            Arg::Bool(true) => true,
            Arg::Bool(false) => false,
            _ => !sheet.rp_mode,
        };
        sheet.rp_mode = enable;
        set_sheet(&player, sheet);
        let _ = save_sheets();
        let state = if enable { "activé" } else { "désactivé" };
        sender.send_message(TextComponent::text(&format!("Mode RP {}.", state)));
        Ok(0)
    }
}

// --- Livres configurables ---

struct LivreHandler;

impl CommandHandler for LivreHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let Some(player) = sender.as_player() else {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Cette commande est réservée aux joueurs.",
            )));
        };
        let id = arg_string(&args, "id");
        let book = with_state(|s| s.books.get(&id).cloned());
        let Some(book) = book else {
            let known = with_state(|s| books::list_ids(&s.books));
            return Err(CommandError::CommandFailed(TextComponent::text(
                &if known.is_empty() {
                    "Aucun livre disponible. Placez des fichiers JSON dans plugins/data/rpumpkin/livres/".into()
                } else {
                    format!("Livre « {id} » introuvable. Livres disponibles : {known}")
                },
            )));
        };

        let stack = ItemStack::new("minecraft:written_book", 1);
        stack.set_component(
            DataComponent::WrittenBookContent,
            &books::written_book_content(&book),
        );
        let title = TextComponent::from_legacy_string(&book.title);
        stack.set_custom_name(Some(title));

        let inventory = player.get_inventory().as_inventory();
        let size = inventory.get_size();
        let mut placed = false;
        for slot in 0..size {
            if inventory.get_item(slot).is_none() {
                inventory.set_item(slot, Some(stack));
                placed = true;
                break;
            }
        }
        if !placed {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Inventaire plein. Libérez de la place pour recevoir le livre.",
            )));
        }
        sender.send_message(TextComponent::text(&format!(
            "Vous avez reçu le livre « {} ».",
            book.title
        )));
        Ok(0)
    }
}

struct LivreListHandler;

impl CommandHandler for LivreListHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let known = with_state(|s| books::list_ids(&s.books));
        let message = if known.is_empty() {
            "Aucun livre disponible.".to_string()
        } else {
            format!("Livres disponibles : {known}")
        };
        sender.send_message(TextComponent::text(&message));
        Ok(0)
    }
}

// --- Chat RP : formatte les messages des joueurs en mode RP ---

struct ChatHandler;

impl EventHandler<PlayerChatEvent> for ChatHandler {
    fn handle(
        &self,
        _server: Server,
        mut event: pumpkin_plugin_api::events::EventData<PlayerChatEvent>,
    ) -> pumpkin_plugin_api::events::EventData<PlayerChatEvent> {
        let player_name = event.player.get_name();
        let sheet = sheet_of(&player_name);
        if !sheet.rp_mode {
            return event;
        }
        let display = rp_display_name(&player_name);
        let message = format!("&f[RP] &7{} &f: {}", display, event.message);
        for recipient in &event.recipients {
            let component = TextComponent::from_legacy_string(&message);
            recipient.send_system_message(component, false);
        }
        event.message = String::new();
        event
    }
}

// --- Plugin ---

struct RPumpkin;

impl Plugin for RPumpkin {
    fn new() -> Self {
        RPumpkin
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "rpumpkin".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["damienmillet".into()],
            description: "Notions de RolePlay unifiées pour serveur Pumpkin".into(),
            dependencies: vec![],
            permissions: vec![FS_READ_DATA.into(), FS_WRITE_DATA.into()],
        }
    }

    fn on_load(&self, context: Context) -> pumpkin_plugin_api::Result<()> {
        let data_folder = context.get_data_folder();
        let plugin_config = config::Config::load(&data_folder);
        let loaded_books = books::load_books(&data_folder);
        with_state(|s| {
            s.data_folder = data_folder;
            s.plugin_config = plugin_config;
            s.books = loaded_books;
        });
        load_sheets();

        let chat_enabled = with_config(|c| c.enabled(config::FEATURE_CHAT_RP));
        if chat_enabled {
            context.register_event_handler(ChatHandler, EventPriority::Normal, true)?;
        }

        if with_config(|c| c.enabled(config::FEATURE_SHEETS)) {
            context.register_permission(&Permission {
                node: "rpumpkin.command.fiche".into(),
                description: "Gérer sa fiche de personnage RP".into(),
                default: PermissionDefault::Allow,
                children: vec![],
            })?;

            let fiche = Command::new(&["fiche".to_string()], "Gérer sa fiche de personnage RP")
                .then(
                    CommandNode::literal("set").then(
                        CommandNode::argument(
                            "field",
                            &ArgumentType::String(StringType::SingleWord),
                        )
                        .then(
                            CommandNode::argument(
                                "value",
                                &ArgumentType::String(StringType::Greedy),
                            )
                            .execute(FicheSetHandler),
                        ),
                    ),
                )
                .then(
                    CommandNode::argument("joueur", &ArgumentType::String(StringType::SingleWord))
                        .execute(FicheShowHandler),
                )
                .execute(FicheShowHandler);
            context.register_command(fiche, "rpumpkin.command.fiche");
        }

        if with_config(|c| c.enabled(config::FEATURE_RP_COMMANDS)) {
            context.register_permission(&Permission {
                node: "rpumpkin.command.rp".into(),
                description: "Commandes RolePlay (roll, me, do, hrp)".into(),
                default: PermissionDefault::Allow,
                children: vec![],
            })?;

            let roll = Command::new(&["roll".to_string()], "Lancer un dé").then(
                CommandNode::argument("faces", &ArgumentType::Integer((Some(2), Some(1000))))
                    .execute(RollHandler),
            );
            context.register_command(roll, "rpumpkin.command.rp");

            let me = Command::new(&["me".to_string()], "Décrire une action RP").then(
                CommandNode::argument("action", &ArgumentType::String(StringType::Greedy))
                    .execute(MeHandler),
            );
            context.register_command(me, "rpumpkin.command.rp");

            let do_cmd = Command::new(&["do".to_string()], "Décrire un événement RP").then(
                CommandNode::argument("action", &ArgumentType::String(StringType::Greedy))
                    .execute(DoHandler),
            );
            context.register_command(do_cmd, "rpumpkin.command.rp");

            let hrp = Command::new(&["hrp".to_string()], "Message hors-RP").then(
                CommandNode::argument("message", &ArgumentType::String(StringType::Greedy))
                    .execute(HrpHandler),
            );
            context.register_command(hrp, "rpumpkin.command.rp");

            let rp = Command::new(&["rp".to_string()], "Activer/désactiver le mode RP")
                .then(CommandNode::argument("etat", &ArgumentType::Bool).execute(RpModeHandler));
            context.register_command(rp, "rpumpkin.command.rp");
        }

        if with_config(|c| c.enabled(config::FEATURE_METIER)) {
            context.register_permission(&Permission {
                node: "rpumpkin.command.metier".into(),
                description: "Choisir sa tenue de métier".into(),
                default: PermissionDefault::Allow,
                children: vec![],
            })?;
            let metier = Command::new(
                &["metier".to_string()],
                "Choisir sa tenue de métier (tête conservée)",
            )
            .then(
                CommandNode::argument("id", &ArgumentType::String(StringType::SingleWord))
                    .execute(MetierHandler),
            );
            context.register_command(metier, "rpumpkin.command.metier");
        }

        if with_config(|c| c.enabled(config::FEATURE_BOOKS)) {
            context.register_permission(&Permission {
                node: "rpumpkin.command.livre".into(),
                description: "Recevoir un livre configurable côté serveur".into(),
                default: PermissionDefault::Allow,
                children: vec![],
            })?;
            let livre = Command::new(&["livre".to_string()], "Recevoir un livre serveur")
                .then(
                    CommandNode::argument("id", &ArgumentType::String(StringType::SingleWord))
                        .execute(LivreHandler),
                )
                .execute(LivreListHandler);
            context.register_command(livre, "rpumpkin.command.livre");
        }

        Ok(())
    }

    fn on_unload(&self, _context: Context) -> pumpkin_plugin_api::Result<()> {
        save_sheets()?;
        Ok(())
    }
}

register_plugin!(RPumpkin);
