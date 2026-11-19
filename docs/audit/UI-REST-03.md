# Audit UI-REST-03

## Fichiers lus en entier

- [x] `src/ui/style/cards.rs`
- [x] `src/ui/theme.rs`
- [x] `src/ui/theme/file_colors.rs`
- [x] `src/ui/theme/legibility.rs`
- [x] `src/ui/theme/seed.rs`
- [x] `src/ui/theme/states.rs`
- [x] `src/ui/window_geometry.rs`
- [x] `src/ui/workspace.rs`
- [x] `src/ui/workspace/filters.rs`
- [x] `src/ui/workspace/filters/calendar_refresh_tests.rs`
- [x] `src/ui/workspace/filters/criteria.rs`
- [x] `src/ui/workspace/filters/tests.rs`
- [x] `src/ui/workspace/folder_sizes.rs`
- [x] `src/ui/workspace/operations.rs`
- [x] `src/ui/workspace/preparation.rs`
- [x] `src/ui/workspace/presentation.rs`
- [x] `src/ui/workspace/sidebar.rs`
- [x] `src/ui/workspace/snapshots.rs`
- [x] `src/ui/workspace/sorting.rs`
- [x] `src/ui/workspace/tab.rs`
- [x] `src/ui/workspace/tab/prepared_sort_tests.rs`
- [x] `src/ui/workspace/tab/scroll.rs`
- [x] `src/ui/workspace/tree.rs`
- [x] `src/ui/workspace/viewer.rs`

## Constats

Aucun constat critique, élevé ou moyen. La file des tailles de dossiers, les révisions de préparation, le cache de présentation, le tri naturel et la géométrie des fenêtres ont été vérifiés : les résultats périmés sont bien écartés (identifiant, chemin, révision, identité du pointeur de la liste, génération).

### Faible

1. `src/ui/workspace/preparation.rs:139` — faible — plausible.
   L'application d'une préparation remplace la liste mais ne retire ni de la sélection, ni de l'ancre, ni du curseur clavier les chemins qui ont disparu du disque. Aucun nettoyage de la sélection n'existe ailleurs dans `src/ui` (seulement des effacements complets à la navigation).
   Scénario : un fichier sélectionné est supprimé par un autre programme, le watcher recharge la liste ; le chemin reste dans la sélection. Un glisser-déposer construit sa charge directement depuis cette sélection (`file_list/details_row.rs:204`), donc il emporte un chemin inexistant, et les comptes « une seule sélection » (`item_pointer.rs:61`, `shell_features.rs:64`) sont faussés.

2. `src/ui/workspace/filters.rs:335` — faible — confirmé.
   La demande de rafraîchissement calendaire est ignorée quand seules des plages personnalisées (date ou suppression) sont actives, alors que leur évaluation dépend du fuseau local (`criteria.rs:59`).
   Scénario : un filtre « du 1er au 15 » est actif, le fuseau ou l'heure d'été change pendant que l'application tourne ; la liste filtrée en cache n'est pas recalculée tant qu'une autre action ne force pas le cache.

3. `src/ui/workspace/filters.rs:151` — faible — plausible.
   Dans la corbeille, un motif de recherche glob invalide (par exemple un crochet non fermé avec un joker) ne retient aucune entrée, alors que le filtre de nom traite un motif invalide comme « ne filtre rien » (`criteria.rs:90-93`).
   Scénario : l'utilisateur tape un motif en cours de saisie dans la recherche de la corbeille ; la liste se vide puis réapparaît, sans indication d'erreur.

4. `src/ui/workspace/snapshots.rs:22` — faible — confirmé.
   Les piles précédent et suivant sont sauvegardées entièrement dans l'espace de travail, et rien ne les borne (`app/navigation.rs:249` et `:316` ajoutent sans limite, aucune troncature n'existe dans `src/ui/app`). À la restauration (`snapshots.rs:61-62`) elles sont reprises sans validation.
   Scénario : une session longue avec des milliers de navigations par onglet fait grossir le fichier de configuration et la mémoire, et chaque sauvegarde de l'espace de travail resérialise tout.

5. `src/ui/workspace/snapshots.rs:82` — faible — plausible.
   Un seul volet dont tous les onglets ont un chemin vide rend `None` pour toute la disposition (collecte en `Option`), donc la restauration entière est abandonnée au profit d'un espace de travail neuf, qui écrasera l'ancien à la prochaine sauvegarde.
   Scénario : un fichier de configuration partiellement corrompu ou édité à la main avec un volet vide fait perdre tous les autres onglets et volets.

6. `src/ui/workspace/snapshots.rs:52` — faible — plausible.
   La restauration ne vérifie ni l'unicité des identifiants d'onglet et de volet, ni l'invariant « au plus un Viewer, seul dans son volet » que `viewer.rs:73` impose pourtant aux déplacements.
   Scénario : un fichier contenant deux onglets de même identifiant fait que la recherche par identifiant (résultats de listing, de préparation, de rafraîchissement calendaire) touche toujours le premier, l'autre onglet ne recevant jamais ses résultats.

7. `src/ui/workspace/operations.rs:6` (et `snapshots.rs:127`) — faible — confirmé.
   Le constructeur de l'espace de travail énumère les volumes et lit les raccourcis rapides de façon synchrone au démarrage, sur le thread UI. Lors d'une restauration, ce constructeur est appelé uniquement pour être ensuite écrasé, et la barre latérale est reconstruite deux fois (`snapshots.rs:159`, puis `app/mod.rs:459`).
   Scénario : un lecteur réseau inaccessible ou un service Shell lent retarde l'affichage de la première fenêtre ; le travail est fait même quand la disposition vient de la sauvegarde.

8. `src/ui/workspace/operations.rs:64` — faible — plausible.
   À la fermeture d'un volet actif, le volet de remplacement est choisi sans écarter un Viewer s'il est le seul restant, et `keep_explorer_active` n'est appelé ni par cette fonction ni par `close_tab` (`app/navigation.rs:393`).
   Scénario : un explorateur et un Viewer sont ouverts, l'utilisateur ferme l'explorateur ; le Viewer devient volet actif, alors que le reste de l'application suppose qu'il ne l'est jamais (`app/keyboard.rs:5` désactive alors les commandes d'explorateur).

### Optimisation

9. `src/ui/workspace/preparation.rs:41` — optimisation — confirmé.
   Chaque ajout par lots (résultats de recherche rapide, `app/search.rs:911`, et corbeille, `trash_loading.rs:44`) recopie toute la liste existante (la source reste détenue par la requête, donc la copie sur écriture clone), relance la résolution des types natifs sur tous les fichiers (appels Shell sous Windows, cache recréé à chaque appel), retrie, refiltre et reconstruit les libellés de type et de taille.
   Scénario : une recherche renvoyant 100 000 résultats par lots de quelques centaines coûte un nombre de lots fois la taille de la liste, soit un comportement quadratique, avec des appels Shell répétés par extension à chaque lot.

10. `src/ui/workspace/preparation.rs:37` — optimisation — confirmé.
    Pour un listing ou un remplacement, la liste fournie est déjà détenue par la mise à jour conservée dans la requête, puis dans le résultat ; la modification en place force donc un clone profond de tous les `FileEntry`, et la liste d'origine reste en mémoire jusqu'à l'application.
    Scénario : un dossier de 200 000 entrées double sa consommation mémoire et son temps de préparation au chargement.

11. `src/ui/workspace/preparation.rs:65` — optimisation — confirmé.
    Chaque lot de mesures de tailles de dossiers (jusqu'à 128 résultats, `folder_sizes.rs:219`) clone toute la liste, la retrie (même si le tri n'est pas sur la taille), la refiltre et recalcule les ensembles de colonnes.
    Scénario : l'histogramme de tailles est actif dans un dossier de 50 000 entrées dont des milliers de sous-dossiers sont mesurés en continu ; chaque publication refait tout le travail.

12. `src/ui/workspace/filters.rs:180` — optimisation — confirmé.
    `column_filter_flags` est appelé à chaque rendu de l'en-tête (`file_list/headers.rs:37`) et du menu contextuel de la grille (`context_menu/grid.rs:109`). Avec un filtre de date actif, il lit l'heure locale et compte toute la liste à chaque image ; avec un filtre de type, il alloue un libellé par fichier tant que tous les fichiers sont retenus.
    Scénario : un filtre de date sur un dossier de 100 000 entrées coûte un parcours complet et un tableau de comptes par image affichée.

13. `src/ui/workspace/filters.rs:320` — optimisation — confirmé.
    `update_cache_on` filtre la liste entière et recalcule les largeurs de colonnes (un libellé alloué par fichier) de façon synchrone ; il est appelé sur le thread UI depuis `app/rename.rs:539` et `app/search.rs:848`.
    Scénario : la création d'un élément dans un très gros dossier efface les filtres de colonnes et fige brièvement l'interface le temps du parcours.

14. `src/ui/workspace/filters/criteria.rs:87` — optimisation — confirmé.
    La validation d'une expression régulière de nom compile l'expression à chaque rendu du panneau de filtre (`context_menu_render/column_filter.rs:209`), en plus de sa compilation par le filtrage.
    Scénario : une expression volumineuse mais valide (classes Unicode répétées) saisie dans le panneau coûte une compilation à chaque image tant que le panneau est ouvert.

15. `src/ui/theme/states.rs:118` — optimisation — plausible.
    La recherche des remplissages de ligne imbrique 41 forces de sélection, jusqu'à 17 forces d'histogramme et, pour chacune, une recherche de survol d'environ 3 000 évaluations perceptuelles, répétée pour 11 mélanges d'accent. Quand la cible de couleur du survol n'est jamais atteinte (accent gris, donc plafond de chroma proche de zéro, avec un quotient potentiellement non fini ou NaN à `:128`), tout l'espace est parcouru avant le repli. Le calcul est refait à chaque déplacement de curseur dans l'éditeur de thème, sur le thread UI (`app/theme_window/draft.rs:205`).
    Scénario : un thème personnalisé à accent gris ou très pâle fait saccader le sélecteur de couleur pendant le glissement.

<!-- LOT TERMINÉ -->
