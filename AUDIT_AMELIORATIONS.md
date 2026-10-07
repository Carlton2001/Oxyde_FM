# Corrections et optimisations à traiter après le nettoyage

Audit du 7 octobre 2026, base `3cc7d14`, branche `codex/nettoyage-code-mort`. Ces observations proviennent de la revue du code actuel ; aucune correction n'est appliquée ici. Le nettoyage est documenté séparément dans [AUDIT_CODE_MORT_2.md](AUDIT_CODE_MORT_2.md).

Priorités proposées : P1 pour un risque sur les opérations de fichiers à reproduire en premier, P2 pour les comportements et coûts à examiner ensuite, P3 pour l'entretien. Une chaîne de code établie n'est pas une reproduction utilisateur ; les limites sont indiquées pour chaque entrée.

## A01 — Filtres de date différents des groupes de date — P2

**Constat établi dans le code.** `ui/workspace.rs:155` classe Aujourd'hui/Hier par fenêtres glissantes de 24 heures, et Cette semaine/La semaine dernière par intervalles de jours écoulés. `ui/date_groups.rs:35` utilise les dates locales et les limites des semaines calendaires.

À 00 h 10, un fichier modifié la veille à 23 h 50 correspond au filtre Aujourd'hui tout en appartenant au groupe Hier. Définir des règles communes, puis vérifier minuit, lundi, dates futures et changements d'heure. Aucun scénario interactif reproduit pendant cet audit.

## A02 — Annuler/Rétablir peuvent démarrer plusieurs fois en parallèle — P1

**Risque identifié, à reproduire.** `ui/app/mod.rs:716` et `:755` lancent une tâche de travail à chaque invocation sans réservation ou état d'exécution partagé. `commands/ops.rs:900` lit la dernière transaction, effectue les mutations puis appelle `complete_undo` à la fin (`:1213`). Les verrous d'accès à l'historique ne couvrent pas l'ensemble de l'opération.

Deux demandes rapprochées peuvent sélectionner la même transaction ; une nouvelle transaction peut aussi être ajoutée pendant son exécution. La vérification finale de l'historique intervient après les effets sur les fichiers. Concevoir une sérialisation/réservation des opérations d'historique et son interaction avec la file d'opérations. Commencer par une fixture déterministe de demandes concurrentes ; aucun incident ou perte de données observé ici.

## A03 — Marqueur d'onglet potentiellement périmé sur la zone libre — P2

**Risque identifié dans les handlers, à reproduire.** Le wrapper de la barre (`ui/app/layout.rs:794`) choisit la fin du panneau seulement lorsque `drop_target_pane` change. Après survol d'une limite d'onglet, un déplacement vers la zone libre du même panneau peut conserver l'ancien index. Le `on_drop` du wrapper (`:826`) dépose pourtant à la fin.

Vérifier le passage limite d'onglet → espace libre/bouton +, entre panneaux et dans un même panneau, en tenant compte de la propagation GPUI. Corriger uniquement si le scénario confirme un décalage entre marqueur et insertion. Le témoin corrigé au lot 6 est confirmé par l'utilisateur ; cette observation concerne une transition supplémentaire, non reproduite ici.

## A04 — Propriétés d'un élément de corbeille : nom et emplacement physiques — P2

**Parcours établi, affichage à vérifier.** L'action Propriétés transmet le chemin physique de l'élément supprimé à `get_file_properties`. `commands/io.rs:508` récupère son chemin d'origine et sa date de suppression, mais `ui/app/properties.rs` affiche `name` et `parent` dérivés du chemin physique, sans lire ces deux métadonnées.

Vérifier si le dialogue affiche un nom `$R…` et le dossier interne de corbeille. Adapter la présentation au nom/emplacement d'origine tout en conservant le chemin physique pour les opérations natives. Réserver `FileProperties::original_path` et `deleted_time` jusqu'à cette décision. La restauration sous le nom d'origine a déjà été testée séparément et n'est pas remise en cause par cette observation.

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

## Validation et suite

La compilation Windows actuelle passe sans avertissement du projet. Les observations ci-dessus sont issues de la lecture des sources et de Clippy ; aucun nouveau test de comportement ou benchmark n'a été exécuté pendant cet audit. Snap Layouts et le témoin de déplacement actuel sont confirmés par l'utilisateur. Les validations des lots précédents restent décrites dans [AUDIT_CODE_MORT.md](AUDIT_CODE_MORT.md).

Après le nettoyage, commencer par A02, puis les corrections de comportement A01/A03/A04 et les ressources natives A05/A06. Les optimisations demandent des mesures avant modification. Cet ordre est proposé, pas une liste de corrections déjà autorisées ou réalisées.

### État après les lots N01 à N16

Le nettoyage retire les anciennes sérialisations, les estimations et plusieurs lectures natives dont les résultats étaient abandonnés. Il ne corrige pas les observations A01–A13. Les références de lignes ci-dessus restent celles de `3cc7d14` ; utiliser les noms de fonctions pour retrouver leur nouvel emplacement.

Pour A04, `FileProperties::original_path` / `deleted_time` et leur collecte restent réservés. Pour A05, l'énumération de diagnostic du property store et les lectures de métadonnées d'appareil sans affichage disparaissent ; la récupération du nom convivial et son traitement COM restent actifs. Pour A10, les variantes de tri backend inaccessibles disparaissent, mais le tri naturel actif et les tris GPUI restent distincts. Pour A13, la branche de couleur identique des onglets disparaît avec N05 ; les autres suggestions ne font pas l'objet d'une correction générale.
