//! Fonctionnalite "sightline" : les plaques de nom ne sont visibles que lorsque
//! la ligne de vue entre les deux joueurs est degagee (equivalent Pumpkin de VisTag).
//!
//! Chaque joueur recoit son propre scoreboard avec une equipe `rpumpkin_hidden`
//! (`nametag-visibility = never`) : y placer un joueur masque sa plaque pour cet
//! observateur uniquement. Toutes les `check-interval-ticks`, chaque paire de
//! joueurs d'un meme monde est reevaluee (bascule admin, sneak, distance max,
//! cone de FOV, ligne de vue) et l'appartenance a l'equipe est mise a jour.
//!
//! Parametres, listes de blocs et bascules sont persistants dans
//! `<data_folder>/sightline.json`.

use std::collections::HashSet;

use pumpkin_plugin_api::command::{
    Arg, ArgumentType, CommandError, CommandNode, CommandSender, ConsumedArgs, StringType,
};
use pumpkin_plugin_api::commands::CommandHandler;
use pumpkin_plugin_api::common::BlockPos;
use pumpkin_plugin_api::events::player::PlayerLeaveEvent;
use pumpkin_plugin_api::events::server::ServerTickStartEvent;
use pumpkin_plugin_api::events::{EventData, EventHandler};
use pumpkin_plugin_api::scoreboard::Scoreboard;
use pumpkin_plugin_api::text::TextComponent;
use pumpkin_plugin_api::world::BoundingBox;
use pumpkin_plugin_api::{
    BlockState, NametagVisibility, Player, Server, TeamSettingsBuilder, World,
};

/// Nom de l'equipe "plaque cachee", par observateur (16 caracteres max).
pub const TEAM_NAME: &str = "rpumpkin_hidden";

/// Pas d'echantillonnage du rayon, en blocs.
const STEP: f64 = 0.5;

type Vec3 = (f64, f64, f64);

/// Parametres de la fonctionnalite, charges depuis `sightline.json`.
#[derive(Clone)]
pub struct Settings {
    /// Intervalle de verification, en ticks (20 = 1 s).
    pub check_interval_ticks: u32,
    /// Distance max (blocs) a laquelle une plaque peut etre visible.
    pub max_distance: f64,
    /// Masque les plaques des joueurs qui sneak.
    pub hide_when_sneaking: bool,
    /// Demi-angle du cone de FOV en degres (0 = desactive).
    pub fov_cone_degrees: f64,
    /// true = rayon exact sur les formes de collision, false = test par centre de bloc.
    pub exact_raycast: bool,
    /// Blocs qui ne masquent jamais la plaque (priorite la plus haute).
    pub excluded: HashSet<String>,
    /// Blocs qui masquent toujours la plaque, meme transparents.
    pub included: HashSet<String>,
    /// UUID des joueurs dont le filtrage est desactive par un admin.
    pub toggled_off: HashSet<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            check_interval_ticks: 10,
            max_distance: 64.0,
            hide_when_sneaking: true,
            fov_cone_degrees: 0.0,
            exact_raycast: true,
            excluded: HashSet::new(),
            included: HashSet::new(),
            toggled_off: HashSet::new(),
        }
    }
}

impl Settings {
    /// Charge les parametres depuis `path` ; cree le fichier avec les valeurs par
    /// defaut s'il est absent.
    pub fn load(path: &str) -> Settings {
        let mut settings = Settings::default();
        match std::fs::read_to_string(path) {
            Ok(content) => {
                if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&content) {
                    if let Some(v) = map.get("check-interval-ticks").and_then(|v| v.as_u64()) {
                        settings.check_interval_ticks = (v as u32).clamp(1, 1200);
                    }
                    if let Some(v) = map.get("max-distance").and_then(|v| v.as_f64()) {
                        settings.max_distance = v.max(1.0);
                    }
                    if let Some(v) = map.get("hide-when-sneaking").and_then(|v| v.as_bool()) {
                        settings.hide_when_sneaking = v;
                    }
                    if let Some(v) = map.get("fov-cone-degrees").and_then(|v| v.as_f64()) {
                        settings.fov_cone_degrees = v.max(0.0);
                    }
                    if let Some(v) = map.get("exact-raycast").and_then(|v| v.as_bool()) {
                        settings.exact_raycast = v;
                    }
                    let read_list = |key: &str| -> HashSet<String> {
                        map.get(key)
                            .and_then(|v| v.as_array())
                            .map(|list| {
                                list.iter()
                                    .filter_map(|e| e.as_str().map(normalize_block))
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    settings.excluded = read_list("excluded");
                    settings.included = read_list("included");
                    settings.toggled_off = read_list("toggled-off-players");
                }
            }
            Err(_) => {
                let _ = std::fs::write(path, settings.to_json());
            }
        }
        settings
    }

    /// Persiste les parametres dans `path`.
    pub fn save(&self, path: &str) -> Result<(), String> {
        std::fs::write(path, self.to_json()).map_err(|e| e.to_string())
    }

    /// Serialise les parametres en JSON lisible.
    pub fn to_json(&self) -> String {
        let list = |set: &HashSet<String>| -> serde_json::Value {
            let mut items: Vec<&String> = set.iter().collect();
            items.sort();
            items
                .into_iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect::<Vec<_>>()
                .into()
        };
        let mut map = serde_json::Map::new();
        map.insert(
            "check-interval-ticks".into(),
            (self.check_interval_ticks as u64).into(),
        );
        map.insert("max-distance".into(), self.max_distance.into());
        map.insert("hide-when-sneaking".into(), self.hide_when_sneaking.into());
        map.insert("fov-cone-degrees".into(), self.fov_cone_degrees.into());
        map.insert("exact-raycast".into(), self.exact_raycast.into());
        map.insert("excluded".into(), list(&self.excluded));
        map.insert("included".into(), list(&self.included));
        map.insert("toggled-off-players".into(), list(&self.toggled_off));
        serde_json::Value::Object(map).to_string()
    }
}

/// Normalise un identifiant de bloc : minuscules, prefixe `minecraft:` si absent.
pub fn normalize_block(raw: &str) -> String {
    let name = raw.trim().to_lowercase();
    if name.contains(':') {
        name
    } else {
        format!("minecraft:{name}")
    }
}

// --- Geometrie ---

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    (a.0 - b.0, a.1 - b.1, a.2 - b.2)
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2)
}

fn mul(a: Vec3, k: f64) -> Vec3 {
    (a.0 * k, a.1 * k, a.2 * k)
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
}

fn length(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

fn normalize(a: Vec3) -> Vec3 {
    let l = length(a);
    if l < 1.0e-9 {
        (0.0, 0.0, 0.0)
    } else {
        mul(a, 1.0 / l)
    }
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    length(sub(a, b))
}

/// Direction du regard d'un joueur a partir de son yaw/pitch (convention Minecraft).
fn look_direction(yaw: f32, pitch: f32) -> Vec3 {
    let yaw = f64::from(yaw).to_radians();
    let pitch = f64::from(pitch).to_radians();
    (
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

fn block_pos_of(p: Vec3) -> BlockPos {
    BlockPos {
        x: p.0.floor() as i32,
        y: p.1.floor() as i32,
        z: p.2.floor() as i32,
    }
}

// --- Ligne de vue ---

/// true si le bloc masque une plaque : `included` > comportement par defaut
/// (cube plein opaque) ; `excluded` ne masque jamais (priorite la plus haute).
fn hides(settings: &Settings, state: &BlockState) -> bool {
    if state.is_air || state.is_liquid {
        return false;
    }
    if settings.excluded.contains(&state.block_name) {
        return false;
    }
    if settings.included.contains(&state.block_name) {
        return true;
    }
    state.is_full_cube && state.opacity >= 15
}

/// true si aucun bloc masquant ne separe les deux positions des yeux.
fn has_line_of_sight(world: &World, from: Vec3, to: Vec3, settings: &Settings) -> bool {
    let dist = distance(from, to);
    // Marge pres des deux yeux pour eviter les faux positifs a courte distance.
    let margin = 1.0f64.min(dist * 0.25);
    let direction = normalize(sub(to, from));
    let start = add(from, mul(direction, margin));
    let ray_length = dist - 2.0 * margin;
    if ray_length <= 0.0 {
        return true;
    }
    if settings.exact_raycast {
        exact_ray_free(world, start, direction, ray_length, settings)
    } else {
        coarse_ray_free(world, from, direction, dist, margin, settings)
    }
}

/// Rayon exact : pour chaque bloc candidat le long du segment, intersection
/// segment-vs-boite avec chaque forme de collision du bloc (methode des dalles).
fn exact_ray_free(
    world: &World,
    start: Vec3,
    direction: Vec3,
    length: f64,
    settings: &Settings,
) -> bool {
    let end = add(start, mul(direction, length));
    let mut candidates: HashSet<(i32, i32, i32)> = HashSet::new();
    let mut t = 0.0;
    while t <= length {
        let pos = block_pos_of(add(start, mul(direction, t)));
        candidates.insert((pos.x, pos.y, pos.z));
        t += STEP;
    }
    for (bx, by, bz) in candidates {
        let state = world.get_block_state(BlockPos {
            x: bx,
            y: by,
            z: bz,
        });
        if !hides(settings, &state) {
            continue;
        }
        if state.collision_shapes.is_empty() {
            // Pas de forme de collision (ex. torche) : cube unitaire du bloc.
            let unit = BoundingBox {
                min: (bx as f64, by as f64, bz as f64),
                max: (bx as f64 + 1.0, by as f64 + 1.0, bz as f64 + 1.0),
            };
            if point_in_box(start, &unit)
                || point_in_box(end, &unit)
                || segment_intersects_box(start, end, &unit)
            {
                return false;
            }
        } else {
            for shape in &state.collision_shapes {
                if point_in_box(start, shape)
                    || point_in_box(end, shape)
                    || segment_intersects_box(start, end, shape)
                {
                    return false;
                }
            }
        }
    }
    true
}

/// Rayon grossier : echantillonne le segment et teste le bloc de chaque echantillon.
fn coarse_ray_free(
    world: &World,
    from: Vec3,
    direction: Vec3,
    dist: f64,
    margin: f64,
    settings: &Settings,
) -> bool {
    let mut t = margin;
    while t < dist - margin {
        let state = world.get_block_state(block_pos_of(add(from, mul(direction, t))));
        if hides(settings, &state) {
            return false;
        }
        t += STEP;
    }
    true
}

fn point_in_box(p: Vec3, b: &BoundingBox) -> bool {
    p.0 >= b.min.0
        && p.0 <= b.max.0
        && p.1 >= b.min.1
        && p.1 <= b.max.1
        && p.2 >= b.min.2
        && p.2 <= b.max.2
}

/// Intersection segment-AABB (methode des dalles).
fn segment_intersects_box(start: Vec3, end: Vec3, b: &BoundingBox) -> bool {
    let d = [end.0 - start.0, end.1 - start.1, end.2 - start.2];
    let s = [start.0, start.1, start.2];
    let lo = [b.min.0, b.min.1, b.min.2];
    let hi = [b.max.0, b.max.1, b.max.2];
    let mut t_min = 0.0f64;
    let mut t_max = 1.0f64;
    for i in 0..3 {
        if d[i].abs() < 1.0e-9 {
            if s[i] < lo[i] || s[i] > hi[i] {
                return false;
            }
        } else {
            let mut t1 = (lo[i] - s[i]) / d[i];
            let mut t2 = (hi[i] - s[i]) / d[i];
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return false;
            }
        }
    }
    true
}

// --- Visibilite d'une plaque ---

/// true si la plaque de `target` doit etre visible pour `viewer`.
pub fn is_visible_to(viewer: &Player, target: &Player, settings: &Settings) -> bool {
    if settings.toggled_off.contains(&viewer.get_id().to_string()) {
        return true;
    }
    if settings.hide_when_sneaking && target.as_entity().is_sneaking() {
        return false;
    }
    let from = viewer.as_entity().get_eye_position();
    let to = target.as_entity().get_eye_position();
    let dist = distance(from, to);
    if dist > settings.max_distance {
        return false;
    }
    if dist < 1.0 {
        return true;
    }
    if settings.fov_cone_degrees > 0.0 {
        let look = look_direction(viewer.get_yaw(), viewer.get_pitch());
        let to_target = normalize(sub(to, from));
        if dot(look, to_target) < settings.fov_cone_degrees.to_radians().cos() {
            return false;
        }
    }
    has_line_of_sight(&viewer.get_world(), from, to, settings)
}

/// Cree l'equipe cachee sur le scoreboard du joueur si elle n'existe pas.
fn ensure_hidden_team(board: &Scoreboard) {
    if board.get_team(TEAM_NAME).is_none() {
        let team_settings = TeamSettingsBuilder::new()
            .nametag_visibility(NametagVisibility::Never)
            .see_friendly_invisibles(false)
            .build();
        board.create_team(TEAM_NAME, team_settings);
    }
}

/// Reevalue toutes les paires viewer/cible et met a jour les equipes cachees.
pub fn update_all_viewers(server: &Server) {
    let settings = crate::with_state(|s| s.sightline.clone());
    let players = server.get_all_players();
    for viewer in &players {
        let Some(java) = viewer.as_java() else {
            continue;
        };
        let board = java.get_scoreboard();
        ensure_hidden_team(&board);
        let hidden: HashSet<String> = board.get_team_players(TEAM_NAME).into_iter().collect();
        let world_id = viewer.get_world().get_id();
        let viewer_name = viewer.get_name();
        for target in &players {
            let target_name = target.get_name();
            if target_name == viewer_name {
                continue;
            }
            if target.get_world().get_id() != world_id {
                continue;
            }
            let visible = is_visible_to(viewer, target, &settings);
            let is_hidden = hidden.contains(&target_name);
            if visible && is_hidden {
                board.remove_player_from_team(TEAM_NAME, &target_name);
            } else if !visible && !is_hidden {
                board.add_player_to_team(TEAM_NAME, &target_name);
            }
        }
    }
}

// --- Evenements ---

/// Verification periodique de la visibilite des plaques.
pub struct TickHandler;

impl EventHandler<ServerTickStartEvent> for TickHandler {
    fn handle(
        &self,
        server: Server,
        event: EventData<ServerTickStartEvent>,
    ) -> EventData<ServerTickStartEvent> {
        let interval = crate::with_state(|s| s.sightline.check_interval_ticks);
        if event.tick % interval.max(1) as i32 == 0 {
            update_all_viewers(&server);
        }
        event
    }
}

/// Retire le joueur sortant des equipes cachees des autres joueurs.
pub struct LeaveHandler;

impl EventHandler<PlayerLeaveEvent> for LeaveHandler {
    fn handle(
        &self,
        server: Server,
        event: EventData<PlayerLeaveEvent>,
    ) -> EventData<PlayerLeaveEvent> {
        let leaver = event.player.get_name();
        for viewer in server.get_all_players() {
            if viewer.get_name() == leaver {
                continue;
            }
            if let Some(java) = viewer.as_java() {
                let board = java.get_scoreboard();
                if board.get_team(TEAM_NAME).is_some() {
                    board.remove_player_from_team(TEAM_NAME, &leaver);
                }
            }
        }
        event
    }
}

// --- Commandes ---

fn settings_path() -> String {
    crate::with_state(|s| format!("{}/sightline.json", s.data_folder))
}

/// `/sightline toggle <players...>` : active/desactive le filtrage pour des joueurs.
pub struct ToggleHandler;

impl CommandHandler for ToggleHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let targets = match args.get_value("players") {
            Arg::Players(players) => players,
            _ => Vec::new(),
        };
        if targets.is_empty() {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Joueur introuvable.",
            )));
        }
        let mut count = 0;
        for target in &targets {
            let uuid = target.get_id().to_string();
            let enabled = crate::with_state(|s| {
                let enabled = if s.sightline.toggled_off.remove(&uuid) {
                    true
                } else {
                    s.sightline.toggled_off.insert(uuid);
                    false
                };
                let _ = s
                    .sightline
                    .save(&format!("{}/sightline.json", s.data_folder));
                enabled
            });
            let state = if enabled { "active" } else { "desactive" };
            let name = target.get_name();
            sender.send_message(TextComponent::from_legacy_string(&format!(
                "&a[sightline] &7Filtrage des plaques {} pour &f{}.",
                state, name
            )));
            target.send_system_message(
                TextComponent::from_legacy_string(&if enabled {
                    "&a[sightline] Le filtrage des plaques est reactive pour vous.".to_string()
                } else {
                    "&e[sightline] Le filtrage des plaques est desactive pour vous : vous voyez toutes les plaques.".to_string()
                }),
                false,
            );
            count += 1;
        }
        Ok(count)
    }
}

/// Construit le sous-arbre `<liste> add|remove|list` pour `excluded`/`included`.
pub fn list_command(list: &'static str) -> CommandNode {
    CommandNode::literal(list)
        .then(
            CommandNode::literal("add").then(
                CommandNode::argument("block", &ArgumentType::String(StringType::SingleWord))
                    .execute(ListModifyHandler { list, add: true }),
            ),
        )
        .then(
            CommandNode::literal("remove").then(
                CommandNode::argument("block", &ArgumentType::String(StringType::SingleWord))
                    .execute(ListModifyHandler { list, add: false }),
            ),
        )
        .then(CommandNode::literal("list").execute(ListShowHandler { list }))
}

/// `/sightline <liste> add|remove <bloc>` : modifie une liste de blocs persistante.
pub struct ListModifyHandler {
    list: &'static str,
    add: bool,
}

impl CommandHandler for ListModifyHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let raw = crate::arg_string(&args, "block");
        if raw.is_empty() {
            return Err(CommandError::CommandFailed(TextComponent::text(
                "Usage : /sightline <excluded|included> <add|remove> <bloc>",
            )));
        }
        let name = normalize_block(&raw);
        if pumpkin_plugin_api::world::resolve_block_state(&name, &[]).is_none() {
            return Err(CommandError::CommandFailed(TextComponent::text(&format!(
                "Bloc inconnu : {raw}"
            ))));
        }
        let (changed, name) = crate::with_state(|s| {
            let set = if self.list == "excluded" {
                &mut s.sightline.excluded
            } else {
                &mut s.sightline.included
            };
            let changed = if self.add {
                set.insert(name.clone())
            } else {
                set.remove(&name)
            };
            if changed {
                let _ = s
                    .sightline
                    .save(&format!("{}/sightline.json", s.data_folder));
            }
            (changed, name)
        });
        if changed {
            let action = if self.add { "ajoute a" } else { "retire de" };
            sender.send_message(TextComponent::from_legacy_string(&format!(
                "&a[sightline] &f{} &7{} la liste '{}'.",
                name, action, self.list
            )));
            Ok(1)
        } else {
            let already = if self.add { "deja dans" } else { "absent de" };
            sender.send_message(TextComponent::from_legacy_string(&format!(
                "&e[sightline] &f{} &7{} la liste '{}'.",
                name, already, self.list
            )));
            Ok(0)
        }
    }
}

/// `/sightline <liste> list` : affiche une liste de blocs.
pub struct ListShowHandler {
    list: &'static str,
}

impl CommandHandler for ListShowHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let (empty, items) = crate::with_state(|s| {
            let set = if self.list == "excluded" {
                &s.sightline.excluded
            } else {
                &s.sightline.included
            };
            let mut items: Vec<&String> = set.iter().collect();
            items.sort();
            (
                items.is_empty(),
                items
                    .into_iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        });
        if empty {
            sender.send_message(TextComponent::from_legacy_string(&format!(
                "&7[sightline] La liste '{}' est vide.",
                self.list
            )));
        } else {
            sender.send_message(TextComponent::from_legacy_string(&format!(
                "&e[sightline] {} : &f{}",
                self.list, items
            )));
        }
        Ok(1)
    }
}

/// `/sightline reload` : recharge `sightline.json` depuis le disque.
pub struct ReloadHandler;

impl CommandHandler for ReloadHandler {
    fn handle(
        &self,
        sender: CommandSender,
        _server: Server,
        _args: ConsumedArgs,
    ) -> Result<i32, CommandError> {
        let path = settings_path();
        crate::with_state(|s| s.sightline = Settings::load(&path));
        sender.send_message(TextComponent::from_legacy_string(
            "&a[sightline] Configuration rechargee.",
        ));
        Ok(1)
    }
}
