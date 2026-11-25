# Audit : suivi des optimisations (série O)

Ouvert le 2026-11-20, à partir de la catégorie 6 de `101-restes.md`. Le texte de chaque constat est dans l'annexe de `101-restes.md` ; ce fichier n'en garde que l'emplacement et le suivi.

Règle : pas de changement sans gain plausible et mesurable ; un changement simple et sans risque (complexité quadratique, travail répété à chaque rendu, E/S sur le thread UI) peut se faire sans mesure. Un commit par constat sur `audit-fixes`, sans push.

États : « à examiner », « corrigé », « obsolète », « écarté » (raison en note), « à mesurer », « Linux ».

### Archives, workers et IPC

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| O1 | `commands/archive_io_worker.rs:57 et archive_io_codec.rs:77` | corrigé | d709a747 | source TAR, flux compressés et ISO lus par blocs de 256 Kio, octets décodés signalés par blocs : 90 petits fichiers passent de 1355 (TAR) et 1086 (tar.gz) allers-retours à 814 ; le rendez-vous Checkpoint avant chaque message reste (sémantique de pause, changement de protocole) |
| O2 | `commands/archive_io_codec.rs:257` | corrigé | e15cdcbc | la fenêtre de lecture repart à 4 Kio après un saut et double en lecture séquentielle : extraire un membre d'un ZIP de 20 fichiers de 300 Kio lit 412 Ko au lieu de 5,7 Mo ; l'en-tête local reste lu (aller-retour par membre), le crate zip n'expose pas les champs extra sans lui |
| O3 | `commands/archive_io_codec/parallel.rs:94` | corrigé | 50797e20 | threads d'encodage réutilisés (au plus un par travail admis) : 5000 fichiers de 1 Ko passent de 373 à 202 ms en release ; l'archive temporaire par membre reste (raw_copy_file en a besoin pour garder l'ordre) |
| O4 | `commands/archive.rs:231 et archive_handle_worker.rs:108` | écarté |  | changement d'architecture (cache d'index côté hôte validé par identité, admission distincte de la recherche, repli au-delà de MAX_REPLY) ; seuls TAR compressés et flux en profitent, ZIP/7z/ISO ne relisent que leur répertoire |
| O5 | `systems/archive_open_cache/recovery.rs:199` | corrigé | 1bf74130 | bloc à venir en base64 (les preuves en tableau restent lisibles) : preuve de 940 à 350 Ko, et DPAPI de 42 à 1,8 ms par bloc ; 16 Mio passent de 2,6 s à 0,26 s en release. Un sync_all par bloc reste (garantie de reprise) |
| O6 | `systems/archive_owned_tree.rs:463` | corrigé | 645df50b | un fichier sélectionné est retiré sans parcourir l'arbre, et la somme des noms n'est plus recalculée (tenue à jour, y compris au remplacement d'un dossier) ; un dossier sélectionné parcourt encore les clés une fois |
| O7 | `systems/archive_workspace.rs:203 et :191` | corrigé | e7635862 | sous Windows, seconde empreinte seulement sur un volume réseau (le partage en lecture seule refuse les écrivains en local) ; le parcours à la recherche de liens reste (défense en profondeur, sans métadonnée en plus de l'énumération) |
| O8 | `systems/archive_workspace.rs:414 (via archive_transfer.rs:68) et :592` | écarté |  | gros chantier : une source de copie ciblée demande une extraction par préfixe (dossiers) et la publication d'un membre hors de l'arbre possédé (renommage sans que son nettoyage supprime le fichier déplacé) ; l'export de :592 est le chemin de test, la production (prepare_selection_owned) copie aussi |
| O9 | `platform/windows_worker_sandbox.rs:197` | corrigé | 868131bf | source hachée pendant la copie (tenue sans partage en écriture) au lieu d'être relue, et plus de sync_all : le chargeur lit l'image par le même cache ; la copie hachée du binaire reste relue |
| O10 | `platform/windows_preview_quota.rs:290` | écarté |  | le dossier est borné à 4096 entrées (quelques appels d'énumération par écriture), et le compte exact sous bail est ce qui rend le quota juste entre processus ; un compteur en mémoire serait faux dès qu'un second processus écrit |
| O11 | `platform/windows_icon_pixels.rs:84` | écarté |  | une copie de 256 Kio au plus (dizaines de µs) par icône absente du cache de l'interface, qui garde l'image convertie ; l'appelant a besoin d'une image possédée, et l'extraction Shell qu'on évite coûte des millisecondes |

### Fichiers, opérations, historique

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| O13 | `systems/file_ops/file_copy.rs:282` | corrigé | 482a605d | la copie réutilise les parents déjà tenus par la sélection (et par la copie vérifiée de l'historique) au lieu de rouvrir la chaîne par fichier : 400 petits fichiers à profondeur 8, 4,1 s avant, 3,65 s après (release, moyenne de deux) |
| O14 | `systems/file_ops/copy.rs:315` | corrigé | c6b3c062 | à la planification, seules les entrées d'un espace de travail d'archive sont ouvertes pour vérifier leur propriétaire (même condition que verify_archive_item) ; les autres sont ouvertes une fois, à la copie |
| O15 | `systems/file_ops/destination_tree.rs:134` | écarté |  | par niveau, une lecture d'attributs sur un handle déjà tenu et une duplication de handle (vérification voulue qu'aucun ancêtre n'est devenu un point d'analyse) : quelques dizaines de µs par fichier à profondeur 8, contre environ 6 ms mesurées pour copier un petit fichier |
| O17 | `systems/file_ops/file_copy.rs:52` | écarté |  | mesuré : création du staging et de sa note 0,3 ms, nettoyage 0,1 ms, sur environ 6 ms par petit fichier (la copie native et son flush en prennent 3) ; le staging par fichier porte la garantie de publication et de reprise |
| O19 | `systems/history_file_undo.rs:400 et :686-691` | écarté |  | la seconde lecture vérifie le fichier juste avant de le supprimer ; l'éviter demanderait de tenir chaque sortie sans partage d'écriture de la première vérification à la validation, soit des milliers de handles pour une grosse copie |
| O20 | `systems/history_directories.rs:30-64` | corrigé | a29496c9 | dossiers en double ignorés (un stat par dossier distinct au lieu d'un par fichier), et le plan de suppression cherché par ancêtre dans un ensemble au lieu d'un parcours de la liste |
| O21 | `systems/folder_sizes.rs:150-176` | corrigé | e18326dc | sous Windows, tailles et nombre de fichiers lus dans l'énumération du dossier (une lecture de plus par dossier, aucune ouverture par fichier) ; repli par entrée si l'énumération échoue : 165 fichiers, 2,5 ms avant, 0,9 à 1,3 ms après (release) |
| O22 | `systems/history_move_redo.rs:419` | corrigé | 596254fb | partiel : entre volumes sous Windows, la source tenue sans partage en écriture n'est plus relue après la copie (3 lectures deviennent 2) ; la validation de toutes les sources avant le premier effet garde sa lecture (tout ou rien) |
| O23 | `systems/updates.rs:62` | écarté |  | un pic mémoire de la taille du paquet (une vingtaine de Mo en pratique), une fois par mise à jour ; garder le paquet en mémoire permet de vérifier la signature Minisign avant d'écrire quoi que ce soit |
| O24 | `commands/duplicates.rs:335, :390 et :445` | corrigé | 63b3bc9a | verrou par disque seulement pour un disque à pénalité de recherche ou réseau (is_ssd une fois par disque) ; 48 fichiers de 8 Mio en cache sur NVMe : 143 ms avant, 36 ms après (release) |
| O25 | `commands/duplicates.rs:330, :385 et :440, commands/search.rs:520` | corrigé | a4654b0a | identifiant de disque mis en cache par dossier pendant 30 s (4096 dossiers au plus) |
| O26 | `commands/duplicates.rs:409-424` | corrigé | 58d1743d | la seconde passe groupe par taille, hash de début et hash de fin : deux fichiers qui ne partagent que leur fin ne vont plus au hash complet |
| O28 | `commands/ops/trash_restore.rs:69` | corrigé | 354a1eb0 | quand chaque demande est un identifiant de la Corbeille (cas de l'interface), seuls les éléments demandés lisent leur enregistrement $I ; une demande par chemin d'origine garde la résolution de tous |
| O29 | `commands/ops/linux_trash.rs:84` | Linux | | |
| O30 | `commands/search.rs:153, :172, :460 et :539` | Linux | | |
| O32 | `commands/sidebar.rs:42-46 et :117-121, :58 et :133` | écarté |  | le tri passe déjà par sort_by_cached_key (une mise en minuscules par nœud) ; holds_folder s'arrête au premier sous-dossier, et un dossier sans sous-dossier, seul cas lu en entier, n'est pas parcouru ensuite : on ne peut afficher la flèche sans cette lecture |
| O33 | `models/config.rs:299-303 et :313-331` | corrigé | 65117cd4 | un fichier qui contient déjà les octets à écrire n'est plus remplacé ni synchronisé (themes.json à presque chaque sauvegarde) ; les rafales étaient déjà fusionnées (lot 10), la sérialisation de l'espace de travail sur le thread UI reste (quelques Ko) |
| O35 | `utils/audio_metadata.rs:38-44` | corrigé | edf3dd4a | pixels de pochette en base64 dans la réponse du worker (1,33 caractère par octet au lieu d'environ 4), tableaux encore lisibles ; module partagé utils/base64_bytes, repris par la preuve de cache (O5) |
| O36 | `utils/office_document.rs:50-57` | corrigé | 77e6d327 | tampon dimensionné par la taille déclarée (bornée par le budget) au lieu de doubler, et texte UTF-8 valide repris sans copie ; la conversion avec perte ne copie plus que les parties invalides |
| O37 | `utils/preview_protocol.rs:148-156` | corrigé | d64681c6 | la session d'images vérifie la réponse sans copier les pixels (elle en faisait une copie jetée), et l'aperçu décode une image unique dans le tampon reçu (decode_owned_with) : une image de 256 Mo n'est plus tenue deux fois |
| O39 | `utils/office_document/sheets.rs:104-109` | corrigé | 4ce2f4fe | la valeur affichée est copiée une fois, directement dans la cellule (allocation réutilisée), et seulement pour les colonnes gardées |
| O40 | `platform/linux_gio/applications.rs:153` | Linux | | |
| O41 | `platform/linux_permissions.rs:33` | Linux | | |
| O42 | `platform/linux_mime.rs:184 et :227` | Linux | | |
| O43 | `platform/thumbnails.rs:172 et :360` | corrigé | 556ae5e2 | partiel : conversion RGB sans clone RGBA ; le sync_all avant renommage reste (un cache lu après coupure ne doit pas être un fichier rempli de zéros), l'agrandissement vidéo à 640 est côté Linux (linux_impl.rs:333, session Linux) |
| O44 | `platform/windows_peek.rs:22` | écarté |  | hors du thread UI, neuf tests d'existence et deux petits JSON toutes les 5 s : moins d'une milliseconde ; un cache par date de fichier manquerait l'installation ou la désinstallation de PowerToys sans gain visible |
| O45 | `platform/windows_shell_delete_tracked/effects.rs:63` | écarté |  | garder les parents d'un élément pour les suivants tiendrait un handle sur des dossiers que le Shell doit ensuite supprimer, ce qui ferait échouer ou retarder leur suppression ; la preuve est prise puis relâchée par élément exprès |
| O46 | `platform/windows_shell_window.rs:197` | corrigé | 6bfa94d1 | les éléments déjà notés sont gardés dans un ensemble à côté de la liste ordonnée : test en temps constant au lieu d'un parcours |
| O47 | `platform/windows_file_tree.rs:72 et :169` | corrigé | 04412023, 6e20f3f5 | NtQueryDirectoryFile lit un tampon d'entrées (64 Kio pour une liste, 4 Kio par niveau pour une suppression) au lieu d'une entrée par appel ; mesuré 20 000 fichiers : 95 → 8 ms en local, 2,0 s → 23 ms par SMB (partage administratif local) ; la première requête de chaque dossier redémarre la liste, donc un dossier vide sur redirecteur se supprime aussi ; les threads Turbo ne démarrent qu'avec les sous-dossiers qu'ils prennent |

### Interface

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| O50 | `ui/app/archive_password.rs:58` | corrigé | 28ef443b | la racine listée par la sonde (mot de passe 7z/RAR, image disque) est gardée 10 s pour la navigation qui suit (commands/archive_probe.rs), si l'archive n'a pas changé (taille, date) : un lancement de worker et une analyse de l'archive en moins à l'ouverture ; oubliée si l'utilisateur est allé ailleurs |
| O52 | `ui/app/calendar.rs:6` | à mesurer |  | chaque boucle tourne sur le thread UI (cx.spawn), mais ne fait au repos qu'une lecture atomique ; les rendre événementielles demande un signal (Notify) à côté du compteur dans les API watch de Platform, Windows et Linux, et un réveil par session d'infobulle visible seulement : à décider sur une mesure des réveils au repos (changements de contexte du thread UI, consommation sur batterie) |
| O53 | `ui/app/archives.rs:122` | corrigé | 229c0b07 | extraction : un seul passage sur les onglets pour toute la sélection (files_for_action) ; glisser vers un onglet, Ouvrir avec, ouvrir dans un onglet et épingler : sélection en HashSet ; propriétés : un élément est couvert si l'un de ses ancêtres est un dossier sélectionné (ensemble), au lieu de comparer chaque paire |
| O54 | `ui/app/advanced_search/validation.rs:58` | écarté |  | une compilation par validation du dialogue (pas par frappe), bornée par la limite de taille du crate regex : quelques millisecondes au pire pour un motif saisi à la main ; la déplacer hors du thread UI ajouterait un état d'attente au dialogue sans gain visible |
| O55 | `ui/app/drag/external.rs:43-55, :98, :115-120 et :151-154` | corrigé | d2aba017 | chemins convertis une fois par glisser (comparés aux précédents sans allocation), filtrés une fois par cible (quatre dernières gardées) ; la cible n'est plus mise en minuscules pour chaque source |
| O56 | `ui/app/duplicates/search.rs:104-114` | corrigé | fa628e1f | déjà hors du thread UI et retardé de 150 ms ; un chemin ASCII est comparé sans copie (eq_ignore_ascii_case), les autres comme avant, et le nom n'est plus cherché à part quand il termine le chemin |
| O57 | `ui/app/drive_watchers.rs:107 et :453-467` | corrigé | 34041ef9, dd6624c3 | partiel : les comparaisons d'ancêtres d'un événement se font avant le verrou partagé ; l'éjection libère encore tous les observateurs : un dossier de montage (C:\mnt\usb) tient le volume éjecté sans partager sa lettre, et l'éjection reste rare |
| O59 | `ui/app/file_chooser/details.rs:45` | corrigé | aa5934b5 | les chiffres tabulaires ont tous la même chasse : les tailles sont regroupées par motif (chiffres remplacés par 0) et seul chaque motif est mis en forme, quelques dizaines au lieu de dizaines de milliers ; explorateur et sélecteur |
| O60 | `ui/app/file_chooser.rs:377` | corrigé | bfb394bf | le dossier personnel est demandé au système une fois ; le reste (quelques dizaines de favoris et lecteurs clonés puis comparés) est négligeable et garde la comparaison simple |
| O61 | `ui/app/file_chooser/browse.rs:102` | corrigé | 3aa170c4 | partiel : le listage n'est plus trié (arrange trie), et types et tailles des colonnes ne sont recueillis qu'au premier arrangement d'un listage, plus à chaque frappe ; le clonage des entrées affichées reste, hors du thread UI |
| O62 | `ui/app/file_chooser/navbar.rs:84` | corrigé | 5405f397 | partiel : les entrées du menu ne sont plus clonées entières à chaque rendu ; le listage (métadonnées, tri) reste hors du thread UI et sert l'ordre du menu ; les lignes restent reconstruites par rendu, car GenericDropdown possède ses éléments |
| O63 | `ui/app/file_chooser/browse.rs:133` | à mesurer |  | hors du thread UI ; le coût est l'ouverture de chaque sous-dossier pour sa flèche (holds_folder), qui est ce qui rend la flèche juste ; piste : sur un rafraîchissement d'observateur, ne vérifier que les sous-dossiers nouveaux et garder les autres drapeaux, F5 et la navigation vérifiant tout ; à mesurer sur un partage de milliers de sous-dossiers avant de changer ce compromis |
| O64 | `ui/app/duplicates/title_actions.rs:33` | corrigé | 01895a10 | l'ensemble des groupes réduits est partagé (Arc) avec la tâche de filtrage au lieu d'être cloné à chaque frappe ; « Tout réduire » le remplit encore une fois |
| O65 | `ui/app/file_preparation.rs:119-123` | corrigé | f4eaa3aa | cause du coût : une préparation de filtre (Current) copiait, retriait et réétiquetait toute la liste ; l'onglet retient l'ordre de ses fichiers (Weak sur le vecteur), et un filtre ou un regroupement ne copie ni ne trie plus : 100 000 entrées, environ 50 ms → moins d'une ms hors filtre ; un calcul périmé reste non annulable, mais il est désormais court |
| O66 | `ui/app/file_chooser/sidebar.rs:201-209 et :249` | à examiner | | |
| O67 | `ui/app/file_chooser/save.rs:215-219 et :111-119` | à examiner | | |
| O68 | `ui/app/item_pointer.rs:30-34` | à examiner | | |
| O69 | `ui/app/layout/tabs.rs:70-101 et :232-241` | à examiner | | |
| O71 | `ui/app/selection.rs:114 et :193-206` | à examiner | | |
| O72 | `ui/app/type_select.rs:72` | à examiner | | |
| O73 | `ui/app/viewer.rs:243` | à examiner | | |
| O74 | `ui/app/viewer/markdown.rs:162` | à examiner | | |
| O76 | `ui/app/viewer/content.rs:144` | à examiner | | |
| O77 | `ui/app/viewer/text.rs:135, :281-308 et :334` | à examiner | | |
| O78 | `ui/app/viewer/tree/loading.rs:47-55` | à examiner | | |
| O81 | `ui/components/conflict_dialog/advanced.rs:123-243` | à examiner | | |
| O82 | `ui/components/conflict_dialog/controls.rs:46-48` | à examiner | | |
| O83 | `ui/components/dropdown.rs:252-297, :326-336, :395 et :417` | à examiner | | |
| O84 | `ui/components/file_list/card_body.rs:54, :92 et :105` | à examiner | | |
| O85 | `ui/components/file_list/details.rs:101, grid.rs:514 et selection_button.rs:16` | à examiner | | |
| O86 | `ui/components/input.rs:134-140 et :626-647` | à examiner | | |
| O87 | `ui/components/open_with_dialog.rs:28, :108, :113 et :159` | à examiner | | |
| O89 | `ui/components/operations/footer.rs:36-40 et :65-71` | à examiner | | |
| O90 | `ui/components/popup/window/frame.rs:90` | à examiner | | |
| O91 | `ui/components/sidebar/tree.rs:35, :47, :72, :119-129 et :172, rows.rs:49` | à examiner | | |
| O92 | `ui/components/tooltip.rs:78-88` | à examiner | | |
| O93 | `ui/components/tooltip/presentation.rs:45-79 et :251-351, placement.rs:61-72` | à examiner | | |
| O94 | `ui/components/top_bar.rs:62-68 et :20-30` | à examiner | | |
| O95 | `ui/audio_covers.rs:14-32 et :54-59` | à examiner | | |
| O96 | `ui/date_groups.rs:24-29` | à examiner | | |
| O97 | `ui/preview_disk.rs:152 et :166` | à examiner | | |
| O98 | `ui/i18n.rs:1787` | à examiner | | |
| O100 | `ui/workspace/preparation.rs:41, :37 et :65` | à examiner | | |
| O101 | `ui/workspace/filters.rs:180 et :320` | à examiner | | |
| O102 | `ui/workspace/filters/criteria.rs:87` | à examiner | | |
| O103 | `ui/theme/states.rs:118` | à examiner | | |
