# Nouvel audit de code mort — 7 octobre 2026

Base : `3cc7d14`, branche `codex/nettoyage-code-mort`. Cet audit reprend le code après les six premiers lots ; l'ancienne feuille de route ne sert pas de preuve d'inutilisation. Les constats ci-dessous décrivent cette base ; le nettoyage autorisé ensuite est suivi dans le journal en fin de document.

L'utilisateur confirme le fonctionnement de Snap Layouts et du témoin de déplacement des onglets. Linux est différé : la validation cible Windows, sans supprimer le support Linux ni considérer son absence de compilation comme un blocage. La bibliothèque Rust est utilisée uniquement par cette application.

## Périmètre et méthode

Revue de l'inventaire des sources Rust, des déclarations de modules, du manifeste, des fichiers suivis, des ressources embarquées et des catalogues. Recherches des constructions, lectures, écritures et appelants dans le backend, GPUI et les tests ; remontée des parcours actifs pour les opérations, les propriétés, les archives, la recherche et la plateforme.

Les recherches lexicales servent à sélectionner les candidats, puis leurs usages sont relus. Une définition publique, un champ sérialisable ou un test peut masquer du code inutilisé en production sans déclencher `dead_code`. Inversement, une méthode de trait, une ressource chargée dynamiquement et un garde conservé pour son `Drop` restent utiles sans appel explicite ou lecture de champ.

Les constats portent sur le dépôt actuel. Ils ne prouvent pas l'absence de tout défaut fonctionnel. Les observations de comportement et de performance sont consignées dans [AUDIT_AMELIORATIONS.md](AUDIT_AMELIORATIONS.md), avec leurs limites de validation.

## Candidats

Les numéros de ligne ci-dessous sont ceux de la base auditée ; ils évolueront après nettoyage. « Confirmé » signifie absence de consommateur de production dans le dépôt, pas autorisation de supprimer les tests utiles ou de changer un comportement actif.

### N01 — Dépendance `mtzip` — confirmé

- `Cargo.toml:40` déclare `mtzip` avec la fonctionnalité `rayon`. Aucun usage de `mtzip` dans les sources actuelles ; les moteurs d'archives utilisent d'autres crates.
- Retirer la dépendance directe, puis laisser Cargo recalculer le verrouillage. Ne retirer du lockfile que les dépendances devenues inaccessibles.
- Conserver `rayon`, utilisé directement par la recherche de doublons.

### N02 — Helpers de chemins et résolution de raccourcis — confirmé / tests seuls

- `utils/path_security.rs:39` : `join_path` n'est appelé que par ses deux propres tests et se limite à `base.join(input)`. Le helper et ces tests de délégation peuvent disparaître ; les tests de validation des chemins restent nécessaires.
- `commands/io.rs:575` : `resolve_shortcut_target` n'a aucun appelant. Sa branche `Platform::resolve_shortcut` devient également inaccessible depuis l'application. L'implémentation Windows renvoie déjà `None`.
- Conserver `get_shortcut_info`, `Platform::shortcut_info`, `set_shortcut_info` et l'ouverture native : ils servent aux propriétés, à l'édition et à l'ouverture actuelles des raccourcis. La chaîne inutilisée ne doit pas entraîner leur suppression.

### N03 — Wrapper de planification réservé aux tests — tests seuls

`systems/file_ops.rs:354` : `queue_operation` délègue à `queue_operation_on` avec `Handle::current`. Ses appelants sont uniquement des tests ; GPUI utilise explicitement `queue_operation_on`.

Adapter les tests au point d'entrée actif, ou limiter le wrapper à `cfg(test)`. Conserver les assertions de planification, priorité et annulation ; le gestionnaire lui-même reste actif.

### N04 — État GPUI écrit sans lecture — confirmé

- `ui/app/mod.rs:413` : `pane_origins`, initialisé puis alimenté par `layout.rs:199`, n'a pas de lecteur.
- `ui/app/mod.rs:422` : `mouse_pos`, initialisé puis mis à jour par le mouvement de souris, n'a pas de lecteur.
- Retirer les champs et leurs seules écritures. Les tailles, bounds, positions utilisées localement et données de glissement restent nécessaires.

### N05 — Branches de composants sans effet — confirmé

- `ui/components/tabs/tab.rs:83` : `is_drop_target` reste constamment `false`. Le petit élément absolu créé à la ligne 244 ne reçoit jamais sa couleur. Les marqueurs actifs sont ceux de `ui/app/layout.rs` ; ils doivent rester présents.
- `TabPosition::Middle(Ordering)` : les trois valeurs d'`Ordering` ont exactement le même rendu (`tab.rs:237`). Le payload et son calcul peuvent disparaître, avec conservation de la variante `Middle` et de ses bordures.
- Couleur du bouton Fermer (`tab.rs:281`) : les deux branches renvoient `theme.icon_muted`.
- `ButtonVariant::Ghost` : valeur intermédiaire du constructeur, remplacée par `Subtle` dans les trois appels de l'application avant rendu. Le chemin visuel Ghost et l'API de sélection de variante peuvent être simplifiés autour du seul rendu actif. Conserver le composant et ses trois usages.

### N06 — Estimations d'opérations abandonnées — confirmé

- `models/file_entry.rs:146` : seuls les `conflicts` de `ConflictResponse` sont lus. `total_size`, `total_files`, `is_cross_volume` et `likely_large` ne sont pas consommés.
- `commands/ops.rs:364–423` calcule ces valeurs, avec un parcours d'estimation borné des dossiers. Ce travail peut être retiré avec les champs ; conserver la détection réelle des conflits et les estimations actives des opérations.
- `systems/file_ops.rs:96` : `FileOperation::is_cross_volume` reste `false` et n'est que recopié dans le snapshot, sans lecteur fonctionnel.

### N07 — Métadonnées de protection et horodatage — confirmé / tests seuls

- `FileEntry::is_protected` (`models/file_entry.rs:40`) n'a aucun lecteur et reste faux dans les constructions actuelles.
- `FileEntry::is_readonly` est rempli, mais sa seule lecture identifiée est une assertion du test ISO (`commands/archive.rs`). Préserver une vérification pertinente de la lecture seule du moteur ISO avant adaptation du test.
- `SidebarNode::is_readonly` et `is_protected` sont remplis sans lecture GPUI. Conserver le traitement des erreurs et le calcul actif de `has_subdirs`.
- `DriveInfo::is_readonly` est calculé par les plateformes sans consommateur actuel. Ne pas confondre ce champ avec la gestion effective des erreurs d'écriture.
- `Transaction::timestamp` (`models/transaction.rs:39`) est créé puis jamais lu ; l'historique actuel reste en mémoire. Conserver les identifiants et les détails nécessaires à Annuler/Rétablir.

**Rectification de l'audit précédent :** le lot 6 indiquait que `FileEntry::is_protected` était utilisé pour les éléments individuels. La nouvelle revue des lectures infirme cette affirmation.

### N08 — Métadonnées d'icône des raccourcis — tests seuls

`ShortcutInfo::icon_location` et `icon_index` sont produits par `windows_shortcuts.rs`, mais leurs lectures se limitent au test natif de raccourci. L'éditeur charge le `.lnk` existant et conserve son icône sans utiliser ces champs.

Le modèle et l'appel `GetIconLocation` du parcours de lecture sont candidats au retrait. Adapter le test pour contrôler directement l'icône du raccourci après sauvegarde ; ne pas perdre la couverture de sa conservation.

### N09 — Propriétés calculées sans affichage — confirmé, réserve corbeille

Dans `FileProperties` (`models/file_entry.rs:58`), GPUI ne lit pas `is_media_device`, `has_web_page`, `manufacturer`, `model_name`, `model_number`, `serial_number`, `mac_address`, `ip_address`, `unique_id`, `web_page_url` ou `debug_props`. La lecture d'`ip_address` dans `commands/io.rs:459` sert uniquement à remplir `has_web_page`, lui-même abandonné.

La branche Shell de `get_file_properties` énumère toutes les propriétés pour fabriquer `debug_props` (`io.rs:325`), puis interroge les métadonnées d'appareil (`io.rs:373–434`). Aucun de ces résultats n'est affiché. Retirer ces calculs avec leurs champs, **en conservant la récupération du nom convivial** (`io.rs:445`), utilisée par `props.name`, et les valeurs générales encore affichées.

`FileProperties::original_path` et `deleted_time` sont aussi remplis sans lecteur, avec une énumération de la corbeille (`io.rs:508`). Leur traitement est **à différer jusqu'à A04** : les propriétés d'un élément supprimé pourraient justement avoir besoin de ces informations. Les champs homonymes de `FileEntry` et `TrashEntry` restent actifs pour l'affichage, le tri et la restauration.

### N10 — Métadonnées réseau et détection de page web — confirmé

`NetResource::display_type`, `usage`, `provider` et `has_web_page` n'ont pas de consommateur dans `list_network_entries`. La recherche du verbe `open` par création et interrogation d'un menu Shell (`platform/network.rs:139–189`) sert uniquement à remplir le dernier champ : candidat au retrait avec cette branche.

Conserver `network_details`, `resource_type`, `is_media_device`, noms et chemins, effectivement utilisés. Les constantes et structures natives nécessaires à l'énumération WNet restent utiles indépendamment des champs Rust supprimés.

### N11 — Paramètres de copie ignorés — confirmé, préserver les erreurs

`commands/ops.rs:1550` : `_total_bytes` et `_task_name` de `perform_copy_with_progress` sont ignorés. Le helper et son contrôle d'annulation sont actifs.

Le total calculé par le parcours Rétablir (`ops.rs:1303`) est uniquement transmis au paramètre ignoré. Retirer ce total ne doit pas supprimer les erreurs de prévalidation : le calcul appelle actuellement `fs::metadata` avec propagation d'erreur avant la création des destinations. Préserver ce comportement ou traiter son changement séparément.

### N12 — Configuration de tri backend sans appel actif — confirmé

Tous les appelants de `list_dir` transmettent `None` pour `sort_config` ; `SortConfig` n'est pas chargé depuis la configuration. Le backend utilise donc toujours le tri naturel par nom ascendant. Les branches taille, date, type, emplacement, date de suppression et ordre descendant ne sont pas sélectionnées par l'application.

Simplifier cette API et ses types autour du comportement actuel. **Conserver le tri naturel par nom**, utilisé notamment par le fil d'Ariane ; la présence d'un autre tri GPUI ne rend pas ce tri intégralement mort. L'harmonisation éventuelle des tris relève d'A10.

### N13 — Sérialisation héritée sans consommateur — candidats à traiter par type

Les parcours actuels transmettent des valeurs Rust pour les erreurs, opérations, transactions, résultats de listing/propriétés/réseau, doublons et `PeekStatus`. Aucun consommateur de leur sérialisation n'a été identifié en production. Les implémentations Serde de ces types et leurs attributs associés peuvent être revérifiés et retirés progressivement.

Le test `older_entries_without_network_metadata_still_deserialize` entretient notamment une compatibilité JSON de `FileEntry` sans lecteur actuel de cette représentation. Sa nécessité disparaît si cette sérialisation est retirée.

Ce constat **ne concerne pas Serde dans son ensemble** : `AppConfig`, le workspace sauvegardé, les options de recherche avancée encodées dans l'historique `search://` et la désérialisation des images montées depuis PowerShell restent actives. Contrôler chaque type et les appels génériques avant retrait ; ne pas supprimer globalement les fonctionnalités Serde de toutes les dépendances.

### N14 — SVG embarqués sans référence — confirmé dans les parcours actuels

`assets/icons/logo.svg`, `redo.svg`, `refresh.svg` et `undo.svg` n'ont aucune référence dans les sources ou les chemins d'icônes actuels. Les ressources actives correspondantes sont `logo.png`, `redo-2.svg`, `refresh-cw.svg` et `undo-2.svg`.

Retrait possible des quatre anciens SVG après une dernière vérification des chemins embarqués. Les polices sont chargées dynamiquement par énumération des assets : l'absence de leur nom littéral dans les vues ne permet pas de conclure à leur inutilité.

### N15 — Traductions devenues fixtures — tests seuls

`Message::ConflictTitle` et `ConflictDescription` sont limités à `cfg(test)` et leurs anciennes clés demeurent dans les cinq catalogues pour les tests de traduction, de pluriels et de repli. Aucun affichage actuel ne les utilise.

Déplacer cette couverture vers des messages actifs, ou identifier clairement des fixtures dédiées, avant de retirer les clés. Les autres messages recherchés ont des consommateurs ; ne pas retirer les tests d'i18n eux-mêmes.

### N16 — Restes commentés — confirmé, faible enjeu

- Ancienne définition commentée de `SearchEvent` dans `commands/search.rs:17`.
- `.cargo/config.toml` contient uniquement un commentaire de configuration FFmpeg future, sans réglage actif.

Ces restes n'ont aucun effet d'exécution. Leur retrait relève de l'hygiène du dépôt, après les candidats fonctionnels.

## Éléments conservés après revue

- Aucun module Rust orphelin trouvé dans l'inventaire actuel ; les callbacks de traits GPUI et les types de rendu ne sont pas morts parce qu'ils n'ont pas d'appel textuel explicite.
- Tâches, watchers et abonnements conservés pour leur durée de vie, annulation et `Drop`, notamment les abonnements de fermeture et de sauvegarde.
- Moteurs d'archives, caches, restauration native, journaux de fichiers et protections contre écrasement utilisés par les parcours GPUI.
- Sortie finale du processus Windows documentée dans `SHUTDOWN_DIAGNOSTICS.md` : aucun retrait proposé.
- Pas de suppression proposée des fonctionnalités Windows Cargo sur la seule absence d'un nom de namespace : elles peuvent être nécessaires aux bindings utilisés.
- Support Linux conservé et différé ; aucun engagement de compilation ou de comportement Linux.

## Ordre proposé pour les prochains lots

1. Dépendance, façades sans appelant et wrappers de tests : N01–N03.
2. États et branches visuelles inertes : N04–N05, avec contrôle utilisateur des interactions modifiées.
3. Estimations et modèles sans lecteur : N06–N11, en conservant les assertions utiles et en réservant les propriétés de corbeille pour A04.
4. Tri backend, sérialisation, assets et fixtures : N12–N16, par changements limités et vérifiés.

Après chaque lot Rust : compilation Windows sans avertissement du projet et tests adaptés au périmètre. Une modification du parcours de restauration ou des raccourcis justifie les tests natifs concernés. Les corrections d'AUDIT_AMELIORATIONS restent des travaux distincts.

## Vérifications de l'audit initial, avant nettoyage

- `cargo check --all-targets --offline` : réussi sous Windows, sans avertissement du projet.
- `cargo clippy --all-targets --offline --message-format=json` : terminé avec succès ; suggestions consignées dans A13. Des diagnostics sont répétés entre les cibles, donc leur nombre brut n'est pas un nombre de problèmes distincts.
- Aucun code Rust, manifeste ou asset modifié ; suites de tests et tests natifs non relancés pendant cet audit documentaire. Leur dernier résultat reste celui du lot 6, et ne vaut pas reproduction des nouveaux risques.
- Aucun contrôle Computer Use ni compilation Linux entrepris.

## Journal du nettoyage N01 à N16 — 7 octobre 2026

L'utilisateur autorise l'enchaînement jusqu'à N16, sans attente entre lots après vérification, et exclut Computer Use. Les changements restent sur `codex/nettoyage-code-mort`, sans commit automatique. Les corrections et optimisations A01–A13 restent différées.

| Point | Résultat |
| --- | --- |
| N01 | `mtzip` retiré du manifeste ; Cargo retire son entrée du lockfile, sans retirer `rayon`. |
| N02 | `join_path`, ses deux tests de délégation et la chaîne `resolve_shortcut_target` / `Platform::resolve_shortcut` retirés. |
| N03 | Wrapper `queue_operation` retiré ; ses huit appels de tests utilisent `queue_operation_on` avec le même runtime courant. |
| N04 | `pane_origins`, `mouse_pos` et leurs seules écritures retirés. |
| N05 | Marqueur toujours invisible de `Tab`, payload `Ordering`, branche de couleur identique et variante Ghost retirés. Les marqueurs actifs et le rendu Subtle restent présents. |
| N06 | Estimations abandonnées et `FileOperation::is_cross_volume` retirés. `check_conflicts` renvoie directement les conflits ; `ConflictResponse` disparaît. |
| N07 | Champs `is_readonly` / `is_protected` sans lecteur et `Transaction::timestamp` retirés, avec leurs calculs et initialisations. |
| N08 | Champs d'icône de `ShortcutInfo` et lecture native de production correspondante retirés ; le test contrôle directement l'icône du `.lnk` sauvegardé. |
| N09 | Onze champs de propriétés sans affichage et leurs calculs retirés. **Réserve maintenue pour `original_path` et `deleted_time` de `FileProperties`, jusqu'à A04.** |
| N10 | Quatre champs de `NetResource`, conversion du fournisseur et exploration du menu Shell pour détecter une page web retirés. |
| N11 | Deux paramètres ignorés de copie et accumulation du total retirés ; les appels de prévalidation à `fs::metadata` et leurs erreurs restent présents au même stade. |
| N12 | Configuration et variantes de tri backend inactives retirées, ainsi que `models/sort.rs`. Tri naturel par nom ascendant et dossiers en premier conservés. |
| N13 | Serde retiré des modèles sans consommateur JSON, ainsi que leurs attributs et le test de compatibilité JSON de `FileEntry`. |
| N14 | Quatre anciens SVG retirés ; ressources actives et polices conservées. |
| N15 | Deux messages réservés aux tests et leurs clés des cinq catalogues retirés ; assertions de pluriels et de repli déplacées sur des messages actifs. |
| N16 | Ancien `SearchEvent` commenté et configuration Cargo vide de réglages retirés. |

### Points de conservation vérifiés

- Les propriétés et l'édition des raccourcis continuent de passer par les API natives actives. Le test `shortcut_edits_preserve_icon_and_hotkey` vérifie les valeurs éditées, l'icône et le raccourci clavier en relisant le fichier natif. Le contrôle d'icône a quitté le modèle de production, pas la couverture.
- Le test ISO conserve la vérification de `archive_workspace::writable`, de la navigation, de l'extraction et de son annulation. Les gardes du moteur contre la modification d'un format en lecture seule restent présents ; seul le drapeau descriptif inutilisé disparaît.
- Le test existant de listing est étendu aux dossiers en premier et aux noms numériques avec casse différente : mêmes assertions sur un dossier réel, ZIP, 7z, TAR et TAR.ZST, avec conservation des contrôles des niveaux virtuels.
- La revérification de N13 confirme aussi que les thèmes et langues sont persistés comme chaînes dans `AppConfig`, sans sérialiser les enums `ThemeId` / `Language`. Leurs implémentations Serde inutilisées disparaissent ; les tests portent sur `as_str` / `from_id`, les identifiants stables, régions et alias actifs. La configuration, le workspace, la recherche avancée et les réponses PowerShell des images montées conservent Serde.
- L'affichage des noms conviviaux des appareils réseau, les métadonnées réseau réellement affichées, les erreurs de copie, les progressions et l'historique de fichiers restent actifs.
- Les deux champs réservés de `FileProperties` et leur collecte sont conservés conformément à la réserve N09/A04. Les champs d'origine et de date de `FileEntry` / `TrashEntry` sont actifs et ne sont pas candidats à cette réserve.

### Vérifications par lot

| Lot | Compilation Windows | Tests |
| --- | --- | --- |
| N01–N03 | `cargo check --all-targets --offline`, sans avertissement du projet | 93 backend + 89 binaire réussis ; trois tests natifs ignorés. |
| N04–N05 | Même commande, sans avertissement du projet | 89 tests du binaire réussis. |
| N06–N11 | Même commande, sans avertissement du projet | 93 backend + 89 binaire réussis ; trois tests natifs ignorés. |
| N12–N16 | Même commande, sans avertissement du projet | 92 backend + 89 binaire réussis ; trois tests natifs ignorés. |

La suite finale compte **181 tests réussis, aucun échec**. Les trois tests retirés depuis la base sont les deux tests du wrapper `join_path` supprimé et celui de la sérialisation de `FileEntry` supprimée ; les tests de comportements actifs sont conservés ou adaptés. Les tests natifs de restauration réelle et de montage ignorés par défaut ne sont pas exécutés pendant ces lots.

Le premier `cargo check` sous sandbox a échoué parce que RustEmbed ne pouvait pas résoudre le chemin absolu des assets (`PermissionDenied`). La même commande hors sandbox a réussi ; aucun contournement ni modification de l'embarquement des ressources n'a été ajouté.

Construction finale `cargo build --bin oxyde --offline` : réussie, sans avertissement du projet. `git diff --check` ne signale aucune erreur. Aucun lancement ni pilotage de l'application, aucune validation visuelle revendiquée et aucune compilation Linux.

### Suite A04 — propriétés de corbeille

Les champs `FileProperties::original_path` et `deleted_time`, réservés lors de N09, sont désormais lus par le dialogue de propriétés pour afficher le nom, l'emplacement d'origine et la date de suppression. Leur collecte reste active et partage les règles de nommage de la restauration ; ils ne sont plus des champs sans lecteur. Voir [AUDIT_AMELIORATIONS.md](AUDIT_AMELIORATIONS.md).
