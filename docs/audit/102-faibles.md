# Audit : suivi des faibles restants

Ouvert le 2026-11-20, à partir des catégories 4 (faibles Linux F45 à F69) et 5 (faibles non examinés) de `101-restes.md`. Le texte de chaque constat est dans l'annexe de `101-restes.md` ; ce fichier n'en garde que l'emplacement et le suivi.

Chaque constat est vérifié sur le code actuel avant d'être corrigé. Un commit par constat sur `audit-fixes`, sans push.

États : « à examiner », « corrigé », « obsolète » (déjà corrigé ou ne se produit plus), « écarté » (choix volontaire ou gain trop faible, raison en note), « reporté » (refonte ou décision, raison en note), « Linux » (attend une session Linux).

### Plateforme Linux et services communs

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F45 | `platform/linux_file_clipboard.rs:82` | Linux | | |
| F46 | `platform/linux_file_clipboard/wayland.rs:42` | Linux | | |
| F47 | `platform/linux_download.rs:38` | Linux | | |
| F48 | `platform/linux_file_tree.rs:88 et :57` | Linux | | |
| F49 | `platform/linux_gio.rs:384, :391 et :486` | Linux | | |
| F50 | `platform/linux_gio/applications.rs:186` | Linux | | |
| F51 | `platform/linux_extended_attributes.rs:24` | Linux | | |
| F52 | `platform/hardware.rs:46, :80 et :146` | corrigé | e11d6088 | lettre de lecteur lue en ASCII (`drive_letter`) ; « é:\… » n'est plus un lecteur ; test |
| F53 | `platform/hardware.rs:79` | corrigé | a4654b0a | volume monté dans un dossier interrogé par son nom `Volume{…}` (numéro de disque et pénalité de recherche) ; repli sur un identifiant propre au volume ; à tester avec un disque monté dans un dossier |
| F54 | `platform/linux_permissions.rs:103` | Linux | | |
| F55 | `platform/linux_portal.rs:239 et :270` | Linux | | |
| F56 | `platform/linux_portal/chooser.rs:286` | Linux | | |
| F57 | `platform/linux_media/worker.rs:114` | Linux | | |
| F58 | `platform/linux_sandbox/seccomp.rs:38` | Linux | | |
| F59 | `platform/linux_volumes/holders.rs:35` | Linux | | |
| F60 | `platform/linux_trash.rs:72` | Linux | | |
| F61 | `platform/linux_trash.rs:33` | Linux | | |
| F62 | `platform/linux_quick_access.rs:35` | Linux | | |
| F63 | `platform/thumbnails.rs:116` | corrigé | 7f618735 | sans date de modification, miniature générée sans cache ; clé sans casse sous Windows (anciennes entrées orphelines, évincées par le quota) |
| F64 | `platform/thumbnails.rs:87` | écarté |  | choix de sécurité de c37f7b7b : le décodeur suit l'extension (test `cache_and_source_do_not_decode_a_different_format_than_the_extension`) ; un fichier sans extension d'image n'est pas demandé en miniature |
| F65 | `platform/thumbnails.rs:230` | Linux | | |
| F66 | `platform/thread_priority.rs:17` | Linux | | |
| F67 | `platform/viewer_media.rs:191` | corrigé | afc0480b | rapports du worker média Windows suivis du dernier seek appliqué (comme Linux) ; position ignorée tant que le seek demandé n'est pas appliqué ; workers à reconstruire ; à tester : glisser la barre de lecture |
| F68 | `platform/network.rs:112, :214-216, :236 et :181` | corrigé (en partie) | 706942a9 | tampon aligné pour `NETRESOURCEW` ; serveurs connectés : `ERROR_MORE_DATA` agrandit le tampon, autre erreur journalisée ; reste : pas de délai ni d'annulation sur les partages d'un serveur (appel bloquant de WNet) |
| F69 | `platform/network.rs:320` | corrigé | 706942a9 | nom d'utilisateur retiré du journal ; mot de passe du mappage effacé après usage (`Zeroizing`) ; `Debug` de `RemoteAnswer` sans identifiants ; le texte saisi dans l'interface reste une `String` (voir F152) |

### Archives, systèmes de fichiers, historique et opérations

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F70 | `commands/archive_iso_open.rs:208` | corrigé (en partie) | 6e67bba6 | arbre Joliet lu quand l'image en a un (noms Unicode) ; Rock Ridge non pris en charge ; tests ; workers à reconstruire |
| F71 | `commands/archive_iso_open.rs:181 et :195` | corrigé | 6e67bba6 | bornes des enregistrements et bourrage calculés sur le secteur de 2048 octets ; taille de bloc limitée à 512–2048 |
| F73 | `commands/archive.rs:239` | corrigé | 23825b4d | dossier interne absent : erreur « Archive directory not found » pour ZIP, RAR, TAR, flux, 7z (local et worker), comme ISO ; dossier vide ou implicite toujours listé ; test ; workers à reconstruire |
| F74 | `commands/archive.rs:143` | obsolète |  | corrigé par d1302a19 : `list_archive_contents_checked` arrête le listage quand l'onglet l'abandonne |
| F75 | `commands/archive_io_codec/parallel.rs:65` | corrigé | 5a8b8c25 | l'encodeur ZIP Turbo écrit par blocs de 1 Mio et s'arrête quand son travail est abandonné ; le `Drop` attend au plus un bloc ; workers à reconstruire |
| F77 | `systems/archive_open_cache.rs:516` | écarté | | choix du lot 10 : une copie modifiée par l'utilisateur n'est jamais évincée (catégorie 3) |
| F78 | `systems/archive_open_cache.rs:235` | corrigé (en partie) | 4d9bc4a9 | empreinte de la copie calculée avant de reprendre le verrou (l'entrée est privée jusqu'à son marqueur) ; l'éviction hache encore sous le verrou, nécessaire pour prouver l'entrée |
| F79 | `commands/archive_rar.rs:145` | corrigé | 6e12a3dc | RAR solide : premier membre chiffré (les précédents sont décodés de toute façon) ; non solide : le plus petit |
| F80 | `commands/archive_verify.rs:30, :51 et :65` | corrigé (en partie) | 9dfbbed4 | 7z : plus petit bloc chiffré AES ; ZIP : erreur de décompression (`InvalidData`) après le contrôle d'un octet comptée comme mauvais mot de passe (un membre endommagé aussi) ; reste : membre ZipCrypto stocké de plus de 1 Mio et RAR 4 stocké |
| F81 | `commands/archive_rar.rs:39` | corrigé | 6e12a3dc | `ResourceLimit` de rars : « RAR archive exceeds Oxyde's processing limits » au lieu de « Invalid RAR archive » (texte anglais, la limite atteinte n'est pas typée par rars) |
| F82 | `systems/archive_io.rs:171` | corrigé | 8d1d1cbb | documentation corrigée : `Operation::Decode::password` accepté pour ZIP, 7z et RAR |
| F83 | `systems/archive_owned_tree.rs:13` | reporté |  | choix de conception : chaque entrée garde parent et suivi pour un nettoyage par identité (sans énumération) ; réduire les handles demande une refonte du nettoyage |
| F84 | `utils/archive_codec.rs:85 et :87` | corrigé | 51b2ca4e | xz limité à 1 Gio de mémoire (erreur de limite claire) ; fenêtre zstd acceptée jusqu'à 2^30 (`--long` jusqu'à 30) ; documenté ; workers à reconstruire |
| F85 | `utils/archive.rs:112` | corrigé | 30452bff | le menu du dossier lit `archive_root` de l'onglet, résolu par la navigation hors du thread UI ; plus d'accès disque au clic droit |
| F86 | `systems/archive_volumes.rs:27` | Linux |  | réouverture des volumes par nom sous Linux |
| F87 | `systems/archive_workspace.rs:315, :333 et :352` | Linux |  | verrou de l'original sous Linux |
| F88 | `utils/archive_listing_protocol.rs:242` | corrigé (en partie) | 03612ad2 | caractères de contrôle et de mise en forme bidirectionnelle retirés ; test ; les messages internes anglais restent non traduits |
| F89 | `utils/archive_volumes.rs:58 et :44` | corrigé (en partie) | 9155c85a | ancien nommage poursuivi après .r99 par .s00 jusqu'à .z99 ; test ; reste : un double-clic sur film.r03 ne l'ouvre pas, car .rNN n'est pas reconnu comme archive (élargir la reconnaissance toucherait tous les fichiers .r00) |
| F93 | `commands/duplicates.rs:341, :396 et :446` | corrigé | b6b798f4 | fichiers illisibles comptés pendant le hash ; la fenêtre des doublons affiche « N fichiers n'ont pas pu être lus » (5 langues) ; à tester avec un fichier verrouillé |
| F95 | `commands/ops/conflicts.rs:54` | obsolète |  | déjà corrigé : `occupied` et la copie testent le nom par `symlink_metadata` (lien cassé compris) |
| F96 | `commands/ops/trash_listing.rs:53` | obsolète |  | déjà corrigé : le repli lit l'élément par `symlink_metadata`, sans suivre le lien |
| F98 | `commands/ops.rs:482` | reporté |  | rejeu hors du panneau des opérations, sans jeton d'annulation venu de l'interface ; même plomberie que M17 et M74 (annulation à faire passer par l'historique) |
| F99 | `commands/image_handle_worker.rs:76-79` | corrigé (en partie) | 1f32bae5 | workers image, audio et Office : code de sortie 1 en cas d'échec (le parent vérifie le statut) ; sessions d'icônes et d'images inchangées (leur fin habituelle est la fermeture par le parent) ; workers à reconstruire |
| F104 | `systems/file_ops/copy.rs:273` | corrigé | 4c3775b7 | `same_entry` exige aussi le même dossier parent (identité) ; un lien physique ailleurs passe par la question de conflit ; test |
| F105 | `systems/file_ops/archive_transfer.rs:171` | reporté |  | garder les effets d'une extraction vers un vrai dossier demande une entrée d'historique annulable mais non rétablissable (la source est une vue temporaire de l'archive) : à concevoir avec l'historique |
| F107 | `systems/file_ops/copy.rs:561` | corrigé | c2132fc8 | Windows : type de stockage demandé par dossier source (avec F53) ; Linux inchangé (un stat par fichier, sans requête de stockage) |
| F109 | `systems/history_delete.rs:83-90 et :216-226` | reporté |  | une annulation partielle (éléments prouvés restaurés, les autres laissés en corbeille) demande un compte rendu à l'interface, comme `retained` de F111 ; à faire avec ce rapport |
| F110 | `systems/history_links.rs:33-45` | corrigé | 80049d55 | si l'identité du lien recréé ne peut être relevée, ce lien précis (même identité système) est retiré avant l'erreur ; sans test (échec de capture non reproductible) |
| F112 | `systems/file_ops/delete.rs:148-169 et :297-307` | corrigé (en partie) | d681cab7 | suppression définitive : élément déjà disparu compté terminé ; reste : descendants d'un dossier sélectionné écartés par `collapse_roots` (corbeille Windows) toujours « non traités » |
| F113 | `systems/file_ops/scheduler.rs:236-273` | corrigé (en partie) | e6f015bc | préfixes `\\?\C:` et `C:`, `\\?\UNC` et `\\serveur` réservés pareil ; test ; reste : lecteur mappé, jonction et nom d'extraction « X (1) » (demandent des E/S, or la réservation se fait sur le thread UI) |
| F114 | `systems/folder_sizes/pools.rs:59-60` | corrigé | 087c4048 | seul un pool construit est mémorisé ; un échec de construction est retenté au calcul suivant |
| F118 | `systems/history_move.rs:57` | corrigé | 174c8fb8 | la copie native renvoie la taille de la destination (handle créé), plus celle de la source |

### Modèles, utilitaires et noyau

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F124 | `commands/sidebar.rs:25-31, :45 et :120` | Linux |  | lien symbolique vers un dossier dans l'arbre (Unix) |
| F125 | `commands/sidebar.rs:63-79` | corrigé (en partie) | 592da38b | repli et nouvelle demande l'annulaient déjà ; résultat écarté aussi si l'affichage des fichiers cachés ou système a changé ; lecteur déconnecté pendant le parcours non traité |
| F126 | `commands/system.rs:9-19` | corrigé | 4db5c2eb | commande sans appelant supprimée (l'interface passe par `Platform::open_item`) |
| F127 | `commands/system.rs:33-53` | corrigé (en partie) | 4db5c2eb | la purge tente icons et thumbnails puis rend la première erreur ; reste Linux : pas de pause des écritures de miniatures pendant la purge |
| F130 | `models/error.rs:131-143 et autres` | reporté |  | refonte : remplacer les marqueurs texte par des erreurs typées de bout en bout (workers compris) |
| F131 | `models/history.rs:142-148 et :170-183` | corrigé | 64e16632 | entrées expirées nettoyées sur un thread dédié (repli sur place si le thread ne démarre pas) ; synchrone dans les tests |
| F136 | `utils/office_document.rs:109-139` | corrigé | 8a43400a | ligne ou rangée coupée à la place restante (frontière de caractère) ; paragraphe en cours borné au budget ; workers à reconstruire |
| F138 | `utils/office_document.rs:252` | corrigé | 81a29de2 | `<w:tab>` avec position (définition de taquet) ignoré dans l'aperçu et la recherche ; test |
| F140 | `utils/office_document.rs:248-274` | corrigé | d2cf6140 | contenu de `mc:Fallback` ignoré (aperçu et recherche) ; test |
| F141 | `utils/office_document.rs:50-57 et office_document/sheets.rs:76-81` | corrigé | 6d307926 | aperçu Office : 256 Mio décompressés et 4096 parties au plus par document (la recherche avait déjà un budget cumulé) ; workers à reconstruire |
| F142 | `utils/office_preview.rs:21-24 et :41-47` | corrigé | 5f3d2d77 | jusqu'à 64 Kio lus (plus une seule lecture de 8 Ko) ; nom de balise entier comparé (w:pPr n'est plus un paragraphe, a:p et text:h séparent les lignes) ; entités décodées ; test ; workers à reconstruire |
| F143 | `utils/office_document/search.rs:67-71` | corrigé (en partie) | 413ecc3f | recherche Word : en-têtes, pieds de page, notes de bas de page et de fin, commentaires ; reste : graphiques et commentaires Excel |
| F144 | `utils/mod.rs:109-110` | corrigé | 90dd560e | comparaison par caractère entier, en minuscule, comme l'interface ; test (accents, cyrillique) |
| F146 | `utils/image_preview_decode.rs:76-81` | corrigé (en partie) | dad379b4 | l'analyse de taille ne résout plus aucune image liée ou intégrée ; le rendu GPUI garde ses propres options (seconde analyse) |
| F148 | `chemins non UTF-8 : utils/path_text.rs:29, commands/sidebar.rs:35-142, ui/app/file_chooser/navbar.rs:150 et browse.rs:140, commands/ops.rs:332-335 et :515-517, platform/linux_impl.rs:588 et :825, platform/linux_preview.rs:84, platform/windows_pdf_session.rs:42` | reporté |  | refonte transversale : transporter `PathBuf`/`OsString` de bout en bout (sidebar, sélecteur, ops, aperçus) ; surtout Linux, sous Windows limité aux noms à surrogate isolé |

### Interface : application, fenêtres et composants

| ID | Emplacement | État | Commits | Note |
| --- | --- | --- | --- | --- |
| F149 | `ui/app/archive_password.rs:184` | corrigé | acea9f31 | trop long : le champ tremble et affiche « mot de passe trop long » ; vide : bouton Déverrouiller désactivé, Entrée sans effet (comportement voulu) |
| F150 | `ui/app/archive_password.rs:104` | corrigé | 8475e952 | refus « mauvais mot de passe » ignoré pour l'oubli tant qu'un nouveau mot de passe de la même archive est vérifié |
| F151 | `ui/app/archive_password.rs:176 et :187` | corrigé | 8475e952 | vérification identifiée ; annulée pendant la vérification, le mot de passe mémorisé est oublié (sauf vérification plus récente de la même archive) |
| F152 | `ui/app/archive_password.rs:143` | corrigé (en partie) | 4d0f6231 | copies tenues par Oxyde (invite et compression) effacées à chaque remplacement ; reste : le contenu du champ texte et son historique d'annulation |
| F153 | `ui/app/advanced_search/validation.rs:75` | corrigé | 2d1c5506 | volet fermé : recherche dans le volet actif ; même recherche : relancée comme une actualisation |
| F154 | `ui/app/advanced_search/validation.rs:26` | corrigé | 2d1c5506 | minuit sauté : début du jour une heure plus tard ; avant 1970 : borne ramenée à 0 |
| F155 | `ui/app/advanced_search/calendar.rs:36, :19 et :29-32` | corrigé | 5e10037e | choix d'une date efface l'erreur ; valeur tronquée à l'ouverture ; Page préc./suiv. revient au jour choisi (31 après février) |
| F156 | `ui/app/advanced_search/help.rs:171` | corrigé | 316264e2 | échec d'ouverture signalé par une notification d'erreur dans la fenêtre principale |
| F157 | `ui/app/clipboard.rs:18` | corrigé | 3dd2a8cb | nombre annoncé = nombre d'éléments ; l'ordre de copie suit déjà l'affichage (`actionable_selection` parcourt la liste filtrée) |
| F158 | `ui/app/context_menus.rs:305 et :331` | corrigé (en partie) | 61945ba7 | élément dont la date de création précède la commande « Nouveau » écarté (téléchargement terminé, fichier déplacé) ; reste : un fichier créé par un autre programme dans la même seconde |
| F159 | `ui/app/context_menus.rs:206` | corrigé (en partie) | 19827850 | au bout de 10 s, « Le menu système ne répond pas » (5 langues) ; 4 chargements bloqués au plus, les suivants échouent aussitôt ; l'appel Shell bloqué ne peut être interrompu |
| F160 | `ui/app/context_menu_render/native.rs:229` | corrigé | ebc8f2cf | sous-menu ouvert reconnu par son contenu entier (`WinMenuItem: PartialEq`), plus par l'identifiant du premier élément |
| F162 | `ui/app/drag.rs:548-559` | corrigé | b66f2d88 | la minuterie appartient à `DragTabSwitch` : quitter l'onglet l'annule, un nouveau survol repart de 600 ms |
| F163 | `ui/app/drag.rs:51-55` | corrigé | 79b93258 | lecteur ou partage comparé sans préfixe étendu ni casse ; volumes montés dans un dossier relevés en arrière-plan (`mount_points` Windows) ; tests ; à tester : glisser vers un disque monté dans un dossier (déplacer → copier) |
| F165 | `ui/app/duplicates/lifecycle.rs:126 et :129-130` | corrigé | 57ab8d8e | disque le plus profond contenant le dossier (`path_within`) ; racine de lecteur déjà listée seulement cochée ; `file_chooser/sources.rs` n'existe plus |
| F168 | `ui/app/drive_watchers.rs:468-517` | corrigé | 4b1ffde3 | échec mémorisé : nouvel essai après 60 s ou au changement de la liste des lecteurs, avertissement journalisé une fois |
| F169 | `ui/app/drive_watchers.rs:170-176` | corrigé | 9714a19f | volume monté ouvert dans le volet qui l'a demandé, ou le volet actif s'il a été fermé |
| F170 | `ui/app/disk_images.rs:53-80 et :108-111` | corrigé | 4466ff1f | une seule sonde par image en cours ; montage refusé pendant un autre changement de volume : notification « déjà en cours » (5 langues) |
| F173 | `ui/app/file_chooser/browse.rs:68 et :70` | corrigé | 968960c3 | la navigation rend le clavier à la liste quand elle ferme le champ d'adresse ou le renommage ; demande de menu de chevron en cours oubliée |
| F174 | `ui/app/file_chooser/location.rs:39` | corrigé | 22d860d4 | « D: » seul (ou « D:dossier ») part de la racine du lecteur ; test |
| F175 | `ui/app/file_chooser.rs:368 et :387` | corrigé | 2cb1fc11 | dossier initial illisible (clé débranchée) : dossier suivant des candidats (dernier choisi, défaut, dossier personnel, racine), sans historique ; demande retirée : menus fermés et réglages mémorisés |
| F176 | `ui/app/file_chooser/create.rs:34, :53 et :63` | corrigé | b95cefc9 | 10 000 noms essayés au plus ; création historisée même si le sélecteur s'est fermé ; dossier ajouté à la liste tout de suite, sans attendre un nouveau listage (frappes gardées) |
| F177 | `ui/app/file_chooser/band.rs:49` | corrigé | 4d3c572b | rectangle abandonné à la navigation ; positions de départ au-delà d'une liste raccourcie ignorées |
| F178 | `ui/app/file_chooser/overwrite.rs:36` | corrigé | 02d9ed27 | invite ouverte après la fermeture du sélecteur refermée aussitôt ; une invite précédente est fermée avant d'adopter la nouvelle |
| F179 | `ui/app/file_chooser/save.rs:222-229 et :92-94` | corrigé | 7e1b2589 | droit d'écriture réel : ouverture en écriture des données sans rien écrire (Windows), `access(W_OK)` (Linux) ; résultat ignoré si le dossier a changé ; test |
| F180 | `ui/app/file_operations.rs:268-278, :176-183 et :357` | corrigé | 669be063 | avis d'échec mis en file derrière celui affiché (8 au plus), avec leur reprise ; lien brisé gardé dans la reprise (`symlink_metadata`) |
| F181 | `ui/app/file_chooser/setting.rs:13-18 et :46-49, ui/app/folder_handler.rs:13-18 et :54-57` | corrigé | 88050434 | compteur de changements : une lecture commencée avant un changement est ignorée ; échec interrompu : valeur relue au lieu de rétablie |
| F182 | `ui/app/file_chooser/sidebar.rs:52-63` | corrigé | 25f840da | la fenêtre principale reçoit le volume monté même si le sélecteur s'est fermé |
| F183 | `ui/app/file_chooser/tree.rs:93-103` | corrigé | 08c7c73c | un dépliage garde un listage plus récent (`or_insert`) ; le listage du dossier ouvert remplace un dépliage en cours ; copies initiales du suivi cédant aux listages récents |
| F184 | `ui/app/instance.rs:13-15 et :25-31` | corrigé | 41f506d1 | comparaison par `paths_match` (préfixe étendu, séparateurs, casse) ; une racine sans parent s'ouvre elle-même ; chemins relatifs déjà rendus absolus par la plateforme ; test Linux à lancer |
| F188 | `ui/app/navigation.rs:54-56` | corrigé | 3552f865 | niveau en cache sans le dossier suivant relu (fenêtre principale et sélecteur) |
| F189 | `ui/app/navigation/breadcrumb.rs:129-154, :25-31 et :200-211` | corrigé | 38d65b2f | nom relatif résolu depuis le dossier affiché (sans . ni ..), « C: » seul vers sa racine ; vérification bornée à 10 s et une seule à la fois ; échec du chevron signalé par une notification ; comparaison de lecteur par `paths_match` ; tests |
| F190 | `ui/app/navigation.rs:393-416` | corrigé | 8570cfff | fermer des onglets qui change l'onglet actif du volet termine l'édition du chemin |
| F191 | `ui/app/layout/tabs.rs:645-651` | corrigé | c8812d96 | dossiers ajoutés du dernier au premier, chacun après l'onglet actif : ordre gardé |
| F192 | `ui/app/open_with.rs:229-235` | corrigé | e8a36da4 | Entrée sans application choisie (chargement en cours) laisse l'invite ouverte |
| F193 | `ui/app/pinned_order.rs:76` | corrigé | b5e47f92 | dossier enregistré mais absent gardé après celui qu'il suivait (64 au plus) ; test |
| F194 | `ui/app/network.rs:466-468` | corrigé | 6d45ad67 | échec du relâchement journalisé, la déconnexion continue (comme l'éjection) |
| F195 | `ui/app/navigation.rs:700-716` | corrigé | 896f2062 | au démarrage, seul l'onglet affiché de chaque volet est listé ; les autres au premier choix (déjà géré par `switch_tab`) |
| F196 | `ui/app/properties/interactions.rs:148-173` | corrigé (en partie) | 73a1c5bb | fenêtre principale rafraîchie après l'écriture même si Propriétés s'est fermée ; l'écriture (un appel COM bref) reste non annulable |
| F197 | `ui/app/properties/selection.rs:22-30` | corrigé | 3b88d752 | élément illisible compté à part (ligne « Illisibles », 5 langues), échec seulement si aucun n'est lisible ; droits non modifiables dans ce cas ; annulation silencieuse ; test |
| F198 | `ui/app/properties/window.rs:30-38` | corrigé | 7ca9870c | Propriétés déjà ouvertes pour les mêmes éléments ramenées au premier plan |
| F200 | `ui/app/properties/interactions.rs:111` | corrigé | d52b0615 | cible vide acceptée si elle l'était déjà (raccourci Shell) ; le moteur ne réécrit pas une cible inchangée |
| F201 | `ui/app/rename.rs:252-255` | corrigé | ebfe8b0c | mot de passe porté par l'opération Compresser, retenu sous le nom publié après création ; une archive existante du même nom garde le sien ; la compression n'utilise plus le registre ; `remember_new` retiré ; workers à reconstruire |
| F202 | `ui/app/properties/permissions/save.rs:97-98 et properties/dialog.rs:52` | corrigé | 3b5a519f | « modifié depuis l'ouverture » : droits relus, le prochain OK s'applique ; message effacé à la saisie ou au changement d'onglet ; Linux à tester |
| F203 | `ui/app/selection.rs:55 et :354` | corrigé | 047b5295 | curseur dans un groupe replié : fichier affiché le plus proche dans le sens de la flèche ; test |
| F207 | `ui/app/theme_window/draft.rs:251 et :233, theme_window.rs:17` | corrigé | 9e2eb42d | confirmation avant suppression d'un thème et avant d'abandonner un brouillon modifié pour un autre thème (5 langues) ; aperçu retiré quand la fenêtre principale ferme la fenêtre Thèmes |
| F209 | `ui/app/trash_settings/linux.rs:157 et :171` | Linux | | |
| F210 | `ui/app/trash_settings/settings.rs:82, :168, :229 et :281` | corrigé | 9fff2490 | réglages lisibles affichés avec la première erreur ; un enregistrement partiel garde ce qui a été écrit ; valeur vidée vérifiée au changement de lecteur ; usage relu même après un échec de vidage |
| F214 | `ui/app/viewer.rs:196 et viewer/markdown/loading.rs:13` | corrigé | 2ace4aa0 | suppressions en cours connues d'un Viewer créé ensuite ; lecture du .md et des images inscrite au registre de suppression |
| F215 | `ui/app/viewer/fullscreen_controls.rs:154 et :207` | corrigé | 1b713a39 | attente levée seulement après le handle ; échec journalisé une fois, sans nouvelle tentative jusqu'à la fin du plein écran |
| F216 | `ui/app/viewer/markdown.rs:95` | corrigé | afdf5bf9 | au-delà de 32 images, la plus éloignée de la vue est libérée et redemandée si elle revient |
| F217 | `ui/app/viewer/markdown/parse.rs:112 et :51-66` | corrigé | b1bfb9bd | coloration bornée à 1 Mio de code par document (déterministe) ; images avec composant parent ou nom de périphérique Windows refusées ; test |
| F218 | `ui/app/viewer/markdown/selection.rs:134-137 et viewer/text/selection.rs:141-143` | corrigé | 2c2b161c | sélection de tout le Markdown étendue aux blocs chargés ensuite ; copie de texte au-delà de 64 Mo refusée avec notification (5 langues) |
| F219 | `ui/app/viewer/media.rs:238-244` | corrigé | b3fcbcc2 | Lecture après la fin repart du début et joue, lecture automatique ou non |
| F220 | `ui/app/viewer/media_overlay.rs:409-479` | corrigé | 2a3bdab0 | si aucune popup ne s'ouvre : erreur journalisée une fois, pas de nouvelle tentative avant la fermeture du calque |
| F221 | `ui/app/viewer/tree.rs:230-243, :54, tree/loading.rs:75 et :155-157` | corrigé | d0e8a47f | niveau en échec relu à la réouverture ; sommes saturantes ; comptage retiré si son thread ne démarre pas ; tests |
| F223 | `ui/components/conflict_dialog/advanced.rs:135-137 et frame.rs:187-189` | corrigé | 31b3a4a6 | retrait plafonné à 6 niveaux ; Haut, Bas, Début, Fin parcourent la liste avancée (défilée jusqu'à la ligne), Gauche et Droite plient le dossier choisi ; hors champ de recherche ; tests |
| F224 | `ui/components/file_list/icons.rs:281` | corrigé | dca83a30 | « .key » classé certificat (clé privée), retiré des présentations |
| F226 | `ui/components/file_list/presentation.rs:72 et :174` | corrigé | c78eae78 | case de sélection, bouton des tailles et boutons de la barre d'outils passent par `activate_pane` (volet enregistré) |
| F227 | `ui/components/file_list/details.rs:329, grid.rs:433` | corrigé | ffed80b2 | dépôt refusé (sans surlignage) sur la Corbeille, le Réseau et les résultats de recherche, glisser interne ou externe ; garde aussi dans `execute_file_drop` |
| F228 | `ui/components/file_list/icons.rs:411` | corrigé | dae229fc | icônes propres à un fichier limitées à 1024 (la plus ancienne libérée, texture comprise) ; échec redemandé après 30 s quand l'icône est de nouveau affichée |
| F233 | `ui/components/input.rs:129-132` | à examiner | | |
| F235 | `ui/components/operations/items.rs:244 et :267` | à examiner | | |
| F236 | `ui/components/operations/view.rs:163` | à examiner | | |
| F237 | `ui/components/operations/items/icons.rs:14` | à examiner | | |
| F238 | `ui/components/popup/hover.rs:38` | à examiner | | |
| F239 | `ui/components/operations/list.rs:255` | à examiner | | |
| F241 | `ui/components/sidebar.rs:42` | à examiner | | |
| F242 | `ui/components/sidebar/rail.rs:168 et :226-232` | à examiner | | |
| F243 | `ui/components/sidebar/pinned.rs:62 et :143` | à examiner | | |
| F244 | `ui/components/tabs/scroll.rs:44` | à examiner | | |
| F245 | `ui/components/settings/fallback.rs:18` | à examiner | | |
| F246 | `ui/components/top_bar/breadcrumb_menu.rs:57` | à examiner | | |
| F248 | `ui/components/tooltip/hover.rs:176-186` | à examiner | | |
| F249 | `ui/components/tooltip.rs:28-32` | à examiner | | |
| F250 | `ui/components/tooltip/hover/native_window.rs:534-538` | à examiner | | |
| F251 | `ui/audio_covers.rs:25-29` | à examiner | | |
| F252 | `ui/date_groups/grouping.rs:178` | à examiner | | |
| F253 | `ui/date_groups/grouping.rs:121-123 et :141-147` | à examiner | | |
| F254 | `ui/format.rs:69-85, :155-160 et :12-25` | à examiner | | |
| F255 | `ui/errors.rs:134-142 et :48, :91` | à examiner | | |
| F257 | `ui/elements/button.rs:75-93` | à examiner | | |
| F258 | `ui/file_types.rs:6-273` | à examiner | | |
| F259 | `ui/i18n.rs:1797` | à examiner | | |
| F260 | `ui/paths.rs:11` | à examiner | | |
| F261 | `ui/style.rs:98` | à examiner | | |
| F262 | `ui/workspace/filters.rs:335 et :151` | à examiner | | |
| F263 | `ui/workspace/snapshots.rs:22, :82 et :52` | à examiner | | |
| F264 | `ui/workspace/operations.rs:6 et :64` | à examiner | | |
