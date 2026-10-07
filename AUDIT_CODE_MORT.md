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

**Décision prise le 7 octobre 2026 :** supprimer cette ancienne implémentation. L’utilisateur prévoit de concevoir une nouvelle migration plus tard ; voir le lot 3.

### 3.2 Ancienne géométrie du bouton maximiser

Références : `src/lib.rs:14`, `src/models/file_entry.rs:183`, `src/commands/system.rs:481`.

`WindowState`, `SnapRect` et `oxide_sync_snap_rect` constituent un ensemble déconnecté. Aucun lecteur du rectangle stocké n’a été identifié.

**Fonctionnalité probable :** transmission de la position du bouton maximiser pour l’intégration Windows des Snap Layouts dans l’ancienne interface. Ce lien est une inférence fondée sur les noms et les données ; le comportement natif GPUI n’a pas été testé ici.

**Orientation :** supprimer l’ancien pont si le comportement souhaité est assuré par GPUI ; sinon concevoir l’intégration via `Platform`.

**Revérification du lot 4 :** le bouton actuel déclare `WindowControlArea::Max`, et GPUI Windows traduit cette zone en `HTMAXBUTTON` pour une fenêtre redimensionnable. L’ancien pont déconnecté est supprimé ; le comportement interactif des Snap Layouts n’a pas été testé manuellement.

### 3.3 Couleur d’accent du système

Références : `src/commands/system.rs:14`, `Platform::get_accent_color` et ses implémentations Windows/Linux.

La commande n’a pas de consommateur actif identifié ; les implémentations natives ne sont atteintes que par cette façade.

**Fonctionnalité potentiellement absente :** reprendre automatiquement la couleur d’accent du système. Les thèmes GPUI actuels utilisent leurs propres palettes.

**Décision prise au lot 4 :** l’utilisateur demande la suppression de cet ancien support, y compris les méthodes de `Platform` et leurs implémentations Windows/Linux.

### 3.4 Presse-papiers spécifique à la corbeille

Référence : `src/platform/clipboard.rs:303`, `set_clipboard_from_trash`.

Aucun appelant identifié. GPUI utilise le transfert générique des chemins par `Platform::set_clipboard_files`.

**Fonctionnalité potentiellement non portée :** transfert Shell spécifique des objets de la corbeille. Cela ne prouve pas que toute coupe depuis la corbeille échoue : les différences de nom d’origine, de format du presse-papiers et de destination doivent être vérifiées avant suppression.

**Décision prise au lot 4 :** l’utilisateur demande la suppression de l’ancien transfert Shell déconnecté. Il demande également de vérifier, après ce lot, que le parcours GPUI actuel restaure les éléments avec leur nom d’origine.

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

**Traitement final au lot 4 :** les quatre champs quittent le modèle Rust. À la demande de l’utilisateur, la conservation des champs inconnus via `AppConfig::extra` est également supprimée. Les anciennes clés sont ignorées à la lecture et disparaissent à la prochaine sauvegarde ; les configurations neuves ne les ajoutent plus. Le workspace et les préférences de recherche de doublons disposent de champs explicites.

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

### Hors du lot 1

Les chemins d’archives nécessitant une adaptation des fixtures et une revue de leur couverture sont traités dans le lot 2 ci-dessous, et l’ancienne migration dans le lot 3. Le pont Snap Layouts, l’accent système et le presse-papiers spécifique à la corbeille restent en attente de décisions fonctionnelles. Les champs de configuration historiques sont conservés.

### Validation du lot 1

- `cargo check --all-targets --offline` : réussi sous Windows, sans avertissement du projet.
- `cargo test --all-targets --offline` : 91 tests backend et 89 tests du binaire réussis ; aucun échec, un test de montage Windows ignoré. Aucun test existant n’a été supprimé ni modifié dans ce lot.
- Linux et le comportement visuel de l’interface n’ont pas été validés dans ce lot.

### Lot 2 — archives et fixtures utilisant les anciens parcours

Base : commit `bb6f289`, sur la même branche. La recherche des références a été reprise dans le dépôt, tests inclus, et les entrées de compression/extraction ainsi que les mutations d’archives du gestionnaire ont été relues.

- Retrait de `extract_archive`, `cancel_archive_operation`, `add_to_archive`, `compress_to_archive`, de leur `ArchiveState` et des helpers de compression/ajout propres à ces parcours. `extract_archive_with_checkpoint` et les lecteurs partagés ZIP, 7z, TAR et ISO restent actifs.
- Les fixtures de listing et de renommage annulé dans `commands/io.rs`, ainsi que le test aller-retour 7z, produisent leurs archives avec `FileOperationManager::perform_compress`, le moteur utilisé par l’application.
- Retrait de `create_archive_directory` après adaptation du test d’opérations d’archives vers `create_archive_directory_with_quality`, déjà appelée par GPUI.
- **Candidat supplémentaire établi par revérification :** `remove_items_from_archive` / `remove_from_zip` n’avaient qu’un appelant, un test. Ils sont supprimés avec `TemporaryArchive`, désormais sans usage. Le test de suppression est déplacé vers `systems/file_ops.rs` et appelle `perform_delete`, qui utilise l’espace de travail d’archives actuel. Il vérifie ZIP, 7z, TAR et TAR.ZST, la suppression d’un fichier et d’une branche, la conservation d’une branche au préfixe voisin et de son contenu, ainsi que l’absence de fichiers temporaires résiduels.
- Le test de non-écrasement est également déplacé vers le moteur actif et étendu aux quatre formats. Il vérifie le comportement actuel : création d’une archive valide sous un nom numéroté et conservation des octets du fichier existant. L’erreur renvoyée par l’ancienne façade n’était pas le comportement GPUI.
- Retrait du wrapper d’extraction 7z limité aux tests ; les tests d’entrée malveillante et d’aller-retour passent désormais par `extract_archive_with_checkpoint`.
- La validation a révélé une hypothèse incorrecte dans le test existant du watcher d’accès rapide, dont le code de production est inchangé. L’échec s’est reproduit isolément. Dans `notify` 8.2.0, `ReadDirectoryChangesWatcher::drop` envoie une demande d’arrêt sans attendre son traitement. Le test attend désormais, pendant cinq secondes au maximum, que le callback relâche sa référence à la révision ; il vérifie ensuite qu’une nouvelle écriture ne produit plus de notification. Aucun délai fixe supplémentaire ni changement du watcher de production n’est ajouté.

Aucune visibilité supplémentaire n’est donnée aux moteurs pour les tests, et aucun algorithme du gestionnaire actif n’est modifié. Les capacités en attente de décision fonctionnelle et les champs de configuration historiques restent conservés.

### Validation du lot 2

- `cargo check --all-targets --offline` : réussi sous Windows, sans avertissement du projet.
- `cargo test --all-targets --offline --quiet` : 91 tests backend et 89 tests du binaire réussis ; aucun échec, un test de montage Windows ignoré. Le nombre total de tests est conservé ; les deux tests retirés de `commands/archive.rs` sont adaptés dans `systems/file_ops.rs`.
- Le premier passage backend a donné 90 réussites, un échec du test de watcher décrit ci-dessus et un test ignoré. Après synchronisation de ce test avec la libération du callback, le test isolé puis la suite complète ont réussi.
- Linux n’a pas été compilé dans ce lot ; aucune vue UI n’a été modifiée.

### Lot 3 — retrait de l’ancienne migration

L’utilisateur a décidé de supprimer l’ancienne migration et d’en concevoir une nouvelle plus tard. La recherche des références dans le dépôt confirme que `run_migration` n’a aucun appelant ; seule la déclaration de module la raccordait à la compilation.

- Suppression de `src/systems/migration.rs` et de `pub mod migration` dans `src/systems/mod.rs`.
- La conception et le raccordement d’une nouvelle migration sont différés.

Validation : `cargo check --all-targets --offline` réussi sous Windows, sans avertissement du projet. Aucun test spécifique n’existait pour ce module inutilisé ; les suites validées au lot 2 sont conservées et n’ont pas été relancées pour ce retrait sans appelant. Linux n’a pas été compilé.

### Lot 4 — derniers ponts historiques, configuration et annotations

Base : commit `2e959ab`. Les usages ont été revérifiés dans les sources, tests inclus. L’utilisateur a explicitement demandé la suppression de l’ancien support de l’accent système et du transfert Shell spécifique à la corbeille.

- Retrait de `WindowState`, `SnapRect` et `oxide_sync_snap_rect`. Le bouton Maximiser actuel de `title_bar.rs` utilise déjà `WindowControlArea::Max` ; dans la dépendance GPUI au commit `33c9585`, `crates/gpui_windows/src/events.rs` traite cette zone via `HTMAXBUTTON`. Aucun appel ni lecteur de l’ancien rectangle n’a été trouvé.
- Retrait de `get_accent_color` dans les commandes, le trait `Platform` et les deux implémentations natives.
- Retrait de `set_clipboard_from_trash`, de `set_drop_effect_on_data_object` et de leurs imports devenus inutiles. Le presse-papiers générique reste actif.
- Retrait de `font_size`, `layout`, `group_by_date` et `first_launch` d’`AppConfig` et de ses valeurs par défaut. L’utilisateur a ensuite demandé de supprimer également leur conservation comme options inconnues : `extra` et `serde(flatten)` sont retirés, les trois données actuelles `workspace_state`, `duplicate_search_options` et `duplicate_search_window_size` sont déclarées explicitement et les accès GPUI sont adaptés. Le test `unknown_options_are_discarded_without_losing_current_view_state` vérifie la disparition des anciennes clés et d’une clé inconnue quelconque à la sauvegarde, sans perte de ces trois données, après édition, rechargement et réinitialisation. Le regroupement actif des onglets et la disposition du workspace restent présents.
- Retrait de toutes les annotations `allow(dead_code)` restantes. `HardDrive`, `DropdownItem`, `Tooltip::text` et `FileDragData::source_pane_id` ont des usages et restent présents. La compilation sans ces annotations a confirmé que les builders `Button::icon`, `selected` et `tooltip` étaient inutilisés ; ils sont retirés avec leurs champs et branches de rendu inactives. La variante `Primary` n’avait aucun constructeur appelé ; elle est retirée. Les trois boutons actifs utilisent `Subtle`, avec les mêmes couleurs, dimensions et interactions.

Validation Windows : `cargo check --all-targets --offline` sans avertissement du projet ; `cargo test --all-targets --offline --quiet` avec 92 tests backend et 89 tests du binaire réussis, aucun échec et un test de montage ignoré. Linux et l’interface interactive n’ont pas été validés dans ce lot.

### Vérification demandée après le lot 4 — noms restaurés depuis la corbeille

La vue GPUI `execute_trash_action` transmet les identifiants des éléments à `commands::ops::restore_items`, aussi bien pour la sélection que pour Tout restaurer. Ce parcours a été testé dans la corbeille Windows réelle avec trois fichiers temporaires (texte avec espaces et accents, `.lnk`, `.url`) et un dossier avec contenu. Aucun élément utilisateur n’a été sélectionné.

- Le premier passage a confirmé la restauration physique avec les noms et contenus attendus, puis échoué sur les métadonnées : le terminateur UTF-16 était inclus dans les noms et chemins Rust. Il provoquait également l’ajout d’une seconde extension aux raccourcis. La restauration native tronquait au terminateur, ce qui expliquait la différence entre les fichiers restaurés et les chemins renvoyés.
- Correction de la lecture des fichiers `$I…` : retrait du terminateur et du padding ; distinction du format v1 (chemin à l’octet 24) et du format v2 (longueur à l’octet 24, chemin à l’octet 28). Les dispositions sont confirmées par la [documentation du projet recyclebinunit](https://github.com/danielmarschall/recyclebinunit/blob/master/FORMAT.md). L’ancienne fonction traitait les deux versions comme v2.
- La lecture native est déplacée dans `platform/windows_trash.rs`, appelée via `Platform::trash_original_path`. Linux conserve le chemin fourni par la crate `trash`.
- Trois tests de métadonnées couvrent les deux formats, les accents, une paire de surrogates, les extensions, les chemins UNC, les buffers tronqués, les versions inconnues et les chemins non absolus. Le test natif vérifie les noms affichés, les chemins d’origine et renvoyés, le contenu et l’absence de fichiers supplémentaires après restauration. Il reste ignoré par défaut car il utilise la corbeille réelle ; son exécution explicite a réussi après correction.

Cette validation appelle le backend utilisé par GPUI ; elle ne simule pas les clics dans l’interface et ne couvre pas les conflits avec un fichier déjà présent à l’emplacement d’origine. Linux n’a pas été testé.

Validation finale Windows : `cargo check --all-targets --offline` sans avertissement ; suite complète avec 95 tests backend et 89 tests du binaire réussis (184 au total), aucun échec, deux tests natifs ignorés par défaut. Le test `native_restore_preserves_original_names`, exécuté séparément avec `--ignored --exact`, a réussi.

### Lot 5 — événements inutilisés, façades résiduelles et artefacts Tauri

Base : commit `b9bed49`. Ce lot provient d’une nouvelle recherche dans les sources Rust, les tests et les fichiers suivis du dépôt. La bibliothèque est utilisée uniquement par l’application, conformément à la confirmation de l’utilisateur.

- Suppression de `DirBatchEvent` et du module `models/progress.rs` (`ProgressEvent`). Aucune construction ni consommation de ces deux anciens modèles d’événements n’a été trouvée ; la progression actuelle repose sur `FileOperation` et les événements de recherche actifs.
- Retrait de `get_peek_status` et du réexport `commands::system::PeekStatus`, sans appelant. La tâche GPUI de `shell_features.rs` consulte déjà `Platform::peek_status` en arrière-plan. Le type natif, les implémentations Windows/Linux et `open_peek` restent utilisés.
- Retrait de `commands::io::create_dir`, sans appelant. `OxydeApp::create_new_folder` passe déjà par `create_archive_directory_with_quality`, qui gère dossiers ordinaires et archives, puis enregistre l’historique dans la vue.
- Retrait des wrappers `rename_item` / `rename_item_with_quality`. Leur seul point d’entrée était le test de mutations d’archives dans `systems/file_ops.rs`, adapté à `rename_item_controlled`, utilisée par GPUI avec la qualité et l’annulation. Le test conserve les mêmes assertions et les quatre formats. La structure `_tx_details` du renommage ordinaire, construite puis abandonnée, est supprimée avec ses conversions de chemins ; aucun enregistrement d’historique existant n’est retiré.
- Suppression de `TrashDriveSettings` et d’`AppConfig::trash_drive_settings`. La seule utilisation fonctionnelle était l’écriture d’une copie des réglages après leur sauvegarde native ; aucune lecture de cette copie n’a été trouvée. Le dialogue lit et écrit déjà les réglages via `Platform`. Ces appels, le traitement des erreurs et la préférence active `confirm_delete` sont conservés. Le test de sauvegarde de configuration vérifie aussi que l’ancienne clé disparaît.
- Retrait des réexports `commands::clipboard`, `commands::icons`, `commands::network` : les deux relais de presse-papiers Windows et le nettoyage du cache appellent désormais les modules `platform` directement. Retrait des quatre réexports globaux d’`ui/app/mod.rs` et de leurs `allow(unused_imports)` ; les méthodes d’implémentation restent disponibles sans réexport. Retrait des commentaires de commandes WebView déjà supprimées.
- Suppression des quatre fichiers suivis de `gen/schemas/`, générés pour les capacités, permissions et schémas Tauri. Aucun consommateur actuel n’a été trouvé dans le dépôt ; Cargo n’a aucune dépendance Tauri et aucun script de génération ne les utilise.
- Retrait des types de bibliothèque `staticlib` et `cdylib` du manifeste. Aucun consommateur natif, export FFI ni script utilisant ces sorties n’a été identifié. Cargo conserve la bibliothèque Rust utilisée par le binaire, ce que confirme `cargo metadata`. L’annotation `allow(linker_messages)` liée aux sorties d’import/export est également retirée.

Validation Windows : `cargo check --all-targets --offline` sans avertissement du projet ; `cargo test --all-targets --offline --quiet` avec 95 tests backend et 89 tests du binaire réussis (184 au total), aucun échec et deux tests natifs ignorés par défaut. Linux et les interactions du dialogue de corbeille n’ont pas été testés dans ce lot.

### Lot 6 — listing, derniers relais et contrôles natifs

Base : commit `4cff976`. Les appelants actuels ont été relus avant suppression ; ce lot complète la feuille de route par une nouvelle vérification du code.

- Retrait du paramètre inutilisé `_force_refresh` de `list_dir`. Les appelants GPUI de navigation, de complétion et du menu de fil d’Ariane, ainsi que les tests, sont adaptés.
- `list_dir` renvoie directement les entrées : aucun appelant ne lisait `DirResponse::summary`, `is_complete` ou `is_protected`. Le résumé calculé puis abandonné, `DirResponse` et `FileSummary` sont supprimés. Les erreurs, le tri et les filtres restent traités comme auparavant ; `FileEntry::is_protected`, utilisé pour les éléments individuels, est conservé.
- Retrait du constructeur sans appelant `CommandError::new` et de `Platform::name` dans le trait et ses deux implémentations.
- Retrait des deux relais de streaming vidéo dans `commands::thumbnails` et `utils::thumbnails`, puis de l’alias `models::Result` devenu inutilisé. L’infobulle appelle directement `Platform::video_frames` sur son thread existant, avec le même canal borné et les mêmes horodatages ; les moteurs natifs restent inchangés.
- Ajout d’un test natif de conflit de restauration : un fichier temporaire est envoyé à la corbeille, puis un fichier est recréé au même emplacement. La restauration doit signaler le conflit, conserver le contenu du fichier présent et laisser l’original dans la corbeille. Le test retire ensuite uniquement son fichier bloquant et restaure sa fixture sous le nom d’origine, avant les assertions. Aucun fichier utilisateur n’est sélectionné.

Validation Windows : `cargo check --all-targets --offline` sans avertissement du projet ; `cargo test --all-targets --offline --quiet` avec 95 tests backend et 89 tests du binaire réussis (184 au total), aucun échec et trois tests natifs ignorés par défaut. Le nouveau test `native_restore_conflict_preserves_existing_file_and_trash_item`, exécuté séparément avec `--ignored --exact`, a réussi. `cargo build --bin oxyde --offline` a également réussi.

Contrôles visuels réalisés sur le binaire GPUI construit depuis cette branche : le dialogue des propriétés de corbeille charge les réglages natifs des lecteurs C, D et E ; la sélection du lecteur D affiche sa capacité, et le passage de Mo à pourcentage fonctionne. Le dialogue est fermé avec Annuler, sans sauvegarder de réglages natifs. L’agrandissement de la fenêtre et le retour à sa taille initiale fonctionnent. La fenêtre de test a ensuite été fermée.

Deux validations restent ouvertes :

- Le panneau Snap Layouts au survol du bouton Maximiser n’a pas été validé visuellement : l’outil de contrôle utilisé ne fournit pas de déplacement de souris sans clic. Le contrôle visuel ci-dessus porte sur Maximiser/Restaurer.
- `cargo check --all-targets --target x86_64-unknown-linux-gnu --offline` ne peut pas aboutir : `accesskit_atspi_common v0.19.1` manque dans le cache et Cargo devrait la télécharger. Aucune compilation Linux réussie n’est revendiquée.

### Correction signalée après le lot 6 — témoin d’insertion des onglets

Lors d’un glissement entre panneaux vers la fin de la liste d’onglets, deux traits utilisaient le même index de dépôt : le bord droit du dernier onglet dans `layout.rs` et un second trait après le bouton Nouvel onglet dans `TabBar`. Le second rendu, son champ et son builder sont supprimés. Les marqueurs placés aux limites des onglets, les zones de dépôt et la géométrie de la barre restent conservés. `cargo check --all-targets --offline` a réussi sans avertissement du projet. La vérification visuelle est laissée à l’utilisateur, conformément à sa préférence ; Computer Use n’est pas relancé pour cette correction.

### Confirmation utilisateur et nouvel audit — 7 octobre 2026

L’utilisateur confirme Snap Layouts et le témoin de déplacement des onglets. Ces validations sont donc closes. Linux est différé à sa demande ; son absence de validation ne bloque pas la suite Windows.

Une nouvelle revue du code après les six lots est consignée dans [AUDIT_CODE_MORT_2.md](AUDIT_CODE_MORT_2.md). Les corrections, améliorations et optimisations à examiner ensuite sont séparées dans [AUDIT_AMELIORATIONS.md](AUDIT_AMELIORATIONS.md). Aucun nouveau nettoyage n’est appliqué pendant cet audit.

Rectification du lot 6 : contrairement à la mention « utilisé pour les éléments individuels », `FileEntry::is_protected` n’a pas de lecteur actuel. Il est recensé dans le nouvel audit avec les autres métadonnées sans consommateur.

### Nouvelle série de nettoyage N01 à N16

Les points du nouvel audit sont traités en quatre lots enchaînés à la demande de l'utilisateur. Le détail des suppressions et les validations figurent dans le journal de [AUDIT_CODE_MORT_2.md](AUDIT_CODE_MORT_2.md). Deux champs de propriétés de corbeille sont conservés jusqu'au traitement d'A04, comme prévu dans l'audit. Les corrections et optimisations restent dans [AUDIT_AMELIORATIONS.md](AUDIT_AMELIORATIONS.md). Computer Use et la validation Linux sont exclus de cette série à la demande de l'utilisateur.
