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
    // Ok(true) : l'interface écrit aussi les chemins en texte.
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<bool, String>;
    // `text` : texte du presse-papiers système, qui périme une sélection offerte en texte.
    fn get_files(&self, text: Option<String>) -> Result<(Vec<String>, bool), String>;
    fn has_file_format(&self) -> bool;
}
```

- **Windows** (`windows_file_clipboard.rs`) délègue à `clipboard.rs` (`CF_HDROP`,
  `Preferred DropEffect`), sans changement.
- **Linux** (`linux_file_clipboard.rs`) garde la sélection en mémoire, pour toutes les
  fenêtres du processus. Une sélection coupée ne se colle qu'une fois, comme dans les
  gestionnaires de fichiers : ses sources ont déménagé. Quand `wl-copy` est installé, un
  processus `wl-copy --foreground` l'offre au format de Nautilus et se termine dès qu'une
  autre sélection la remplace : sa fin périme la sélection. Au collage, une sélection
  périmée laisse place à `wl-paste`, qui lit les fichiers copiés ou coupés ailleurs
  (`linux_file_clipboard/wayland.rs`).

## Interface utilisateur, hors Windows

- Sans `wl-copy`, copier ou couper écrit les chemins, un par ligne, en texte par GPUI ;
  avec lui, le presse-papiers ne contient que les fichiers, et « Copier le chemin » reste
  le moyen d'obtenir du texte.
- Coller prend d'abord les fichiers que GPUI fournit d'une autre application (copie), puis
  la sélection d'Oxyde tant que le presse-papiers système l'offre encore : sans `wl-copy`,
  tant que son texte est celui qu'Oxyde y a écrit. Sinon, les fichiers lus par `wl-paste`.
- Sans `wl-copy`, après un texte copié ailleurs, le bouton Coller reste actif jusqu'au
  premier collage ; avec lui, il se grise au sondage suivant. Il ne s'active pas pour des
  fichiers copiés ailleurs : les détecter demanderait `wl-paste` à chaque sondage, mais
  Ctrl+V les colle.

## Étapes

1. *Fait.* **Extraire le trait**, déplacements seuls : trois méthodes de `Platform`, deux fichiers
   communs (`ui/app/clipboard.rs`, `drive_watchers.rs`).
2. *Fait.* **Presse-papiers Linux** et règles de collage ci-dessus.
3. *Fait.* Échanges avec Nautilus par `wl-copy`/`wl-paste` quand ils sont installés,
   dans les deux sens, copie et coupe. Sous GNOME, faute de protocole de presse-papiers en
   arrière-plan, `wl-copy` prend le focus un instant : la fenêtre clignote à chaque copie
   ou coupe et les menus ouverts se ferment, sans perte de focus. Retenu : l'échange avec
   le gestionnaire de fichiers du bureau compte plus que ce clignotement, absent sous KDE
   et les compositeurs wlroots, où `wl-copy` passe par `ext-data-control`.
4. **Bilan.**
