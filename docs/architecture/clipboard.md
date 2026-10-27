# Presse-papiers de fichiers

**Statut : domaine terminé** (deuxième domaine de l'option B, après les
[volumes](volumes.md)) ; bilan ci-dessous, suite Windows passée. Ce document est le
contrat du domaine.

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
4. *Fait.* **Bilan**, ci-dessous.

## Bilan

**Résultat.** Ctrl+C/X/V et le bouton Coller fonctionnent sous Linux entre fenêtres
d'Oxyde et, avec `wl-clipboard`, avec Nautilus dans les deux sens, copie et coupe. Le code
Linux tient en 160 lignes de production et 30 de tests ; Windows délègue à son code
inchangé et sa suite de tests passe.

**Fichiers communs touchés** (hors `linux_file_clipboard/`, `windows_file_clipboard.rs` et
documentation) :

| Étape | Fichiers communs | Nature |
| --- | --- | --- |
| 1. Trait | `platform/mod.rs`, `windows_impl.rs`, `linux_impl.rs`, `ui/app/clipboard.rs`, `drive_watchers.rs` | appels seuls, comme prévu |
| 2. Linux | `ui/app/clipboard.rs` | règles de collage prévues |
| 2 bis. Copie vers NTFS | `linux_impl.rs`, `systems/file_copy.rs` | publication Linux, hors domaine |
| 3. Nautilus | `ui/app/clipboard.rs`, signature du trait | texte écrit par l'interface seulement sans `wl-copy` |
| 3 bis. Sources NTFS | `models/transaction.rs` | identité sans date de naissance, hors domaine |

**Hors domaine, révélé par les essais sur NTFS (ntfs-3g).** Le domaine a servi de premier
essai réel du moteur de copie Linux ; deux défauts de l'audit de portabilité sont corrigés :
- **M04, en partie** : la publication exclusive essaie `renameat2(RENAME_NOREPLACE)`, qui
  marche aussi sur FAT et exFAT ; ntfs-3g refuse ce drapeau et garde le lien physique, dont
  le nom caché `.fuse_hidden` est désormais attendu au nettoyage.
- **T04** : hors Windows, l'identité d'un fichier n'exige plus de date de naissance, que
  ntfs-3g ne fournit pas ; l'identifiant seul fait foi, comme pour la copie temporaire.
  Un fichier supprimé puis remplacé par un autre qui reprend son numéro n'est plus détecté
  sur ces systèmes de fichiers.

**Leçons.**
- Les limites de GPUI se contournent hors de GPUI sans le modifier : un processus auxiliaire
  (`wl-copy`) porte ce que la boucle Wayland de GPUI ne sait pas offrir.
- Un trait de domaine peut renvoyer une consigne à l'interface (`set_files` → écrire le
  texte) plutôt que de multiplier les `cfg` dans l'appelant.
- Tester sur un volume NTFS, pas seulement btrfs : les deux défauts du moteur de copie
  n'apparaissaient qu'avec FUSE.

**Suite.** Le domaine suivant est la [surveillance des dossiers](watchers.md) (T11).
