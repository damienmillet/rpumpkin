# rpumpkin — tenues de métier avec tête conservée

Extension [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) : le joueur choisit son métier
dans le lobby, son skin de corps change pour la tenue du métier, **sa tête reste la sienne**.

## Principe

Le skin est une texture unique 64×64 (tête + corps). Impossible de changer "juste les
vêtements" côté client. L'extension fabrique donc un **composite serveur** :

1. `get_skin` → télécharge le PNG du joueur ;
2. copie la zone **tête** du joueur (base 32×16 en (0,0) + overlay chapeau en (32,0))
   dans le PNG de la tenue du métier (dont la zone tête est transparente) ;
3. upload le composite sur **mineskin.org** → renvoie `value`/`signature`
   (URL servie par `textures.minecraft.net`, acceptée par la validation de domaines de Pumpkin) ;
4. `set_skin` applique le résultat.

## Contrainte de propagation

Le GameProfile (donc le skin) est envoyé aux autres clients **uniquement à leur jointure**.
Ce plugin cible donc le cas : **choix du métier dans le lobby, avant l'entrée en jeu**.
Un changement en pleine session n'est visible que par les joueurs qui se connectent après.

## Contenu

- `plugin/` — extension Rust (WASM, cible `wasm32-wasip2`), scaffolding à ajuster
  aux signatures exactes de `pumpkin-plugin-api` pour ta version de Pumpkin ;
- `skins/gen_skins.py` — générateur de tenues (stdlib Python uniquement) ;
- `skins/metier_fermier.png.b64`, `skins/metier_mineur.png.b64` — tenues de test
  64×64 (zone tête transparente), encodées en base64.

## Tenues de test

Les tenues ont la tête **transparente** : après composite, la tête affichée est celle du
joueur. Deux métiers : `fermier` (salopette denim + chemise) et `mineur` (gilet orange
de sécurité + bandes réfléchissantes).

Pour obtenir les PNG :

```bash
# soit via le générateur (recommandé, reproductible)
python3 skins/gen_skins.py

# soit en décodant les .b64
base64 -d skins/metier_fermier.png.b64 > skins/metier_fermier.png
base64 -d skins/metier_mineur.png.b64 > skins/metier_mineur.png
```

## Build du plugin

```bash
rustup target add wasm32-wasip2
cd plugin && cargo build --target wasm32-wasip2 --release
# placer le .wasm dans le dossier plugins du serveur Pumpkin
```
