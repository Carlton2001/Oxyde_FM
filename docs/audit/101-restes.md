# Audit : points restant à corriger

État au 2026-11-19, après la reprise des reportés (M62, F111, M29, M63/M64 corrigés). Source : tableau de suivi de `100-plan-remediation.md` et synthèse `99-synthese.md`. Chaque point donne d'abord ce que voit l'utilisateur, puis le détail technique du constat.

## Vue d'ensemble

| Catégorie | Points | Verdict |
| --- | --- | --- |
| 1. Corrigeables dès à présent | M96, M97, faibles Windows F32 à F44 | corrigés le 2026-11-19 (lignes « catégorie 1 » du suivi) |
| 2. Gros chantiers | M17, M67, reste de M43, M31, M32 | faisables, mais refonte |
| 3. Impossibles ou choix volontaires | F26, F72, F119, WM1 (reste), M14, M88 (icônes), F77, ordre de deux lancements | à laisser, sauf décision contraire |
| 4. Besoin d'une autre machine | WM6, M57/M58, E9/E10 sous Linux, faibles Linux F45 à F69 | Win10 LTSC ou session Linux |
| 5. Faibles non examinés | archives, noyau et interface (voir annexe) | à traiter petit à petit |
| 6. Optimisations (série O) | 86 constats (voir annexe) | à mesurer avant tout changement |

## 1. Corrigeables dès à présent

**Corrigés le 2026-11-19** sur `audit-fixes` (`7d56f6b6` à `daad4df9`), détail dans les lignes « catégorie 1 » du tableau de suivi de `100-plan-remediation.md`.

### M96 : miniatures transparentes qui changent d'aspect (moyen)

**Pour l'utilisateur :** une image PNG ou WebP transparente s'affiche d'abord avec sa transparence, puis avec un fond noir ou blanc au passage suivant, une fois la miniature mise en cache.

**Détail :** platform/thumbnails.rs:173 (+ windows_thumbnail_bitmap.rs:129, WIN-WORKERS-02) (PLATFORM-04) : le cache JPEG supprime le canal alpha ; la miniature diffère entre première génération et relecture.

**Note :** ce constat moyen n'a jamais figuré dans le tableau de suivi (oubli du lot 7, qui couvrait M93 à M99).

### M97 : pages de PDF décalées (moyen)

**Pour l'utilisateur :** dans un PDF dont une page a des dimensions invalides, les pages suivantes s'affichent avec de mauvaises proportions et les dernières deviennent inaccessibles.

**Détail :** platform/thumbnails.rs:734 (PLATFORM-04) : l'analyse des tailles de pages PDF écarte les lignes invalides au lieu de les remplacer. Scénario : MediaBox dégénérée sur une page.

**Note :** oublié du tableau de suivi, comme M96.

### Faibles Windows F32 à F44

Douze constats faibles sur la corbeille, les workers et le bac à sable, jamais traités (le lot 6 s'est arrêté à F34). Le détail de chacun est dans l'annexe, section « Plateforme Windows : fichiers, lecteurs, corbeille, workers ». Exemples :

- F32 : un seul élément illisible dans la corbeille la fait relire trois fois.
- F33 : une jonction recyclée vers un partage hors ligne bloque l'affichage de la corbeille.
- F44 : si Oxyde est tué au mauvais moment, des profils AppContainer `Oxyde.Archive.<uuid>` restent dans Windows.

## 2. Gros chantiers

### M17 : copie recommencée après un mot de passe d'archive (moyen)

**Pour l'utilisateur :** on copie plusieurs fichiers dont un dans une archive chiffrée. Après la saisie du mot de passe, toute la copie recommence : doublons « (2) », échecs, et les questions de conflit reviennent.

**Détail :** ui/app/archive_password.rs:282 (et :287-311) (UI-APP-01) : la reprise après mot de passe remet en file l'opération entière avec des réglages neufs.

**Obstacle :** le moteur de copie, de déplacement et de suppression doit dire quelles sources sont terminées. Une heuristique risquerait d'omettre des sources sans le dire.

### M67 : panneau des opérations lent sur les grosses opérations (moyen)

**Pour l'utilisateur :** pendant la copie ou la suppression de dizaines de milliers d'éléments, la fenêtre des opérations ouverte ralentit l'interface et l'opération elle-même.

**Détail :** ui/components/operations/view.rs:161 (+ systems/file_ops.rs:494, operations/items/table.rs:235 et :194, operations/items.rs:225-230 et :263) (UI-COMP-04, FILEOPS-01, UI-COMP-03) : chaque rendu demande un instantané complet de l'opération (clone des sources et de la table des issues) sous le verrou que les workers prennent pour publier leur progression, puis reclone la table et la parcourt neuf fois.

**Obstacle :** refonte du panneau (instantanés incrémentaux par révision).

### M43 (reste) : création de dossier dans une grosse archive (moyen)

**Pour l'utilisateur :** créer un dossier dans une archive de plusieurs Go peut durer longtemps, sans progression ni bouton Annuler.

**Détail :** systems/archive_workspace.rs:448 (ARCHIVES-03) : la création de dossier prend le verrou de mutation sans point d'annulation ni progression. Déjà fait (2b6aa0ae) : une seule extraction et une seule reconstruction quel que soit le nombre de noms pris.

**Obstacle :** un nouveau genre d'opération dans le panneau des opérations.

### M31 (reste) : sélection recopiée à chaque image (moyen)

**Pour l'utilisateur :** après Ctrl+A sur 100 000 fichiers, le survol de la liste rame.

**Détail :** ui/components/file_list/details_row.rs:204 et :211, grid.rs:188 (+ details.rs:98 et :207, grid.rs:35 et :156) (UI-COMP-02) : chaque ligne sélectionnée visible recopie toute la sélection. Déjà fait (cd347c81) : sélection de glisser partagée. Reste : la copie de `selected_paths` par rendu.

**Obstacle :** type partagé par environ 90 endroits du code.

### M32 (reste) : filtres lents sur les très gros dossiers (moyen)

**Pour l'utilisateur :** le filtre Type sur plusieurs centaines de milliers de fichiers gèle l'interface à chaque case cochée.

**Détail :** ui/components/file_list/filters.rs:38 (UI-COMP-02) : le calcul des valeurs de filtre parcourt la liste sur le thread UI, et son cache est invalidé à chaque révision. Déjà fait (e7f47d6c) : un seul parcours au lieu de deux.

**Obstacle :** passer le calcul hors du thread UI, avec un résultat asynchrone dans le menu des filtres.

## 3. Impossibles ou choix volontaires

| Point | Pour l'utilisateur | Raison de le laisser |
| --- | --- | --- |
| F26 | si Windows refuse d'enregistrer la cible de dépôt, plus aucun glisser-déposer n'est accepté | Windows n'a pas de remplacement atomique ; déjà atténué (aa79fa36 : nouvelle tentative, message clair) |
| F72 (reste) | un ZIP chiffré à l'ancienne (ZipCrypto) et abîmé redemande le mot de passe | le format ne distingue pas un membre abîmé d'un mauvais mot de passe ; le cas de plusieurs membres est corrigé (7a996e47) |
| F119 (reste) | déplacer un lien entre volumes avec « Remplacer » écrase la destination avant la vérification | l'utilisateur a demandé le remplacement ; l'erreur n'est plus masquée (40b32267) |
| WM1 (reste) | les fichiers OneDrive, compressés par Windows (WOF/CompactOS) ou dédoublonnés restent refusés | les ouvrir comme liens lirait un fichier WOF vide ; risque de perte de données. Disques montés dans un dossier, profils en jonction et Scoop corrigés (45d0f1a8, ca25109e) |
| M14 | en mode avancé du dialogue de conflits, le premier choix applique l'action opposée aux autres conflits | comportement voulu et testé ; à changer seulement sur décision |
| M88 (reste) | en défilant vite, des icônes périmées peuvent retarder les bonnes | les assets GPUI n'ont pas de notion de visibilité ; requêtes courtes ; le Viewer est déjà couvert |
| F77 | les copies modifiées de fichiers ouverts depuis une archive ne sont jamais supprimées du cache, qui peut finir plein | choix de sécurité : ne jamais effacer une modification de l'utilisateur |
| Ordre de deux lancements | deux ouvertures lancées à quelques ms d'écart peuvent arriver dans l'ordre inverse | voulu : un partage bloqué ne retient pas les lancements suivants |

Détails techniques :

- **F26** platform/windows_file_drag/target.rs:194 (WIN-FS-01) : la cible est révoquée avant l'enregistrement de la nouvelle.
- **F72** commands/archive_io_codec.rs:307 (ARCHIVES-01) : toute erreur de somme de contrôle ZipCrypto devient « mauvais mot de passe ».
- **F119** systems/link_moves.rs:17, :34 et :40 (FILEOPS-03) : avec remplacement, la destination est écrasée avant la vérification.
- **WM1** platform/windows_no_reparse.rs:353 et :310, windows_file_copy.rs:118, windows_file_tree.rs:138 et :249, windows_cache_proof/key.rs:106, windows_worker_sandbox.rs:97 et :107, windows_preview_cache.rs:11, utils/archive_security.rs:79 : refus de tout objet portant l'attribut de reparse, quelle que soit son étiquette.
- **M14** ui/components/conflict_dialog/rules.rs:12-29 (+ frame.rs:54-57) (UI-COMP-01) : le premier choix explicite applique l'action opposée à tous les conflits non décidés, et Appliquer est actif aussitôt.
- **M88** platform/windows_preview_worker/icon_session.rs:117 et image_session.rs:153 (WIN-WORKERS-02) : requêtes en file sans annulation ni contrôle d'actualité.
- **F77** systems/archive_open_cache.rs:516 (ARCHIVES-02) : une copie modifiée n'est jamais évincée et compte dans le quota (512 entrées, 1 Gio).

## 4. Besoin d'une autre machine

| Point | Pour l'utilisateur | Ce qu'il faut |
| --- | --- | --- |
| WM6 | sous Windows 10 LTSC, les entrées « Linux » de la barre latérale mènent à des chemins introuvables | corrigé au lot 9 (1c16de6a) ; à tester sur la machine Win10 LTSC |
| M57, M58 | sous Linux, déplacer beaucoup de fichiers vers une archive, ou annuler une très grosse copie, peut échouer faute de descripteurs | atténués par E6 (374c5078) ; la structure ne change pas sans session Linux |
| E9, E10 | sous X11, répétition de touche ; saisie IBus/Fcitx | tests sur une vraie machine Linux (`tests-linux.md`) |
| F45 à F69 | 25 faibles Linux (presse-papiers, corbeille, GIO, montages) | session Linux ou WSL ; détail dans l'annexe |

Détails techniques :

- **WM6** platform/windows_wsl.rs:12 (+ platform/mod.rs:323) (WIN-FS-02, PLATFORM-04) : distributions exposées seulement sous \\wsl.localhost.
- **M57** systems/file_ops/worker.rs:263 (FILEOPS-01) : déplacement vers ou depuis une archive, environ 2 x (profondeur + 2) descripteurs par fichier jusqu'au commit.
- **M58** systems/history_file_undo.rs:484-511 (FILEOPS-02) : chaque sortie à écarter reçoit son dossier caché .oxyde-undo-* et environ 2 handles conservés.

## 5. Faibles non examinés

Environ 140 constats faibles, ni examinés ni corrigés : archives et opérations (F70 à F118), modèles et noyau (F124 à F148), interface (F149 à F264). Ils se traitent petit à petit, en vérifiant chacun sur le code actuel. Détail dans l'annexe.

## 6. Optimisations (série O)

86 constats. Chacun demande une mesure ou un changement de protocole entre processus : on ne les corrige pas sans gain prouvé. Les optimisations Linux (O29, O30, O40 à O42) attendent une session Linux. Détail dans l'annexe.


## Annexe : détail des faibles et des optimisations restants

Texte repris de la synthèse (`99-synthese.md`), groupé par section. Aucun de ces constats n'a été examiné de nouveau ; vérifier sur le code actuel avant de corriger.

### Faibles : Plateforme Windows : fichiers, lecteurs, corbeille, workers (12, corrigés le 2026-11-19)

- **F32** platform/windows_trash/listing.rs:43 (WIN-FS-02) : un seul $I absent ou illisible fait échouer l'énumération native. Scénario : élément purgé pendant l'énumération : la corbeille est parcourue trois fois.
- **F33** platform/windows_trash/listing.rs:73 (WIN-FS-02) : taille et date lues en suivant les liens. Scénario : jonction recyclée vers un partage hors ligne : énumération bloquée jusqu'au délai réseau.
- **F35** platform/windows_archive_io_worker.rs:284 (+ platform/linux_archive_io.rs:198, PLATFORM-01) (WIN-WORKERS-01) : si l'accusé de pause arrive plus d'une seconde après la demande, la pause est comptée dans le délai de l'opération. Scénario : pause laissée plusieurs minutes pendant un gros bloc LZMA2 : échec sur la limite de durée.
- **F36** platform/windows_archive_io_worker.rs:232 (WIN-WORKERS-01) : après Finished, fermeture et sortie en moins d'une seconde exigées. Scénario : machine saturée ou worker 7z qui libère 2 Gio : une compression terminée est signalée en échec.
- **F37** platform/windows_archive_io_worker.rs:85 (+ windows_archive_worker.rs:19 et :77, windows_office_search.rs:26) (WIN-WORKERS-01) : un emplacement d'admission empoisonné renvoie « invalide » pour toujours. Scénario : panique dans un handler : toutes les opérations d'archive de la catégorie échouent jusqu'au redémarrage.
- **F38** platform/windows_icon_pixels.rs:96 (+ platform/icons.rs:58, PLATFORM-01) (WIN-WORKERS-01) : la clé du cache d'icônes ne dépend que du chemin et de la taille (casse non normalisée). Scénario : .exe, .lnk ou .ico remplacé au même chemin : ancienne icône conservée, même après redémarrage.
- **F39** platform/windows_pdf_thumbnail.rs:130 (WIN-WORKERS-01) : sizes_of appelle GetPage pour chaque page (jusqu'à 100 000) avant la première réponse. Scénario : PDF de milliers de pages sur disque lent : ouverture au-delà de 15 s, Viewer qui refuse.
- **F40** platform/windows_preview_quota.rs:274 (WIN-WORKERS-02) : le bail exclusif tenu pendant scan et publication fait abandonner un écrivain concurrent sans nouvelle tentative. Scénario : certaines miniatures jamais mises en cache, régénérées à chaque visite.
- **F41** platform/windows_preview_worker.rs:519 (WIN-WORKERS-02) : l'horloge de progression démarre avant l'admission au budget (jusqu'à 30 s). Scénario : processus vidéo lancé puis tué aussitôt, erreur « no frame for 15 seconds ».
- **F42** platform/windows_preview_worker.rs:180 (WIN-WORKERS-02) : le repli PDF pour chemin non Unicode ou trop long lance un worker non LPAC sans prendre l'emplacement. Scénario : dossier de PDF aux noms longs : plusieurs analyseurs en parallèle.
- **F43** platform/windows_preview_worker/image_session.rs:129 (WIN-WORKERS-02) : release_viewer utilise try_send, le message est perdu si le canal est plein. Scénario : l'enveloppe de 1 Gio reste réservée jusqu'à 60 s et bloque l'admission vidéo.
- **F44** platform/windows_worker_sandbox.rs:45 (WIN-WORKERS-02) : profil AppContainer supprimé seulement après la préparation (environ 1,4 s), sans nettoyage au démarrage. Scénario : Oxyde tué dans cette fenêtre : profils Oxyde.Archive.<uuid> orphelins.

### Faibles : Plateforme Linux et services communs (25)

- **F45** platform/linux_file_clipboard.rs:82 (PLATFORM-01) : has_file_format ne connaît que la sélection d'Oxyde. Scénario : fichiers copiés dans Nautilus : le bouton Coller reste désactivé alors que Ctrl+V fonctionne.
- **F46** platform/linux_file_clipboard/wayland.rs:42 (PLATFORM-01) : wl-paste sans délai maximal. Scénario : propriétaire du presse-papiers figé : le thread de collage est bloqué.
- **F47** platform/linux_download.rs:38 (PLATFORM-01) : avec wget, redirection HTTP suivie, ~/.wgetrc lu, aucune durée totale. Scénario : machine sans curl : manifeste de mise à jour altéré (la signature limite l'impact).
- **F48** platform/linux_file_tree.rs:88 et :57 (PLATFORM-01) : deux descripteurs par niveau de pile sans relèvement de RLIMIT_NOFILE, et suppression pendant readdir sur FUSE ou réseau. Scénario : arborescence de plusieurs centaines de niveaux : EMFILE et arbre à moitié supprimé ; sur gvfsd-fuse ou CIFS : entrées sautées et rmdir final en ENOTEMPTY.
- **F49** platform/linux_gio.rs:384, :391 et :486 (PLATFORM-01) : montage, énumération et démontage synchrones sans annulation ni délai. Scénario : serveur SMB injoignable : un thread occupé tant que GVfs attend.
- **F50** platform/linux_gio/applications.rs:186 (PLATFORM-01) : l'association par défaut est enregistrée avant le lancement. Scénario : « Toujours utiliser » avec une application qui échoue : elle reste l'application par défaut.
- **F51** platform/linux_extended_attributes.rs:24 (PLATFORM-01) : boucle ERANGE sans plafond ni annulation. Scénario : FUSE bogué : copie qui tourne indéfiniment.
- **F52** platform/hardware.rs:46, :80 et :146 (PLATFORM-01) : lettre de lecteur prise par tranche d'octets. Scénario : chemin « é:\… » : coupe au milieu d'un caractère UTF-8, panique du thread.
- **F53** platform/hardware.rs:79 (PLATFORM-01) : seule la lettre de lecteur ou l'hôte UNC identifie le disque. Scénario : volume monté dans un dossier ou chemin \\?\Volume{...} : limitation d'E/S par disque faussée.
- **F54** platform/linux_permissions.rs:103 (PLATFORM-02) : verrou global du cache des comptes tenu pendant l'exécution de getent. Scénario : annuaire LDAP ou SSSD injoignable : toutes les fenêtres Propriétés attendent.
- **F55** platform/linux_portal.rs:239 et :270 (PLATFORM-02) : sans configuration système, activer puis désactiver laisse un fichier de portail vide qui masque la future configuration ; chaque changement redémarre xdg-desktop-portal. Scénario : l'option basculée pendant un partage d'écran Wayland ou un dialogue de portail d'une autre application les interrompt.
- **F56** platform/linux_portal/chooser.rs:286 (PLATFORM-02) : lecture disque synchrone sur l'exécuteur du bus. Scénario : dossier personnel NFS lent : les autres appels de la connexion sont bloqués.
- **F57** platform/linux_media/worker.rs:114 (PLATFORM-02) : la demande de seek est marquée appliquée avant de savoir si elle a réussi. Scénario : flux en mise en tampon : échec silencieux, barre de progression qui revient en arrière.
- **F58** platform/linux_sandbox/seccomp.rs:38 (PLATFORM-03) : le filtre refuse unshare et setns mais pas clone ou clone3 avec CLONE_NEWUSER. Scénario : repli Landlock (sans bubblewrap) : un décodeur compromis peut créer un espace de noms utilisateur.
- **F59** platform/linux_volumes/holders.rs:35 (PLATFORM-03) : chemin de /proc/<pid>/maps tronqué au premier espace. Scénario : clé montée sous « Ma Clé » retenue par un fichier mappé : la boîte « volume occupé » ne nomme personne.
- **F60** platform/linux_trash.rs:72 (PLATFORM-03) : la restauration refuse EXDEV au lieu de copier puis supprimer. Scénario : fichier d'une clé FAT placé dans la corbeille personnelle : listé mais jamais restaurable.
- **F61** platform/linux_trash.rs:33 (avec linux_trash/transfer.rs:33) (PLATFORM-03) : tout dossier nommé Trash avec files et info est pris pour une corbeille. Scénario : sauvegarde d'un ancien ~/.local/share : couper-coller hors de files échoue.
- **F62** platform/linux_quick_access.rs:35 (PLATFORM-03) : surveillance des signets installée seulement si ~/.config/gtk-3.0 existe au démarrage. Scénario : profil neuf : signets de Nautilus non vus avant redémarrage.
- **F63** platform/thumbnails.rs:116 (PLATFORM-04) : clé de cache à date nulle si la date est absente, chemin sensible à la casse. Scénario : FS sans date fiable : vignette périmée ; C:\A.png et c:\a.png : deux entrées.
- **F64** platform/thumbnails.rs:87 (PLATFORM-04) : format déduit de l'extension seule. Scénario : image sans extension ou à extension fausse : « format inconnu ».
- **F65** platform/thumbnails.rs:230 (PLATFORM-04) : la miniature Office Linux s'arrête à l'erreur du premier candidat. Scénario : aperçu JPEG corrompu mais vignette PNG valide : pas de miniature.
- **F66** platform/thread_priority.rs:17 (PLATFORM-04) : la priorité est sans effet sous Linux. Scénario : les modes Arrière-plan, Normal et Turbo ne changent rien aux copies et suppressions.
- **F67** platform/viewer_media.rs:191 (PLATFORM-04) : côté Windows, les rapports du worker n'ont pas d'identifiant de seek. Scénario : barre de progression qui revient brièvement à l'ancienne position après un seek.
- **F68** platform/network.rs:112, :214-216, :236 et :181 (PLATFORM-04) : erreurs d'énumération avalées, aucun délai ni annulation sur les partages d'un serveur, tampon d'alignement 1 réinterprété en NETRESOURCEW. Scénario : voisinage réseau lent : liste partielle présentée comme complète ; serveur SMB éteint : un thread bloquant par navigation ; comportement indéfini théorique.
- **F69** platform/network.rs:320 (+ platform/remote.rs:33) (PLATFORM-04) : nom d'utilisateur journalisé au niveau info, mot de passe en clair dans des String non effacées, RemoteAnswer dérive Debug avec son mot de passe. Scénario : journal partagé pour diagnostic ou image mémoire révèle comptes et mot de passe ; tout futur affichage de la réponse écrit les identifiants.

### Faibles : Archives, systèmes de fichiers, historique et opérations (32)

- **F70** commands/archive_iso_open.rs:208 (ARCHIVES-01) : ni Joliet ni Rock Ridge. Scénario : ISO Joliet niveau 1 : « MYDOCU_1.TXT » au lieu de « Mes documents.txt ».
- **F71** commands/archive_iso_open.rs:181 et :195 (ARCHIVES-01) : limite de fin d'enregistrement et bourrage calculés sur la taille de bloc logique et non le secteur de 2048 octets. Scénario : image valide en blocs de 512 ou 1024 octets refusée comme corrompue.
- **F73** commands/archive.rs:239 (et archive_handle_worker.rs:32) (ARCHIVES-01) : un dossier interne inexistant renvoie une liste vide (sauf list_iso sous Linux, qui échoue). Scénario : archive remplacée : onglet resté sur un dossier disparu affiche un dossier vide.
- **F74** commands/archive.rs:143 (ARCHIVES-01) : le listage n'a aucun point d'annulation. Scénario : entrer puis sortir de plusieurs gros .tar.xz ou RAR multivolumes : un thread et un worker occupés jusqu'à 120 s chacun.
- **F75** commands/archive_io_codec/parallel.rs:65 (ARCHIVES-01) : le Drop de Job attend la fin du thread d'encodage. Scénario : annuler une compression ZIP Turbo avec 16 fichiers de 200 Mio en vol : le worker finit après ces compressions inutiles.
- **F77** systems/archive_open_cache.rs:516 (ARCHIVES-02) : une copie modifiée par l'utilisateur n'est jamais évincée et compte dans le quota (512 entrées, 1 Gio). Scénario : édition régulière de documents ouverts depuis des archives : cache « plein » définitif.
- **F78** systems/archive_open_cache.rs:235 (ARCHIVES-02) : le verrou inter-processus est tenu pendant le hachage complet (jusqu'à 256 Mio) et l'éviction. Scénario : disque lent : les ouvertures parallèles, autres fenêtres ou instances, échouent en « occupé » après 15 s.
- **F79** commands/archive_rar.rs:145 (ARCHIVES-02) : la vérification du mot de passe choisit le plus petit membre chiffré quelle que soit sa position. Scénario : RAR solide de plusieurs Gio : presque toute l'archive est décodée, erreur de worker au-delà de 120 s.
- **F80** commands/archive_verify.rs:30, :51 et :65 (+ archive_rar.rs:122) (ARCHIVES-02) : échantillon limité à 1 Mio avant le contrôle d'intégrité, seul le texte « Invalid checksum » signale un mauvais mot de passe, et le plus petit bloc 7z n'est pas vérifié chiffré. Scénario : environ un mauvais mot de passe sur 256 accepté pour ZipCrypto, RAR 4 en mode stocké ou 7z mixte : l'action échoue ensuite avec une erreur générique ; contredit docs/architecture/archives.md.
- **F81** commands/archive_rar.rs:39 (ARCHIVES-02) : les erreurs de limite de ressources RAR deviennent « Invalid RAR archive ». Scénario : fichier après 256 Mio dans un RAR solide : l'utilisateur lit « archive invalide ».
- **F82** systems/archive_io.rs:171 (ARCHIVES-02) : le mot de passe est transmis pour RAR alors que la documentation dit ZIP et 7z seulement. Scénario : relecture de sécurité sous-estimée ; il suffit de corriger la documentation.
- **F83** systems/archive_owned_tree.rs:13 (ARCHIVES-03) : deux handles conservés par entrée possédée. Scénario : espace de travail Windows de 100 000 entrées : environ 200 000 handles ouverts.
- **F84** utils/archive_codec.rs:85 et :87 (ARCHIVES-03) : xz sans limite mémoire, zstd à fenêtre maximale par défaut non documentée. Scénario : .xz à dictionnaire de 1,5 Gio : échec générique ; .tar.zst créé avec --long=31 refusé.
- **F85** utils/archive.rs:112 (+ ui/components/context_menu.rs:358, UI-COMP-01) (ARCHIVES-03) : resolve_virtual_path teste chaque ancêtre sur disque, sur le thread UI lors du menu contextuel. Scénario : clic droit dans un dossier d'archive sur partage lent ou déconnecté : interface figée.
- **F86** systems/archive_volumes.rs:27 (ARCHIVES-03) : chaque volume RAR est rouvert par nom sans comparer l'identité. Scénario : sous Linux, film.part1.rar remplacé entre l'ouverture vérifiée et la réouverture.
- **F87** systems/archive_workspace.rs:315, :333 et :352 (ARCHIVES-03) : sous Linux, rien ne verrouille l'original entre la dernière empreinte et le renommage en sauvegarde. Scénario : données ajoutées par un autre programme ignorées, sauvegarde qui les contenait supprimée.
- **F88** utils/archive_listing_protocol.rs:242 (ARCHIVES-04) : une erreur de worker (jusqu'à 4096 octets, caractères de contrôle et bidirectionnels) est publiée telle quelle. Scénario : worker compromis : message trompeur dans l'interface ; erreurs internes anglaises non traduites.
- **F89** utils/archive_volumes.rs:58 et :44 (ARCHIVES-04) : ancien nommage reconnu seulement à partir de .rar, suite arrêtée à .r99. Scénario : double-clic sur film.r03 n'ouvre pas l'ensemble ; plus de 101 volumes coupés.
- **F93** commands/duplicates.rs:341, :396 et :446 (CORE-01) : erreurs de lecture avalées pendant le hash. Scénario : fichier verrouillé absent du résultat sans indication d'incomplétude.
- **F95** commands/ops/conflicts.rs:54 (+ systems/file_ops/copy.rs:387, FILEOPS-01) (CORE-01) : conflit testé par une existence qui suit les liens. Scénario : lien symbolique cassé occupant le nom cible : aucune question, la publication échoue ensuite en « existe déjà » après une partie des fichiers.
- **F96** commands/ops/trash_listing.rs:53 (CORE-01) : repli de listage Windows qui suit les liens. Scénario : lien de dossier recyclé affiché avec la taille et le type de sa cible.
- **F98** commands/ops.rs:482 (CORE-01) : le rejeu d'une copie utilise un drapeau d'annulation jamais levé. Scénario : grosse copie en redo bloque les autres actions d'historique sans moyen d'arrêt.
- **F99** commands/image_handle_worker.rs:76-79 (et audio, office, session d'icônes, session d'images) (CORE-01) : un worker en échec écrit sur stderr puis sort avec le code 0. Scénario : le parent ne distingue pas un échec d'une réponse vide ou tronquée.
- **F104** systems/file_ops/copy.rs:273 (FILEOPS-01) : l'identité de fichier d'un lien physique le fait traiter comme auto-transfert. Scénario : déplacer A\a.txt vers B où B\a.txt est un lien physique : ignoré sans conflit ; en copie, « a - Copie.txt ».
- **F105** systems/file_ops/archive_transfer.rs:171 (FILEOPS-01) : tout le journal d'effets est vidé après succès, y compris pour une destination qui n'est pas une archive. Scénario : extraction par glisser vers un dossier réel : aucune entrée annulable.
- **F107** systems/file_ops/copy.rs:561 (FILEOPS-01) : détection SSD par préfixe de lecteur sous Windows, un stat par fichier sous Linux. Scénario : disque dur monté dans C:\mnt\hdd reçoit 4 workers Turbo ; 100 000 stat avant la copie.
- **F109** systems/history_delete.rs:83-90 et :216-226 (FILEOPS-02) : un élément sans identité est mis à la corbeille sans vérification et rend tout le journal non annulable. Scénario : sélection mixte avec un volume sans identifiant : Annuler refuse tous les éléments.
- **F110** systems/history_links.rs:33-45 (FILEOPS-02) : le rollback ne retire pas le lien tout juste créé. Scénario : échec de lecture des métadonnées après création : lien orphelin, Rétablir suivant en « existe déjà ».
- **F112** systems/file_ops/delete.rs:148-169 et :297-307 (FILEOPS-02) : un élément déjà disparu n'est pas compté terminé, et les descendants d'un dossier sélectionné restent « non traités ». Scénario : issue « Interrompu » au lieu de « Terminé » ; fichier d'un dossier supprimé affiché « non traité ».
- **F113** systems/file_ops/scheduler.rs:236-273 (FILEOPS-02) : réservation de chemins purement lexicale. Scénario : \\?\C:\x et C:\x, lecteur mappé et UNC, jonction, ou « X (1) » d'une extraction : opérations parallèles sur les mêmes objets.
- **F114** systems/folder_sizes/pools.rs:59-60 (FILEOPS-02) : la cellule mémorise aussi l'erreur de construction du pool. Scénario : échec transitoire : ce couloir est inutilisable jusqu'au redémarrage.
- **F118** systems/history_move.rs:57 (FILEOPS-03) : la copie native renvoie la taille de la source, le contrôle de taille compare la source à elle-même. Scénario : copie tronquée détectée seulement par l'empreinte, après relecture complète.

### Faibles : Modèles, utilitaires et noyau (15)

- **F124** commands/sidebar.rs:25-31, :45 et :120 (CORE-02) : sous Unix, un lien symbolique vers un dossier est écarté de l'arbre. Scénario : ~/Documents pointant vers un autre disque sans nœud ni flèche.
- **F125** commands/sidebar.rs:63-79 (+ ui/app/navigation.rs:130-161, UI-APP-05) (CORE-02) : le résultat du déploiement récursif est fusionné sans vérifier qu'il correspond à l'état. Scénario : fichiers cachés activés, dossier replié ou lecteur déconnecté pendant le parcours : résultat périmé qui écrase les nœuds récents.
- **F126** commands/system.rs:9-19 (CORE-02) : open_item est async mais appelle directement validate_path et la plateforme. Scénario : défaut latent (aucun appelant) ; les préfixes « ::{ » et « ? » contournent validate_path.
- **F127** commands/system.rs:33-53 (CORE-02) : la purge s'arrête à la première erreur et Linux n'a pas de pause d'écriture du cache. Scénario : vignette écrite pendant la purge : icons échoue, thumbnails reste intact alors que l'interface a invalidé.
- **F130** models/error.rs:131-143 et autres (CORE-02) : causes structurées retrouvées par sous-chaîne dans des messages contenant des noms de fichiers. Scénario : fichier nommé comme un marqueur : invite de mot de passe ou refus d'accès faussés, emplacements de récupération perdus.
- **F131** models/history.rs:142-148 et :170-183 (CORE-02) : push purge les entrées expirées en hachant les sauvegardes de métadonnées sur le thread appelant. Scénario : créer un dossier depuis le thread UI avec une pile de rétablissement chargée en suppressions : E/S sur l'UI.
- **F136** utils/office_document.rs:109-139 (CORE-03) : le plafond de sortie n'est testé qu'avant l'ajout d'une ligne non bornée. Scénario : docx à un paragraphe de plus de 32 Mo : réponse refusée, le Viewer échoue au lieu de tronquer.
- **F138** utils/office_document.rs:252 (et search.rs:207) (CORE-03) : les définitions de taquets ajoutent chacune une espace. Scénario : docx à taquets personnalisés : lignes décalées, espaces parasites en recherche.
- **F140** utils/office_document.rs:248-274 (CORE-03) : alternative de contenu non traitée. Scénario : docx avec zone de texte enregistrée par Word : texte en double.
- **F141** utils/office_document.rs:50-57 et office_document/sheets.rs:76-81 (CORE-03) : décompression jusqu'à 128 Mo par partie sans budget cumulé ni limite du nombre d'entrées. Scénario : xlsx ou pptx malveillant avec de nombreuses feuilles très compressibles : le worker consomme tout son délai en CPU.
- **F142** utils/office_preview.rs:21-24 et :41-47 (CORE-03) : une seule lecture de 8 Ko, étiquette tronquée à 10 caractères comparée strictement, entités non décodées. Scénario : infobulle docx courte ou vide ; odt sans sauts de ligne et entités brutes.
- **F143** utils/office_document/search.rs:67-71 (CORE-03) : recherche limitée au corps principal Word (pas en-têtes, notes) ni graphiques ni commentaires Excel. Scénario : mot présent seulement dans un en-tête : aucun résultat sans signal.
- **F144** utils/mod.rs:109-110 (CORE-03) : comparaison insensible à la casse faite octet par octet. Scénario : tri backend (commands/io.rs:259) de noms accentués ou cyrilliques incohérent avec le comparateur de l'interface.
- **F146** utils/image_preview_decode.rs:76-81 (CORE-03) : SVG analysé deux fois avec les options par défaut d'usvg (résolution des images externes). Scénario : SVG référençant un fichier local : fichier lu pour obtenir seulement la taille ; risque borné par le confinement du worker.
- **F148** chemins non UTF-8 : utils/path_text.rs:29, commands/sidebar.rs:35-142, ui/app/file_chooser/navbar.rs:150 et browse.rs:140, commands/ops.rs:332-335 et :515-517, platform/linux_impl.rs:588 et :825, platform/linux_preview.rs:84, platform/windows_pdf_session.rs:42 (CORE-03, CORE-02, CORE-01, PLATFORM-02, UI-APP-03, WIN-WORKERS-01) : conversion avec perte, échappement NUL non décodé, ou refus « Preview path is not Unicode ». Scénario : sous Linux, un nom non UTF-8 ne s'ouvre pas, n'a pas d'aperçu, ne peut être annulé après renommage ; sous Windows, un nom NTFS à surrogate isolé devient U+FFFD et vise un autre fichier ou aucun.

### Faibles : Interface : application, fenêtres et composants (89)

- **F149** ui/app/archive_password.rs:184 (UI-APP-01) : mot de passe vide ou de plus de 1024 octets refusé sans message ni effet.
- **F150** ui/app/archive_password.rs:104 (UI-APP-01) : un refus « mauvais mot de passe » venu d'ailleurs efface le mot de passe saisi en cours de vérification. Scénario : miniature de la même archive en échec pendant la saisie : invite fermée puis rouverte.
- **F151** ui/app/archive_password.rs:176 et :187 (+ ui/components/archive_password_dialog.rs:68-72, UI-COMP-01) (UI-APP-01) : le mot de passe est mémorisé avant vérification et conservé si on annule. Scénario : prochain accès : « mauvais mot de passe » sans saisie.
- **F152** ui/app/archive_password.rs:143 (+ archive_protection.rs:41 et :97) (UI-APP-01) : copies en clair du mot de passe non effacées à chaque frappe.
- **F153** ui/app/advanced_search/validation.rs:75 (UI-APP-01) : volet disparu ou chemin identique : navigation sans effet ni message, erreur pourtant effacée.
- **F154** ui/app/advanced_search/validation.rs:26 (UI-APP-01) : une date valide est rejetée pour minuit local inexistant (changement d'heure) ou antérieur à 1970.
- **F155** ui/app/advanced_search/calendar.rs:36, :19 et :29-32 (UI-APP-01) : choix de date sans réinitialisation de l'erreur, espaces non tronqués à l'ouverture, pas de mois sur les jours 29 à 31 qui perdent le jour d'origine.
- **F156** ui/app/advanced_search/help.rs:171 (UI-APP-01) : lien regex101 sans retour d'erreur si aucun navigateur.
- **F157** ui/app/clipboard.rs:18 (UI-APP-01) : nombre annoncé calculé sur les lignes du texte (faussé par un nom contenant un saut de ligne) ; l'ordre de copie vient d'un ensemble non ordonné (:29).
- **F158** ui/app/context_menus.rs:305 et :331 (UI-APP-01) : après « Nouveau » du menu natif, l'élément créé est deviné par différence d'instantané. Scénario : un téléchargement simultané : renommage en ligne sur un fichier étranger.
- **F159** ui/app/context_menus.rs:206 (UI-APP-01) : chargement du menu natif sans délai ni annulation. Scénario : extension Shell bloquée : fil occupé indéfiniment.
- **F160** ui/app/context_menu_render/native.rs:229 (UI-APP-01) : deux sous-menus frères dont le premier élément est lui-même un sous-menu (identifiant -1) sont confondus.
- **F162** ui/app/drag.rs:548-559 (UI-APP-02) : minuterie de survol d'onglet non annulable. Scénario : survol, sortie, nouveau survol 400 ms plus tard : changement d'onglet après 200 ms seulement.
- **F163** ui/app/drag.rs:51-55 (UI-APP-02) : comparaison de lecteurs sur le premier composant. Scénario : préfixe étendu : « copier » au lieu de « déplacer » ; volume monté dans un dossier : déplacer = copie puis suppression inter-volumes.
- **F165** ui/app/duplicates/lifecycle.rs:126 et :129-130 (+ ui/app/file_chooser/sources.rs:37, UI-APP-03) (UI-APP-02) : disque déduit par préfixe sans frontière de composant, et dossier courant ajouté sans dédoublonnage. Scénario : /mnt/usb/x étiqueté disque système ; racine de lecteur listée deux fois, cochée ensemble.
- **F168** ui/app/drive_watchers.rs:468-517 (UI-APP-02) : une création d'observateur en échec n'est pas mémorisée. Scénario : lecteur optique vide : nouvelle tentative et avertissement journalisé toutes les 2 s (Linux) ou 30 s (Windows).
- **F169** ui/app/drive_watchers.rs:170-176 (UI-APP-02) : fin de montage qui navigue dans le panneau actif et non celui d'origine.
- **F170** ui/app/disk_images.rs:53-80 et :108-111 (UI-APP-02) : double sonde sans protection contre le double clic. Scénario : deux navigations vers le même chemin ou second montage ignoré.
- **F173** ui/app/file_chooser/browse.rs:68 et :70 (UI-APP-03) : go supprime des champs sans redonner le focus et ne réinitialise pas la requête du menu de chevron. Scénario : édition du chemin puis clic sur un fil d'Ariane : clavier muet ; menu de l'ancien chevron ouvert sur le nouveau dossier.
- **F174** ui/app/file_chooser/location.rs:39 (UI-APP-03) : le préfixe de lecteur seul (D:) est interprété comme le dossier courant du lecteur.
- **F175** ui/app/file_chooser.rs:368 et :387 (UI-APP-03) : retrait de la requête sans fermeture des menus ni mémorisation ; dossier initial non vérifié. Scénario : clé débranchée : la fenêtre s'ouvre sur une erreur à chaque fois.
- **F176** ui/app/file_chooser/create.rs:34, :53 et :63 (UI-APP-03) : recherche de nom libre en boucle non bornée et non annulable ; création non historisée si la fenêtre est fermée ; frappes perdues avant le re-listage.
- **F177** ui/app/file_chooser/band.rs:49 (UI-APP-03) : le rectangle de sélection garde des positions de l'ancien tableau. Scénario : navigation pendant le tracé : entrées fantômes, bouton d'acceptation actif à tort.
- **F178** ui/app/file_chooser/overwrite.rs:36 (UI-APP-03) : invite de remplacement orpheline si le sélecteur se ferme dans l'intervalle (cas étroit).
- **F179** ui/app/file_chooser/save.rs:222-229 et :92-94 (UI-APP-04) : résultat appliqué sans vérifier que le dossier est resté le même ; refus de lecture seule qui ne reflète pas le droit d'écriture réel.
- **F180** ui/app/file_operations.rs:268-278, :176-183 et :357 (UI-APP-04) : chaque échec écrase l'avis et la reprise de suppression en cours ; un lien brisé jugé inexistant est exclu de la reprise.
- **F181** ui/app/file_chooser/setting.rs:13-18 et :46-49, ui/app/folder_handler.rs:13-18 et :54-57 (UI-APP-04) : une lecture d'état antérieure à la bascule l'écrase ; un échec rétablit une valeur peut-être fausse.
- **F182** ui/app/file_chooser/sidebar.rs:52-63 (UI-APP-04) : la fenêtre principale est mise à jour dans celle du sélecteur. Scénario : sélecteur fermé pendant le montage : le volume reste proposé comme non monté.
- **F183** ui/app/file_chooser/tree.rs:93-103 (UI-APP-04) : résultat de chargement de l'arbre inséré sans tenir compte d'une mise à jour intermédiaire.
- **F184** ui/app/instance.rs:13-15 et :25-31 (UI-APP-04) : élément sans parent ignoré, chemin relatif ouvrant un onglet vide, comparaison Windows sans normalisation de \\?\ ni des séparateurs (onglet en double).
- **F188** ui/app/navigation.rs:54-56 (UI-APP-05) : seul le parent immédiat est relu pour révéler le dossier actif.
- **F189** ui/app/navigation/breadcrumb.rs:129-154, :25-31 et :200-211 (+ navigation.rs:245) (UI-APP-05) : chemin saisi accepté s'il existe par rapport au répertoire courant du processus, comparaisons de chemins en chaînes, erreur de chevron ignorée, validation sans délai. Scénario : « docs » ou « C: » accepté ; réseau injoignable : threads bloqués qui s'accumulent.
- **F190** ui/app/navigation.rs:393-416 (UI-APP-05) : fermer un onglet ne ferme pas l'édition du fil d'Ariane. Scénario : champ avec le texte de l'ancien onglet, Entrée navigue dans le nouvel onglet.
- **F191** ui/app/layout/tabs.rs:645-651 (UI-APP-05) : plusieurs dossiers déposés sur la barre d'onglets apparaissent dans l'ordre inverse.
- **F192** ui/app/open_with.rs:229-235 (UI-APP-05) : valider pendant le chargement ferme l'invite sans rien lancer.
- **F193** ui/app/pinned_order.rs:76 (UI-APP-05) : le rang enregistré d'un dossier momentanément absent est perdu à la réorganisation.
- **F194** ui/app/network.rs:466-468 (UI-APP-05) : à la déconnexion, l'échec du relâchement d'un observateur empêche la déconnexion demandée.
- **F195** ui/app/navigation.rs:700-716 (UI-APP-05) : au démarrage, tous les onglets restaurés sont listés, y compris distants. Scénario : connexions et invites d'identifiants non demandées.
- **F196** ui/app/properties/interactions.rs:148-173 (UI-APP-06) : l'écriture du raccourci n'est pas annulable et ne rafraîchit rien si la fenêtre a disparu.
- **F197** ui/app/properties/selection.rs:22-30 (UI-APP-06) : un seul chemin en échec fait échouer toute l'inspection ; « Cancelled » affiché en anglais.
- **F198** ui/app/properties/window.rs:30-38 (UI-APP-06) : aucune fenêtre Propriétés n'est réutilisée. Scénario : Alt+Entrée répété sur un gros dossier : N calculs concurrents.
- **F200** ui/app/properties/interactions.rs:111 (UI-APP-06) : cible de raccourci vide refusée même pour une cible non fichier. Scénario : changer le commentaire d'un .lnk Shell : « cible invalide ».
- **F201** ui/app/rename.rs:252-255 (UI-APP-06) : mot de passe enregistré ou oublié avant tout conflit de nom. Scénario : créer docs.zip face à une archive protégée du même nom puis annuler : mot de passe mémorisé remplacé ou effacé.
- **F202** ui/app/properties/permissions/save.rs:97-98 et properties/dialog.rs:52 (UI-APP-06) : droits non relus après « modifié depuis l'ouverture » (erreur à chaque essai) ; message de validation non effacé en changeant de champ ou d'onglet.
- **F203** ui/app/selection.rs:55 et :354 (UI-APP-07) : curseur dans un groupe replié : la flèche saute au premier fichier.
- **F207** ui/app/theme_window/draft.rs:251 et :233, theme_window.rs:17 (UI-APP-07) : suppression sans confirmation, changement de thème qui abandonne le brouillon, aperçu non rétabli si la fenêtre principale ferme.
- **F209** ui/app/trash_settings/linux.rs:157 et :171 (UI-APP-07) : pourcentage KDE arrondi puis relu comme modifié. Scénario : Percent=12.34 devient 12.3 à l'enregistrement sans changement.
- **F210** ui/app/trash_settings/settings.rs:82, :168, :229 et :281 (UI-APP-08) : chargement tout ou rien ; saisie vide abandonnée au changement de volume ; état initial non mis à jour après échec partiel (volume A jugé inchangé) ; occupation non relue après vidage partiel.
- **F214** ui/app/viewer.rs:196 et viewer/markdown/loading.rs:13 (+ markdown.rs:101) (UI-APP-08) : suppressions en cours non connues d'un Viewer créé ensuite, et lecture du .md ou des images hors registre de suppression. Scénario : le fichier est ouvert par le worker pendant sa suppression.
- **F215** ui/app/viewer/fullscreen_controls.rs:154 et :207 (UI-APP-08) : drapeau d'attente levé avant la récupération du handle : barre plein écran jamais créée si elle échoue ; nouvelles tentatives et journal à chaque placement.
- **F216** ui/app/viewer/markdown.rs:95 (UI-APP-08) : au plus 32 entrées d'images, jamais retirées. Scénario : au-delà des 32 premières images, les suivantes restent sur leur texte de remplacement.
- **F217** ui/app/viewer/markdown/parse.rs:112 et :51-66 (UI-APP-09) : coloration des blocs de code dépendante du temps écoulé (200 ms) ; images locales relatives acceptant « .. » et noms de périphériques réservés. Scénario : premier document sans couleur ; fichier de périphérique qui bloque la lecture, sans confinement au dossier du document.
- **F218** ui/app/viewer/markdown/selection.rs:134-137 et viewer/text/selection.rs:141-143 (UI-APP-09) : Ctrl+A puis Ctrl+C copie un document tronqué pendant l'analyse progressive, ou copie des centaines de Mio sur le thread UI.
- **F219** ui/app/viewer/media.rs:238-244 (UI-APP-09) : lecture après la fin du média sans reprise enregistrée. Scénario : lecture automatique désactivée : second clic nécessaire.
- **F220** ui/app/viewer/media_overlay.rs:409-479 (UI-APP-09) : échec d'ouverture des popups : boucle d'erreurs journalisées tant que le pointeur reste dessus.
- **F221** ui/app/viewer/tree.rs:230-243, :54, tree/loading.rs:75 et :155-157 (UI-APP-09) : niveau en échec jamais retenté, sommes de tailles non protégées contre le débordement, comptage bloqué si la création du thread échoue.
- **F223** ui/components/conflict_dialog/advanced.rs:135-137 et frame.rs:187-189 (UI-COMP-01) : indentation sans plafond (nom tronqué à 12 niveaux) ; lignes de la liste avancée non focalisables ni pilotables au clavier.
- **F224** ui/components/file_list/icons.rs:281 (UI-COMP-02) : l'extension « key » est classée présentation avant certificat.
- **F226** ui/components/file_list/presentation.rs:72 et :174 (UI-COMP-02) : la case de sélection et le bouton des tailles affectent le volet actif sans passer par l'activation. Scénario : cocher dans le volet inactif puis quitter : volet actif non persisté.
- **F227** ui/components/file_list/details.rs:329, grid.rs:433 (+ ui/components/sidebar/rows.rs:168, UI-COMP-05) (UI-COMP-02) : les dépôts internes sur Corbeille ou Réseau (liste et barre latérale) ne sont pas refusés et la zone est surlignée. Scénario : l'opération échoue en « préparation » au lieu d'être refusée au survol.
- **F228** ui/components/file_list/icons.rs:411 (UI-COMP-02) : icônes système uniques conservées pour la session, et échec transitoire mis en cache définitivement. Scénario : icône de repli conservée jusqu'au vidage du cache.
- **F233** ui/components/input.rs:129-132 (UI-COMP-03) : saisie de navigation rognée de ses espaces. Scénario : dossier Linux au nom finissant par une espace : chemin autre ou invalide.
- **F235** ui/components/operations/items.rs:244 et :267 (+ items/table.rs:139, UI-COMP-04) (UI-COMP-03) : tri par état figé sur les résultats de l'instant du calcul ; taille et type des éléments déjà déplacés figés.
- **F236** ui/components/operations/view.rs:163 (avec items.rs:233-246, icons.rs:14) (UI-COMP-04) : chargement des lignes en boucle d'accès disque non annulable. Scénario : naviguer entre grosses opérations : autant de threads qui énumèrent des dizaines de milliers de chemins pour rien.
- **F237** ui/components/operations/items/icons.rs:14 (UI-COMP-04) : ligne résolue sur « dossier de destination + nom d'origine ». Scénario : « garder les deux » : la ligne affiche le fichier préexistant et le double-clic le révèle.
- **F238** ui/components/popup/hover.rs:38 (UI-COMP-04) : intention de survol jamais effacée si la fenêtre se ferme avant le délai. Scénario : sous-menu « Langue » qui ne s'ouvre plus au survol après réouverture.
- **F239** ui/components/operations/list.rs:255 (UI-COMP-04) : seule la borne basse de la largeur est appliquée. Scénario : glissement au bord de l'écran : largeur énorme enregistrée.
- **F241** ui/components/sidebar.rs:42 (UI-COMP-05) : filtre de visibilité qui n'examine que le nœud lui-même. Scénario : AppData masqué, ses sous-dossiers restent affichés sous la ligne précédente.
- **F242** ui/components/sidebar/rail.rs:168 et :226-232 (UI-COMP-05) : le rail ignore volumes non montés et distributions WSL, et n'a pas « Démonter l'image disque ».
- **F243** ui/components/sidebar/pinned.rs:62 et :143 (UI-COMP-05) : limites de bouton de 2 rem alors que la cellule en fait 2,25 : un clic sur le bord ferme puis rouvre le menu.
- **F244** ui/components/tabs/scroll.rs:44 (UI-COMP-05) : arrêt puis redémarrage du défilement avant la trame suivante lance une seconde chaîne. Scénario : bord traversé rapidement pendant un glisser d'onglet.
- **F245** ui/components/settings/fallback.rs:18 (UI-COMP-05) : sans surfaces natives, le voile ne gère que le bouton gauche. Scénario : clic droit derrière le panneau ouvre le menu contextuel de la liste.
- **F246** ui/components/top_bar/breadcrumb_menu.rs:57 (UI-COMP-06) : le handle de défilement partagé n'est pas remis à zéro à l'ouverture. Scénario : un menu de chevron s'ouvre avec le décalage du précédent.
- **F248** ui/components/tooltip/hover.rs:176-186 (avec tooltip.rs:131-163) (UI-COMP-06) : infobulle construite une fois à l'entrée. Scénario : fichier en cours de copie : taille et aperçu anciens jusqu'à la sortie.
- **F249** ui/components/tooltip.rs:28-32 (avec assets.rs:94-106) (UI-COMP-06) : invalidate_media_preview ne vide pas les assets GPUI. Scénario : Viewer ouvert sur une image modifiée : infobulle aux anciens pixels.
- **F250** ui/components/tooltip/hover/native_window.rs:534-538 (UI-COMP-06) : échec transitoire de création de la surface native marque la fenêtre parente en échec définitif. Scénario : infobulles image et PDF dans la fenêtre jusqu'à sa fermeture.
- **F251** ui/audio_covers.rs:25-29 (UI-REST-01) : à 4096 entrées vivantes, une clé arbitraire est supprimée. Scénario : pochette identique plus partagée, mémoire dupliquée.
- **F252** ui/date_groups/grouping.rs:178 (UI-REST-01) : jour de référence relu indépendamment de celui du cache. Scénario : minuit entre les deux lectures : un rendu incohérent, auto-corrigé.
- **F253** ui/date_groups/grouping.rs:121-123 et :141-147 (UI-REST-01) : date de modification absente rangée en « Plus ancien » (autres modes : « date inconnue ») ; origine non normalisée. Scénario : « C:\Users » et « c:\users » forment deux sections.
- **F254** ui/format.rs:69-85, :155-160 et :12-25 (UI-REST-01) : signe plus accepté dans une taille saisie, année à un ou deux chiffres lue comme l'an 26, nom de volume « Disque local () » pour la racine sans libellé.
- **F255** ui/errors.rs:134-142 et :48, :91 (UI-REST-01) : « existe déjà » détecté sur un texte anglais ; repli des annulations qui affiche le Debug complet de l'erreur.
- **F257** ui/elements/button.rs:75-93 (UI-REST-01) : le bouton agit à l'appui, sans annulation en sortant, et n'est pas activable au clavier. Scénario : bouton « Ouvrir » du Viewer inaccessible à la navigation clavier.
- **F258** ui/file_types.rs:6-273 (UI-REST-01) : catalogue de code trop large (noms entiers courants, suffixes d'une lettre, wasm, fasl). Scénario : fichier sans extension « config » ou binaire .wasm rangé en Code, aperçu texte tenté sur du binaire.
- **F259** ui/i18n.rs:1797 (UI-REST-02) : une erreur de formatage Fluent est journalisée à chaque appel sans limitation. Scénario : message défectueux dans un panneau animé : journal inondé à chaque image.
- **F260** ui/paths.rs:11 (UI-REST-02) : comparaison toujours insensible à la casse sous Windows. Scénario : dossier sensible à la casse (WSL) : A et a confondus, glisser A sur a refusé à tort.
- **F261** ui/style.rs:98 (UI-REST-02) : nombre de colonnes de la grille calculé sans la barre de défilement. Scénario : au seuil d'un changement de colonne, noms tronqués plus tôt que prévu.
- **F262** ui/workspace/filters.rs:335 et :151 (UI-REST-03) : rafraîchissement calendaire ignoré pour les seules plages personnalisées (fuseau ou heure d'été) ; motif glob invalide de la corbeille qui vide la liste.
- **F263** ui/workspace/snapshots.rs:22, :82 et :52 (UI-REST-03) : piles précédent et suivant sauvegardées sans borne ; un volet à onglets vides abandonne toute la disposition ; identifiants d'onglet non vérifiés uniques ni l'invariant du Viewer. Scénario : sessions longues qui grossissent le fichier ; fichier édité ou corrompu qui fait perdre tous les autres onglets.
- **F264** ui/workspace/operations.rs:6 et :64 (UI-REST-03) : énumération des volumes et lecture de l'Accès rapide synchrones au démarrage, même quand la disposition vient de la sauvegarde ; fermer l'explorateur peut rendre le Viewer volet actif. Scénario : lecteur réseau inaccessible retarde la première fenêtre.

### Optimisations : Archives, workers et IPC (11)

- **O1** commands/archive_io_worker.rs:57 et archive_io_codec.rs:77 (+ archive_iso_open.rs:178, archive_io_codec.rs:336 et :505) (ARCHIVES-01) : environ 5 allers-retours IPC par lecture (rendez-vous Checkpoint, Decoded, BudgetReader), et lecteurs distants non tamponnés (un octet puis le reste pour chaque enregistrement ISO, un en-tête TAR de 512 octets par requête). Coût : 100 000 petits fichiers d'un .tar.gz ou 50 000 entrées ISO : centaines de milliers d'allers-retours ; un WindowReader déjà utilisé pour ZIP les réduirait d'un ordre de grandeur.
- **O2** commands/archive_io_codec.rs:257 (ARCHIVES-01) : le manifeste ZIP relit l'en-tête local de chaque membre, chaque fenêtre de 256 Kio traverse l'IPC. Coût : un fichier dans un ZIP de 10 000 photos de 3 Mo transfère environ 2,5 Go avant de décoder.
- **O3** commands/archive_io_codec/parallel.rs:94 (ARCHIVES-01) : un thread OS, un ZipWriter d'au moins 64 Kio et une archive temporaire par petit fichier en Turbo. Coût : 100 000 petits fichiers : 100 000 threads.
- **O4** commands/archive.rs:231 et archive_handle_worker.rs:108 (ARCHIVES-01) : chaque navigation dans un sous-dossier réindexe toute l'archive, sans cache. Coût : cinq niveaux d'un .tar.zst de 4 Go : 20 Go décompressés.
- **O5** systems/archive_open_cache/recovery.rs:199 (ARCHIVES-02) : par bloc de 256 Kio, sérialisation JSON de tout l'état (le bloc à venir encodé en tableau de nombres, environ 1 Mio) et sync_all. Coût : fichier de 256 Mio : environ 1 024 fsync et près de 1 Gio de JSON.
- **O6** systems/archive_owned_tree.rs:463 (ARCHIVES-03) : remove_selected recalcule toutes les clés pour chaque élément. Coût : 10 000 éléments d'une archive de 100 000 entrées : environ 10^9 comparaisons.
- **O7** systems/archive_workspace.rs:203 et :191 (ARCHIVES-03) : seconde empreinte complète de la source aussi sous Windows (déjà protégée en écriture), et WalkDir complet à la recherche de liens que le décodage de production ne crée pas. Coût : archive de 20 Go lue trois fois ; parcours de 100 000 entrées à chaque ouverture.
- **O8** systems/archive_workspace.rs:414 (via archive_transfer.rs:68) et :592 (ARCHIVES-03) : une source virtuelle de copie ouvre un espace de travail complet au lieu de l'extraction ciblée, et l'export copie le membre au lieu de le renommer. Coût : 1 Ko hors d'une archive de 30 Go décode 30 Go ; exporter 10 Go écrit 20 Go.
- **O9** platform/windows_worker_sandbox.rs:197 (WIN-WORKERS-02) : chaque démarrage LPAC copie l'exécutable, fait un sync_all puis relit la source pour la hacher. Coût : chaque requête ponctuelle (texte Office, audio, archives) paie une écriture forcée et la relecture de dizaines de Mio.
- **O10** platform/windows_preview_quota.rs:290 (WIN-WORKERS-02) : chaque écriture du cache énumère tout le dossier sous le bail exclusif. Coût : O(N) par écriture, O(N²) pour remplir le cache.
- **O11** platform/windows_icon_pixels.rs:84 (+ windows_icon_cache.rs:42) (WIN-WORKERS-01) : chaque accès au cache clone l'image (jusqu'à 256 Kio) sous le mutex global. Coût : défilement rapide d'exécutables : copies et verrou sérialisé.

### Optimisations : Fichiers, opérations, historique (29)

- **O13** systems/file_ops/file_copy.rs:282 (+ systems/path_parents.rs:54, platform/windows_file_copy.rs:92) (FILEOPS-01, FILEOPS-03, WIN-FS-01) : pour chaque fichier, les parents de la source sont résolus de nouveau (canonicalize, une ouverture par ancêtre, duplication de tous les handles de la chaîne). Coût : 100 000 petits fichiers à profondeur 8 : environ un million d'ouvertures redondantes.
- **O14** systems/file_ops/copy.rs:315 (FILEOPS-01) : sous Windows, chaque entrée parcourue est ouverte et verrouille l'opération même sans archive. Coût : déplacement de 100 000 fichiers sur le même volume : ouverture en lecture de chacun à la planification.
- **O15** systems/file_ops/destination_tree.rs:134 (FILEOPS-01) : ensure_directory repart de la racine pour chaque fichier. Coût : arborescence profonde de petits fichiers : appels système et contention sur le mutex lu par l'interface.
- **O17** systems/file_ops/file_copy.rs:52 (+ systems/staging_journal.rs:78) (FILEOPS-01, FILEOPS-03) : chaque fichier copié crée puis supprime un dossier de staging, un pin et une note JSON verrouillée dans le profil. Coût : 10 000 petits fichiers : environ 40 000 opérations de métadonnées, chacune scrutée par l'antivirus.
- **O19** systems/history_file_undo.rs:400 et :686-691 (FILEOPS-02) : annuler une copie lit deux fois chaque fichier. Coût : copie de 100 Go : 200 Go relus avant de libérer l'espace.
- **O20** systems/history_directories.rs:30-64 (FILEOPS-02) : un stat par chemin avant le cache, test d'ancêtre quadratique. Coût : annuler un déplacement de 50 000 fichiers : 50 000 stat inutiles et environ 10^8 comparaisons.
- **O21** systems/folder_sizes.rs:150-176 (FILEOPS-02) : un entry.metadata() par fichier alors que l'énumération fournit la taille sous Windows. Coût : un million de fichiers : un million de stat.
- **O22** systems/history_move_redo.rs:419 (FILEOPS-03) : la validation hache chaque fichier, puis le déplacement le hache encore et, entre volumes, relit la source. Coût : rétablir 20 Go entre volumes : 60 Go lus sur la source.
- **O23** systems/updates.rs:62 (FILEOPS-03) : installateur (jusqu'à 512 Mo) téléchargé entièrement en mémoire. Coût : pic mémoire de la taille du paquet.
- **O24** commands/duplicates.rs:335, :390 et :445 (CORE-01) : un verrou par disque physique est tenu pendant tout le hash, y compris sur SSD. Coût : NVMe : tout le hash blake3 sur un seul cœur par volume.
- **O25** commands/duplicates.rs:330, :385 et :440, commands/search.rs:520 (+ platform/hardware.rs:61 et :90) (CORE-01, PLATFORM-01) : l'identifiant du disque physique est recalculé par fichier (sous Windows : ouverture de \\.\X: et IOCTL, sans cache). Coût : 1 million de candidats : 3 millions d'ouvertures de périphérique.
- **O26** commands/duplicates.rs:409-424 (CORE-01) : la seconde passe regroupe sans rappeler le hash de début. Coût : fichiers à pied commun (zéros, signature) envoyés au hash complet sans pair réel.
- **O28** commands/ops/trash_restore.rs:69 (CORE-01) : sous Windows, le nom de restauration est résolu pour tous les éléments avant la sélection. Coût : restaurer un fichier d'une corbeille de 50 000 éléments lit 50 000 fichiers.
- **O29** commands/ops/linux_trash.rs:84 (+ :51 et commands/io.rs:576, platform/linux_trash.rs:51 et :94) (CORE-01, PLATFORM-03) : unicité par contains sur un vecteur (quadratique), listage complet de la corbeille relancé par propriété d'élément, et directorysizes relu puis réécrit avec fsync par dossier purgé. Coût : restaurer 50 000 éléments : 1,25 milliard de comparaisons ; vider des milliers de dossiers : autant de relectures et de fsync.
- **O30** commands/search.rs:153, :172, :460 et :539 (CORE-01) : deux lstat par entrée sous Linux, chaque candidat de la recherche de contenu ouvert deux fois.
- **O32** commands/sidebar.rs:42-46 et :117-121, :58 et :133 (CORE-02) : has_subdirs lit chaque dossier jusqu'au premier sous-dossier (deux fois en « tout développer »), tri avec to_lowercase par comparaison. Coût : dossier de 100 000 fichiers lu en entier pour rien.
- **O33** models/config.rs:299-303 et :313-331 (+ ui/app/preferences.rs:425-445 et :230-243) (CORE-02, UI-APP-05) : chaque sauvegarde réécrit themes.json puis config.json avec deux sync_all, sérialise l'espace de travail sur le thread UI et n'en fusionne aucune. Coût : chaque changement d'onglet ou de largeur de volet, rafale d'écritures synchronisées.
- **O35** utils/audio_metadata.rs:38-44 (CORE-02) : pixels de pochette (jusqu'à 1 Mio) en tableau JSON, 3 à 4 Mo de texte par fichier audio.
- **O36** utils/office_document.rs:50-57 (CORE-03) : tampon doublé jusqu'à 256 Mio puis conversion UTF-8 avec perte copiée. Coût : pic d'environ 384 Mo pour une limite worker de 512 Mo.
- **O37** utils/preview_protocol.rs:148-156 (CORE-03) : chaque image recopiée alors que la réponse entière reste en mémoire. Coût : image d'environ 67 mégapixels : pic double.
- **O39** utils/office_document/sheets.rs:104-109 (CORE-03) : deux copies par cellule. Coût : feuille de centaines de milliers de cellules.
- **O40** platform/linux_gio/applications.rs:153 (PLATFORM-01) : icon_file teste des dizaines de chemins par application sans mémorisation. Coût : centaines d'applications : dizaines de milliers de stat à chaque ouverture du choix.
- **O41** platform/linux_permissions.rs:33 (PLATFORM-02) : un processus getent par nom de groupe, propriétaire et groupe. Coût : utilisateur de vingt groupes : une vingtaine de processus après expiration du cache de 10 s.
- **O42** platform/linux_mime.rs:184 et :227 (PLATFORM-02) : parcours linéaire de toutes les globs par fichier et recalcul de l'étendue magique à chaque appel. Coût : dossier de milliers de fichiers dans le sélecteur : millions de correspondances.
- **O43** platform/thumbnails.rs:172 et :360 (+ linux_impl.rs:333) (PLATFORM-04) : clone RGBA puis conversion RGB, sync_all avant chaque renommage de cache, et vidéo agrandie à 640 sur le grand côté sur le CPU. Coût : milliers de miniatures : autant de synchronisations disque ; vidéo 320x240 : quatre fois plus de données dans le tuyau.
- **O44** platform/windows_peek.rs:22 (WIN-SHELL-01) : jusqu'à neuf sondages de fichiers et deux JSON relus toutes les 5 s (shell_features.rs:202).
- **O45** platform/windows_shell_delete_tracked/effects.rs:63 (WIN-SHELL-02) : pour chaque descendant supprimé, toute la chaîne d'ancêtres est rouverte sur le thread de rappel du Shell. Coût : fichiers x profondeur ouvertures.
- **O46** platform/windows_shell_window.rs:197 (WIN-SHELL-02) : dédoublonnage par Vec::contains. Coût : plusieurs milliers de fichiers d'un même dossier : quadratique sur le thread qui répond au Shell.
- **O47** platform/windows_file_tree.rs:72 et :169 (WIN-FS-01) : NtQueryDirectoryFile en une entrée par appel avec allocation par nom (un aller-retour SMB par entrée possible), et thread::scope crée des threads par élément supprimé. Coût : dossiers de dizaines de milliers d'entrées sur partage ; suppression Turbo de nombreux dossiers : création de threads en rafale.

### Optimisations : Interface (46)

- **O50** ui/app/archive_password.rs:58 (UI-APP-01) : listage complet avant la navigation, puis second listage, pour un 7z ou RAR sans mot de passe mémorisé.
- **O52** ui/app/calendar.rs:6 (+ ui/app/shell_features.rs:183, ui/app/drive_watchers.rs:212-258 et :273-280, ui/app/viewer/media.rs:189-220, ui/components/tooltip/hover/native_window.rs:516-528) (UI-APP-01, UI-APP-07, UI-APP-02, UI-APP-09, UI-COMP-06) : boucles de réveil permanentes (5 s avec deux tâches bloquantes ; 100 ms en continu, et accès système toutes les 5 s ; 100 ms pour l'éjection et 500 ms pour le presse-papiers ; 80 ms avec clone des informations média même média terminé ; 100 ms par fenêtre parente après une infobulle native). Coût : environ 10 réveils par seconde au repos.
- **O53** ui/app/archives.rs:122 (+ ui/app/drag.rs:317-323, ui/app/open_with.rs:66-69, ui/components/context_menu/item.rs:119 et :185, ui/app/properties/selection.rs:53-59) (UI-APP-01, UI-APP-02, UI-APP-05, UI-COMP-01, UI-APP-06) : recherche linéaire (contains sur Vec ou comparaison de chaînes) entre fichiers de tous les onglets et grosse sélection, sur le thread UI. Coût : 5 000 éléments glissés sur 100 000 fichiers : environ 5 x 10^8 comparaisons par mouvement de souris ; 20 000 sur 20 000 : centaines de millions ; Ctrl+A sur 50 000 fichiers : milliards au calcul des propriétés.
- **O54** ui/app/advanced_search/validation.rs:58 (UI-APP-01) : regex du nom et du contenu compilées sur le thread UI à chaque validation, puis par le worker. Coût : motif volumineux : gel bref.
- **O55** ui/app/drag/external.rs:43-55, :98, :115-120 et :151-154 (UI-APP-02) : liste de chemins reconvertie, filtrée et passée en minuscules à chaque mouvement et image. Coût : 10 000 fichiers glissés depuis l'Explorateur : dizaines de milliers d'allocations par mouvement.
- **O56** ui/app/duplicates/search.rs:104-114 (UI-APP-02) : to_lowercase du chemin et du nom de chaque fichier à chaque frappe. Coût : 500 000 fichiers : environ un million d'allocations par frappe.
- **O57** ui/app/drive_watchers.rs:107 et :453-467 (+ changes.rs:143-146) (UI-APP-02) : l'éjection libère tous les observateurs puis les recrée ; le calcul des chemins se fait sous le verrou partagé dans le rappel du système de fichiers. Coût : éjecter une clé pendant le suivi d'un gros dossier système : arbre reparcouru ; compilation à milliers d'événements par seconde.
- **O59** ui/app/file_chooser/details.rs:45 (+ ui/app/layout/measurements.rs:21-25, UI-APP-04) (UI-APP-03) : mise en forme d'un texte par taille distincte sur le thread UI à chaque changement de colonne ou préparation. Coût : dizaines de milliers de tailles distinctes : gel à chaque navigation ou frappe de filtre.
- **O60** ui/app/file_chooser.rs:377 (UI-APP-03) : Places::of reconstruit (clone, dirs::home_dir) à chaque notification de OxydeApp.
- **O61** ui/app/file_chooser/browse.rs:102 (+ arrange.rs:126-136) (UI-APP-03) : liste déjà triée par list_dir puis retriée ; chaque entrée clonée et libellés recalculés à chaque frappe de filtre.
- **O62** ui/app/file_chooser/navbar.rs:84 (UI-APP-03) : le menu d'un chevron passe par list_dir (métadonnées et tri) et reconstruit tout à chaque rendu. Coût : dossier de dizaines de milliers d'entrées.
- **O63** ui/app/file_chooser/browse.rs:133 (+ ui/app/navigation.rs:639-670, UI-APP-05) (UI-APP-03) : chaque listage relance get_sidebar_nodes qui relit le dossier et un niveau de chaque sous-dossier, à chaque rafraîchissement d'observateur, Retour, F5 ou Ctrl+H. Coût : milliers de sous-dossiers sur partage réseau.
- **O64** ui/app/duplicates/title_actions.rs:33 (UI-APP-03) : « Tout réduire » insère un indice par groupe et prepare_results clone l'ensemble à chaque frappe.
- **O65** ui/app/file_preparation.rs:119-123 (UI-APP-04) : tri et filtre sur le fil bloquant non annulables, deux calculs périmés occupent les permis. Coût : frappe rapide sur 100 000 entrées.
- **O66** ui/app/file_chooser/sidebar.rs:201-209 et :249 (UI-APP-04) : la liste dépliée de l'arbre est reconstruite (avec clonage) quatre à cinq fois par rendu.
- **O67** ui/app/file_chooser/save.rs:215-219 et :111-119 (UI-APP-04) : chemin testé jusqu'à quatre fois sur disque. Coût : aller-retour réseau par appel.
- **O68** ui/app/item_pointer.rs:30-34 (UI-APP-04) : ensemble de la sélection cloné à chaque clic simple.
- **O69** ui/app/layout/tabs.rs:70-101 et :232-241 (UI-APP-05) : tab_drive_name parcourt tout l'arbre de la barre latérale par onglet à chaque rendu, avec recalcul de chaînes normalisées ; le rappel de bornes met à jour l'entité et clone une chaîne à chaque rendu.
- **O71** ui/app/selection.rs:114 et :193-206 (UI-APP-07) : chaque événement de souris du rectangle de sélection reconstruit les lignes et clone les chemins couverts. Coût : plus de 100 000 entrées : O(n) allocations par déplacement.
- **O72** ui/app/type_select.rs:72 (+ ui/date_groups/grouping.rs:163-198 et date_groups/families.rs:38-67, UI-REST-01) (UI-APP-08) : chaque frappe reconstruit les lignes d'affichage, la liste de noms passés en minuscules (jusqu'à trois fois), et recalcule le regroupement hors cache de présentation. Coût : 100 000 entrées : centaines de milliers d'allocations par touche.
- **O73** ui/app/viewer.rs:243 (UI-APP-08) : à chaque notification, le Viewer recapture la source, compare et clone la sélection entière.
- **O74** ui/app/viewer/markdown.rs:162 (UI-APP-08) : le texte de tout le tableau est assemblé à chaque rendu d'en-tête (bouton de copie masqué) ; un bloc de code copié deux fois par rendu. Coût : mégaoctets par image.
- **O76** ui/app/viewer/content.rs:144 (UI-APP-08) : analyse du texte Office sur le thread UI. Coût : classeur volumineux : gel avant le premier affichage.
- **O77** ui/app/viewer/text.rs:135, :281-308 et :334 (UI-APP-09) : conversion avec perte qui recopie tout le tampon (pic double), réencadrage complet non interruptible à chaque changement de largeur, copier-sur-écriture de jusqu'à un million de segments par lot de coloration. Coût : fichiers de centaines de Mio.
- **O78** ui/app/viewer/tree/loading.rs:47-55 (UI-APP-09) : jusqu'à 1000 sous-dossiers ouverts un par un avant publication, sans affichage partiel. Coût : lecteur réseau : longue attente à « chargement ».
- **O81** ui/components/conflict_dialog/advanced.rs:123-243 (+ conflict_dialog.rs:114-120, rules.rs:100-115) (UI-COMP-01) : une ligne par conflit visible à chaque rendu, sans virtualisation, avec filtrage et comptages complets. Coût : fusion de dizaines de milliers d'entrées : chaque survol coûteux.
- **O82** ui/components/conflict_dialog/controls.rs:46-48 (avec rules.rs:34-42) (UI-COMP-01) : record_choice parcourt tous les conflits pour chaque dossier. Coût : 30 000 conflits dont 5 000 dossiers : plus de 100 millions de recherches au clic.
- **O83** ui/components/dropdown.rs:252-297, :326-336, :395 et :417 (UI-COMP-01) : à chaque rendu, mise en forme de toutes les entrées, jusqu'à douze recherches de traduction par entrée, vecteur cloné deux fois.
- **O84** ui/components/file_list/card_body.rs:54, :92 et :105 (UI-COMP-01) : deux identifiants par format! avec le chemin complet par carte et par rendu. Coût : centaines de cartes : milliers d'allocations par image.
- **O85** ui/components/file_list/details.rs:101, grid.rs:514 et selection_button.rs:16 (UI-COMP-02) : résumé de sélection recalculé à chaque rendu par un parcours complet avec hachage, sans cache par révision.
- **O86** ui/components/input.rs:134-140 et :626-647 (UI-COMP-03) : chaque complétion liste le dossier en entier avec tri et métadonnées puis refiltre et retrie ; tâches non interrompues. Coût : 100 000 entrées : listage, deux tris, 100 000 allocations par pause de frappe de plus de 150 ms.
- **O87** ui/components/open_with_dialog.rs:28, :108, :113 et :159 (UI-COMP-03) : liste visible (minuscules, accents retirés) recalculée au moins trois fois par rendu.
- **O89** ui/components/operations/footer.rs:36-40 et :65-71 (UI-COMP-03) : l'animation de respiration redessine la vue entière à chaque image et le compteur verrouille toutes les opérations à chaque rendu.
- **O90** ui/components/popup/window/frame.rs:90 (UI-COMP-04) : la table des bornes de lignes est clonée en entier à chaque peinture pour la comparer.
- **O91** ui/components/sidebar/tree.rs:35, :47, :72, :119-129 et :172, rows.rs:49 (UI-COMP-05) : liste des éléments visibles clonée trois fois par rendu, et parcours complet pour largeur, hauteur et emplacements de réorganisation. Coût : « Tout développer » sur des milliers de nœuds : coût linéaire en nœuds à chaque trame.
- **O92** ui/components/tooltip.rs:78-88 (UI-COMP-06) : chaque contrôle d'état (toutes les 16 ms) clone tout PreviewTask, texte d'aperçu compris.
- **O93** ui/components/tooltip/presentation.rs:45-79 et :251-351, placement.rs:61-72 (UI-COMP-06) : rendu de l'infobulle refait à chaque mouvement avec découpage quadratique (x_for_index par graphème) ; taille maximale dépendant de la position verticale du pointeur qui déclenche une notification de plus.
- **O94** ui/components/top_bar.rs:62-68 et :20-30 (UI-COMP-06) : icône racine qui parcourt tous les ancêtres pour chaque lecteur, drive_display en majuscules pour chaque segment, à chaque rendu.
- **O95** ui/audio_covers.rs:14-32 et :54-59 (UI-REST-01) : verrou global tenu pendant le hachage du tampon puis parcours de 4096 entrées faibles. Coût : grille de musique à pochettes uniques : les threads de chargement se sérialisent.
- **O96** ui/date_groups.rs:24-29 (UI-REST-01) : la référence calendaire interroge le système et construit une chaîne à chaque rendu en regroupement calendaire.
- **O97** ui/preview_disk.rs:152 et :166 (UI-REST-02) : budget de 24 Ko par entrée dépassé par la plupart des miniatures PNG transparentes 128 px, qui ne sont jamais persistées. Coût : un worker de décodage relancé à chaque affichage après éviction ou redémarrage.
- **O98** ui/i18n.rs:1787 (UI-REST-02) : chaque appel de traduction relance la résolution Fluent et alloue sans mémoïsation, dans des fonctions de rendu.
- **O100** ui/workspace/preparation.rs:41, :37 et :65 (UI-REST-03) : chaque ajout par lots, chaque listing et chaque lot de mesures de tailles clonent toute la liste, refont les appels Shell par extension, retrient, refiltrent et reconstruisent les libellés. Coût : 100 000 résultats par lots de quelques centaines : quadratique ; dossier de 200 000 entrées : mémoire doublée.
- **O101** ui/workspace/filters.rs:180 et :320 (UI-REST-03) : column_filter_flags recompte toute la liste à chaque rendu d'en-tête ; update_cache_on filtre la liste entière sur le thread UI.
- **O102** ui/workspace/filters/criteria.rs:87 (UI-REST-03) : l'expression régulière du nom est recompilée à chaque rendu du panneau de filtre.
- **O103** ui/theme/states.rs:118 (UI-REST-03) : recherche des remplissages de ligne en environ 3 000 évaluations perceptuelles par force, refaite à chaque déplacement de curseur de l'éditeur de thème. Coût : thème à accent gris ou très pâle : sélecteur de couleur qui saccade.
