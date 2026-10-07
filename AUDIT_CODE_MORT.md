# Audit du code mort — Oxyde GPUI

Date : 7 octobre 2026. Base : commit `88634a2` (`Add Peek integration, shortcut editing and quick access synchronization`).

## Périmètre et méthode

Audit statique des sources Rust, des déclarations de modules, des appels depuis GPUI et des chemins de remplacement. Les conclusions ont été revérifiées après l’implémentation de Peek, de l’éditeur de raccourcis et du watcher d’accès rapide.

Ce document exclut ces trois fonctionnalités désormais raccordées, ainsi que les corrections de présentation et de saisie réalisées pendant leur implémentation. Il ne propose pas leur suppression. `Tooltip::text`, utilisé dans les propriétés, est également retiré de la liste précédente. L’ancienne méthode `reload_quick_access_and_refresh` a déjà été supprimée au profit du chargement asynchrone.

Aucun nettoyage de code n’a été effectué pour produire l’audit initial. Les références de lignes des sections 1 à 6 décrivent cette base ; le suivi du nettoyage et les corrections issues de la revérification figurent en section 7.

### Niveau de preuve

- **Sans appelant identifié** : aucune utilisation trouvée dans les sources actuelles. Cela ne prouve pas qu’un consommateur externe de la bibliothèque n’utilise pas une API publique.
- **Îlot déconnecté** : plusieurs fonctions s’appellent entre elles, mais aucune entrée du parcours n’est raccordée à l’application.
- **Tests seulement** : fonction encore utilisée pour préparer ou vérifier des fixtures, sans chemin d’appel de production identifié.
- **À confirmer fonctionnellement** : code non raccordé pouvant représenter une capacité non portée ; ne pas le supprimer avant de décider du comportement attendu.

Les recherches de symboles servent à établir des pistes ; la qualification tient compte des appels indirects, des méthodes homonymes, des tests et de `cfg`. La simple présence de `pub` ou de `allow(dead_code)` ne constitue pas une preuve. Linux n’a pas été compilé dans cet audit.

## 1. Vestiges structurels de la migration Tauri

### 1.1 Anciennes commandes de session — îlot déconnecté

Référence : `src/commands/session.rs:15`.

Le module expose `get_session_state`, `create_tab`, `close_tab`, `switch_tab`, `active_tab_navigate`, `set_active_panel`, `duplicate_tab`, `close_other_tabs`, `reorder_tabs`, `update_sort_config` et `swap_panels`.

Aucun appel depuis GPUI à ce module n’a été identifié. Ces commandes manipulent l’ancien `SessionManager`, avec deux panneaux fixes `left` et `right`.

**Fonctionnalités associées :** création, fermeture, navigation, duplication et réorganisation des onglets ; sélection et échange des panneaux. Les opérations courantes sur les onglets et panneaux sont reprises par `src/ui/workspace.rs` et `src/ui/app/navigation.rs`. L’échange spécifique des chemins des deux panneaux, réalisé par `swap_panels`, n’a pas de raccordement identifié.

**Orientation :** candidat à la suppression après vérification des consommateurs de la bibliothèque. Ne pas confondre ces fonctions avec les méthodes GPUI homonymes encore actives.

### 1.2 Ancien modèle de session et persistance — îlot déconnecté

Références : `src/models/session.rs:49`, `src/models/session.rs:219`.

`Tab`, `SearchContext`, `CachedResults`, `PanelState`, `SessionState` et `SessionManager` restent associés aux anciennes commandes. La sauvegarde et le chargement de `session.json`, ainsi que les watchers des deux panneaux, ne sont pas utilisés par le parcours GPUI actuel. Le callback de ces anciens watchers ne fait que journaliser les changements.

**Remplacement actif :** `TabState`, `PaneState`, `WorkspaceState`, `SavedWorkspace`, la persistance du workspace dans la configuration et les watchers de `src/ui/app/drive_watchers.rs`.

**Exception essentielle :** `SortConfig`, `SortField` et `SortDirection`, également définis dans ce module, restent utilisés par `commands/io.rs` pour le listing et le tri. Le fichier entier ne peut donc pas être supprimé tel quel.

### 1.3 Ancienne recherche à état de session — îlot déconnecté

Références : `src/commands/search.rs:942`, `src/commands/search.rs:1289`.

`start_search` et `cancel_search` n’ont aucun appelant identifié. Leur état et leur annulation reposent sur `SessionManager` et `SearchContext`.

**Fonctionnalité associée :** recherche avec options et annulation. Elle est implémentée dans GPUI avec `quick_search`, `advanced_search`, leurs événements et les tâches de `src/ui/app/search.rs`.

**Orientation :** retirer l’ancien parcours, en conservant le moteur et les helpers partagés avec les nouvelles entrées. Ne pas supprimer le module de recherche entier.

### 1.4 Anciennes commandes de configuration — îlot déconnecté

Référence : `src/commands/config.rs:3`.

`get_config`, `set_config_value`, `set_trash_settings` et `reset_config_to_default` sont sans appelant identifié.

**Remplacement actif :** `src/ui/app/preferences.rs` pour les préférences et `src/ui/app/trash_settings.rs` pour les réglages de corbeille.

La façade avec état sous mutex de `ConfigManager` reste principalement liée à ces commandes et à l’ancienne recherche. Ses fonctions statiques `config_path`, `read_from_path` et `write_to_path` restent actives et doivent être conservées.

### 1.5 Fichier Shell non compilé — vestige établi

Référence : `src/commands/system_verb.rs:2`.

Aucune déclaration `mod system_verb` ni inclusion de ce fichier n’a été trouvée. Sa fonction `execute_shell_verb_by_canonical_name` conserve un paramètre `AppHandle` hérité de Tauri.

**Fonctionnalité associée :** épinglage/désépinglage de l’accès rapide. Une autre implémentation est utilisée dans `src/commands/system.rs`.

**Orientation :** candidat direct à la suppression. Son absence du graphe de compilation explique pourquoi son ancien contexte Tauri n’entraîne pas d’erreur Rust.

## 2. Façades inutilisées de fonctionnalités toujours présentes

| Référence | Code concerné | Remplacement ou précaution |
|---|---|---|
| `src/commands/ops.rs:94` | `cancel_file_operation`, `pause_file_operation`, `resume_file_operation`, `toggle_turbo`, `get_op_status` | L’UI appelle directement `FileOperationManager` : annulation, pause, Turbo et consultation des opérations restent actifs. |
| `src/commands/ops.rs:904` | `delete_items`, `copy_items`, `move_items` ; `purge_items` appelé par l’ancien parcours `delete_items` | GPUI construit les opérations et utilise `queue_operation_on`. `move_items` doit être distingué de `fs_extra::move_items`, encore utilisé. |
| `src/commands/ops.rs:1364` | `get_history` | L’historique est consulté directement ; annuler/refaire restent implémentés. |
| `src/commands/ops.rs:750` | `get_recycle_bin_config`, ainsi que l’ancienne façade `set_recycle_bin_config` | Les réglages actuels passent directement par `Platform::recycle_bin_config` et `Platform::set_recycle_bin_config`. Conserver ces méthodes et `TrashSystemConfig`. |
| `src/commands/archive.rs:433` | `extract_archive`, `cancel_archive_operation`, `add_to_archive` | Aucun appelant identifié. Le moteur d’opérations et l’espace de travail d’archives assurent les parcours actuels. `extract_archive_with_checkpoint` reste actif. |
| `src/commands/archive.rs:772` | `compress_to_archive` et son parcours de compression | Des tests l’utilisent encore ; aucun appel depuis l’UI identifié. Vérifier les helpers partagés avant nettoyage. |
| `src/commands/archive.rs:117` | `create_archive_directory` | Wrapper utilisé par des tests ; l’UI appelle `create_archive_directory_with_quality`. Conserver l’entrée avec qualité. |
| `src/commands/system.rs:400` | `get_mounted_images`, wrappers `mount_disk_image` et `unmount_disk_image` | `src/ui/app/disk_images.rs` utilise directement `Platform`. L’ancien wrapper de démontage manipule encore `SessionManager`. |
| `src/commands/thumbnails.rs:5` | `get_image_thumbnail`, `get_office_thumbnail`, `get_pdf_thumbnail`, `get_office_text_preview` | Les listes et infobulles appellent directement `Platform`. Leurs relais correspondants dans `utils::thumbnails` sont également déconnectés de la production. **`stream_video_frames` reste utilisé.** |
| `src/commands/io.rs:37` | `get_file_entry_from_metadata` | Aucun appelant identifié. Ne pas confondre avec les fonctions de construction d’entrées encore utilisées par les moteurs. |
| `src/commands/io.rs:752` | `get_files_summary` | Aucun appelant identifié ; les résumés et compteurs existent dans les parcours actuels. `FileSummary` reste utilisé. |
| `src/commands/io.rs:808` | `show_system_properties` | Les propriétés GPUI utilisent directement `Platform::show_properties_dialog`. Le bouton Propriétés système reste fonctionnel. |
| `src/models/history.rs:66` | `pop_undo`, `pop_redo`, `push_redo`, `push_undo_raw` | Sans appelant identifié. Les parcours actuels utilisent `last_undo`/`last_redo`, puis `complete_undo`/`complete_redo`. |
| `src/platform/clipboard.rs:218` | `get_clipboard_text` | Aucun appelant identifié ; les champs et commandes de texte utilisent le presse-papiers GPUI. |
| `src/commands/system.rs:904` | `restart_app` | Aucun appelant identifié. Les changements de préférences actuels s’appliquent sans redémarrage. |

Ces lignes concernent des points d’entrée ou des chemins précis, pas la suppression globale des modules `ops`, `archive`, `system`, `io`, `history` ou des implémentations natives.

## 3. Capacités non raccordées ou compatibilité à décider

### 3.1 Migration des anciennes données

Référence : `src/systems/migration.rs:3`, `run_migration`.

La fonction n’est pas appelée au démarrage. Elle cherche les anciens répertoires `com.oxyde.app` pour déplacer leur contenu vers `oxyde`.

**Fonctionnalité potentiellement manquante :** reprise des données de l’ancienne application lors du passage à GPUI.

**Décision nécessaire :** conserver une migration, l’abandonner explicitement ou distinguer les données encore compatibles. La fonction actuelle remplace certains fichiers existants et ignore des erreurs de déplacement avant de nettoyer l’ancien répertoire : la réactiver telle quelle demande une revue de sûreté.

### 3.2 Ancienne géométrie du bouton maximiser

Références : `src/lib.rs:14`, `src/models/file_entry.rs:183`, `src/commands/system.rs:481`.

`WindowState`, `SnapRect` et `oxide_sync_snap_rect` constituent un ensemble déconnecté. Aucun lecteur du rectangle stocké n’a été identifié.

**Fonctionnalité probable :** transmission de la position du bouton maximiser pour l’intégration Windows des Snap Layouts dans l’ancienne interface. Ce lien est une inférence fondée sur les noms et les données ; le comportement natif GPUI n’a pas été testé ici.

**Orientation :** supprimer l’ancien pont si le comportement souhaité est assuré par GPUI ; sinon concevoir l’intégration via `Platform`.

### 3.3 Couleur d’accent du système

Références : `src/commands/system.rs:14`, `Platform::get_accent_color` et ses implémentations Windows/Linux.

La commande n’a pas de consommateur actif identifié ; les implémentations natives ne sont atteintes que par cette façade.

**Fonctionnalité potentiellement absente :** reprendre automatiquement la couleur d’accent du système. Les thèmes GPUI actuels utilisent leurs propres palettes.

### 3.4 Presse-papiers spécifique à la corbeille

Référence : `src/platform/clipboard.rs:303`, `set_clipboard_from_trash`.

Aucun appelant identifié. GPUI utilise le transfert générique des chemins par `Platform::set_clipboard_files`.

**Fonctionnalité potentiellement non portée :** transfert Shell spécifique des objets de la corbeille. Cela ne prouve pas que toute coupe depuis la corbeille échoue : les différences de nom d’origine, de format du presse-papiers et de destination doivent être vérifiées avant suppression.

## 4. Éléments UI et champs hérités

### Helpers UI sans usage identifié

- `src/ui/elements/icon.rs:4` : `Icon` et `IconSize`, avec leurs builders ; aucun usage du composant identifié. Les SVG et icônes rendus ailleurs restent actifs.
- `src/ui/style.rs:162` : `v_flex`.
- `src/ui/workspace.rs:579` : `LayoutNode::is_pane`.
- `src/ui/workspace.rs:584` : `LayoutNode::as_pane_mut`.
- `src/ui/workspace.rs:879` : accesseur `WorkspaceState::active_pane_id` ; le champ du même nom est utilisé directement.
- **Correction après revérification :** `src/ui/workspace.rs:903`, `WorkspaceState::active_tab_mut`, est utilisée par `src/ui/app/sizes.rs` pour le calcul des tailles. La conserver, ainsi que `PaneState::active_tab_mut`.
- **Correction après revérification :** `src/ui/workspace.rs:1296`, `_collapse_all_sidebar_path`, est appelée par `src/ui/components/sidebar.rs` pour replier une branche. La conserver.
- `src/ui/components/tabs/tab_bar.rs:72` et `:78` : `start_child` et `end_child`.

Les annotations `allow(dead_code)` peuvent être nettoyées séparément lorsqu’elles masquent des méthodes désormais actives. Par exemple, `LayoutNode::visit_panes_mut` a des usages et ne doit pas être supprimé sur la seule présence de cette annotation. Le composant `Button` est également utilisé.

### Champs de configuration sans lecture fonctionnelle GPUI identifiée

Référence : `src/models/config.rs`.

| Champ | Situation actuelle | Lien fonctionnel |
|---|---|---|
| `font_size` | Conservé dans la configuration ; ancienne commande capable de l’écrire | L’échelle actuelle est pilotée par `ui_rem_size`. |
| `layout` | Ancienne valeur globale, distincte de la disposition du workspace | La disposition active est restaurée via `SavedWorkspace`. |
| `group_by_date` | Ancienne préférence globale | Le regroupement actuel est propre à chaque `TabState` et persisté par onglet. |
| `first_launch` | Pas de lecture fonctionnelle GPUI identifiée | Probable ancien parcours de premier démarrage ; son contenu exact n’est pas établi par cet audit. |

Ces champs restent lus et écrits par sérialisation. Ils sont **inactifs fonctionnellement**, et non nécessairement inutiles pour la compatibilité. Leur suppression demande de décider du traitement des anciens fichiers et de la conservation des options inconnues.

## 5. Dépendances candidates au nettoyage

Référence : `Cargo.toml:34` et suivantes.

Aucune utilisation directe de `hex`, `base64` ou de la crate externe `time` n’a été trouvée dans les sources. Les usages de `std::time` ne justifient pas la dépendance `time`.

Ce sont des candidats à retirer du manifeste, à confirmer par compilation après retrait. Une dépendance peut rester présente dans `Cargo.lock` parce qu’une autre crate l’utilise transitivement. Cet audit ne conclut pas que toutes les features Windows du manifeste sont inutiles.

## 6. Ordre de nettoyage proposé

1. Supprimer le fichier non compilé `system_verb.rs` et les petits helpers sans appelant, après contrôle des éventuels consommateurs externes.
2. Séparer les types de tri encore actifs du modèle de session historique, puis retirer ensemble les anciennes commandes de session et la recherche fondée sur `SessionManager`.
3. Retirer les façades de commandes remplacées par les appels GPUI/Platform et les parties de `ConfigManager` devenues exclusivement liées aux anciens parcours.
4. Décider du sort de la migration des données, des Snap Layouts, de l’accent système et du presse-papiers de corbeille avant de supprimer leurs capacités backend.
5. Examiner les chemins d’archives utilisés uniquement par les tests : adapter les fixtures vers les parcours de production avant suppression, sans perdre de couverture utile.
6. Nettoyer les dépendances directes et les annotations devenues superflues.

Pour chaque lot : vérifier `cargo check --all-targets` sans avertissement du projet, exécuter les tests appropriés et préserver les implémentations conditionnelles Windows/Linux. La validation précédente du commit a réussi avec 91 tests backend, 86 tests du binaire et un test de montage Windows ignoré ; cette rédaction documentaire n’a pas relancé ces suites.

## 7. Suivi du nettoyage — 7 octobre 2026

Branche : `codex/nettoyage-code-mort`. L’utilisateur a confirmé que la bibliothèque Rust sert uniquement à cette application : aucune compatibilité d’API avec un projet tiers n’est requise.

### Lot 1 — anciens parcours et façades déconnectées

Les références ont été recherchées dans les sources Rust du dépôt, tests inclus, en distinguant les méthodes homonymes et les implémentations conditionnelles. Les points d’entrée GPUI et leurs remplacements ont été relus avant suppression.

- Suppression du fichier non compilé `commands/system_verb.rs` ; l’implémentation active de l’accès rapide dans `commands/system.rs` est conservée.
- Suppression de `commands/session.rs`, de l’ancien modèle de session et du parcours `start_search` / `cancel_search`. Les types `SortConfig`, `SortField` et `SortDirection` sont déplacés sans changement dans `models/sort.rs`, et leurs usages dans `commands/io.rs` sont adaptés. Les moteurs `quick_search` / `advanced_search`, leurs helpers et leurs tests sont conservés.
- Suppression de `commands/config.rs` et de l’état sous mutex de `ConfigManager`. Ses trois fonctions actives `config_path`, `read_from_path` et `write_to_path` sont conservées, ainsi que tous les champs de `AppConfig` et la préservation des champs inconnus.
- Suppression des façades d’opérations identifiées en section 2 : contrôle des opérations, `delete_items` / `purge_items` / `copy_items` / `move_items`, configuration de corbeille et `get_history`. Les estimations locales de capacité/suppression de corbeille, exclusivement liées à `delete_items`, sont retirées avec ce parcours. Les appels GPUI au gestionnaire d’opérations, les règles natives de corbeille de `Platform`, la restauration et Annuler/Rétablir sont conservés.
- Suppression des wrappers d’images disque, de `restart_app`, des façades de miniatures inutilisées et de leurs relais dans `utils::thumbnails`. Les API de `Platform` et le streaming vidéo utilisé par les infobulles sont conservés. La fermeture finale du processus reste inchangée.
- Suppression des trois helpers IO, des quatre méthodes d’historique et de `get_clipboard_text` listés en section 2.
- Suppression de l’élément `Icon`, de `v_flex`, de `LayoutNode::is_pane` / `as_pane_mut`, de l’accesseur `WorkspaceState::active_pane_id`, et des builders `TabBar::start_child` / `end_child` avec leurs emplacements toujours vides. Les méthodes UI effectivement utilisées restent présentes ; les annotations `allow(dead_code)` superflues sont retirées de `active_tab_mut` et `visit_panes_mut`.
- Retrait des dépendances directes `hex`, `base64` et `time` après recherche d’usages et compilation ; le verrouillage des dépendances est actualisé par Cargo. Leurs usages transitifs éventuels restent dans `Cargo.lock`.

### Hors de ce lot

Les chemins d’archives nécessitant une adaptation des fixtures et une revue de leur couverture restent à nettoyer séparément. La migration des données, le pont Snap Layouts, l’accent système et le presse-papiers spécifique à la corbeille restent en attente de décisions fonctionnelles. Les champs de configuration historiques sont conservés.

### Validation du lot

- `cargo check --all-targets --offline` : réussi sous Windows, sans avertissement du projet.
- `cargo test --all-targets --offline` : 91 tests backend et 89 tests du binaire réussis ; aucun échec, un test de montage Windows ignoré. Aucun test existant n’a été supprimé ni modifié dans ce lot.
- Linux et le comportement visuel de l’interface n’ont pas été validés dans ce lot.
