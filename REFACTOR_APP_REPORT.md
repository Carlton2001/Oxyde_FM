# Refactorisation de `src/ui/app/mod.rs`

Validation du 10 octobre 2026, sur Windows. Branche : `codex/refactor-app-modules`, créée depuis `master` à `5d75bd1` après inspection du dépôt. Plan approuvé avant modification du code. Exploration, implémentation et revue confiées aux rôles configurés `explorer`, `implementer` et `reviewer`, avec étapes successives et revues avant passage à la suivante.

Le fichier racine passe de **3 960 à 500 lignes**. Il conserve les types partagés, les 121 champs de `OxydeApp`, les actions GPUI et le constructeur. Les responsabilités déplacées restent dans `src/ui/app/`. Aucun appelant, composant de présentation, backend, dépendance, catalogue, icône ou parcours de sortie finale du processus n'a été modifié.

L'utilisateur a ensuite validé ses tests visuels et autorisé le commit ainsi que la fusion locale dans `master`. Aucun push n'est demandé. Les fichiers non suivis préexistants (`.codex/`, `Comptage lignes Rust.ps1`, `REFACTOR 1.md` à `REFACTOR 4.md`) sont préservés. Les fixtures et journaux de validation sont dans `target/`, ignoré par Git.

## Arborescence et responsabilités

`E` : fichier préexistant ; `M` : préexistant enrichi par cette refactorisation ; `N` : nouveau.

```text
src/ui/app/
├── mod.rs                         M  façade, état, types, actions, construction
├── advanced_search.rs             E
├── archives.rs                    N  compression, extraction et mise en file
├── calendar.rs                    E
├── clipboard.rs                   N  copier, couper, coller, noms et chemins
├── context_menus.rs               N  requêtes natives, exécution et géométrie
├── context_menu_render/           N
│   ├── mod.rs                     N  orchestration et ancrage des overlays
│   ├── items.rs                   N  menus d'items et en-têtes des filtres
│   ├── native.rs                  N  menus Shell et sous-menus asynchrones
│   └── name_filter.rs             N  filtre de nom et focus de son champ
├── disk_images.rs                 E
├── drag.rs                        N  données d'aperçu, mode de transfert, pointeur
├── drive_watchers.rs              M  tâches, réconciliation et lecteurs
├── duplicates.rs                  E
├── file_operations.rs             N  transferts, conflits, suivi, annuler/rétablir
├── filters.rs                     E
├── keyboard.rs                    E
├── layout.rs                      E
├── navigation.rs                  M  fil d'Ariane et navigation existante
├── network.rs                     E
├── preferences.rs                 M  préférences et position du panneau d'opérations
├── properties.rs                  E
├── rename.rs                      N  renommage, création de dossier et préparation
├── render.rs                      N  impl Render, assemblage principal et événements
├── search.rs                      E
├── selection.rs                   M  actions de sélection et logique existante
├── shell_features.rs              M  ouverture et intégrations Shell existantes
├── sizes.rs                       E
├── toasts.rs                      E
└── trash_settings.rs              M  suppression et parcours corbeille existants
```

Les 18 modules préexistants sont conservés. Le découpage ajoute 11 fichiers Rust. Les modules préexistants qui dépassaient déjà la taille indicative ne sont pas redécoupés artificiellement. Le constructeur reste racine pour conserver son ordre et son hook de fermeture.

## Comptage des lignes

Lignes physiques, commentaires et lignes vides compris. La colonne Tests compte les blocs de modules `#[cfg(test)]`, attribut et délimiteurs inclus. Les autres lignes sont classées Production. Les tests externes au sous-arbre `app/` ne sont pas comptés ici.

| Fichier sous `src/ui/app/` | État | Production | Tests | Total |
|---|---|---:|---:|---:|
| `mod.rs` | M | 500 | 0 | 500 |
| `advanced_search.rs` | E | 1554 | 13 | 1567 |
| `archives.rs` | N | 115 | 0 | 115 |
| `calendar.rs` | E | 49 | 0 | 49 |
| `clipboard.rs` | N | 66 | 0 | 66 |
| `context_menus.rs` | N | 324 | 0 | 324 |
| `context_menu_render/mod.rs` | N | 57 | 0 | 57 |
| `context_menu_render/items.rs` | N | 485 | 0 | 485 |
| `context_menu_render/native.rs` | N | 335 | 0 | 335 |
| `context_menu_render/name_filter.rs` | N | 97 | 0 | 97 |
| `disk_images.rs` | E | 223 | 25 | 248 |
| `drag.rs` | N | 474 | 0 | 474 |
| `drive_watchers.rs` | M | 516 | 141 | 657 |
| `duplicates.rs` | E | 1098 | 0 | 1098 |
| `file_operations.rs` | N | 319 | 0 | 319 |
| `filters.rs` | E | 400 | 0 | 400 |
| `keyboard.rs` | E | 102 | 0 | 102 |
| `layout.rs` | E | 1154 | 156 | 1310 |
| `navigation.rs` | M | 663 | 75 | 738 |
| `network.rs` | E | 555 | 120 | 675 |
| `preferences.rs` | M | 364 | 0 | 364 |
| `properties.rs` | E | 1094 | 112 | 1206 |
| `rename.rs` | N | 504 | 0 | 504 |
| `render.rs` | N | 376 | 0 | 376 |
| `search.rs` | E | 740 | 176 | 916 |
| `selection.rs` | M | 282 | 16 | 298 |
| `shell_features.rs` | M | 245 | 20 | 265 |
| `sizes.rs` | E | 105 | 0 | 105 |
| `toasts.rs` | E | 335 | 0 | 335 |
| `trash_settings.rs` | M | 1053 | 11 | 1064 |

## Interfaces et invariants préservés

Les définitions de `OxydeApp`, `FileDragData`, `FileDragPreview`, `DragTarget`, `ContextMenuHeader`, `ContextMenuContent`, `ContextMenuInstance`, `PendingFileOperation`, `SettingsDataAction` et `SettingsDataConfirmation` restent dans le module racine. Leurs champs et visibilités restent identiques.

Les réexportations de `OxydeTheme`, `ThemeContext`, `WorkspaceState`, `FileEntry` et du prélude GPUI sont conservées. `drives_differ` est implémenté dans `drag.rs` et réexporté avec `pub use drag::drives_differ`, conservant le chemin `app::drives_differ`.

Les méthodes restent attachées à `OxydeApp` par des blocs `impl` enfants : leurs chemins d'appel, paramètres, types de retour et visibilités existantes sont conservés, sous réserve des portées minimales listées ci-dessous. Cela comprend les actions de fichiers, navigation, sélection, ouverture, renommage, création de dossier, transferts, conflits, archives, watchers et préférences. `FileDragPreview::new` et ses implémentations GPUI conservent leur interface.

La macro `actions!` reste à son emplacement dans `app`, sous le namespace **`oxyde`**, avec les 12 mêmes actions : `Ouvrir`, `Copier`, `CopierNom`, `CopierChemin`, `Couper`, `Coller`, `Renommer`, `Supprimer`, `ToutSelectionner`, `Annuler`, `Refaire`, `Actualiser`.

La revue mécanique a comparé les corps déplacés avec l'état initial. Pour le rendu principal, elle a reconstitué le bloc original en réinsérant les helpers extraits : préparation du renommage après création, boucle des menus et événement de déplacement du pointeur. Les trois branches de menus ont ensuite été comparées séparément après normalisation de l'indentation. La position originale du menu est explicitement transmise au helper d'items ; les captures et les clones restent identiques.

Les paramètres backend, chemins physiques, snapshots de sélection, intentions du presse-papiers, décisions de conflits et entrées d'historique restent identiques. Les tâches conservées/détachées, subscriptions, références faibles, tokens et révisions, rejets de réponses obsolètes et ordre des mises à jour n'ont pas été modifiés. Les handles de focus et de défilement, identifiants GPUI, enregistrements d'actions, phases d'événements, géométrie et ordre des overlays sont conservés. Aucun changement de curseur, couleur, texte, icône ou dimension n'a été introduit.

## Ajustements de visibilité

Les nouveaux modules sont privés. Aucun champ de `OxydeApp` n'a été élargi.

| Éléments | Ajustement | Justification |
|---|---|---|
| `native_menu_max_height`, `context_menu_contains_point`, `context_menu_height`, `context_menu_visible_top` | privé → `pub(super)` | Partage des helpers géométriques avec les autres enfants de `app` |
| `archive_qualities`, `archive_quality_for_path`, `queue_archive_operation` | privé → `pub(super)` | Appels depuis les frères `rename` et `preferences` après extraction |
| `watch_file_operation` | privé → `pub(super)` | Suivi partagé des opérations, notamment archives |
| `begin_rename`, `queue_created_item_rename` | privé → `pub(super)` | Appels depuis les autres parcours de `app` |
| `prepare_created_item_rename`, `handle_pointer_move`, `render_context_menus` | nouveaux helpers `pub(super)` | Appels depuis le rendu principal du module frère |
| `render_items_menu`, `render_native_menu`, `render_name_filter_menu` | nouveaux helpers `pub(super)` | Appels limités au parent `context_menu_render` |

Les méthodes et types qui étaient déjà `pub` ou `pub(crate)` conservent leur portée. Deux exceptions locales `#[allow(clippy::ptr_arg)]` sur les helpers d'items et natifs conservent les paramètres empruntés et les clones d'origine ; aucune substitution de collection ni réécriture du corps n'a été faite pour satisfaire le lint.

## Vérifications Rust

Les vérifications initiales avaient déjà réussi : `cargo check`, `cargo check --all-targets --offline` et la suite complète (852 réussis, 17 ignorés). Après chaque étape, le reviewer a comparé les corps et exécuté `cargo check --all-targets --offline` ainsi que les tests concernés : conflits, renommage, création de dossier, entrées clavier, navigation, watchers, Shell, sélection, drag, menus et layout. Les erreurs intermédiaires de paramètre de position et d'import devenu inutilisé ont été corrigées avant validation finale.

| Vérification finale | Résultat |
|---|---|
| `cargo check --all-targets --offline` | Réussi, sans avertissement |
| `cargo test --offline` | 852 réussis, 17 ignorés, aucun échec |
| `cargo clippy --all-targets --offline` | Réussi, aucune nouvelle alerte après les exceptions ciblées |
| `cargo build --offline --bin oxyde` | Réussi |
| `git diff --check` | Réussi |

Répartition de la suite complète : bibliothèque 640 réussis/16 ignorés ; binaire 163/1 ; intégration archives I/O 20 ; worker archives 14 ; listing archives 6 ; worker aperçu 9 ; doctests 0. Le test de sous-processus affiché séparément dans le journal est déjà inclus dans le total bibliothèque.

Clippy conserve les alertes préexistantes : clones/allocations et références inutiles, simplifications arithmétiques et de contrôle de flux, signatures complexes, organisation des tests, conversions et `set_readonly(false)`. Les comptes affichés par cible incluent des doublons (30 bibliothèque, 67 bibliothèque/tests, 5 binaire, 11 binaire/tests, une exemple, une test aperçu). Aucun nettoyage hors périmètre n'a été effectué. Le build et les tests peuvent afficher l'avertissement environnemental préexistant de cache incrémental sans liens physiques ; le dernier `cargo check` n'affiche aucun avertissement.

Journaux : `target/refactor-final-check.log`, `target/refactor-final-test.log`, `target/refactor-final-clippy.log`, `target/refactor-final-build.log`. Aucun test reproduisant uniquement le découpage n'a été ajouté.

## Contrôles visuels et interactifs réellement effectués

Computer Use a servi à observer le binaire initial, un binaire intermédiaire après les extractions fonctionnelles, puis le binaire final. Les opérations ont utilisé des fixtures sous `target/refactor-ui-fixtures`, avec vérifications sur le système de fichiers pour les transferts. Les captures étaient observées directement ; aucune comparaison pixel à pixel automatisée n'a été réalisée.

| Parcours | Contrôle et résultat |
|---|---|
| Démarrage et restauration | Binaire initial, intermédiaire et final lancés ; dossier et mode du workspace restaurés |
| Navigation / fil d'Ariane | Saisie d'un chemin de fixture, validation, retour et avance entre source/destination |
| Détails / Grille | Détails et Grille observés avant extraction ; Grille vérifiée après extraction et sur le binaire final |
| Menu fichier / Shell | Menu contextuel et « Plus d'options » natif chargés sur le binaire initial ; pas de nouvelle observation native finale avant arrêt de Computer Use |
| Renommage | F2, sélection du nom sans extension, validation vers `renamed.txt` sur le binaire intermédiaire ; fichier vérifié |
| Nouveau dossier | Menu de fond, création puis saisie inline de `created-folder` ; résultat vérifié |
| Annuler / rétablir | Création du dossier annulée puis rétablie ; notifications et présence physique vérifiées |
| Presse-papiers | Sous-menu « Copier le nom » et notification ; Ctrl+C puis Ctrl+V entre dossiers : contenu copié vérifié ; la valeur texte du presse-papiers n'a pas été vérifiée directement |
| Couper / déplacer | Ctrl+X puis Ctrl+V : source absente, destination présente et contenu vérifié |
| Conflit | Second collage : dialogue simple, passage avancé, décision Ignorer puis Appliquer ; fermeture sans remplacement |
| Watchers | Création externe de `watcher.txt`, apparition automatique dans la liste sans rafraîchissement manuel ; transferts également reflétés |
| Notifications | Toasts de copie, coupe, annulation et rétablissement observés |
| Fermeture | Binaire initial et intermédiaire fermés normalement ; pour l'intermédiaire, sortie GPUI et arrêt Tokio complets dans le journal, sans blocage |
| Thèmes, binaire final | Les 13 palettes sélectionnées et observées avec la Grille et les menus de préférences : Oxyde, Ayu, GitHub, One, Solarized, Windows en clair/sombre, et Monokai |
| Langues, binaire final | Français, anglais, allemand, espagnol et italien observés dans les préférences, leurs sous-menus et les compteurs ; largeurs adaptées et libellés longs visibles ; français rétabli |

Le mécanisme de saisie de Computer Use remplaçait le presse-papiers lors de la saisie d'un chemin. Le contrôle copie/collage a donc été repris avec Retour/Avance sans saisie entre les deux actions ; le transfert a alors réussi et son contenu a été vérifié. Cela ne constitue pas un changement du code de l'application.

## Limites et état laissé après arrêt de Computer Use

L'utilisateur a demandé d'arrêter Computer Use pendant la validation finale. Aucune autre action dans l'interface n'a été effectuée ensuite. L'application reste ouverte dans le dossier de fixture `destination`, en Grille, thème **Windows sombre**, langue **français**, échelle **16 px**. Le thème initial Oxyde clair et le dossier initial `D:\Tests` n'ont donc pas été rétablis ; aucune modification directe de la configuration n'a été faite pour contourner cet arrêt.

Les parcours suivants n'ont pas été validés interactivement après refactorisation : panneaux multiples, zoom, drag et modificateurs, filtres de colonnes/nom extraits, suppression normale/permanente et transferts depuis la corbeille, compression/extraction par l'UI, progression longue/annulation d'une opération active, et nouvelle ouverture finale des sous-menus Shell. Les tests concernés et la comparaison mécanique apportent une couverture, sans remplacer ces observations UI. Les langues et thèmes ont été parcourus séparément ; les 65 combinaisons et tous les états erreur/vide/modale n'ont pas été testés.

Seule la cible Windows a été compilée et exécutée ici. Linux n'a pas été compilé ni testé interactivement. Les 17 tests ignorés restent ignorés. La fermeture après aperçu PDF/vidéo n'a pas été rejouée : les ressources d'aperçu et le parcours de sortie n'ont pas été modifiés ; le déplacement de `FileDragPreview` conserve son abonnement. Les garanties asynchrones et de sécurité ont été contrôlées par comparaison et par la suite existante, sans nouvelle campagne exhaustive de courses ou d'erreurs natives.

Aucune régression n'a été identifiée dans les comparaisons, tests et parcours effectivement observés. Les limites ci-dessus restent explicites et ne sont pas considérées comme validées par la seule compilation.

## Validation complémentaire de l'utilisateur

Après transmission de la liste des contrôles visuels restants, l'utilisateur a confirmé « test OK » et demandé le commit puis la fusion dans `master`. Cette validation manuelle complète la revue de l'agent ; elle ne constitue pas une nouvelle observation par Computer Use et n'élargit pas les cibles compilées ni les tests automatisés exécutés.
