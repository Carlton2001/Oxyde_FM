# Accès rapide et ouverture

**Statut : domaine terminé** (quatrième domaine de l'option B, après les
[volumes](volumes.md), le [presse-papiers](clipboard.md) et la [surveillance](watchers.md)) ;
bilan ci-dessous, suite Windows encore à passer. Ce document est le contrat du domaine.

## Périmètre

Les dossiers épinglés de la section « Accès rapide », épingler et désépingler, leur
rafraîchissement ; sous Linux, le dossier personnel parmi les emplacements, l'ouverture
d'un élément par l'application du bureau et le bouton système des Propriétés (point T12 de
l'audit). OneDrive, le réseau et la corbeille restent hors domaine.

## Constat Linux

- Les favoris venaient de `~/.config/gtk-3.0/bookmarks`, sans suivre `XDG_CONFIG_HOME` :
  le profil de test isolé lisait les vrais favoris. Épingler et désépingler échouaient.
- Sans fichier de favoris, un « Dossier personnel » écrit en dur en français remplaçait
  la liste ; avec un fichier, le dossier personnel n'apparaissait nulle part.
- `xdg-open` était lancé sans attendre : ses échecs se perdaient et chaque ouverture
  laissait un processus zombie jusqu'à la fermeture d'Oxyde.
- « Propriétés système » exécutait `gio open`, qui ouvre l'élément.

## Interface

`Platform` expose `fn quick_access(&self) -> &dyn QuickAccess`, défini dans
`src/platform/quick_access.rs` :

```rust
pub trait QuickAccess: Send + Sync {
    fn items(&self) -> Vec<QuickAccessItem>;
    fn pin(&self, path: &str) -> Result<(), CommandError>;
    fn unpin(&self, path: &str) -> Result<(), CommandError>;
    fn watch(&self, revision: Arc<AtomicU64>) -> Result<Option<Box<dyn Send>>, String>;
}
```

- **Windows** (`windows_quick_access.rs`) délègue à `windows_shell` (verbes `pintohome`,
  `unpinfromhome`) et surveille `Recent`, sans changement.
- **Linux** (`linux_quick_access/`) lit et réécrit `$XDG_CONFIG_HOME/gtk-3.0/bookmarks`,
  le fichier que Nautilus partage : dossiers locaux seulement, ordre et noms conservés,
  lignes non affichées (emplacements réseau) préservées, réécriture atomique comme GTK.
  Il surveille le dossier `gtk-3.0`, que GTK remplace à chaque changement.

## Interface utilisateur, hors Windows

- Le dossier personnel ouvre la section « Accès rapide », au-dessus des favoris, comme
  dans Nautilus, avec l'icône Lucide `house` ; il n'est pas désépinglable. Un élément sans nom
  le désigne ; l'interface le nomme dans la langue choisie (`sidebar-home`).
- Ouvrir attend le verdict de `xdg-open` (3 s au plus) : un échec s'affiche comme sous
  Windows ; au-delà, le processus est récupéré en arrière-plan.
- `show_properties_dialog` ne fait rien : la fenêtre Propriétés d'Oxyde sert Linux.

## Étapes

1. *Fait.* **Extraire le trait**, déplacements seuls : quatre méthodes de `Platform`,
   deux fichiers communs (`commands/system.rs`, `ui/app/shell_features.rs`).
2. *Fait.* **Favoris GTK** : lecture, épingler, désépingler, surveillance ; tests sur
   fichiers temporaires.
3. *Fait.* **Dossier personnel, ouverture, Propriétés système.**
4. *Fait.* **Bilan**, ci-dessous.

## Bilan

**Résultat.** Sous Linux, l'accès rapide montre le dossier personnel puis les favoris de
Nautilus ; épingler, désépingler et les changements faits ailleurs fonctionnent. Ouvrir un
fichier sans application signale l'échec. Le code Linux tient en 150 lignes de production
et 50 de tests.

**Fichiers communs touchés** (hors `linux_quick_access/`, `windows_quick_access.rs`,
catalogues et documentation) :

| Étape | Fichiers communs | Nature |
| --- | --- | --- |
| 1. Trait | `platform/mod.rs`, `windows_impl.rs`, `linux_impl.rs`, `commands/system.rs`, `ui/app/shell_features.rs` | appels seuls, comme prévu |
| 3. Dossier personnel | `workspace/sidebar.rs`, `sidebar.rs`, `sidebar/rows.rs`, `sidebar/rail.rs`, `i18n.rs`, icône `house` | interface, Linux seul |
| 3. Ouverture | `linux_impl.rs` | Linux seul |

**Leçons.**
- Respecter `XDG_CONFIG_HOME` dès qu'un domaine écrit un fichier du bureau : le profil de
  test isolé protège alors les vraies données sans autre précaution.
- GNOME identifie un fichier par son contenu : un essai d'échec d'ouverture demande un
  fichier binaire, pas seulement une extension inconnue.

**Reporté : domaine Réseau (T10).** Nautilus passe par GVfs pour le voisinage réseau, les
adresses `smb://` et `sftp://` et leurs montages dans `/run/user/<uid>/gvfs`. Oxyde, fondé
sur des chemins, devra découvrir, se connecter (mot de passe), monter, puis naviguer ces
montages : un domaine à part, décidé pour plus tard.
