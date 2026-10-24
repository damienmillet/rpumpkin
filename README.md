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

### Livres configurables
- `/livre` — liste les livres disponibles
- `/livre <id>` — reçoit le livre dans son inventaire (premier slot libre)

Le contenu est défini côté serveur, sans recompilation : chaque fichier JSON déposé dans `plugins/data/rpumpkin/livres/` devient un livre utilisable en jeu. Exemple `plugins/data/rpumpkin/livres/reglement.json` :

```json
{
  "titre": "Règlement de la ville",
  "auteur": "La Mairie",
  "pages": [
    "&lRèglement de la ville&\n&7Bienvenue, citoyen !",
    "&eArticle 1&f : restez courtois envers les autres habitants.",
    "&eArticle 2&f : le HRP se fait via /hrp."
  ]
}
```

Champs :
- `titre` (défaut : `Sans titre`) — titre du livre et du nom d'item
- `auteur` (défaut : `Serveur`) — auteur affiché dans le livre
- `pages` — liste de chaînes (ou une chaîne unique pour une seule page) ; codes couleur legacy supportés (`&e`, `&l`, `&o`...), `
` pour un saut de ligne. Un livre Minecraft est limité à 100 pages / 255 caractères par page en jeu.

Les livres sont chargés au démarrage du plugin. L'`id` du livre est le nom du fichier sans extension.

## Configuration des fonctionnalités

Chaque fonctionnalité du plugin (pas le plugin lui-même) peut être activée ou désactivée via `plugins/data/rpumpkin/config.json`, créé automatiquement avec les valeurs par défaut au premier démarrage :

```json
{
  "fonctionnalites": {
    "fiches": true,
    "chat_rp": true,
    "commandes_rp": true,
    "metier": true,
    "livres": true
  }
}
```

| Clé | Fonctionnalité contrôlée |
|-----|--------------------------|
| `fiches` | Fiches de personnage (`/fiche`, persistance `sheets.json`) |
| `chat_rp` | Formatage RP du chat (handler d'événement) |
| `commandes_rp` | Commandes `/roll`, `/me`, `/do`, `/hrp`, `/rp` |
| `metier` | Commande `/metier` et tenues |
| `livres` | Commande `/livre` et livres serveur |

Une fonctionnalité désactivée n'enregistre ni ses commandes, ni ses permissions, ni ses handlers — elle est totalement absente du serveur jusqu'à réactivation (les données persistées comme `sheets.json` sont conservées). La configuration est lue au démarrage du plugin.

## Permissions

| Nœud | Description | Défaut |
|------|-------------|--------|
| `rpumpkin.command.fiche` | Gérer sa fiche de personnage | Autorisé |
| `rpumpkin.command.rp` | Commandes RolePlay (roll, me, do, hrp) | Autorisé |
| `rpumpkin.command.metier` | Choisir sa tenue de métier | Autorisé |
| `rpumpkin.command.livre` | Recevoir un livre serveur | Autorisé |

Le plugin demande aussi les permissions d'accès à son dossier de données (`fs.read.data`, `fs.write.data`).

## Compatibilité plateforme

Le plugin est compilé en WebAssembly (`wasm32-wasip2`) : un unique `rpumpkin.wasm` fonctionne sur toutes les architectures supportées par le serveur Pumpkin — x86_64, ARM64 (Raspberry Pi 5, Apple Silicon). Aucun build spécifique n'est nécessaire pour le Pi 5 ; seule le binaire du serveur Pumpkin doit être la version `aarch64`.

## CI / Release GitHub

- `.github/workflows/ci.yml` — à chaque push/PR : `cargo fmt --check`, `cargo clippy -D warnings`, build WASM.
- `.github/workflows/release.yml` — sur un tag `vX.Y.Z` : build, création d'une release GitHub avec `rpumpkin.wasm` en pièce jointe, puis publication sur le Pumpkin Market.

Pour publier une nouvelle version :

```bash
git tag v0.1.1
git push origin v0.1.1
```

### Publication sur le Pumpkin Market

La publication Market est faite par l'action `publish-to-pumpkin-market` (depuis [filiphsps/pumpkin-plugins](https://github.com/filiphsps/pumpkin-plugins)). Prérequis :

1. Créer une fiche plugin « rpumpkin » sur [market.pumpkinmc.org](https://market.pumpkinmc.org) (première fois seulement).
2. Générer un token d'API Market avec les scopes `plugins:update` et `plugins:versions:upload`.
3. L'ajouter comme secret `PUMPKIN_MARKET_TOKEN` dans `Settings > Secrets and variables > Actions`.

À chaque tag poussé, la version est alors publiée automatiquement sur le Market (track `stable`), en plus de la release GitHub.

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
    ├── books.rs         # module livres configurables (encodage written_book_content)
    ├── config.rs        # module configuration des fonctionnalités
    └── skins/           # module tenues de métier (composite tête/corps)
```

## Roadmap

- Canaux de chat par proximité (radius RP)
- Système d'inventaire RP (items fictifs)
- Intégration de teams/factions RP
- Événements scriptés (PNJ, quêtes)

## Licence

MIT OR Apache-2.0
