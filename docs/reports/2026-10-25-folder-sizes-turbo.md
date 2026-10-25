# Tailles des dossiers et Turbo — 25 octobre 2026

Modifications locales à partir de `4be8e6f`. Le réglage Turbo général pilote désormais les parcours de tailles, Propriétés comprises. Les boutons propres aux opérations de fichiers gardent leur portée individuelle. La priorité Windows suit le réglage aux points de contrôle ; le nombre de workers est choisi au démarrage de chaque parcours, sans recommencer un travail en cours.

## Comportement

- Deux parcours physiques globaux, permis conservés jusqu’à la fin effective même après annulation. Les deux pools restent indépendants pour qu’un gros dossier ne bloque pas l’autre parcours.
- SSD : jusqu’à 2 workers par parcours sans Turbo, jusqu’à 8 avec Turbo ; le plafond Turbo par parcours est la moitié arrondie au supérieur des processeurs logiques disponibles, bornée entre 1 et 8. HDD, réseau ou stockage inconnu : 1 worker par parcours dans les deux modes.
- Priorité d’arrière-plan Windows sans Turbo, normale avec Turbo ; restitution à la priorité normale des threads du pool et du thread bloquant réutilisé à la fin du parcours.
- Activation explicite immédiate ; regroupement de 500 ms conservé pour les relances de surveillance. Les compteurs sont fusionnés par répertoire plutôt que par fichier.
- Les places de parcours sont libérées avant la préparation d’affichage. Une tâche indépendante regroupe les résultats sur 50 ms, par lots de 128 au plus. Tri, filtres et résumés sont préparés une fois par lot en arrière-plan.
- Les changements ordinaires gardent les résultats publiables comme mesures à actualiser, signalées par la couleur `accent` du thème actif ; les disparitions et navigations les invalident toujours. L’acquittement d’un ancien lot ne retire pas un nouveau résultat. Les recalculs automatiques attendent cinq secondes sans événement et au moins cinq secondes après la fin du précédent parcours ; les premières mesures ont priorité dans l’onglet. Une actualisation explicite relance les mesures nécessaires pendant l’activité, sans relire celles qui sont restées valides sous surveillance fiable.
- Les exclusions `System Volume Information` et `Microsoft\Windows\Containers`, les liens et jonctions gardent leur comportement. Pas de recalcul incrémental interne à l’arborescence.

## Mesures locales

Windows, SSD détecté par `Platform::is_ssd`, 32 processeurs logiques, profil de développement optimisé du dépôt, cache chaud et fixtures dans un `TEMP`/`TMP` privé sous `target/folder-size-tests-temp`. Aucun document personnel utilisé.

Le profil crée 8 192 fichiers de 64 octets, répartis dans 32 dossiers de 8 sous-dossiers. Il compare la racine complète puis ses 32 enfants, avec deux parcours simultanés au maximum. Sept mesures par mode après échauffement ; l’ordre Normal/Turbo alterne. Tous les parcours vérifient les mêmes tailles et nombres de fichiers.

| Parcours | Normal, médiane | Turbo, médiane |
| --- | --- | --- |
| Une racine de 8 192 fichiers | 356,32 ms | 161,63 ms |
| 32 dossiers de 256 fichiers | 316,89 ms | 198,05 ms |

Il s’agit d’une comparaison entre les deux modes du nouveau moteur, pas entre deux révisions. Ces résultats confortent les plafonds de 2/8 workers sur ce SSD ; ils ne mesurent ni HDD, ni réseau, ni cache froid, ni la charge concurrente d’une copie réelle.

La comparaison de publication utilise 2 000 lignes de dossiers et 128 résultats. Avec une préparation individuelle par résultat : **66,61 ms** ; avec un seul lot : **0,546 ms**. C’est une mesure synthétique unique du travail de préparation des données, avec vérification des totaux, sans rendu GPUI ni délai de regroupement de 50 ms. Elle ne donne pas le temps complet perçu par l’utilisateur.

## Validation

37 tests ciblés réussis : moteur et pools (7), file/publication (15), commande de calcul et exclusions (3), transitions natives de priorité Windows (3), préparation de liste (5), catalogues des cinq langues (1), résumé des Propriétés (3). Les deux profils manuels ci-dessus ont également été exécutés avec succès. Le profil historique `profile_many_folder_scans` reste ignoré.

Commandes, avec le `TEMP`/`TMP` privé :

```powershell
cargo check --offline --locked --all-targets
cargo test --offline --locked --all-targets folder_sizes -- --test-threads=1
cargo test --offline --locked --lib size_scan_tests -- --test-threads=1
cargo test --offline --locked --lib thread_priority -- --test-threads=1
cargo test --offline --locked --bin oxyde preparation:: -- --test-threads=1
cargo test --offline --locked --bin oxyde catalogs_preserve_all_keys_and_named_parameters -- --test-threads=1
cargo test --offline --locked --bin oxyde properties::selection::tests -- --test-threads=1
cargo test --offline --locked --lib profile_normal_and_turbo_size_scans -- --ignored --test-threads=1 --nocapture
cargo test --offline --locked --bin oxyde profile_grouped_folder_size_publication -- --ignored --test-threads=1 --nocapture
```

Les autres cibles ont été compilées par les vérifications toutes cibles ; leurs suites ne sont pas couvertes par le filtre `folder_sizes`. Pas d’essai visuel GPUI ni de validation Linux, HDD ou réseau dans ce bilan.

## Complément : dossiers continuellement modifiés

Le rejet strict d’un résultat après modification pouvait empêcher toute première taille d’apparaître sur une arborescence très active, notamment dans un scénario comme `C:\Users` ou `C:\Windows`. Les tests de débit ci-dessus ne couvraient pas cette activité concurrente. Le nouveau contrat conserve la mesure du parcours, explicitement provisoire, puis traite une seule actualisation différée. Le watcher reste actif pendant le premier passage ; il ne faut donc pas arrêter les écritures pour obtenir une valeur.

Sept nouveaux tests déterministes couvrent 15 000 événements simulés, la publication du premier résultat, une échéance qui ne glisse pas sous des événements continus, la priorité d’un dossier jamais mesuré, l’espacement après un parcours long, plusieurs cycles de mesures provisoires puis une stabilisation, les changements pendant la préparation, les suppressions/recréations et une fixture réelle passant de 10 à 70 octets. Ils utilisent des instants injectés pour éviter des attentes réelles de cinq secondes. Les anciens tests de coalescence et d’acquittement ont été adaptés au maintien des mesures provisoires.

Validation de ce complément : `cargo check --offline --locked --all-targets` sans avertissement et **41 tests ciblés réussis** : `folder_sizes` (22, profil manuel ignoré), `drive_watchers` (12, dont surveillance récursive native et polling), `preparation::` (5), parité des catalogues (1) et validité des paramètres des messages (1). Ces commandes ont été exécutées avec `--bin oxyde -- --test-threads=1`, dans le même TEMP/TMP privé. Les benchmarks de débit de la première partie n’ont pas été relancés pour ce complément.

Ce correctif ne constitue pas une observation du processus Oxyde ouvert chez l’utilisateur, ni une mesure sur ses arborescences personnelles. La surcharge propre de la surveillance récursive ou du polling reste possible ; ce changement supprime surtout les parcours complets rejetés et les relances immédiates en boucle.

## Correction du marquage systématique des deux premiers dossiers

La première version lançait les deux parcours avant l’installation asynchrone du watcher récursif. À son installation, une invalidation générale destinée à couvrir cet intervalle ajoutait systématiquement une relance des deux parcours déjà partis. Leurs mesures portaient alors le signe provisoire même sans événement de modification ; les demandes suivantes, encore en file, étaient mesurées après installation et ne conservaient pas ce signe. Ce mécanisme explique le motif des deux premières lignes indépendamment de l’arborescence.

L’activation demande maintenant le watcher avant de lancer les parcours et le départ attend la fin de sa création. Le callback reprend aussi le calcul après un échec journalisé. La vérification de l’intervalle de remplacement ne réenfile que les mesures déjà commencées ou disponibles ; les demandes initiales intactes restent inchangées. La simple mise en file d’un dossier sans mesure ne pose plus le signe provisoire. Le texte des cinq langues parle désormais d’actualisation en cours ou en attente, sans affirmer qu’une modification a été observée.

Trois tests supplémentaires couvrent une installation initiale sur `C:/`, `D:/` et une arborescence imbriquée, les événements précédant le premier départ et le maintien de la protection lors du remplacement d’un watcher après le départ d’une mesure. Le scénario initial vérifie l’absence de marquage et de relance des deux premières mesures. La coordination GPUI et l’affichage dans une session réelle restent à vérifier visuellement.

Vérification de cette correction : **44 tests ciblés réussis** (25 pour les tailles, 12 pour les watchers, 5 pour la préparation, 2 pour les catalogues), un profil manuel ignoré et `cargo check --offline --locked --all-targets` sans avertissement.

## Colonne de taille et occupation disque sous activité continue

Le préfixe `≈` et son conteneur réduisaient la place disponible dans une colonne dimensionnée pour la taille seule. La nouvelle indication conserve le texte numérique et les unités ; seule la couleur devient `warning_text`. En grille, le libellé de type porte cette couleur, sans badge supplémentaire. L’infobulle des cinq langues explique que le recalcul automatique attend une accalmie et qu’Actualiser permet de le forcer.

La cadence précédente ne laissait que cinq secondes entre deux parcours complets, même si des changements persistaient. Sur une arborescence dont le parcours dure longtemps, cela pouvait entretenir une occupation disque élevée. Le nouveau délai dépend aussi du dernier événement reçu : une activité continue conserve la mesure visible et ne lance plus de passage automatique jusqu’à cinq secondes d’accalmie. Une demande manuelle force au plus un passage supplémentaire ; elle survit aux événements du watcher, mais ne rend pas les passages suivants automatiquement forcés. Cette politique remplace la cadence fixe décrite dans les étapes précédentes du bilan.

Le polling récursif passe de deux à trente secondes entre instantanés pour réduire ses propres lectures ; le non-récursif reste à deux secondes. Les tests de mutation de ce backend injectent une cadence de 100 ms pour vérifier son fonctionnement sans attendre trente secondes. Le coût d’un instantané initial ou périodique demeure ; aucun débit disque réel sur les dossiers personnels n’a été mesuré ici.

Quatre tests supplémentaires simulent une minute d’activité continue avec un voisin calme, une actualisation manuelle sous ces événements, cent demandes manuelles coalescées pendant un parcours et le rafraîchissement explicite d’un onglet. Le scénario d’événements continus existant vérifie désormais le report automatique jusqu’à l’accalmie, tout en conservant la première mesure publiable. Les preuves des étapes précédentes restent historiques ; les mesures de débit ne sont pas relancées pour ce changement.

Validation finale de ce complément : **48 tests ciblés réussis** (29 tailles, 12 watchers, 5 préparation, 2 catalogues), un profil manuel ignoré, et `cargo check --offline --locked --all-targets` sans avertissement. Les assertions anciennes de relance immédiate ont été adaptées au délai d’accalmie tout en conservant la vérification d’équité et d’absence de parcours concurrents du même dossier. Pas de mesure d’occupation disque ni de vérification visuelle GPUI sur la session de l’utilisateur.

## Refresh sélectif et infobulle unique

Un refresh sous surveillance native récursive saine réutilise les tailles non invalidées et force seulement les dossiers modifiés ou inconnus. Il ne duplique pas un parcours propre déjà en cours ni un résultat prêt à publier ; un échec sur un dossier invalidé reste relançable. Les nouveaux dossiers sont détectés par le listing. Sans watcher, avec du polling ou après une erreur de surveillance, le refresh reste complet. Une perte d’événements ou un débordement conserve l’invalidation globale. Les événements reçus avant le tick de 500 ms sont appliqués une seule fois à tous les onglets concernés ; les demandes de listing et les autres racines restent en file.

Le marquage de la taille utilise désormais l’accent du thème au lieu du jeton d’avertissement, pour suivre le thème actif. Les cinq catalogues présentent le message en trois phrases sur trois lignes. Le message est ajouté aux lignes de contexte de l’infobulle normale : la cellule de taille et le libellé en grille ne créent plus de seconde infobulle native, ce qui supprime la concurrence avec la session de survol du fichier. Les comportements de fermeture de cette session ne sont pas modifiés.

Cinq nouveaux tests couvrent le refresh sélectif et la conservation des valeurs ; trois vérifient la consommation des événements de tailles sans perte de listing, répétition ou suppression d’une autre racine. Un test vérifie les trois lignes dans les cinq langues. La coordination GPUI et le comportement de survol physique restent à vérifier en session réelle.

Validation : **62 tests ciblés réussis** (34 tailles, 15 watchers, 5 préparation, 2 catalogues, 1 message en trois lignes, 5 cycle de survol), un profil manuel ignoré et `cargo check --offline --locked --all-targets` sans avertissement. Les scénarios de survol couvrent transitions, temporisations et rejet des anciennes cibles ; ils ne constituent pas un essai visuel des fenêtres natives.
