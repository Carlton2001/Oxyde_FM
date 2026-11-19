# Audit UI-APP-05

## Fichiers lus en entier

- [x] `src/ui/app/layout/tabs.rs`
- [x] `src/ui/app/layout/tree.rs`
- [x] `src/ui/app/main_window.rs`
- [x] `src/ui/app/mod.rs`
- [x] `src/ui/app/navigation.rs`
- [x] `src/ui/app/navigation/breadcrumb.rs`
- [x] `src/ui/app/navigation/trash_loading.rs`
- [x] `src/ui/app/network.rs`
- [x] `src/ui/app/open_with.rs`
- [x] `src/ui/app/pinned_order.rs`
- [x] `src/ui/app/preferences.rs`

Aucun constat critique ou élevé. `tree.rs`, `mod.rs` et `main_window.rs` n'appellent pas d'autre remarque que celle du dernier constat.

## Constats

1. `src/ui/app/navigation.rs:639-670` : optimisation, confirmé. Chaque listing réussi (donc aussi chaque rafraîchissement déclenché par un observateur) relance une lecture complète du dossier par la commande de nœuds de la barre latérale, qui ouvre en plus un `read_dir` par sous-dossier, alors que le listing vient de lire les mêmes entrées. Seul `trash://` est exclu : un dossier de plusieurs milliers de sous-dossiers, surtout sur partage réseau ou URI distante, subit ces milliers d'accès supplémentaires à chaque modification du dossier affiché.

2. `src/ui/app/navigation.rs:570-572` (et `trash_loading.rs:46`, `52`) : faible, plausible. Quand la préparation renvoie faux pour une autre raison que la péremption (panique du calcul en tâche bloquante, sémaphore fermé), la fonction sort sans retirer la requête de `pending_file_requests` ni changer l'état de chargement : l'onglet reste en chargement indéfiniment, sans erreur. Dans `trash_loading.rs`, le retour anticipé quand l'onglet a été fermé laisse aussi une entrée orpheline dans la table des requêtes.

3. `src/ui/app/navigation.rs:374-381` (et `395-405`) : faible, confirmé. Changer d'onglet ou fermer l'onglet actif relance un listing dès que l'onglet cible est en chargement avec une liste vide, sans vérifier qu'une requête est déjà en vol ; la nouvelle requête invalide l'ancienne dont le travail est perdu. Sur un dossier lent (réseau), passer d'un onglet à l'autre redémarre le listing à chaque fois et peut l'empêcher d'aboutir ; au démarrage, tous les onglets restaurés sont dans ce cas.

4. `src/ui/app/navigation.rs:606-610` : faible, plausible. Un échec de listing, même lors d'un rafraîchissement d'arrière-plan (observateur, actualisation), vide la liste de l'onglet et affiche l'erreur. Une coupure réseau momentanée ou un refus d'accès transitoire pendant un rafraîchissement fait disparaître le contenu affiché, alors que le commentaire de `refresh_all` promet de ne pas perturber l'état de l'onglet.

5. `src/ui/app/navigation.rs:54-56` : faible, plausible. Seul le parent immédiat est relu pour révéler le dossier actif ; les ancêtres intermédiaires utilisent les nœuds en cache. Si un dossier intermédiaire a été créé hors d'Oxyde après la mise en cache de son parent, la recherche s'arrête sans erreur et la barre latérale ne suit pas le chemin actif (navigation par le fil d'Ariane vers `a/nouveau/b`).

6. `src/ui/app/navigation.rs:130-161` : faible, plausible. Le déploiement récursif de la barre latérale n'a ni identifiant de requête, ni test de `ejecting_drive`, ni annulation, contrairement à `request_sidebar_nodes`. Le résultat est appliqué même si l'utilisateur a replié le dossier entre-temps, si les réglages d'affichage des fichiers cachés ou système ont changé (valeurs capturées au lancement), ou si un lecteur réseau a été déconnecté pendant l'opération (nœuds d'un lecteur disparu réinsérés).

7. `src/ui/app/navigation/breadcrumb.rs:129-154` (et `navigation.rs:245`) : faible, plausible. Le chemin saisi n'est validé que par un test de dossier sur le texte brut : un chemin relatif (`docs`, `..`) ou propre à un lecteur (`C:`) est accepté s'il existe par rapport au répertoire courant du processus, puis devient le chemin de l'onglet et est sauvegardé tel quel. `navigate_to` compare en outre les chemins comme des chaînes : `c:\users` ou une saisie avec séparateur final, différents de la forme actuelle, empilent une entrée d'historique et relancent un listing du même dossier.

8. `src/ui/app/navigation/breadcrumb.rs:25-31` : faible, confirmé. Quand le listing d'un chevron échoue (accès refusé, réseau), l'erreur est ignorée sans journal ni message : le clic n'a aucun effet visible.

9. `src/ui/app/navigation/breadcrumb.rs:200-211` : faible, plausible. La validation par test de dossier s'exécute sans délai maximal dans une tâche bloquante. Sur un chemin réseau injoignable, la tâche reste bloquée longtemps sans indication d'attente ; chaque nouvel Entrée invalide la précédente côté état mais n'arrête pas son thread, et les threads bloqués s'accumulent.

10. `src/ui/app/navigation.rs:393-416` : faible, plausible. Fermer un onglet ou les autres onglets ne ferme pas l'édition du fil d'Ariane, contrairement à `switch_tab` et `load_path`. Si le panneau édité disparaît, `breadcrumb_editing` reste vrai sans champ visible ; si seul l'onglet actif change, le champ garde le texte de l'ancien onglet et Entrée navigue dans le nouvel onglet actif.

11. `src/ui/app/layout/tabs.rs:138-143` : faible, confirmé. Sous Windows, le bouton de nouvel onglet ouvre toujours `C:/` : sur une machine sans lecteur C: ou avec Windows installé ailleurs, l'onglet s'ouvre en erreur. La forme avec barre oblique diffère aussi de `C:\` utilisée pour les panneaux, d'où des comparaisons de chaînes inégales.

12. `src/ui/app/layout/tabs.rs:645-651` : faible, confirmé. Déposer plusieurs dossiers sur la barre d'onglets les ajoute un par un, chacun inséré juste après l'onglet actif sans l'activer : ils apparaissent dans l'ordre inverse du dépôt.

13. `src/ui/app/layout/tabs.rs:70-101` (appels en `157-159` et `203-205`) : optimisation, confirmé. À chaque rendu et pour chaque onglet, `tab_drive_name` parcourt tous les éléments de la barre latérale (arbre déployé compris, sous Linux pour tous les onglets) en recalculant les chaînes normalisées de l'onglet dans la boucle, et l'icône parcourt les lecteurs avec `paths_match` qui alloue deux chemins à chaque comparaison. Parcourir `state.drives` et normaliser une fois suffirait.

14. `src/ui/app/layout/tabs.rs:232-241` : optimisation, plausible. Le rappel de bornes de chaque onglet met à jour l'entité de l'application et clone une chaîne dans une table à chaque passage de rendu, même quand le point d'ancrage n'a pas changé.

15. `src/ui/app/preferences.rs:425-445` (et `230-243`) : optimisation, confirmé. Chaque navigation, changement d'onglet ou de panneau sérialise l'espace de travail sur le fil d'interface, clone toute la configuration et met en file une écriture ; la chaîne n'en fusionne aucune : chaque génération en attente écrit son instantané (fichier de thèmes puis configuration, chacun avec `sync_all`), soit une rafale d'écritures synchronisées pour une série de navigations rapides. Sauter les instantanés dépassés par une génération déjà en file suffirait.

16. `src/ui/app/preferences.rs:112-119` : optimisation, plausible. Après le nettoyage du cache, la boucle d'invalidation des icônes et vignettes parcourt sur le fil d'interface tous les fichiers de tous les onglets ; sur des dizaines de milliers d'entrées, cela bloque l'interface.

17. `src/ui/app/open_with.rs:66-69` : optimisation, confirmé. La liste des fichiers d'ouverture est obtenue en testant `paths.contains` sur un vecteur pour chaque fichier du dossier, avec en plus des appels de plateforme par fichier : complexité quadratique sur le fil d'interface lors d'un clic droit avec une grosse sélection dans un grand dossier (par exemple 20 000 sur 20 000, quelques centaines de millions de comparaisons).

18. `src/ui/app/open_with.rs:229-235` : faible, confirmé. Valider (Entrée ou bouton Ouvrir) pendant le chargement des applications ferme l'invite sans rien lancer, car `choices` n'est pas encore rempli et la sélection est vide ; aucun retour n'est donné.

19. `src/ui/app/pinned_order.rs:76` : faible, confirmé. À chaque réorganisation, l'ordre enregistré est remplacé par la seule liste des dossiers épinglés présents à cet instant : le rang enregistré d'un dossier momentanément absent (lecteur démonté, élément retiré puis réépinglé) est perdu.

20. `src/ui/app/network.rs:466-468` : faible, plausible. Lors d'une déconnexion, tous les observateurs, y compris ceux de dossiers locaux sans rapport, sont relâchés avant la déconnexion, et l'échec du relâchement d'un seul interrompt l'opération par un retour d'erreur sans tenter la déconnexion demandée, après avoir déjà supprimé les observateurs restants.

21. `src/ui/app/navigation.rs:700-716` (appelé depuis `main_window.rs:76`) : faible, plausible. Au démarrage, `refresh_all_tabs` lance un listing pour tous les onglets restaurés, y compris ceux qui ne sont pas affichés et les onglets distants, ce qui déclenche leur connexion (et leurs éventuelles invites d'identifiants) sans que l'utilisateur les ait ouverts ; `switch_tab` sait pourtant charger à la demande.

<!-- LOT TERMINÉ -->
