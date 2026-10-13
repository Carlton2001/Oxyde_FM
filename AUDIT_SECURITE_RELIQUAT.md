# Oxyde — reliquat de sécurité pour la reprise

Point d'entrée pour les travaux restants. Borne visuelle historique : 10 octobre 2026, sur `master` après `30d77f7`, pour le code livré à `243140d`. L'ancien `AUDIT_SECURITE.md` a été retiré du checkout à `c8f70de` ; les rapports conservés dans `security/` et l'historique Git portent les preuves antérieures.

Actualisation du **13 octobre 2026**, sur `codex/security-r1-r2-r3` : R2 à `e3d6d74`, extractions à `9e30a8f`, confinement et budget commun à `bcceb0e`. Le [rapport R1/R2/R3](security/AUDIT_R123_RESULTATS.md) décrit les garanties, les décisions utilisateur et la nouvelle validation automatisée. La borne visuelle du 10 octobre reste historique. Les fichiers racines peuvent servir de façades ; consulter leurs sous-modules.

Passe suivante demandée sur cette même branche : **R4, R6, R7, R8 et R9**. Corrections et limites détaillées dans le [rapport R4/R6/R7/R8/R9](security/AUDIT_R46789_RESULTATS.md). R8 suit le choix utilisateur de préparer toutes les archives puis restaurer le groupe en cas d'échec ; R9 est corrigé pour les contenus et métadonnées définies de TAR non compressé, et reste ouvert pour les autres périmètres.

## Base acquise

- Isolation Windows de l'extraction complète/ciblée, du cache/export et des workspaces pour les huit familles d'archives ; création/reconstruction et mutations pour ZIP/7z/TAR/TAR.ZST. Aucun repli local Windows sur panne du worker.
- Courtiers bornés, sources et sorties retenues, contrôles de chemins et de provenance, comparaison identité/taille/date/BLAKE3 de l'original, publication exclusive, rollback et récupération des effets partiels.
- Navigation, recherche et aperçus Windows dans leurs workers distincts. LPAC sans capacité pour archive-I/O Decode/Encode, listing/index d'archives, images et texte Office. Les modes natifs PDF/vidéo/Shell restent hors LPAC. Job agrégé, admission commune et surveillance de pression livrés dans le périmètre R3 décrit au rapport ; les garanties du parent/GPU/COM/disque restent distinctes.
- Borne historique du 10 octobre : **852 tests réussis**, aucun échec, 17 ignorés préexistants ; check et build sans avertissement du projet. Retour utilisateur **« Tests visuels OK »** consigné dans [security/VERIFICATION_VISUELLE.md](security/VERIFICATION_VISUELLE.md).
- Borne automatisée du groupe 1, 12 octobre : **908 tests réussis**, aucun échec, 17 ignorés préexistants, avec TEMP/TMP privé ; check/build sans avertissement du projet, clippy avec avertissements préexistants. Source : [rapport groupe 1](security/AUDIT_GROUPE_1_RESULTATS.md). Sa checklist visuelle et de distribution reste non exécutée ; ne pas lui attribuer le retour visuel historique.
- Borne automatisée R1/R2/R3, 13 octobre : **986 tests réussis**, aucun échec, 17 ignorés, sans filtre/exclusion et avec TEMP/TMP privé ; check/build réussis sans avertissement du projet. Source : [rapport R1/R2/R3](security/AUDIT_R123_RESULTATS.md). Validation visuelle et installation propre de cette branche non exécutées ; Linux non validé.
- Nouvelle borne R4/R6/R7/R8/R9, 13 octobre : **1 023 tests réussis**, aucun échec, 17 ignorés, sans filtre/exclusion et avec TEMP/TMP privé ; check/build sans avertissement du projet. Source : [rapport détaillé et commits](security/AUDIT_R46789_RESULTATS.md). Les limites R4/R6, le cache legacy R7 et le périmètre restant R9 sont distingués du code livré ; aucune validation visuelle nouvelle ni validation Linux.

Le chantier d'isolation des archives est clôturé dans son périmètre. Il n'est pas à recommencer pour traiter le reliquat. Ces résultats ne sont pas une certification générale de sécurité.

## Priorités proposées pour une prochaine passe

R1/R2/R3 ont fait l'objet de la passe demandée le 13 octobre. Le tableau distingue les protections livrées de leurs limites restantes ; aucune exploitation nouvelle par corruption mémoire n'a été démontrée.

| ID | Point ouvert et statut | Critères de fin à préciser au cadrage |
| --- | --- | --- |
| R1 | **Correction partielle étendue.** Archive-I/O, listing/index d'archives, images et texte Office en LPAC sans capacité ni repli. Exceptions ACL retenues selon le choix utilisateur. | Reste PDF, vidéo, miniatures/Shell et icônes ; réseau de production et héritage universel hors preuve. Sondes COM/registre exclusivement en tests, aucun droit supplémentaire activé. Voir le rapport avant toute intégration de permissions natives. |
| R2 | **Effets partiels et historique corrigés à `e3d6d74`.** Reçus des descendants confirmés, payloads séparés, restauration par niveaux et reprise après conflit. | Corbeille Windows conservée selon le choix explicite utilisateur ; course par chemin ouverte. Pas d'atomicité sur l'objet initial, ni validation de tous les fournisseurs Shell/volumes. Tests sur fixtures privées. |
| R3 | **Enveloppe commune livrée à `bcceb0e`.** Job agrégé 2,5 Gio/4 processus/CPU 70 %, admission avant préparation, réserve pour petits workers, pixels conservés 256 Mio/256 images, mesures de pression et récupération testée. | Parent/handles sous politique mesurée, sans plafond instantané ; RSS global, GPU, COM externe, disque physique et cache inconnu après crash hors garantie. Ne pas transformer ce périmètre en promesse universelle de non-saturation. |

Emplacements de départ :

- R1 : confinement dans [windows_worker_sandbox.rs](src/platform/windows_worker_sandbox.rs), coordination dans [windows_process_launch.rs](src/platform/windows_process_launch.rs), binaires minimaux dans [src/bin](src/bin), intégrations [windows_archive_io_worker.rs](src/platform/windows_archive_io_worker.rs), [windows_archive_worker.rs](src/platform/windows_archive_worker.rs) et [windows_preview_worker.rs](src/platform/windows_preview_worker.rs). Contrat système dans [platform/mod.rs](src/platform/mod.rs) ; services natifs restants dans les modes internes de [main.rs](src/main.rs).
- R2 : [windows_shell_delete_tracked.rs](src/platform/windows_shell_delete_tracked.rs), [windows_shell_delete.rs](src/platform/windows_shell_delete.rs) et [windows_impl.rs](src/platform/windows_impl.rs) pour le parcours natif ; [file_ops/delete.rs](src/systems/file_ops/delete.rs) et [file_ops/execution.rs](src/systems/file_ops/execution.rs) pour suppression, exécution et historique, derrière la façade [file_ops.rs](src/systems/file_ops.rs). Côté UI : [trash_settings/actions.rs](src/ui/app/trash_settings/actions.rs), [trash_settings/deletion_policy.rs](src/ui/app/trash_settings/deletion_policy.rs) et [trash_settings/confirmation.rs](src/ui/app/trash_settings/confirmation.rs). Preuves historiques : lots 52, 59, 60 et correction 66 de l'audit complet.
- R3 : superviseurs ci-dessus ; admissions dans [preview_jobs.rs](src/ui/preview_jobs.rs), quotas/cache natifs dans [windows_preview_quota.rs](src/platform/windows_preview_quota.rs) et [windows_preview_cache.rs](src/platform/windows_preview_cache.rs), propriétaires des assets dans [tooltip/assets.rs](src/ui/components/tooltip/assets.rs), chargement et vidéo dans [tooltip/preview.rs](src/ui/components/tooltip/preview.rs) et [tooltip/video.rs](src/ui/components/tooltip/video.rs). Voir aussi [archive_open_cache.rs](src/systems/archive_open_cache.rs), [file_ops/scheduler.rs](src/systems/file_ops/scheduler.rs) et l'orchestration UI dans [app/file_operations.rs](src/ui/app/file_operations.rs) et [app/archives.rs](src/ui/app/archives.rs). Le point couvre aussi les limites des quotas logiques de caches : espace physique/flux et écritures externes ne sont pas automatiquement bornés.
- Protections communes R3 : [windows_worker_budget.rs](src/platform/windows_worker_budget.rs), [windows_resource_pressure.rs](src/platform/windows_resource_pressure.rs), [preview_budget.rs](src/ui/preview_budget.rs). Détails chiffrés et preuves dans le rapport R1/R2/R3.

## Autres limites : décision de portée avant implémentation

R4/R6/R7/R8/R9 ont fait l'objet de la passe suivante demandée par l'utilisateur. Le tableau distingue le code livré et ce qui reste sans preuve ; R5 demeure exclu. Ces limites ne sont pas clôturées par une validation visuelle.

| ID | Limite actuelle | Décision ou preuve nécessaire pour clôturer le point |
| --- | --- | --- |
| R4 | **Preuves natives élargies, matrice locale NTFS.** Identité nulle refusée, objets étrangers refusés sans suppression, liens physiques et modifications de provenance testés ; provenance originale comparée avant remplacement. | C: NTFS fixe validé. Autres volumes/fournisseurs, tous les ADS et snapshot immutable hors preuve. Un autre lien peut être renommé ; groupes d'archives liées au même original refusés. Voir le rapport et ses sources Microsoft sur les limites des IDs. |
| R5 | **Linux.** Les codecs restent locaux. La compilation croisée tentée s'arrête dans Fontconfig faute de sysroot/configuration `pkg-config`, avant validation d'Oxyde. | Décider si l'isolation Linux appartient à une prochaine passe. La validation de compilation/exécution sur Linux et l'ajout d'une isolation native sont deux objectifs distincts, chacun à délimiter. [linux_impl.rs](src/platform/linux_impl.rs), [security/ISOLATION_ARCHIVES.md](security/ISOLATION_ARCHIVES.md). |
| R6 | **Interruption coopérative ajoutée.** Moniteur du thread propriétaire pour le courtier, ouvertures/empreintes, reconstructions, cache et sources retenues. Scopes imbriqués, pause, délai et reprise native testés. | `CancelSynchronousIo` reste une tentative ; pilotes non coopératifs et appels hors scopes empêchent toute promesse de délai absolu du parent. Aucun thread du parent n'est tué, aucune garde n'est abandonnée pendant une I/O en cours. |
| R7 | **Nouvelles préparations récupérables avec preuves authentifiées.** Cache Windows `v2`, propriétaire version 4, clé privée Oxyde/DPAPI/MAC, journal des créations et écritures ; crash réel et reprise exercés. | Ancien cache `v1` et préparations version 3 sans preuve conservés, sans adoption ni purge globale ; hors quota logique du nouveau stockage. Clé inconnue/corrompue, objets étrangers/modifiés et états incomplets sans preuve suffisante protégés. [Implémentation](src/systems/archive_open_cache/recovery.rs). |
| R8 | **Préparation et restauration groupées livrées selon le choix utilisateur.** Originaux retenus jusqu'au succès de toutes les publications ; rollback de trois archives, annulation et remplacement étranger testés. | Pas d'atomicité de visibilité, ni de transaction durable multi-fichiers après crash/panne machine. Les backups non restaurables sont conservés et signalés. Retrait de sources ordinaires du filesystem distinct. [Parcours natif](src/systems/archive_workspace/native_commit.rs). |
| R9 | **Correction indépendante pour TAR non compressé.** Le parent dérive inventaire/digests depuis la source ou les octets fournis à l'encodeur, puis contrôle les résultats ; worker hostile avec protocole valide refusé avant publication. | Autres formats, listing/index et pixels/textes d'aperçus sans référence indépendante générale. Fonctions existantes conservées en attendant la décision demandée sur leur maintien/blocage ; aucune acceptation ni clôture déduite de l'absence de réponse. [Vérificateur](src/utils/tar_fidelity.rs). |

## Décisions déjà prises et sujets distincts

- **Conserver** `paste 1.0.15`, `rustybuzz 0.20.1`, `ttf-parser 0.25.1`, selon la décision utilisateur. Leur remplacement n'est pas une correction en attente autorisée. Avis et conditions de réexamen : [security/DEPENDENCY_MAINTENANCE.md](security/DEPENDENCY_MAINTENANCE.md). La revue RustSec du 9 octobre est historique ; ne pas la présenter comme une vérification actualisée.
- **Refactorisation des gros fichiers :** plusieurs extractions et déplacements de tests ont été réalisés depuis la borne du 10 octobre. Ils constituent des travaux de maintenance distincts et ne clôturent aucun point R1–R9. Les références ci-dessus suivent désormais les implémentations extraites. Ne pas recommencer les découpages déjà réalisés ; si une nouvelle extraction est utile au point choisi, nommer son périmètre, préserver le comportement et séparer ses commits des changements de sécurité. Pas de refonte générale implicite ni de nouveau défaut de sécurité déduit de la taille d'un fichier.
- **Fermeture PDF/Intel :** diagnostic et résolution dans [SHUTDOWN_DIAGNOSTICS.md](SHUTDOWN_DIAGNOSTICS.md). Consulter avant de modifier la sortie finale ; une ancienne mention de blocage n'est pas une régression actuelle confirmée.

## Comment reprendre sans recréer une liste sans fin

### Repères après les refactorisations

Les anciens chemins cités dans l'historique restent des points d'entrée, mais l'implémentation peut désormais être dans leurs enfants. Avant une correction, rechercher la fonction et ses appelants dans le répertoire concerné ; ne pas supposer que le fichier racine contient encore tout le parcours.

| Ancien ensemble | Organisation actuelle utile à la reprise |
| --- | --- |
| `src/systems/file_ops.rs` | Types/façade et modules `file_ops/{scheduler,execution,delete,copy,archive_transfer,compress,extract}.rs`, worker de copie dans `file_ops/copy/worker.rs`. |
| `src/ui/app/mod.rs` | État/assemblage et parcours extraits dans `file_operations.rs`, `archives.rs`, `clipboard.rs`, `rename.rs`, `drag.rs`, `context_menus.rs`, `render.rs` et `context_menu_render/`. |
| Liste et conflits | Façades `components/file_list.rs` et `components/conflict_dialog.rs`, implémentations dans leurs répertoires homonymes. Ne pas confondre décisions UI et garanties backend des transferts. |
| Sidebar, layout et workspace | Façades conservées ; sous-modules dans `components/sidebar/`, `app/layout/` et `ui/workspace/` pour arbre, onglets, filtres, opérations et snapshots. |
| Recherche avancée et doublons | `app/advanced_search/` et `app/duplicates/` séparent fenêtre, interactions/recherche et présentation. |
| Aperçus et propriétés | `components/tooltip/` contient assets, preview, vidéo, présentation et modules de placement/survol/cache PDF ; `app/properties/` contient chargement, interactions et présentation Général/Raccourci. Ces répertoires comportent aussi des évolutions ultérieures : ne pas déduire l'équivalence fonctionnelle de la seule mention de refactorisation. |
| Corbeille et paramètres | `app/trash_settings/` sépare actions, politique de suppression, confirmation, réglages et rendu ; `components/operations/` et `components/settings/` portent les sections extraites de leurs façades. |

Les tests de `commands/ops`, `systems/history_file_undo`, `commands/search`, `platform/thumbnails`, `systems/file_copy`, `systems/history_move`, `systems/history_move_redo` et `commands/archive` ont été déplacés dans les sous-répertoires `tests/` correspondants. Les déclarations `#[path]` conservent les modules logiques ; rechercher les scénarios dans ces fichiers avant d'ajouter des doublons. D'autres tests peuvent rester intégrés : cette liste ne prétend pas couvrir tout le dépôt.

L'extraction du codec minimal dans `src/bin/oxyde-archive-io-worker.rs` à `3f135b2` est, elle, une préparation structurelle du lot R1 ; le confinement et ses nouvelles preuves sont livrés à `2e69e05`. Distinguer ces deux commits et leurs finalités. Aucune extraction structurelle ne clôture à elle seule un point de sécurité.

### Procédure de reprise

1. Lire [AGENTS.md](AGENTS.md), ce reliquat et le rapport du point choisi ; consulter l'historique seulement pour ce point. Lire [UI_ARCHITECTURE.md](UI_ARCHITECTURE.md) si l'interface est touchée.
2. Vérifier la branche, le commit et les changements locaux. La borne ci-dessus est celle de la dernière validation, pas une instruction de reset ; préserver les travaux ultérieurs et les fichiers utilisateur.
3. Choisir **un point par son ID**, définir périmètre, critères vérifiables et limites conservées avant de coder. La reprise de « tout le reliquat » exige d'abord un ordre et des décisions de portée.
4. Garder des commits distincts, exécuter les vérifications pertinentes et préparer les tests visuels utilisateur si le comportement visible change. Aucun Computer Use, suivi automatique ou push ; conserver les trois dépendances selon la décision actuelle.
5. Mettre à jour ce fichier quand le point est corrigé, reporté ou accepté, avec preuve/décision et commit. Un défaut nouveau confirmé est consigné séparément ; il ne doit pas agrandir silencieusement le chantier en cours.

Exemple de demande de reprise : « Reprends la branche codex/security-r1-r2-r3. Lis AGENTS.md, AUDIT_SECURITE_RELIQUAT.md et security/AUDIT_R123_RESULTATS.md. Traite le reste R1 ; explique les permissions nécessaires aux services natifs, leur portée et les critères de validation. »

L'actualisation du 13 octobre accompagne les corrections et tests rapportés. Elle n'engage pas de travaux supplémentaires sur R4–R9.

## R1 — protections livrées et reprise restante

Le premier lot validé par l'utilisateur concernait uniquement **archive-I/O Windows v1 : Decode complet/ciblé et Encode**. Modèle retenu : parent de confiance, worker pouvant exécuter du code arbitraire après compromission d'un codec ; noyau, lancement et runtime dans la base de confiance. Administrateur hostile et parent compromis hors contrat.

### Protections à conserver

- Exécutable minimal `oxyde-archive-io-worker.exe` en LPAC sans capacité, sans repli moins restrictif. Le binaire GPUI complet échouait à l'initialisation des DLL sous LPAC ; l'extraction du codec a permis de fonctionner sans élargir les permissions.
- Enfant créé suspendu, jobs attachés avant reprise, seuls les trois handles IPC nécessaires et, pour les nouveaux modes, le fichier sélectionné en lecture sont hérités ; environnement minimal et répertoire courant explicite. Profil bootstrap supprimé avant reprise ; buffers d'attributs conservés jusqu'à leur destruction.
- Course d'héritage corrigée pour les launchers `Command` Oxyde recensés : verrou partagé, fermeture des handles temporaires avant libération sur succès/erreur, waits hors verrou. Sonde concurrente réelle avec preuve d'identité des handles et BrokenPipe pendant la survie de l'autre enfant.
- Runtime privé à DACL protégée, lecture/exécution pour le SID exact de session ; sources, ancêtres, runtime et exécutable retenus, comparaison d'identité et BLAKE3. Altération des octets détectée et nettoyage par handles limité aux objets détenus ; enfant étranger conservé. Terminaison retardée : conservation des objets et journalisation PID/chemin.
- Sondes natives : refus de lecture/écriture/création privées, HKCU du parent et droits dangereux sur le parent ; témoins positifs hors sandbox. Runtime lisible mais modifications/DAC/owner/création dans cwd refusées. Témoin AAP lisible en AppContainer normal et refusé en LPAC.
- Timeout, annulation, crash, OOM, trames falsifiées et saturation IPC exercés avec readiness. Encodage/décodage ZIP/7z/TAR/TAR.ZST et lecture GZIP/XZ/BZIP2/ISO vérifiés ; garanties de publication/rollback conservées.

### Reste à traiter pour R1

1. **Modes natifs Windows** : thumbnail, office, PDF, cached-thumbnail, cached-office, icon-small/large et vidéo restent à confiner. Image, office-text, navigation/listing et index d'archives sont désormais en LPAC sans capacité à `bcceb0e`. Les besoins Shell/COM/Media Foundation/GPU des modes restants doivent être validés ; l'étude temporaire COM/registre décrite au rapport n'a pas activé ces droits en production.
2. **Réseau** : zéro capacité imposée et témoins TCP IPv4/IPv6 disponibles hors sandbox/AppContainer normal, mais Winsock échoue à l'initialisation sous LPAC (10107). Les connexions TCP n'ont pas été exercées : aucun refus de permission TCP ne peut être conclu de cet échec.
3. **Exceptions ACL retenues** : un accès explicitement accordé au SID ou à ALL RESTRICTED APPLICATION PACKAGES peut rester permis ; le témoin ARAP est effectivement lisible. L'utilisateur a choisi de retenir ce contrat, sans interdiction absolue indépendante des ACL.
4. **Héritage hors launchers recensés** : ShellExecute et launchers tiers hors verrou non validés. La correction ne démontre pas une garantie universelle.

Limites connexes à ne pas confondre avec une clôture : requête token de classe 46 en erreur 87, donc aucune propriété déduite ; renommage natif refusé par partage, mais branche de rejet d'une substitution d'identité non exercée (R4) ; I/O natives bloquées non interruptibles (R6) ; résidus inconnus après crash (R7). La préparation est désormais couverte par l'admission R3, dont le périmètre et les limites sont détaillés au rapport.

Commits : `3f135b2` (codec minimal) et `2e69e05` (confinement LPAC et preuves). [Rapport détaillé et commandes reproductibles](security/AUDIT_GROUPE_1_RESULTATS.md) : `cargo test --all-targets` avec TEMP/TMP privé, 908 réussis/17 ignorés ; `cargo check --all-targets` et `cargo build --bins` réussis sans avertissement du projet ; clippy réussi avec avertissements préexistants. Résultats rapportés, non relancés pour cette synthèse.

Livraison actuelle : distribuer les quatre exécutables `oxyde.exe`, `oxyde-archive-io-worker.exe`, `oxyde-archive-list-worker.exe` et `oxyde-image-worker.exe` côte à côte, construire avec `cargo build --bins`. Runtime absent/préparation refusée échoue sans repli UI. La checklist visuelle et d'installation propre reste manuelle et non exécutée. **R1 reste ouvert** pour les modes et preuves restants ; les seules exceptions ACL sont retenues selon le choix utilisateur. Borne nouvelle et commandes : [rapport R1/R2/R3](security/AUDIT_R123_RESULTATS.md).

## R9 — correction TAR livrée et reprise restante

Le même modèle de worker compromis s'applique. Les contrôles indépendants actuels bornent noms, ressources, offsets, tailles et transitions ; le parent retient les objets, applique la provenance, contrôle publication/récupération et compare l'empreinte de l'original. Cela protège le protocole et certains effets, pas la correspondance entre sources attendues et résultat produit.

Pour TAR non compressé, un vérificateur distinct dans le parent contrôle désormais les enregistrements, contenus et métadonnées définies, avec références dérivées indépendamment. Les tests d'un worker hostile consommant les sources puis renvoyant d'autres octets échouent sans publication. Le sous-ensemble TAR accepté et les refus d'extensions sont détaillés dans le rapport R4/R6/R7/R8/R9.

Restent sans preuve indépendante générale : autres formats, inventaires listing/index et pixels/textes des aperçus. Même si un encodeur consomme toutes les sources, ses blocs de sortie peuvent être arbitraires. Un hash fourni par le worker, le confinement LPAC ou un round-trip normal ne remplace pas une référence indépendante.

Alternatives pour le périmètre restant :

- Format canonique construit ou vérifié par le parent, en évaluant l'effet sur sa surface de confiance.
- Vérificateur indépendant isolé comparant inventaire et digests à un plan de confiance ; préciser indépendance réelle, sources des références et propriétés vérifiées.
- Référence externe fiable lorsque le scénario en dispose ; ce n'est pas une garantie générale pour toute archive.

La reprise doit définir les références indépendantes des formats/aperçus restants, puis tester les falsifications couvertes. **R9 général reste ouvert ; la correction livrée concerne le périmètre TAR précisé.** Une décision sur le maintien ou le blocage des fonctions sans vérification a été demandée et reste sans réponse. La documentation seule ne clôture pas ce reliquat ; aucun report ou acceptation de limite n'est déduit du silence.

R5 reste exclu selon la décision utilisateur, sans être corrigé : Linux n'a été ni compilé ni exécuté dans les passes rapportées. Ces mises à jour n'engagent aucun travail supplémentaire hors des points demandés.
