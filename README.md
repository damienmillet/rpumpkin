# RPumpkin

**RPumpkin** est une solution unifiée pour ajouter des notions de RolePlay Minecraft dans un serveur [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin).

C'est un plugin Pumpkin (WebAssembly, écrit en Rust) qui regroupe les briques RolePlay essentielles : fiches de personnage persistantes, chat RP formaté, et commandes d'action RolePlay.

## Fonctionnalités

### Fiches de personnage
- `/fiche` — affiche votre fiche
- `/fiche <joueur>` — affiche la fiche d'un joueur
- `/fiche set nom <nom>` — définit le nom RP de votre personnage
- `/fiche set metier <métier>` — définit le métier du personnage
- `/fiche set description <texte>` — définit l'histoire/description

Les fiches sont persistées dans `plugins/data/rpumpkin/sheets.json` et rechargées au démarrage.

### Commandes RolePlay
- `/roll [faces]` — lance un dé (2 à 1000 faces, 100 par défaut), résultat annoncé à tous
- `/me <action>` — décrit une action à la 3e personne (« \* Jean frappe \* »)
- `/do <événement>` — décrit un événement/état RP
- `/hrp <message>` — message Hors-RP, clairement balisé
- `/rp [true|false]` — active/désactive le mode RP (bascule si sans argument)

### Tenues de métier
- `/metier <fermier|mineur>` — applique la tenue du métier en conservant la tête du joueur

Le skin Minecraft étant une texture unique 64x64, la tenue est un composite : tête du joueur + corps de la tenue. Les tenues sont générées par `skins/gen_skins.py` (PNG 64x64, zone tête transparente) et doivent être placées dans le dossier de données du plugin (`plugins/data/rpumpkin/tenues/`).

**Contrainte API** : le plugin API 0.2 n'expose pas de client HTTP, l'upload automatique mineskin.org n'est donc pas possible depuis le plugin. Les composites doivent être pré-générés. L'application automatique (téléchargement du skin joueur + upload mineskin) sera activée quand l'API exposera un client HTTP.

### Chat RP
Quand le mode RP est activé, les messages de chat du joueur sont automatiquement formatés `[RP] <NomRP> : message`, en utilisant le nom de la fiche plutôt que le pseudo Minecraft.

## Permissions

| Nœud | Description | Défaut |
|------|-------------|--------|
| `rpumpkin.command.fiche` | Gérer sa fiche de personnage | Autorisé |
| `rpumpkin.command.rp` | Commandes RolePlay (roll, me, do, hrp) | Autorisé |
| `rpumpkin.command.metier` | Choisir sa tenue de métier | Autorisé |

Le plugin demande aussi les permissions d'accès à son dossier de données (`fs.read.data`, `fs.write.data`).

## Build

Prérequis : Rust (via rustup) et la cible WASI :

```bash
rustup target add wasm32-wasip2
cargo build --release
```

Le fichier `target/wasm32-wasip2/release/rpumpkin.wasm` est à placer dans le dossier `plugins/` du serveur Pumpkin.

## Structure du projet

```
├── .cargo/config.toml   # cible de build wasm32-wasip2
├── Cargo.toml
├── skins/               # générateur de tenues + tenues de test (b64)
└── src/
    ├── lib.rs           # plugin complet
    └── skins/           # module tenues de métier (composite tête/corps)
```

## Roadmap

- Canaux de chat par proximité (radius RP)
- Système d'inventaire RP (items fictifs)
- Intégration de teams/factions RP
- Événements scriptés (PNJ, quêtes)

## Licence

MIT OR Apache-2.0
