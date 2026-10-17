# Sécurité : protections et limites

Cette référence synthétise les protections livrées et les points encore ouverts. Elle ne constitue pas une nouvelle expertise ni une certification. Les preuves datées sont dans le [bilan du 13 octobre 2026](../reports/2026-10-13-security.md).

## Modèle de confiance Windows

Le parent est de confiance ; un worker peut être compromis par un codec. Noyau, lancement et runtime font partie de la base de confiance ; administrateur hostile et parent compromis sont hors contrat.

Decode/Encode d’archives, listing/index, images/SVG et texte Office utilisent des workers LPAC sans capacité, sans repli local permissif. Le parent garde les sources, les contrôles et les écritures. Le lancement suspendu attache les jobs avant reprise ; seuls les handles IPC et sources explicitement autorisés sont transmis.

Les exceptions ACL ont été retenues par l’utilisateur : un accès explicite au SID exact du worker ou à `ALL RESTRICTED APPLICATION PACKAGES` peut rester possible. Le confinement n’est pas indépendant des ACL.

PDF, vidéo et services natifs Shell/miniatures/icônes restent hors LPAC. Leurs jobs et délais ne retirent pas les permissions ordinaires. Les sessions réutilisables d’images peuvent conserver l’état d’un décodeur compromis entre demandes : [aperçus](../architecture/previews.md).

## Suivi R1–R9

| ID | Protection livrée | Limite conservée |
| --- | --- | --- |
| R1 — permissions | LPAC pour les codecs purs, runtime privé, héritage explicite, contrôles natifs | Services natifs restants hors LPAC ; réseau de production et héritage universel non prouvés. Les permissions COM/registre étudiées en tests ne sont pas activées. |
| R2 — suppressions | Reçus de descendants, effets partiels, restauration et reprise après conflit | Corbeille Windows conservée par choix utilisateur ; course par chemin, absence d’atomicité sur l’objet initial et fournisseurs non validés. |
| R3 — ressources | Job agrégé 2,5 Gio / 4 processus / CPU 70 %, admission commune, pixels retenus 256 Mio / 256 images, surveillance de pression | Aucun plafond instantané universel du parent, RSS/GPU global, COM externe ou disque physique. Le cache inconnu après crash n’est pas inclus automatiquement. |
| R4 — identité/provenance | Rejet d’identité nulle/objet étranger, handles retenus, contrôle BLAKE3 et `Zone.Identifier` | Matrice native locale C: NTFS fixe ; autres volumes, cloud/SMB, tous les ADS et snapshots immuables non prouvés. Un autre lien physique peut être renommé. |
| R5 — Linux | Codecs locaux conservés | Pas de confinement équivalent ; compilation/exécution Linux non validées. Le blocage Fontconfig/sysroot de l’environnement croisé ne prouve pas un défaut du projet. |
| R6 — annulation | `Platform::watch_broker_io`, tentative `CancelSynchronousIo`, scopes de préparation et courtier | Interruption coopérative : un pilote peut refuser. Aucun thread parent n’est tué, aucune garde n’est abandonnée pendant une I/O. |
| R7 — récupération | Cache Windows v2, propriétaire v4, journal authentifié DPAPI/MAC, reprise après crash | Cache v1/préparations v3 sans preuve conservés, hors quota du nouveau stockage. Clé perdue, objets étrangers/modifiés et preuves insuffisantes interdisent adoption/purge globale. |
| R8 — groupes d’archives | Préparation de toutes les archives, originaux retenus puis restauration du groupe sur échec | Pas de visibilité atomique ni transaction durable multi-fichiers après crash ; backups non restaurables conservés et signalés. |
| R9 — fidélité | Vérification indépendante du contenu/inventaire/métadonnées définies pour TAR non compressé | Autres formats, listing/index et pixels/textes sans référence indépendante générale. Fonctions conservées ; aucune acceptation de ce risque ni clôture déduite de l’absence de réponse utilisateur. |

## Où intervenir

- Confinement/lancement : `platform/windows_worker_sandbox.rs`, `windows_process_launch.rs` et `src/bin/`.
- Budgets/pression : `platform/windows_worker_budget.rs`, `windows_resource_pressure.rs`, `ui/preview_budget.rs`.
- Suppressions : `platform/windows_shell_delete*`, `systems/file_ops/delete.rs` et `execution.rs`.
- Archives/récupération : `systems/archive_io.rs`, `archive_encode.rs`, `archive_workspace/native_commit.rs`, `archive_open_cache/recovery.rs` et `utils/tar_fidelity.rs`.

Avant de reprendre un point, rechercher ses fonctions dans les sous-modules, choisir son ID et définir les preuves attendues. Une refactorisation ou un essai visuel ne clôture pas une limite de sécurité. Ne pas transformer une reprise ciblée en nouveau chantier général.

Conserver la [décision de dépendances](dependencies.md) et le [contrat de fermeture](../diagnostics/shutdown.md). Toute nouvelle validation doit préciser révision, cible et périmètre ; les anciennes bornes restent historiques.
