# Audit UI-COMP-02

Fichiers lus en entier :

- [x] src/ui/components/file_list/details.rs
- [x] src/ui/components/file_list/details_row.rs
- [x] src/ui/components/file_list/filter_status.rs
- [x] src/ui/components/file_list/filters.rs
- [x] src/ui/components/file_list/grid.rs
- [x] src/ui/components/file_list/grid_card.rs
- [x] src/ui/components/file_list/grid_scroll.rs
- [x] src/ui/components/file_list/group_header.rs
- [x] src/ui/components/file_list/headers.rs
- [x] src/ui/components/file_list/presentation.rs
- [x] src/ui/components/file_list/scroll_top.rs
- [x] src/ui/components/file_list/selection_button.rs
- [x] src/ui/components/file_list/thumbnails.rs
- [x] src/ui/components/file_list/thumbnails/lifecycle.rs
- [x] src/ui/components/history_actions.rs
- [x] src/ui/components/icons.rs
- [x] src/ui/components/icons/colors.rs

## Constats

### Moyen

1. `details_row.rs:204` (et 211) — moyen — confirmé. Chaque ligne sélectionnée visible reconstruit deux fois la liste complète de la sélection (clone de toutes les chaînes) à chaque rendu, pour une opération de glisser qui n'a peut-être jamais lieu. Scénario : Ctrl+A dans un dossier de 100 000 fichiers, puis tout survol ou notification qui relance le rendu : environ 40 lignes visibles x 2 x 100 000 chemins copiés par image, soit des millions d'allocations sur le thread UI. Le coût est quadratique (lignes visibles x taille de la sélection).

2. `grid.rs:188` — moyen — confirmé. Même défaut dans la grille : chaque carte sélectionnée visible clone toute la sélection dès que plus d'un élément est sélectionné, à chaque rendu. Scénario : sélectionner tout un grand dossier en vue grille, puis survoler les cartes ; chaque changement de carte survolée relance le rendu et recopie la sélection pour chaque carte visible.

3. `details.rs:101` (et `grid.rs:514`) — moyen — confirmé. Le résumé de sélection de la barre d'état ne compte que les entrées de la liste filtrée visible, alors que les actions (copier, couper, via `clipboard.rs:29`) agissent sur tout l'ensemble des chemins sélectionnés. Aucun élagage de la sélection n'a été trouvé après un filtre de colonne, un rafraîchissement ou une suppression externe. Scénario : sélectionner trois fichiers, appliquer un filtre de colonne qui en masque deux (ou en supprimer deux depuis un autre programme puis rafraîchir) : la barre indique un seul élément sélectionné, mais Ctrl+C copie trois chemins, dont deux invisibles ou inexistants.

4. `filters.rs:38` — moyen — plausible. Le calcul des valeurs d'un filtre parcourt deux fois toute la liste du dossier et alloue un libellé de type par fichier à chaque passage. Il est exécuté de façon synchrone sur le thread UI, et son cache (`app/filters/panel.rs:59`) est invalidé à chaque changement de révision de filtre. Scénario : dossier de plusieurs centaines de milliers de fichiers, ouverture du filtre Type puis cases cochées successivement ; chaque coche recalcule tout sur le thread UI et fige l'interface.

5. `icons.rs:401` (et `thumbnails/lifecycle.rs:146`) — moyen — plausible. L'invalidation d'icône est appelée une fois par fichier de chaque onglet par `preferences.rs:113` après « vider le cache », sur le thread UI. Chaque appel réinitialise la globale des icônes de dossier, effectue sept retraits d'assets et trois constructions de source, et l'invalidation de miniature parcourt linéairement tous les propriétaires. Elle est de plus redondante avec `clear_grid_thumbnails`, qui a déjà tout évincé. Scénario : vider le cache avec un dossier de 200 000 entrées ouvert ; l'interface se fige pendant la boucle.

### Faible

6. `icons.rs:281` — faible — confirmé. Le jeu d'extensions PowerPoint contient « key » et est testé avant le jeu des certificats, qui contient aussi « key » : la branche certificat ne peut jamais s'appliquer à cette extension. Scénario : une clé privée nommée « id.key » reçoit l'icône de présentation au lieu de l'icône de clé.

7. `presentation.rs:95` — faible — plausible. Un nom se terminant par un point (possible sous Linux) a une extension vide selon la bibliothèque standard, donc un libellé de type vide (colonne Type, liste du filtre Type, valeur filtrable vide). Scénario : fichier « rapport. » dans un dossier Linux : cellule Type vide et entrée sans nom dans le filtre.

8. `presentation.rs:72` et `presentation.rs:174` — faible — plausible. Le bouton des tailles et la case de sélection affectent directement le volet actif au lieu de passer par l'activation du volet, qui vérifie l'existence du volet, exclut les volets de visionneuse et enregistre l'espace de travail. Scénario : cocher une case dans le volet inactif puis quitter l'application sans autre interaction ; le volet actif n'est pas persisté.

9. `details.rs:329` et `grid.rs:433` — faible — plausible. Les dépôts internes sur l'arrière-plan de la liste ne rejettent pas les onglets virtuels (corbeille, voisinage réseau), contrairement aux dépôts externes (`drag/external.rs:44`) ; la zone est même mise en surbrillance comme cible valide. Scénario : glisser un fichier depuis l'autre volet sur la vue Corbeille ou Réseau : l'opération part vers un chemin non réel et échoue avec un avis d'échec de préparation au lieu d'être refusée à l'entrée.

10. `icons.rs:411` — faible — plausible. Les icônes système uniques par chemin (exe, lnk, ico, dossiers spéciaux) restent dans le cache d'assets pour toute la session, et un échec transitoire de chargement est mis en cache définitivement pour ce chemin. Scénario : parcourir de nombreux dossiers d'exécutables et de raccourcis fait croître la mémoire sans libération ; une icône dont le premier chargement a échoué (fichier verrouillé, lecteur réseau lent) garde l'icône de repli jusqu'au vidage du cache.

11. `details_row.rs:393` — faible — plausible. Une date de suppression absente ou négative est rendue comme la date zéro, donc « 1970 », au lieu d'une cellule vide. Scénario : entrée de corbeille sans date lisible : la colonne affiche le 1er janvier 1970.

### Optimisation

12. `details.rs:98` (et ligne 207) ; `grid.rs:35` (et ligne 156) — optimisation — confirmé. L'ensemble des chemins sélectionnés est cloné en profondeur deux fois par rendu (une fois pour le rendu, une fois pour la fermeture de la liste virtualisée), alors que les autres structures de présentation sont partagées par `Arc`. Scénario : grande sélection, chaque rendu recopie deux fois toutes les chaînes.

13. `details.rs:101`, `grid.rs:514` et `selection_button.rs:16` — optimisation — confirmé. Le résumé de sélection est recalculé à chaque rendu par un parcours de toute la liste filtrée avec recherche par hachage, et le test « tout est sélectionné » parcourt toute la liste quand tout est sélectionné. Scénario : Ctrl+A sur 100 000 fichiers ; deux à trois parcours complets avec hachage de chemins par rendu, sans mise en cache selon la révision de la liste ou de la sélection.

<!-- LOT TERMINÉ -->
