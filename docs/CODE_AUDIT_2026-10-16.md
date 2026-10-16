# Audit transversal du code — 16 octobre 2026

## Base et conclusion

Audit effectué sur `main`, révision `4f16bc915ea8d286d5e3eac422d32a17fc630e78`, sous Windows x86_64, avec Rust/Cargo 1.98.1. Aucun changement du code applicatif, des tests existants, des dépendances ou des réglages utilisateur. Aucun commit ni push.

Les constats et chiffres de cette section décrivent la passe initiale d'audit. Les corrections ultérieures A01 et A02, leurs branches et leurs validations sont suivies dans les sections dédiées ci-dessous.

Au départ, seuls quatre CSV étaient non suivis : `Benchmark_Smalls_Source_ED.csv`, `Benchmark_Video_Source_DD.csv`, `Benchmark_Video_Source_ED.csv`, `resultats.csv`. Ils ont été laissés intacts. Les sondes et journaux de cette passe sont dans `target/audit-2026-10-16/`, ignoré par Git ; ils ne constituent pas des tests de non-régression conservés.

**Priorité : A01 reproduit une perte du contenu d'une source lors d'un déplacement de fichiers homonymes.** L'absence de collision entre sources ne garantit pas l'absence de collision entre destinations calculées.

La compilation normale est propre et **1 057 tests passent** sur les suites effectivement exécutées hors du bac à sable : 772 backend, 256 interface, 29 intégration du parcours de production des archives. Il reste 21 tests ignorés dans ces suites. En parallèle, trois autres suites d'intégration ne compilent plus : le projet n'a donc pas une validation de toutes ses cibles réussie.

Le dépôt contient 455 fichiers Rust sous `src/`, environ 112 104 lignes, tests inclus. L'audit couvre transversalement les domaines ci-dessous, avec approfondissement des parcours à risque. Il ne s'agit pas d'une lecture exhaustive de chaque ligne ni d'une preuve d'absence de défaut dans les parties partiellement examinées. La matrice finale précise la profondeur et les limites.

## Méthode et architecture examinée

Lecture des consignes `AGENTS.md`, de `UI_ARCHITECTURE.md`, de l'audit des fenêtres transitoires et du diagnostic de fermeture PDF/Intel. Consultation du reliquat de sécurité et des rapports R1/R2/R3 et R4/R6/R7/R8/R9 pour distinguer les mécanismes actuels des limites déjà documentées. Les résultats historiques ne sont pas présentés comme des validations de cette passe.

Architecture relevée :

- `main.rs` initialise stockage, logs, Tokio et GPUI ; les modes internes de workers sont interceptés avant la session applicative. Les hooks sauvegardent l'état avant l'arrêt borné de Tokio et la sortie finale Windows.
- `ui/app/` possède les parcours et l'état ; `ui/workspace/` les panneaux, onglets, filtres et instantanés ; les composants GPUI portent rendu et interactions locales.
- `commands/` expose opérations, recherche, archives, propriétés et commandes système ; `systems/` porte planification, journaux, transferts, workspaces et récupération.
- `platform/` concentre les comportements natifs ; Windows possède des modules de sélection par handles, copie, renommage, corbeille, fenêtres et confinement. Linux dispose de replis et de fonctionnalités explicitement indisponibles.
- Les quatre exécutables applicatifs sont `oxyde`, `oxyde-image-worker`, `oxyde-archive-list-worker`, `oxyde-archive-io-worker`. `copy_benchmark` est un outil supplémentaire, exclu du payload applicatif par le script de distribution.
- Les archives Windows de production passent par le superviseur et les workers ; plusieurs tests unitaires utilisent des implémentations locales sous `cfg(test)`. La suite `archive_io_pipeline` vérifie séparément le chemin de production.

Parcours suivis : sélection/copier-couper/coller → préparation des conflits → planification → publication → journal ; navigation → listing → application par identifiant d'onglet ; recherche → flux borné → tri/filtre → rendu ; survol/grille → admission → workers → caches ; fin des opérations → affichage/persistance ; fermeture → sauvegardes → arrêt des superviseurs.

Les constats ci-dessous distinguent reproduction, démonstration par le code et conséquences restant à mesurer. P1 signifie correction prioritaire pour protéger les données ; P2 un défaut fonctionnel ou de validation ; P3 une incohérence documentaire. Les gains de performances ne sont pas chiffrés sans mesure.

## Constats par priorité

### A01 — P1 — Deux sources homonymes peuvent être déplacées vers le même fichier avec perte d'une source

**Suivi de correction :** implémentée sur `codex/a01-transfer-destination-collisions`. Le plan complet réserve maintenant les noms des fichiers et dossiers avant tout effet de transfert (`src/systems/file_ops/copy/destinations.rs`). Les fusions de dossiers disjoints et les résolutions Ignorer/Remplacer existantes sont conservées ; une collision interne arrête la préparation et affiche un message traduit dans les cinq langues. Sous Windows, les règles de casse du dossier sont interrogées et les parents existants sont rapprochés par leur identité. La validation détaillée de cette correction est consignée en fin de rapport ; les observations ci-dessous décrivent le défaut initial.

**Catégorie / confiance :** intégrité des données ; bug confirmé par exécution du backend de production, confiance élevée.

**Références :** `src/commands/ops.rs:203`, `:207` ; `src/systems/file_ops/copy.rs:199`, `:213`, `:245`, `:313`, `:420`. Le parcours UI collecte les chemins sélectionnés dans `src/ui/app/clipboard.rs:34` puis prépare les conflits dans `src/ui/app/file_operations.rs`.

**Déclencheur :** sélectionner `a/report.txt` et `b/report.txt`, par exemple dans des résultats de recherche, puis les déplacer vers `target/`, où `report.txt` existe déjà ; choisir Remplacer pour les deux conflits.

**Reproduction :** trois contenus différents ont été créés dans une fixture privée. `check_conflicts` renvoie deux conflits contre la destination initiale. `perform_copy(..., is_move=true, Replace pour les deux)` renvoie `Ok(())`. Les deux sources disparaissent, le journal contient deux déplacements vers la même destination, et celle-ci ne contient que `source B unique`. Le contenu de A n'existe plus dans ces emplacements. Les sources et destinations réelles de l'utilisateur n'ont pas été utilisées.

**Cause :** `SourcePathSet` rejette les sources qui se recouvrent, mais les destinations ne font pas l'objet d'une réservation équivalente pour l'ensemble du plan. `replace_existing` est calculé avant les effets : les deux tâches peuvent donc remplacer le même nom. La protection contre les mêmes noms d'origine ne concerne que les entrées de corbeille. La réservation des noms de copie concerne les copies sur place, pas ce cas général.

**Attendu / impact :** chaque source déplacée doit rester récupérable et chaque remplacement doit viser l'objet explicitement présenté au choix utilisateur. Le deuxième déplacement ne doit pas écraser silencieusement le résultat du premier. Annuler ne peut pas recréer les octets de A à partir des deux références au même chemin final.

**Correction minimale proposée :** vérifier l'unicité des destinations de fichiers sur tout le plan, avant toute création/suppression/publication, avec les règles d'identité et de casse de la cible. Détecter aussi les collisions entre descendants de racines distinctes. Une collision interne doit donner une décision explicite ou faire échouer la préparation sans effet ; une autorisation de remplacer le fichier initial ne vaut pas autorisation de remplacer une autre source du lot.

**Régression / validation :** mêmes noms, casse différente sur volume Windows insensible à la casse, dossiers fusionnés avec descendants homonymes, cible initialement présente/absente, copie et déplacement, ordre inversé, modes normal/turbo, annulation et Undo. Vérifier les octets et les journaux, pas seulement le statut final. Ne pas modifier la politique existante de remplacement explicite d'un fichier indépendant.

### A02 — P2 — Trois suites d'intégration ne compilent plus

**Suivi de correction :** traitée sur `codex/a02-worker-integration-tests`, issue du commit A01 `1459b66`. Les trois suites importent désormais `platform::worker_test_support` dans la bibliothèque compilée : leurs superviseurs, verrous de lancement et budgets sont ceux de la production. Les implémentations natives restent privées ; les façades artificielles `commands`/`utils`/`app_paths` et les inclusions de leurs sources disparaissent des harnesses. Les 25 scénarios propres à ces suites sont conservés. Le script `scripts/test-windows.ps1` et le workflow `.github/workflows/tests.yml` couvrent compilation de toutes les cibles, construction explicite des exécutables et tests Windows natifs. Les observations ci-dessous décrivent le défaut initial ; les résultats actuels sont consignés en fin de rapport.

**Catégorie / confiance :** validation ; confirmé par compilation, confiance élevée.

**Références :** `src/platform/windows_preview_worker.rs:22`, `:23`, `:158`, `:180` ; `tests/preview_worker.rs:7`, `:83` ; `tests/archive_listing_worker.rs:2`, `:16` ; `tests/archive_io_worker.rs:11`, `:24`, `:428` ; `src/platform/windows_worker_sandbox.rs:106`.

**Déclencheur / résultat :** `cargo check --offline --locked --all-targets` échoue pour `preview_worker`, `archive_listing_worker`, `archive_io_worker` avec E0583, E0433 et E0425. Les enfants `image_session` et `icon_session` sont cherchés au mauvais emplacement lorsque le module de production est inclus avec `#[path]`. Les façades locales ne réexportent pas tous les nouveaux modules `commands`/`app_paths`. Deux suites appellent encore `run_image_in`, absent du module actuel.

**Attendu / impact :** toutes les suites doivent compiler pour vérifier réellement les frontières des workers et les aperçus PDF. La réussite des tests unitaires n'exerce pas ces suites et ne compense pas leur absence. Les workflows inspectés ne lancent pas actuellement un ensemble complet de tests à chaque modification Rust : le workflow sécurité filtre surtout les dépendances ; le workflow distribution manuel vérifie la compilation normale et le stockage portable.

**Correction proposée :** remettre les harnesses en cohérence avec les sessions actuelles, expliciter les chemins des sous-modules si l'inclusion est conservée, puis choisir une interface de test ciblée qui évite de reconstruire une fausse racine de crate à chaque évolution. Garder la hiérarchie logique et les visibilités de production existantes. Ajouter une vérification automatique couvrant les cibles et tests pertinents.

**Validation :** `cargo check --offline --locked --all-targets`, puis exécution non vide des trois suites sous Windows avec TEMP/TMP privés et appels natifs autorisés. Ne pas résoudre le problème en supprimant ou en ignorant les scénarios concernés.

### A03 — P2 — Le cache GPUI des miniatures de grille n'est ni invalidé lors des changements ni borné par leur cycle de vie

**Catégorie / confiance :** affichage obsolète et rétention mémoire ; établi par le code, confiance élevée ; effets visuels et RSS non mesurés.

**Références :** `src/ui/components/file_list/thumbnails.rs:7`, `:13`, `:42` ; `src/ui/app/preferences.rs:91` ; application des listings dans `src/ui/app/navigation.rs:550`. Contrat vérifié dans la dépendance GPUI réellement épinglée, `crates/gpui/src/window.rs:3974`, `app.rs:2785` et `asset_cache.rs:17`.

**Déclencheurs :** afficher une image en grille, modifier/remplacer ce même chemin puis actualiser ; ou parcourir successivement de nombreux fichiers différents pendant une longue session.

**Cause / résultat déduit :** la source GPUI est uniquement le chemin. GPUI conserve le résultat, succès ou erreur, jusqu'à éviction explicite. Le seul appel applicatif trouvé à `invalidate_thumbnail` est l'effacement manuel du cache, limité aux fichiers des onglets présents à cet instant. Le listing et les watchers ne retirent pas les assets modifiés/supprimés, et quitter un dossier ne les libère pas. La signature taille/date du cache disque n'est consultée qu'en cas de nouvel appel à `load_grid` : elle ne corrige pas un hit dans le cache GPUI.

**Impact :** miniature ancienne ou échec conservé malgré la modification du fichier ; accumulation des pixels au fil de la navigation. À titre de dimensionnement, 10 000 miniatures RGBA de 128 × 128 représentent 625 Mio de pixels bruts, hors GPU et structures. C'est un calcul de volume, pas une mesure de consommation observée. Le budget des infobulles ne couvre volontairement pas ces miniatures.

**Correction proposée :** invalidation à partir de la révision des fichiers obtenue en arrière-plan, et politique d'éviction bornée des assets de grille, respectant les usages simultanés de plusieurs panneaux. Réutiliser la logique de propriété/libération des assets d'infobulles lorsqu'elle correspond au besoin. Ne pas faire de `metadata` synchrone par carte au rendu.

**Validation :** modifier une image visible, remplacer après erreur, supprimer/recréer le même chemin, ouvrir le même fichier dans deux panneaux et parcourir plusieurs milliers de miniatures. Vérifier pixels actualisés, éviction, résultats tardifs, retour dans le dossier et stabilisation mémoire. Essai visuel encore nécessaire.

### A04 — P2 — Les résultats d'archives peuvent contourner les critères de recherche avancée

**Catégorie / confiance :** exactitude des résultats ; reproduction confirmée, confiance élevée.

**Références :** `src/commands/search.rs:214`, `:217`, `:436`, `:473`. Tests actuels séparés dans `search/tests/quick_search_tests.rs` et `archive_search_tests.rs`.

**Déclencheur :** recherche `query=needle`, `content=ABSENT_MARKER`, `min_size=10`, `archives=true`, sur un ZIP contenant `needle.txt` dont le contenu est `no match here` et le dossier `needle_folder/`.

**Observé :** les deux entrées sont renvoyées, alors qu'aucune ne satisfait la recherche de contenu et que le dossier ne satisfait pas le contrat appliqué aux dossiers ordinaires avec ces critères.

**Cause :** la branche filesystem applique le contenu et écarte les dossiers avec filtre de taille/contenu ; `SearchParams` ne transporte pas le filtre de contenu et `search_archive` réinjecte directement ses résultats. Les dossiers archivés passent aussi les bornes de taille. Les anciens choix de lecture des métadonnées n'établissent pas que la combinaison actuelle de critères a été vérifiée.

**Impact / attendu :** un résultat affiché comme correspondant doit satisfaire les critères applicables, avec les limites explicitement indiquées. De faux positifs sont particulièrement gênants lorsque l'utilisateur agit sur la sélection.

**Correction proposée :** partager le contrat de prédicats entre les deux parcours. Si le contenu interne n'est pas recherché dans une archive, signaler cette limite et ne pas compter ces entrées comme des correspondances au filtre de contenu ; conserver la recherche des noms/métadonnées sans ce filtre. Une éventuelle prise en charge du contenu doit respecter l'isolation existante.

**Validation :** combinaisons contenu littéral/regex, minimum/maximum, dossiers/fichiers et formats ; comparer les mêmes fixtures sous forme ordinaire et archivée. Ne pas inventer des dates pour ISO/7z lorsque les lecteurs n'en fournissent pas.

### A05 — P2 — La recherche n'applique pas les mêmes règles de visibilité que le listing

**Catégorie / confiance :** résultats manquants ; deux reproductions confirmées, confiance élevée.

**Références :** `src/commands/search.rs:154`, `:161`, `:292`, `:304`, `:317` ; contrat du listing dans `src/commands/io.rs:105`.

**Reproductions :**

1. Un dossier utilisateur ordinaire nommé `Windows` contient `needle.txt`. Avec `show_system=false`, la recherche rapide renvoie zéro résultat ; la recherche avancée récursive trouve le fichier. Le dossier n'a pas l'attribut système.
2. Un fichier temporaire possède les attributs Hidden et System. Avec `show_system=true` et `show_hidden=false`, le listing l'affiche, mais la recherche avancée l'exclut.

**Cause :** la recherche rapide exclut certaines branches selon leur nom seul, quelle que soit leur localisation ; les recherches combinent ensuite hidden et system avec un ET. Le listing donne explicitement priorité à l'attribut system, auquel s'applique uniquement `show_system`.

**Impact / attendu :** le même réglage ne doit pas produire des omissions silencieuses selon le parcours. Un nom de dossier utilisateur ne doit pas devenir un attribut système implicite.

**Correction proposée :** une fonction de décision de visibilité commune et testable ; si des exclusions de branches système sont nécessaires, identifier précisément leur emplacement/rôle au lieu d'exclure tous les noms identiques. Préserver le comportement documenté du listing.

**Validation :** les quatre combinaisons de réglages, fichiers Hidden/System/les deux/aucun, dossiers homonymes utilisateurs et véritables branches système, recherche rapide et avancée.

### A06 — P2 — Le renommage qui change seulement la casse est refusé sous Windows

**Catégorie / confiance :** fonctionnalité native ; confirmé par appel public, confiance élevée.

**Références :** `src/commands/io.rs:257` ; `src/systems/item_rename.rs:44`.

**Reproduction :** `photo.txt` → `Photo.txt` dans une fixture sur le volume Windows de cette passe. Résultat : `Rename target already exists`, source conservée.

**Cause / attendu :** le chemin cible désigne la source elle-même sur un dossier insensible à la casse. Les deux validations assimilent cette situation à une cible indépendante existante. Retirer seulement la première vérification ne suffit pas : le helper natif effectue un second refus.

**Impact :** impossibilité de corriger la capitalisation d'un nom avec le parcours de renommage normal.

**Correction proposée :** distinguer une cible indépendante de l'objet source retenu et traiter le changement de casse avec le renommage natif approprié. Conserver l'interdiction de remplacer un autre objet, y compris lors d'une course. Ne pas introduire un détour de renommage temporaire non protégé.

**Validation :** fichier/dossier, nom strictement identique, changement de casse, cible indépendante, dossier Windows sensible à la casse, reparse points et Undo/Redo du renommage.

### A07 — P2 — Un vrai dossier dont le nom finit par une extension d'archive est interprété comme une archive

**Suivi de correction :** implémentée sur `codex/a07-real-and-archive-paths`, issue du commit A02 `e7f5f53`. Le résolveur commun cherche le fichier archive réel en ignorant les dossiers à extension d'archive. Les listings et recherches portent l'origine dans `FileEntry::archive_path`, partagée par `Arc` au sein d'un listing. L'interface utilise ces faits pour les actions et dépôts, sans requête disque pendant le rendu. Le contexte d'archive des onglets est résolu en arrière-plan avec le listing et ses protections contre les résultats obsolètes ; les watchers observent le vrai dossier ou le parent de l'archive physique. Les détails et validations sont consignés en fin de rapport ; les observations suivantes décrivent le défaut initial.

**Catégorie / confiance :** classification des chemins ; suppression reproduite, autres conséquences établies par lecture, confiance élevée.

**Références :** `src/utils/archive.rs:52` ; `src/systems/file_ops/delete.rs:30`, `:245` ; `src/ui/app/archives.rs:126`, `:136`. Comparer avec `archive_workspace::virtual_source` et `virtual_destination`, qui vérifient `archive.is_file()`.

**Reproduction :** créer le vrai dossier `ordinary.zip/` avec `plain.txt`, puis demander la suppression permanente de ce seul fichier temporaire via `perform_delete`. Le chemin est classé comme membre d'archive ; l'opération échoue avec Accès refusé et le fichier reste présent. Ce n'est pas un refus du bac à sable : la reproduction a aussi été exécutée hors de celui-ci.

**Cause :** `split_virtual_path` est lexical et retient le plus haut ancêtre à extension reconnue. Certains appelants l'utilisent directement pour décider du parcours métier, alors que d'autres confirment qu'il s'agit réellement d'un fichier. Les filtres de préparation de compression/extraction écartent également les chemins classés comme internes sans vérifier cet objet.

**Impact / attendu :** un dossier ordinaire portant un point dans son nom doit garder les opérations d'un dossier. La classification peut aussi masquer une archive réelle située plus bas dans cette arborescence.

**Correction proposée :** distinguer le découpage lexical de la résolution métier d'une archive, puis faire converger les appelants vers un contrat commun fondé sur l'objet réel. Résoudre cette information en arrière-plan lorsque le parcours est déclenché depuis l'UI.

**Validation :** dossiers `.zip`, `.7z`, `.tar.gz`, fichiers et sous-dossiers internes, archive réelle sous un dossier ainsi nommé, copie/déplacement/suppression/corbeille/compression/extraction. Vérifier aussi les watchers de ces onglets.

### A08 — P2 — Le graphe de vitesse disparaît à la fin d'une opération

**Catégorie / confiance :** incohérence entre exécution, interface et persistance ; établi directement par le code, confiance élevée ; pas d'essai visuel effectué.

**Références :** `src/systems/file_ops/execution.rs:83` ; `src/systems/file_ops/persistence.rs:81` ; `src/ui/components/operations/card/metrics.rs:135` ; contrat dans `docs/UI_ARCHITECTURE.md`, section Organisation.

**Déclencheur / résultat déduit :** une opération qui a accumulé des échantillons atteint son état final. `execute_operation` appelle immédiatement `locked.speed_history.clear()`. Le rendu n'affiche le graphe que si cette liste n'est pas vide ; la persistance sérialise alors une liste vide. Le contrat documentaire indique pourtant que le graphe reste consultable dans l'historique.

**Impact :** perte de l'historique de vitesse à la fin et après redémarrage. Les tests actuels vérifient l'alimentation et la borne des échantillons, pas leur conservation après ce parcours complet.

**Correction proposée :** conserver la liste déjà bornée aux 60 échantillons ; remettre à zéro uniquement les indicateurs de vitesse instantanée nécessaires.

**Validation :** fin normale, annulation, erreur après progression, persistance/rechargement et effacement d'une opération. Un test doit passer par la finalisation réelle du gestionnaire.

### A09 — P2 — La recherche de contenu peut allouer une ligne entière sans borne

**Catégorie / confiance :** robustesse mémoire ; allocation reproduite, épuisement mémoire non provoqué.

**Références :** `src/commands/search.rs:528`, `:557`, `:745`, `:755`.

**Déclencheur :** grand texte sans retours à la ligne, par exemple JSON minifié, export ou fichier généré, avec recherche de contenu ne trouvant pas de correspondance.

**Mesure bornée :** une fixture de 16 777 216 octets ASCII sans retour à la ligne a été parcourue par `advanced_search`. Une instrumentation de l'allocateur de la sonde mesure une allocation individuelle maximale de **16 777 216 octets** pendant cette recherche ; zéro résultat, comme attendu pour le contenu. Cette valeur mesure une allocation, pas le RSS du processus.

**Cause / impact :** limiter chaque `Read` à 64 Kio permet les checkpoints d'annulation mais ne borne pas le `String` assemblé par `BufRead::lines`. La mémoire croît avec la plus longue ligne, dans le processus principal. L'absence de NUL dans les premiers 1 024 octets suffit pour entrer dans ce parcours. De très grandes lignes peuvent donc provoquer une forte pression mémoire, voire un échec d'allocation ; ce dernier n'a pas été reproduit volontairement.

**Correction proposée :** étudier une recherche incrémentale ou sur vue de fichier adaptée, avec équivalence du comportement actuel, et des allocations fallibles lorsque possible. Préserver accents, UTF-8, ancres et expressions régulières. Un simple chevauchement fixe de blocs n'est pas une solution générale pour des regex de longueur non bornée. Ne pas corriger en tronquant silencieusement les textes.

**Validation :** croissance mémoire pour lignes de tailles croissantes, correspondance en fin et frontière de blocs, accents/Unicode, regex longues, annulation pendant lecture et erreur d'encodage. Le gain exact et la stratégie générale restent à établir.

### A10 — P3 — Le guide d'architecture décrit encore l'extraction/reconstruction comme locale sous Windows

**Catégorie / confiance :** documentation ; incohérence confirmée, confiance élevée.

**Références :** `docs/UI_ARCHITECTURE.md:282` ; `src/systems/archive_workspace.rs:21`, `:24`, `:191` ; `src/systems/archive_workspace/native_commit.rs` ; rapports de sécurité du 13 octobre.

**Observé / cause :** le paragraphe Archives natives se termine par « Extraction et reconstruction restent locales ». Le code Windows de production effectue le décodage via `archive_io::decode` et prépare la reconstruction native ; la publication reste dans le parent. Les variantes locales existent notamment sous `cfg(test)` et sur Linux. Le document mélange l'ancien état et l'état actuel.

**Impact / correction :** une maintenance fondée sur ce paragraphe peut modifier le mauvais chemin ou surestimer la portée des tests unitaires. Actualiser le guide en distinguant codecs isolés, décisions/publication dans le parent et variantes de test/Linux. Aucun changement de fonctionnalité requis.

**Validation :** vérifier les descriptions contre les conditions de compilation et les 29 tests de production. Ne pas déclarer fermés les reliquats R1/R9 au seul motif de cette mise à jour.

## Optimisations proposées sans changement fonctionnel

Ces points correspondent à des coûts identifiables dans le code. Leur gain final n'a pas été benchmarké ; ils ne sont pas présentés comme des accélérations déjà démontrées.

| ID | Coût actuel et références | Proposition et mesure utile | Risque à contrôler |
| --- | --- | --- | --- |
| O01 | Chaque rendu Détails/Grille clone tous les `FileEntry` filtrés, clone la carte globale `folder_has_subdirs`, puis reconstruit les lignes : `components/file_list/details.rs:34`, `:38`, `:128`, `grid.rs:29`, `:43`, `:58`. Une opération active notifie la vue toutes les 200 ms (`app/file_operations.rs:222`), même en pause. La virtualisation des lignes ne supprime pas ce coût préalable. | Instantanés immuables partagés par révision et cache de géométrie indexé par révision/colonnes/groupement ; limiter les notifications aux changements visibles. Mesurer temps de rendu, allocations et latence de sélection sur 20 000–100 000 entrées, plusieurs panneaux, opération active et pause. | Préserver les identifiants GPUI, les closures des lignes, la sélection, les groupes et les mises à jour concurrentes. Éviter une abstraction générale de toutes les vues. |
| O02 | Le listing lit les métadonnées puis appelle `get_file_entry_from_path`, qui les relit ; il trie naturellement avant que `navigation.rs:556` retrie selon l'onglet. Les lots de recherche retrient et reclonent la totalité des résultats à chaque application (`search.rs:902`, `workspace/filters.rs:147`). | Préparer les entrées à partir des métadonnées déjà obtenues ; distinguer listing brut et contrat public trié ; préparer le tri/filtre en arrière-plan avec révision. Mesurer appels de métadonnées, CPU UI et délai jusqu'au premier affichage sur disque local et partage lent. | Ne pas supprimer le tri attendu par les autres appelants. Préserver l'ordre lexical actuel de l'UI et les égalités stables testées ; un passage au tri naturel visible serait une décision fonctionnelle distincte. |
| O03 | `commands/ops.rs:226` recalcule la taille de chaque sous-arbre source et cible avec WalkDir puis récursive sur les conflits enfants (`:285`). Une chaîne de dossiers conflictuels de profondeur d revisite ses descendants à chacun des d niveaux. La vérification est répétée à la sortie de la file. | Collecter une fois les informations par arbre pour la présentation des conflits et agréger les totaux ; garder les revérifications d'identité/conflit nécessaires au lancement. Mesurer visites d'entrées et durée avec arbres profonds et larges, dont SMB. | Ne pas supprimer les contrôles contre une modification de destination pendant l'attente. Les liens et les erreurs d'accès doivent conserver le contrat de sécurité. |
| O04 | Le champ de navigation appelle `Path::is_dir` synchroniquement (`app/navigation.rs:99`), et le rendu du fantôme de glissement le refait (`app/drag.rs:99`). Un partage ou support lent peut bloquer le thread UI. Le blocage réel sur réseau indisponible n'a pas été mesuré. | Valider la navigation en arrière-plan avec une révision de saisie et récupérer `is_dir` depuis l'entrée déjà sélectionnée pour le glissement. Mesurer les latences et la disponibilité d'un battement UI avec un backend de test lent. | Préserver l'affichage de chemin invalide, les serveurs UNC et le rejet des résultats tardifs après nouvelle saisie ou changement d'onglet. Aucun changement de curseur. |

Factorisation utile : un contrat commun pour visibilité (A05) et résolution d'archives (A07), une vérification des destinations dans le plan de transfert (A01), et une responsabilité de cache de grille clairement identifiée (A03). Aucun découpage général de gros modules n'est justifié par cette passe. Les réexportations et portées privées doivent être conservées.

## Vérifications et preuves

| Commande / configuration | Résultat de cette passe | Limite |
| --- | --- | --- |
| `cargo check --offline --locked` | Réussite, sans avertissement du projet. | Compilation de la configuration Windows normale. |
| `cargo check --offline --locked --all-targets` | Échec des trois suites listées dans A02. | Aucune affirmation de réussite all-targets. |
| `cargo build --offline --locked --lib` | Réussite ; bibliothèque courante utilisée pour la sonde. | Pas une construction de distribution. |
| `cargo test --offline --locked --lib -- --test-threads=1 --quiet`, hors bac à sable, TEMP/TMP privés | **772 réussis, 20 ignorés, 0 échec**, 792 découverts ; 75,36 s. | Les branches sous `cfg(test)` ne couvrent pas seules le chemin archives de production. |
| `cargo test --offline --locked --bin oxyde -- --test-threads=1 --quiet`, même configuration | **256 réussis, 1 ignoré, 0 échec**, 257 découverts ; 4,79 s. | Tests unitaires, aucune validation visuelle de la fenêtre principale. |
| `cargo test --offline --locked --test archive_io_pipeline -- --test-threads=1 --quiet`, même configuration | **29 réussis, 0 ignoré, 0 échec** ; 131,78 s. | Vérifie les workers et effets filesystem de ce pipeline, pas les trois suites cassées. |
| `cargo check --offline --locked --target x86_64-unknown-linux-gnu --lib` | Bloqué dans le build script de `yeslogic-fontconfig-sys` : pkg-config non configuré pour la compilation croisée. | Échec de préparation de l'environnement, pas un défaut Rust Linux établi. Ni compilation projet Linux complète ni exécution Linux validées. |
| Sonde native liée à la bibliothèque courante, fixtures privées | A01, A04, A05 (deux cas), A06, A07 reproduits ; allocation de A09 mesurée. | Tests backend ciblés ; pas de reproduction par clics dans l'UI. |

Configuration PowerShell des tests natifs, limitée au processus de commande :

```powershell
New-Item -ItemType Directory -Path target/audit-2026-10-16/tmp -Force | Out-Null
$env:TEMP = (Resolve-Path target/audit-2026-10-16/tmp).Path
$env:TMP = $env:TEMP
cargo test --offline --locked --lib -- --test-threads=1 --quiet
cargo test --offline --locked --bin oxyde -- --test-threads=1 --quiet
cargo test --offline --locked --test archive_io_pipeline -- --test-threads=1 --quiet
```

Les deux premières tentatives de la suite bibliothèque dans le bac à sable ont été interrompues après des refus d'accès et de handles natifs ; elles ne sont pas comptées comme des défauts applicatifs. Rediriger TEMP/TMP résout certains refus, mais ne suffit pas pour les appels natifs concernés. Les résultats réussis ci-dessus proviennent des exécutions ensuite autorisées hors du bac à sable. Une autorisation de sonde a expiré dans le contrôle automatique ; la tentative suivante a été acceptée et exécutée avec succès.

Journaux temporaires : `target/audit-2026-10-16/lib-tests-native.log`, `ui-tests-native.log`, `pipeline-tests-native.log`, `probe-native.log`. Le dernier contient la première reproduction des six scénarios fonctionnels ; la sonde finale instrumentée a également imprimé la mesure de 16 Mio décrite dans A09. Source temporaire : `target/audit-2026-10-16/probe.rs`, exécutable : `target/debug/audit_20261016.exe`.

La sonde a été compilée avec `rustc --edition=2021`, les `.rlib` courantes de `oxyde_lib`, `tokio`, `tempfile`, `zip` dans `target/debug/deps`, les chemins natifs des build scripts Cargo et la bibliothèque Windows x64 disponible localement. Elle appelle les API publiques et contient des assertions sur les comportements constatés. Ces assertions démontrent les défauts présents ; elles doivent être inversées/adaptées lors de la création des futurs tests de non-régression dans les dossiers `tests/` des modules concernés.

## Matrice de couverture

« Approfondi » signifie suivi des appelants, contrôles et effets pour les parcours nommés, et non relecture de tous les fichiers du domaine. Les tests exécutés sont une couverture supplémentaire, distincte de la revue humaine du code.

| Domaine | Profondeur de lecture | Validation / reste à examiner |
| --- | --- | --- |
| Entrée, stockage portable, sauvegarde des préférences, arrêt | Approfondi sur `main.rs`, `app_paths`, chaîne de sauvegarde et hook de sortie ; partiel sur géométrie de chaque outil | Tests stockage et fermeture inclus ; pas de nouvelle reproduction du problème pilote PDF/Intel ni d'installation propre. |
| Navigation, onglets, identifiants, listings, watchers | Approfondi sur requêtes et application des résultats, parcours principaux des watchers ; partiel sur toutes les interactions de layout | Identifiant d'onglet et numéro de requête protègent le listing principal contre les retours obsolètes. Pas de parcours manuel multiécran/lecteur débranché. |
| Transferts, conflits, planification, publication | Approfondi sur préparation UI, plan de copie/déplacement, worker, réservations et identités | A01 reproduit ; tests backend passants. Les scénarios de second volume ignorés restent non vérifiés sur cette machine. |
| Historique Undo/Redo, corbeille, effets partiels | Partiel sur détails des implémentations, approfondi sur réservation de l'historique et lien avec les transferts | Tests concernés exécutés ; pas de manipulation de la corbeille personnelle. Revue de chaque branche native de restauration non exhaustive. |
| Recherche rapide/avancée, filtres, contenu | Approfondi sur les deux traversées, combinaisons de filtres, flux et application UI | A04/A05/A09 reproduits ; détails de tous les formats Office et encodages non entièrement relus. |
| Doublons | Lecture du scan, déduplication des racines, phases de hash, annulation et filtrage UI | Les appelants normalisent déjà la requête en minuscules : aucune anomalie de casse retenue pour ce filtre. Courses de modification externe pendant les phases de hash non reproduites. |
| Archives : codecs, protocoles, workspaces, rollback, récupération | Approfondi sur classification et frontières parent/worker, sélection/publication ; partiel sur chaque codec et totalité de la récupération après crash | 29 tests de production passants, dont formats, rollback groupé, échecs injectés et fidélité TAR. A07 reproduit ; suites listing/I/O dédiées bloquées par A02. |
| Images, PDF, vidéo, icônes, caches | Approfondi sur admission, sessions et politique de cache des miniatures/infobulles ; partiel sur codecs et détails Media Foundation/Shell | A03 établi par le contrat GPUI. Tests de sessions unitaires passants ; suite preview dédiée non compilable ; pas de QA visuelle ou mesure GPU. |
| Fenêtres transitoires, clavier, champs et propriétaires | Lecture des façades, contrats, transport, focus et libération ; partielle sur chaque callback de fenêtre | Tests UI passants ; clavier réel, DPI, zoom, focus interapplication, cinq langues et thèmes restent à tester manuellement. |
| Préférences, thèmes, i18n, dates, composants partagés | Lecture des contrats, filtres/caches et points de changement ; partielle sur chaque rendu et catalogue | Tests unitaires inclus ; pas d'audit visuel exhaustif de chaque texte/couleur/icône SVG. |
| Réseau, lecteurs, montage, propriétés, Shell | Partiel : parcours UI, révisions, délégation plateforme, gestion des erreurs | Pas de connexion/déconnexion réseau, montage ou éjection d'un support réel ; pas de revue exhaustive COM/WMI/Win32. |
| Plateforme Windows : sécurité et FFI | Partiel : rétention, ouverture relative, copie native, renommage, lancement, budgets et protocoles inspectés | Tests natifs passants sur les suites compilables. Aucun nouvel argument de fermeture générale des reliquats R1–R9. |
| Plateforme Linux | Inspection statique partielle des replis et sélection par descripteurs | Compilation complète et runtime non vérifiés ; pas de changement de portée du reliquat Linux historique. |
| Dépendances, CI, distribution, scripts | Manifest/lock et workflows, lecture du script principal de distribution | Pas de nouveau scan RustSec actualisé, de revue des sources de toutes les dépendances, de build release/installeur ou d'installation vierge. Les anciens résultats RustSec restent historiques. |

Code tiers, sources de référence Zed, ressources binaires et CSV utilisateur ne font pas l'objet d'un audit exhaustif. La source GPUI épinglée a été consultée uniquement pour établir le contrat de cache utilisé par Oxyde. La revue partielle des API Windows ne permet pas de conclure à l'absence de défaut FFI dans chaque fonction `unsafe`.

## Garanties observées et limites de sécurité conservées

Les mécanismes vérifiés méritent d'être conservés lors des corrections : sélection des objets et rétention des parents, refus des chemins d'archive dangereux, création exclusive, publication contrôlée, journaux des effets réels, réservation Undo/Redo sans I/O sous le mutex global, numéros de requêtes pour les listings, admission bornée gardée jusqu'à la fin physique des workers et callbacks de copie protégés contre l'unwind à travers FFI.

Les rapports de sécurité documentent encore les modes natifs non confinés au même niveau que les codecs LPAC, les exceptions ACL retenues, les limites des preuves de fidélité hors TAR, et les différences Linux. La présente passe ne les redécouvre pas comme de nouveaux bugs, ne les clôture pas et ne déduit aucune autorisation de supprimer des fonctionnalités. La sortie finale PDF/Intel documentée reste à préserver.

## Plan d'action proposé

Les efforts sont relatifs : faible = correction locale ; moyen = plusieurs points d'un même parcours ; élevé = contrat partagé ou validation native plus large. Ils ne constituent pas des estimations calendaires.

| Ordre | Travaux | Bénéfice | Effort | Risque de régression |
| --- | --- | --- | --- | --- |
| 1 | A01 : collisions de destinations et tests de préservation | Prévenir une perte de données reproduite | Moyen | Moyen : fusions et résolutions existantes à préserver |
| 2 | A02 : remettre les trois harnesses en état et couvrir les tests en CI | Retrouver les preuves manquantes avant les corrections suivantes | Moyen | Faible à moyen : préserver modules et conditions de compilation |
| 3 | A07 : contrat de chemins réels/virtuels | Rétablir les opérations ordinaires sous dossiers à suffixe d'archive | Moyen | Moyen : navigation et mutations d'archives |
| 4 | A04/A05 : critères et visibilité partagés | Résultats de recherche fiables | Moyen | Moyen : combinaisons de critères et formats |
| 5 | A03 : invalidation et éviction des miniatures | Affichage actuel et mémoire maîtrisée | Moyen à élevé | Moyen : assets partagés, retours tardifs et rendu |
| 6 | A06 et A08 : casse des noms et conservation des graphes | Corriger deux incohérences locales visibles | Faible à moyen | Renommage : moyen ; graphe : faible |
| 7 | A09 : stratégie de recherche sur lignes très longues | Réduire le risque de pression mémoire sans tronquer les résultats | Élevé | Élevé : sémantique regex/Unicode à conserver |
| 8 | O01–O04, après mesures ciblées | Réactivité des gros dossiers et supports lents | Variable | Moyen : révisions d'état et comportements UI |
| 9 | A10 : synchroniser le guide d'architecture | Maintenance et validation mieux orientées | Faible | Faible |

Pour chaque correction, ajouter des tests significatifs dans le sous-répertoire `tests/` du module concerné conformément à AGENTS.md. Garder les corrections séparées des optimisations mesurées et des éventuelles extractions locales nécessaires.

**Workers :** aucun code de worker ou code partagé n'a été modifié par cet audit ; il ne nécessite donc pas de recompilation pour appliquer un changement applicatif. Pour les corrections futures qui toucheraient les workers ou leur code partagé, fermer Oxyde puis exécuter `cargo build --offline --bins`, ensuite `cargo run --offline`. `cargo run` seul ne reconstruit pas l'ensemble des exécutables auxiliaires.

## Correction A01 — validation et limites

Les tests ciblés vérifient les octets des sources et des destinations, les journaux d'effets et l'absence d'entrée Undo lors du refus. Ils couvrent copie/déplacement, ordre des sources, modes normal/turbo, cible présente/absente, collisions de descendants et fichier/dossier, noms Unicode et différences de casse. Les tests natifs Windows vérifient aussi qu'un dossier sensible à la casse accepte deux noms distincts. Les tests d'intégration `transfer_destination_collisions` exercent la bibliothèque de production et les transferts vers/depuis un ZIP via le worker natif.

Vérifications de la branche, avec `--offline --locked` : `cargo check` sans avertissement ; suite `--lib` : **781 réussis, 20 ignorés** ; suite `--bin oxyde` : **257 réussis, 1 ignoré** ; nouvelle suite `--test transfer_destination_collisions` : **3 réussis**. Les exécutions natives ont été réalisées hors bac à sable, avec TEMP/TMP privés sous `target/a01-validation/tmp` et un seul thread de tests. Les compteurs d'archives incluent l'extraction préparatoire dans un workspace temporaire ; l'absence de publication est vérifiée par les journaux et les octets des fichiers originaux, plutôt que déduite uniquement de ces compteurs.

La suite de régression `--test archive_io_pipeline` réussit également : **29 tests passés**. Total de cette validation : **1 070 tests réussis et 21 ignorés**, sans échec dans les suites exécutées. `git diff --check` ne signale pas d'erreur d'espacement.

La vérification protège contre les collisions internes du plan ; elle ne constitue pas une transaction globale ni une nouvelle garantie contre toute modification concurrente externe. Aucun essai visuel GPUI ni runtime Linux n'a été effectué pour cette correction. Le repli Linux compare les noms exactement ; les montages Linux présentant d'autres règles de noms restent hors de cette validation. Les suites cassées décrites dans A02 ne sont pas corrigées ici.

Cette correction modifie du code partagé par les workers : fermer Oxyde, exécuter `cargo build --offline --bins`, puis `cargo run --offline` avant de tester l'application reconstruite.

## Correction A02 — validation et limites

Avant correction, `cargo check --offline --locked --all-targets` reproduit E0583, E0433 et E0425 dans les trois harnesses. Après correction, cette commande réussit sans avertissement. La découverte et la comparaison des noms avec le commit A01 confirment **10 scénarios `preview_worker`, 4 `archive_listing_worker` et 11 `archive_io_worker`**, avec leurs noms et fixtures conservés. Les tests unitaires internes restent dans leur suite `--lib`, sans inclusion ni duplication dans chaque intégration.

La première exécution complète révèle aussi une attente PDF obsolète : 256 × 512 pixels étaient attendus pour la page de 72 × 144 points. Le contrat actuel du renderer (`windows_pdf_thumbnail::PixelLayout::fit`), ses tests unitaires et `docs/SHARED_PREVIEW_CACHE.md` prévoient une enveloppe de 1024 pixels, soit 512 × 1024 dans ce scénario. L'assertion est mise en cohérence et le pixel central reste vérifié, avec les mêmes exigences de couleur/opacité et de suppression du fichier après réponse. Le renderer et son comportement ne changent pas.

L'interface `platform::worker_test_support`, limitée à Windows et masquée de la documentation publique, appelle les superviseurs compilés de production. Elle fournit les points d'injection des exécutables et commandes adverses, et convertit les deux métriques de supervision sans exposer les modules natifs. La compilation des sondes utilise le même verrou d'héritage des handles que les workers. Aucun repli de décodage local, changement du confinement LPAC ou modification du chemin de fermeture PDF/Intel n'est introduit.

Le workflow `Windows native tests` s'exécute sur les PR et les pushes sur `main` touchant les sources ou leurs ressources, ainsi que sur déclenchement manuel. Il appelle `scripts/test-windows.ps1` : compilation de toutes les cibles, construction de tous les exécutables, puis `cargo test --locked --all-targets -- --test-threads=1`, avec TEMP/TMP privés et restaurés à la sortie du script. La variante locale `./scripts/test-windows.ps1 -Offline` conserve les mêmes vérifications sans accès réseau. Le workflow hébergé n'a pas été exécuté sur GitHub pendant cette tâche ; aucun push n'a été demandé.

**Résultat final :** `./scripts/test-windows.ps1 -Offline` réussit hors bac à sable. `cargo check --offline --locked --all-targets` et `cargo build --offline --locked --bins` passent sans avertissement. L'exécution de toutes les cibles totalise **1 095 tests réussis, 21 ignorés, aucun échec** : 781 backend, 257 interface, 29 pipeline d'archives, 11 entrées/sorties d'archives, 4 listing d'archives, 10 aperçus et 3 collisions de transfert. Les exécutables sans tests et l'exemple compilent également. Les 25 scénarios restaurés passent tous ; aucun n'est ignoré. Journaux locaux dans `target/a02-validation/`, dont `native-tests-final.log`. Le script PowerShell est également validé par son parseur et `git diff --check` ne relève pas d'erreur d'espacement.

Les workers et leur code partagé doivent être reconstruits avant un essai applicatif : fermer Oxyde, lancer `cargo build --offline --bins`, puis `cargo run --offline`. La validation reste native Windows ; aucune exécution Linux ni validation visuelle GPUI n'est ajoutée par A02.

## Correction A07 — contrat et validation

Le découpage lexical reste disponible pour les candidats et les réservations prudentes du scheduler. Il n'est plus utilisé pour aiguiller suppression, corbeille, listing, création locale ou workspaces d'archives. `resolve_virtual_path` vérifie les ancêtres à extension reconnue jusqu'à trouver le fichier archive réel ; les dossiers ordinaires sont ignorés et une archive réellement placée plus bas peut être résolue. Le retour conserve une racine et un chemin interne, vide pour la racine elle-même. Les contrôles de sélection, rétention, publication et chemins internes des archives restent appliqués ensuite.

Les producteurs de métadonnées, y compris le décodeur du protocole de listing, renseignent `FileEntry::archive_path`. La racine est partagée par `Arc` au sein de chaque listing ; aucune donnée supplémentaire provenant du worker n'est nécessaire sur le protocole. Les entrées réelles conservent `None`. Les actions d'ouverture, de montage, de compression/extraction et les cibles de dépôt se basent sur cette origine et le type déjà connus. Les archives imbriquées dans une archive restent des fichiers ; les vrais dossiers à suffixe d'archive restent des dossiers. Ces décisions de rendu ne font pas d'I/O.

Le contexte d'un onglet est résolu en arrière-plan et appliqué avec le listing sous les protections existantes par identifiant d'onglet et de requête. Les deux parcours de watchers (abonnement et rechargement) utilisent `TabState::watched_path` : vrai dossier pour une navigation ordinaire, parent physique de l'archive pour une navigation virtuelle. La racine connue reste disponible après une erreur de rechargement pour recevoir une éventuelle recréation de l'archive. La création de dossier revalide son parcours sur le worker et renvoie cette décision avec son reçu ; l'interface ne crée une entrée Undo locale que pour une création réelle.

Les régressions couvrent les vrais dossiers `.zip`, `.7z`, `.tar.gz`, l'archive réelle placée sous ces dossiers, les descendants et les noms Unicode, copie/déplacement/suppression, corbeille suivie d'Undo, création/renommage local, compression/extraction natives et origine des entrées issues du worker de listing. Les tests d'interface vérifient les sélections mixtes et le chemin effectivement utilisé par les watchers, y compris après disparition de l'archive. Ils ne constituent pas un essai visuel des menus ni une validation Linux.

**Résultat final :** `./scripts/test-windows.ps1 -Offline` réussit hors bac à sable, avec TEMP/TMP privés. `cargo check --offline --locked --all-targets` et la reconstruction de tous les exécutables passent sans avertissement. La suite complète totalise **1 106 tests réussis, 21 ignorés, aucun échec** : 787 backend, 259 interface et 60 intégrations natives. Les 11 nouveaux tests sont exécutés, sans exclusion. La construction `release` de tous les exécutables et les mêmes 60 tests d'intégration en `release` réussissent également. Journaux locaux : `target/a07-validation/native-tests-final.log`, `release-build.log` et `release-integration.log`. Le premier essai de reconstruction de développement avait été bloqué par l'exécutable Oxyde ouvert ; la validation finale a été relancée après sa fermeture. `git diff --check` ne relève pas d'erreur d'espacement.

Ce changement modifie les workers et leur code partagé : fermer Oxyde puis reconstruire avec `cargo build --offline --bins`, avant `cargo run --offline`.
