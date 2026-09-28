# Audit et feuille de route Oxyde — 27 septembre 2026

## Mandat pour l’IA chargée de l’implémentation

Ce document décrit des corrections à implémenter dans l’ordre ci-dessous. Il ne constitue pas une autorisation de modifier des fichiers personnels pour tester les opérations. Tous les scénarios destructifs doivent utiliser des répertoires temporaires dédiés. Conserver les changements locaux existants ; ne pas réinitialiser le dépôt.

Base examinée : commit `c8de4759b766839ebcdbd908d61e6b96b08098e1` **plus les modifications non commitées** de navigation, menus et barre supérieure. Les références de lignes sont indicatives : rechercher les symboles cités avant de modifier.

Lire `AGENTS.md`. Respecter l’OAL : les nouvelles API natives passent par `src/platform/mod.rs`, avec implémentations Windows/Linux. Ne pas changer les dépendances GPUI pour résoudre un problème de composant. Le code UI principal est actuellement `src/ui/app/mod.rs`, et non `src/ui/app.rs`.

Effectuer un lot à la fois : lire les fonctions et leurs appelants, reproduire le défaut, ajouter les tests nécessaires, corriger, vérifier, puis consigner le résultat. Éviter une réécriture globale du gestionnaire de fichiers. Avant toute optimisation, mesurer un scénario reproductible.

## Portée et niveau de preuve

Audit statique ciblé des menus, navigation, onglets/panneaux, watchers, rendu des listes, moteur d’opérations, historique et frontière OS ; sondage des archives et utilitaires de chemins. Ce n’est pas une revue exhaustive de la recherche, des miniatures, des formats d’archives ou de tous les appels FFI.

- **Établi** : branche ou invariant problématique observable dans le code ; cela ne signifie pas qu’un scénario a été exécuté dans l’interface.
- **À reproduire** : risque identifié nécessitant une observation GPUI ou un test d’intégration avant de choisir la correction.
- **Optimisation** : coût visible dans le code, gain à mesurer.
- **P0** : risque de perte de données ; **P1** : blocage ou comportement incorrect important ; **P2** : fiabilité/performance ; **P3** : entretien.

Vérifications exécutées sur Windows : `cargo check --all-targets` réussi, `cargo test --lib` réussi (9 tests). Aucun avertissement Rust du projet dans ces sorties ; le lanceur affiche `could not canonicalize path C:\Users\Biluf`. Les tests existants portent sur les chemins, attributs, une erreur de listing et deux opérations ZIP. Aucun test interactif des menus ni test de résistance des déplacements n’a été réalisé dans cet audit. Linux n’a pas été compilé.

Points positifs à préserver : listing de dossiers déporté via `spawn_blocking`, contrôle de génération des réponses de navigation, mutualisation des watchers par chaîne de chemin, remplacement ZIP avec sauvegarde de l’original, séparation partielle via `Platform`.

## Ordre des lots

| Lot | Priorité | Objet | Dépendances |
|---|---|---|---|
| 1 | P0 | Préserver les sources et destinations pendant copie/déplacement | Aucune |
| 2 | P1 | Terminaison des workers et propagation des erreurs | 1 ; tests à préparer ensemble |
| 3 | P1 | Complétude des copies : dossiers vides, liens, annulation du scan | 1–2 |
| 4 | P0 | Historique fondé sur les effets réels | 1–3 |
| 5 | P1 | Navigation : cache, erreurs et réponse après déplacement d’onglet | Aucune dépendance aux opérations |
| 6 | P1 | Invariants des onglets/panneaux et watchers | 5 |
| 7 | P1/P2 | État, fermeture et géométrie des menus | 5–6 pour les actions ciblées |
| 8 | P2 | Travail natif hors du thread UI, respect de l’OAL | 7 pour le cycle de vie des menus |
| 9 | P2 | Coûts du rendu et rafraîchissements | 5–8 |
| 10 | P2/P3 | Portabilité, chemins et structure du code | Corrections précédentes stabilisées |

L’ordre fait passer la sûreté des opérations avant le confort UI. Le lot 4 reste P0 malgré sa position : son implémentation dépend du journal d’exécution des lots 1–3.

## Lot 1 — Copie/déplacement sans perte de données

**Établi, P0.** `src/systems/file_ops.rs`, `perform_copy`, particulièrement autour des lignes 516–643.

1. La boucle de transfert sort par `break` si l’annulation ou une erreur partagée est détectée. Le code incrémente ensuite `processed_files` et, pour un déplacement, appelle `remove_file(src)` même si EOF n’a pas été atteint. Une annulation peut donc supprimer la source d’un fichier partiellement copié.
2. Le nettoyage final appelle `remove_dir_all(src)` lorsqu’il ne voit aucun `Skip` explicite dans la table de résolutions. Or les conflits non résolus peuvent être ignorés implicitement dans le parcours. L’absence de `Skip` explicite ne prouve pas que tous les fichiers ont été transférés. Le nettoyage ne vérifie pas non plus l’annulation avant de supprimer les arbres sources.
3. `File::create(dest)` tronque une destination existante avant que la copie réussisse. Un échec ou une annulation détruit potentiellement l’ancien contenu. Le moteur ne vérifie pas lui-même l’identité source/destination ; un appel incorrect peut ouvrir puis tronquer le même fichier.

**Modifications demandées :**

- Représenter le résultat par fichier : copié/validé, ignoré, annulé, échoué. Ne compter comme terminé et ne supprimer une source que si la copie complète a réussi et que les erreurs de finalisation ont été traitées.
- Copier vers un fichier temporaire unique dans le répertoire destination ; publier le résultat après succès. Définir explicitement le remplacement d’une destination existante et sa récupération en cas d’échec. Les particularités natives du remplacement passent par l’OAL.
- Supprimer les dossiers sources uniquement avec `remove_dir` après traitement de leurs enfants ; un dossier non vide doit rester. Proscrire le nettoyage récursif d’un arbre source supposé transféré.
- Valider dans le moteur les cas source = destination, destination descendante de la source, sources qui se recouvrent. Tenir compte des liens/jonctions et des chemins équivalents ; une comparaison textuelle de préfixes ne suffit pas.
- Propager les erreurs de création de parents et de suppression des sources. Une copie réussie avec source non supprimée est un résultat partiel, pas un déplacement réussi.

**Validation :** utiliser uniquement des fixtures temporaires ; injecter une annulation après quelques blocs, une erreur de lecture/écriture, un conflit implicite et explicite. Vérifier les octets de chaque original et destination. Tester copie sur soi, destination imbriquée, déplacement avec fichier ignoré. Ne jamais obtenir une source supprimée pour un transfert incomplet.

## Lot 2 — Workers : terminaison, erreurs, priorité

**Établi, P1.** `src/systems/file_ops.rs`, `perform_copy` et `perform_delete`, vers 479, 614, 786 et 853.

- Les workers d’indice supérieur à 1 dorment puis font `continue` lorsque Turbo est désactivé, avant de vérifier l’épuisement de la file. Ils peuvent attendre indéfiniment après le travail des deux premiers workers. Le superviseur attend leur fin, puis les joint. Cela concerne aussi les opérations sans fichier à traiter.
- `let _ = handle.join()` ignore à la fois les erreurs renvoyées et les paniques. Certaines erreurs d’ouverture/création/suppression retournent avant d’alimenter `error_captured`. Un échec peut ainsi ne pas atteindre le statut final.
- La priorité du thread `spawn_blocking` est abaissée sans restauration garantie ; le commentaire annonçant une restauration ne constitue pas un mécanisme. Ce thread appartient à un pool réutilisé.

**Modifications :** prévoir une condition de fin indépendante de Turbo, une terminaison coordonnée après erreur, et la collecte de tous les `JoinHandle<Result<...>>`. Utiliser un garde de restauration de priorité, y compris sur retour anticipé. Conserver un statut exact et ne pas forcer la progression à 100 % sur annulation/échec. Éviter d’ajouter des boucles de polling pour résoudre le problème.

**Validation :** délais maximum de terminaison pour 0, 1 et plusieurs fichiers en mode discret ; Turbo activé/désactivé pendant le travail ; erreur et panique injectées ; annulation pendant pause. Vérifier que tous les workers se terminent. Pour un test susceptible de bloquer le processus, isoler l’exécution dans un processus enfant avec timeout.

## Lot 3 — Complétude et sémantique de copie

**Établi pour dossiers vides et scan, P1.** `perform_copy` ignore les entrées répertoire ; seuls les parents des fichiers copiés sont créés. Une arborescence vide disparaît dans une copie ou un déplacement passant par le chemin lent. Le scan préalable ne consulte pas les drapeaux pause/annulation.

**À caractériser :** les liens sont parcourus avec `WalkDir` puis ouverts comme fichiers ou traités via `is_dir`; aucune politique explicite de préservation des liens n’est visible dans ce flux. Les métadonnées ne sont pas réappliquées par le transfert manuel.

**Modifications :** planifier les créations de répertoires, rendre le scan annulable, définir une politique explicite pour liens symboliques/jonctions et métadonnées. Ne pas suivre implicitement un lien vers un arbre hors source. Conserver le renommage rapide quand ses préconditions sont satisfaites.

**Validation :** dossiers vides imbriqués ; liens vers fichier/dossier et lien cassé sur les OS autorisant leur création ; annulation durant un grand scan ; comparaison des chemins et contenus après déplacement lent.

## Lot 4 — Annuler/rétablir les effets effectivement réalisés

**Établi, P0.** `src/systems/file_ops.rs::execute_operation` renseigne `created_files: None`. `src/commands/ops.rs::undo_last_action` reconstruit les destinations à partir des noms sources et place ces destinations à la corbeille.

Après fusion dans un dossier préexistant, annuler une copie peut donc placer à la corbeille tout ce dossier destination, y compris des éléments qui existaient avant l’opération. Une entrée ignorée ne doit pas devenir une suppression lors d’Undo. Le contenu remplacé n’est pas restaurable à partir des seuls chemins.

`undo_last_action` et `redo_last_action` dépilent avant exécution : un retour `Err` avant transfert vers la pile opposée perd la transaction de l’historique. Les opérations partiellement réussies exigent un traitement explicite.

**Modifications :** utiliser un journal des effets réels produit par les lots 1–3 : chemins créés, déplacements réussis, éléments ignorés, sauvegardes éventuelles des remplacements. Undo ne doit agir que sur ces effets et doit gérer les changements externes depuis l’action. Ne valider le déplacement entre piles qu’après résultat connu ; conserver un état récupérable en cas d’échec partiel. Réexaminer également le moteur distinct `perform_copy_with_progress` utilisé par Undo/Redo avant toute mutualisation.

**Validation :** copier dans une destination préexistante, avec enfants préexistants et conflits ignorés, puis Undo/Redo ; aucun enfant préexistant ne doit disparaître. Injecter une erreur d’Undo et vérifier que l’historique reste cohérent. Tester un remplacement et un fichier modifié extérieurement entre action et Undo.

## Lot 5 — Navigation : données cohérentes et erreurs visibles

**Établi, P1.** `src/ui/app/navigation.rs::load_path` vide `files`, mais pas `cached_filtered_files`, alors que `TabState::filtered_files` retourne ce cache. Le nouveau chemin peut être affiché avec les fichiers de l’ancien dossier jusqu’à la réponse ; en cas d’erreur, ce contenu peut rester affiché. L’erreur de listing est seulement journalisée.

La même fonction efface certains filtres mais conserve les filtres taille/date et `selection_anchor`, sans politique de transition homogène.

`request_tab_files` protège bien contre les réponses périmées par génération, mais cherche l’onglet dans le panneau capturé au départ. Si l’onglet est déplacé/scindé pendant le chargement, la réponse est abandonnée car l’onglet n’est plus dans ce panneau.

**Modifications :** définir un état `Loading/Ready/Error` par onglet ; maintenir le cache affiché cohérent avec le chemin et désactiver les actions sur un contenu périmé. Définir la conservation/réinitialisation des filtres et de la sélection. Appliquer la réponse en retrouvant globalement l’onglet par identifiant, puis vérifier génération et chemin. Garder les protections anti-réponse périmée existantes.

**Validation :** A → B lent → C ; B inaccessible ; déplacement de B en cours de chargement vers un autre panneau ; fermeture avant réponse ; dossier réellement vide. Ne jamais confondre erreur et dossier vide ni afficher A sous le chemin B.

## Lot 6 — Onglets et panneaux : mutations cohérentes

**Établi, P1/P2.** `src/ui/workspace.rs` :

- `close_tab_id` ne décrémente pas l’index actif lorsqu’un onglet situé avant lui est retiré. Exemple : A, B actif, C ; fermer A sélectionne C au lieu de conserver B. Les retraits dans `split_tab_to_axis` et `move_tab_to_pane_id` présentent le même type de logique.
- `close_tab_id` utilise `remove(tab_idx)` sans vérifier la borne lorsque plusieurs onglets existent. `switch_tab` accepte aussi un index arbitraire.
- Les fonctions de déplacement retirent l’onglet avant de vérifier que la destination est valide ; en cas d’échec elles ne le restaurent pas.
- `add_tab` reconstruit un ID à partir du maximum des onglets présents, ce qui autorise la réutilisation d’identifiants après fermeture.
- Les actions contextuelles « Fermer » et « Fermer les autres » dans `components/mod.rs` appellent directement `state`, contournant la réconciliation des watchers effectuée par `OxydeApp::close_tab`.
- La duplication copie seulement le chemin. C’est une sémantique à décider explicitement, pas forcément un bug : faut-il dupliquer aussi historique, tri et filtres ? Le wrapper actuel recharge l’onglet actif même si l’index source de duplication est invalide.

**Modifications :** utiliser des identifiants non réutilisés ; préserver l’identité de l’onglet actif lors des retraits ; valider toutes les destinations avant mutation ; retourner `Option/Result` avec l’identifiant créé ou déplacé. Centraliser les effets annexes dans les commandes d’application (watchers, requêtes, notification). Si duplication d’état, créer des scroll handles indépendants : leur `Clone` ne garantit pas l’indépendance.

**Validation :** table de tests avec fermeture avant/après/à l’index actif, index invalide, destination inexistante, transfert du dernier onglet, duplication d’un onglet inactif. Aucun panic, aucune perte d’onglet, watchers devenus inutiles libérés.

## Lot 7 — Menus : cycle de vie et géométrie

Fichiers : `src/ui/components/dropdown.rs`, `src/ui/app/mod.rs` (boucle `CONTEXT MENU OVERLAY`), `src/ui/app/layout.rs`, `src/ui/components/mod.rs`.

### 7.1 Stabiliser et vérifier le survol — P1, à reproduire

Les changements locaux remplacent les entités recréées à chaque rendu par `RenderOnce` et ajoutent un ID par menu et par ligne. Ils compilent mais leur résultat visuel n’a pas été confirmé. Ne pas considérer les anciennes affirmations de correction comme une validation interactive.

**Travail :** reproduire le survol lent/rapide sur chaque ligne, avec et sans icône, menu natif, filtre et sous-menu. Observer les transitions de hitbox/hover et les rafraîchissements si nécessaire. Préserver le curseur flèche. N’ajouter un état de sélection explicite que si l’observation le justifie ; ne pas masquer un défaut d’identité ou de hit testing par un repaint permanent.

### 7.2 Unifier fermeture, notifications et tâches — P1/P2

Chaque menu installe son propre `on_mouse_down_out` qui ferme toute la pile. Un clic dans un sous-menu est géométriquement hors du parent : **à reproduire**, car la propagation/occlusion GPUI décide du résultat. Plusieurs callbacks `on_close` effacent les menus sans `cx.notify()`. Les tâches de sous-menu ne sont pas systématiquement annulées quand la pile est effacée.

L’ouverture passe par `on_mouse_move`, `defer`, `spawn` et un timer `Duration::ZERO`. Chaque mouvement sur la même ligne peut relancer ce flux. Une durée nulle ne supprime ni l’ordonnancement ni le risque de rappel obsolète.

**Travail :** extraire un contrôleur de pile de menus. Fermeture unique sur clic extérieur à l’ensemble de la pile, notification explicite, annulation des tâches, génération de session vérifiée dans les callbacks différés. Déclencher l’ouverture au changement de ligne, avec maintien du sous-menu lorsque le pointeur entre dedans. Ne pas ajouter de délai artificiel sans raison UX mesurée.

**Validation :** clic sur une commande de sous-menu exécuté exactement une fois ; déplacement parent/enfant sans fermeture intempestive ; fermeture puis réouverture rapide sans réapparition d’un ancien menu ; retrait effectif après clic extérieur.

### 7.3 Calculer depuis les bounds réels — P1, établi

La position enfant est calculée avec `parent.position.y + index * 26`, sans défilement du parent ni correction de son ancrage après `snap_to_window`. Le calcul horizontal traite `position.x` comme bord gauche même pour un parent ancré à droite (`open_left`). La largeur enfant est anticipée avec une constante de 220 px. Le clamp des menus dans `layout.rs` utilise 28 px par ligne alors que le dropdown utilise 26 px.

**Travail :** centraliser les métriques ; capturer les bounds réels de la ligne et du menu après placement ; positionner le sous-menu à partir de ces bounds, de sa largeur mesurée et du viewport. Les éventuels canvas de mesure doivent rester passifs, sans appeler la logique métier pendant paint.

**Validation :** menus longs défilés ; bords bas/droit ; trois niveaux ouverts vers la gauche ; plusieurs DPI. Le sous-menu doit rester aligné sur sa ligne déclenchante et entièrement accessible.

### 7.4 Cibler la bonne sélection — P2, à caractériser

`oxyde_context_menu_items` reçoit un chemin, mais certaines commandes consultent ensuite la sélection du panneau actif ; « Plus d’options » utilise `selected_paths.iter().next()`. Capturer pane/tab/selection au moment de l’ouverture, ou invalider le menu si son contexte disparaît. Tester clic droit sur un panneau inactif et sélection multiple. Ne pas dépendre de l’ordre d’un `HashSet`.

## Lot 8 — Menu natif et OAL

**Établi, P2.** `components/mod.rs` appelle synchroniquement `get_native_context_menu_items` depuis les callbacks UI. La commande contient du COM/Shell dans `commands/system.rs`. Des appels `windows::` subsistent aussi dans `commands/io.rs` et `commands/ops.rs`, contrairement à la frontière prescrite par `AGENTS.md`.

**Travail :** déplacer progressivement les implémentations OS vers `Platform`, en conservant des modèles de données neutres. Déporter la construction du menu natif hors du thread UI avec un état de chargement et rejet des résultats obsolètes. Auditer l’affinité COM, le cycle de vie du contexte natif et la validité des IDs de commandes avant de choisir un worker : ne pas envoyer aveuglément des objets COM sur un pool arbitraire. Traiter les erreurs d’exécution dans l’interface.

**Validation :** extension Shell lente, ouverture/fermeture avant réponse, changement de sélection, commande native exécutée sur le bon objet. Mesurer le temps de blocage UI. Compiler Windows ; vérifier Linux dans un environnement Linux disponible.

## Lot 9 — Réduire les coûts UI et fiabiliser les rafraîchissements

### 9.1 Rendu des grands dossiers — optimisation P2

`components/file_list.rs` clone toute la liste filtrée avec `to_vec()` à chaque rendu, puis parcourt les fichiers pour déterminer les filtres présents. `workspace.rs::update_cache` clone les entrées, reconvertit le filtre de nom en minuscules pour chaque fichier et recalcule les largeurs ; `sort_files` alloue des minuscules dans le comparateur. Le backend trie déjà dans `list_dir_blocking` avant le tri UI.

**Travail :** profiler 1 000/10 000/100 000 entrées ; conserver un snapshot partagé des entrées et des indices filtrés ; ne cloner que le nécessaire aux callbacks ; pré-calculer clés de tri, texte du filtre, catégories et référence temporelle des dates. Invalider caches/largeurs uniquement quand leurs données changent. Évaluer un seul tri selon le contrat backend/UI, sans casser d’autres appelants.

**Validation :** relever durée de rendu/filtrage/tri et mémoire avant/après, sur mêmes fixtures et build. Les simples mouvements souris ne doivent pas provoquer de copie de toutes les entrées. Ne pas promettre 60 fps sans mesure.

### 9.2 Paramètres et watchers — P2, établi

`components/settings.rs` recharge seulement l’onglet actif de chaque panneau pour les options fichiers cachés/système. Un onglet inactif garde donc son listing selon l’ancienne option après activation. `refresh_all` utilise `load_path`, qui réinitialise sélection et certains filtres.

`reconcile_watchers` crée un watcher par chaîne de chemin et effectue `exists()`/`watch()` synchroniquement ; il essaie aussi les destinations virtuelles. Les erreurs sont ignorées. Le polling de 500 ms relance des listings sans annuler le travail précédent ; les générations protègent la cohérence mais pas le coût du travail devenu inutile.

**Travail :** invalider tous les onglets affectés par les options, avec rechargement immédiat ou paresseux explicite ; distinguer navigation et rafraîchissement conservant l’état. Filtrer les destinations virtuelles ; normaliser l’identité des chemins ; limiter/coalescer les requêtes par dossier et partager les résultats compatibles. Profiler les watchers réseau avant de modifier leur architecture ; rendre leurs échecs diagnostiquables.

**Validation :** basculer cachés/système puis visiter des onglets inactifs ; rafraîchir sans perdre sélection valide ; rafale d’événements de copie ; lecteur absent ; deux onglets sur le même dossier. Vérifier l’absence d’accumulation de requêtes et l’application du dernier état.

## Lot 10 — Portabilité, chemins, entretien

1. **Établi, P2 : tests Windows non conditionnés.** `utils/path_security.rs` appelle des chemins `C:/...` « Unix absolus » et attend leur acceptation sur tous les OS. Corriger les fixtures/`cfg` et vérifier les vrais chemins absolus Linux.
2. **Établi, P2 : contrat trompeur.** `safe_join` annonce une protection contre la traversée, mais fait uniquement `base.join(input)`. Ne pas le traiter comme une barrière de sécurité. Rechercher ses appelants, renommer la fonction si elle ne doit rien restreindre, ou définir et tester le contrat requis. L’explorateur doit pouvoir naviguer hors d’un dossier ; les noms de renommage et les chemins d’extraction ont des contraintes différentes.
3. **À vérifier, P2 : noms et extractions.** `execute_file_rename` fait `parent.join(new_name)` ; tracer la validation backend avant de conclure à un défaut exploitable. Tester séparateurs, chemins absolus, `..`, noms réservés Windows. Pour les archives, auditer chaque format, liens et chemin de sortie ; la présence de `safe_join` seule ne prouve aucune vulnérabilité d’extraction.
4. **P3 : découpage.** Après stabilisation, extraire le contrôleur des menus et les opérations UI de `app/mod.rs`, factoriser les branches Items/NativeItems, puis documenter la source de vérité entre `ui/workspace.rs` et `models/session.rs` héritée de la migration. Ne pas supprimer l’ancien modèle avant d’avoir tracé les consommateurs backend, notamment recherche et session.

## Conditions de clôture pour l’implémentation

- Pour chaque lot, consigner les scénarios exécutés, les résultats et les points restant non vérifiés dans ce document ou un compte rendu lié.
- `cargo fmt --check`, `cargo check --all-targets`, tests pertinents et tests existants passent ; distinguer warnings Rust et messages de l’environnement.
- Ajouter prioritairement des tests de non-régression sur invariants de données, terminaison des workers, historique et transitions d’onglets. Pour le survol, compléter les tests par une vraie vérification interactive ; la compilation ne le valide pas.
- Réserver Linux, réseau, COM/extensions Shell et injection d’erreurs à des environnements adaptés ; déclarer explicitement ce qui n’a pas été testé.
- Ne pas présenter une optimisation comme un gain acquis sans mesure ni un risque statique comme un bug reproduit en session.

## Journal d’implémentation — 27 septembre 2026

Une première passe a été appliquée dans le code et vérifiée par compilation/tests. Les cases ci-dessous distinguent les invariants corrigés des travaux qui nécessitent encore une validation interactive ou une refonte dédiée.

| Lot | État | Résultat / reste à faire |
|---|---|---|
| 1 | Corrigé, validation partielle | Copie vers fichier temporaire puis publication ; sauvegarde d’une destination remplacée ; sources supprimées uniquement après publication ; nettoyage par `remove_dir` ; conflits ignorés conservés lors d’un déplacement ; vérification des sources qui se recouvrent et destination imbriquée ; liens symboliques explicitement refusés. Tests temporaires couvrent remplacement, conflit ignoré, déplacement et annulation avant transfert. L’injection d’erreurs au milieu d’une lecture/écriture et le remplacement natif restent à tester. |
| 2 | Corrigé, validation partielle | Fin des workers quand la file est épuisée, collecte des erreurs/jointures et garde de restauration de priorité. `cargo check --all-targets` passe. Pas encore de test de stress avec timeout ni d’injection de panique. |
| 3 | Partiel | Les dossiers vides sont préservés ; l’annulation est consultée durant le scan. Politique choisie pour cette passe : refuser les liens symboliques plutôt que les suivre ou les recopier. Restauration des métadonnées et tests de liens sur les deux OS restent en attente. |
| 4 | Partiel | Les opérations consignent fichiers/répertoires créés, copies, déplacements, sauvegardes de remplacement et empreintes BLAKE3 des contenus copiés (calculées dans le flux de copie). Undo/Redo ne dépile qu’après succès, refuse les anciennes entrées sans journal et refuse de toucher un fichier modifié extérieurement. Redo Copy publie atomiquement et annule ses fichiers nouveaux en cas de conflit ultérieur ; Undo/Redo Move tentent de restaurer les renames déjà faits en cas d’échec et nettoient les répertoires créés. Limites restantes : Undo multi-éléments, rétablissement des backups et suppressions via corbeille peuvent encore être partiels si le système échoue au milieu ; déplacement inter-volume fonctionne au niveau du worker initial mais son Undo/Redo reste à concevoir et tester. |
| 5 | Implémenté, validation UI partielle | État Loading/Ready/Error par onglet, cache vidé à la navigation, réponse retrouvée globalement par ID stable et chemin vérifié. Le rafraîchissement conserve filtres, sélection et position de défilement. Les erreurs de listing classifient désormais chemin introuvable / accès refusé et sont rendues en messages français sans détail OS brut ; le détail demeure dans les logs. Le test manuel Windows a confirmé navigation et distinction erreur/dossier vide. Les scénarios A→B→C et déplacement pendant requête restent à jouer. Il n’existe toujours pas de système global de notification : plusieurs actions ne font que journaliser leurs erreurs ; une file de toasts unifiée reste une évolution UX séparée. |
| 6 | Implémenté en partie | Fermeture invalide sans mutation, conservation de l’onglet actif après retrait/réordonnancement, IDs monotones, validation des panneaux destination et réconciliation des watchers après fermeture, fermeture des autres, déplacement et split par glisser-déposer. Tests unitaires de fermeture, IDs, onglet actif, déplacement vers panneau invalide et split. La vérification pratique des watchers reste à faire. |
| 7 | Partiel | Index survolé conservé pour ne pas relancer les tâches sur chaque mouvement dans la même ligne ; sa mise à jour notifie explicitement GPUI et alimente maintenant la surbrillance contrôlée des lignes virtualisées (en plus du pseudo-état hover), ce qui corrige une cause plausible des états visuels manquants. Fermeture extérieure vérifie les bounds approximatifs des menus frères. La construction et l’exécution du menu Shell passent désormais par `spawn_blocking`, affichent un état de chargement et rejettent les réponses obsolètes par jeton. Les appels synchrones dans les callbacks UI ont disparu. Survol, hit testing, géométrie après clamp/scroll, sélection multi-fichiers, erreur Shell et clic réel restent à vérifier avec l’application. La mesure depuis les bounds réels et l’extraction du contrôleur ne sont pas faites. |
| 8 | Partiel | Les appels UI au menu natif utilisent maintenant deux méthodes neutres de `Platform`; Windows les délègue encore à l’implémentation historique de `commands/system.rs`, à déplacer dans `platform/windows_impl.rs`. L’initialisation COM STA est gardée par RAII et nettoyée sur retours d’erreur. Les autres appels `windows::` de `commands/io.rs` et `commands/ops.rs` restent à migrer ; le test d’une extension lente et la validation d’affinité COM sur Windows restent nécessaires. |
| 9 | Partiel | Les préférences fichiers cachés/système rechargent désormais tous les onglets ; `refresh_all` conserve l’état utilisateur. Aucun profil de rendu/watchers n’a été mesuré : ne pas appliquer les optimisations proposées sans baseline. |
| 10 | Partiel | Tests de chemins conditionnés par l’OS ; `safe_join` est renommé `join_path` et documenté comme non-confinant ; renommage backend limité au dossier source et refus du remplacement implicite, avec test sur fixtures. La dépendance abandonnée/vulnérable `sevenz-rust` est remplacée par `sevenz-rust2 0.23.0` pour extraction, lecture/recherche et création ; lockfile régénéré. L’extraction valide les noms absolus/parent/ADS, reconstruit le chemin sous la racine et contrôle chaque parent/destination existant après canonicalisation. ISO rejette les noms de composants dangereux et partage ce contrôle. Tests avec un vrai 7z malveillant et un aller-retour création/extraction. ZIP/TAR s’appuient sur les protections documentées de leurs crates, mais fixtures hostiles ZIP/TAR, archives ISO réelles, liens/races filesystem et Linux restent à valider. |

Vérifications sur Windows après cette passe : `cargo fmt --check`, `cargo check --offline --all-targets`, `cargo test --offline --all-targets` (26 tests bibliothèque et 7 tests UI/workspace) réussis. Le linker émet son message d’environnement (« Création de la bibliothèque… ») pendant les tests du binaire ; aucun avertissement Rust du projet dans `cargo check`. L’accès réseau direct du terminal a nécessité une autorisation ponctuelle ; une fois accordée, le miroir GitHub a répondu et Cargo a téléchargé `sevenz-rust2` ainsi que ses dépendances. Linux et les essais interactifs GPUI ne sont pas disponibles dans cette validation.

## Retour des essais Windows et correctifs de suivi — 28 septembre 2026

Essais rapportés par l’utilisateur : tests 1, 2, 4 et 5 réussis ; test 3 réussi sauf l’action « Tout cocher » ; test 5 a aussi relevé l’apparence assombrie des entrées parent de sous-menu ; test 6 a révélé des conflits sans message ni remplacement ; test 7 n’a pas permis d’annuler ; test 8 a montré un éditeur de renommage réduit à un caret sans texte visible ; test 9 a confirmé que le collage ne traitait que le premier élément.

Correctifs de suivi appliqués mais à rejouer dans l’interface Windows :

- La sélection complète des filtres n’est plus normalisée en filtre vide ; les en-têtes indiquent désormais « Tout décocher » lorsque toutes les catégories visibles sont cochées. Un test de non-régression couvre la persistance du filtre complet.
- Les éléments natifs `id < 0` ne sont assombris que s’ils sont des feuilles : Windows utilise l’ID `-1` pour les éléments popup qui ouvrent un sous-menu.
- Copie/déplacement passent désormais par `FileOperationManager`, avec pré-vérification des conflits et choix modal « Remplacer tout / Ignorer tout / Annuler ». Le worker enregistre l’effet détaillé pour Undo/Redo ; une fixture confirme la copie de plusieurs sources.
- Les erreurs de transferts et d’Undo/Redo sont présentées dans un dialogue au lieu de rester seulement dans les logs.
- Le champ texte traduit maintenant les plages UTF-16 échangées avec GPUI en offsets UTF-8 et inversement. Cette correction du contrat d’input est compilée, mais seul un essai interactif pourra confirmer la saisie/affichage du renommage sous Windows.

Validation de cette passe : `cargo fmt --check`, `cargo check --all-targets` et `cargo test --all-targets` réussis (27 tests bibliothèque + 8 tests UI/workspace). Aucun avertissement Rust du code lors de `cargo check`; le linker affiche son avertissement environnemental habituel de création des fichiers `.lib`/`.exp`. Les tests interactifs cités restent à rejouer ; rien ici ne prétend les avoir exécutés.

### Suivi du retour Undo/transfert — 28 septembre 2026

La capture Windows montre `TrashError (0x7C)` à l’annulation. La copie/Undo envoyait les destinations copiées au Shell Windows pour les mettre dans la corbeille ; Undo supprime maintenant directement les fichiers créés après vérification de leur hash, puis restaure les backups, sans appeler le Shell. Une régression isolée vérifie qu’Undo Copy réussit sans le chemin corbeille.

Le worker appliquait aussi une pause de 1 ms après chaque bloc de lecture de 512 Kio en mode discret. Cette temporisation artificielle est supprimée ; le buffer est porté à 1 Mio dans les deux modes, tout en gardant la priorité basse et le hash d’intégrité. Les fichiers de staging utilisent un nom préfixé par `.oxyde-`, sont marqués cachés sur Windows pendant la copie puis réaffichés avant publication. Les backups d’Undo sont dot-prefixed pour qu’Oxyde les masque normalement, mais demeurent présents dans le dossier jusqu’à expiration/nettoyage de leur transaction ; les afficher explicitement via « fichiers cachés » reste possible. Ils ne doivent pas être supprimés manuellement si l’utilisateur veut conserver Undo.

Validation après ce suivi : `cargo fmt --check`, `cargo check --all-targets`, `cargo test --all-targets` (29 tests bibliothèque + 8 UI/workspace) réussis. Deux tests isolés couvrent Undo Copy sans corbeille et restauration d’un backup. À rejouer sous Windows : Undo après copie simple et remplacement, vérification de restauration de l’ancien contenu, absence de `.tmp` après fin, absence de `.oxyde-*.tmp`/`.oxyde-*.bak` dans la vue normale, visibilité/fin du transfert d’un gros fichier avec mode discret et Turbo. Le temps réel dépend du disque, de l’antivirus et des performances du périphérique ; aucun benchmark n’a été exécuté ici.

### Suivi du renommage inline — 28 septembre 2026

Retour Windows : le texte saisi était pris en compte à la validation, mais restait invisible dans le champ inline ; une collision de nom ne donnait qu’un log technique. L’édition inline a été remplacée par une boîte de dialogue spécialisée, dotée d’un champ large et d’actions Renommer/Annuler. L’échec de renommage conserve la boîte ouverte et affiche un message localisé ; la collision indique maintenant qu’un élément porte déjà ce nom. La liste n’embarque plus le champ d’édition dans une ligne virtualisée. La cause exacte de l’invisibilité n’a pas été reproduite par test graphique, donc vérifier l’affichage et la saisie dans cette nouvelle popup reste nécessaire.

Le champ de la popup conserve maintenant son padding visuel, affiche une sélection, prend en charge le clic-glissé pour sélectionner une plage, le double-clic pour un mot, le triple-clic et Ctrl+A pour tout sélectionner. L’indexation souris reste à confirmer visuellement sur Windows, notamment avec des noms Unicode.

La popup reste le mode de renommage du menu contextuel et de F2. Un second parcours inline est ajouté au clic lent répété sur le texte du nom (250 ms à 2 s) ; l’édition inline reprend la sélection native et sa surbrillance, valide avec Entrée ou un clic hors du champ, et annule avec Échap. À vérifier sous Windows : fenêtre de clic lent, absence de renommage sur clic rapide/double-clic rapide, navigation par double-clic rapide sur les dossiers, et validation hors champ sans double soumission.

## Feuille de route de tests pratiques — Windows

Exécuter les essais de fichiers uniquement dans un dossier temporaire dédié, avec une copie de sauvegarde des fixtures. Noter version/commit, mode Turbo, résultat attendu/observé et éventuels logs. Ne pas utiliser les dossiers personnels comme cibles.

1. **Démarrage et navigation** : lancer l’application, naviguer rapidement A → B → C, revenir à A/B, actualiser puis vérifier que chaque onglet affiche le bon chemin et les bons fichiers. Ouvrir un chemin inexistant pour vérifier l’état d’erreur, puis un dossier réellement vide pour vérifier qu’il est distinct.
2. **Onglets/panneaux** : créer trois onglets, activer le deuxième et fermer le premier ; réordonner un onglet inactif ; dupliquer un onglet non actif ; déplacer un onglet dans un autre panneau puis le scinder dans les quatre zones ; fermer le dernier onglet d’un panneau. Vérifier que l’onglet actif, les chemins, les watchers et le chargement restent cohérents.
3. **Menus GPUI** : survoler lentement puis rapidement chaque entrée des menus de fichiers, dossiers, panneau, onglet, filtres, sidebar et breadcrumb, avec/sans icône. Vérifier curseur flèche, surbrillance stable, clic extérieur, clic dans un sous-menu sans fermeture du parent, maintien du menu pendant le déplacement du pointeur et fermeture après sortie.
4. **Géométrie des menus** : tester menus près des quatre bords, menus longs après défilement, sous-menus ouvrant à gauche, puis plusieurs niveaux si disponibles. Répéter à 100 %, 125 % et 150 % de mise à l’échelle Windows. Vérifier alignement sur l’entrée déclenchante et absence de découpe.
5. **Menu Shell natif** : sur un fichier, un dossier et le fond d’un panneau, ouvrir « Plus d’options ». Confirmer l’apparition immédiate de l’indicateur de chargement, l’absence de gel UI, l’arrivée des entrées, l’ouverture des sous-menus et l’exécution d’une action non destructive (p. ex. propriétés). Répéter après fermeture/réouverture rapide et changement de sélection ; vérifier qu’aucun ancien menu ne réapparaît. Tester aussi une cible réseau ou une extension Shell lente si disponible.
6. **Copie/déplacement** : dans les fixtures, copier un fichier, un arbre avec dossiers vides, puis fusionner dans un dossier préexistant avec conflits ignorés et remplacement explicite. Vérifier les octets, les dossiers préexistants, les sauvegardes de remplacement et le comportement des liens symboliques (refus explicite). Annuler puis refaire chaque cas. Pendant une copie de fichier volumineux, annuler et confirmer que source et destination restent intègres.
7. **Historique et changements externes** : après une copie/déplacement, modifier le fichier destination puis tenter Undo ; l’application doit refuser sans l’écraser. Faire aussi un Undo/Redo sans changement extérieur. Tester un conflit apparu dans le chemin destination avant Redo et vérifier que le fichier tiers est préservé et que l’action reste dans l’historique.
8. **Renommage et réglages** : depuis le menu contextuel/F2, vérifier que la popup est conservée. Depuis une ligne, faire deux clics lents sur le texte du nom et tester sélection clavier/souris, Enter, Échap et validation par clic hors du champ ; confirmer qu’un double-clic rapide de dossier navigue et n’active pas le renommage. Renommer vers un nom simple, puis tenter `..`, un séparateur, une cible existante, un nom Windows réservé et un chemin absolu ; aucun fichier ne doit sortir du dossier ni être écrasé, et la collision doit afficher un message compréhensible. Basculer cachés/système avec plusieurs onglets, y compris inactifs, puis les visiter.
9. **Workers** : copier un dossier entièrement conflictuel (zéro tâche), un fichier, puis plusieurs centaines de petits fichiers en mode discret et Turbo. Vérifier que chaque opération se termine, que pause/reprise et annulation répondent, et que la priorité du système revient à la normale après succès comme après erreur.

Une validation réussie de ces scénarios clôt les constats d’interface uniquement pour la configuration Windows testée ; elle ne remplace ni l’essai Linux ni une vérification avec extensions Shell tierces.

## Prompt de reprise suggéré

« Lis AGENTS.md et AUDIT_ROADMAP.md, inspecte les changements locaux sans les écraser, puis implémente les lots dans l’ordre. Commence par les lots 1 et 2 avec tests isolés sur fixtures temporaires. Vérifie les constats dans le code actuel avant chaque modification. Conserve les protections asynchrones existantes, respecte Platform pour les API OS, et documente la validation et les limites de chaque lot. »
