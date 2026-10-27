# Presse-papiers de fichiers

**Statut : domaine en cours** (deuxième domaine de l'option B, après les
[volumes](volumes.md)). Ce document deviendra le contrat du domaine une fois terminé.

## Périmètre

Copier, couper et coller une sélection de fichiers (Ctrl+C/X/V, bouton Coller), et l'état
du bouton. Le texte copié (« Copier le chemin », noms) reste à GPUI et n'entre pas dans le
domaine. Sous Linux aujourd'hui, Ctrl+C/X/V renvoient une erreur (M08).

## Contrainte Linux

Sous Wayland, seule l'application active lit ou remplit le presse-papiers, par sa propre
connexion : pour Oxyde, celle de GPUI. GPUI Linux n'y échange que du texte et des images
(`gpui_linux/src/linux/wayland/clipboard.rs`) ; les listes de fichiers ne passent que par le
glisser-déposer. GNOME 50 n'offre pas de protocole de presse-papiers en arrière-plan
(`ext-data-control`). Nautilus échange ses fichiers en `x-special/gnome-copied-files`.

Zed (`project_panel`) fait de même sur toutes les plateformes : presse-papiers interne qui
distingue copier et couper, chemins en texte dans le presse-papiers système ; au collage,
d'abord les fichiers que GPUI lit d'une autre application (`ClipboardEntry::ExternalPaths`,
jamais rempli sous Linux), puis le presse-papiers interne.

## Interface

`Platform` expose `fn file_clipboard(&self) -> &dyn FileClipboard`, défini dans
`src/platform/file_clipboard.rs` :

```rust
pub trait FileClipboard: Send + Sync {
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<(), String>;
    fn get_files(&self) -> Result<(Vec<String>, bool), String>;
    fn has_file_format(&self) -> bool;
    fn discard(&self) {} // le presse-papiers système a remplacé les fichiers
}
```

- **Windows** (`windows_file_clipboard.rs`) délègue à `clipboard.rs` (`CF_HDROP`,
  `Preferred DropEffect`), sans changement.
- **Linux** (`linux_file_clipboard.rs`) garde la sélection en mémoire, pour toutes les
  fenêtres du processus. Une sélection coupée ne se colle qu'une fois, comme dans les
  gestionnaires de fichiers : ses sources ont déménagé.

## Interface utilisateur, hors Windows

- Copier ou couper écrit aussi les chemins, un par ligne, en texte par GPUI : un terminal
  ou un éditeur les reçoit.
- Coller prend d'abord les fichiers que GPUI fournit d'une autre application (copie), puis
  la sélection d'Oxyde, seulement si le texte du presse-papiers système est encore celui
  qu'Oxyde y a écrit : un texte copié ailleurs depuis remplace les fichiers, comme dans
  tout presse-papiers, et `discard` les oublie.
- Coller dans Nautilus des fichiers copiés dans Oxyde donne leurs chemins en texte, et
  l'inverse ne colle rien : limite de GPUI, comme dans Zed.
- Après un texte copié ailleurs, le bouton Coller reste actif jusqu'au premier collage :
  le griser plus tôt demanderait de lire le presse-papiers Wayland à chaque sondage.

## Étapes

1. *Fait.* **Extraire le trait**, déplacements seuls : trois méthodes de `Platform`, deux fichiers
   communs (`ui/app/clipboard.rs`, `drive_watchers.rs`).
2. *Fait.* **Presse-papiers Linux** et règles de collage ci-dessus.
3. *Essai à part* : échanges avec Nautilus par `wl-copy`/`wl-paste` quand ils sont
   installés. Abandonné si le focus d'Oxyde vacille.
4. **Bilan.**
