# Aperçus et décodeurs sous Linux

**Statut : tous les décodeurs confinés par bubblewrap, vérifiés dans l'interface
(PDF, vidéo, lecteur, images, Office, pochettes et archives)** (T09 et M09 de
l'[audit de portabilité](../reports/2026-10-26-linux-portability-audit.md)).
Windows ne change pas.

## Principe : comme Nautilus

Nautilus délègue les miniatures à des programmes externes : `papers-thumbnailer`
(poppler) pour les PDF, `gst-video-thumbnailer` (GStreamer) pour les vidéos. Il les
lance hors de son processus, si bien qu'un fichier piégé ne peut faire tomber que le
miniaturiseur. Oxyde utilise les mêmes bibliothèques dans un worker, c'est-à-dire
son propre binaire lancé en sous-processus (`--oxyde-preview-worker-v1`). Le
décodage reste ainsi hors du processus de l'interface.

## Worker

- `platform/linux_library.rs` : macro `library!` commune à GIO, poppler et GStreamer,
  qui charge une bibliothèque système à la première utilisation par `libloading`.
  Sans elle, seule sa fonction manque ; aucun paquet `-devel` n'est nécessaire à la
  compilation.
- `platform/linux_worker.rs` : lancement du worker avec entrée et sortie en tubes,
  octet de démarrage `0xA7` comme sous Windows, lecture bornée dans le temps par
  `poll`, arrêt forcé (`SIGKILL`) et récolte à la libération.
- Mémoire : la mémoire privée du worker est plafonnée par `RLIMIT_DATA`, qui joue le
  rôle de la limite de mémoire engagée du job Windows. L'espace d'adressage
  (`RLIMIT_AS`) compterait le code mappé : le binaire de debug pèse 1 Gio.
  `MALLOC_ARENA_MAX=2` évite que glibc réserve 64 Mio par fil de décodage.
- Mort avec le parent : Oxyde garde l'autre bout de l'entrée standard du worker
  jusqu'à le libérer. Un fil du worker attend la fermeture de ce tube (`poll` sans
  lire les données) et se tue alors. `PR_SET_PDEATHSIG` ne convient pas : le signal
  suit le fil qui a lancé le worker, et un pool peut retirer ce fil. Le parent réel
  n'est pas visible non plus, car le bac à sable a son propre espace de processus.

Trois exécutables : `oxyde` pour les aperçus, `oxyde-archive-list-worker` et
`oxyde-archive-io-worker` pour les archives, sans GPUI. [Les paquets](../guides/linux-distribution.md)
les installent tous, et un échec de lancement ne déclenche jamais de décodage local.

## Confinement (bubblewrap)

GNOME confine ainsi ses miniaturiseurs, et glycin ses chargeurs d'images.
`platform/linux_sandbox.rs` lance chaque worker par `bwrap` :

- Espaces de noms : utilisateur (sans espace imbriqué, `--disable-userns`), processus,
  réseau, IPC, nom d'hôte et cgroup. Nouvelle session : pas d'injection dans le
  terminal.
- Fichiers : racine vide, `/usr` en lecture seule et ses liens (`/lib64`…),
  `/etc/ld.so.cache`, `/proc` et `/dev` minimaux, `/tmp` vide. Le binaire d'Oxyde est
  monté seul s'il est hors de `/usr`. Le document est ouvert par Oxyde et monté seul
  par son descripteur (`--ro-bind-fd`) à `/oxyde/source.<extension>`. L'extension
  sert aux formats sans signature. Le worker ne voit aucun autre fichier de
  l'utilisateur. Variables d'environnement effacées, puis fixées une à une.
- Selon le mode :
  - Codecs purs (archives, Office, tags et pochettes audio, miniatures raster) : rien
    de plus.
  - PDF et images, dont les SVG utilisent les polices : `/etc/fonts`, polices, configuration et cache fontconfig de l'utilisateur,
    en lecture seule.
  - Survol vidéo : le registre GStreamer d'Oxyde, en lecture seule.
  - Lecteur du Viewer : le registre, plus les sockets PulseAudio
    (`$XDG_RUNTIME_DIR/pulse/native`, servie par PipeWire) et PipeWire, avec
    `/etc/pulse`, `/etc/pipewire` et `/etc/machine-id`. Aucun accès au GPU : le
    décodage est logiciel.
- Filtre seccomp (`linux_sandbox/seccomp.rs`), compilé par libseccomp, chargée à
  l'exécution, puis installé par bubblewrap. Comme Flatpak, c'est une liste de refus,
  qui renvoie `EPERM` : administration du noyau, mémoire d'autres processus
  (`ptrace`, `process_vm_*`), montages et espaces de noms, `bpf`, `io_uring`,
  `userfaultfd`, `perf_event_open`, clés, `TIOCSTI`/`TIOCLINUX`, changement de
  `personality`. Toute socket autre que locale (`AF_UNIX`) renvoie `EAFNOSUPPORT`.
  Les appels d'une autre architecture tuent le worker. glycin préfère une liste
  d'autorisations ; elle casserait au gré des versions de GStreamer, de ses greffons
  et de glibc.
- Registre GStreamer : un balayage complet des greffons coûte 2,5 s. Oxyde garde son
  propre registre dans `<cache>/Oxyde/gstreamer-1.0/registry.bin`. Avant le premier
  worker multimédia de la session, un worker `media-registry` le met à jour : il
  n'ouvre aucun document et seul ce dossier lui est accessible en écriture. Les
  autres workers le lisent sans pouvoir le modifier (`GST_REGISTRY_UPDATE=no`). Un
  worker compromis ne peut donc ni changer les greffons que chargent les suivants,
  ni toucher au registre des autres programmes. Les greffons de l'utilisateur
  (`~/.local/share/gstreamer-1.0`) ne sont pas chargés.
- Sans bubblewrap utilisable (absent, ou espaces de noms utilisateur interdits), le
  worker est lancé directement, comme le fait glycin. Le journal le signale une fois.
  Ubuntu interdit ces espaces de noms sans profil AppArmor : [le DEB en installe
  un](../guides/linux-distribution.md#ubuntu-et-apparmor).
  Sans libseccomp, le confinement reste, sans filtre d'appels système.
- Coût : 40 ms pour une miniature PDF, 80 ms jusqu'à la première image d'une vidéo
  1080p.

## Images, Office et audio

`platform/linux_preview.rs` : le parent lance un worker par demande et valide sa
réponse ; le worker lit le document monté et ne renvoie que des pixels ou du texte
bornés. Aucune session réutilisable : un décodeur compromis ne voit jamais le
document suivant, au prix d'un lancement par image.

- Modes : `image`, `image-thumbnail`, `image-viewer` (`ui/image_preview.rs`, polices
  chargées seulement pour un SVG), `thumbnail`, `office`, `office-text`,
  `office-document`, `office-search`, `audio-metadata` (JSON), `audio-cover`,
  `audio-cover-thumbnail`.
- Plafonds : 512 Mio et 15 s ; 1 Gio et 30 s pour une image du Viewer ; 256 Mio pour
  une pochette. Réponses bornées comme sous Windows (32 Mio, 256 Mio pour le Viewer,
  8 Mio pour la recherche Office). L'annulation d'une recherche tue le worker.
- Un tube nommé ou un dossier est refusé sans être ouvert en attente.

## Archives

- Navigation et index de recherche (`platform/linux_archive_list.rs`) : le worker
  ne voit que l'archive ; le parent reconstruit chaque chemin virtuel à partir de
  métadonnées validées. 512 Mio, 120 s, 32 Mio de réponse.
- Extraction, création, reconstruction et ouverture d'un fichier
  (`platform/linux_archive_io.rs`) : le protocole de Windows. Le worker ne voit aucun
  fichier ; il demande chaque bloc à lire et confie chaque bloc à écrire. Le parent
  garde sources et destinations (`OwnedTree`), contrôle noms, types et tailles, et
  retire ce qu'il a créé en cas d'échec. 1 Gio, 2 Gio pour créer un 7z.
- Superviseur : une conversation à tour de rôle, sur des tubes de 1 Mio. Écritures non
  bloquantes et attentes de 20 ms : annulation et délai restent actifs quoi que fasse
  le worker. Seule une pause confirmée suspend le délai.
- Écarts avec Windows : pas de `fsync` par fichier extrait, comme Nautilus (2000 petits
  fichiers : 5 s de plus sur Btrfs) ; Turbo augmente le parallélisme, sans changer la
  priorité ; modification d'archive par reconstruction locale, sans la publication de
  groupe (R8).
- Mesures, 1 Gio plus 2000 petits fichiers, Btrfs puis NTFS (ntfs-3g), créer / extraire :
  TAR 2,0 s / 2,1 s puis 6,4 s / 5,1 s ; ZIP 23 s / 7,1 s puis 25 s / 9,6 s ; TAR.ZST 0,6 s /
  1,4 s puis 1,0 s / 4,1 s, données très répétitives.

## PDF (poppler)

- `platform/linux_pdf.rs` (côté worker) : `poppler_document_new_from_fd`, puis rendu
  cairo sur fond blanc en ARGB32 prémultiplié, c'est-à-dire BGRA en mémoire, le format
  que l'interface envoie au GPU. La taille de page de poppler tient compte de la
  rotation. Les côtés sont les mêmes que sous Windows : 128 px pour la grille, 1024 px
  pour le survol, 2048 px pour le Viewer. Au plus 100 000 pages listées.
- `platform/linux_pdf/session.rs` (côté parent) : un worker par miniature (`pdf-grid`,
  `pdf`). Pour le Viewer, un worker par document (`pdf-session`) garde le document
  ouvert et rend une page par requête. Le protocole est celui de Windows : tailles des
  pages, puis une trame par page, et `u32::MAX` pour fermer, avec un accusé de
  fermeture.
- Plafonds : 1 Gio et 15 s par réponse ; la fermeture attend 2 s. Délai, plantage ou
  flux illisible retirent le worker. Une erreur de page garde la session ouverte.
- Une miniature coûte un lancement de processus, environ 50 à 90 ms en debug. Une
  session réutilisable comme `pdf-previews` sous Windows n'est pas implémentée. Les
  documents protégés par mot de passe affichent l'erreur de poppler.

## Vidéo au survol (GStreamer)

- `platform/linux_video.rs` (côté worker) : `playbin3 flags=video`, qui ne
  sélectionne que le flux vidéo : ni piste audio ni sous-titres décodés ou mis en
  file. `playbin` gardait l'audio ignoré en file : 770 Mio contre 205 Mio pour une
  vidéo HEVC 4K. Le puits est un `appsink` sans horloge précédé de
  `videoconvert ! videoscale`, avec des caps BGRA, pixels carrés et 640 px au plus par
  côté. `videoscale` garde les proportions et n'agrandit pas les petites vidéos.
  Chaque image part avec son horodatage, que l'interface utilise pour cadencer. En
  fin de flux, une pause de 200 ms puis un redémarrage depuis READY font boucler la
  vidéo, comme la réouverture sous Windows. La fin de flux et le choix des flux sont
  lus sur le bus par type exact : les types étendus de GStreamer partagent des bits
  avec EOS et ERROR. Un fichier sans flux vidéo est refusé dès que `playbin3` a
  choisi ses flux. Un flux qui se termine sans image est une erreur.
- `platform/linux_video/stream.rs` (côté parent) : un seul worker vidéo à la fois.
  Les paquets ont le format de Windows (longueur, secondes, nanosecondes, trame) et
  sont vérifiés : 2 Mio et 640 × 640 au plus. L'annulation est vérifiée toutes les
  50 ms ; sans image pendant 15 s, l'aperçu est « expiré ».
- Mesures : première image en 60 ms pour un H.264 1080p (74 Mio), en 150 ms pour un
  HEVC 4K avec libde265 (215 Mio), décodé à environ 29 images par seconde.
- Codecs : ceux de GStreamer installés. Fedora ne fournit pas de décodeur HEVC
  logiciel libre : sans codecs RPM Fusion, une vidéo H.265 (iPhone, 4K) affiche
  « Aperçu indisponible », comme Totem et le miniaturiseur de Nautilus. Oxyde ne
  propose pas d'installer de codec.

## Lecteur audio et vidéo du Viewer

Sous Windows, le worker affiche la vidéo dans une surface native placée au-dessus de
la fenêtre. Wayland ne permet pas à GPUI d'insérer une surface étrangère : sous
Linux, le worker envoie les images et GPUI les peint lui-même. Le son sort
directement du worker vers la sortie par défaut du bureau (PipeWire).

- `platform/linux_gstreamer.rs` : `Player` commun au survol et au lecteur
  (`playbin3`, puits `appsink` nommé `frames`, filtre de caps `size` facultatif, bus
  lu par type exact, position, durée, recherche à l'image clé la plus proche).
- `platform/linux_media/worker.rs` (mode `viewer-media`) : `playbin3` avec image et
  son, sans sous-titres. Avec `native-video`, `playbin3` laisse l'image décodée
  intacte jusqu'à la sortie d'Oxyde, qui convertit et met à l'échelle en une seule
  passe (`videoconvertscale`) et lit la taille source sur son entrée. Les rapports
  donnent cette taille source, non celle de l'image réduite : l'arrondi de l'image
  réduite changeait le ratio, donc le cadre calculé par l'interface, donc la taille
  demandée, et la vidéo et sa barre tremblaient. Le puits suit l'horloge audio et jette une image plutôt que
  de retarder le son si Oxyde prend du retard (`max-buffers=1 drop=true`). Les images
  sont mises à l'échelle à la taille affichée en pixels physiques (3840 px au plus,
  sans agrandissement), renégociée 150 ms après la fin d'un redimensionnement. En
  pause, l'image de pré-roulement n'est envoyée qu'une fois ; une image est identifiée
  par son tampon et son horodatage, car les pools réutilisent les tampons. Rapports
  toutes les 100 ms au format de Windows (25 octets), suivis de la dernière recherche
  appliquée. La fin de lecture reste arrêtée ; une recherche en repart.
- `platform/linux_media.rs` (côté parent) : commandes de 21 octets (pause, recherche,
  taille), envoyées à chaque changement et relevées toutes les 20 ms. Les images sont
  vérifiées (3840 × 3840 au plus, longueur exacte) puis déposées dans
  `ViewerMediaControl::frame`, avec un `Notify`. Une position antérieure à la
  dernière recherche n'est pas publiée, pour que la barre ne recule pas. Sans
  rapport pendant 15 s, la session échoue.
- Interface : la tâche de la session attend une image ou 80 ms, crée la
  `RenderImage`, et libère la précédente du GPU (`drop_image`), y compris après la
  fin de la session. `VideoLayout` peint l'image dans le cadre de la vidéo. En
  plein écran, la barre flottante est peinte dans la fenêtre du Viewer, par-dessus
  l'image, au lieu d'une fenêtre séparée que Wayland ne sait pas placer. Elle
  disparaît sans fondu de sortie. Les boutons superposés à la vidéo sont aussi
  peints dans la fenêtre, comme pour les images. Leur visibilité suit les
  événements de souris de GPUI : la position globale du pointeur, que Windows
  interroge toutes les 100 ms, n'existe pas sous Wayland.
- Plafonds : 1 Gio de mémoire privée pour le worker. Sans autre réglage, les
  décodeurs logiciels lancent un fil par cœur, et chaque fil garde des images : sur
  32 cœurs, 760 Mio pour une vidéo HEVC 4K, puis des allocations refusées, d'où des
  images corrompues, puis l'échec de la lecture. `Player` règle donc à 4 les fils
  (`max-threads`, `n-threads`) de chaque élément que `playbin3` ajoute
  (`element-setup`), survol compris. Cette vidéo se lit alors à 30 images par
  seconde avec 490 à 560 Mio, y compris à 3840 × 2160. Le décodeur seul atteint 85
  images par seconde.

## Limites

- Le confinement n'est pas celui du LPAC Windows. Le noyau reste exposé aux appels
  système autorisés. Le lecteur du Viewer parle au serveur de son, qui accepte aussi
  la capture : c'est le même accès qu'une application Flatpak autorisée à utiliser
  PulseAudio. Sans bubblewrap utilisable, les workers ont les droits de
  l'utilisateur.
- Sans bac à sable, le mode vidéo exige un chemin UTF-8, comme sous Windows.
- Les images du lecteur traversent un tube puis une texture GPU neuve par image :
  environ 110 Mio/s en 720p à 30 images par seconde.

## Tests

`cargo test --offline --lib -- linux_sandbox linux_pdf linux_video linux_media linux_preview linux_archive`
et `cargo test --offline --test linux_archive_workers`, seul à exercer le parcours de
production des archives : les tests unitaires compilent les codecs locaux.
Bac à sable : le worker lit son document et ne voit pas un fichier voisin ; sockets
réseau, `unshare` et `ptrace` refusés ; seul le lecteur atteint le serveur de son.
Décodage : rendu, rotation, pages hors
document, fichiers endommagés, échelle et forme des vidéos, boucle, annulation ; lecture à la
taille du Viewer, pause, recherche, fin, son seul. Les
vidéos sont encodées par `gst-launch-1.0` pendant le test (ignoré sans encodeur).
Les tests côté parent lancent le vrai worker et exigent d'abord
`cargo build --offline --bins`.
