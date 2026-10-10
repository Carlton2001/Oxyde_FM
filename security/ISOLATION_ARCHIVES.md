# Isolation de l’extraction et de la reconstruction d’archives

Date : 10 octobre 2026. Lot 68 : cadrage documentaire, avant implémentation.
Base de reprise : `master`, `c0f9c58` (arbre propre au début du cadrage).

## Point de départ et résultat attendu

La passe précédente est terminée : 782 tests réussis, aucun échec, 17 tests préexistants ignorés ; check et build sans avertissement. La validation visuelle utilisateur consignée en tête de [VERIFICATION_VISUELLE.md](VERIFICATION_VISUELLE.md) reste acquise pour cette passe. Ces résultats sont historiques, pas des contrôles réexécutés au lot 68.

Le chantier doit empêcher un décodeur ou un encodeur d’archives bloqué, en dépassement mémoire ou en crash de bloquer ou de faire tomber le processus principal sous Windows. Le parent conserve la sélection des objets, les contrôles de chemins, les écritures, la provenance, la publication et les récupérations. Les garanties des lots antérieurs sur les objets retenus doivent traverser cette nouvelle frontière.

Ce document fixe le périmètre et les critères de fin. Aucun code Rust n’est modifié au lot 68 ; l’isolation demeure à réaliser. Il n’y a ni Computer Use, ni lancement d’interface par l’agent, ni suivi automatique, ni push. Chaque lot d’implémentation sera vérifié et commité localement. Les essais visuels seront effectués par l’utilisateur lorsque le code et le binaire seront terminés.

## Périmètre fonctionnel

| Parcours à migrer sous Windows | Formats et comportement à conserver |
| --- | --- |
| Extraction explicite, via `FileOperationManager::perform_extract_prepared` et les wrappers de commandes | ZIP, 7z, TAR, TAR.GZ, TAR.XZ, TAR.BZ2, TAR.ZST, ISO/IMG ; dossier distinct, aucun écrasement, nettoyage sur échec |
| Ouverture d’un fichier interne et export permanent | Mêmes formats ; sélection exacte, cache privé et quotas existants, suffixe de collision pour export ; décompression des voisins seulement si le format l’exige |
| Préparation d’`ArchiveWorkspace` | Décodage complet borné, original retenu et empreinte, fichiers/dossiers ordinaires seulement |
| Reconstruction après création de dossier, copie, déplacement, suppression ou renommage interne | ZIP, 7z, TAR, TAR.ZST seulement, conformément à `writable` ; qualité choisie, transfert entre archives, contrôles de conflit et récupération |
| Création d’une nouvelle archive | ZIP, 7z, TAR, TAR.ZST ; même encodeur que la reconstruction, sources préparées et publication exclusive existantes |

RAR reste refusé sans appel à un outil externe. Aucun nouveau format, chiffrement, mot de passe ou support de liens n’est introduit. ISO et TAR compressées autres que TAR.ZST restent en lecture seule pour les modifications.

L’isolation native de cette passe concerne Windows. Le contrat IPC et les adaptateurs de codecs seront portables ; les appels système passeront exclusivement par `Platform`, avec implémentation Windows et comportement Linux explicite derrière les conditions de compilation. Linux conserve son exécution locale actuelle, sans prétention d’isolation nouvelle. Un échec du lancement Windows ne doit jamais déclencher de repli local.

La navigation et l’indexation de recherche utilisent déjà des workers distincts : leurs budgets et admissions restent indépendants. La fermeture principale conserve l’ordre et le contournement documentés dans [SHUTDOWN_DIAGNOSTICS.md](../SHUTDOWN_DIAGNOSTICS.md).

## Hors périmètre et limites qui resteront ouvertes

- Sandbox des permissions, jeton restreint/AppContainer, accès réseau et protection contre un worker compromis exécutant volontairement des appels système arbitraires. Le courtier réduit les pouvoirs du protocole ; le worker conserve les droits du compte. Un Job Object ne constitue pas un sandbox de sécurité.
- Budget global RSS/GPU/handles de l’application, des autres workers ou des serveurs COM ; validation exhaustive des fournisseurs de volumes, partages et identifiants natifs.
- Course Shell par chemin, reçus des descendants partiellement supprimés et autres chantiers de corbeille/historique sans lien direct avec cette migration.
- Atomicité globale des transferts entre plusieurs archives : conserver les récupérations existantes, sans promettre une transaction multi-archives indivisible.
- Validation sémantique de tous les octets produits par un worker compromis. Une réponse terminale correcte ne prouve pas à elle seule que l’archive encodée représente exactement les sources ; les tests de round-trip couvriront les codecs normaux.
- Mise à jour ou remplacement de `paste 1.0.15`, `rustybuzz 0.20.1`, `ttf-parser 0.25.1`. Ces trois dépendances sont conservées, avis visibles, selon [DEPENDENCY_MAINTENANCE.md](DEPENDENCY_MAINTENANCE.md). Aucune actualisation du graphe Cargo n’est prévue pour cette passe.

## Architecture retenue

### Parent : courtier d’objets et responsable des effets

Le parent ouvre la source avec `HeldParents` et `Platform::open_archive_source`, capture les informations nécessaires et conserve les handles jusqu’au dernier effet ou à la récupération. Il prépare les sources de compression avec `compression_sources` et vérifie leur identité sur le handle réellement lu, y compris `retained` et les transferts depuis une archive. Il conserve les verrous de mutation et leur ordre stable.

Les messages du worker désignent des identifiants de ressources propres à la session, jamais un chemin natif à rouvrir. Le parent sert des lectures bornées avec offsets contrôlés depuis les objets retenus. Les adaptateurs `Read + Seek` côté worker remplacent les dépendances actuelles à `File` dans ZIP, 7z et ISO. Les offsets de chaque lecteur sont indépendants : ne pas supposer que `File::try_clone` produit un curseur indépendant.

Pour l’extraction, le worker annonce des noms internes, types, tailles et métadonnées autorisées, puis émet des blocs de contenu. Le parent revalide les noms avec `relative_archive_path`, les budgets et l’ordre des messages avant toute création. Il utilise `ExtractionRoot`, garde les parents et writers nécessaires, crée exclusivement les objets, applique les dates admissibles et la provenance capturée côté parent. Les liens, points de réanalyse, flux alternatifs et types spéciaux restent refusés.

Pour l’encodage, le parent transmet un plan de sources avec noms internes et métadonnées issues des handles vérifiés. Le worker lit uniquement les identifiants autorisés et produit les écritures/positions d’un writer distant `Write + Seek`. Le parent applique ces écritures à une seule sortie privée retenue via `compression_output::Output`. Chaque offset et longueur est vérifié, y compris les écritures arrière nécessaires à ZIP/7z ; la taille maximale du fichier est comptée séparément du cumul des réécritures.

Le worker ne choisit ni le répertoire final, ni un nom de backup, ni les chemins de récupération. Le parent effectue flush/sync, publication exclusive, vérification de l’original (identité, taille, date et BLAKE3), provenance, sauvegarde, remplacement et rollback. Une sortie incomplète ou une session en erreur n’est jamais publiée comme résultat réussi.

### Worker : codecs sans état applicatif

Un mode interne versionné est dispatché avant logs utilisateur, préférences, caches et GPUI. Il attend la porte de démarrage après l’attachement au Job Object. Une session exécute une opération puis se termine ; aucun cache d’archive ni handle de destination persistant dans le worker.

Les codecs effectuent indexation, décompression, vérification CRC et encodage dans le worker. Le parent n’instancie plus de parseur d’archive pour le précontrôle, les totaux, le contrôle du résultat ou la préparation des workspaces Windows. Les validations lexicales, comptages, empreintes BLAKE3 et copies de blocs demeurent côté parent : elles ne décodent pas l’archive.

Les helpers locaux restent accessibles au worker et à Linux ; les entrées de production Windows passent toujours par le courtier. Les tests unitaires locaux ne suffisent pas à prouver cette propriété : les tests d’intégration doivent invoquer les wrappers réels.

### Protocole borné et défiance envers les réponses

Contrat à figer au lot 69 : version, opération, identifiant de session et séquence monotone, lecture de source, début/fin d’entrée, données, contrôle pause/reprise/turbo, erreur typée et résultat terminal. Aucun pointeur ou handle brut accepté depuis le worker.

Valeurs de départ fixées pour l’implémentation : blocs de données et lectures de 256 Kio maximum, trames de 1 Mio maximum en incluant l’en-tête, noms internes de 32 Kio maximum en UTF-8, détails d’erreur de 16 Kio maximum. Vérifier les longueurs et les additions avant allocation. Les plans/index sont transmis par pages ; une limite de trame ne devient pas une allocation de toute l’archive ou de tout le manifeste. Les files IPC doivent avoir une capacité finie ; aucune accumulation de toute la sortie en `Vec<u8>`.

Le parent rejette version inconnue, ressource étrangère, séquence incorrecte, nom ambigu, doublon incompatible, type interdit, taille incohérente, dépassement d’offset, trame tronquée, données après fin d’entrée, résultat multiple ou données après résultat. Une réponse partielle/EOF prématuré, un crash ou une erreur système échoue de façon fermée.

ZIP conserve le précontrôle de tout l’index avant les premiers fichiers, y compris les noms dangereux situés après une entrée saine. Le worker envoie le manifeste borné ; le parent l’admet entièrement avant d’autoriser les données. L’ouverture ciblée ZIP/7z/ISO conserve le contrôle d’index et l’unicité de la sélection. TAR reste parcouru de manière bornée en flux ; aucune garantie nouvelle de précontrôle global TAR n’est annoncée.

Les erreurs transportent des codes et valeurs numériques vérifiés (limite, délai, conflit de source, corruption, arrêt utilisateur, worker indisponible). Les traductions sont choisies dans le parent selon les conventions UI ; un texte arbitraire du worker ne commande ni une action ni un chemin de récupération.

## Ressources, pause et arrêt

| Paramètre | Contrat de la passe |
| --- | --- |
| Admission | Au plus deux sessions de ce chantier par instance : une ouverture/export ciblé et une extraction/workspace/encodage. Attente annulable, distincte des slots de navigation/recherche/aperçus. Aucun slot gardé d’une phase de décodage à une phase d’encodage ; éviter l’acquisition imbriquée lors des transferts. |
| Mémoire Windows | Plafond Job Object de 1 Gio par session, processus unique et destruction à la fermeture du job. Il s’agit de mémoire engagée privée, pas d’un plafond RSS global. Valeur proposée à confirmer par les essais de codecs au lot 69, notamment 7z « best » ; toute révision sera chiffrée et documentée avant migration, sans repli local ni réduction silencieuse de qualité. |
| Extraction complète | Limites actuelles : 100 000 entrées, 64 Gio de contenu, profondeur 64, 30 minutes actives ; bytes déclarés/écrits/décodés vérifiés selon le format. |
| Workspace | 8 Gio, autres limites d’extraction conservées. |
| Ouverture/export ciblé | 256 Mio de contenu sélectionné, 120 secondes actives, autres limites actuelles conservées ; budgets des blocs solides et flux voisins conservés. |
| Reconstruction | Plan de workspace limité à 8 Gio de données ; délai supervisé de 30 minutes actives pour l’encodage. |
| Création d’archive | Nouveau budget explicite pour le chemin partagé : 100 000 entrées, profondeur 64, 64 Gio de sources et 30 minutes actives. Cette limite nouvelle doit recevoir une erreur traduite et des tests de frontière ; ne pas l’assimiler à une limite déjà présente en `perform_compress_impl`. |
| Sortie encodée | Taille maximale : données admises + 64 Mio + 4 Kio par entrée, additions vérifiées. Les réécritures du writer ne gonflent pas artificiellement cette taille. Tester les sorties incompressibles et les finalisations ZIP/7z/TAR.ZST ; ajustement documenté si un format valide exige davantage. |
| Arrêt supervisé | Observation des contrôles au plus toutes les 50 ms hors appel système bloqué ; fermeture des pipes et job sur annulation, dépassement, protocole invalide ou sortie du parent. Attente native finale bornée à 1 seconde, sans `wait` ou `join` infini. |

Le superviseur doit pouvoir traiter annulation et échéance pendant une lecture **et pendant une écriture** IPC, y compris si le worker cesse de vider son entrée. La supervision actuelle des réponses de listing n’est pas réutilisable telle quelle : elle n’a pas ce dialogue bidirectionnel de contenu.

Pause : arrêter les nouveaux effets/lectures du courtier dès observation, obtenir un accusé du worker à un point sûr et exclure seulement la pause confirmée du temps actif. Sans accusé sous une seconde, garder le délai actif et l’annulation opérationnels ; ne pas accorder une suspension illimitée à un codec bloqué. Après accusé, le worker attend la reprise, ne décode/encode plus et n’émet plus de contenu. Les données en vol sont bornées. L’annulation reste possible en pause et en attente d’admission ; les compteurs et la vitesse ne progressent pas en pause confirmée.

Turbo conserve le comportement de qualité et de priorité existant selon le format ; les changements de priorité native du worker passent par `Platform`. Aucun mutex de `FileOperation` n’est conservé pendant l’IPC ou une attente.

Ces bornes concernent la supervision du worker. Elles ne promettent pas d’interrompre un appel de pilote ou un fournisseur de fichiers bloqué dans le parent ; cette limite doit rester explicite dans le bilan final.

## Publication, nettoyage et récupération

L’ordre de fin est : derniers contrôles de protocole/budget, résultat terminal valide, fin du transport et statut du worker compatible avec le mode de sortie native, flush/sync et fermeture de session, puis contrôles et effets finaux du parent. Le succès annoncé par le worker n’autorise pas un remplacement à lui seul.

Une annulation admise avant publication interdit une nouvelle publication. Une fois la phase de remplacement de l’original engagée, terminer le rollback ou la récupération avant de rendre l’état final ; ne pas interrompre entre deux renommages au détriment de l’original. Les effets déjà validés entre plusieurs archives restent décrits dans le résultat et les récupérations existantes.

Le nettoyage cible uniquement les objets effectivement créés et retenus par la session. En présence d’un remplacement ou d’un enfant étranger, préserver l’objet étranger et signaler la récupération nécessaire. Les suppressions récursives par chemin des dossiers provisoires (`PendingDirectory`, `TempDir`) doivent être examinées aux points migrés : ne pas reprendre aveuglément un `remove_dir_all` après fermeture des handles. Le journal du courtier doit rester borné et conserver assez de preuves pour ce nettoyage.

L’ouverture externe n’est déclenchée qu’après succès complet du fichier ciblé et attribution de sa provenance. Le cache ouvert par une autre application conserve son cycle de vie et ses quotas actuels. Aucun nettoyage global de TEMP ou des anciens jeux de validation.

## Lots et critères de sortie intermédiaires

Chaque lot est commité sur `master` après ses vérifications, sans push. L’audit indique exactement les parcours migrés et ceux encore locaux ; les garanties partielles ne deviennent pas un bilan de chantier terminé.

| Lot | Livrable | Critère de sortie du lot |
| --- | --- | --- |
| 68 | Présent cadrage et lien dans l’audit | Périmètre, exclusions, limites, matrice de tests et critères de fin écrits ; aucun code modifié |
| 69 | Contrat IPC, machine d’états, adaptateurs de lecture/écriture, essai des codecs sur flux distants | Tests de séquences/offsets/longueurs ; round-trip des quatre encodeurs et lecture des huit familles ; mesure 7z best, validation des valeurs mémoire/sortie et des métadonnées nécessaires avant raccordement aux effets réels |
| 70 | Supervision native Windows et admission via `Platform` | Vrai worker caché sous job, porte avant parsing ; crash/mémoire/délai/annulation/IPC bloqué/pause/reprise testés ; libération des ressources et aucune attente illimitée ; comportement Linux explicite |
| 71 | Extraction complète et décodage de workspaces | Tous les formats du périmètre passent par le worker ; ZIP précontrôlé ; validation des sorties côté courtier et nettoyage possédé ; sources retenues/concurrence/provenance sans régression |
| 72 | Ouverture ciblée, cache et export permanent | Uniquement le fichier demandé publié pour chaque format ; CRC/voisins solides, quotas, sélection ambiguë, provenance et collisions testés via les wrappers de production |
| 73 | Encodage commun de compression/reconstruction | Sources et sortie gardées côté parent ; ZIP/7z/TAR/TAR.ZST normal/fast/best, turbo, nouvelles limites et round-trip par le vrai worker ; publication exclusive conservée |
| 74 | Raccordement complet des mutations et transferts | Tous les callers de `commit_with_retained` migrés ; conflits d’original, sources retained, transferts inter-archives, annulation avant/après début du commit, échecs de publication/rollback/récupération vérifiés |
| 75 | Validation automatisée finale, build et protocole visuel ciblé | Critères ci-dessous satisfaits ; relevé des résultats exacts, risques restants, binaire et fixtures préparés ; attente du retour visuel utilisateur clairement indiquée |

Les numéros 69–75 sont prévisionnels. Un lot trop large sera scindé avec ses critères propres ; il ne sera pas commité à moitié fonctionnel ni déclaré terminé pour tenir ce découpage.

## Matrice de validation automatisée

| Axe | Preuves requises |
| --- | --- |
| Transport hostile | Faux worker/trames : longueurs avant allocation, ordre/identifiants/offsets, doublons, fin prématurée, erreur typée invalide, stdout après résultat ; parent préservé et aucune publication |
| Superviseur réel | Worker sans réponse, lecture/écriture IPC bloquée, crash, dépassement du job, échec de lancement/attachement, annulation en admission/décodage/encodage/pause ; autre opération admise ensuite et fichiers témoins réouvrables |
| Pause et temps actif | Pause confirmée plus longue que le délai de test sans timeout ; worker muet sans accusé ne neutralise pas le délai ; reprise et annulation sans progression ni effets pendant la pause confirmée |
| Codecs réels | Extraction normale/corrompue/tronquée sur chaque format ; CRC 7z individuel/bloc et ZIP, TAR compressé, extents ISO ; ouverture ciblée sans publication de voisins ; reconstruction round-trip aux qualités existantes, contenu/noms/dossiers/dates admissibles |
| Limites | Petits budgets injectés aux frontières entries/depth/bytes/durée/sortie ; déclarations trompeuses et données réellement reçues ; aucune archive géante nécessaire ; compteurs vérifiés côté parent |
| Concurrence et provenance | Remplacement source/parent, reparse/jonction, lien/ADS, original différent à taille/date identiques, même contenu mais autre identité, mauvais handle retained ; témoins étrangers et original conservés, provenance Windows encore appliquée |
| Publication et rollback | Collision de destination, annulation avant publication, échec de sync/rename, restitution de backup et échec de restitution ; récupérations explicitement conservées, aucun succès trompeur ni suppression d’objet étranger |
| Parcours utilisateur sans UI | Wrappers réels d’extraction/ouverture/export/compression/mutation et transferts entre deux archives ; erreur Windows sans repli local ; listing/recherche/aperçus capables d’avancer avec un worker de ce chantier occupé |

Ne pas activer les 17 tests historiquement ignorés ni exécuter des suppressions Shell sur des documents personnels. Les essais destructifs utilisent uniquement de petits témoins privés et des fautes injectées. Les tests de délai doivent contrôler les états et tolérer l’ordonnancement, pas dépendre d’une archive énorme ou d’une vitesse de machine particulière.

## Critères de fin du chantier

Le code est terminé seulement lorsque **tous** les points suivants sont établis :

1. Tous les parcours Windows du tableau de périmètre exécutent leurs parseurs/codecs dans un processus supervisé ; recherche des appels directs et tests des wrappers à l’appui. Aucun repli local sur panne et aucun format laissé local dans ce périmètre.
2. Le courtier contrôle ressources, chemins, types, budgets et transitions avant les effets. Les handles/identités retenus, la provenance, les règles de collision et la comparaison BLAKE3 de l’original sont conservés jusqu’à la publication/récupération.
3. Les bornes mémoire, IPC, contenu, taille de sortie et temps actif sont effectives et chiffrées ; crash/timeout/cancel/protocole invalide ne publient pas un résultat incomplet. Admission, pause, reprise, turbo et progression respectent le contrat.
4. La matrice automatisée passe sur les workers réels et les fautes injectées ; le compte final est consigné, avec au moins la suite précédente sans régression ni nouveaux tests ignorés pour masquer un échec.
5. `cargo check --all-targets --locked --offline`, `cargo test --all-targets --locked --offline` et `cargo build --bin oxyde --locked --offline` passent sans avertissement du projet. Une vérification Linux pertinente est consignée si la cible/environnement est disponible ; sinon la limite de validation Linux est annoncée explicitement.
6. Aucun remplacement des trois dépendances, aucune modification de curseur. Si de nouveaux messages sont nécessaires, les cinq catalogues et les conventions de [UI_ARCHITECTURE.md](../UI_ARCHITECTURE.md) sont respectés.
7. L’audit distingue les garanties obtenues des limites encore ouvertes. Les commits locaux couvrent chaque lot et l’arbre de travail est propre ; aucun push.
8. Le binaire final et un jeu neuf de petites fixtures sont prêts. Le protocole visuel ciblé couvre extraction/ouverture/export, modification et transfert d’archives, pause/reprise/annulation/progression, erreurs traduites et fermeture avec une opération en cours. Aucune nouvelle validation visuelle n’est déduite de celle de `c0f9c58`.

La clôture visuelle est distincte de la fin du code : elle interviendra seulement après le retour de l’utilisateur sur ce nouveau binaire. Le lot 68 n’en demande pas et ne rouvre pas la passe précédente.
