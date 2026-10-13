# R1, R2, R3 — résultats du 13 octobre 2026

Branche : `codex/security-r1-r2-r3`, depuis `c8f70de`. Ce rapport décrit le code effectivement livré et la validation Windows de cette branche. Il ne clôture pas les garanties qui restent sans preuve. R4–R9 ne font pas partie de cette passe ; Linux n'a été ni compilé ni exécuté.

| Point | Résultat | Limite conservée |
| --- | --- | --- |
| R1 | Confinement LPAC sans capacité étendu au listing/index d'archives, aux images et au texte Office. | PDF, vidéo, miniatures et icônes natives/Shell restent hors LPAC. R1 reste ouvert. |
| R2 | Reçus de descendants confirmés, historique des effets partiels et restauration/reprise corrigés. | Corbeille Windows conservée selon le choix utilisateur ; course de suppression par chemin explicitement ouverte. |
| R3 | Admission commune, job agrégé natif, surveillance de pression et budget commun des pixels conservés. | Enveloppe définie ci-dessous ; pas de plafond exact pour tout le parent, le GPU, les serveurs COM externes ou le disque physique. |

## Commits et organisation

- `e3d6d74` — `Preserve confirmed descendant effects in Windows delete history` : R2.
- `9e30a8f` — `Extract headless metadata and preview decoders` : préparation structurelle, sans activation du confinement ni du budget commun.
- `bcceb0e` — `Confine document workers and enforce shared resource admission` : lancement R1 et protections communes R3, avec leurs tests.

Les décodeurs purs sont dans `utils/image_preview_decode.rs` et `utils/office_preview.rs`. Le SVG reste dans le binaire GPUI et le worker d'image, sans introduire GPUI dans la bibliothèque backend. Les anciens tests d'image conservent leur module logique et leurs assertions dans `ui/image_preview/tests/decoder_tests.rs`. Les nouveaux tests unitaires sont dans les dossiers `tests/` des modules concernés.

## R1 — permissions réellement imposées

LPAC est le mécanisme Windows qui retire les permissions usuelles du worker. Le parent conserve les droits nécessaires à l'explorateur et lui transmet seulement le fichier choisi, ouvert en lecture, et ses canaux de communication. Cela réduit les accès possibles après compromission d'un décodeur.

Le choix utilisateur retient ce contrat et ses exceptions : une ACL accordant explicitement un accès au SID exact du worker ou à `ALL RESTRICTED APPLICATION PACKAGES` peut permettre cet accès. Le témoin ARAP reste effectivement lisible. Il ne s'agit pas d'une interdiction indépendante des ACL.

| Mode de production Windows | Exécutable | Permissions de production |
| --- | --- | --- |
| Archive-I/O Decode/Encode | `oxyde-archive-io-worker.exe` | LPAC sans capacité, contrat déjà acquis. |
| Listing/navigation interne d'archive et index de recherche | `oxyde-archive-list-worker.exe` | LPAC sans capacité, fichier source retenu et hérité en lecture. |
| Image, animation, SVG/SVGZ | `oxyde-image-worker.exe` | LPAC sans capacité, fichier source retenu et hérité en lecture. |
| Texte Office | `oxyde-image-worker.exe` | Même confinement ; extension passée comme indication, sans ouverture du chemin par le worker. |
| PDF, vidéo, thumbnail/office, cached-thumbnail/cached-office, icon-small/large | `oxyde.exe` | Jobs et surveillance de ressources ; permissions ordinaires du processus conservées. |

Le lancement confiné reste suspendu jusqu'à l'attachement des jobs. La liste explicite d'héritage comporte les trois handles IPC et, pour les nouveaux modes, le handle du fichier retenu. Les gardes des ancêtres restent vivantes dans le parent. Le worker ne reçoit aucun droit d'écriture sur la source. Les nouveaux parcours n'ont aucun repli vers le binaire complet ou le décodage local si leur exécutable manque.

Les tests vérifient la lecture du handle choisi, le refus de son écriture et le refus d'ouverture directe d'un fichier privé par son chemin. Les parcours réels conservent listing/index, pixels raster/animation, SVG avec texte et texte Office. Les protections existantes contre crash, timeout, annulation, saturation IPC, altération du runtime et sortie falsifiée sont exercées avec ces superviseurs.

### Étude des services natifs, sans activation en production

La question sur des permissions supplémentaires était prématurée pour l'utilisateur, qui ne comprenait pas les implications. La décision de cette passe est donc de conserver les permissions de production et de réaliser uniquement des sondes temporaires de test. Le variant de lancement et la dérivation de capacités ci-dessous sont derrière `cfg(test)`.

Résultats d'initialisation sur cette machine :

| Profil de sonde | COM | Media Foundation | Fabriques PDF et Storage WinRT |
| --- | --- | --- | --- |
| LPAC sans capacité | `80070005` | `80004001` | `800401F0` |
| LPAC + `lpacCom` | `0` | `80004001` | `80070005` |
| LPAC + `lpacCom` + `registryRead` | `0` | `0` | `0` |
| Même profil + `lpacMedia` | `0` | `0` | `0` |

Le profil `lpacCom` + `registryRead` refuse encore les accès aux fichiers privés testés, les écritures/créations privées, la clé HKCU privée du parent et les droits dangereux sur le parent, avec témoins positifs hors sandbox. Dans cette sonde, Winsock s'initialise et les connexions TCP de boucle locale IPv4/IPv6 sont refusées avec `10013`, avec témoins positifs. Cela ne prouve pas une interdiction universelle du réseau, ni UDP, ni l'Internet.

Ces résultats concernent l'initialisation des services et des ressources privées choisies. Ils ne valident pas un véritable rendu PDF/vidéo sous ce profil, ni tous les serveurs COM activables. Activer ces droits autoriserait davantage d'interactions avec les services Windows ; leur portée effective et les parcours de fichiers doivent encore être établis avant une décision concrète d'intégration.

En production sans capacité, l'échec Winsock `10107` reste un échec d'initialisation, sans preuve de refus TCP. ShellExecute et les launchers tiers hors verrou restent sans garantie universelle d'héritage. La sonde concurrente de pipes conserve ses assertions d'identité et de fermeture, mais s'exécute dans un sous-harnais isolé : d'anciennes fixtures de junction/exécutables lancent des enfants hors verrou et pouvaient prolonger ses handles temporaires.

**R1 reste ouvert pour les modes natifs précités, la couverture réseau et les launchers hors contrat.** Les exceptions ACL sont documentées selon le choix utilisateur ; aucun droit COM/registre supplémentaire n'est activé dans Oxyde.

## R2 — effets partiels de suppression Windows

Le sink global `IFileOperation::Advise` reçoit les callbacks de descendants. Avant une suppression, il capture l'identité du descendant admissible ; après, seuls les statuts de suppression confirmée sont journalisés. Un chemin absent, un callback étranger, une entrée sautée ou encore en attente ne constitue pas un reçu.

Le journal est borné à 65 536 entrées et 8 Mio de noms ; un dépassement arrête les éléments restants. Il conserve un effet confirmé même si la requête optionnelle du chemin du payload recyclé échoue. Il évite le double comptage des descendants contenus dans un payload de racine et conserve les payloads recyclés séparément, avec leurs dépendances de restauration.

L'historique conserve ces effets quand la racine échoue. Undo restaure d'abord les parents, puis leurs enfants recyclés séparément. Un conflit laisse les reçus non restaurés disponibles pour la reprise. Les clones d'historique partagent l'état des effets partiels ; Redo ne rejoue pas les effets déjà effectués. Le compteur de progression reste celui des racines choisies, sans compter chaque callback comme une racine supplémentaire.

Les essais comprennent des callbacks adverses, une suppression permanente native partielle avec enfant verrouillé, et la restauration de payloads racine/enfant dans une corbeille de fixture privée, avec conflit puis reprise. Aucun document personnel ni corbeille personnelle n'a été supprimé pour les tests.

La corbeille Windows continue de travailler par nom. La retenue et PreDelete réduisent les courses sans rendre la suppression atomique sur l'objet initial. Cette limite a été explicitement conservée par l'utilisateur. Les fournisseurs Shell et volumes ne sont pas tous validés ; R4 reste distinct.

## R3 — enveloppe commune et récupération

| Ressource | Politique | Nature de la garantie |
| --- | --- | --- |
| Ensemble des workers de parsing | 2,5 Gio de mémoire engagée, 4 processus | Job Windows agrégé, imbriqué avec les jobs individuels ; pas de breakaway. Le parent est hors job. |
| CPU des workers du job commun | 70 % | `JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP`, valeur native relue par test. Pas de borne CPU pour le parent ou les serveurs externes. |
| Préparation + workers actifs | Réservation commune de 2,5 Gio et 4 places | Admission logique avant préparation du runtime ; enveloppes individuelles conservées. |
| Disponibilité pour petits workers | 512 Mio et une place réservés face aux workers de plus de 512 Mio | Réduit la concurrence des grosses tâches ; ce n'est pas une garantie de latence de toute navigation. |
| Admission occupée | Attente annulable, vérification toutes les 10 ms, maximum 30 s | Refus si le budget reste occupé ; les délais propres aux opérations restent applicables. |
| Pression du parent/système | Parent : 2 Gio de mémoire privée engagée ou 8 192 handles ; disponibilité physique/commit inférieure à 512 Mio | Mesure toutes les 250 ms, refus/arrêt du travail parser ; ce ne sont pas des plafonds natifs instantanés. Échec de mesure : refus. |
| Handles d'un worker | Arrêt à partir de 4 096 handles observés | Mesure pendant la supervision, pas un maximum dur instantané. |
| Pixels CPU d'aperçus conservés | 256 Mio et 256 `RenderImage` | Budget logique commun image/PDF/vidéo ; comptage des octets réels, déduplication des clones, libération après le dernier propriétaire. |
| Copie du runtime confiné | Exécutable limité à 128 Mio | Vérification de la taille annoncée et du flux effectivement copié. |

Le job commun est attaché avant le job individuel, avant reprise du thread confiné ou ouverture de la gate de parsing des anciens workers. Toute erreur d'attachement arrête le lancement. Les réservations restent détenues pendant une terminaison non confirmée : un reaper attend la sortie et nettoie uniquement les objets retenus, avant de libérer la place. Un échec de duplication/lancement du reaper conserve la réservation plutôt que suradmettre. Les enfants étrangers du runtime ne sont pas supprimés.

Les tests couvrent concurrence d'enveloppes différentes, réserve pour petits workers, annulation avant admission, erreur de préparation, délai de sortie et récupération. Un vrai job agrégé de test refuse deux allocations de 48 Mio sous une limite commune de 64 Mio alors qu'elles tiennent chacune dans un job individuel de 128 Mio. Un vrai worker dépassant 4 096 handles est arrêté puis un autre worker peut démarrer. Le budget pixels couvre les clones, leur libération et l'admission concurrente.

Cette enveloppe ne borne pas exactement le RSS total, les allocations temporaires déjà réalisées par le parent, toutes les files de travaux, les textures GPU, les serveurs COM externes, les flux et l'espace disque physique. Les caches conservent leurs quotas locaux ; les inconnus après crash ne sont pas purgés. R6 et R7 restent ouverts. **Les protections R3 sont livrées dans ce périmètre ; une garantie universelle de non-saturation reste hors preuve.**

## Validation de la branche

Le 13 octobre 2026, Windows, configuration de développement :

- `cargo test --all-targets --offline` avec TEMP/TMP dans un dossier privé du dépôt : **986 réussis, 0 échec, 17 ignorés**. Aucun filtre ni exclusion dans cette exécution finale.
- Répartition : bibliothèque 672/16 ignorés, application 198/1 ignoré, pipeline archive 20, worker archive-I/O 37, listing 27, preview 32. Les binaires auxiliaires et l'exemple ont zéro test. Le résultat d'un sous-harnais déjà inclus dans le test parent n'est pas ajouté une seconde fois.
- `cargo check --all-targets --offline` et `cargo build --bins --offline` : réussis sans avertissement du projet.
- `git diff --check` : réussi. Dépendances `paste`, `rustybuzz` et `ttf-parser` et `Cargo.lock` inchangés. Clippy n'a pas été relancé pour cette passe.

Une régression réelle du test PDF a été détectée dans une version intermédiaire : l'introduction de GPUI dans la bibliothèque backend modifiait le comportement du binaire de test Windows. GPUI a été retiré du backend et le rendu SVG passé par callback depuis les deux exécutables concernés. Le test PDF original, y compris ses pixels exacts, passe sans affaiblissement d'assertion. Les échecs intermédiaires de fixtures de concurrence/reprise ont été corrigés avant l'exécution finale, sans exclusion persistante.

Le journal local est `target/security-validation/r123-final-387af9ddf8144754ad83ed4288a38eb7/tests-verified.log` ; il est ignoré par Git. Les 852 tests et le retour visuel du 10 octobre, ainsi que les 908 tests du 12 octobre, restent des bornes historiques distinctes.

## Distribution et vérification manuelle restante

Distribuer côte à côte `oxyde.exe`, `oxyde-archive-io-worker.exe`, `oxyde-archive-list-worker.exe` et `oxyde-image-worker.exe`, construits avec `cargo build --bins`. Un sibling absent échoue sans repli moins restrictif pour le mode concerné.

La validation visuelle de cette branche et l'installation propre restent à faire par l'utilisateur : archives/navigation/recherche, images/GIF/SVG avec texte, texte Office, PDF/vidéo/miniatures/icônes, suppression puis Undo/Redo et reprise après conflit. Vérifier aussi les messages existants en cas de worker absent ou de budget refusé. Aucun Computer Use, push ni suivi automatique n'a été réalisé. Cette passe ne reprend pas implicitement R4–R9 et ne valide pas Linux.
