# Audit PLATFORM-04

Fichiers lus en entier :

- [x] `src/platform/mod.rs` (1371)
- [x] `src/platform/network.rs` (420)
- [x] `src/platform/quick_access.rs` (14)
- [x] `src/platform/remote.rs` (66)
- [x] `src/platform/thread_priority.rs` (50)
- [x] `src/platform/thumbnails.rs` (814)
- [x] `src/platform/video_thumbnail.rs` (78)
- [x] `src/platform/viewer_media.rs` (198)
- [x] `src/platform/volumes.rs` (81)
- [x] `src/platform/worker_test_support.rs` (151)

Aucun constat critique ou élevé. `quick_access.rs`, `volumes.rs`, `video_thumbnail.rs` et `worker_test_support.rs` n'appellent aucune remarque.

## Constats

### Moyen

- `src/platform/thumbnails.rs:173` | moyen | confirmé | Le cache JPEG des miniatures supprime le canal alpha (conversion en RGB à l'écriture, relecture en JPEG), donc l'image rendue à la relecture diffère de celle de la première génération. Scénario : sous Linux, une miniature de PNG ou WebP à fond transparent s'affiche d'abord avec sa transparence, puis, une fois relue depuis le cache (autre session, ou éviction mémoire), avec les pixels transparents remplacés par leur couleur brute, souvent noire ou blanche.

- `src/platform/thumbnails.rs:734` | moyen | plausible | L'analyse des tailles de pages PDF écarte silencieusement toute ligne invalide (taille nulle, non finie, ligne mal formée) au lieu de la remplacer par une valeur par défaut, ce qui décale l'indice des pages suivantes par rapport au document. Scénario : un PDF dont une page intermédiaire a une MediaBox vide ou dégénérée donne une liste plus courte que le nombre de pages. Le Viewer affiche alors le ratio d'une autre page pour chaque page suivante, et les dernières pages deviennent inaccessibles, puisque le nombre de pages est dérivé de cette liste.

- `src/platform/thumbnails.rs:143` | moyen | confirmé | Le cache de miniatures sous Linux n'a ni quota ni éviction : l'écriture se limite à un fichier temporaire suivi d'un renommage (`linux_impl.rs:309-341`), contrairement à Windows qui applique un quota (`windows_preview_quota`). Les clés contiennent la date de modification et la taille, donc chaque modification d'un fichier laisse une entrée orpheline. Scénario : parcours répété de gros dossiers d'images ou de vidéos pendant des mois, avec un dossier de cache qui grossit sans limite, seulement vidé par l'action manuelle de nettoyage du cache.

- `src/platform/mod.rs:794` | moyen | plausible | Sur Linux, l'implémentation par défaut de la suppression suivie ignore la preuve d'identité qu'elle reçoit et supprime par chemin. Windows la vérifie à l'envoi. Scénario : un Rétablir (redo) de suppression de l'historique contrôle l'identité en amont, puis le fichier est remplacé avant l'appel de suppression. L'objet remplaçant part alors à la corbeille sans contrôle (récupérable, mais ce n'est pas l'élément de l'historique), et aucun effet n'est enregistré dans le reçu.

### Faible

- `src/platform/thumbnails.rs:116` | faible | plausible | La clé de cache retombe sur une date nulle quand la date de modification est absente ou antérieure à 1970, et elle contient le chemin tel quel, sensible à la casse. Scénario : sur un système de fichiers qui ne fournit pas de date fiable, une modification de même taille garde la vignette périmée. Sous Windows, `C:\A.png` et `c:\a.png` produisent deux entrées de cache distinctes pour un même fichier.

- `src/platform/thumbnails.rs:87` | faible | confirmé | Le format d'image est déduit uniquement de l'extension, sans repli sur le contenu. Scénario : une image sans extension, ou nommée avec une extension fausse (typique des téléchargements et des fichiers sous Linux), échoue en « format inconnu » ou en erreur de décodage alors que `with_guessed_format` aurait fonctionné.

- `src/platform/thumbnails.rs:230` | faible | confirmé | Sous Linux, la miniature Office renvoie l'erreur du premier candidat (`docProps/thumbnail.jpeg`) sans essayer le second, alors que la version Windows poursuit. Scénario : un document dont l'aperçu JPEG est corrompu mais qui contient une vignette PNG valide n'a pas de miniature sous Linux.

- `src/platform/mod.rs:323` | faible | plausible | La détection WSL ne reconnaît que `\\wsl.localhost`, pas l'ancienne racine `\\wsl$`. Scénario : un chemin collé ou renvoyé par un outil qui utilise `\\wsl$\Ubuntu\...` (versions de Windows 10 antérieures à l'introduction de `wsl.localhost`, scripts existants) n'est pas traité comme WSL : le fil d'Ariane, l'icône et les exclusions d'expansion le traitent comme un partage réseau ordinaire.

- `src/platform/thread_priority.rs:17` | faible | confirmé | Sous Linux, l'application de priorité est un no-op (`linux_impl.rs:750`) : les modes Arrière-plan, Normal et Turbo n'ont aucun effet sur les copies, suppressions et calculs de tailles. Scénario : une grosse copie en mode normal sous Linux concurrence l'interface au lieu de tourner à faible priorité CPU et E/S, contrairement à Windows.

- `src/platform/viewer_media.rs:191` | faible | plausible | `seek` écrit immédiatement la position demandée, mais du côté Windows les rapports du worker n'ont pas d'identifiant de seek et réécrivent la position sans condition (`windows_viewer_media.rs:105`), alors que Linux filtre les rapports périmés. Scénario : l'utilisateur déplace le curseur de lecture alors que plusieurs rapports antérieurs au seek sont déjà dans le tuyau (jusqu'à huit lus d'un coup). La barre de progression revient brièvement à l'ancienne position avant de rejoindre la nouvelle.

- `src/platform/network.rs:112` | faible | confirmé | Les erreurs d'énumération sont avalées : la boucle Shell s'arrête à la première erreur de `Next`, et l'énumération des connexions arrête sur `ERROR_MORE_DATA` ou toute erreur sans la signaler (lignes 214-216). Scénario : un voisinage réseau lent ou en erreur donne une liste partielle présentée comme complète, sans message.

- `src/platform/network.rs:181` | faible | plausible | Le tampon d'énumération est un `Vec<u8>` (alignement 1) réinterprété en tableau de `NETRESOURCEW`, qui exige un alignement pointeur (idem lignes 258-260). Scénario : comportement indéfini en théorie. En pratique l'allocateur renvoie des blocs alignés sur 16 octets, donc sans effet observable aujourd'hui, mais rien ne le garantit.

- `src/platform/network.rs:236` | faible | plausible | L'énumération des partages d'un serveur n'a ni délai ni annulation. Scénario : un serveur SMB éteint ou filtré par un pare-feu bloque `WNetOpenEnumW` plusieurs dizaines de secondes. Chaque navigation vers ce serveur occupe un thread du pool bloquant, et la navigation quittée ne peut pas l'interrompre.

- `src/platform/network.rs:187` | faible | plausible | Les connexions actives sont transformées en « serveurs » en prenant le premier segment du nom distant, ce qui inclut les cibles WebDAV du type `\\hôte@SSL\DavWWWRoot`. Scénario : un lecteur WebDAV monté crée une entrée « hôte@SSL » dans le voisinage réseau dont l'ouverture échoue.

- `src/platform/network.rs:320` | faible | confirmé | Le nom d'utilisateur du montage de lecteur est écrit dans le journal (niveau info), et le mot de passe reste en clair dans des `String` et `Vec<u16>` non effacés jusqu'à leur libération. Scénario : un journal partagé pour un diagnostic révèle les comptes utilisés. Une image mémoire de processus contient le mot de passe.

- `src/platform/remote.rs:33` | faible | confirmé | `RemoteAnswer` dérive `Debug` alors que sa variante `Login` contient le mot de passe. Scénario : tout futur `{:?}` sur la réponse, un journal ou un `assert` de test, écrit les identifiants en clair. Aujourd'hui aucun appel de ce genre n'a été repéré.

### Optimisation

- `src/platform/thumbnails.rs:172` | optimisation | confirmé | Chaque écriture de miniature en cache clone l'image RGBA puis la convertit en RGB (copie inutile) et, côté Linux, force un `sync_all` avant le renommage (`linux_impl.rs:333`). Scénario : l'ouverture d'un dossier de milliers d'images enchaîne autant de synchronisations disque, coûteuses sur disque dur ou ext4, pour un cache purement régénérable.

- `src/platform/thumbnails.rs:360` | optimisation | confirmé | La taille de sortie vidéo est toujours portée à 640 sur le grand côté, y compris quand la source est plus petite : la négociation native ne réduit que les grandes sources (lignes 484-488), mais la mise à l'échelle bilinéaire en Rust agrandit ensuite les petites. Scénario : l'aperçu au survol d'une vidéo 320x240 est agrandi en 640x480 sur le CPU pour chaque image, avec quatre fois plus de données à transmettre par le tuyau du worker.

<!-- LOT TERMINÉ -->
