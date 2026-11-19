# Audit UI-APP-02

Fichiers lus en entier :

- [x] `src/ui/app/dialog_windows.rs`
- [x] `src/ui/app/dialog_windows/fixed.rs`
- [x] `src/ui/app/dialog_windows/fixed/notice.rs`
- [x] `src/ui/app/disk_images.rs`
- [x] `src/ui/app/drag.rs`
- [x] `src/ui/app/drag/external.rs`
- [x] `src/ui/app/drag/native.rs`
- [x] `src/ui/app/drag/native/ghost.rs`
- [x] `src/ui/app/drag/outgoing.rs`
- [x] `src/ui/app/drag/popup.rs`
- [x] `src/ui/app/drag/volumes.rs`
- [x] `src/ui/app/drive_watchers.rs`
- [x] `src/ui/app/drive_watchers/changes.rs`
- [x] `src/ui/app/duplicates.rs`
- [x] `src/ui/app/duplicates/lifecycle.rs`
- [x] `src/ui/app/duplicates/presentation.rs`
- [x] `src/ui/app/duplicates/results.rs`
- [x] `src/ui/app/duplicates/scroll.rs`
- [x] `src/ui/app/duplicates/search.rs`

Bilan : 0 critique, 0 élevé, 5 moyens, 10 faibles, 6 optimisations. Aucun défaut relevé dans `dialog_windows*`, `popup.rs`, `presentation.rs`, `scroll.rs` ni dans le rendu de `results.rs` (les indices de lignes et de groupes restent cohérents entre `groups` et `result_rows`, remplacés ensemble).

## Moyens

- `src/ui/app/drive_watchers.rs:115-117` (liés : `drive_watchers/changes.rs:186-189`, `disk_images.rs:141-148`) — moyen, confirmé. L'éjection ou le démontage libère tous les observateurs et s'arrête à la première erreur de désinscription, qui annule l'opération. Sous Linux, la bibliothèque `notify` retire elle-même l'observateur quand le dossier surveillé est supprimé ou déplacé, et la désinscription suivante échoue avec « watch not found ». Scénario : un onglet reste ouvert sur `/media/usb/dossier`, un autre programme supprime ce dossier, puis l'utilisateur éjecte la clé. L'éjection échoue avec un message technique sans rapport, alors que le volume est libre. Même défaut pour le démontage d'une image disque.

- `src/ui/app/drive_watchers.rs:286-293` — moyen, confirmé. La tâche de surveillance vide la file des changements avant de tester `ejecting_drive`, puis quitte sans rien appliquer : les changements sont perdus définitivement. Scénario : montage d'une image disque, qui laisse les observateurs actifs, ou éjection longue avec attente d'authentification. Les modifications survenues dans les dossiers ouverts pendant ce temps ne rafraîchissent jamais les onglets ni les tailles, et la liste reste périmée jusqu'à une navigation manuelle.

- `src/ui/app/drag.rs:497-498` (origine : `src/ui/workspace/operations.rs:181`) — moyen, confirmé. Le redimensionnement de panneaux borne le poids du panneau entre 0,1 et la somme de la paire moins 0,1. Cet appel panique dès que la somme est inférieure à 0,2. Or chaque division d'un panneau à droite divise son poids par deux, et la restauration d'un espace de travail n'exige qu'un poids strictement positif. Scénario : environ six panneaux obtenus par divisions successives du dernier, puis glissement du séparateur entre les deux derniers. L'application s'arrête sur une panique.

- `src/ui/app/drag/outgoing.rs:28-32` (et `drag/native.rs:58-62`) — moyen, plausible. Le refus d'exporter le contenu de la corbeille repose sur l'onglet actif du panneau source au moment de la sortie de la fenêtre. Le survol d'un onglet pendant 600 ms (`drag.rs:536-560`) change cet onglet actif en cours de glisser. Scénario : glisser des éléments depuis la corbeille, survoler un autre onglet du même panneau, puis sortir de la fenêtre. Le garde-fou ne s'applique plus et les fichiers de la corbeille sont proposés à l'application tierce, qui peut les déplacer sans passer par l'enregistrement de la corbeille. Le chemin Windows natif n'a aucun garde équivalent : il vérifie seulement l'existence des chemins.

- `src/ui/app/drive_watchers.rs:388-413` et `520-538` — moyen, plausible. Le premier instantané rapide des racines suffit à déclarer un lecteur disparu : les onglets sont aussitôt redirigés vers le dossier personnel, et les historiques arrière et avant sont purgés. Aucune confirmation sur plusieurs relevés n'est faite. Scénario : un lecteur réseau ou une clé se réénumère brièvement, ou la liste des montages passe par un état transitoire. Les onglets perdent leur emplacement, leur sélection et leur historique sans retour possible.

## Faibles

- `src/ui/app/drag.rs:548-559` — faible, confirmé. Chaque survol d'onglet lance une minuterie non annulable. Quitter l'onglet efface seulement la cible, et la minuterie d'origine reste armée. Scénario : survol, sortie, puis nouveau survol du même onglet 400 ms plus tard. L'ancienne minuterie compare la même cible et change d'onglet 200 ms après le retour, bien avant les 600 ms prévus.

- `src/ui/app/drag.rs:51-55` — faible, plausible. Sous Windows, la comparaison de lecteurs porte sur le premier composant du chemin. Un préfixe étendu (`\\?\C:\`) diffère du préfixe ordinaire (`C:`), et un volume monté dans un dossier se confond avec son hôte. Scénario : un chemin long en préfixe étendu glissé vers le même lecteur donne « copier » par défaut au lieu de « déplacer ». Un volume monté dans `C:\Montage` donne « déplacer », donc une copie suivie d'une suppression sur un autre volume.

- `src/ui/app/duplicates/lifecycle.rs:117-133` (avec `commands/duplicates.rs:143-158`) — faible, confirmé. L'emplacement de l'onglet actif est présélectionné s'il n'est pas un identifiant virtuel connu. Les chemins d'archive et `\\wsl.localhost` passent le filtre, mais la recherche canonicalise chaque racine et abandonne toute la recherche à la première erreur ou au premier non-dossier. Scénario : ouvrir les doublons depuis un onglet dans une archive, puis lancer la recherche. Elle échoue entièrement, comme si un seul emplacement invalide invalidait tous les autres, y compris un lecteur débranché depuis l'ouverture de la fenêtre.

- `src/ui/app/duplicates/lifecycle.rs:126` — faible, confirmé. Le disque d'un emplacement se déduit d'une comparaison de préfixe en minuscules, sans frontière de composant ni distinction de casse selon la plateforme. Scénario sous Linux : avec `/` listé avant `/mnt/usb`, le dossier `/mnt/usb/x` reçoit l'étiquette du disque système. `/mnt/a` correspond aussi à `/mnt/ab/...`.

- `src/ui/app/duplicates/lifecycle.rs:129-130` — faible, confirmé. Le dossier courant est ajouté à la liste sans contrôle de doublon avec les lecteurs. Scénario : ouvrir la fenêtre depuis la racine d'un lecteur. La liste montre deux lignes de même chemin, cochées et décochées ensemble.

- `src/ui/app/duplicates/lifecycle.rs:27-35` et `59-90` — faible, plausible. La détection d'une fenêtre existante lit les fenêtres ouvertes, mais la création est différée. Deux appels dans le même tour ne voient aucune fenêtre et en ouvrent deux, chacune avec sa propre recherche.

- `src/ui/app/duplicates/search.rs:57-74` (avec `results.rs:59-80`) — faible, plausible. Une recherche annulée ne remplit rien mais garde l'état « recherché ». L'écran affiche alors le message « aucun doublon » au lieu de l'état initial. Scénario : lancer une recherche sur un gros disque, l'annuler. L'utilisateur lit qu'il n'y a pas de doublons.

- `src/ui/app/drive_watchers.rs:468-517` — faible, plausible. Une création d'observateur qui échoue (dossier illisible, lecteur sans média, limite inotify atteinte) n'est jamais mémorisée comme échec. Chaque rapprochement relance la création en arrière-plan et journalise un avertissement. Ce rapprochement est appelé à chaque cycle de rafraîchissement des lecteurs : toutes les 2 secondes sous Linux sans notification native, toutes les 30 secondes sous Windows. Scénario : une racine de lecteur optique vide surveillée en permanence.

- `src/ui/app/drive_watchers.rs:170-176` — faible, plausible. À la fin d'un montage (parfois après une fenêtre d'authentification), l'application navigue dans le panneau actif au moment de la fin, pas dans celui d'où l'action est partie. Scénario : lancer le montage, changer de panneau ou d'onglet pendant l'attente. La navigation s'applique au mauvais endroit.

- `src/ui/app/disk_images.rs:53-80` et `108-111` — faible, plausible. Le test de lisibilité d'une image dresse une liste complète, qui est refaite lors de la navigation. Sans protection contre les doubles clics, deux sondes partent, et la seconde passe la vérification d'origine puisque la navigation n'a pas encore eu lieu. Selon l'image, il en résulte deux navigations vers le même chemin (historique dupliqué) ou un second montage ignoré en silence parce qu'une opération est déjà en cours.

## Optimisations

- `src/ui/app/drag.rs:317-323` (appelant : `layout/tabs.rs:564`) — optimisation, confirmé. À chaque mouvement de souris au-dessus de la barre d'onglets pendant un glisser interne, la fonction parcourt tous les fichiers de tous les onglets du panneau source et cherche chacun dans la liste glissée. Le coût est le produit de ces deux nombres. Scénario : 100 000 fichiers chargés et 5 000 éléments glissés, soit environ 5·10^8 comparaisons de chaînes par mouvement, sur le thread UI.

- `src/ui/app/drag/external.rs:43-55`, `98`, `115-120`, `151-154` — optimisation, confirmé. Pour un glisser externe, la liste de chemins est reconvertie, filtrée et passée en minuscules à chaque mouvement et à chaque image, pour la cible survolée. Elle est aussi comparée aux volumes pour chaque source. Scénario : glisser 10 000 fichiers depuis l'Explorateur au-dessus de la liste, avec des dizaines de milliers d'allocations par mouvement.

- `src/ui/app/duplicates/search.rs:104-114` — optimisation, confirmé. Le filtre convertit en minuscules le chemin et le nom de chaque fichier de chaque groupe, à chaque frappe, alors que la requête est déjà en minuscules. Scénario : 500 000 fichiers en résultat, soit environ un million d'allocations par frappe en arrière-plan. Précalculer ces chaînes une fois par résultat suffirait.

- `src/ui/app/drive_watchers.rs:212-258` et `273-280` — optimisation, confirmé. Deux boucles tournent en permanence. L'une réveille le thread UI 10 fois par seconde juste pour lire l'état d'éjection. L'autre lance un travail bloquant toutes les 500 ms pour tester le presse-papiers. Attendre un signal remplacerait les deux réveils.

- `src/ui/app/drive_watchers.rs:107` et `453-467` — optimisation, plausible. L'éjection d'un lecteur libère tous les observateurs de l'application, y compris les observateurs récursifs d'autres disques pour les tailles. Ils sont ensuite tous recréés. Scénario : éjecter une clé alors qu'un gros dossier de disque système est suivi pour les tailles. L'arbre est reparcouru pour rien. Il suffirait de libérer ceux situés sur le volume visé.

- `src/ui/app/drive_watchers/changes.rs:143-146` — optimisation, confirmé. Le calcul des chemins (conversion de chaque ancêtre, comparaison normalisée) est fait pendant que le verrou partagé est tenu, dans le rappel du système de fichiers. La lecture côté UI attend ce calcul. Scénario : une compilation produisant des milliers d'événements par seconde dans un dossier suivi en mode récursif.

<!-- LOT TERMINÉ -->
