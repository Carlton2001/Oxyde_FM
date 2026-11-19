# Audit UI-APP-07

## Fichiers lus en entier

- [x] `src/ui/app/search.rs`
- [x] `src/ui/app/search/content_warnings.rs`
- [x] `src/ui/app/selection.rs`
- [x] `src/ui/app/shell_features.rs`
- [x] `src/ui/app/shutdown.rs`
- [x] `src/ui/app/sizes.rs`
- [x] `src/ui/app/system_menus.rs`
- [x] `src/ui/app/theme_window.rs`
- [x] `src/ui/app/theme_window/draft.rs`
- [x] `src/ui/app/theme_window/editor.rs`
- [x] `src/ui/app/theme_window/list.rs`
- [x] `src/ui/app/theme_window/render.rs`
- [x] `src/ui/app/toasts.rs`
- [x] `src/ui/app/tool_geometry.rs`
- [x] `src/ui/app/trash_settings.rs`
- [x] `src/ui/app/trash_settings/actions.rs`
- [x] `src/ui/app/trash_settings/confirmation.rs`
- [x] `src/ui/app/trash_settings/deletion_policy.rs`
- [x] `src/ui/app/trash_settings/linux.rs`

## Constats

### Élevé

1. `src/ui/app/trash_settings/actions.rs:70` (purge réelle lignes 16-22) — élevé, confirmé.
   Les purges définitives de la corbeille n'ont aucune confirmation et ignorent la préférence de confirmation de suppression.
   Scénario : dans la vue corbeille, la touche Suppr purge la sélection aussitôt, car `execute_supprimer` renvoie vers `execute_trash_action` avant le test de confirmation. Le menu contextuel « supprimer définitivement » et « tout supprimer » (`context_menu.rs:380`) et le bouton de barre (`top_bar.rs:260`) font de même. Un Ctrl+A suivi de Suppr, ou un clic malheureux sur « tout supprimer », vide la corbeille de façon irréversible.

### Moyen

2. `src/ui/app/trash_settings/linux.rs:262` — moyen, confirmé.
   Le bouton « tout supprimer » de la fenêtre de propriétés vide toute la corbeille sans confirmation (même chemin dans la variante Windows).
   Scénario : un clic dans les propriétés de la corbeille appelle directement `empty_trash`. Rien ne distingue l'action d'un simple affichage, et la suppression est définitive.

3. `src/ui/app/shell_features.rs:94` — moyen, plausible.
   `execute_ouvrir` ouvre chaque élément sélectionné sans plafond ni confirmation.
   Scénario : Ctrl+A dans un dossier de plusieurs centaines de fichiers, puis Entrée, lance autant de processus ou d'applications externes. Pour plusieurs dossiers, chaque ouverture navigue à son tour et seul le dernier reste affiché.

### Faible

4. `src/ui/app/selection.rs:295` — faible, plausible.
   `select_all` sélectionne aussi les fichiers des groupes repliés, alors que la sélection par plage et le rectangle ne prennent que les lignes visibles.
   Scénario : groupement par date avec un groupe replié, Ctrl+A puis Suppr. Des éléments invisibles sont supprimés. Seul le compte de la confirmation le laisse deviner.

5. `src/ui/app/selection.rs:55` (calcul de la cible ligne 354) — faible, confirmé.
   Si le curseur clavier ou l'ancre est dans un groupe replié, `arrow_target` ne le retrouve pas dans les lignes affichées et renvoie le premier fichier.
   Scénario : sélectionner un fichier, replier son groupe, appuyer sur flèche bas. La sélection saute en haut de la liste.

6. `src/ui/app/search.rs:873` — faible, plausible.
   Le retour anticipé quand `handle` est absent arrive après le démarrage du worker et la mise à jour de l'état (fichiers vidés, statut en chargement).
   Scénario : si `handle` vaut `None`, l'onglet reste en chargement sans suivi du worker. Le test devrait précéder les effets de bord.

7. `src/ui/app/search.rs:911` — faible, plausible.
   Si `prepare` renvoie false pour une autre raison qu'un résultat périmé (échec du `spawn_blocking`, sémaphore fermé), la boucle s'arrête sans appliquer l'événement terminal.
   Scénario : un échec de préparation pendant une recherche laisse l'onglet en chargement et la bannière « en cours » jusqu'à un clic sur Arrêter.

8. `src/ui/app/shell_features.rs:182` — faible, plausible.
   `Instant::now() - Duration::from_secs(10)` panique si l'horloge monotone a moins de 10 s d'âge, car le temps vient du démarrage de la machine.
   Scénario : lancement d'Oxyde dans les 10 premières secondes après le démarrage (démarrage automatique très rapide). Il faudrait `checked_sub`.

9. `src/ui/app/shell_features.rs:20` (extraction : `toasts.rs:186`) — faible, plausible.
   Un fichier tiré d'une archive est ouvert par `open_item` sans avertissement ni marque d'origine. Je n'ai pas trouvé de propagation de marque d'origine web dans ce chemin.
   Scénario : double-clic sur un `.exe`, `.bat` ou `.lnk` dans un zip téléchargé. Il s'exécute directement, alors que l'Explorateur appliquerait SmartScreen.

10. `src/ui/app/system_menus.rs:26` — faible, plausible.
    Chaque appel lance un `spawn_blocking` indépendant, sans sérialisation ni fusion.
    Scénario : deux rafraîchissements rapprochés (démarrage plus changement des épinglés, ou de langue). Ils s'exécutent en parallèle sur la liste de raccourcis de la barre des tâches et le registre. Le plus ancien peut finir en dernier et imposer un menu périmé. Je n'ai trouvé aucun verrou dans le module de jump list.

11. `src/ui/app/toasts.rs:107` — faible, confirmé.
    Le plafond de 5 notifications retire la plus ancienne même si elle est persistante (récupération d'archive ou échec de cache avec action).
    Scénario : une notification de récupération d'archive, qui ne expire jamais, disparaît quand cinq notifications arrivent après elle (opérations en série), avant que l'utilisateur ait pu la lire.

12. `src/ui/app/theme_window/draft.rs:251` (liste : ligne 233) — faible, confirmé.
    « Supprimer » détruit le thème sans confirmation. Un clic sur un autre thème de la liste abandonne sans avertissement les modifications non enregistrées.
    Scénario : édition de six couleurs, puis clic sur une autre ligne de la liste. Tout le brouillon est perdu.

13. `src/ui/app/theme_window.rs:17` — faible, plausible.
    `close_theme_windows` retire la fenêtre par `remove_window`, sans passer par `discard_draft` : le callback de fermeture n'est appelé que pour une demande de fermeture de l'utilisateur.
    Scénario : la fenêtre principale se ferme alors que l'application continue (sélecteur de fichiers en attente) avec un brouillon non enregistré. L'aperçu du thème reste appliqué au thème de l'application jusqu'au prochain `apply_theme`.

14. `src/ui/app/trash_settings/deletion_policy.rs:77` (appel : `actions.rs:85`) — faible, plausible.
    `selection_deletes_permanently` ne retire pas le préfixe `\\?\` que `for_path` gère, et un échec du `spawn_blocking` est converti en « non permanent » (`unwrap_or(false)`).
    Scénario : une sélection en chemin verbatim sur un lecteur configuré en suppression immédiate affiche « récupérable » alors que la suppression est définitive. Ce défaut est borné, puisque le type d'opération ne dépend pas de ce calcul.

15. `src/ui/app/trash_settings/actions.rs:118` — faible, plausible.
    `released.await` n'a pas de délai. Pendant ce temps, `trash_delete_check_pending` reste vrai et `execute_supprimer` renvoie silencieusement.
    Scénario : un travail du Viewer qui ne se termine jamais (lecture d'un fichier réseau bloquée) bloque toutes les suppressions suivantes jusqu'au redémarrage.

16. `src/ui/app/trash_settings/linux.rs:157` (commit ligne 171) — faible, confirmé.
    Le pourcentage KDE est affiché arrondi à une décimale, puis relu comme valeur modifiée. Enregistrer sans rien changer réécrit donc une valeur différente.
    Scénario : `Percent=12.34` dans `trashrc`. L'ouverture puis Enregistrer écrit 12.3, car la valeur arrondie diffère de la valeur initiale.

### Optimisation

17. `src/ui/app/selection.rs:114` (parcours lignes 193-206) — optimisation, confirmé.
    Chaque événement de souris pendant un rectangle de sélection reconstruit les lignes d'affichage et parcourt toutes les lignes. Il clone aussi les chemins couverts, au lieu de se limiter à la plage visible ou à une dichotomie sur les hauteurs.
    Scénario : dossier de plus de 100 000 entrées, glisser un rectangle. Le coût par déplacement est O(n) en allocations.

18. `src/ui/app/shell_features.rs:183` (rafraîchissement ligne 106) — optimisation, confirmé.
    La tâche réveille le thread principal toutes les 100 ms en permanence. Toutes les 5 s, elle relance les accès Accès rapide, OneDrive, WSL et Peek, même sans changement signalé.
    Scénario : application au repos. Environ 10 réveils par seconde, plus quatre lectures système toutes les 5 s. Une attente sur l'événement de la révision, ou une période plus longue, suffirait.

<!-- LOT TERMINÉ -->
