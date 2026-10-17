# Turbo : couverture et suites — 17 octobre 2026

**Turbo agit réellement, mais de façon inégale : parallélisme pour certaines opérations, priorité seule pour d’autres, aucun effet explicite dans le worker de Corbeille des dossiers.** Audit du code à la révision `ecab3ad`, principalement sous Windows. Les pistes ci-dessous ne constituent pas des gains mesurés. **Mise à jour :** les trois chantiers de priorité 1 sont traités ; le contrat en vigueur est décrit dans [Opérations](../architecture/operations.md#contrat-turbo). L’effet réel du Shell sur la Corbeille reste à mesurer. Les chantiers de priorité 2 sont traités ou tranchés par mesure : voir [Résultats de priorité 2](#résultats-de-priorité-2).

## Couverture constatée

Le bouton est proposé pour les six types d’opérations. La préférence par défaut s’applique aux six, malgré un libellé qui ne cite que copie et déplacement. Le bouton agit sur l’opération concernée ; Turbo ne pilote ni navigation, ni recherche, ni aperçus ou miniatures.

| Opération | Normal → Turbo | Limite actuelle |
| --- | --- | --- |
| Copie | 1 → jusqu’à 4 fichiers simultanés ; sortie du mode de priorité d’arrière-plan | Parallélisme seulement avec au moins 8 fichiers et une taille moyenne inférieure à 32 Mio. Sinon, priorité seule. |
| Déplacement | Même parallélisme lorsque le déplacement nécessite une copie | Le renommage sur le même volume reste séquentiel ; priorité seule. |
| Suppression définitive | 1 → jusqu’à 4 éléments sélectionnés simultanés | Un dossier sélectionné reste parcouru par un seul worker. La priorité haute prévue est omise si le worker démarre directement en Turbo. |
| Corbeille | Priorité relevée au démarrage de chaque lot de fichiers | Ni taille des lots ni parallélisme modifiés. Pour un dossier, le thread natif ne consulte pas Turbo. |
| Compression | Parallélisme ZIP, 7z et TAR.ZST ; priorité du processus relevée | Voir tableau suivant. TAR reste séquentiel, avec priorité seule. |
| Extraction | Priorité du processus relevée | Aucun parallélisme ajouté ; le décodage 7z complet est explicitement limité à un thread. |

| Compression Windows | Normal | Turbo et limites |
| --- | --- | --- |
| ZIP | 1 tâche | Jusqu’à 16 fichiers simultanés, sous budget mémoire. Un fichier de plus de 256 Mio reste encodé seul en flux. |
| 7z | 1 thread de compression | Plafonds de 16 / 6 / 3 threads en Rapide / Normal / Meilleur, selon CPU, mémoire et blocs disponibles. Au-delà de 256 Mio par fichier, encodage en flux monothread. |
| TAR.ZST | 1 thread | Jusqu’à 8, ou 2 en qualité maximale ; nombre choisi au démarrage du flux. |
| TAR | Écriture séquentielle | Priorité seule ; ce format ne compresse pas les données. |

Les copies, déplacements, suppressions et renommages internes aux archives héritent du comportement de leur recompression. Les plafonds sont des capacités, pas un nombre garanti de threads occupés. Les [mesures de compression](2026-10-17-archive-turbo.md) restent la référence pour les temps observés.

## Pistes à suivre, par priorité

| Priorité | Chantier | Action et critère de réussite |
| --- | --- | --- |
| 1 | Priorités Windows | Centraliser les transitions normal/arrière-plan/Turbo. Corriger le démarrage Turbo des suppressions. Vérifier le retour à la priorité normale : `Normal` appelle actuellement seulement `THREAD_MODE_BACKGROUND_END`, sans remise explicite à `THREAD_PRIORITY_NORMAL`, et les erreurs sont ignorées. Tester démarrage dans chaque mode et bascules répétées. |
| 1 | Corbeille | Transmettre la politique au thread natif des dossiers et réévaluer le mode aux points de contrôle des lots. Mesurer l’effet réel du Shell avant d’ajouter du parallélisme. Préserver reçus, annulation, historique et comportement natif. |
| 1 | Contrat utilisateur | Corriger le libellé de préférence dans les cinq langues. Définir les effets garantis par opération : priorité, concurrence, moment d’application. Distinguer les limites structurelles des défauts d’implémentation. |
| 2 | Extraction | Mesurer séparément lecture, décodage et écriture. Étudier le décodage multithread 7z et l’extraction ZIP par entrées indépendantes. Conserver budgets, validation des chemins, gestion des collisions et annulation ; ne pas présumer que tous les flux se parallélisent. |
| 2 | Copie et déplacement entre volumes | Comparer les seuils fixes à une concurrence adaptée au stockage et aux tailles. Tester notamment les gros fichiers ; davantage de transferts simultanés peut ralentir un disque mécanique ou saturer le même support. |
| 2 | Suppression d’un gros dossier | Mesurer ce cas seul. Étudier une file bornée de sous-arbres indépendants, avec suppression des parents après leurs enfants ; préserver contrôles d’identité, liens, annulation et bilans. |
| 2 | Compression et bascules | Pour TAR.ZST, choisir un contrat explicite : nombre de threads fixé au démarrage, ou évolution du moteur si elle est possible sans casser le flux. Pour ZIP/7z, mesurer les gros fichiers monothread et rechercher une solution aux files mémoire du codec avant de relever la limite de 256 Mio. |
| 3 | Cas déjà peu coûteux | Conserver priorité seule pour TAR et renommages tant qu’un benchmark ne justifie pas davantage. Ne pas ajouter des threads uniquement pour rendre Turbo visible. |

Turbo doit exploiter les ressources disponibles **dans les budgets de sécurité**, sans modifier la qualité choisie ni bloquer l’interface. L’arrêt de Turbo peut laisser finir les tâches de compression ou copies déjà engagées ; vérifier la réduction de concurrence sur les tâches suivantes.

## Résultats de priorité 2

Mesures Windows, deux SSD NVMe (C:/D: sur un disque, E: sur l’autre), cache chaud, profil de développement ; jeu `ControlCenter_5` (195 fichiers, 274 Mo) pour les archives. Aucun disque rotatif ni partage réseau n’a été mesuré.

| Chantier | Constat | Décision |
| --- | --- | --- |
| Extraction | ZIP : 26–28 s contre ~5 s en 7z ; 314 000 lectures de source pour 285 Mo, car `BufReader` vidait son tampon à chaque repositionnement. | Fenêtre de 256 Kio conservée : 2 200 lectures, **6–8 s**. Turbo n’apporte que la priorité (−10 à −25 %). Décodage 7z multithread non activé : mémoire non bornée dans lzma-rust2. |
| Copie entre volumes | Gros fichiers : 1,8 s → 1,0 s avec 4 workers ; petits fichiers : 30–34 s → 5–7 s. Chaque relecture de vérification attend Defender 15–25 ms. Boucle de suivi : jusqu’à 500 ms ajoutées à chaque fin de copie. | 4 workers sur SSD dès 2 fichiers ; règle antérieure ailleurs. Boucle de suivi à 20 ms. |
| Suppression d’un gros dossier | 20 000 fichiers en 0,65–0,75 s (~35 µs/fichier). Déléguer chaque fichier à 4 workers : 0,73–0,83 s, plus lent. Sous-arbres indépendants : ~1,7× en essai isolé. Boucle de suivi : jusqu’à 300 ms ajoutées. | Parcours séquentiel conservé ; boucle de suivi à 20 ms. Les sous-arbres parallèles demanderaient un point de contrôle partageable entre threads. |
| Compression et bascules | 7z d’un fichier de 1 Gio en rapide : 39,1 s monothread, **7,5 s** en Turbo après mise à jour. | sevenz-rust2 0.24 (lzma-rust2 0.22, file bornée) : limite de 256 Mio levée pour 7z, conservée pour ZIP. TAR.ZST : nombre de threads fixé au démarrage du flux. |

Outils : `archive_extraction_benchmark`, `copy_concurrency_benchmark`, `delete_tree_benchmark` et `archive_compression_benchmark` (exemples Cargo).

## Validation à prévoir

Comparer Normal et Turbo sur les mêmes données : beaucoup de petits fichiers, quelques gros fichiers, un dossier unique, plusieurs racines et archives solides. Séparer même volume, volumes distincts et réseau ; répéter les mesures en distinguant caches froids et chauds.

Relever durée, CPU, mémoire maximale, débit, concurrence effective et latence UI. Tester aussi l’activation en cours d’opération, la désactivation, pause/reprise et annulation. Vérifier inventaire/contenu des archives et effets réellement accomplis pour les opérations destructives, sur des fixtures jetables. Un gain sur machine occupée peut venir de la priorité ; il ne prouve pas un meilleur débit maximal.

## Preuves et limites

Lecture du code et deux tests ciblés réussis : `copy_uses_parallel_workers_only_for_many_small_files` et `controls_and_dismissal_target_only_one_operation`. Aucun benchmark comparatif supplémentaire ni modification du comportement dans cet audit.

Points d’entrée, depuis la racine du dépôt :

- Bouton et propagation : `src/ui/components/operations/card/controls.rs`, `src/systems/file_ops.rs` ; préférence : `src/ui/app/file_operations.rs`, `archives.rs`, `trash_settings/actions.rs` et `locales/fr.ftl`.
- Copie/déplacement : `src/systems/file_ops/copy.rs`, `copy/worker.rs` ; suppression/Corbeille : `src/systems/file_ops/delete.rs`, `delete/trash_batch.rs`.
- Shell et priorités : `src/platform/windows_shell_delete_tracked.rs`, `windows_shell_delete_tracked/batch.rs`, `windows_impl.rs`.
- Archives : `src/commands/archive_io_codec/encode.rs`, `parallel.rs`, `src/commands/archive_io_codec.rs`, `src/platform/windows_archive_io_worker.rs`.

Linux suit des chemins différents : `set_thread_priority` est vide et la compression locale ZIP force le niveau rapide en Turbo ; les encodeurs locaux 7z/Zstandard ne reprennent pas le parallélisme Windows. Prévoir leur alignement fonctionnel et des tests sur cette cible avant toute promesse commune aux deux systèmes. Aucun essai Linux effectué ici.
