# Aperçus PDF et vidéo sous Linux

**Statut : implémenté, vérification visuelle en attente** (T09 de l'[audit de
portabilité](../reports/2026-10-26-linux-portability-audit.md), hors lecteur intégré).
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
- Mort avec le parent : le worker surveille son parent toutes les 500 ms et se tue
  s'il a changé. `PR_SET_PDEATHSIG` ne convient pas : le signal suit le fil qui a
  lancé le worker, et un pool peut retirer ce fil.

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

## Limites

- Les workers ne sont **pas confinés** : ils ont les droits de l'utilisateur
  (M09/R5). Ils isolent les plantages, pas les fuites de données.
- Le mode vidéo du worker exige un chemin UTF-8, comme sous Windows.
- Le lecteur audio et vidéo du Viewer reste indisponible.

## Tests

`cargo test --offline --lib -- linux_pdf linux_video` : rendu, rotation, pages hors
document, fichiers endommagés, échelle et forme des vidéos, boucle, annulation. Les
vidéos sont encodées par `gst-launch-1.0` pendant le test (ignoré sans encodeur).
Les tests côté parent lancent le vrai worker et exigent d'abord
`cargo build --offline --bins`.
