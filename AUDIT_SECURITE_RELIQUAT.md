# Oxyde — reliquat de sécurité pour la reprise

État du 10 octobre 2026, sur `master` après `30d77f7` (validation visuelle du code livré à `243140d`). Ce fichier est le point d'entrée pour les travaux restants ; [AUDIT_SECURITE.md](AUDIT_SECURITE.md) conserve les preuves et l'historique. Il synthétise le **bilan actuel**, sans transformer les anciennes mentions « encore à faire » en nouvelles tâches.

Références de code et bilan partiel R1/R9 actualisés le **12 octobre 2026**, avec les travaux livrés à `3f135b2` et `2e69e05`. La borne visuelle du 10 octobre reste historique ; les preuves automatisées plus récentes du groupe 1 sont distinguées ci-dessous. Cette synthèse documentaire ne constitue pas une nouvelle exécution des tests. Les fichiers racines peuvent désormais servir de façades ; consulter les sous-modules indiqués pour les implémentations.

## Base acquise

- Isolation Windows de l'extraction complète/ciblée, du cache/export et des workspaces pour les huit familles d'archives ; création/reconstruction et mutations pour ZIP/7z/TAR/TAR.ZST. Aucun repli local Windows sur panne du worker.
- Courtiers bornés, sources et sorties retenues, contrôles de chemins et de provenance, comparaison identité/taille/date/BLAKE3 de l'original, publication exclusive, rollback et récupération des effets partiels.
- Navigation, recherche et aperçus Windows dans leurs workers distincts. Les plafonds individuels et les admissions ne constituent pas un budget global. Le confinement de permissions livré le 12 octobre concerne uniquement archive-I/O Decode/Encode ; il ne s'étend pas automatiquement aux autres workers.
- Borne historique du 10 octobre : **852 tests réussis**, aucun échec, 17 ignorés préexistants ; check et build sans avertissement du projet. Retour utilisateur **« Tests visuels OK »** consigné dans [security/VERIFICATION_VISUELLE.md](security/VERIFICATION_VISUELLE.md).
- Borne automatisée du groupe 1, 12 octobre : **908 tests réussis**, aucun échec, 17 ignorés préexistants, avec TEMP/TMP privé ; check/build sans avertissement du projet, clippy avec avertissements préexistants. Source : [rapport groupe 1](security/AUDIT_GROUPE_1_RESULTATS.md). Sa checklist visuelle et de distribution reste non exécutée ; ne pas lui attribuer le retour visuel historique.

Le chantier d'isolation des archives est clôturé dans son périmètre. Il n'est pas à recommencer pour traiter le reliquat. Ces résultats ne sont pas une certification générale de sécurité.

## Priorités proposées pour une prochaine passe

L'ordre ci-dessous est une proposition à cadrer, pas une nouvelle série de travaux engagée. Les limites sont constatées ; aucune nouvelle exploitation par corruption mémoire n'a été démontrée. Chaque point exige un périmètre précis avant de coder.

| ID | Point ouvert et statut | Critères de fin à préciser au cadrage |
| --- | --- | --- |
| R1 | **Permissions des workers — correction partielle livrée.** Archive-I/O Decode/Encode fonctionne dans un LPAC sans capacité ni repli. Les autres workers restent à traiter ; réseau, exceptions ACL et couverture universelle de l'héritage restent ouverts. | Reprendre les preuves acquises et le reste à traiter détaillés en fin de document. Étendre le confinement aux modes restants avec tests permis/refusés et parcours ordinaires ; résoudre ou soumettre explicitement les limites restantes. Un Job Object seul ou le confinement d'un seul worker ne clôture pas R1. |
| R2 | **Suppressions Shell : course par chemin et effets partiels.** Risque résiduel malgré retenue des objets et contrôle PreDelete. Les reçus concernent les racines, pas tous les descendants d'un dossier partiellement supprimé. | Séparer la garantie sur l'objet visé de la couverture des effets/historique. Délimiter recyclage, suppression et reprise concernés ; vérifier les courses et erreurs sur fixtures privées, sans supprimer de documents personnels. Si l'API retenue ne permet pas la garantie voulue, choisir une alternative ou accepter explicitement la limite ; une vérification supplémentaire par chemin ne suffit pas à annoncer l'atomicité. |
| R3 | **Budget global de ressources.** Protection contre la saturation encore absente : plafonds de jobs et quotas locaux ne couvrent pas l'ensemble du parent, des workers, du GPU, des handles et des serveurs COM externes. | Définir les ressources réellement comptabilisables, une enveloppe chiffrée et la politique d'admission/récupération ; tester la concurrence et le retour à un état utilisable. Distinguer mesures/enveloppes logiques et plafonds effectivement imposés. Lister ce qui échappe au contrôle, sans annoncer un plafond RSS/GPU exact non mesuré. |

Emplacements de départ :

- R1 : confinement acquis dans [windows_worker_sandbox.rs](src/platform/windows_worker_sandbox.rs), coordination des lancements dans [windows_process_launch.rs](src/platform/windows_process_launch.rs), exécutable minimal [oxyde-archive-io-worker.rs](src/bin/oxyde-archive-io-worker.rs) et intégration [windows_archive_io_worker.rs](src/platform/windows_archive_io_worker.rs). Modes restants : [windows_preview_worker.rs](src/platform/windows_preview_worker.rs), [windows_archive_worker.rs](src/platform/windows_archive_worker.rs), modes internes dans [main.rs](src/main.rs) ; contrat système dans [platform/mod.rs](src/platform/mod.rs).
- R2 : [windows_shell_delete_tracked.rs](src/platform/windows_shell_delete_tracked.rs), [windows_shell_delete.rs](src/platform/windows_shell_delete.rs) et [windows_impl.rs](src/platform/windows_impl.rs) pour le parcours natif ; [file_ops/delete.rs](src/systems/file_ops/delete.rs) et [file_ops/execution.rs](src/systems/file_ops/execution.rs) pour suppression, exécution et historique, derrière la façade [file_ops.rs](src/systems/file_ops.rs). Côté UI : [trash_settings/actions.rs](src/ui/app/trash_settings/actions.rs), [trash_settings/deletion_policy.rs](src/ui/app/trash_settings/deletion_policy.rs) et [trash_settings/confirmation.rs](src/ui/app/trash_settings/confirmation.rs). Preuves historiques : lots 52, 59, 60 et correction 66 de l'audit complet.
- R3 : superviseurs ci-dessus ; admissions dans [preview_jobs.rs](src/ui/preview_jobs.rs), quotas/cache natifs dans [windows_preview_quota.rs](src/platform/windows_preview_quota.rs) et [windows_preview_cache.rs](src/platform/windows_preview_cache.rs), propriétaires des assets dans [tooltip/assets.rs](src/ui/components/tooltip/assets.rs), chargement et vidéo dans [tooltip/preview.rs](src/ui/components/tooltip/preview.rs) et [tooltip/video.rs](src/ui/components/tooltip/video.rs). Voir aussi [archive_open_cache.rs](src/systems/archive_open_cache.rs), [file_ops/scheduler.rs](src/systems/file_ops/scheduler.rs) et l'orchestration UI dans [app/file_operations.rs](src/ui/app/file_operations.rs) et [app/archives.rs](src/ui/app/archives.rs). Le point couvre aussi les limites des quotas logiques de caches : espace physique/flux et écritures externes ne sont pas automatiquement bornés.

## Autres limites : décision de portée avant implémentation

Ces points restent ouverts, mais n'imposent pas automatiquement un développement supplémentaire. Ils peuvent être traités, reportés ou conservés comme limites explicites. Ils ne doivent pas être déclarés corrigés par les tests visuels.

| ID | Limite actuelle | Décision ou preuve nécessaire pour clôturer le point |
| --- | --- | --- |
| R4 | **Volumes, fournisseurs et identités.** Validation non exhaustive ; identifiants/date non cryptographiques, alias/liens et modifications du contenu/ADS du même objet ne fournissent pas toutes les garanties d'un snapshot. | Fixer une matrice de cibles prises en charge ; tester retenue, remplacement, reparse, partage et identité sur ces cibles. Distinguer garanti, refusé et non validé. Ne pas promettre tous les fournisseurs. Départ : `source_selection`, `path_parents`, implémentations natives et preuves d'historique. |
| R5 | **Linux.** Les codecs restent locaux. La compilation croisée tentée s'arrête dans Fontconfig faute de sysroot/configuration `pkg-config`, avant validation d'Oxyde. | Décider si l'isolation Linux appartient à une prochaine passe. La validation de compilation/exécution sur Linux et l'ajout d'une isolation native sont deux objectifs distincts, chacun à délimiter. [linux_impl.rs](src/platform/linux_impl.rs), [security/ISOLATION_ARCHIVES.md](security/ISOLATION_ARCHIVES.md). |
| R6 | **Appels bloquants dans le parent.** Le délai du superviseur ne garantit pas l'interruption d'un appel de pilote/fournisseur bloqué dans le courtier. | Identifier les appels et cibles concernés, puis délimiter un mécanisme d'interruption ou de déplacement compatible avec les preuves d'objet/publication. À défaut, conserver cette limite de délai explicitement. Ne pas assimiler délai du worker et annulation de toute I/O native. |
| R7 | **Cache abandonné après crash.** Les préparations version 3 sans preuve persistante restent conservées et comptées dans l'admission. | Si leur récupération automatique est souhaitée, établir une preuve ou une politique de récupération justifiée, testée sur crash/remplacement/enfant étranger. Ne jamais résoudre le problème par purge globale de TEMP ou suppression d'inconnus par nom. [archive_open_cache.rs](src/systems/archive_open_cache.rs). |
| R8 | **Transferts entre archives.** Pas d'atomicité globale : un résultat peut être publié avant l'échec d'une autre archive. Les emplacements/effets conservés sont signalés. | Décider si une transaction multi-archives est nécessaire ou si ce contrat est conservé. Une amélioration d'atomicité exige son propre périmètre et ses essais de récupération après panne/crash. [archive_workspace.rs](src/systems/archive_workspace.rs), [file_ops/archive_transfer.rs](src/systems/file_ops/archive_transfer.rs), [file_ops/compress.rs](src/systems/file_ops/compress.rs) et [file_ops/extract.rs](src/systems/file_ops/extract.rs), derrière la façade [file_ops.rs](src/systems/file_ops.rs). |
| R9 | **Fidélité des sorties d'un worker compromis — analysé, non corrigé.** Les contrôles bornent le protocole et les effets, mais ne prouvent ni fidélité des contenus encodés/extraits, ni exhaustivité des inventaires, ni fidélité des pixels/textes. | Partir de l'analyse et des alternatives détaillées en fin de document pour définir puis implémenter une correction vérifiable. Un hash annoncé par le worker ou une relecture par le même codec n'est pas une preuve indépendante. Une impossibilité doit être démontrée ; report ou acceptation de limite nécessitent une décision utilisateur, pas une clôture documentaire. |

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

1. Lire [AGENTS.md](AGENTS.md), ce reliquat et le bilan actuel en tête de l'audit ; consulter l'historique seulement pour le point choisi. Lire [UI_ARCHITECTURE.md](UI_ARCHITECTURE.md) si l'interface est touchée.
2. Vérifier la branche, le commit et les changements locaux. La borne ci-dessus est celle de la dernière validation, pas une instruction de reset ; préserver les travaux ultérieurs et les fichiers utilisateur.
3. Choisir **un point par son ID**, définir périmètre, critères vérifiables et limites conservées avant de coder. La reprise de « tout le reliquat » exige d'abord un ordre et des décisions de portée.
4. Garder des commits distincts, exécuter les vérifications pertinentes et préparer les tests visuels utilisateur si le comportement visible change. Aucun Computer Use, suivi automatique ou push ; conserver les trois dépendances selon la décision actuelle.
5. Mettre à jour ce fichier quand le point est corrigé, reporté ou accepté, avec preuve/décision et commit. Un défaut nouveau confirmé est consigné séparément ; il ne doit pas agrandir silencieusement le chantier en cours.

Exemple de demande de reprise : « Reprends sur master. Lis AGENTS.md et AUDIT_SECURITE_RELIQUAT.md. Traite uniquement R1 ; précise les workers concernés, les critères de fin et les limites conservées avant de coder. Consulte AUDIT_SECURITE.md pour les preuves utiles. »

Ce fichier n'engage aucune implémentation. Création documentaire après la validation visuelle ; tests automatisés non relancés.

## R1 — protections livrées et reprise restante

Le premier lot validé par l'utilisateur concernait uniquement **archive-I/O Windows v1 : Decode complet/ciblé et Encode**. Modèle retenu : parent de confiance, worker pouvant exécuter du code arbitraire après compromission d'un codec ; noyau, lancement et runtime dans la base de confiance. Administrateur hostile et parent compromis hors contrat.

### Protections à conserver

- Exécutable minimal `oxyde-archive-io-worker.exe` en LPAC sans capacité, sans repli moins restrictif. Le binaire GPUI complet échouait à l'initialisation des DLL sous LPAC ; l'extraction du codec a permis de fonctionner sans élargir les permissions.
- Enfant créé suspendu, job attaché avant reprise, seuls les trois handles IPC nécessaires hérités, environnement minimal et répertoire courant explicite. Profil bootstrap supprimé avant reprise ; buffers d'attributs conservés jusqu'à leur destruction.
- Course d'héritage corrigée pour les launchers `Command` Oxyde recensés : verrou partagé, fermeture des handles temporaires avant libération sur succès/erreur, waits hors verrou. Sonde concurrente réelle avec preuve d'identité des handles et BrokenPipe pendant la survie de l'autre enfant.
- Runtime privé à DACL protégée, lecture/exécution pour le SID exact de session ; sources, ancêtres, runtime et exécutable retenus, comparaison d'identité et BLAKE3. Altération des octets détectée et nettoyage par handles limité aux objets détenus ; enfant étranger conservé. Terminaison retardée : conservation des objets et journalisation PID/chemin.
- Sondes natives : refus de lecture/écriture/création privées, HKCU du parent et droits dangereux sur le parent ; témoins positifs hors sandbox. Runtime lisible mais modifications/DAC/owner/création dans cwd refusées. Témoin AAP lisible en AppContainer normal et refusé en LPAC.
- Timeout, annulation, crash, OOM, trames falsifiées et saturation IPC exercés avec readiness. Encodage/décodage ZIP/7z/TAR/TAR.ZST et lecture GZIP/XZ/BZIP2/ISO vérifiés ; garanties de publication/rollback conservées.

### Reste à traiter pour R1

1. **Autres modes Windows** : image, thumbnail, office, PDF, office-text, cached-thumbnail, cached-office, icon-small/large, vidéo, navigation/listing et index recherche. Leurs besoins document/Shell/COM/polices/Media Foundation/GPU doivent être cadrés ; le lot archive-I/O ne les confine pas.
2. **Réseau** : zéro capacité imposée et témoins TCP IPv4/IPv6 disponibles hors sandbox/AppContainer normal, mais Winsock échoue à l'initialisation sous LPAC (10107). Les connexions TCP n'ont pas été exercées : aucun refus de permission TCP ne peut être conclu de cet échec.
3. **Exceptions ACL** : un accès explicitement accordé au SID ou à ALL RESTRICTED APPLICATION PACKAGES peut rester permis ; le témoin ARAP est effectivement lisible. Pas d'interdiction absolue indépendante des ACL, ni acceptation implicite de cette limite.
4. **Héritage hors launchers recensés** : ShellExecute et launchers tiers hors verrou non validés. La correction ne démontre pas une garantie universelle.

Limites connexes à ne pas confondre avec une clôture : requête token de classe 46 en erreur 87, donc aucune propriété déduite ; renommage natif refusé par partage, mais branche de rejet d'une substitution d'identité non exercée (R4) ; préparation avant compteur actif, I/O natives bloquées non interruptibles (R6) ; budgets globaux et résidus après crash ouverts (R3/R7).

Commits : `3f135b2` (codec minimal) et `2e69e05` (confinement LPAC et preuves). [Rapport détaillé et commandes reproductibles](security/AUDIT_GROUPE_1_RESULTATS.md) : `cargo test --all-targets` avec TEMP/TMP privé, 908 réussis/17 ignorés ; `cargo check --all-targets` et `cargo build --bins` réussis sans avertissement du projet ; clippy réussi avec avertissements préexistants. Résultats rapportés, non relancés pour cette synthèse.

Livraison : distribuer les deux exécutables côte à côte, construire avec `cargo build --bins` ; runtime absent/préparation refusée doit échouer sans repli UI. La checklist d'installation propre, codecs, annulation et sibling absent reste manuelle et non exécutée. **R1 reste ouvert**, sans décision utilisateur d'accepter ses limites comme clôture.

## R9 — analyse acquise, correction à entreprendre

Le même modèle de worker compromis s'applique. Les contrôles indépendants actuels bornent noms, ressources, offsets, tailles et transitions ; le parent retient les objets, applique la provenance, contrôle publication/récupération et compare l'empreinte de l'original. Cela protège le protocole et certains effets, pas la correspondance entre sources attendues et résultat produit.

Restent sans preuve hostile : fidélité des contenus extraits/encodés, exhaustivité des inventaires et fidélité des pixels/textes. Même si l'encodeur consomme toutes les sources, ses blocs de sortie bornés peuvent être arbitraires. Un hash fourni par le worker, le confinement LPAC ou un round-trip normal ne remplace pas une preuve indépendante.

Alternatives déjà identifiées, **sans correction implémentée ni choix autorisé** :

- Format canonique construit ou vérifié par le parent, en évaluant l'effet sur sa surface de confiance.
- Vérificateur indépendant isolé comparant inventaire et digests à un plan de confiance ; préciser indépendance réelle, sources des références et propriétés vérifiées.
- Référence externe fiable lorsque le scénario en dispose ; ce n'est pas une garantie générale pour toute archive.

La reprise doit choisir un contrat testable, définir octets/propriétés attendus et exhaustivité, puis implémenter et tester les falsifications couvertes. Une difficulté ou impossibilité doit être étayée, sans annoncer la correction de l'ensemble à partir de contrôles partiels. **R9 reste ouvert : seule l'analyse a été réalisée.** La documentation seule ne clôture pas le point ; report ou acceptation d'une limite exigent une décision explicite utilisateur.

R5 reste exclu de la prochaine passe selon la décision utilisateur, sans être corrigé : Linux n'a été ni compilé ni exécuté dans le groupe 1. Cette mise à jour documentaire n'engage aucune nouvelle implémentation.
