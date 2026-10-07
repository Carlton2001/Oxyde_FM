# Corrections et optimisations à traiter après le nettoyage

Audit du 7 octobre 2026, base `3cc7d14`, branche `codex/nettoyage-code-mort`. Ces observations proviennent de la revue du code actuel. Les corrections réalisées ensuite sont indiquées dans les entrées concernées. Le nettoyage est documenté séparément dans [AUDIT_CODE_MORT_2.md](AUDIT_CODE_MORT_2.md).

Priorités proposées : P1 pour un risque sur les opérations de fichiers à reproduire en premier, P2 pour les comportements et coûts à examiner ensuite, P3 pour l'entretien. Une chaîne de code établie n'est pas une reproduction utilisateur ; les limites sont indiquées pour chaque entrée.

## A01 — Filtres de date différents des groupes de date — P2

**Corrigé le 7 octobre 2026, après reproduction déterministe.** Un test à 00 h 10 avec un fichier modifié la veille à 23 h 50 échouait avant correction : le filtre Hier l'excluait, alors que le regroupement le classait dans Hier. Les fenêtres glissantes de 24 heures ont été retirées.

Le filtrage de `TabState`, les compteurs du menu, l'indicateur de filtre et Tout sélectionner utilisent désormais `DateGroup`, déjà employé par le regroupement. Les jours suivent le calendrier local, les semaines commencent le lundi, et la priorité des périodes reste Aujourd'hui, Hier, Cette semaine, La semaine dernière, Ce mois-ci, Plus ancien. Ce mois-ci est ajouté au filtre, avec les traductions existantes ; seules les périodes présentes dans le dossier sont proposées. Les dates futures restent dans Aujourd'hui, comme dans le regroupement existant ; les timestamps invalides rejoignent Plus ancien. Une date de référence unique est utilisée pendant chaque parcours du cache ou des compteurs.

Validation : le test de minuit passe après correction. Les fixtures couvrent lundi, frontières de mois et d'année, jours de 23/25 heures avec décalages explicites, conversion du timestamp vers le jour local, dates futures/invalides, compteurs des six périodes, Tout sélectionner, ordre des fichiers et combinaison avec le filtre de nom. Les cas de décalage horaire ne valident pas la base de règles de fuseau du système. Aucun Computer Use ni Linux. Le rafraîchissement d'un filtre déjà ouvert lors d'un changement de jour reste à examiner en A15.

## A02 — Annuler/Rétablir peuvent démarrer plusieurs fois en parallèle — P1

**Corrigé le 7 octobre 2026, après reproduction déterministe.** Avant correction, sélectionner la dernière transaction, ajouter une nouvelle entrée puis valider Annuler fait échouer `complete_undo`. Le test de régression a échoué sur cette assertion avant modification. Deux workers pouvaient également sélectionner la même transaction sans réservation commune.

`HistoryManager::begin_undo` / `begin_redo` réservent désormais une transaction jusqu'à la fin du traitement. Une seconde demande est refusée avant les effets sur les fichiers. Les ajouts d'historique arrivant pendant ce traitement sont différés, puis intégrés dans leur ordre d'arrivée ; une nouvelle action invalide Rétablir après la validation de la transaction réservée. Le verrou du modèle reste libéré pendant les opérations de fichiers. La réservation se libère aussi sur erreur ou panique du worker, et le nettoyage attend sa fin pour préserver les sauvegardes utilisées.

GPUI partage un état d'exécution entre Annuler et Rétablir : les clics et raccourcis répétés sont ignorés pendant le travail, les deux boutons sont désactivés, et l'état est rétabli au retour du worker, y compris sur erreur. Le rejeu de l'ancienne API de restauration avait été adapté, puis cette chaîne sans appelant GPUI a été retirée après revalidation d'A14. La sortie finale du processus et le délai d'arrêt Tokio restent ceux documentés dans `SHUTDOWN_DIAGNOSTICS.md` ; si un worker dépasse ce délai à la fermeture, son nettoyage différé reste soumis à cette limite existante.

Validation : demandes concurrentes sur deux threads, entrées ajoutées pendant Annuler et Rétablir, abandon/erreur/panique, ordre et limite de 100 entrées, nettoyage différé des sauvegardes. Une fixture de renommage vérifie le refus avant mutation, puis l'aller-retour Annuler/Rétablir et la conservation du contenu. Ces contrôles portent sur le modèle et les commandes ; aucune validation interactive GPUI ni Linux.

**Limite de portée :** cette réservation protège l'historique ; elle ne réserve pas les chemins auprès du planificateur de copies/déplacements. La coordination des effets physiques d'une opération de fichiers indépendante avec Annuler/Rétablir reste à examiner séparément. L'observation A14 a ensuite été requalifiée : son API n'avait pas d'appelant GPUI et a été retirée.

## A03 — Marqueur d'onglet potentiellement périmé sur la zone libre — P2

**Corrigé le 7 octobre 2026, après reproduction déterministe du handler.** Une fixture entre panneaux sélectionne une limite d'onglet, puis passe à la zone libre du même panneau. Avant correction, le test obtient l'index 1 au lieu de l'index 3, alors que le dépôt du wrapper insère réellement à la fin.

Le wrapper et les deux moitiés d'onglet mettent désormais à jour la cible dans `on_drag_move`, avant la construction du rendu, au lieu de modifier cet état dans `drag_over`. Le wrapper choisit la fin à chaque événement dans la barre ; la moitié d'onglet survolée affine ensuite cet index. L'ordre de dispatch Capture (parent avant enfants) a été vérifié dans les sources de la révision GPUI utilisée par Cargo (`33c9585`). Les zones d'onglet sont limitées au viewport défilant pour ignorer leurs portions masquées en débordement. Les mouvements sans effet effacent les deux champs de ciblage, et les changements notifient GPUI depuis l'événement.

Validation : quatre tests couvrent limite → zone libre/bouton +, correspondance avec l'insertion réelle entre panneaux et dans un même panneau, priorité des moitiés d'onglet, déplacements sans effet et zones masquées par le défilement. Les marqueurs visuels existants et la position avant le bouton + restent conservés ; aucun nouveau témoin ou changement de curseur. Aucun Computer Use ni validation visuelle interactive. La validation automatique porte sur les calculs et transitions de cible, pas sur un rendu de fenêtre GPUI simulé.

## A04 — Propriétés d'un élément de corbeille : nom et emplacement physiques — P2

**Corrigé le 7 octobre 2026, après reproduction déterministe de la présentation.** Avant correction, une fixture de fichier supprimé affiche `$Rfixture` au lieu de son nom d'origine. Le dialogue affichait également le dossier physique de corbeille sans lire `original_path` et `deleted_time`.

La présentation utilise désormais le nom et le dossier du chemin d'origine, avec les libellés existants Emplacement d'origine et Date de suppression. Les métadonnées partagent les règles de nommage de la liste et de la restauration : lecture du fichier `$I` via Platform et conservation des extensions `.lnk` / `.url` lorsque le Shell les masque. Le chemin physique reste utilisé pour les icônes natives, les calculs de taille et les actions système. En l'absence d'un chemin d'origine absolu exploitable, le dialogue conserve la présentation physique.

La date de suppression du crate `trash`, exprimée en secondes Unix, est convertie en millisecondes comme les autres dates du modèle et le formateur. Les dates négatives ou débordantes ne sont pas publiées comme dates valides. La largeur des libellés tient compte des deux lignes supplémentaires seulement lorsqu'elles sont présentes, avec les traductions existantes dans les cinq langues.

Validation : six nouveaux tests couvrent l'identité affichée, la conservation du chemin physique, dossiers et raccourcis, le repli sans métadonnées, les unités et dates invalides. Une fixture locale Windows `$I` vérifie le nom Unicode d'origine malgré un libellé Shell différent. Aucun élément n'est envoyé à la corbeille par ces tests ; aucune validation visuelle interactive, aucun Computer Use ni Linux.

## A05 — Durée de vie des ressources COM/Shell — P2

**Points de code établis, conséquences à mesurer.** `platform/network.rs:94` et `commands/io.rs:313` ignorent le résultat de `CoInitializeEx` puis appellent `CoUninitialize` sans condition. Des parcours analogues existent dans les commandes système. Revoir l'équilibre initialisation/désinitialisation, les erreurs et l'ordre de destruction des objets avec un garde commun conforme aux contrats COM.

Les noms et chemins issus de `IShellItem::GetDisplayName` (`network.rs:115`, `:122`) sont convertis sans libération explicite du buffer ; le parcours d'accès rapide dans `commands/system.rs` libère les buffers correspondants. Auditer ces allocations, ainsi que les valeurs obtenues depuis le property store, avant de conclure sur l'ampleur des fuites. Aucun profil mémoire effectué. Refaire cette revue après N09/N10, qui peuvent retirer une partie des appels.

## A06 — Presse-papiers : allocations et erreurs du format Couper — P2

**Branches d'erreur identifiées, non injectées.** `platform/clipboard.rs:153–207` et `:232–249` allouent de la mémoire globale puis peuvent sortir sur échec de verrouillage ou publication sans libération locale. La publication du format d'effet de dépôt ignore son résultat (`:207`), ce qui pourrait perdre l'intention Couper alors que les chemins sont publiés.

Revoir la propriété des buffers et les erreurs de chaque publication, avec un garde qui transfère la propriété uniquement au moment approprié. Prévoir des simulations d'échec et une vérification Copier/Couper native. Ne pas modifier les formats ni le comportement nominal pendant un simple nettoyage.

## A07 — API Windows encore dans les commandes — P2

**Dette d'architecture établie.** Plusieurs traitements Shell/COM restent dans `commands/io.rs`, `commands/system.rs` et `commands/ops.rs`. Certaines méthodes de plateforme délèguent encore à ces commandes. Le filtre de dossiers système de `commands/sidebar.rs` connaît également des détails Windows.

Déplacer les appels natifs actifs derrière `Platform` et les modules Windows, selon AGENTS.md, en conservant les résultats et erreurs. Après retrait des branches mortes, établir une liste précise des responsabilités restantes. Linux reste hors validation pour le moment ; conserver ses conditions de compilation et ses interfaces.

## A08 — Annulation de recherche et détection SSD — P2

**Limite structurelle établie, latence non mesurée.** La recherche avancée contrôle son annulation entre les entrées. Les lecteurs de contenu texte et Office (`commands/search.rs:800`, `:886`, `:918`) n'ont pas de paramètre d'annulation pour arrêter une lecture en cours. Un fichier volumineux peut retarder l'arrêt du travail après fermeture/annulation de la recherche.

Propager l'annulation jusqu'aux boucles de lecture et vérifier un arrêt pendant un gros fichier texte et une archive Office. Le seul appel de `file_contains_content` passe aussi `ssd_hint = false` (`:200`) : la branche permettant d'éviter le verrou de disque sur SSD n'est jamais choisie. Décider de raccorder une détection effective ou de simplifier ce paramètre ; ne pas activer la concurrence SSD sans mesure.

## A09 — Calcul des tailles : tâches et pools cumulés — P2

**Organisation établie, coût à mesurer.** `ui/app/sizes.rs:26` crée un travail bloquant pour chaque chemin à calculer. `commands/io.rs:603` crée un pool jwalk par calcul, avec davantage de threads sur SSD. Le parcours ne reçoit pas de signal d'annulation interne ; abandonner la tâche GPUI ne garantit pas l'arrêt des travaux bloquants déjà lancés.

Mesurer un dossier contenant beaucoup de sous-dossiers, puis une navigation rapide entre dossiers. Envisager une limite globale de concurrence, un pool partagé et une annulation propagée. Garder les calculs hors du thread UI et préserver le cache et les mises à jour des résultats.

## A10 — Tri et normalisation répétés — P2

**Calculs répétés établis, gain à mesurer.** `commands/io.rs:140` trie par nom naturel ; `ui/workspace.rs` retrie les listes et normalise les noms dans les comparateurs/filtres. La complétion effectue également son propre tri. Les règles de comparaison ne sont pas toutes identiques.

Clarifier les règles attendues pour noms numériques, accents, dossiers et ordre descendant avant factorisation. Préparer les clés normalisées une fois par entrée/filtre, et éviter les tris répétés lorsqu'un consommateur partage réellement le même ordre. Le tri backend reste utile au fil d'Ariane et ne doit pas être supprimé aveuglément.

## A11 — Copie de l'historique pendant le rendu — P2

**Coût structurel établi, impact non mesuré.** `ui/components/top_bar.rs:121` appelle `HistoryManager::get_state`, qui clone les piles de transactions et leurs détails (`models/history.rs:102`). Le rendu peut être répété pour chaque panneau, alors que les boutons n'ont besoin que d'un résumé de l'état et des transactions disponibles.

Exposer un petit état de présentation plutôt que recopier les chemins et journaux de fichiers de tout l'historique. Mesurer le rendu avec un historique chargé et plusieurs panneaux ; conserver un accès de test à l'état détaillé si nécessaire.

## A12 — Polling et notifications sans changement — P2

**Parcours établis, fréquence utile à mesurer.** `ui/app/shell_features.rs` réévalue régulièrement Peek et notifie l'interface après chaque résultat, même identique. La surveillance du presse-papiers (`ui/app/mod.rs:2268`) lit périodiquement ses chemins pour déduire la disponibilité du collage.

Notifier uniquement lors d'un changement pertinent. Examiner un indicateur natif de présence de fichiers ou de changement du presse-papiers pour éviter la lecture répétée complète, en gardant les formats nécessaires aux actions actuelles. Comparer coûts et réactivité avant remplacement du polling.

## A13 — Suggestions Clippy et simplifications locales — P3

`cargo clippy --all-targets --offline --message-format=json` termine avec succès. Ses diagnostics comprennent des doublons entre cibles et des suggestions concernant uniquement les tests ; ils ne signalent pas tous du code mort ni des défauts.

Exemples à revoir après les retraits : emprunts inutiles dans `commands/duplicates.rs` et `conflict_dialog.rs`, copies clonées dans `file_list.rs` et `tooltip.rs`, comparaisons avec allocation dans `commands/ops.rs`, casts superflus dans `ui/app/sizes.rs`, remplacement de caractères dans `input.rs`, organisation de modules de tests. La branche de couleur identique des onglets est déjà recensée en N05.

Traiter les simplifications utiles dans des changements limités ; ne pas convertir automatiquement tous les diagnostics ni modifier les tests uniquement pour satisfaire une règle stylistique.

## A14 — Rétablir une restauration déplacée depuis la corbeille — P2

**Requalifié le 7 octobre 2026 : chaîne sans appelant dans l'application, retirée après A14.** La relecture des appelants établit que `move_from_trash` était invoqué uniquement par le test natif ajouté en A14. GPUI utilise `execute_coller` → `execute_file_move` → le gestionnaire `file_ops`, qui ne produit aucune transaction Restore. L'action Restaurer passe directement par `restore_items`.

Le commit A14 `b8c7523` validait le rejeu de cette API isolée, pas une fonctionnalité raccordée à GPUI. La variante Restore, `move_from_trash`, le journal et ses tests dédiés, ses nouvelles méthodes Platform et son libellé traduit d'historique sont donc retirés dans le lot suivant. Les tests et gardes d'A02 pour les opérations actives restent conservés. La restauration ordinaire et ses deux fixtures natives restent actives.

L'affirmation initiale d'un défaut Annuler/Rétablir de restauration dans l'application était incorrecte. Le parcours réel de Couper/Coller depuis la corbeille fait l'objet de l'observation distincte A18 ; aucune nouvelle intégration de l'ancienne API n'est ajoutée implicitement.

## A15 — Cache de filtrage lors d'un changement de jour — P2

**Corrigé le 7 octobre 2026, avec validation déterministe.** Le cache de chaque onglet mémorise désormais le jour local, le décalage UTC et la signature des règles du fuseau Windows utilisés lors de sa construction. Une tâche dédiée vérifie cette référence toutes les cinq secondes, indépendamment du watcher de fichiers et du presse-papiers, et prépare uniquement les caches avec un filtre Date actif et une référence différente.

Les règles de filtrage sont partagées avec le calcul ordinaire du cache. Le recalcul calendaire s'effectue sur un worker à partir d'un snapshot des fichiers et critères ; chaque réponse conserve l'identifiant de l'onglet et la révision de son cache. Une modification de filtre, de tri, une navigation ou une réception de fichiers invalide les anciennes réponses. Un changement de référence pendant le calcul fait aussi abandonner les résultats. Les onglets inactifs et ceux déplacés entre panneaux sont traités, les onglets fermés sont ignorés.

Les fichiers ne sont pas relus, et les filtres, sélection, position de défilement et mesures des colonnes sont conservés. Les onglets regroupés sans filtre Date demandent uniquement un nouveau rendu lorsque la référence change. Aucune notification périodique si la référence et les caches sont déjà à jour. Un retour de veille ou un saut d'horloge est pris en compte à la prochaine vérification ; la conversion des dates continue d'utiliser Chrono et le fuseau natif. Les menus reprennent leurs compteurs lors de leur ouverture, comme auparavant.

Validation : six fixtures déterministes couvrant minuit dans plusieurs panneaux et onglets actifs/inactifs, sauts d'horloge, changements de décalage et de signature de fuseau, résultats obsolètes, fermeture/déplacement d'onglet, conservation de l'état et absence de recalcul sans filtre Date. Aucun Computer Use ni modification de l'horloge ou du fuseau de la machine ; Linux reste hors validation.

## A16 — Conflits et espace temporaire du déplacement initial depuis la corbeille — P1

**Clos par suppression de la chaîne inutilisée, après revalidation des appelants.** Les conflits et sauvegardes partagées recensés appartenaient à `move_from_trash`, sans appelant de production GPUI. La fonction et sa chaîne Restore sont retirées avec la correction de qualification A14. Il n'y a donc pas de nouvel espace temporaire ou de nouveau dialogue de conflit à implémenter pour cette API. Le parcours actif de Couper/Coller doit être examiné séparément en A18.

## A17 — Énumérations de corbeille lors du rejeu des restaurations — P2

**Clos par retrait du journal A14 inutilisé.** Les énumérations par effet appartenaient au journal d'une API sans appelant de l'application. Ce journal a été supprimé ; aucun benchmark ou optimisation n'est nécessaire pour ce parcours retiré.

## A18 — Couper/Coller depuis la corbeille dans le parcours GPUI actuel — P1

**Parcours de code établi, à reproduire séparément sur Windows.** `copy_files_to_clipboard` publie les chemins physiques sélectionnés, `execute_coller` les transmet au gestionnaire générique, et `perform_copy` construit la destination depuis `source.file_name()`. Ce parcours ne consulte ni le nom d'origine de la corbeille ni son fichier `$I`, et ne passe pas par `restore_items`. Le nom `$R…` et la cohérence des métadonnées de corbeille après déplacement sont donc à vérifier.

Reproduire Couper/Coller sur des fichiers et dossiers temporaires, depuis GPUI et avec l'Explorateur Windows, avant de choisir l'intégration native adaptée. Vérifier noms, conflits, métadonnées et comportement de l'historique. Ne pas réintroduire automatiquement `move_from_trash` : les défauts de cette ancienne API ne décrivaient pas les effets du gestionnaire actuel. Cette observation est consignée pour le lot suivant ; aucun scénario supplémentaire n'a été exécuté sur des fichiers utilisateur.

## Validation et suite

La compilation Windows actuelle passe sans avertissement du projet. Les observations ci-dessus sont issues de la lecture des sources et de Clippy ; aucun nouveau test de comportement ou benchmark n'a été exécuté pendant cet audit. Snap Layouts et le témoin de déplacement actuel sont confirmés par l'utilisateur. Les validations des lots précédents restent décrites dans [AUDIT_CODE_MORT.md](AUDIT_CODE_MORT.md).

Après A01/A02/A03/A04 et A15, examiner le parcours actif A18 et les ressources natives A05/A06. Les optimisations demandent des mesures avant modification. Cet ordre est proposé, pas une liste de corrections déjà autorisées ou réalisées.

### État après les lots N01 à N16

Le nettoyage retire les anciennes sérialisations, les estimations et plusieurs lectures natives dont les résultats étaient abandonnés. Ce nettoyage ne corrige pas les observations A01–A13. A01, A02, A03, A04 et A15 sont traités dans les lots suivants ; A14/A16/A17 ont été requalifiés après relecture des appelants, puis leur chaîne inutilisée a été retirée. Ces suites sont décrites plus haut. Les références de lignes ci-dessus restent celles de `3cc7d14` ; utiliser les noms de fonctions pour retrouver leur nouvel emplacement.

Depuis A04, `FileProperties::original_path` / `deleted_time` et leur collecte alimentent les propriétés des éléments de corbeille. Pour A05, l'énumération de diagnostic du property store et les lectures de métadonnées d'appareil sans affichage disparaissent ; la récupération du nom convivial et son traitement COM restent actifs. Pour A10, les variantes de tri backend inaccessibles disparaissent, mais le tri naturel actif et les tris GPUI restent distincts. Pour A13, la branche de couleur identique des onglets disparaît avec N05 ; les autres suggestions ne font pas l'objet d'une correction générale.

### Validation du lot A02

`cargo test --all-targets --offline` : 101 tests backend et 89 tests du binaire passent, soit 190 ; les trois tests natifs déjà ignorés restent ignorés. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Neuf tests de régression ont été ajoutés, et le test existant de déplacement de l'historique a été adapté à la réservation. Aucun Computer Use et aucune validation Linux.

### Validation du lot A01

`cargo test --bin oxyde --offline` : 95 tests passent, dont six nouveaux tests de régression. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Les validations portent sur les règles de calcul et les parcours de filtrage ; aucune validation visuelle interactive, aucun Computer Use ni Linux.

### Validation du lot A03

`cargo test --bin oxyde --offline` : 99 tests passent, dont quatre nouveaux tests de régression. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Aucun Computer Use ni test visuel de fenêtre GPUI ; Linux reste hors validation.

### Validation du lot A04

`cargo test --all-targets --offline` : 104 tests backend et 102 tests du binaire passent, soit 206 ; les trois tests natifs déjà ignorés restent ignorés. Six tests de régression ont été ajoutés. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Les traductions des deux libellés existent dans les cinq catalogues. Aucun Computer Use, aucune validation visuelle interactive ni Linux.

### Validation historique du lot A14 — API isolée, retirée ensuite

Ces résultats valident l’API isolée testée dans `b8c7523`, sans prouver son usage par GPUI. Ses huit tests dédiés sont retirés avec cette chaîne ; les trois tests natifs antérieurs restent conservés.

`cargo test --all-targets --offline` : 111 tests backend et 102 tests du binaire passent, soit 213. Quatre tests natifs restent ignorés dans cette invocation ; le nouveau test `native_restore_history_roundtrip_and_conflict` est exécuté explicitement avec `--ignored` et passe, soit 214 tests distincts validés dans le lot. Les trois anciens tests natifs ignorés n'ont pas été relancés. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Aucun Computer Use ni Linux ; aucun fichier utilisateur n'est utilisé par les fixtures natives.

### Validation du retrait de la chaîne Restore et du lot A15

`cargo test --all-targets --offline` : 104 tests backend et 108 tests du binaire passent, soit 212 ; les trois tests natifs antérieurs restent ignorés. Les huit tests du journal A14 isolé disparaissent avec sa chaîne, et six tests A15 sont ajoutés. Les tests des catalogues et de l'historique actif passent. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. La vérification des appelants confirme que la tâche A15 est démarrée par `start_watcher_task`, appelé à l'initialisation de la fenêtre principale. Aucun Computer Use ni validation Linux, aucune modification du fuseau ou de l'horloge système.
