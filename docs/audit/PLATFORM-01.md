# Audit PLATFORM-01

## Fichiers lus en entier

- [x] `src/platform/built_binary.rs`
- [x] `src/platform/clipboard.rs`
- [x] `src/platform/file_clipboard.rs`
- [x] `src/platform/file_drag.rs`
- [x] `src/platform/file_names.rs`
- [x] `src/platform/hardware.rs`
- [x] `src/platform/icons.rs`
- [x] `src/platform/linux_access.rs`
- [x] `src/platform/linux_archive_io.rs`
- [x] `src/platform/linux_archive_list.rs`
- [x] `src/platform/linux_cache.rs`
- [x] `src/platform/linux_download.rs`
- [x] `src/platform/linux_extended_attributes.rs`
- [x] `src/platform/linux_file_clipboard.rs`
- [x] `src/platform/linux_file_clipboard/tests.rs`
- [x] `src/platform/linux_file_clipboard/wayland.rs`
- [x] `src/platform/linux_file_copy.rs`
- [x] `src/platform/linux_file_names.rs`
- [x] `src/platform/linux_file_tree.rs`
- [x] `src/platform/linux_gio.rs`
- [x] `src/platform/linux_gio/applications.rs`
- [x] `src/platform/linux_gio/library.rs`
- [x] `src/platform/linux_gio/servers.rs`
- [x] `src/platform/linux_gio/tests.rs`
- [x] `src/platform/linux_gpu.rs`
- [x] `src/platform/linux_gpu/compositor.rs`
- [x] `src/platform/linux_gstreamer.rs`
- [x] `src/platform/linux_gstreamer/library.rs`

## Constats

### Élevé

1. `src/platform/linux_file_clipboard/wayland.rs:16` (et `linux_file_clipboard.rs:69-70`, `:21`). Élevé, plausible. `wl-copy` est lancé sans vérifier qu'une session Wayland existe : si le lancement réussit, l'écriture sur stdin réussit presque toujours avant que le processus échoue, si bien que l'interface n'écrit plus les chemins en texte. Scénario : session X11 (GNOME sur Xorg, Xfce, Cinnamon, MATE) avec wl-clipboard installé. `wl-copy` se termine aussitôt (« Failed to connect to a Wayland server »), `served()` renvoie faux, et `current()` comme `has_file_format()` ignorent la sélection : copier, couper puis coller des fichiers ne fonctionne plus du tout, même entre deux fenêtres d'Oxyde.

### Moyen

2. `src/platform/linux_file_clipboard/wayland.rs:17` (et `linux_file_clipboard.rs:70`). Moyen, plausible. Seul le type `x-special/gnome-copied-files` est proposé (ni `text/uri-list` ni texte brut), et le repli texte est désactivé dès que `wl-copy` sert. Scénario : sous KDE Plasma Wayland, on copie dans Oxyde puis on colle dans Dolphin, ou bien on colle dans un éditeur de texte ou un terminal. Rien n'est collé, alors que Nautilus propose aussi ces deux types.
3. `src/platform/linux_access.rs:12-13`. Moyen, confirmé (latent). `O_NOFOLLOW` (0x20000) et `O_DIRECTORY` (0x10000) sont codés en dur avec leurs valeurs x86. Sur arm et aarch64, ces valeurs correspondent à `O_LARGEFILE` et `O_DIRECT`. Scénario : un build depuis les sources sur aarch64 (Raspberry Pi, Asahi). `hold_directory(follow = false)` suit alors un lien symbolique et ne vérifie plus qu'il s'agit d'un dossier, ce qui casse la garantie anti-lien des opérations qui tiennent ce descripteur.
4. `src/platform/linux_cache.rs:19-21`. Moyen, confirmé (latent). Même défaut, alors que le commentaire affirme que ces valeurs sont partagées par arm et aarch64 : c'est vrai pour les commandes `F_*`, faux pour `O_NOFOLLOW` et `O_DIRECTORY`. Scénario : sur aarch64, un lien placé dans le cache après l'inventaire est suivi à l'ouverture pour le nettoyage. Le bail et les contrôles de type portent alors sur la cible, et `O_DIRECT` peut faire échouer l'ouverture (EINVAL sur tmpfs).
5. `src/platform/linux_archive_list.rs:31` (et `:49`, `:59`). Moyen, plausible. `list` et `verify` prennent `SLOT` par un verrou bloquant, et leur point de contrôle renvoie toujours Ok : une liste devenue inutile ne peut pas être annulée et garde l'emplacement jusqu'à 120 s. Scénario : on ouvre une grosse archive lente à analyser, puis on navigue ailleurs et on ouvre une autre archive. La seconde liste, ou la vérification d'un mot de passe, attend la fin de la première.
6. `src/platform/linux_file_tree.rs:79`. Moyen, plausible. La frontière de système de fichiers ne compare que `st_dev`, et un montage lié (bind mount) d'un dossier du même système de fichiers a le même `st_dev`. Scénario : un dossier lié par `mount --bind ~/data ~/projet/data`, puis une suppression définitive de `~/projet`. Le parcours entre dans le montage et vide `~/data` ; seul le `rmdir` du point de montage échoue (EBUSY).
7. `src/platform/linux_file_tree.rs:117`. Moyen, plausible. La racine sélectionnée n'est pas comparée au périphérique de son parent tenu : si elle est elle-même un point de montage, tout le volume monté est vidé avant l'échec final. Scénario : suppression définitive de `~/mnt/sauvegarde`, où est monté un disque externe. Tout son contenu est effacé, ce qui contredit l'invariant « un autre système de fichiers n'est jamais parcouru ».
8. `src/platform/linux_gio/servers.rs:28` (et `:96`, `:106`). Moyen, plausible. L'erreur de chargement du fichier de signets est ignorée, puis `remember` réécrit le fichier depuis un objet vide. Scénario : un `~/.config/gtk-4.0/servers` illisible ou mal formé (écrit par une autre version de GTK, ou tronqué), puis une connexion à un serveur. Toute la liste des serveurs récents de Nautilus est remplacée par une seule entrée. La lecture-modification-écriture, sans verrou, peut aussi perdre une entrée ajoutée en parallèle par Nautilus.
9. `src/platform/clipboard.rs:165`. Moyen, confirmé. Le presse-papiers reste ouvert (la session est fermée en fin de fonction) pendant un `symlink_metadata` par chemin. Scénario : des fichiers copiés depuis un partage UNC lent ou injoignable. Chaque stat peut attendre plusieurs secondes de délai réseau, et pendant ce temps aucune autre application ne peut lire ni écrire le presse-papiers Windows.

### Faible

10. `src/platform/linux_file_clipboard.rs:82`. Faible, confirmé. `has_file_format` ne connaît que la sélection d'Oxyde. Scénario : des fichiers copiés dans Nautilus. Le bouton Coller de la barre supérieure reste désactivé, alors que Ctrl+V les colle grâce au repli `wl-paste`.
11. `src/platform/linux_file_clipboard/wayland.rs:42`. Faible, plausible. `wl-paste` est lancé avec `output()`, sans délai maximal. Scénario : l'application propriétaire du presse-papiers est figée. Le collage bloque indéfiniment le thread de travail qui le sert.
12. `src/platform/linux_cache.rs:66`. Faible, plausible. Un bail en écriture est obligatoire pour évincer une entrée, mais certains systèmes de fichiers le refusent (NFS en v2/v3 ou en écriture, et d'autres) avec EINVAL ou EAGAIN. Scénario : un répertoire personnel sur NFS. Aucune entrée du cache d'ouverture d'archives ne peut jamais être évincée.
13. `src/platform/linux_archive_io.rs:198`. Faible, plausible. Si le worker met plus d'une seconde à confirmer la pause, toute la durée de la pause compte dans le délai de l'opération. Scénario : une pause demandée pendant la compression d'un gros bloc LZMA2, puis laissée plusieurs minutes. L'opération échoue sur la limite de temps alors qu'elle était en pause.
14. `src/platform/linux_download.rs:38`. Faible, confirmé. Avec wget, `--https-only` ne s'applique qu'en mode récursif : une redirection vers HTTP est suivie. De plus, `~/.wgetrc` est lu (aucun équivalent du `-q` de curl) et aucune durée totale n'est bornée. Scénario : machine sans curl. Une redirection HTTP ou une option de `.wgetrc` modifie la récupération du manifeste ; la signature du paquet limite l'impact au contenu du manifeste.
15. `src/platform/linux_file_copy.rs:46` (et `:100`). Faible, plausible. Un échec de `set_permissions` est fatal après l'écriture complète des données. Or vfat et exfat refusent (EPERM) tout mode portant setgid ou sticky, et `mode()` ne retire ces bits que si le propriétaire ou le groupe diffèrent. Scénario : copie sur une clé FAT, montée au nom de l'utilisateur, d'un dossier setgid ou sticky de cet utilisateur. La restauration des attributs du dossier échoue et l'opération est rapportée en échec.
16. `src/platform/linux_file_tree.rs:88`. Faible, plausible. Chaque niveau de la pile garde deux descripteurs (le descripteur tenu et celui du `Dir`), et la limite RLIMIT_NOFILE n'est jamais relevée. Scénario : une arborescence de plusieurs centaines de niveaux, ou plusieurs workers en parallèle. La suppression s'arrête sur EMFILE et laisse un arbre partiellement supprimé.
17. `src/platform/linux_file_tree.rs:57`. Faible, plausible. Les entrées sont supprimées pendant la lecture du dossier, et certains systèmes de fichiers FUSE ou réseau (gvfsd-fuse, CIFS) renvoient des positions `readdir` instables dans ce cas. Scénario : suppression définitive d'un gros dossier sur un partage monté par GVfs. Des entrées sont sautées et le `rmdir` final échoue (ENOTEMPTY).
18. `src/platform/linux_gio.rs:384` (et `:391`, `:486`). Faible, plausible. `children()` monte et énumère de façon synchrone avec un drapeau d'annulation jamais levé et un cancellable nul, et `unmount()` attend sans cancellable ni délai. Scénario : un serveur SMB devenu injoignable. L'actualisation de la liste du réseau ou un démontage occupe un thread de travail pendant toute l'attente de GVfs, sans moyen d'annuler.
19. `src/platform/linux_gio/applications.rs:186`. Faible, confirmé. L'association (application par défaut, ou dernière utilisée) est enregistrée dans `mimeapps.list` avant le lancement. Scénario : « Toujours utiliser » avec une application dont le lancement échoue. Elle reste quand même l'application par défaut du type.
20. `src/platform/linux_file_names.rs:17`. Faible, plausible. Les montages FUSE insensibles à la casse (partages SMB via gvfsd-fuse, exfat-fuse) ont le magic FUSE et sont traités comme sensibles. Scénario : copie de « A.txt » vers un partage GVfs qui contient « a.txt ». La pré-vérification ne signale aucun conflit et l'échec n'arrive qu'à la publication, sans dialogue de conflit.
21. `src/platform/linux_extended_attributes.rs:24`. Faible, plausible. La boucle de `sized` n'a ni plafond ni point de contrôle quand ERANGE se répète. Scénario : un système de fichiers FUSE bogué dont la requête de taille sous-estime toujours la valeur. La copie tourne indéfiniment, sans pause ni annulation possible.
22. `src/platform/clipboard.rs:139`. Faible, confirmé. Un échec d'`OpenClipboard` donne une liste vide, sans réessai ni erreur. Scénario : un gestionnaire de presse-papiers tient brièvement le presse-papiers. Ctrl+V ne fait rien, sans aucun message.
23. `src/platform/hardware.rs:46` (et `:80`, `:146`). Faible, plausible. La lettre de lecteur est prise par une tranche d'octets, alors que la vérification porte sur le deuxième caractère. Scénario : un chemin dont le premier caractère n'est pas ASCII, suivi de « : » (« é:\… »). La tranche coupe un caractère UTF-8 et le thread panique.
24. `src/platform/hardware.rs:79`. Faible, plausible. Seuls la lettre de lecteur ou l'hôte UNC identifient le disque. Scénario : un volume monté dans un dossier (`C:\Montage\Disque2`) reçoit l'identifiant du disque de C:, et les chemins `\\?\Volume{GUID}\` reçoivent 0. La limitation d'I/O par disque sérialise des disques distincts ou parallélise un même disque dur.
25. `src/platform/icons.rs:58`. Faible, plausible. Le cache mémoire des icônes n'est indexé que par le chemin, sans date de modification ni normalisation de la casse. Scénario : un .exe, un .ico ou un .lnk remplacé au même chemin garde son ancienne icône jusqu'à une purge ou au recyclage du worker. « C:\Dossier » et « c:\dossier » occupent deux entrées.
26. `src/platform/icons.rs:50`. Faible, plausible. L'icône générique de dossier interroge le vrai chemin `C:\Windows`, sans les attributs de fichier. Scénario : Windows installé sur un autre lecteur, ou C: absent. L'extraction de l'icône générique de dossier échoue.

### Optimisation

27. `src/platform/hardware.rs:61` (appelé dans `commands/duplicates.rs:330,385,440` et `commands/search.rs:520`). Optimisation, confirmé. Chaque appel ouvre le volume, émet un IOCTL puis ferme la poignée, et il est fait pour chaque fichier, sans cache par lecteur. Scénario : la recherche de doublons ou la recherche de contenu sur des centaines de milliers de fichiers fait autant d'ouvertures de `\\.\X:`.
28. `src/platform/linux_gio/applications.rs:153`. Optimisation, confirmé. `icon_file` teste jusqu'à (thèmes × 10 tailles × 2 extensions + pixmaps) chemins par application, pour chaque application de la liste « Ouvrir avec », sans mémorisation. Scénario : avec plusieurs centaines d'applications installées et cinq ou six bases XDG, chaque ouverture du choix d'application fait des dizaines de milliers de `stat`.

<!-- LOT TERMINÉ -->
