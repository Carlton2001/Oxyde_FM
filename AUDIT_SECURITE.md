# Oxyde — audit de sécurité Windows et suivi des corrections

Date : 8 octobre 2026.

Base examinée : `master`, commit `8be1ebef191342d493bb8c18136143a5bb307ab9`.

Ce document reprend les corrections décrites dans la conversation, à partir de la lecture du code et des essais locaux réalisés pendant cette session. Les sections 1 à 5 décrivent l'état observé sur le commit de référence avant correction ; le suivi des correctifs figure en fin de document. Les audits précédents ne sont pas considérés comme une validation de sécurité. Aucun outil Computer Use n'a été utilisé.

La priorité est de sécuriser les chemins d'archives : les essais ont confirmé qu'un ZIP peut écrire hors du dossier d'extraction et écraser un fichier local avec les droits du processus Oxyde.

## Avancement des six chantiers annoncés

État actualisé au onzième lot. Ces six points sont des chantiers de tailles différentes, et non six étapes presque achevées. Les lots de concurrence Windows détaillés ci-dessous appartiennent principalement au premier point ; leur nombre ne constitue pas un pourcentage d’achèvement de l’audit.

| Point annoncé dans la conversation | État et limite actuelle |
| --- | --- |
| 1. Modifications concurrentes des fichiers | Partiellement traité : extraction, publication, copies, déplacements, suppression récursive, dossiers provisoires et nettoyage des dossiers après déplacement réel protégés dans les scénarios testés. Onzième lot : vérification et suppression des fichiers des transferts avec archives liées au même objet. Restent notamment les nettoyages de dossiers des transferts mixtes, certaines opérations de dossiers de l’historique et la fixation des sources avant leur premier parcours. |
| 2. Reconstruction des archives : identité et empreinte de l’original | Terminé pour le défaut confirmé et testé au quatrième lot, commité sous `005125d`. Cela ne valide pas tous les chemins de destination ni les métadonnées et flux alternatifs concurrents. |
| 3. Décodeurs et aperçus : allocations et isolation dans un processus | Plafonds applicatifs déjà ajoutés ; isolation et maîtrise des allocations internes non réalisées. Chantier architectural distinct. Aucune corruption mémoire exploitable n’a été démontrée. |
| 4. Ouverture ciblée hors ZIP | Non réalisée pour 7z, TAR et ISO : préparation complète encore plafonnée. Amélioration d’usage distincte ; il ne s’agit pas d’une nouvelle exploitation confirmée. |
| 5. Dépendances non maintenues | Analyse Windows effectuée et version retirée corrigée. Les trois alertes de maintenance consignées dans le second lot restent ouvertes ; elles ne sont pas assimilées à trois vulnérabilités confirmées. |
| 6. Validation finale des messages et du sélecteur | Tests automatisés des traductions, seuils et états réalisés. Vérification visuelle et essai interactif du sélecteur non effectués, conformément à la consigne de ne pas utiliser Computer Use. Les commits sont identifiés dans chaque lot. |

**Borne du onzième lot :** protection des fichiers lors des transferts avec archives, validation et documentation de ce résultat. Le nettoyage de leurs dossiers, l’historique, les décodeurs, l’ouverture ciblée et les dépendances ne sont pas ajoutés automatiquement à ce lot. La vérification visuelle reste un essai manuel à organiser, pas une validation déjà acquise.

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

Ce tableau synthétise l’état des durcissements. Les scénarios confirmés par reproduction sont détaillés dans les lots ci-dessous ; les autres points restent à vérifier.

| Point | Emplacements | Vérification ou durcissement proposé |
| --- | --- | --- |
| Changement concurrent de liens ou jonctions | `archive.rs`, `utils/archive_security.rs`, `platform/windows_no_reparse.rs`, `platform/windows_file_tree.rs`, `platform/windows_copy_staging.rs`, `file_ops.rs`, `file_copy.rs`, `path_parents.rs`, `destination_tree.rs`, `history_move.rs`, `source_directories.rs`, `archive_move_sources.rs` | Troisième lot : créations d’archive et nettoyage du cache. Quatrième lot : identité et empreinte de l’archive originale. Cinquième lot : déplacements rapides et publication de copies liés aux handles Windows. Sixième lot : suppression récursive par handles, sans parcourir les cibles des jonctions. Septième lot : création et nettoyage des dossiers provisoires des copies réelles par handles, avec un témoin empêchant leur conversion en jonction. Huitième lot : destination des copies réelles détenue avant le plan et créations de sous-dossiers liées aux handles. Neuvième lot : destinations et dossiers provisoires des déplacements d’historique, nettoyage des copies vérifiées et attributs liés aux handles. Dixième lot : nettoyage des dossiers après déplacement réel lié aux identités du plan. Onzième lot : fichiers des transferts avec archives conservés depuis la copie jusqu’à leur suppression par handle. Le parcours initial des sources, les nettoyages de dossiers des transferts mixtes et certaines créations/suppressions de l’historique restent à confiner. |
| Budgets globaux de décompression | `archive.rs`, `archive_workspace.rs`, `file_ops.rs`, `utils/archive_limits.rs` | Limites ajoutées dans le second lot décrit ci-dessous. Ouverture ciblée pour ZIP ; extraction complète plafonnée pour les autres formats. L'allocation interne des parseurs et leur interruption forcée restent à vérifier. |
| Résolution des exécutables Windows | `windows_disk_images.rs`, `windows_shell.rs`, `windows_impl.rs`, `windows_executables.rs` | Les quatre appels à PowerShell/Explorer utilisent désormais des chemins système obtenus par API Windows via `Platform`. Les tests contrôlent le PATH, le répertoire courant et les variables de racine Windows. |
| Cache des ouvertures externes | `systems/archive_open_cache.rs`, `platform/windows_cache.rs`, `archive_workspace.rs` | Troisième lot : quota comptabilisé de 1 Gio, 512 entrées, admission selon les octets réels, expiration ordinaire après sept jours et éviction anticipée sous pression ; fichiers ouverts ou modifiés préservés. Ancien cache non versionné exclu du nettoyage. |
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
| Parents Windows | `platform/windows_archive_security.rs` ouvre les ancêtres et parents sans suivre leurs points de réanalyse. Des handles de lecture refusant le partage en écriture et suppression restent détenus pendant l’extraction et bloquent les renommages ainsi que les ouvertures en écriture de données. Le troisième lot a confirmé que ces partages seuls ne bloquent pas une mutation via les attributs ; les créations utilisent désormais une résolution native refusant les points de réanalyse. La canonicalisation de la racine a été remplacée par une résolution absolue lexicale pour éviter de suivre un parent modifié. |
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


## Troisième lot — parents, résolution native et cache — 8 octobre 2026

Le second lot a été commité sur `master` sous `cd34e18` (`fix(security): limit archive resources and resolve Windows tools safely`). Les changements suivants prolongent ce commit, sans modification d’interface.

### Contournement des partages Windows confirmé et corrigé pour les créations d’archive

**Constat confirmé :** un handle ouvert avec le seul droit `FILE_WRITE_ATTRIBUTES` a permis d’appliquer `FSCTL_SET_REPARSE_POINT` à un dossier temporaire vide alors qu’un handle de lecture refusant le partage en écriture et suppression le retenait. Le refus du partage en écriture bloque les écrivains de données, mais ne suffit donc pas à empêcher la transformation en jonction. La garantie annoncée pour ce mécanisme dans le premier lot était trop large ; elle est rectifiée ci-dessus.

L’auteur de cette modification doit pouvoir écrire les attributs du dossier concerné et effectuer une modification concurrente pendant une opération d’Oxyde. Cet essai local n’a touché que des dossiers et cibles temporaires. Aucune élévation de privilèges ni exécution de contenu extrait n’a été démontrée.

**Correction :** `src/platform/windows_no_reparse.rs` appelle `NtCreateFile` avec `OBJ_DONT_REPARSE`. Les composants sont résolus par Windows pendant l’ouverture native ; un point de réanalyse intermédiaire provoque un refus. `FILE_OPEN_REPARSE_POINT` évite également de suivre le dernier composant, puis les attributs du handle retourné sont contrôlés pour refuser un objet de réanalyse final. Les accès synchrones demandent explicitement `SYNCHRONIZE`. Les créations de fichiers restent exclusives, sans écrasement.

`windows_archive_security.rs` utilise ce mécanisme pour ouvrir les parents d’extraction, créer ou ouvrir leurs sous-dossiers et créer leurs fichiers. `utils/archive_security.rs::ExtractionRoot` conserve une racine absolue sans canonicalisation susceptible de suivre une jonction ajoutée après le contrôle. `windows_cache.rs` utilise aussi cette résolution pour le verrou et les handles de nettoyage. Les API restent derrière `Platform` et la compilation conditionnelle Windows.

Références primaires : [OBJECT_ATTRIBUTES et OBJ_DONT_REPARSE](https://learn.microsoft.com/en-us/windows/win32/api/ntdef/ns-ntdef-_object_attributes), [NtCreateFile en mode utilisateur](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile), [droits de FSCTL_SET_REPARSE_POINT](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4aeefef8-92c3-4abc-af7a-a610caf8a165).

**Reproduction du contrôle :** `cargo test --lib --locked --offline attribute_only_reparse_mutation -- --nocapture`. Le test crée une jonction modèle inoffensive, copie ses données de réanalyse sur un autre dossier retenu à l’aide d’un accès aux attributs, puis vérifie que les créations native de fichier et de sous-dossier échouent sans créer quoi que ce soit dans la cible extérieure. Les tests préexistants vérifient aussi le refus des jonctions déjà présentes.

### Protection partielle des autres opérations de fichiers

`src/systems/path_parents.rs::HeldParents` retient les ancêtres du chemin demandé et ceux de son parent résolu avant de commencer l’opération. Ces handles sont conservés pendant les copies et publications (`file_copy.rs`, `windows_file_copy.rs`, `windows_file_moves.rs`), les déplacements d’historique (`history_move.rs`), la vérification et suppression des sources copiées (`copied_source.rs`), les déplacements du gestionnaire et les suppressions sélectionnées (`file_ops.rs`). Le résultat de copie conserve ses parents jusqu’à la fin de la vérification de la source.

Les jonctions existantes restent utilisables dans les opérations ordinaires. Les dossiers ordinaires autorisent le partage en écriture nécessaire au renommage natif des fichiers enfants. Les tests montrent que les parents source, destination et cible résolue ne peuvent pas être renommés pendant les opérations testées et qu’ils redeviennent renommables après libération.

`ArchiveWorkspace` retient les parents de l’archive pendant sa durée de vie. Sa publication ouvre l’original et le fichier reconstruit, effectue les renommages exclusifs par handles, et utilise le même mécanisme pour restaurer l’original en cas d’échec. Un fichier apparu à destination n’est pas écrasé par cette publication sous Windows. Une sauvegarde impossible à supprimer est conservée avec un message dans le journal.

**Limites restantes :** ces protections ordinaires ne bloquent pas les mutations de réanalyse via les attributs et ne constituent pas un confinement complet. Les créations initiales de dossiers, certains nettoyages et le déplacement rapide du gestionnaire utilisent encore des opérations par nom. À cette étape du troisième lot, le contrôle d’une archive avant publication reposait sur taille et date ; le quatrième lot ci-dessous y ajoute identité et empreinte dès l’ouverture. Il reste à lier les autres opérations et les destinations de publication à une identité de parent stable, tout en conservant les jonctions choisies par l’utilisateur. Aucun confinement concurrent sous Linux n’est revendiqué.

Contrôles reproductibles : `cargo test --lib --locked --offline path_parents`, `cargo test --lib --locked --offline copy_keeps_source_and_publication_parents_fixed`, `cargo test --lib --locked --offline archive_parent_remains_fixed`.

### Quota et expiration des ouvertures externes

`archive_workspace.rs::external_file` utilise désormais `CacheEntry` sous `%TEMP%/oxyde-archive-open/v1`. Chaque extraction reçoit un dossier `entry-*`, un sous-dossier de données et un marqueur de propriété versionné. Les échecs suppriment l’entrée provisoire ; seules les ouvertures préparées avec succès la conservent.

La politique interne limite les octets comptabilisés conservés à 1 Gio et les entrées à 512. L’admission se fonde sur les octets réels, avec une marge de 16 Kio par fichier pour la provenance : les petites sélections ne réservent plus chacune 256 Mio. Le comptage ne mesure pas tous les blocs ou flux NTFS possibles. Un verrou interprocessus protège l’admission et la publication ; il est libéré pendant la décompression. Au maximum quatre préparations peuvent coexister. Leurs dossiers ne sont pas parcourus pendant le décodage : ils sont bornés par leurs limites d’extraction puis comptabilisés à la publication. Une ouverture attend en arrière-plan, au plus quinze secondes, si les quatre places sont occupées ou si le verrou est indisponible. Une préparation abandonnée de version 2 peut être récupérée après soixante secondes si tous ses handles exclusifs peuvent être obtenus ; une préparation active conserve son marqueur et reste protégée.

Le nettoyage se fait lors de la préparation suivante. Sept jours constituent une expiration ordinaire de confort, sans exigence de sécurité. Sous pression de taille ou de nombre, les copies les plus anciennes peuvent être supprimées avant ce délai, après une grâce de soixante secondes pour le lancement du lecteur. Avant toute suppression, Windows doit ouvrir tous les fichiers de l’entrée sans partage et obtenir les handles des dossiers. Un lecteur déjà ouvert empêche la suppression même s’il autorise normalement le partage en suppression. Pour les nouvelles entrées, un marqueur de version 2 conserve le chemin relatif, la taille, la date de modification et une empreinte BLAKE3 : toute modification ou tout fichier supplémentaire empêche le nettoyage, même si la taille et la date ont été rétablies. Les anciennes entrées de version 1 restent soumises à sept jours sans éviction anticipée. Les fichiers en lecture seule, propriétaires inconnus et points de réanalyse sont préservés ou provoquent un refus. La suppression utilise l’identité des handles détenus, avec le marqueur supprimé en dernier. Les parcours et nettoyages sont bornés à 65 536 éléments et 80 niveaux ; le budget de nettoyage est partagé entre les entrées parcourues.

Si aucune copie ne peut être supprimée sans risque, Oxyde affiche une explication localisée et une action « Extraire et ouvrir… ». L’utilisateur choisit un dossier avec le sélecteur natif ; le fichier est extrait puis ouvert depuis une copie permanente. Les noms existants sont préservés grâce à un suffixe et la provenance Internet est conservée. Les répertoires provisoires de cette action restent dans le dossier choisi, y compris pour les formats nécessitant une extraction complète. Aucune consigne de nettoyage manuel de `%TEMP%` n’est présentée.

**Limites :** le quota de 1 Gio concerne les entrées conservées après admission. Les quatre préparations transitoires peuvent ajouter chacune jusqu’à 256 Mio de données extraites et une copie sélectionnée de 256 Mio pour les formats autres que ZIP, soit environ 3 Gio au total avec le cache conservé, hors métadonnées et écritures externes. Le disque peut manquer de place avant ces seuils ; le dossier de secours doit lui-même avoir assez de place et permettre l’écriture. L’ancien cache non versionné reste intact et peut occuper de l’espace hors du nouveau quota. Les écritures externes ultérieures ne sont pas plafonnées. Une application qui ferme son handle mais compte réutiliser le chemin n’est pas détectable comme lecteur actif ; la grâce réduit ce risque sans garantir son cycle de vie. Aucun nettoyage périodique n’est effectué quand Oxyde ne prépare plus d’ouvertures. Sous Linux, le verrou et le quota sont implémentés mais le nettoyage automatique est refusé, faute de preuve équivalente de fermeture des lecteurs externes ; aucune exécution Linux n’a été validée.

**Reproduction :** `cargo test --lib --locked --offline archive_open_cache`. Des quotas et délais réduits couvrent la limite d’octets, le nombre d’entrées, l’expiration après fermeture d’un lecteur autorisant la suppression, la conservation des modifications récentes, les jonctions vers un témoin extérieur, les parcours trop profonds et le nettoyage après échec de construction. Les tests de provenance des copies externes exercent également le chemin public de préparation.

### Validation du troisième lot

- `cargo check --all-targets --locked --offline` : réussi, sans avertissement du projet.
- `cargo test --all-targets --locked --offline -- --quiet` : 225 tests de bibliothèque et 112 tests d’interface réussis, soit 337 réussites ; 17 tests préexistants ignorés ; aucun échec.
- Douze nouveaux tests couvrent le contournement par attributs et son refus natif, les parents retenus et le cache ; les tests préexistants d’extraction, de provenance, de copie, de déplacement et d’historique passent également.
- `git diff --check` : réussi. Document vérifié en UTF-8.
- Aucun changement du verrou des dépendances, push, Computer Use, essai visuel, exécution Linux ou déclenchement distant de CI pendant ce lot.

Les échecs intermédiaires ont permis de corriger les règles de partage nécessaires aux renommages natifs, le refus du point de réanalyse final et l’attente du verrou de cache. Les résultats ci-dessus décrivent la validation du troisième lot avant les ajustements d’usage suivants.

### Ajustements d’usage — cache et erreurs de limites

Les politiques de cache décrites ci-dessus intègrent les ajustements demandés après le troisième lot : éviction avant sept jours sous pression, admission selon les octets réels, protection des modifications par empreinte et extraction explicite vers un dossier choisi. Le délai de sept jours sert uniquement au nettoyage ordinaire ; il ne bloque plus la libération anticipée de copies inutilisées.

`models/error.rs::ArchiveLimitFailure` transporte la ressource et son seuil à travers les erreurs de décodeurs et les états d’opération existants. `utils/archive_limits.rs::ArchiveBudget` fournit cette cause pour les octets déclarés ou écrits, le nombre d’entrées, la profondeur, la durée active et le flux TAR décodé incluant les en-têtes. `ui/format.rs::error_detail` traduit au rendu dans les cinq langues. Les onglets de navigation, cartes d’opérations, notices et notifications d’échec utilisent ce traitement ; les notifications de limites et celles proposant l’extraction restent disponibles jusqu’à fermeture ou remplacement par de nouvelles notifications.

Exemples français : « la taille totale décompressée dépasse la limite de 256,0 Mo pour cette action » ; « l’archive contient plus de 100 000 fichiers et dossiers » ; « la durée maximale de 2 minutes a été atteinte ». Les unités suivent les conventions actuelles de l’application, calculées avec un diviseur de 1 024. Les messages distinguent la taille décompressée de la taille du fichier compressé et précisent que les pauses ne consomment pas la durée active. Les autres erreurs conservent leur traitement existant.

Contrôles reproductibles : `cargo test --lib --locked --offline archive_open_cache` couvre notamment l’éviction anticipée, les lecteurs ouverts, les modifications avec taille et date rétablies, les petites sélections et le maximum de quatre préparations simultanées. `cargo test --lib --locked --offline explicit_export_preserves` couvre ZIP et TAR.ZST, le maintien de l’archive source, les collisions de noms et la provenance Internet. `cargo test --lib --locked --offline failures_preserve_the_resource` vérifie les causes et seuils. Les tests d’interface `archive_failures_display_localized_thresholds_in_all_languages` et `archive_listing_limits_retain_the_threshold_in_the_tab` vérifient leur traduction et leur conservation dans l’onglet.

Les tests d’interface ci-dessus sont automatisés, sans lancement de fenêtre ni Computer Use. La lisibilité visuelle de la notification et le sélecteur de dossier n’ont pas fait l’objet d’un essai interactif.

Validation finale des ajustements du 8 octobre 2026 : `cargo check --all-targets --locked --offline` sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` avec 231 tests de bibliothèque et 114 tests d’interface réussis (345 réussites, 17 tests préexistants ignorés, aucun échec) ; `git diff --check` réussi et document/catalogues vérifiés en UTF-8. Un échec intermédiaire de partage Windows du dossier de secours et une course de comptage avec les dossiers provisoires ont été corrigés ; le test de régression du cache vérifie désormais qu’une préparation active n’est pas parcourue et qu’une préparation abandonnée est récupérée. Le troisième lot et ses ajustements ont été commités sur `master` sous `2544692` (`fix(security): harden Windows archive paths and manage open cache`).

## Quatrième lot — préserver l’archive originale avant publication — 8 octobre 2026

### Contournement confirmé du contrôle taille/date

**Emplacement :** `src/systems/archive_workspace.rs::ArchiveWorkspace::{open_in_directory, commit}` sur le commit `2544692`.

**Conditions et impact :** après l’extraction de la vue temporaire, une autre application ou un acteur disposant des droits d’écriture sur l’archive pouvait modifier son contenu en conservant sa longueur, puis rétablir sa date de modification. La publication de la reconstruction ne détectait pas cette modification et pouvait l’écraser. Le remplacement du fichier par un autre objet ayant les mêmes octets, longueur et date n’était pas détecté non plus. L’impact confirmé est une perte de modifications concurrentes ou le remplacement non voulu d’une nouvelle identité de fichier ; aucune élévation de privilèges n’a été démontrée.

**Reproduction inoffensive :** deux tests créent uniquement des ZIP temporaires avec une entrée de six octets. Le premier remplace « before » par « edited » dans une autre archive ZIP valide de même longueur et rétablit la date initiale. Le second remplace l’objet d’origine par un fichier distinct aux octets et à la date identiques. Tous deux ajoutent un fichier à la vue d’Oxyde et tentent sa publication. `cargo test --lib --locked --offline publication_preserves -- --nocapture` a échoué avant correction : les deux publications renvoyaient `Ok(())` au lieu de préserver la version actuelle. Après correction, elles sont refusées et les témoins restent intacts.

### Identité et empreinte dès l’ouverture

`Platform::open_archive_source` et `Platform::file_identity` sont implémentés dans `windows_archive_source.rs` via `windows_impl.rs`, et dans `linux_impl.rs`. Windows utilise l’identifiant natif de 128 bits et le numéro de volume de `GetFileInformationByHandleEx(FileIdInfo)`. Un fournisseur qui ne prend pas en charge cette classe peut fournir l’identité historique de 64 bits via `GetFileInformationByHandle` ; une erreur ou une identité historique nulle provoque un refus. Linux utilise le périphérique et l’inode obtenus sur le fichier ouvert.

Références : [Microsoft — FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info), [Microsoft — GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex), [Microsoft — GetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle).

La lecture initiale Windows refuse le partage en écriture et suppression. `ArchiveWorkspace` calcule une empreinte BLAKE3 du flux principal sur le fichier ouvert puis fournit ce fichier au décodeur. `commands/archive.rs::extract_open_archive_with_budget` transmet le même fichier à ZIP, 7z, TAR et ses variantes, ainsi qu’à ISO, sans rouvrir la source par son nom. Une seconde lecture vérifie l’empreinte avant d’accepter la vue. Les deux calculs et l’extraction partagent la même durée active, pauses exclues. L’empreinte est calculée par blocs de 64 Kio, avec vérification de l’annulation entre les lectures.

Avant de libérer cette protection initiale, un handle de suivi conservé pendant la vie de l’espace de travail est ouvert et son identité est comparée. Ce handle permet les modifications et remplacements externes après préparation de la vue, tout en maintenant l’objet initial vivant pour éviter la réutilisation de son identifiant.

À la publication, le handle destiné au renommage natif refuse les écritures et remplacements concurrents sous Windows. Le contrôle porte sur le caractère ordinaire du fichier, longueur, date, identité et empreinte. Une divergence arrête l’opération avant le renommage ; les fichiers reconstruits provisoires sont nettoyés et l’archive actuelle est conservée. La provenance Internet est transférée après ce contrôle. `ArchiveSourceConflict` transmet un marqueur de cause aux états d’opération ; `ui/format.rs::error_detail` et `ui/errors.rs::rename_message` affichent un message dans les cinq langues, y compris lors d’un renommage dans une archive : l’archive a changé, l’opération a été arrêtée et doit être relancée depuis la version actuelle. Un nom contenant seulement le marqueur technique n’est pas interprété comme une cause de conflit.

### Validation et limites du quatrième lot

Contrôles reproductibles :

- `cargo test --lib --locked --offline publication_preserves` : contenu modifié avec taille/date rétablies et remplacement par un autre fichier identique.
- `cargo test --lib --locked --offline initial_archive_read_blocks` : refus des ouvertures en écriture et du renommage pendant la lecture initiale Windows ; une écriture externe redevient possible après préparation.
- `cargo test --lib --locked --offline extraction_decodes_the_held_file` : ZIP, 7z, TAR et TAR.ZST restent décodés depuis le fichier détenu quand leur ancien chemin désigne une autre donnée.
- `cargo test --lib --locked --offline fingerprinting_is_cancellable` : annulation après une lecture partielle de 64 Kio.
- Test d’interface `archive_source_conflicts_are_translated_in_all_languages` et contrôle des catalogues : aucune cause technique brute dans le texte traduit.

**Limites :** les empreintes ajoutent des lectures du fichier compressé, en arrière-plan, et peuvent augmenter la durée des opérations sur de grandes archives. Elles portent sur le flux principal, pas sur l’ensemble des métadonnées et flux alternatifs. Les mappings mémoire modifiables préexistants et les fournisseurs réseau particuliers n’ont pas été éprouvés. Le confinement des destinations face aux mutations de réanalyse reste le chantier distinct décrit au troisième lot. Sous Linux, le fichier et son identité sont suivis mais les partages Windows n’ont pas d’équivalent dans cette implémentation ; les opérations de publication par nom restent exposées aux courses et aucune exécution Linux n’a été validée. Aucun Computer Use ni test visuel n’a été effectué.

**Résultat final :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 236 tests de bibliothèque et 116 tests d’interface réussis, soit 352 réussites, 17 tests préexistants ignorés et aucun échec. `git diff --check` réussi ; document et cinq catalogues valides en UTF-8. Sept nouveaux tests couvrent les deux contournements reproduits avant correction, la lecture initiale Windows, le décodage du handle, l’annulation et les messages de conflit traduits, dont le renommage. Aucun changement de dépendance ni push. Ce quatrième lot a été commité sur `master` sous `005125d` (`fix(security): verify archive identity and content before publication`).

## Cinquième lot — lier les opérations de fichiers à leurs parents Windows — 8 octobre 2026

### Redirection confirmée d’une suppression et d’une copie

**Emplacements :** `src/systems/path_parents.rs::remove`, `src/systems/file_copy.rs::copy_with_source`, `src/platform/windows_file_moves.rs` et déplacement rapide de `src/systems/file_ops.rs::perform_copy`.

**Conditions :** un autre processus doit pouvoir modifier les attributs d’une jonction traversée par le chemin sélectionné pendant l’opération. Les handles des lots précédents bloquent son renommage et les écrivains de données, mais ne bloquent pas un handle limité à `FILE_WRITE_ATTRIBUTES`. Réorienter cette jonction par `FSCTL_SET_REPARSE_POINT` peut faire résoudre le même nom de fichier dans un autre dossier accessible à Oxyde. Aucune élévation de privilèges n’a été démontrée.

**Reproductions inoffensives avant correction des sites d’appel :** `deletion_cannot_be_redirected_by_attribute_only_junction_retargeting` crée deux fichiers temporaires de même nom, sélectionne celui du premier dossier par une jonction et réoriente celle-ci au point de contrôle. Le test a échoué parce que le témoin du second dossier avait été supprimé. `copy_stays_in_the_selected_parent_after_junction_retargeting` réoriente la destination au premier rappel de progression : la copie se terminait avec succès mais sa sortie était absente du dossier initialement choisi. Les tests utilisent exclusivement des dossiers temporaires et des données témoins. Les deux tests passent après correction ; le test de copie couvre aussi le remplacement accepté et la réorientation de la source, en vérifiant tous les témoins étrangers et l’absence de dossiers provisoires résiduels.

### Corrections intégrées

`HeldParents` conserve désormais le handle de son parent résolu ordinaire, ouvert avec une résolution native refusant les points de réanalyse. `Platform::open_operation_file` ouvre le nom final relativement à ce handle avec `NtCreateFile`, sans suivre de lien final. Les opérations choisies avant un point de contrôle continuent donc à utiliser leur parent initial. Les jonctions existantes choisies par l’utilisateur restent utilisables : elles servent à la résolution initiale, puis le parent cible est détenu par identité.

`Platform::rename_operation_file` utilise `NtSetInformationFile(FileRenameInformation)` avec le handle de destination dans `RootDirectory` et un nom final unique, sans composant de traversée ni flux NTFS. L’essai initial via le wrapper Win32 renvoyait l’erreur 87 sur cette machine ; la forme native a été validée par les tests. Le déplacement rapide du gestionnaire appelle `path_parents::move_file` ; il ouvre la source et retient la destination avant son point de contrôle. Le déplacement garde l’identité du fichier, ses dates et ses flux alternatifs. Il n’autorise le remplacement que pour un conflit accepté ; sinon l’existence d’une destination est contrôlée par le renommage natif lui-même. Le plan copie/déplacement conserve également le résultat du contrôle de conflit, au lieu de relire l’existence de la destination pour décider a posteriori de l’autorisation de remplacement. Ce dernier intercalage a été identifié par lecture du code ; aucun test n’injecte précisément une écriture entre les deux anciennes lectures du plan.

La suppression d’un fichier ordinaire l’ouvre avant le point de contrôle et le supprime par son handle. Les déplacements rapides et suppressions n’exigent pas de lire le contenu du fichier : leur ouverture demande les droits d’attributs et de suppression, contrairement aux ouvertures utilisées pour vérifier une empreinte.

Les copies préparent leurs fichiers provisoires dans le chemin du parent résolu initialement et publient leur résultat relativement au handle de destination conservé depuis le début. Le chemin source résolu est également fixé avant le premier rappel de progression. L’empreinte de la copie et celle d’un fichier supprimé lors d’une annulation d’historique sont désormais calculées sur le handle qui servira à l’action, sans rouvrir son nom. `CopiedSource` utilise directement son parent retenu pour ouvrir la source.

Références : [Microsoft — NtSetInformationFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntsetinformationfile), [Microsoft — FILE_RENAME_INFORMATION et RootDirectory](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information).

### Validation et limites du cinquième lot

Contrôles reproductibles : `cargo test --lib --locked --offline path_parents` et `cargo test --lib --locked --offline copy_stays_in_the_selected_parent`.

Sept nouveaux tests couvrent les suppressions redirigées, le changement des jonctions source/destination pendant un déplacement, une destination apparue après préparation, le remplacement accepté avec conservation d’identité/date/provenance, l’annulation d’un déplacement, l’installation d’un nouveau point de réanalyse sur un parent ordinaire déjà détenu et les copies avec réorientation des parents. Le test d’installation d’un nouveau point de réanalyse vérifie que le renommage ne publie aucun fichier dans le dossier étranger, puis retire ce point de réanalyse dans la fixture avant de vérifier le résultat. Les tests préexistants de déplacement du gestionnaire, de copie native, de provenance et d’historique restent inclus dans la validation générale.

**Travaux restant à réaliser à l’issue du cinquième lot :** le début du parcours et du plan d’une opération précède encore la fixation individuelle des fichiers ; une modification avant cette fixation peut changer l’objet pris en compte. Les créations initiales de dossiers, la création du dossier provisoire d’une copie, les chemins de `CopyFileExW`, les suppressions récursives de dossiers et certains nettoyages/permutations de permissions restent des opérations par nom. Fixer le chemin résolu neutralise la réorientation des jonctions initiales reproduite ici, mais ne prouve pas un confinement complet de ces opérations face à toutes les mutations de leur arborescence. La destination des déplacements d’historique est fixée à chaque renommage, pas dès leur première préparation. Les replis entre volumes et les transferts combinant fichiers réels et archives doivent encore être relus et éprouvés avec ces mêmes scénarios. Les ouvertures de fichiers spéciaux ou points de réanalyse sont refusées dans les actions sécurisées ; une application maintenant un partage Windows incompatible peut également empêcher l’action. Aucun fallback par nom n’est ajouté en cas de refus natif. Sous Linux, les nouvelles méthodes conservent les implémentations par nom existantes : aucun confinement concurrent ni essai d’exécution Linux n’est revendiqué.

**Résultat final :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 243 tests de bibliothèque et 116 tests d’interface réussis, soit 359 réussites, 17 tests préexistants ignorés et aucun échec. `git diff --check` réussi ; les fichiers modifiés ont été vérifiés en UTF-8. Aucun Computer Use, test visuel, changement de dépendance, push ou activation de test ignoré n’a été effectué. Ce cinquième lot a été commité sur `master` sous `9f9d3c8` (`fix(security): bind Windows file operations to retained parents`).

## Sixième lot — suppression récursive liée aux dossiers ouverts — 8 octobre 2026

### Remplacement du dossier sélectionné confirmé

**Emplacement :** `src/systems/path_parents.rs::remove`, appelé par la suppression réelle de `src/systems/file_ops.rs::perform_delete`, sur `9f9d3c8`.

**Conditions et impact :** le parent était détenu, mais le dossier sélectionné lui-même était ouvert seulement après le point de contrôle, au cours du `remove_dir_all` par chemin. Un processus pouvant renommer les enfants du parent pouvait déplacer le dossier sélectionné et mettre un autre dossier à sa place pendant cet intervalle. La suppression parcourait le remplaçant, effaçait ses données et laissait subsister le dossier original. L’action reste limitée aux droits d’Oxyde ; aucune élévation de privilèges n’a été démontrée.

**Reproduction inoffensive :** `recursive_delete_keeps_the_selected_directory_identity` crée deux dossiers temporaires, l’un contenant le fichier sélectionné, l’autre un témoin étranger. Au premier point de contrôle, le test tente de renommer le dossier sélectionné puis de mettre le second à sa place. Avant correction, le test a échoué lors de la lecture du témoin : celui-ci avait été effacé. Après correction, le renommage est refusé pendant l’action, le dossier sélectionné est supprimé et le témoin demeure intact.

### Corrections intégrées

`Platform::remove_operation_tree` est implémenté dans `platform/windows_file_tree.rs` via `windows_impl.rs` ; `linux_impl.rs` conserve une suppression par chemin. `path_parents::remove` transmet le handle du parent retenu et le nom sélectionné. Windows ouvre ce dernier avec les droits d’attributs et de suppression, sans partage en suppression, avant le premier point de contrôle. Cette ouverture accepte un objet de réanalyse pour supprimer le lien lui-même, mais utilise `FILE_OPEN_REPARSE_POINT` et un seul nom final validé : elle ne parcourt pas sa cible.

Pour un dossier ordinaire, un lecteur d’énumération est ouvert relativement au même parent pendant que le handle initial empêche le remplacement du dossier. Cette seconde ouverture refuse les points de réanalyse. Elle ne requiert pas la disponibilité d’un identifiant de fichier natif ; l’identité est conservée par les handles et les règles de partage, sans introduire une dépendance à `FileIdInfo` pour chaque dossier.

Le parcours utilise `NtQueryDirectoryFile(FileNamesInformation)`. Chaque enfant est ensuite ouvert relativement au dossier détenu et supprimé par son handle. Les jonctions et autres points de réanalyse ne sont pas énumérés, même lorsqu’ils portent l’attribut de dossier. Un enfant disparu avant son ouverture est ignoré ; une autre erreur interrompt le parcours. Le dossier est supprimé uniquement après son énumération, et Windows refuse cette suppression s’il contient encore des éléments. Le lecteur est fermé avant la suppression du dossier.

Le parcours est itératif, sans récursion Rust. Un tampon partagé de 64 Kio plus 16 octets suffit à l’énumération d’un nom à la fois ; les longueurs et limites de ce tampon sont contrôlées avant décodage. La mémoire et les handles de parcours dépendent de la profondeur, pas du nombre total d’enfants. Les points de contrôle interviennent entre les lectures, ouvertures et suppressions. Le worker utilise les indicateurs de pause/annulation existants et ne compte pas un dossier annulé comme terminé. Le changement de vitesse pendant une grande suppression ajuste sa priorité sans répétition systématique d’un appel de priorité pour chaque enfant.

Références : [Microsoft — NtQueryDirectoryFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntquerydirectoryfile), [Microsoft — FILE_NAMES_INFORMATION](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_names_information).

### Validation et limites du sixième lot

Contrôles reproductibles : `cargo test --lib --locked --offline recursive_delete` et `cargo test --lib --locked --offline real_directory_delete`.

Sept nouveaux tests couvrent le remplacement du dossier sélectionné, les jonctions sélectionnées ou imbriquées réorientées pendant l’opération, l’installation d’une nouvelle jonction sur le dossier initialement ordinaire, 300 fichiers avec des noms Unicode et 80 niveaux de sous-dossiers, la conservation d’un lien physique extérieur, l’annulation après une suppression partielle avec libération des handles, et le refus d’un enfant en lecture seule sans perte de son contenu. Le test du gestionnaire réel vérifie également la protection du témoin derrière une jonction et le compteur de fin d’opération.

**Limites et suite :** une suppression interrompue ou refusée après progression peut être partielle ; les éléments déjà supprimés ne sont pas restaurés. Les fichiers ouverts avec un partage incompatible et les enfants en lecture seule conservent les refus Windows. Les ajouts concurrents peuvent ne pas apparaître dans l’énumération ; un dossier encore rempli provoque alors un échec, sans recours à une suppression par chemin. La pause et l’annulation sont vérifiées entre les appels système et n’interrompent pas un appel déjà bloqué dans un fournisseur de fichiers. Les fournisseurs SMB, FAT/ReFS et les arbres beaucoup plus profonds que la fixture n’ont pas fait l’objet d’essais dédiés. Sous Linux, le comportement reste fondé sur les chemins et aucune exécution n’a été validée.

Les créations initiales de dossiers, les dossiers provisoires et chemins de copie native, les nettoyages d’arborescences après déplacement, le début du plan, les replis entre volumes et les transferts combinant fichiers réels et archives restent à examiner. Ce lot remplace la suppression réelle sélectionnée, pas toutes les occurrences de `remove_dir_all` du dépôt. Aucun Computer Use, essai visuel, changement de dépendance, push ou activation de test ignoré n’a été effectué. Ce sixième lot a été commité sur `master` sous `d2c1328` (`fix(security): retain Windows directory identities during recursive deletion`).

**Résultat final :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 250 tests de bibliothèque et 116 tests d’interface réussis, soit 366 réussites, 17 tests préexistants ignorés et aucun échec. `git diff --check` réussi et fichiers modifiés vérifiés en UTF-8. Le compteur de suppression est également actualisé après la jonction des workers, même quand une petite opération se termine avant le premier rafraîchissement périodique. Un essai intermédiaire de réouverture du lecteur de dossier a été refusé par Windows ; le lecteur finalement retenu utilise l’ouverture native relative au parent pendant que l’objet sélectionné reste détenu.

## Septième lot — dossiers provisoires des copies réelles — 8 octobre 2026

### Création détournée par une nouvelle jonction confirmée

**Emplacement :** `src/systems/file_copy.rs::copy_with_source`, création par `tempfile::Builder::tempdir_in` sur `d2c1328` ; ancien `Staging::drop` utilisant `metadata` et `set_permissions` par chemin.

**Conditions et impact démontré :** le dossier de destination est initialement ordinaire et vide. Un processus disposant du droit d’écriture des attributs peut lui installer une jonction après la fixation des parents, avant la création du dossier provisoire. Le partage Windows interdisait son remplacement, mais ne suffisait pas à empêcher cette mutation d’attributs. La création par chemin suivait alors la nouvelle cible : un dossier `.oxyde-copy-*` et des données provisoires apparaissaient hors de la destination détenue. Les droits du processus continuent de s’appliquer ; aucune élévation de privilèges ni écrasement arbitraire d’un fichier existant n’a été démontré.

**Reproduction inoffensive :** `staging_is_never_created_in_a_new_reparse_target` crée un fichier source de 1 Mio, un dossier de destination vide et une cible extérieure avec témoin, tous dans une fixture temporaire. Le premier callback de progression installe la jonction via un handle d’attributs déjà ouvert. Le callback suivant recherche un dossier provisoire dans la cible puis annule la copie. Avant correction, le test a échoué avec `copy preparation wrote outside the retained destination`. Après correction, l’action refuse le dossier devenu point de réanalyse avant la copie, le témoin reste intact et la destination initiale reste vide.

### Corrections intégrées

`Platform::create_copy_staging` et `remove_copy_staging` passent par les implémentations Windows/Linux. Sous Windows, `platform/windows_copy_staging.rs` crée un seul nom relatif au handle de destination par `NtCreateFile`, avec `FILE_CREATE`, refus atomique des points de réanalyse et attribut caché dès la création. Un UUID nomme le dossier ; une collision est un refus, sans ouverture, écrasement ni nettoyage du dossier existant. Une vérification des attributs des handles intervient avant et après la préparation. Le test de création native couvre également une réorientation du parent déjà effectuée, sans dépendre du contrôle préalable des métadonnées : aucun enfant n’apparaît dans la cible étrangère.

Un fichier caché `.pin` est créé relativement au dossier provisoire. Son handle interdit sa suppression et son remplacement pendant l’action. Il maintient le dossier provisoire non vide, même avant la première écriture de données ; Windows refuse alors sa conversion en jonction. Le dossier provisoire détenu maintient lui-même son parent non vide. La copie native reçoit ce handle déjà détenu ; la réouverture de `output` avant vérification et publication est relative à ce même handle. Cela évite de rouvrir par chemin le dossier provisoire avec un partage incompatible avec son droit de suppression. La copie native conserve `CopyFileExW`, ses flux alternatifs, ses attributs, sa progression et les contrôles de publication précédents.

Le nettoyage ouvre seulement `output` relativement au dossier détenu, sans suivre un point de réanalyse final, puis supprime les objets par leurs handles. Il retire l’attribut « lecture seule » sur le handle d’un fichier ordinaire uniquement si celui-ci n’a qu’un lien physique, afin de préserver les attributs d’un autre nom. Un lien substitué est supprimé lui-même sans parcourir sa cible. Le témoin est supprimé par son handle et fermé avant la suppression du dossier. Des enfants inconnus empêchent la suppression finale ; aucune suppression récursive de remplacement n’est utilisée. Un refus de nettoyage est consigné dans les logs et peut laisser un dossier caché résiduel. Les copies, annulations et conflits ordinaires doivent ne laisser aucun dossier provisoire.

Références : [Microsoft — NtCreateFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntcreatefile), [Microsoft — règles de FSCTL_SET_REPARSE_POINT](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4aeefef8-92c3-4abc-af7a-a610caf8a165).

### Validation et limites du septième lot

Contrôles reproductibles : `cargo test --lib --locked --offline windows_copy_staging` et `cargo test --lib --locked --offline systems::file_copy::tests`. Huit nouveaux tests couvrent la reproduction initiale, la création native relative après réorientation du parent, la protection du témoin et des dossiers avant toute donnée, les collisions avec dossiers et jonctions, le nettoyage d’un lien substitué, la conservation d’enfants inconnus, le refus de modifier les attributs d’un lien physique en lecture seule, et une tentative de conversion du dossier provisoire pendant la copie suivie d’une annulation.

**Effet pour l’utilisateur :** les copies normales conservent leur comportement et leurs métadonnées Windows. Un dossier temporaire caché contient un témoin pendant l’action, puis disparaît lors du nettoyage normal. Une mutation concurrente suspecte ou un partage incompatible peut faire échouer l’action ; aucune reprise par chemin n’est ajoutée en cas de refus natif. Ce dossier de travail accompagne chaque copie réelle ; ce n’est pas le cache d’ouverture des archives et ses limites/expiration ne changent pas.

**Suite à l’issue du septième lot :** les créations initiales de sous-dossiers dans `file_ops.rs::perform_copy` utilisent encore `create_dir_all` ; la fixation de la destination dès le début du plan reste à réaliser. Les dossiers provisoires d’historique entre volumes, les nettoyages de dossiers sources après déplacement, les permissions de certains autres nettoyages et les transferts combinant fichiers réels et archives restent à examiner. `CopyFileExW` reste une API par chemin : ce lot protège les dossiers de sa destination provisoire, sans revendiquer une fixation complète du dernier composant de sa source face à toute mutation. Des essais SMB/FAT/ReFS et entre volumes distincts restent à faire. Sous Linux, le repli reste par chemin ; aucune garantie de confinement concurrent ni exécution Linux n’est revendiquée. Aucun Computer Use, changement d’interface, dépendance, push ou activation de test ignoré. Le septième lot a été commité sur `master` sous `4bc0785` (`fix(security): anchor Windows copy staging to retained directories`).

**Résultat final :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 258 tests de bibliothèque et 116 tests d’interface réussis, soit 374 réussites, 17 tests préexistants ignorés et aucun échec. Les six tests du module de staging et les onze tests de copie passent également séparément. Une première validation globale avait détecté des refus de partage à la réouverture du dossier provisoire : le passage et la réutilisation de son handle ont corrigé ces refus avant la validation finale. `git diff --check` réussi ; les neuf fichiers modifiés ou créés ont été vérifiés en UTF-8.

## Huitième lot — destination et créations de dossiers des copies réelles — 8 octobre 2026

### Création via une jonction imbriquée confirmée

**Emplacement :** `src/systems/file_ops.rs::perform_copy`, boucle `directories_to_create` et préparation du parent dans chaque worker, sur `4bc0785`. Les deux emplacements appelaient `create_dir_all` par chemin ; la destination racine était seulement résolue avant le plan, puis les parents des fichiers étaient fixés individuellement.

**Conditions et impact :** une copie ou un déplacement de dossiers fusionne dans une destination contenant une jonction imbriquée, préexistante ou introduite avant la création correspondante. La création par chemin traverse cette jonction et peut créer des sous-dossiers dans sa cible, hors de la racine choisie pour l’opération. Une jonction choisie directement comme destination est un cas distinct : l’utilisateur sélectionne alors sa cible résolue. La reproduction porte sur une jonction imbriquée, un arbre de dossiers vides et un témoin extérieur ; elle ne démontre ni élévation de privilèges ni écrasement arbitraire d’un fichier.

**Reproduction inoffensive :** `directory_copy_refuses_a_nested_destination_junction_before_creation` crée `source/nested/empty`, un dossier `target`, une cible `outside` avec témoin et la jonction `target/source -> outside`, uniquement dans une fixture temporaire. Il appelle le gestionnaire réel pour copier `source` vers `target`, puis vérifie le refus, l’absence de `outside/nested`, la conservation du témoin et de la source, et un journal de création vide. Avant correction, l’opération était acceptée et le test échouait avec `directory creation accepted a nested destination junction`. Après correction, il passe : la jonction est refusée avant toute création de ses descendants.

### Corrections intégrées

`src/systems/destination_tree.rs::DestinationTree` fixe la racine de destination avant le parcours des sources et la construction du plan. Les jonctions initialement présentes dans le chemin choisi sont résolues par `HeldParents` ; les noms demandés et les parents résolus restent détenus. Les dossiers descendants sont ouverts ou créés un composant à la fois, relativement aux dossiers déjà détenus. Le chemin doit être sous la racine résolue et ses composants relatifs doivent être ordinaires ; les échappements par `..` sont refusés avant toute création.

`Platform::ensure_operation_directory` passe par `windows_impl.rs` et `windows_file_moves.rs`. L’appel Windows utilise `NtCreateFile` avec `FILE_OPEN_IF` et refus des points de réanalyse, puis conserve un handle interdisant le remplacement du dossier. Le résultat `IO_STATUS_BLOCK.Information` indique si cet appel l’a réellement créé. Le journal `created_dirs` reçoit seulement ces créations, y compris les parents intermédiaires ; une ouverture d’un dossier existant ne le déclare pas créé. Cette journalisation ne dépend plus d’un test `exists` précédant la création. Elle intervient aussi avant de propager une éventuelle erreur de duplication du handle.

Les dossiers déjà ouverts sont conservés dans une table partagée jusqu’à la fin de l’opération, afin d’empêcher leur remplacement entre la préparation et les workers. Chaque copie et déplacement rapide reçoit des parents issus de cette racine, sans les résoudre de nouveau par chemin. Les handles nécessaires aux ancêtres sont également transmis aux copies publiées jusqu’à leur vérification et, pour un déplacement par copie, la suppression de la source. Le contenu est toujours copié avec la méthode native précédente ; le verrou de la table protège les ouvertures initiales et duplications, pas les transferts de données.

Les points de contrôle existants de pause et d’annulation interviennent entre les étapes de création. Une annulation après des créations peut laisser les dossiers déjà créés, qui restent inscrits dans le journal ; aucune suppression par chemin de remplacement n’est ajoutée. Les handles sont libérés à la fin de l’action. Sous Linux, le repli utilise `create_dir` et contrôle les dossiers ordinaires ; il reste par chemin et aucune garantie de confinement concurrent n’est revendiquée.

Référence native : [Microsoft — NtCreateFile, dispositions et résultat de création](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntcreatefile).

### Validation, effet utilisateur et suite

Contrôles reproductibles : `cargo test --lib --locked --offline destination_tree` et `cargo test --lib --locked --offline directory_copy_refuses_a_nested`. Sept nouveaux tests couvrent le gestionnaire réel avec une jonction imbriquée, huit workers créant les mêmes descendants Unicode avec journalisation unique, une nouvelle jonction installée entre deux créations, le maintien de la cible initiale d’une jonction sélectionnée pour copier et déplacer, le rejet des chemins extérieurs et traversées parentes, l’annulation après création partielle et la libération des handles, ainsi que le refus de remplacer un dossier préparé jusqu’à la fin de l’opération ou des parents encore détenus.

**Effet utilisateur :** les copies dans des dossiers ordinaires et la sélection directe d’une destination via une jonction existante restent utilisables. Une fusion rencontrant une jonction ou un autre point de réanalyse dans ses descendants est désormais refusée, avec l’emplacement dans l’erreur ; elle n’écrit pas dans cette cible étrangère. Les autres tags de réanalyse, notamment les dossiers de fournisseurs cloud, doivent faire l’objet d’essais de compatibilité dédiés. Les droits d’accès et partages nécessaires à la détention des dossiers restent requis. Ce lot ne modifie ni le cache d’ouverture des archives ni ses limites, expiration ou messages.

**Limites et suite à l’issue du huitième lot :** la table conserve un handle par dossier distinct ouvert ou créé, avec des duplications liées à la profondeur pour les workers. Sa mémoire et les ressources Windows dépendent donc du nombre de dossiers du plan ; les très grands arbres et fournisseurs SMB/FAT/ReFS restent à mesurer. Un fournisseur bloqué dans un appel système ne peut pas être interrompu par le contrôle entre étapes. Le parcours et la fixation initiale des sources, les dossiers provisoires d’historique entre volumes, les nettoyages de dossiers sources après déplacement, les autres permutations de permissions et les transferts combinant fichiers réels et archives restent à examiner. La conservation des dossiers pendant l’action ne prouve pas leur identité lors d’une annulation d’historique effectuée plus tard. `CopyFileExW` reste par chemin pour le dernier composant de sa source. Aucune exécution Linux, activation des tests ignorés, utilisation de Computer Use, modification de l’interface, dépendance ou push. Ce huitième lot a été commité sur `master` sous `79448d3` (`fix(security): retain Windows copy destination trees before planning`).

**Résultat final :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 265 tests de bibliothèque et 116 tests d’interface réussis, soit 381 réussites, 17 tests préexistants ignorés et aucun échec. Les six tests de `destination_tree` et la reproduction du gestionnaire réel passent également séparément. `git diff --check` réussi ; les onze fichiers modifiés ou créés ont été vérifiés en UTF-8.

## Neuvième lot — déplacements d’historique et nettoyage des copies — 8 octobre 2026

### Réorientation de la destination d’historique confirmée

**Emplacement :** `src/systems/history_move.rs::move_with` et ancien `NativeIo::rename`, sur `79448d3`. Le handle du parent choisi était conservé, mais le renommage appelait `rename_open_file_exclusive`, qui fixait de nouveau la destination par son chemin.

**Conditions et impact :** la destination demandée passe par une jonction. Un processus pouvant réécrire ses attributs peut réorienter celle-ci après la préparation, avant le renommage. La nouvelle résolution prend alors la nouvelle cible ; le fichier sélectionné est déplacé hors de la destination initiale, sous réserve des droits du processus. Aucune élévation de privilèges n’est démontrée.

**Reproduction inoffensive :** `history_move_keeps_the_initial_destination_after_junction_retargeting` prépare une source, un dossier choisi et une cible extérieure avec témoin, dans une fixture temporaire. Le point d’injection du renommage réoriente la jonction, puis appelle le véritable renommage Windows. Avant correction, le test échouait avec `history move followed the retargeted destination junction` : le fichier n’était pas dans le dossier choisi. Après correction, le fichier est dans la destination initiale, la source a été déplacée, le témoin reste intact et aucun fichier de sortie n’apparaît dans la cible étrangère.

### Modification des attributs d’un autre lien physique confirmée

**Emplacement :** `src/systems/file_copy.rs::remove_verified_output`, appelé notamment par les nettoyages d’annulation dans `commands/ops.rs` et `systems/history_file_undo.rs`.

**Conditions et impact :** une sortie vérifiée en lecture seule possède un autre lien physique. Retirer cet attribut sur le nom de sortie change aussi les attributs accessibles par l’autre nom, car ils désignent le même fichier. La concordance de l’empreinte du flux principal ne permet pas de détecter ce partage. Le défaut démontré modifie l’attribut « lecture seule » du témoin ; son contenu reste inchangé.

**Reproduction inoffensive :** `verified_copy_cleanup_preserves_readonly_foreign_hardlink_attributes` crée deux noms pour le même fichier temporaire, le rend en lecture seule et fournit son empreinte BLAKE3 attendue au nettoyage réel. Avant correction, le test échouait avec `verified copy cleanup changed the foreign hardlink attributes`. Après correction, le nettoyage refuse de retirer l’attribut, conserve les deux noms et leur contenu, et le témoin demeure en lecture seule.

### Corrections intégrées

`history_move::move_with` ouvre la source relativement à ses parents détenus et utilise les chemins résolus lors de la préparation. Chaque tentative de renommage reçoit le parent de destination initial, y compris la publication après copie ; aucune nouvelle résolution de destination n’est effectuée par l’adaptateur Windows. Le déplacement rapide reste prioritaire ; seul un retour `CrossesDevices` déclenche la copie.

Le repli utilise maintenant le même dossier provisoire détenu et son témoin non supprimable que les copies normales, avec le préfixe `.oxyde-history-`. La création, la publication et le nettoyage conservent les protections par handles. Une nouvelle jonction sur un parent initialement ordinaire est refusée avant la création provisoire. `CopyFileExW` reçoit la source déjà détenue et le parent provisoire déjà détenu, copie vers un nom absent et effectue les vidages des flux via le callback natif existant, avant les attributs finaux. Le changement supprime les anciennes permutations de permissions et réouvertures par nom autour de `sync_all`. L’empreinte du flux principal de la sortie et celle de la source sont vérifiées avant la publication et la suppression de la source.

`Platform::open_copied_file` ouvre relativement au parent la sortie destinée à être vérifiée, publiée ou nettoyée. Sous Windows, le handle autorise les attributs nécessaires au nettoyage, sans autoriser l’écriture de son contenu ni son remplacement. `remove_copied_file`, implémenté dans `windows_copy_staging.rs`, supprime par ce handle. Pour un fichier ordinaire en lecture seule, il vérifie le nombre de liens physiques et refuse d’en modifier les attributs s’il n’a pas exactement un lien. Une suppression refusée après retrait de cet attribut entraîne sa restauration sur le même handle ; un échec de restauration est rapporté. Les points de réanalyse sont traités comme objets lors du nettoyage provisoire, sans mutation d’attributs de leur cible. `remove_verified_output` conserve sa vérification BLAKE3, puis utilise ce nettoyage partagé et ses parents initiaux.

**Durcissement supplémentaire fondé sur la documentation :** `windows_file_copy.rs` ajoute `COPY_FILE_COPY_SYMLINK` à `COPY_FILE_FAIL_IF_EXISTS`. Microsoft documente qu’en son absence, un lien symbolique de destination déjà présent peut être suivi lorsque sa cible n’existe pas. La combinaison ajoutée refuse ce lien dans tous les cas ; une source devenue lien symbolique est copiée comme lien, puis refusée à l’ouverture native de la sortie avant publication. Cette protection s’applique aux copies normales et à celles d’historique. Le scénario de lien symbolique final n’a pas été reproduit localement dans ce lot : il reste distinct des deux défauts confirmés ci-dessus. Pour le vérifier dans une fixture disposant du droit de créer des liens symboliques, créer un lien `output` vers un fichier temporaire absent juste avant l’appel natif de copie ; attendre un refus et vérifier que sa cible reste absente. Les autres tags de réanalyse ne sont pas assimilés à une garantie générale par ce drapeau.

Références : [Microsoft — CopyFileExW et règles des liens symboliques](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-copyfileexw), [Microsoft — informations par handle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle).

### Validation, effet utilisateur et suite

Sept nouveaux tests couvrent la destination du renommage réorientée, les jonctions initiales de source et destination réorientées avant un repli par copie, le refus d’une nouvelle jonction avant la création provisoire, le nettoyage en lecture seule après publication et réorientation, la conservation des flux alternatifs, de `Zone.Identifier`, de l’attribut caché et de la date de modification, la restauration de « lecture seule » après un véritable refus Windows de suppression, et le nettoyage refusé d’un lien physique en lecture seule malgré une empreinte correcte. Les essais de repli injectent le retour de frontière de volumes puis utilisent la vraie copie native ; ils ne remplacent pas l’essai sur deux volumes réels, toujours ignoré.

**Effet utilisateur :** les déplacements et annulations ordinaires conservent les données et métadonnées Windows couvertes par les tests. Une destination réorientée pendant l’action ne détourne plus la publication. Un nettoyage d’une sortie en lecture seule partagée par plusieurs liens physiques est désormais refusé afin de préserver les attributs des autres noms. Si la source ne peut pas être supprimée après copie, la sortie appartenant à l’action est nettoyée ; un échec de ce nettoyage est signalé avec son chemin, sans suppression par nom de remplacement. Les règles du cache d’ouverture des archives et l’interface ne changent pas.

**Limites et suite :** les nettoyages de dossiers sources après déplacement (`file_ops.rs::perform_copy`, `remove_empty_copied_directories`) et les créations/suppressions de dossiers dans `history_file_undo.rs` restent par chemin. Le parcours initial des sources, la préparation globale de l’historique, les transferts mixtes avec archives, les très grands arbres et les fournisseurs SMB/FAT/ReFS restent à éprouver. La copie native emploie encore un chemin pour le dernier composant source ; l’ajout du drapeau de lien symbolique ne prouve pas son confinement face à tous les tags. BLAKE3 vérifie le flux principal : les écritures concurrentes des flux alternatifs et la validation des métadonnées restent à examiner. Les repliements Linux restent par chemin et aucune exécution Linux n’est revendiquée. Aucun Computer Use, changement de dépendance, activation de test ignoré ni push. Ce neuvième lot a été commité sur `master` sous `5c2022b` (`fix(security): bind history moves and copy cleanup to retained handles`).

**Résultat final :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 272 tests de bibliothèque et 116 tests d’interface réussis, soit 388 réussites, 17 tests préexistants ignorés et aucun échec. La sélection des tests d’historique passe également séparément (33 réussites, six ignorés). `git diff --check` réussi et les neuf fichiers modifiés vérifiés en UTF-8.

## Dixième lot — nettoyage des dossiers après déplacement réel — 8 octobre 2026

### Suppression par nom après déplacement : défaut confirmé de la primitive

**Emplacement :** sur `5c2022b`, la fin de `src/systems/file_ops.rs::perform_copy_real` reparcourt les sources après le déplacement des fichiers, trie les dossiers puis appelle `std::fs::remove_dir` sur leurs chemins. La liste et la suppression ne conservent pas l’identité initiale des dossiers.

**Conditions et impact :** un autre processus disposant des droits nécessaires remplace un dossier source devenu vide ou réoriente une jonction d’un parent entre la préparation et le nettoyage. Une suppression par nom peut enlever un autre dossier vide. Le parcours effectué après transfert peut également inclure un nouveau dossier vide qui ne faisait pas partie du plan. Le défaut décrit concerne les dossiers vides : la suppression de fichiers étrangers par cette seule primitive n’est pas démontrée, et aucune élévation de privilèges n’est revendiquée.

**Reproduction inoffensive et portée de la preuve :** les fixtures de `src/systems/source_directories.rs` utilisent exclusivement leurs dossiers temporaires et les véritables appels Windows. `source_directory_cleanup_preserves_a_replacement_identity` renomme le dossier initial et crée un autre dossier vide au même nom au point de contrôle du nettoyage. `source_directory_cleanup_stays_in_the_initial_junction_parent` réoriente le parent vers une seconde cible contenant un dossier vide de même nom et un témoin. Une contre-épreuve temporaire remettant exactement `std::fs::remove_dir(&entry.path)` à la place de la suppression protégée fait échouer ces deux tests : le remplacement est accepté et le dossier initial n’est pas celui supprimé. Le test d’un nouvel objet de réanalyse échoue également avec cette primitive, qui accepte sa suppression. Cette contre-épreuve reproduit le défaut de l’appel précédent dans les fixtures ; elle n’est pas une exécution du binaire complet du commit précédent avec un processus concurrent. L’appel vulnérable a ensuite été retiré avant les validations finales.

### Correction intégrée

`file_ops.rs::perform_copy_real` capture maintenant chaque dossier accepté du plan de déplacement, avant que les workers déplacent ses fichiers. Le nouveau module `source_directories.rs` conserve son objet ouvert, son identité Windows, son parent initial et la correspondance de destination. Les descendants sont ouverts relativement aux dossiers sources déjà capturés ; la table partage leurs handles et ne duplique pas tous leurs ancêtres pour chaque descendant. Les sous-arbres refusés par la politique de conflit ne sont pas ajoutés au nettoyage.

Les nouvelles méthodes de `Platform`, implémentées dans `windows_file_moves.rs` via `windows_impl.rs`, ouvrent un dossier ordinaire sans suivre un point de réanalyse. Le handle de suivi demande seulement les attributs et autorise le partage nécessaire aux déplacements de contenu. Il garde l’objet et son identifiant en vie ; il ne prétend pas immobiliser son nom. Au nettoyage, une ouverture relative au parent retenu demande le droit de suppression, refuse un point de réanalyse, compare l’identité avec celle du plan et supprime par ce même handle. Une substitution est donc refusée avant suppression. L’identité est vérifiée dès la préparation pour signaler un fournisseur incompatible avant tout déplacement de fichier.

La table et les protections de préparation sont libérées avant le traitement des entrées, puis les handles propres aux descendants sont libérés de bas en haut. Aucun second parcours des sources, changement d’attributs ou effacement récursif n’est effectué. Windows décide si le dossier est vide au moment de l’appel : les fichiers ignorés, nouveaux fichiers et nouveaux sous-dossiers sont conservés. Un dossier absent est ignoré ; une identité changée ou une autre erreur est signalée avec le chemin et la mention que les contenus ont déjà été déplacés. Seules les suppressions acceptées sont ajoutées à `moved_dirs`. La suppression native peut rester en attente de fermeture d’autres handles autorisant sa suppression, selon les règles Windows.

Références : [Microsoft — informations de fichier par handle](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex), [Microsoft — modification et suppression par handle, droits requis et fermeture](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle).

### Validation, effet utilisateur et travaux restants

Huit nouveaux tests couvrent le remplacement d’identité, le maintien du parent initial après réorientation d’une jonction, le refus d’un nouvel objet de réanalyse, l’arrivée d’un fichier, la conservation d’un nouveau dossier vide absent du plan, le nettoyage de descendants Unicode avec maintien des contenus ignorés, l’annulation et la libération des handles, ainsi que le déplacement complet avec conservation des données et journalisation des dossiers vides. Commandes reproductibles : `cargo test --lib --locked --offline source_directory_cleanup` et `cargo test --lib --locked --offline directory_move_cleans_initial`.

**Effet utilisateur :** les déplacements ordinaires et les dossiers vides continuent à fonctionner. Si une autre application ajoute un contenu à une source, ce contenu et son dossier sont conservés. Un remplacement du dossier produit une erreur explicite indiquant que son remplacement a été préservé ; les fichiers déjà déplacés restent à destination. Le cache, les limites d’archives et l’interface ne changent pas.

**Risques encore à vérifier et durcissements suivants :** `perform_archive_transfer` utilise toujours `remove_empty_copied_directories` après une copie vers ou depuis une archive ; ce chemin n’utilise pas encore les identités de ce lot. Sa vérification d’empreinte des sources réelles puis leur suppression par nom doivent également être liées au même objet conservé. Les opérations de dossiers de `history_file_undo.rs` restent à traiter. Les chemins enregistrés dans l’historique ne constituent pas une preuve d’identité lors d’une annulation ultérieure. La fixation des sources avant leur premier parcours, les flux alternatifs concurrents, les très grands arbres et les fournisseurs SMB/FAT/ReFS restent à éprouver. Ce lot ajoute un handle partagé par dossier distinct et des protections d’ancêtres pour les racines : le coût augmente avec le nombre de dossiers. Le repli Linux compare l’identité puis supprime par chemin et ne fournit pas la garantie atomique Windows ; aucune exécution Linux n’est revendiquée. Aucun Computer Use, changement de dépendance, activation de test ignoré ni push. Ce dixième lot a été commité sur `master` sous `68f6a89` (`fix(security): bind moved directory cleanup to planned identities`).

**Validation finale :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 280 tests de bibliothèque et 116 tests d’interface réussis, soit 396 réussites, 17 tests préexistants ignorés et aucun échec. Les sept scénarios du nouveau module et le déplacement complet passent dans cette suite. La première assertion du test d’intégration a été corrigée pour comparer la destination au chemin Windows canonique déjà utilisé par l’application ; les données, dossiers et trois entrées du journal étaient corrects. `git diff --check` réussi ; les neuf fichiers modifiés ou créés vérifiés en UTF-8.

## Onzième lot — fichiers des transferts avec archives — 8 octobre 2026

### Vérification et suppression par ouvertures distinctes

**Emplacement :** `src/systems/file_ops.rs::perform_archive_transfer`, sur `68f6a89`. Les sources réelles sont copiées, leur empreinte est ensuite vérifiée par `File::open`, puis elles sont supprimées par `std::fs::remove_file` dans une autre boucle. Les fichiers des espaces d’archives sont eux aussi supprimés par leur nom après la copie. Aucun objet de source n’est conservé entre ces étapes.

**Conditions et impact :** un processus disposant des droits nécessaires remplace une source ou réoriente une jonction de son parent après la vérification et avant la suppression. La suppression par chemin peut alors effacer un fichier différent de celui qui a été vérifié et copié. Une modification pendant la reconstruction n’est pas couverte par une simple comparaison effectuée avant suppression. Le scénario confirmé ci-dessous porte sur une réorientation de parent et une suppression de témoin avec les droits d’Oxyde ; aucune élévation de privilèges n’est démontrée.

**Reproduction inoffensive :** `archive_move_sources_remove_the_original_after_parent_junction_retargeting`, dans `src/systems/archive_move_sources.rs`, prépare un fichier choisi, un témoin étranger de contenu distinct et une sortie, uniquement dans une fixture temporaire. Après copie et vérification, le point de contrôle avant suppression réoriente la jonction du parent. Une contre-épreuve remplaçant temporairement la suppression protégée par la primitive précédente `std::fs::remove_file(&entry.path)` échoue avec `NotFound` à la lecture du témoin : il a été supprimé. Avec la correction, le témoin et son contenu restent intacts, la source initiale a disparu et la sortie conserve les octets choisis. Il s’agit de la reproduction de la primitive vulnérable dans la fixture et non d’une exécution complète du binaire antérieur avec un second processus. La contre-épreuve a été retirée avant la validation finale.

### Correction intégrée

Lors d’un déplacement vers ou depuis une archive, les workers de copie ouvrent désormais chaque fichier réellement accepté avec `CopiedSource` avant sa copie. Les conflits ignorés ne demandent pas ce verrouillage. Les objets sources, sorties copiées, parents retenus et empreintes sont transmis au nouveau module `archive_move_sources.rs`. Les copies ordinaires sans déplacement conservent leur comportement.

`CopiedSource::verify` lit le flux principal sur le handle conservé et compare son empreinte à celle de la sortie copiée. `ArchiveMoveSources::verify_all` vérifie le lot entier avant toute suppression et n’autorise les suppressions qu’après sa réussite complète. Les objets restent ouverts durant cette vérification et la reconstruction ; sous Windows, les partages utilisés refusent les ouvertures incompatibles d’écriture de contenu ou de remplacement. `CopiedSource::remove_verified` effectue le contrôle d’annulation, puis appelle `Platform::remove_open_file` avec le même handle. L’implémentation Windows supprime par cet objet ; aucune nouvelle résolution de source n’intervient à cette étape.

Les suppressions sont séparées selon leur rôle. Les fichiers copiés des espaces d’archives sont retirés de ces espaces pour préparer la reconstruction ; les archives originales ne sont pas modifiées avant publication. Les sources réelles ne sont supprimées qu’après la publication de l’archive cible et les reconstructions sources prévues. La sortie copiée reste détenue jusqu’à la suppression native de sa source. Un échec de copie, de vérification, de publication ou une annulation libère les handles ; les sources réelles qui n’ont pas encore été supprimées sont préservées. Une erreur de suppression reste rapportée avec son chemin. La reconstruction de plusieurs archives conserve ses limites transactionnelles antérieures : une archive déjà publiée n’est pas annulée automatiquement si une publication ultérieure échoue.

Référence : [Microsoft — ouvertures natives, chemins relatifs et partages d’accès](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-zwcreatefile).

### Validation et portée utilisateur

Sept nouveaux tests couvrent la conservation des deux fichiers et la libération des handles après abandon de publication, la jonction réorientée avant suppression, le refus de supprimer sans vérification complète du lot, l’annulation avant suppression, la séparation des suppressions provisoires et réelles, le refus de publication après une véritable modification de l’archive cible et la conservation d’un conflit ignoré encore modifiable. Les deux derniers passent par le gestionnaire complet. Le test préexistant de conflits de déplacement d’archive passe également dans `cargo test --lib --locked --offline archive_move` : huit réussites. La suite globale couvre aussi les déplacements entre fichiers réels et archives ainsi qu’entre archives dans les formats existants.

**Effet utilisateur :** les fichiers réellement transférés peuvent refuser une modification dans une autre application jusqu’à la fin de la publication. Cette durée peut inclure la compression de l’archive. Un fichier ignoré pour conflit reste libre et est conservé. Une modification concurrente détectée de l’archive cible refuse sa publication et conserve les sources réelles ; le nouveau test vérifie également l’absence de résidus temporaires dans sa fixture. Aucun nouveau plafond, changement du cache ou modification d’interface n’est ajouté.

**Limites du lot :** le nettoyage des dossiers dans `remove_empty_copied_directories` reste par chemin ; il n’a pas été présenté comme corrigé. Les opérations de dossiers de l’historique et la fixation des sources avant le premier parcours restent distinctes. Les handles sources et sorties ainsi que leurs protections de parents sont conservés pour chaque fichier copié : le coût en ressources augmente avec le nombre de fichiers et la profondeur, et doit être mesuré pour les grands lots. BLAKE3 vérifie le flux principal ; les flux alternatifs, métadonnées et mappings modifiables préexistants ne sont pas validés globalement par ce changement. La copie native conserve un chemin pour le dernier composant de sa source. Les fournisseurs SMB/FAT/ReFS et les limites de partage propres aux volumes ne sont pas éprouvés dans ce lot. L’implémentation Linux de la suppression reste par chemin ; aucune garantie équivalente ni exécution Linux n’est revendiquée. Aucun Computer Use, test visuel, changement de dépendance, activation de test ignoré ni push. Ce onzième lot reste non commité et ne déclenche pas automatiquement un nouveau chantier.

**Validation finale :** `cargo check --all-targets --locked --offline` réussi sans avertissement du projet ; `cargo test --all-targets --locked --offline --quiet` : 287 tests de bibliothèque et 116 tests d’interface réussis, soit 403 réussites, 17 tests préexistants ignorés et aucun échec. La sélection `archive_move` passe également séparément : huit réussites, dont les sept nouveaux tests. `git diff --check` réussi ; les cinq fichiers modifiés ou créés vérifiés en UTF-8.
