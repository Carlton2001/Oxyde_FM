# Audit groupe 1 — R1 et R9

Date : 12 octobre 2026. Base : `master`, `cac46682db04530009761a115d3841917c1c7435`.
Branche de travail : `codex/security-r1-archive-lpac`.

## Décision et périmètre

Le contrat et le premier lot ont été validés par l'utilisateur (« go ») après exploration par `security_explorer` et revue indépendante par `security_reviewer`. Le lot porte uniquement sur R1, worker Windows archive-I/O v1 (`Decode` complet/ciblé et `Encode`). R9 est cadré séparément ; aucune limite de fidélité n'est implicitement acceptée. R5/Linux est exclu. Aucun changement UI, fermeture, dépendances conservées, Computer Use, automation ou push.

Le parent est de confiance ; le worker peut exécuter volontairement du code arbitraire après compromission d'un codec. Le noyau Windows, les composants de lancement et le runtime sélectionné restent dans la base de confiance. Un administrateur hostile ou une compromission du parent ne sont pas couverts.

Contrat validé : LPAC sans repli moins restrictif ; job avant toute exécution enfant ; héritage explicite des seuls handles IPC nécessaires ; environnement minimal et répertoire courant explicite. Les accès directs aux documents privés, sources/destinations, registre privé du parent, réseau et accès dangereux au parent doivent être refusés. Les pipes, ressources système/runtime nécessaires et stockage propre du sandbox sont des exceptions explicites. Un éventuel runtime privé est en lecture/exécution pour le SID exact, sans ACL globale, avec provenance et objets retenus. Si la viabilité nécessite un élargissement du contrat, arrêter et présenter le blocage.

## Matrice des modes et accès

| Modes Windows | Besoins actuels | État du lot |
| --- | --- | --- |
| image, thumbnail, office, pdf, office-text | Document par chemin ; runtime, polices ; Shell/COM selon mode | R1 reste ouvert |
| cached-thumbnail, cached-office, icon-small/large | Document/Shell, caches et publication native | R1 reste ouvert |
| video | Document par chemin, Media Foundation, codecs/GPU | R1 reste ouvert |
| Navigation/listing et index recherche | Archive par chemin, codecs | R1 reste ouvert |
| Archive-I/O Decode complet/ciblé | Documents uniquement par pipes et courtier borné du parent | Seul périmètre du lot |
| Archive-I/O Encode ZIP/7z/TAR/TAR.ZST | Sources et writer détenus par parent, IPC | Seul périmètre du lot |

## Résultats initiaux actuels

- `cargo check --all-targets` : réussi ; messages de canonicalisation propres au sandbox, aucun avertissement de code du projet.
- `cargo test --all-targets` dans le sandbox : compilation empêchée par RustEmbed (résolution absolue des assets : accès refusé), aucun résultat de tests déduit.
- Même commande dans l'environnement Windows normal : bibliothèque **644 réussis, 1 échec, 16 ignorés**. Échec préexistant avant changement : `commands::archive::archive_tests::archive_outputs_and_external_copies_preserve_internet_provenance`, `ArchiveCache(Full)`. Arrêt avant les autres exécutables.
- `cargo test --test archive_io_worker --test archive_io_pipeline` dans l'environnement normal : **14 + 20 réussis**, aucun échec/ignoré. Les tests adverses utilisent encore PowerShell à cette borne ; leur migration vers une sonde native est nécessaire pour prouver les scénarios sous LPAC.
- Avertissements de liens physiques du cache incrémental : environnement de compilation, pas avertissements de code du projet. Les 852 tests et retours visuels du 10 octobre demeurent historiques.

## R9 — contrat distinct à décider

Les contrôles indépendants actuels bornent noms, ressources, offsets, tailles et transitions ; le parent retient les objets, applique la provenance, contrôle publication et récupération, et compare l'empreinte de l'original. Ils ne prouvent pas la fidélité des contenus extraits ou encodés, l'exhaustivité d'un inventaire ni celle des pixels/textes.

L'encodeur doit consommer les sources, mais ses blocs de sortie bornés restent arbitraires. Un hash annoncé par le worker ou une relecture par le même codec ne prouve pas une correspondance hostile. Alternatives à cadrer après le lot R1 : format canonique construit/vérifié par le parent ; vérificateur indépendant isolé comparant inventaire et digests au plan de confiance ; référence externe fiable. Aucune de ces alternatives n'est encore autorisée à l'implémentation.

## Sources du mécanisme

- [Microsoft — lancement AppContainer et LPAC](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer) : modèle DACL à deux identités, différences LPAC/AppContainer, profil et attributs de lancement.
- [Microsoft — attributs de processus](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute) : liste de handles, capacités et rattachement au job.

## Implémentation et preuves finales — 12 octobre 2026

Le worker archive-I/O Windows utilise désormais un exécutable minimal séparé, `oxyde-archive-io-worker.exe`, lancé dans un LPAC sans capacité. Le lancement ne dispose d'aucun repli : un runtime absent ou une préparation refusée échoue avant le protocole. Le binaire GPUI complet échouait à l'initialisation des DLL sous LPAC ; l'extraction minimale permet aux codecs de fonctionner sans élargir les permissions. `SHUTDOWN_DIAGNOSTICS.md` a été consulté ; la sortie finale conserve le contrat Windows existant.

Le lanceur natif maintient les buffers d'attributs jusqu'à leur destruction, crée l'enfant suspendu, attache son job avant reprise et limite les handles hérités aux trois handles IPC. Un verrou partagé couvre les launchers `Command` Oxyde recensés ; les handles temporaires sont fermés avant de libérer ce verrou, y compris sur erreur. Les waits restent hors du verrou. Le profil bootstrap est supprimé avant reprise : aucun nettoyage de profil n'agit après l'exécution hostile.

Le runtime est copié dans un répertoire privé à DACL protégée, RX pour le SID exact de la session. Source, ancêtres, répertoire et exécutable restent retenus ; le transfert de handles compare l'identité complète puis le BLAKE3. Le nettoyage utilise les handles des seuls objets détenus. Un enfant étranger est conservé. Une terminaison retardée conserve les objets et journalise PID et chemin, au lieu de supprimer une image encore utilisée.

| Garantie | Imposé / observé / refusé | Non validé ou limite |
| --- | --- | --- |
| Token et permissions | AppContainer observé, zéro capacité ; attribut LPAC opt-out imposé. Le témoin AAP est lisible dans l'AppContainer normal et refusé dans le LPAC. | La requête native de classe 46 renvoie 87 sur cette machine ; aucune propriété n'est déduite de ce résultat. |
| Fichiers privés, registre, parent | Sonde hostile native : lecture/écriture/création privées, HKCU du parent et droits dangereux du processus refusés ; témoins positifs hors sandbox. Runtime lisible, modifications/DAC/owner/création dans cwd refusées. | Une ACL accordée explicitement au SID ou à ALL RESTRICTED APPLICATION PACKAGES peut autoriser l'accès. Témoin ARAP effectivement lisible. Ce n'est pas une interdiction absolue indépendante des ACL ; limite non acceptée implicitement. |
| Héritage | Handles étrangers absents selon identité d'objet ; test concurrent réel avec autre enfant et preuve BrokenPipe pendant sa survie. | ShellExecute et launchers tiers hors du verrou non validés ; pas de garantie universelle d'héritage. |
| Runtime / provenance | Altération réelle des octets du même objet détectée ; annulation pendant préparation ; nettoyage préserve l'objet étranger. Renommage natif refusé par partage (c0000043). | Branche de rejet d'une substitution d'identité non exercée ; fournisseurs et volumes non exhaustifs (R4). |
| Réseau | Zéro capacité réseau imposée. Témoins TCP IPv4/IPv6 hors sandbox et AppContainer normal disponibles. | Winsock échoue à l'initialisation sous LPAC (10107), donc connexions TCP non exercées. Aucun refus de permission TCP conclu : critère réseau R1 ouvert. |
| Ressources et annulation | Job existant avant reprise ; timeout, annulation, crash, OOM, trames falsifiées et saturation IPC exercés par sondes natives avec readiness. | Préparation avant compteur actif ; appels natifs bloqués non interruptibles (R6). Quotas globaux/résidus après crash restent ouverts (R3). |
| Codecs | ZIP/7z/TAR/TAR.ZST encodage et décodage, plus GZIP/XZ/BZIP2/ISO lecture ; tests de publication et rollback conservés. | Round-trips normaux ne prouvent aucune fidélité hostile (R9). |

Revue indépendante finale : favorable pour les protections partielles démontrées, sans nouveau défaut trouvé après correction de la course d'héritage. R1 entier reste ouvert : autres workers non confinés, réseau non validé et exceptions ACL. R9 n'a reçu aucune correction ni décision d'acceptation. Linux n'a été ni compilé ni exécuté ; R5 reste hors périmètre.

### Validation reproductible

Environnement : Windows 10.0.26300.0, Rust 1.98.1. Utiliser un TEMP/TMP privé neuf sous `target/security-validation`, sans purge du cache global. Les journaux locaux sont dans ce répertoire (non versionnés).

- `cargo test --lib platform::windows_worker_sandbox -- --nocapture` : 6 réussis, zéro ignoré ; les limites TCP/token/substitution ci-dessus sont consignées explicitement.
- `cargo test --lib platform::windows_process_launch -- --nocapture` : 1 réussi, zéro ignoré, concurrence réelle.
- `cargo test --test archive_io_worker --test archive_io_pipeline` : 23 + 20 réussis, zéro échec/ignoré.
- `cargo check --all-targets` et `cargo build --bins` finaux : réussis, sans avertissement du projet.
- `cargo clippy --all-targets` : réussi ; avertissements préexistants aux emplacements conservés, nouveaux diagnostics corrigés. Cette commande n'est pas sans avertissement.
- `git diff --check` : réussi.

La suite globale et les références de commits sont consignées dans l'addendum final ci-dessous. Les validations portent sur l'état intégré ; le préfixe du commit d'extraction n'a pas été exécuté dans un checkout séparé.

### Livraison et checklist manuelle non exécutée

Distribuer les deux exécutables côte à côte ; lancer `cargo build --bins` avant un premier `cargo run --bin oxyde`. Aucun repli vers le binaire UI. Vérifier manuellement sur une installation propre : ouverture/extraction des huit familles, compression des quatre formats, annulation et message d'échec si le sibling est absent. Vérifier les variantes de volume/fournisseur voulues avant toute promesse supplémentaire. Aucun test visuel utilisateur de cette passe n'est déduit des tests automatiques ; aucun Computer Use ni push.

Pour la discussion suivante, décider du contrat R9 séparément : octets attendus, exhaustivité et vérificateur indépendant. Un hash du worker, une consommation complète des sources ou une relecture par le même codec ne suffisent pas. Ne pas élargir ce lot automatiquement.

## Addendum final

`cargo test --all-targets` avec TEMP/TMP privé : **908 réussis, aucun échec, 17 ignorés préexistants** (lib 652/16, application 196/1, pipeline 20, archive-I/O 23, listing 7, preview 10). Le résultat imbriqué de la sonde enfant n'est pas compté deux fois. L'échec initial ArchiveCache(Full) disparaît avec le cache temporaire privé ; aucune purge globale ni modification de ce test.

Commits locaux : `3f135b2` — extraction du codec headless ; `2e69e05` — confinement LPAC et preuves adverses. Aucun push. Les changements utilisateur préexistants sont exclus des commits.
