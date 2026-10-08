# Oxyde — audit de sécurité Windows et suivi des corrections

Date : 8 octobre 2026.

Base examinée : `master`, commit `8be1ebef191342d493bb8c18136143a5bb307ab9`.

Ce document reprend les corrections décrites dans la conversation, à partir de la lecture du code et des essais locaux réalisés pendant cette session. Les sections 1 à 5 décrivent l'état observé sur le commit de référence avant correction ; le suivi des correctifs figure en fin de document. Les audits précédents ne sont pas considérés comme une validation de sécurité. Aucun outil Computer Use n'a été utilisé.

La priorité est de sécuriser les chemins d'archives : les essais ont confirmé qu'un ZIP peut écrire hors du dossier d'extraction et écraser un fichier local avec les droits du processus Oxyde.

## 1. Bloquer les sorties du dossier d'extraction

**Statut : confirmé par reproduction locale. Priorité : haute.**

### Emplacements

- `src/commands/archive.rs` : branche ZIP de `extract_archive_with_checkpoint()`.
- `src/systems/file_ops.rs` : branche ZIP de `perform_extract()`.
- `src/systems/archive_workspace.rs` : `ArchiveWorkspace::open_with_checkpoint()` et `external_file()`.

### Problème et conditions

Les deux chemins d'extraction utilisent `enclosed_name()` de la dépendance ZIP puis joignent son résultat au dossier cible. Avec la version verrouillée `zip 8.6.0`, certains noms produisent encore un chemin Windows préfixé par une lettre de lecteur. La jointure peut alors abandonner le dossier cible.

Un attaquant doit fournir une archive que l'utilisateur extrait ou dont il ouvre un fichier. L'ouverture d'un fichier interne prépare actuellement un espace temporaire en extrayant toute l'archive : une autre entrée peut donc déclencher le défaut. Les droits d'écriture restent ceux d'Oxyde ; aucune élévation de privilèges n'a été démontrée.

### Reproduction observée

Dans un répertoire de test isolé sur le lecteur C:, utilisé comme répertoire courant du processus de test :

1. Créer un fichier témoin `victim.txt` contenant `ORIGINAL`.
2. Créer un ZIP contenant un fichier inoffensif `readme.txt` et une entrée `a/C:victim.txt` contenant `OVERWRITTEN`.
3. Appeler `prepare_archive_file_for_open()` sur le chemin virtuel de `readme.txt`.
4. Constater que le fichier témoin extérieur à l'espace d'extraction contient désormais `OVERWRITTEN`.

Le même résultat a été obtenu avec `a/../C:victim.txt`. `enclosed_name()` renvoyait `Some("C:victim.txt")`. La variante passant par l'opération publique d'extraction a également créé un fichier hors du dossier cible ; cette branche utilise `create_new(true)`, ce qui empêche l'écrasement d'un fichier déjà présent mais ne garantit pas le confinement.

Les essais ont uniquement touché des fichiers témoins. Aucun contenu extrait n'a été exécuté.

### Correction proposée

Valider les noms bruts avant toute construction de chemin. Refuser les préfixes Windows, les composants contenant `:`, les remontées `..`, les chemins absolus et les noms Windows ambigus. Le rejet de `:` doit aussi empêcher la création de flux NTFS alternatifs : une entrée `file.txt:extra` a été acceptée et extraite pendant les essais.

Partager la validation entre les chemins d'extraction. Vérifier ensuite que la destination reste sous la racine et contrôler les liens et jonctions des répertoires traversés. Une vérification lexicale seule ne couvre pas les points de réanalyse ; une canonicalisation suivie d'une ouverture par nom reste exposée aux changements concurrents du système de fichiers.

### Validation attendue

Les entrées malformées doivent être rejetées sans création ni modification extérieure. Couvrir les chemins Windows, les flux alternatifs, les liens/jonctions et les archives normales, pour l'extraction explicite comme pour l'ouverture d'un fichier interne.

## 2. Valider les chemins dès l'affichage des archives

**Statut : confusion de chemin confirmée par reproduction locale. Priorité : haute.**

### Emplacements

- `src/commands/archive.rs` : `virtual_entry_path()`, `list_zip()`, `list_seven_zip()`, `list_tar()` et `list_iso()`.
- `src/ui/app/mod.rs` : `open_external_item()`.
- `src/systems/archive_workspace.rs` : résolution des chemins virtuels et `external_file()`.

### Problème et conditions

Les noms d'entrées servent à construire des chemins avec `PathBuf::push()`. Sous Windows, un composant préfixé par une lettre de lecteur peut remplacer le chemin de l'archive. Un élément affiché dans une archive peut ainsi désigner un fichier local extérieur.

L'utilisateur doit sélectionner ou ouvrir l'entrée concernée. Le fichier local effectivement visé dépend notamment du lecteur et du répertoire courant. Le comportement du lancement final par Explorer n'a pas été testé ; la mauvaise résolution du fichier a été confirmée avant ce lancement.

### Reproduction observée

Créer un ZIP contenant une entrée `C:sentinel.txt` avec un contenu distinct de celui d'un fichier témoin `sentinel.txt` dans le répertoire courant du lecteur C:. Appeler `list_archive_contents()`, puis `prepare_archive_file_for_open()` avec le chemin retourné.

Le listing et la préparation renvoyaient tous deux `C:sentinel.txt`. La lecture de ce chemin renvoyait le contenu du témoin local, et non celui de l'entrée du ZIP.

### Correction proposée

Appliquer la validation commune aux noms utilisés par les listings ZIP, 7z, TAR et ISO, avant de construire un `FileEntry`. Faire retourner une erreur à la construction d'un chemin virtuel invalide. À terme, représenter séparément le chemin de l'archive et son chemin interne, plutôt que les confondre dans un chemin système.

### Validation attendue

Aucune entrée malformée ne doit produire un chemin local extérieur. Vérifier le résultat du listing et les actions qui le consomment, notamment l'ouverture, la copie, le renommage et la suppression.

## 3. Préserver la provenance Internet des fichiers extraits

**Statut : perte de la marque confirmée par reproduction locale. Priorité : haute.**

### Emplacements

- `src/commands/archive.rs` : routines d'extraction.
- `src/systems/file_ops.rs` : `perform_extract()`.
- `src/systems/archive_workspace.rs` : `external_file()` et publication des archives modifiées.
- `src/platform/mod.rs`, `windows_impl.rs` et `linux_impl.rs` : abstraction à étendre pour le traitement propre à Windows.

### Problème et conditions

L'extraction crée de nouveaux fichiers sans transmettre la marque de provenance Internet de l'archive. Les copies temporaires destinées à une ouverture externe perdent également cette provenance.

Le scénario concerne une archive possédant un flux NTFS `Zone.Identifier`, notamment après un téléchargement. L'absence de la marque prive les protections Windows et les applications de cette information. L'effet précis sur les avertissements, SmartScreen ou le mode protégé dépend des politiques et applications installées ; leur contournement complet n'a pas été testé.

### Reproduction observée

1. Créer un ZIP inoffensif contenant `readme.txt`.
2. Lui ajouter un flux `Zone.Identifier` contenant `[ZoneTransfer]` et `ZoneId=3`.
3. Exécuter l'extraction via `FileOperationManager`, puis tester séparément `prepare_archive_file_for_open()`.
4. Vérifier le flux de provenance sur les fichiers produits.

Le flux de l'archive était lisible. Les fichiers extraits et la copie temporaire ne possédaient pas ce flux : la lecture échouait avec `NotFound`.

### Correction proposée

Implémenter le traitement Windows derrière le trait `Platform`, avec un comportement approprié pour Linux. Transmettre la provenance de l'archive aux fichiers produits avant de les rendre disponibles à l'ouverture. Couvrir l'extraction explicite, les espaces temporaires et les copies ouvertes dans une application. Examiner également la conservation de cette provenance lors de la reconstruction d'une archive.

Évaluer l'intégration d'Attachment Services, notamment `IAttachmentExecute`, pour respecter les politiques Windows. Propager une erreur de marquage pertinente plutôt que publier silencieusement un fichier dont la provenance devait être conservée.

Référence : [Microsoft — IAttachmentExecute::Save](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iattachmentexecute-save).

### Validation attendue

Sur NTFS, une archive marquée doit produire des fichiers conservant la provenance attendue, y compris après préparation pour ouverture. Tester aussi les archives non marquées et les échecs d'écriture du flux. Vérifier ensuite le comportement des applications cibles dans un environnement Windows de test.

## 4. Limiter réellement les aperçus texte

**Statut : lecture et aperçu non bornés confirmés par le code et un essai limité. Priorité : moyenne.**

### Emplacement

`src/ui/components/tooltip.rs` : branche `PreviewType::Text` de `PreviewTask::new()`.

### Problème et conditions

L'aperçu charge le fichier entier avec `read_to_string()`, puis conserve quinze lignes. Ce traitement ne limite ni la mémoire consommée par la lecture ni la longueur d'une ligne affichée.

Un fichier texte volumineux accessible dans un dossier parcouru peut provoquer une consommation excessive de mémoire lorsque son aperçu est demandé. Le traitement en arrière-plan ne supprime pas ce risque. Aucun épuisement mémoire volontaire de l'application n'a été provoqué.

### Reproduction observée

Appliquer la séquence actuelle de lecture et de sélection des quinze premières lignes à un fichier de 4 Mio contenant une seule ligne. La chaîne lue et l'aperçu obtenu mesurent chacun 4 194 304 octets. Ce test reproduit le traitement des données ; il ne constitue pas un essai de l'interface GPUI.

### Correction proposée

Remplacer la lecture intégrale par une lecture bornée. Combiner un plafond d'octets lus, un nombre maximal de lignes et un plafond de caractères affichés. Traiter correctement les caractères UTF-8 coupés à la limite. Ne pas utiliser uniquement la taille initiale du fichier comme protection, car celui-ci peut changer pendant la lecture.

### Validation attendue

Tester les fichiers courts, les fichiers volumineux, une ligne très longue, les caractères multioctets à la limite et les erreurs de lecture. La quantité lue et le texte envoyé au rendu doivent rester bornés.

## 5. Borner la décompression des miniatures Office

**Statut : risque établi par lecture du code ; épuisement mémoire non reproduit. Priorité : moyenne.**

### Emplacement

`src/platform/thumbnails.rs` : `get_office_thumbnail_uncached()`, lecture des entrées `docProps/thumbnail.jpeg` et `Thumbnails/thumbnail.png`.

### Problème et conditions

Après échec de la miniature fournie par le Shell, le code lit la miniature embarquée avec un `read_to_end()` sans plafond, puis la décode. Un document contenant une entrée fortement compressée ou une image aux dimensions excessives peut donc solliciter beaucoup de mémoire. La branche de repli doit être atteinte pour exploiter ce chemin.

### Moyen de vérification

Dans un environnement de test, préparer un document avec une miniature embarquée dépassant un budget raisonnable, forcer l'échec du fournisseur Shell et mesurer les allocations du repli. Commencer par des tailles modestes. Ce scénario n'a pas été exécuté pendant la session.

### Correction proposée

Remplacer `read_to_end()` par une lecture plafonnée, avec détection du dépassement. Vérifier les dimensions et configurer les limites de décodage avant l'allocation de l'image complète. Retourner une absence de miniature ou une erreur maîtrisée lorsque le budget est dépassé.

### Validation attendue

Une miniature normale doit continuer à s'afficher. Une entrée trop grande ou une image aux dimensions excessives doit être rejetée sans allocation proportionnelle à la taille annoncée.

## Risques complémentaires à vérifier et durcissements

Ces points ne sont pas présentés comme des exploitations démontrées pendant cette session.

| Point | Emplacements | Vérification ou durcissement proposé |
| --- | --- | --- |
| Changement concurrent de liens ou jonctions | Extraction dans `archive.rs`, publication et copie dans `file_ops.rs` et `file_copy.rs` | Tester le remplacement concurrent d'un parent entre contrôle et ouverture. Préférer des opérations liées à des handles et refuser les points de réanalyse lorsque le confinement est requis. |
| Budgets globaux de décompression | `archive.rs`, `archive_workspace.rs`, `file_ops.rs`, `utils/archive_limits.rs` | Limites ajoutées dans le second lot décrit ci-dessous. Ouverture ciblée pour ZIP ; extraction complète plafonnée pour les autres formats. L'allocation interne des parseurs et leur interruption forcée restent à vérifier. |
| Résolution des exécutables Windows | `windows_disk_images.rs`, `windows_shell.rs`, `windows_impl.rs`, `windows_executables.rs` | Les quatre appels à PowerShell/Explorer utilisent désormais des chemins système obtenus par API Windows via `Platform`. Les tests contrôlent le PATH, le répertoire courant et les variables de racine Windows. |
| Parseurs et fournisseurs d'aperçu | `thumbnails.rs`, `icons.rs`, chargement d'images GPUI | Vérifier les limites et l'isolation des décodeurs et extensions Shell. Aucune corruption mémoire exploitable n'a été démontrée. |
| Dépendances | `Cargo.lock`, `security/RUSTSEC_WINDOWS.json` | Analyse RustSec effectuée avec filtres Windows x86_64 : zéro vulnérabilité publiée signalée, trois alertes de maintenance. `yoke-derive` retiré a été mis à jour vers `0.8.4`. Un script local et une CI reproduisent le contrôle ; les alertes de maintenance restent visibles. |

Référence sur la résolution des exécutables : [Rust — Command, comportement propre à la plateforme](https://doc.rust-lang.org/std/process/struct.Command.html#platform-specific-behavior).

## Vérifications déjà réalisées et limites

- Lecture directe des modules concernés et des chemins d'appel sur `master`.
- Reproductions Windows par un programme de test lié à la bibliothèque Oxyde construite depuis le dépôt, avec uniquement des fichiers témoins. Les essais de texte ont repris la séquence de traitement concernée.
- `cargo build --lib --locked --offline` réussi. Un avertissement lié aux liens physiques du cache incrémental est apparu dans l'environnement.
- `cargo check --locked --offline` réussi, sans avertissement du code du projet. L'environnement a émis des messages de canonicalisation de chemins.
- Lors de l'audit initial, aucun correctif ni test de régression n'avait été ajouté aux sources. Aucune validation visuelle de l'interface n'a été effectuée.

Ce document constitue un relevé ciblé des résultats de la session et des corrections proposées, pas une certification de sécurité de l'ensemble d'Oxyde.

## Suivi des corrections du 8 octobre 2026

Les cinq points décrits ci-dessus ont fait l'objet de corrections dans la copie de travail.

| Point | Correction intégrée |
| --- | --- |
| Chemins d'extraction | `utils/archive_security.rs` valide les noms bruts avant leur jointure : chemins absolus, lecteurs, `..`, flux NTFS, caractères invalides, noms de périphériques et suffixes ambigus sont refusés. ZIP, 7z, TAR et ISO utilisent cette validation. Les fichiers sont créés avec `create_new`, ce qui refuse aussi les destinations déjà présentes, liens et liens physiques compris. |
| Parents Windows | `platform/windows_archive_security.rs` ouvre les ancêtres et parents sans suivre leurs points de réanalyse. Des handles de lecture refusant le partage en écriture et suppression restent détenus pendant l'extraction, pour empêcher les renommages et l'installation concurrente de points de réanalyse. Les ancêtres sont contrôlés avant la canonicalisation de la racine. |
| Listings et chemins virtuels | Les listings des quatre familles d'archives et les chemins internes des espaces de travail passent par la validation commune. Un nom invalide produit une erreur au lieu d'une identité de fichier extérieur. |
| Provenance Internet | Lecture du flux `Zone.Identifier` bornée à 16 Kio et propagation aux fichiers extraits, copies ouvertes à l'extérieur et archives reconstruites. Les erreurs de marquage remontent à l'appelant. Les API Windows passent par `Platform` ; Linux n'écrit pas de flux Windows. |
| Aperçus texte | `utils/preview.rs` borne la lecture à 64 Kio et l'affichage à 15 lignes et 4 096 caractères. La branche d'aperçu texte utilise ce lecteur. |
| Miniatures Office | Le repli lit au maximum 8 Mio plus un octet de détection du dépassement, limite largeur et hauteur à 4 096 pixels et configure un budget de décodage de 64 Mio. |

Les liens et fichiers spéciaux provenant de TAR sont désormais refusés lors de l'extraction. Les noms incompatibles avec les règles Windows et les entrées de fichier dupliquées ne sont plus acceptés. Une destination dont un ancêtre est un point de réanalyse est également refusée. Les archives ordinaires restent couvertes par les tests de parcours existants.

La propagation conserve la provenance de l'archive ; elle n'ajoute pas une nouvelle intégration à `IAttachmentExecute`. Les politiques SmartScreen et des applications externes n'ont pas fait l'objet d'une validation interactive. À cette étape du premier lot, les limites de décompression globales, l'isolation des parseurs, la résolution des exécutables et l'analyse complète des dépendances restaient des travaux distincts à vérifier. Le second lot ci-dessous actualise ces points. Le verrouillage des répertoires décrit ici est propre à Windows ; aucun confinement face aux changements concurrents de parents sous Linux n'est revendiqué.

### Validation des corrections

Les tests ajoutés couvrent les noms Windows malformés dans les listings, espaces temporaires et opérations ZIP ; les renommages et écritures sur les parents Windows ; le refus des jonctions ; la provenance des sorties ZIP, 7z, TAR et Zstandard et des copies externes ; les lectures texte bornées ; les miniatures Office trop volumineuses ou aux dimensions excessives.

Résultats de validation finale sur Windows :

- `cargo check --all-targets --locked --offline` : réussi, sans avertissement du projet.
- `cargo test --all-targets --locked --offline -- --quiet` : 202 tests de la bibliothèque et 112 tests de l'interface réussis, soit 314 réussites ; 17 tests préexistants ignorés ; aucun échec.
- `git diff --check` : réussi.
- Les tests de renommage et d'écriture des répertoires Windows et le test de jonction passent. Les tests de provenance couvrent ZIP, 7z, TAR et Zstandard, les copies externes et l'extraction ZIP du gestionnaire d'opérations.
- Aucun test ignoré n'a été activé. Aucune validation visuelle ni utilisation de Computer Use. Aucune validation d'exécution sous Linux.

La compilation et les tests Windows ont été exécutés avec l'accès local nécessaire : l'environnement restreint refusait la résolution des ressources embarquées et l'ouverture de certains ancêtres. Les résultats finaux ci-dessus proviennent des exécutions complètes réussies, après résolution des échecs intermédiaires.


## Second lot de durcissements — 8 octobre 2026

Le premier lot a été commité sur `master` sous `c7ffecb` (`fix(security): confine archive extraction and bound previews`). Les changements suivants prolongent ce commit.

### Budgets et ouverture ciblée

`src/utils/archive_limits.rs` centralise les politiques suivantes :

| Usage | Octets de fichiers | Entrées | Profondeur | Durée active coopérative |
| --- | --- | --- | --- | --- |
| Extraction explicite | 64 Gio | 100 000 | 64 composants | 30 minutes |
| Espace temporaire pour mutation d'archive | 8 Gio | 100 000 | 64 composants | 30 minutes |
| Ouverture externe d'un fichier | 256 Mio | 100 000 | 64 composants | 2 minutes |
| Listing | 8 Gio de flux TAR, hors marge de métadonnées | 100 000 | 64 composants | 2 minutes |

Les tailles annoncées sont vérifiées avant écriture et les octets réellement copiés sont comptabilisés séparément. Un dépassement renvoie une erreur avant d'écrire les octets excédentaires. Les additions sont contrôlées contre les débordements. Les boucles d'extraction vérifient l'annulation et le délai ; le temps passé dans les checkpoints de pause est exclu du délai. Pour TAR et ses variantes compressées, le flux décodé est lui aussi plafonné, avec une marge maximale de 4 Kio par entrée autorisée (390,625 Mio pour 100 000 entrées), afin de couvrir en-têtes, bourrage et métadonnées étendues, même pour des données sautées pendant un listing.

ZIP et 7z contrôlent le nombre d'entrées après lecture de leur index. Les listings contrôlent les noms, la profondeur et le nombre d'entrées. ISO limite également le nombre de records collectés par l'extraction avant qu'ils soient traités récursivement.

`commands/archive.rs::extract_zip_file_for_open` valide les noms de tout l'index ZIP puis ne décompresse que le fichier demandé. Les entrées non sélectionnées ne consomment pas le budget d'octets de sortie. Les sélections ambiguës après normalisation des séparateurs sont refusées. La provenance Internet reste propagée avant de rendre le chemin à l'appelant.

`archive_workspace.rs::external_file` conserve une extraction complète pour 7z, TAR et ISO, plafonnée à 256 Mio pour cette action. Cela peut refuser l'ouverture d'un petit fichier contenu dans une archive plus grande ; une extraction ciblée de ces formats reste une amélioration possible. Les espaces de mutation utilisent le plafond de 8 Gio. Ces plafonds sont des constantes internes, pas des préférences utilisateur.

`file_ops.rs::perform_extract_with_limits` applique le même budget aux ZIP du gestionnaire d'opérations et aux autres décodeurs. Son répertoire provisoire est supprimé après erreur ou annulation. Les espaces de travail et les tentatives d'ouverture utilisent des `TempDir` détruits sur erreur. Le cache conservé après une ouverture réussie ne dispose pas encore d'une politique globale d'expiration ou de taille.

Ces limites ne constituent pas une sandbox mémoire. Les allocations internes lors du parsing initial des index, les dictionnaires des décodeurs et un appel natif bloqué ne sont pas interrompus de force. Les durées sont vérifiées aux points de contrôle et après lecture, pas par un processus superviseur. L'isolation des parseurs et fournisseurs d'aperçu reste à traiter séparément.

### Exécutables Windows

Le trait `Platform` expose `system_executable(SystemExecutable)`. `windows_executables.rs` utilise `GetSystemDirectoryW` pour Windows PowerShell et `GetSystemWindowsDirectoryW` pour Explorer, sans utiliser `PATH`, `SystemRoot` ou `WINDIR` comme source du chemin. Aucun repli vers un nom nu n'est effectué si la résolution échoue. Les appels de `windows_disk_images.rs`, `windows_shell.rs` et les deux appels Explorer de `windows_impl.rs` utilisent ces chemins. Linux retourne une erreur « unsupported » pour cette API propre aux outils Windows.

Références : [GetSystemDirectoryW](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemdirectoryw), [GetSystemWindowsDirectoryW](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemwindowsdirectoryw).

Les tests lancent une commande PowerShell fixe et inoffensive avec un `PATH` et un répertoire courant contenant un faux exécutable. Un processus de test Rust séparé vérifie également que des variables `PATH`, `SystemRoot` et `WINDIR` modifiées n'altèrent pas les chemins retournés. Cela ne garantit pas que PowerShell puisse fonctionner avec un environnement système incohérent : falsifier `SystemRoot` empêche son runtime .NET de démarrer sur la machine testée.

### Analyse des dépendances

`cargo-audit 0.22.2` a été installé sous `target/security-tools`. La commande `./scripts/audit-security.ps1` télécharge la base RustSec sous `target/security-advisory-db`, analyse `Cargo.lock` avec les filtres Windows et x86_64, conserve le JSON sous `target/security-audit-windows.json` et échoue pour une vulnérabilité publiée ou une version retirée de crates.io. Aucune advisory n'est ignorée. Les avertissements de maintenance restent affichés et n'arrêtent pas seuls la commande.

Le résultat daté est conservé dans `security/RUSTSEC_WINDOWS.json`, avec la révision de la base et l'empreinte SHA-256 du verrou normalisé en LF. La base consultée contient 1 294 avis, à la révision `5701a6d2a6158d5d4a933df5141a0415c285075d`. L'analyse du verrou actuel ne signale aucune vulnérabilité publiée pour les filtres choisis. Elle ne prouve pas l'absence de vulnérabilités inconnues et ne constitue pas une revue du code des dépendances. Les filtres portent sur l'applicabilité des avis ; leur résultat ne remplace pas l'analyse du graphe Cargo propre à la cible.

| Alerte restante | Dépendance Windows observée | Suite proposée |
| --- | --- | --- |
| [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html), `paste 1.0.15` non maintenu | `image` via `exr/pulp` et `ravif/rav1e`, dont GPUI dépend également | Étudier les versions ou les fonctionnalités amont qui suppriment cette macro ; aucune vulnérabilité exploitable n'est affirmée par cet avis de maintenance. |
| [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206.html), `rustybuzz 0.20.1` non maintenu | `usvg/resvg` via GPUI | Planifier une mise à jour cohérente de la pile SVG/typographie et vérifier le rendu des langues et des SVG. |
| [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html), `ttf-parser 0.25.1` non maintenu | GPUI directement, `fontdb` et `rustybuzz` | Même chantier de typographie ; un remplacement isolé incompatible avec les API amont n'est pas appliqué ici. |

L'alerte initiale sur la version retirée `yoke-derive 0.8.3` a été résolue par une mise à jour ciblée vers `0.8.4`. Le reste du verrou et la révision GPUI n'ont pas été mis à jour. « Retirée » ne signifie pas automatiquement « vulnérable ».

`.github/workflows/security.yml` reprend le script sur Windows lors des changements du verrou ou du contrôle, des pull requests concernées, d'un contrôle hebdomadaire et d'un déclenchement manuel. Permissions GitHub limitées à la lecture et checkout fixé à un commit officiel ; aucun push ni exécution distante de cette CI n'a été effectué pendant cette session.

### Reproductions des contrôles du second lot

- `cargo test --lib archive_limits --locked --offline` : petites limites injectées pour tester les octets réels, les tailles annoncées, les débordements, les entrées, la profondeur, le flux décodé, le délai et l'annulation.
- `cargo test --lib targeted_zip_open --locked --offline` : ouverture d'un fichier de 4 octets dans un ZIP contenant un autre fichier dépassant le budget ; seule la sélection apparaît en sortie.
- `cargo test --lib zip_preflight_limits --locked --offline` : limites d'entrées/profondeur et collision de noms après normalisation, sans fichier créé en sortie.
- `cargo test --lib extraction_limits_apply --locked --offline` : plafonds réduits sur ZIP, 7z, TAR, Zstandard et ISO avec des fichiers témoins.
- `cargo test --lib extraction_failure_cleans --locked --offline` : nettoyage du gestionnaire pour ces cinq formats, après dépassement et après annulation, avec conservation des archives sources.
- `cargo test --lib windows_executables --locked --offline` : résolution native et lancement PowerShell contrôlé ; aucun lancement d'Explorer ni manipulation de l'interface.
- `./scripts/audit-security.ps1` : analyse actuelle, résultat JSON et code de sortie du contrôle automatisé.


### Validation finale du second lot

- `cargo check --all-targets --locked --offline` : réussi, sans avertissement du projet.
- `cargo test --all-targets --locked --offline -- --quiet` : 213 tests de bibliothèque et 112 tests d'interface réussis, soit 325 réussites ; 17 tests préexistants ignorés ; aucun échec. Le sous-processus de vérification des variables Windows a également réussi.
- Les tests supplémentaires vérifient le refus des listings trop profonds pour ZIP, 7z, TAR et Zstandard, ainsi que l'arrêt d'une copie après un premier bloc effectivement écrit.
- `./scripts/audit-security.ps1` : code de sortie 0, zéro vulnérabilité publiée signalée et trois avertissements de maintenance, aucune version retirée restante.
- L'empreinte du verrou dans `security/RUSTSEC_WINDOWS.json` a été vérifiée contre le fichier actuel.
- `git diff --check` : réussi. Pas de Computer Use, de push, de validation visuelle, ni d'exécution de la CI sur GitHub ou de tests sous Linux.

Les risques restant ouverts concernent principalement les allocations et l'isolation internes des parseurs/fournisseurs d'aperçu, les courses sur les parents dans les autres opérations de fichiers, le remplacement cohérent des dépendances non maintenues et la gestion globale du cache temporaire. Les limites coopératives ajoutées ne clôturent pas ces sujets.
