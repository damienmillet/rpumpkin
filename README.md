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

### Chat RP
Quand le mode RP est activé, les messages de chat du joueur sont automatiquement formatés `[RP] <NomRP> : message`, en utilisant le nom de la fiche plutôt que le pseudo Minecraft.

## Permissions

| Nœud | Description | Défaut |
|------|-------------|--------|
| `rpumpkin.command.fiche` | Gérer sa fiche de personnage | Autorisé |
| `rpumpkin.command.rp` | Commandes RolePlay (roll, me, do, hrp) | Autorisé |

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
└── src/lib.rs            # plugin complet
```

## Roadmap

- Canaux de chat par proximité (radius RP)
- Système d'inventaire RP (items fictifs)
- Intégration de teams/factions RP
- Événements scriptés (PNJ, quêtes)

## Licence

MIT OR Apache-2.0
