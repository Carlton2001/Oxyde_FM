# Oxyde — reliquat de sécurité pour la reprise

État du 10 octobre 2026, sur `master` après `30d77f7` (validation visuelle du code livré à `243140d`). Ce fichier est le point d'entrée pour les travaux restants ; [AUDIT_SECURITE.md](AUDIT_SECURITE.md) conserve les preuves et l'historique. Il synthétise le **bilan actuel**, sans transformer les anciennes mentions « encore à faire » en nouvelles tâches.

## Base acquise

- Isolation Windows de l'extraction complète/ciblée, du cache/export et des workspaces pour les huit familles d'archives ; création/reconstruction et mutations pour ZIP/7z/TAR/TAR.ZST. Aucun repli local Windows sur panne du worker.
- Courtiers bornés, sources et sorties retenues, contrôles de chemins et de provenance, comparaison identité/taille/date/BLAKE3 de l'original, publication exclusive, rollback et récupération des effets partiels.
- Navigation, recherche et aperçus Windows dans leurs workers distincts. Les plafonds individuels et les admissions ne constituent pas un budget global ni une restriction des permissions.
- Dernière borne : **852 tests réussis**, aucun échec, 17 ignorés préexistants ; check et build sans avertissement du projet. Retour utilisateur **« Tests visuels OK »** consigné dans [security/VERIFICATION_VISUELLE.md](security/VERIFICATION_VISUELLE.md).

Le chantier d'isolation des archives est clôturé dans son périmètre. Il n'est pas à recommencer pour traiter le reliquat. Ces résultats ne sont pas une certification générale de sécurité.

## Priorités proposées pour une prochaine passe

L'ordre ci-dessous est une proposition à cadrer, pas une nouvelle série de travaux engagée. Les limites sont constatées ; aucune nouvelle exploitation par corruption mémoire n'a été démontrée. Chaque point exige un périmètre précis avant de coder.

| ID | Point ouvert et statut | Critères de fin à préciser au cadrage |
| --- | --- | --- |
| R1 | **Permissions des workers.** Limite d'architecture : ils conservent les droits du compte. Les jobs limitent les ressources et la durée de vie, sans confiner les accès arbitraires aux fichiers/réseau d'un worker compromis. | Nommer les modes de worker concernés et leurs accès nécessaires ; définir les accès interdits ; choisir puis tester le mécanisme natif via `Platform`, sur accès permis/refusés et parcours ordinaires. Consigner les garanties obtenues et les exceptions. Un Job Object seul ne clôture pas ce point. |
| R2 | **Suppressions Shell : course par chemin et effets partiels.** Risque résiduel malgré retenue des objets et contrôle PreDelete. Les reçus concernent les racines, pas tous les descendants d'un dossier partiellement supprimé. | Séparer la garantie sur l'objet visé de la couverture des effets/historique. Délimiter recyclage, suppression et reprise concernés ; vérifier les courses et erreurs sur fixtures privées, sans supprimer de documents personnels. Si l'API retenue ne permet pas la garantie voulue, choisir une alternative ou accepter explicitement la limite ; une vérification supplémentaire par chemin ne suffit pas à annoncer l'atomicité. |
| R3 | **Budget global de ressources.** Protection contre la saturation encore absente : plafonds de jobs et quotas locaux ne couvrent pas l'ensemble du parent, des workers, du GPU, des handles et des serveurs COM externes. | Définir les ressources réellement comptabilisables, une enveloppe chiffrée et la politique d'admission/récupération ; tester la concurrence et le retour à un état utilisable. Distinguer mesures/enveloppes logiques et plafonds effectivement imposés. Lister ce qui échappe au contrôle, sans annoncer un plafond RSS/GPU exact non mesuré. |

Emplacements de départ :

- R1 : [windows_preview_worker.rs](src/platform/windows_preview_worker.rs), [windows_archive_worker.rs](src/platform/windows_archive_worker.rs), [windows_archive_io_worker.rs](src/platform/windows_archive_io_worker.rs), modes internes dans [main.rs](src/main.rs) ; contrat système dans [platform/mod.rs](src/platform/mod.rs).
- R2 : [windows_shell_delete_tracked.rs](src/platform/windows_shell_delete_tracked.rs), [windows_shell_delete.rs](src/platform/windows_shell_delete.rs), [windows_impl.rs](src/platform/windows_impl.rs), [file_ops.rs](src/systems/file_ops.rs). Preuves historiques : lots 52, 59, 60 et correction 66 de l'audit complet.
- R3 : superviseurs ci-dessus, admissions/caches des aperçus, [archive_open_cache.rs](src/systems/archive_open_cache.rs) et orchestration applicative. Le point couvre aussi les limites des quotas logiques de caches : espace physique/flux et écritures externes ne sont pas automatiquement bornés.

## Autres limites : décision de portée avant implémentation

Ces points restent ouverts, mais n'imposent pas automatiquement un développement supplémentaire. Ils peuvent être traités, reportés ou conservés comme limites explicites. Ils ne doivent pas être déclarés corrigés par les tests visuels.

| ID | Limite actuelle | Décision ou preuve nécessaire pour clôturer le point |
| --- | --- | --- |
| R4 | **Volumes, fournisseurs et identités.** Validation non exhaustive ; identifiants/date non cryptographiques, alias/liens et modifications du contenu/ADS du même objet ne fournissent pas toutes les garanties d'un snapshot. | Fixer une matrice de cibles prises en charge ; tester retenue, remplacement, reparse, partage et identité sur ces cibles. Distinguer garanti, refusé et non validé. Ne pas promettre tous les fournisseurs. Départ : `source_selection`, `path_parents`, implémentations natives et preuves d'historique. |
| R5 | **Linux.** Les codecs restent locaux. La compilation croisée tentée s'arrête dans Fontconfig faute de sysroot/configuration `pkg-config`, avant validation d'Oxyde. | Décider si l'isolation Linux appartient à une prochaine passe. La validation de compilation/exécution sur Linux et l'ajout d'une isolation native sont deux objectifs distincts, chacun à délimiter. [linux_impl.rs](src/platform/linux_impl.rs), [security/ISOLATION_ARCHIVES.md](security/ISOLATION_ARCHIVES.md). |
| R6 | **Appels bloquants dans le parent.** Le délai du superviseur ne garantit pas l'interruption d'un appel de pilote/fournisseur bloqué dans le courtier. | Identifier les appels et cibles concernés, puis délimiter un mécanisme d'interruption ou de déplacement compatible avec les preuves d'objet/publication. À défaut, conserver cette limite de délai explicitement. Ne pas assimiler délai du worker et annulation de toute I/O native. |
| R7 | **Cache abandonné après crash.** Les préparations version 3 sans preuve persistante restent conservées et comptées dans l'admission. | Si leur récupération automatique est souhaitée, établir une preuve ou une politique de récupération justifiée, testée sur crash/remplacement/enfant étranger. Ne jamais résoudre le problème par purge globale de TEMP ou suppression d'inconnus par nom. [archive_open_cache.rs](src/systems/archive_open_cache.rs). |
| R8 | **Transferts entre archives.** Pas d'atomicité globale : un résultat peut être publié avant l'échec d'une autre archive. Les emplacements/effets conservés sont signalés. | Décider si une transaction multi-archives est nécessaire ou si ce contrat est conservé. Une amélioration d'atomicité exige son propre périmètre et ses essais de récupération après panne/crash. [archive_workspace.rs](src/systems/archive_workspace.rs), [file_ops.rs](src/systems/file_ops.rs). |
| R9 | **Fidélité des sorties d'un worker compromis.** Le protocole valide ressources, noms, budgets et transitions ; il ne prouve pas que tous les octets encodés représentent les sources attendues. | Définir le modèle de menace et la garantie supplémentaire souhaitée. Une relecture par le même codec ne constitue pas à elle seule une preuve indépendante. Sinon conserver explicitement la limite, sans déduire une fidélité hostile des round-trips normaux. |

## Décisions déjà prises et sujets distincts

- **Conserver** `paste 1.0.15`, `rustybuzz 0.20.1`, `ttf-parser 0.25.1`, selon la décision utilisateur. Leur remplacement n'est pas une correction en attente autorisée. Avis et conditions de réexamen : [security/DEPENDENCY_MAINTENANCE.md](security/DEPENDENCY_MAINTENANCE.md). La revue RustSec du 9 octobre est historique ; ne pas la présenter comme une vérification actualisée.
- **Refactorisation des gros fichiers :** amélioration de maintenance distincte. Si retenue avant un point ci-dessus, nommer les fichiers et responsabilités à extraire, garder le comportement et séparer ses commits des changements de sécurité. Pas de refonte générale implicite ni de nouveau défaut de sécurité déduit de la taille d'un fichier.
- **Fermeture PDF/Intel :** diagnostic et résolution dans [SHUTDOWN_DIAGNOSTICS.md](SHUTDOWN_DIAGNOSTICS.md). Consulter avant de modifier la sortie finale ; une ancienne mention de blocage n'est pas une régression actuelle confirmée.

## Comment reprendre sans recréer une liste sans fin

1. Lire [AGENTS.md](AGENTS.md), ce reliquat et le bilan actuel en tête de l'audit ; consulter l'historique seulement pour le point choisi. Lire [UI_ARCHITECTURE.md](UI_ARCHITECTURE.md) si l'interface est touchée.
2. Vérifier la branche, le commit et les changements locaux. La borne ci-dessus est celle de la dernière validation, pas une instruction de reset ; préserver les travaux ultérieurs et les fichiers utilisateur.
3. Choisir **un point par son ID**, définir périmètre, critères vérifiables et limites conservées avant de coder. La reprise de « tout le reliquat » exige d'abord un ordre et des décisions de portée.
4. Garder des commits distincts, exécuter les vérifications pertinentes et préparer les tests visuels utilisateur si le comportement visible change. Aucun Computer Use, suivi automatique ou push ; conserver les trois dépendances selon la décision actuelle.
5. Mettre à jour ce fichier quand le point est corrigé, reporté ou accepté, avec preuve/décision et commit. Un défaut nouveau confirmé est consigné séparément ; il ne doit pas agrandir silencieusement le chantier en cours.

Exemple de demande de reprise : « Reprends sur master. Lis AGENTS.md et AUDIT_SECURITE_RELIQUAT.md. Traite uniquement R1 ; précise les workers concernés, les critères de fin et les limites conservées avant de coder. Consulte AUDIT_SECURITE.md pour les preuves utiles. »

Ce fichier n'engage aucune implémentation. Création documentaire après la validation visuelle ; tests automatisés non relancés.

## Preuve partielle R1 — groupe 1, 12 octobre 2026

Archive-I/O Windows Decode/Encode : LPAC sans capacité ni repli, job avant reprise, sélection explicite des handles IPC ; course d'héritage corrigée pour les launchers Command Oxyde recensés. Runtime privé RX, provenance/identités retenues et nettoyage ciblé. Refus privés/HKCU/droits dangereux et témoin différentiel AAP démontrés. Validation : 908 tests réussis, aucun échec, 17 ignorés préexistants ; check/build sans avertissement, clippy avec avertissements préexistants. Commits et preuves : [rapport groupe 1](security/AUDIT_GROUPE_1_RESULTATS.md).

**R1 reste ouvert** : autres workers, TCP non exercé, ACL explicitement accordées au sandbox, Shell/tiers hors verrou. Ces limites n'ont pas été acceptées comme clôture par l'utilisateur. Renommage refusé ; substitution d'identité non exercée. R3/R4/R6 restent ouverts. **R9 reste ouvert**, contrat distinct à décider, aucune preuve de fidélité hostile ; **R5 inchangé**, Linux non exécuté. Aucun nouveau chantier autorisé par cet addendum.
