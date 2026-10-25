# Audit du code Oxyde — 25 octobre 2026

La compilation et les **1 405 tests Windows exécutés passent**, mais l’audit relève **10 constats : 1 P1, 8 P2 et 1 P3**. Le problème prioritaire est la recherche de contenu Office : elle décompresse les documents dans le processus principal sans borne de taille. L’audit RustSec actualisé échoue sur `ctutils 0.4.2`, présent dans le graphe Windows actif. Aucun correctif applicatif n’a été réalisé pendant cet audit.

## Suivi des corrections après audit

Les constats et chiffres ci-dessous décrivent la révision auditée. Le suivi suivant décrit les corrections ultérieures, sans clôturer les ID non mentionnés.

| ID | Correction | Validation ciblée |
| --- | --- | --- |
| A05 | Lancement du setup via `Platform` et le verrou Windows commun ; flux standard explicites | Check toutes cibles sans avertissement ; 2 tests natifs d’héritage réussis, dont lancement concurrent du setup fixture |
| A02 | Mise à jour ciblée de `ctutils` vers 0.4.4 ; notices et empreintes régénérées | Check toutes cibles sans avertissement ; audit RustSec actualisé réussi, 0 vulnérabilité publiée ; avis de maintenance/Tokio conservés |

## Révision, méthode et portée

- Révision : `c0dc529356dc30db82fffbcd9f7ec184b4e34183`, paquet `oxyde 2.0.1` ; arbre Git initial propre.
- Environnement : Windows x64 MSVC, `rustc 1.98.1 (48a229cea 2026-09-01)`, Cargo `1.98.1` ; profil de développement du dépôt.
- Inventaire de `src/` : **670 fichiers Rust, 142 573 lignes**, tests et commentaires inclus. Ce total ne mesure pas les lignes de production.
- Cartographie de l’ensemble de `src/`, des tests, des manifestes, des scripts et des workflows ; relecture ciblée des chemins critiques, recherche transversale des I/O, lancements, allocations, limites, annulations et points natifs ; confrontation aux contrats et exécution des tests.
- Le périmètre comprend opérations/historique, archives/workers, recherche/doublons, navigation/préférences, aperçus/Viewer, abstraction système, dépendances, distribution et patch GPUI local.
- Cette couverture transversale ne signifie pas une relecture ligne par ligne des 142 573 lignes, ni une expertise exhaustive des dépendances tierces. `reference_zed/` est une référence externe ; la revue du code vendored se concentre sur le patch DirectX documenté.
- Pas de modification des sources de production, de mise à jour de dépendances, de commit, de push, d’installation de mise à jour Oxyde ou de publication. Les sondes temporaires ont été retirées de `tests/` après exécution.

P1 = correction prioritaire, avec impact important sur la disponibilité. P2 = défaut fonctionnel, de robustesse, de sécurité ou de livraison à planifier. P3 = défaut de présentation à impact limité. La priorité n’est pas un score CVSS.

## Résultats des vérifications

| Vérification | Résultat observé |
| --- | --- |
| `cargo check --offline --locked --all-targets` | Réussite, aucun avertissement du projet |
| `scripts/test-windows.ps1 -Offline`, hors sandbox | Réussite ; construction explicite des binaires et tests en série |
| Bibliothèque | 900 réussis, 34 ignorés |
| Application/UI | 441 réussis, 16 ignorés |
| Intégrations | 64 réussis : pipeline archives 31, worker I/O 12, listing 4, résolution de chemins 3, aperçus 10, mémoire recherche 1, collisions de transfert 3 |
| Total suite du dépôt | **1 405 réussis, 0 échec, 50 ignorés** ; le test enfant d’héritage déjà inclus dans la bibliothèque n’est pas compté deux fois |
| Durées script, compilation incrémentale chaude | Check 1,0 s ; binaires 0,8 s ; compilation/exécution des tests 187,0 s |
| Sondes supplémentaires de cet audit | 5 réussies ; elles affirment la présence des comportements défectueux, pas le comportement souhaité |
| Helper local atlas DirectX, compilé par `rustc --test` | 2 réussis ; validation des dimensions/buffers, sans preuve GPU réelle |
| `scripts/tests/setup-distribution.tests.ps1` | Réussite des 5 groupes de contrôles simulés ; aucun téléchargement de toolchain |
| `cargo clippy --offline --locked --all-targets` | Réussite avec avertissements ; 39 pour la bibliothèque de production et 16 pour le binaire principal, plus tests/outils et doublons |
| Même commande Clippy avec `-- -D warnings` | Échec ; dette existante, principalement complexité de types, simplifications et organisation des tests |
| `cargo fmt --all -- --check` | Échec ; différences proposées dans 394 fichiers distincts, sans appliquer le formatage |
| `scripts/audit-security.ps1`, hors sandbox | **Échec de sécurité attendu, code 1** : 1 vulnérabilité publiée ; rapport JSON produit |

La première campagne native en sandbox a rencontré des refus Windows `PermissionDenied` et a été interrompue. Un test de copie identique passe hors sandbox, puis la campagne complète ci-dessus passe. Ces refus ne sont donc pas présentés comme des régressions du produit. Le premier téléchargement RustSec était également empêché par la sandbox ; la relance hors sandbox a effectivement actualisé la base.

## Constats classés

| ID | Priorité | Constat | Niveau de preuve |
| --- | --- | --- | --- |
| A01 | P1 | Recherche Office : décompression non bornée dans le parent | Reproduction mémoire + lecture |
| A02 | P2 | `ctutils 0.4.2` vulnérable dans le graphe actif | RustSec actualisé + `cargo tree` |
| A03 | P2 | Recherche Office : résultats manquants sans avertissement | Reproduction de trois variantes |
| A04 | P2 | Taille de dossier incomplète renvoyée comme un succès | Reproduction native avec contrôle positif |
| A05 | P2 | Lancement de l’updater hors du verrou commun d’héritage | Reproduction d’un handle transmis au fils |
| A06 | P2 | Résultats de doublons : nombre de lignes rendues non borné | Lecture du parcours de rendu |
| A07 | P2 | Nettoyage d’historique avec I/O synchrones dans le hook UI de fermeture | Chaîne d’appels vérifiée |
| A08 | P2 | Linux : chemins de montage échappés utilisés littéralement | Lecture + contrat Linux ; exécution Linux absente |
| A09 | P2 | Release indépendante des tests complets et de l’audit de sécurité | Workflows locaux vérifiés |
| A10 | P3 | Une taille unique trompeuse pour les doublons par nom | Reproduction + rendu |

### A01 — Décompression Office sans plafond dans le processus principal

**Localisation :** [recherche](../../src/commands/search.rs), lignes 479–545 et 615–638 ; contraste avec [extraction Office du Viewer](../../src/utils/office_document.rs), lignes 8–10 et 55–64.

`advanced_search` atteint directement `office_file_contains_content`, qui ouvre le ZIP et lit une entrée XML entière avec `read_to_string`. Le XML est ensuite recopié dans `stripped`, avec agrandissements de buffers. Il n’existe ici ni plafond de XML décompressé, ni worker LPAC, ni job mémoire. Les contrôles d’annulation à chaque lecture sont utiles, mais ne bornent pas une recherche laissée active. Le parcours du Viewer, lui, utilise une extraction bornée et son worker.

**Reproduction :** un DOCX ZIP/Deflate contenant `word/document.xml`, avec 32 Mio de `x` puis `needle` dans un paragraphe. Fichier obtenu : **32 787 octets**. Recherche publique `advanced_search(... content="needle" ...)` : résultat trouvé, mais **117 520 582 octets supplémentaires au pic d’allocations Rust vivantes**, environ **112,1 Mio**, mesurés dans le processus de test appelant. La sonde utilise un allocateur compteur ; ce chiffre n’est ni un RSS ni une mesure GPU.

**Impact :** un document très compressible rencontré pendant une recherche peut épuiser la mémoire du parent et arrêter Oxyde. Aucune exécution arbitraire n’a été démontrée. L’affirmation générale d’isolation du texte Office doit distinguer ce parcours de recherche.

**Correction proposée :** faire passer la recherche Office par une interface `Platform` vers un worker borné, avec plafond cumulé de décompression, délai et annulation. Pour préserver la recherche de grands documents, envisager une extraction en flux ; si un plafond est atteint, signaler une recherche incomplète. Ajouter un test de forte compression qui vérifie la borne du parent et l’absence de repli local Windows.

### A02 — Dépendance active concernée par RUSTSEC-2026-0385

**Localisation :** [Cargo.lock](../../Cargo.lock), entrée `ctutils` ligne 1456 ; [audit de sécurité](../../scripts/audit-security.ps1).

Le script officiel du dépôt signale `ctutils 0.4.2`. L’avis publié le 23 octobre décrit des générations de masques non constantes en temps ; une autre partie concerne les comparaisons `u128` sur cibles 32 bits. Correction publiée : **`>=0.4.4`**. [Avis RustSec officiel](https://rustsec.org/advisories/RUSTSEC-2026-0385.html).

Le paquet n’est pas seulement un reliquat du verrou : `cargo tree --target x86_64-pc-windows-msvc -i ctutils` le relie à `digest 0.11.3`, puis notamment `hmac/pbkdf2/zip 9.0.4`, `sha2/sevenz-rust2 0.24.0` et `rust-embed-utils`.

**Impact établi :** l’audit de sécurité Windows actuel échoue. **Limite :** aucun canal auxiliaire ni secret récupérable n’a été démontré dans Oxyde ; la présence du paquet actif ne prouve pas que le code affecté est exploitable dans les usages de l’application.

**Correction proposée :** actualiser `ctutils` vers une version corrigée compatible avec ses contraintes transitives, sans modifier arbitrairement les autres dépendances. Relancer l’audit et la suite native ; vérifier le verrou et les notices de licences. Aucune dépendance n’a été modifiée ici.

### A03 — Recherche Office silencieusement incomplète

**Localisation :** [recherche](../../src/commands/search.rs), lignes 500–513 et 548–591.

Trois défauts sont reproduits par le point d’entrée public :

1. PowerPoint : seules `slide1.xml`, `slide2.xml` et `slide3.xml` sont examinées. `needle` dans `slide4.xml` ne renvoie aucun résultat ; le contrôle dans `slide1.xml` est trouvé.
2. Excel : seule `xl/sharedStrings.xml` est examinée. Une cellule `t="inlineStr"` contenant `needle` dans `xl/worksheets/sheet1.xml` est ignorée ; le contrôle dans les chaînes partagées est trouvé.
3. Word : `<w:t>A&amp;B</w:t>` ne correspond pas à `A&B`, alors qu’une recherche de la représentation XML `A&amp;B` correspond. Le retrait artisanal des balises ne décode pas les entités.

**Impact :** la recherche peut annoncer une fin normale et aucun résultat alors que le texte existe. Aucun événement d’avertissement n’identifie ces omissions, contrairement aux limitations de recherche dans les archives.

**Correction proposée :** mutualiser une extraction de texte cohérente avec le Viewer, dans les limites de A01 ; couvrir toutes les diapositives/feuilles pertinentes et les entités. Rendre explicite toute troncature. Conserver les trois fixtures comme tests de régression, avec assertions de résultats positifs.

### A04 — Les erreurs de parcours des tailles deviennent un total apparemment complet

**Localisation :** [calcul des tailles](../../src/systems/folder_sizes.rs), lignes 151–195 ; [API publique](../../src/commands/io.rs), lignes 425–443.

Les erreurs des enfants sont exclues du comptage par `children.iter().flatten()`, une erreur de métadonnées est ignorée, puis `for _ in walker` ne traite aucune erreur remontée. Le résultat ne contient aucun indicateur d’incomplétude.

**Reproduction Windows :** créer `root/blocked/data` de 1 024 octets, garder `blocked` ouvert avec un partage nul et vérifier que `read_dir(blocked)` échoue. `calculate_folder_size(root)` renvoie pourtant **`Ok(size=0, files=0, folders=1)`**. Après libération du handle, le même appel renvoie 1 024 octets.

**Impact :** affichage d’une taille et de compteurs sous-estimés sans signaler de sous-arbre inaccessible. Ce cas est distinct des exclusions système intentionnelles et de l’annulation, déjà testée.

**Correction proposée :** propager les erreurs de parcours ou enrichir `FolderSizeResult` avec un statut incomplet et un compteur d’erreurs ; traduire ce statut dans les cellules et les Propriétés. Ajouter le contrôle positif de la sonde au test de régression.

### A05 — Le lancement de la mise à jour échappe au mécanisme d’héritage commun

**Localisation :** [updater](../../src/systems/updates.rs), lignes 51–55 ; [verrou des lancements Windows](../../src/platform/windows_process_launch.rs), lignes 1–60.

`updates::launch` utilise directement `std::process::Command::spawn`, alors que le code Windows dispose d’un verrou commun protégeant les courts intervalles où les handles des workers sont héritables. Cet appel contourne aussi la règle du dépôt imposant le passage des API système par `Platform`.

**Reproduction :** compiler un fils inoffensif dans une fixture privée, ouvrir un fichier et rendre son handle héritable, puis appeler **le vrai `updates::launch`** sur ce fils. Celui-ci peut lire le contenu du fichier par le handle hérité. Aucun installateur réel n’a été lancé. Les arguments `/UPDATE` et `/WAITPID` sont simplement ignorés par cette fixture.

**Impact :** un lancement concurrent du setup peut recevoir des handles sans rapport avec lui et prolonger leur durée de vie, notamment des extrémités IPC. Cela ne démontre pas un compromis du setup signé ni une fuite à un tiers hostile ; c’est une rupture du mécanisme commun de contrôle d’héritage.

**Correction proposée :** introduire un lancement de mise à jour dans `Platform`, implémenté sous Windows avec le helper de lancement synchronisé et des flux explicitement choisis. Tester la concurrence avec `InheritedHandles`, l’identité des objets et la fermeture des pipes.

### A06 — Un seul groupe de doublons peut produire une très grande scène GPUI

**Localisation :** [rendu des doublons](../../src/ui/app/duplicates/results.rs), lignes 78 et 165–282.

`self.limit` borne le nombre de **groupes**, mais chaque groupe déplié rend tous ses fichiers avec une boucle qui construit les lignes, icônes et closures. Le défilement utilise un `Div` ordinaire ; les lignes ne sont pas virtualisées. Les groupes sont dépliés après une recherche.

**Déclencheur :** rechercher par taille ou contenu dans un ensemble comprenant des dizaines de milliers de fichiers vides. Ils peuvent former un unique groupe, qui échappe entièrement à la limite initiale de 100 groupes.

**Impact :** travail proportionnel à tout le groupe sur le thread UI à chaque rendu, avec risque de gel et de consommation mémoire élevée. Aucun temps de frame n’a été mesuré pendant cet audit ; le défaut de bornage est établi par le code.

**Correction proposée :** préparer une séquence de lignes avec en-têtes et fichiers, puis virtualiser le viewport avec les primitives GPUI existantes. À défaut, paginer aussi les membres. Vérifier qu’un groupe de 100 000 fichiers n’instancie que les lignes visibles et conserve sélection, focus et défilement.

### A07 — Le hook de fermeture effectue des I/O sur le thread UI

**Localisation :** [hook applicatif](../../src/ui/app/mod.rs), lignes 485–501 ; [historique](../../src/models/history.rs), lignes 4–43 et 267–282 ; [nettoyage vérifié](../../src/systems/file_copy.rs), fonction `remove_verified_output_checked`.

Le callback synchrone `on_app_quit` appelle `this.history.clear()` **avant** de renvoyer son futur. `clear` parcourt les transactions expirées et appelle immédiatement le nettoyage des backups de métadonnées. Ce nettoyage ouvre des parents/fichiers, vérifie identité et hash, puis supprime les objets.

**Impact :** avec des backups sur un support lent ou en difficulté, le thread UI peut rester bloqué avant même l’attente asynchrone des préférences. Le délai de deux secondes de `shutdown_runtime` intervient plus tard et ne borne pas cette phase. La chaîne d’appels est vérifiée ; aucun blocage matériel n’a été provoqué.

**Correction proposée :** extraire les éléments à nettoyer sans I/O dans le callback, exécuter le nettoyage en arrière-plan et l’attendre selon le contrat de fermeture. Préserver les contrôles d’identité et la conservation des données non vérifiables ; ne pas masquer le problème en sautant le nettoyage ou en avançant `TerminateProcess`.

### A08 — Linux : les échappements des points de montage ne sont pas décodés

**Localisation :** [plateforme Linux](../../src/platform/linux_impl.rs), lignes 579–633.

Le code découpe `/proc/mounts` puis utilise directement le champ `mount` comme chemin. Les champs de montage représentent notamment les espaces et tabulations par des échappements octaux ; leur décodage est décrit par [Linux man-pages, getmntent(3)](https://man7.org/linux/man-pages/man3/getmntent.3.html).

**Déclencheur :** une entrée avec `/media/Mon\040Disque` doit désigner `/media/Mon Disque`. Le code conserve le premier texte, avec un antislash et trois chiffres. Les libellés, la navigation et la recherche du périphérique à éjecter héritent de ce chemin erroné.

**Correction proposée :** parser et déséchapper les champs au niveau plateforme avant leur utilisation, en conservant correctement espaces, tabulations et antislashs. Tester un parseur pur avec des lignes de montage, puis valider sur une machine Linux. **Preuve actuelle : analyse statique ; aucune exécution Linux.**

### A09 — La release peut être produite sans tests complets ni audit de sécurité réussis

**Localisation :** [release](../../.github/workflows/release.yml), [tests](../../.github/workflows/tests.yml), [sécurité](../../.github/workflows/security.yml).

Le workflow `Release` part d’un tag `v*`, vérifie sa version, teste uniquement `app_paths::tests`, construit, signe et crée une release brouillon. Il n’appelle pas la suite native complète ni l’audit et n’a pas de dépendance explicite vers leur succès sur le SHA du tag. Les autres workflows sont indépendants ; les déclenchements `push` concernés ciblent `main`.

**Impact :** la protection n’est pas garantie par le code des workflows. Le verrou actuellement refusé par RustSec en A02 reste admissible pour ce parcours de fabrication. Une règle GitHub externe pourrait compléter la protection ; les réglages distants du dépôt n’ont pas été consultés et leur absence n’est pas affirmée. Le brouillon impose encore une publication manuelle.

**Correction proposée :** rendre les validations nécessaires réutilisables et les exécuter sur le SHA du tag, avec un job de release dépendant de leur réussite. Ajouter au minimum l’audit avant signature et une politique explicite de tests natifs. Tester qu’un audit en échec empêche la génération de la release signée.

### A10 — La taille affichée d’un groupe par nom correspond à un membre arbitraire

**Localisation :** [moteur de doublons](../../src/commands/duplicates.rs), lignes 249–260 ; [taille du groupe](../../src/ui/app/duplicates/results.rs), lignes 151–160.

Quand seul le critère « nom » est actif, des fichiers de tailles différentes sont légitimement regroupés. `DuplicateGroup.size` prend néanmoins `files[0].size`, avant le tri final, et le rendu présente cette valeur unique sans préciser sa signification.

**Reproduction :** deux `same.txt` de 1 et 100 octets dans deux sous-dossiers donnent un groupe avec `size=1` dans la sonde et des tailles réelles `[1, 100]`. L’ordre de parcours peut changer le membre pris comme référence.

**Correction proposée :** afficher une plage ou les tailles individuelles lorsque la taille n’est pas un critère ; réserver une taille commune aux groupes qui garantissent son égalité. Ne pas laisser entendre que le nom seul prouve un contenu identique.

## Dépendances : résultat actuel et limites d’interprétation

Base RustSec : **1 352 avis**, commit `21d909e4c8bd4e14c7aea58bf980c881a0053ab1`, dernière mise à jour enregistrée `2026-10-24T10:30:13+02:00`. Le script a effectué son actualisation réseau le 25 octobre et produit un JSON valide. Le résultat du 9 octobre dans la documentation demeure historique.

| Catégorie | Paquets signalés | Interprétation pour cette revue |
| --- | --- | --- |
| Vulnérabilité | `ctutils 0.4.2`, RUSTSEC-2026-0385 | Actif Windows ; A02 |
| Maintenance, actifs Windows | `bincode 1.3.3`, `paste 1.0.15`, `rustybuzz 0.20.1`, `ttf-parser 0.25.1`, `yaml-rust 0.4.5` | Avis à suivre ; ils ne constituent pas à eux seuls des vulnérabilités exploitables |
| Maintenance, absents du graphe Windows examiné | `zbus_names 4.3.4`, `zvariant 5.15.0`, `zvariant_derive 5.15.0` | Présence dans le verrou/rapport ; ne pas les attribuer au binaire Windows actif |
| Unsoundness | `tokio 1.53.1`, RUSTSEC-2026-0386 | Avis portant sur les constructeurs Unix `AsyncFd` ; aucune référence `AsyncFd` dans `src/`, pas de feature Tokio `net` dans le graphe Windows examiné |

L’avis Tokio donne notamment `>=1.53.3` comme plage corrigée. Il justifie une réévaluation de version, sans démontrer ici une exposition Windows ni un use-after-free dans Oxyde. [Avis RustSec officiel](https://rustsec.org/advisories/RUSTSEC-2026-0386.html).

Conserver la [décision utilisateur sur paste/rustybuzz/ttf-parser](../security/dependencies.md) : ne pas les remplacer simplement pour faire disparaître un avis de maintenance. Le graphe Linux et les chemins transitifs effectivement exécutés nécessitent leur propre analyse.

## Couverture, éléments solides et dette de maintenance

| Domaine | Éléments examinés et preuves | Réserve |
| --- | --- | --- |
| Copie/déplacement/Undo/Redo | Sélection retenue, destination liée, staging, publication, identité/hash et reçus ; suite native et intégrations de collisions réussies | Matrice locale Windows ; pas de nouvelle preuve SMB/cloud/autres volumes |
| Suppression/corbeille | Reçus partiels, annulation, restauration, suivi des descendants ; tests natifs réussis | Courses par chemin et fournisseurs restent dans les limites R2 existantes |
| Archives | Validation des noms, état du protocole, budgets, handles, publication/rollback groupés et récupération ; 31 tests du pipeline de production réussis | Fidélité indépendante générale non établie au-delà du périmètre TAR documenté |
| Workers/confinement | Lancement LPAC, handles explicites, jobs/admission, transport borné et annulation ; tests natifs réussis | A01 est un parcours distinct ; sessions PDF/Shell/vidéo hors LPAC déjà documentées |
| Aperçus/Viewer | Politiques de décodage, pixels retenus, leases, admissions et propriété des tâches ; tests automatisés réussis | Pas de mesure d’une nouvelle session GPU réelle, ni de validation de tous les pilotes |
| Navigation/recherche | Résultats préparés, propriétaires/révisions, flux bornés, filtre contenu ; sondes Office et tailles | A03/A04 ; pas de mesure globale sur des millions de fichiers |
| Préférences/session | Écriture temporaire synchronisée puis publication ; écritures ordonnées, conservation d’une config illisible ; tests réussis | Pas de test de coupure électrique ou de deux instances concurrentes |
| Interface/langues | Conventions, catalogues, tests i18n/géométrie et principales files asynchrones | A06/A07 ; aucune validation manuelle générale des cinq langues, DPI, thèmes ou accessibilité |
| Distribution | Scripts, sommes de contrôle, notices, actions épinglées, test simulé de toolchain | A09 ; pas de reconstruction release, installation/désinstallation ou signature réelle pendant l’audit |
| Patch GPUI local | Tracé de provenance, initialisation de texture et helper de taille ; 2 tests supplémentaires réussis | Les mesures Intel antérieures restent historiques |

La structure distingue utilement état métier, composants et primitives, et les protections sur l’identité des fichiers sont largement testées. La règle `cargo check` sans avertissement est satisfaite. En revanche, Clippy strict et rustfmt ne forment pas aujourd’hui une base verte ; établir une politique progressive avant d’ajouter ces contrôles à la CI. Ne pas lancer une réécriture de 394 fichiers pour corriger les défauts fonctionnels de cet audit.

Les tailles brutes ne doivent pas déclencher des découpages automatiques : par exemple, `archive_seven_open.rs` contient un gros bloc de tests après environ 318 lignes, et `archive_workspace.rs` après environ 600 lignes. Le catalogue déclaratif `i18n.rs` et les fichiers upstream vendored demandent aussi une lecture spécifique. La façade `platform/windows_impl.rs` et certains modules UI restent à examiner avant une extension importante ; ce point est une dette de maintenabilité, pas un défaut fonctionnel supplémentaire démontré.

## Ordre de traitement proposé

1. Corriger A01 en conservant une recherche Office utile et en ajoutant ses tests de ressources ; traiter A03 dans le même périmètre d’extraction.
2. Actualiser la dépendance de A02 et ajouter la barrière de livraison A09.
3. Corriger A04 et A05 avec les contrôles positifs des sondes natives.
4. Virtualiser les résultats A06, déplacer les I/O de fermeture A07, puis traiter A08 sur Linux et A10 dans la présentation des doublons.

Les [limites R1–R9](../security/status.md) restent ouvertes selon leur statut documenté : services natifs hors LPAC, ressources globales du parent/GPU non plafonnées universellement, récupération conservatrice, absence de transaction durable multi-archives et garanties Linux différentes. Les tests verts de cet audit ne clôturent pas ces limites.

Si une correction touche les workers ou leur code partagé, **fermer Oxyde**, exécuter `cargo build --offline --bins`, puis `cargo run --offline`. `cargo run` seul ne reconstruit pas les auxiliaires.

## Preuves locales et reproduction

Les journaux et sources des sondes sont conservés dans [`target/code-audit-2026-10-25/`](../../target/code-audit-2026-10-25/), **non versionné et supprimable avec `target/`** :

- `native-tests-unsandboxed.log` : campagne de référence complète ; `native-tests.log` : campagne sandbox interrompue.
- `probes-complete.log`, `audit_20261025.rs` : cinq sondes via les API publiques ; la mesure mémoire inclut les allocations Rust du parent et ses threads de recherche. Les fixtures Office sont des ZIP minimaux contenant les parties XML pertinentes ; leur ouverture dans Microsoft Office n’a pas été validée.
- `dependencies-unsandboxed.log`, `rustsec-windows.json`, `windows-tree.txt` : avis actualisés et graphe Windows.
- `clippy-default.log`, `clippy.log`, `format.log` : diagnostics sans correction automatique.
- `atlas-tests.log`, `setup-distribution.log` : contrôles supplémentaires.

Pour rejouer les sondes dans ce checkout, copier temporairement `audit_20261025.rs` depuis ce dossier vers `tests/`, puis lancer `cargo test --offline --locked --test audit_20261025 -- --nocapture --test-threads=1`. Retirer ensuite ce seul fichier de `tests/`. Les assertions observent volontairement les défauts actuels ; elles devront être inversées et adaptées pour devenir des tests de régression après correction. Le lanceur de la sonde utilise un petit exécutable fixture, jamais un setup réel.

Ce rapport conserve les déclencheurs, résultats et localisations nécessaires à la compréhension même après suppression des journaux. Il n’atteste ni d’un fuzzing, ni d’un pentest, ni d’une analyse complète de chaque bloc `unsafe`, ni de la conformité des réglages GitHub distants.
