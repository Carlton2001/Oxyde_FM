# Tests à réaliser sous Linux

Liste tenue pendant la remédiation, à exécuter à la fin de l'audit. Chaque lot ajoute ses tests ; cocher au fur et à mesure.

## Préalable

- [x] `scripts/check-linux.sh` : compilation et tests Linux (le code Linux du lot 1 n'a pas été compilé sous Windows). Fait le 2026-11-18 sous Fedora 45 WSL, Rust 1.100.0 : aucun avertissement ; `cargo test` : 1 échec intermittent préexistant (`pressure_evicts_unmodified_closed_entries_before_expiry`, environ 1 lancement sur 15), le reste passe ; `linux_permissions` sous `unshare` : 15 tests passés.

## Environnement WSL

Distribution `FedoraLinux-45`, utilisateur `biluf`, clone dans `~/oxyde` (ext4, pas `/mnt/c` : `mkfifo` et les liens n'y ont pas le comportement Linux). Mettre à jour avec `git pull origin audit-fixes` depuis le dépôt Windows. Lancer l'interface sous WSLg (Wayland, rendu llvmpipe) sur un profil isolé :

```bash
cd ~/oxyde && export CARGO_TARGET_DIR=~/.cache/oxyde-gpui/target && cargo build --offline --bins
P=~/oxyde-test; XDG_CONFIG_HOME=$P/config XDG_CACHE_HOME=$P/cache XDG_DATA_HOME=$P/data TMPDIR=$P/tmp $CARGO_TARGET_DIR/debug/oxyde
```

Restent sur une vraie machine : propriétés de corbeille GNOME/KDE, répétition de touches Wayland/X11 (E9), volumes, GPU.

## Lot 1

### E8 : confirmation de la purge de la corbeille (`28e32428`)

- [ ] Vue corbeille, sélection puis Suppr : confirmation affichée si « Confirmer la suppression » est actif, purge directe sinon.
- [ ] Menu contextuel du fond de la corbeille, « Vider la corbeille » : confirmation toujours affichée, même préférence désactivée.
- [ ] Propriétés de la corbeille (GNOME ou KDE), bouton « Vider la corbeille » : confirmation affichée ; Annuler ne supprime rien ; fermer les propriétés pendant la question puis confirmer vide quand même.

### E9 : choix de conflit (`1cc5172d`)

- [ ] Plusieurs conflits en mode simple, double-clic sur Remplacer : seul le premier conflit est remplacé, le suivant reste affiché.
- [ ] Wayland : Entrée maintenue sur Remplacer ne répond qu'au premier conflit.
- [ ] X11 : même test. Limite connue : X11 ne signale pas la répétition de touche dans GPUI, Entrée maintenue peut encore enchaîner les conflits. Noter le résultat.

### M1 : sélection masquée par un filtre (`d4743871`)

- [ ] Ctrl+A, filtre de nom qui ne laisse qu'un fichier, puis Couper/Coller, Copier, Compresser et Suppr : seul le fichier visible est concerné.
- [ ] Sélectionner un fichier, le supprimer depuis un terminal, puis Copier : le chemin disparu n'est pas copié.

### E5 : liens symboliques cassés (`2da636fa`, `a16779c0`)

- [ ] `ln -s absent photos.zip` puis compresser le dossier `photos` au même endroit : archive publiée sous `photos (1).zip`, pas de processeur à 100 %, opération terminée.
- [ ] `ln -s absent notes` à côté de `notes.gz`, puis extraire : fichier publié sous `notes (1)`.
- [ ] `ln -s absent dossier` à côté de `dossier.zip`, puis extraire : dossier créé sous `dossier (1)`.
- [ ] Copier `rapport.pdf` vers un dossier qui contient un lien cassé `rapport.pdf` : la boîte de conflit s'ouvre ; Ignorer garde le lien ; Remplacer est refusé proprement (« pas un fichier ordinaire »), sans arrêter le reste de la copie.

### M100 : tubes nommés (lot 2)

- [ ] `mkfifo .hidden` dans un dossier, l'ouvrir : le listage s'affiche, sans blocage.
- [ ] `mkfifo x.zip` dans un dossier, l'ouvrir et le survoler : pas de blocage, refus propre.
- [ ] `mkfifo` dans l'arborescence d'une recherche de contenu : la recherche se termine (Finished).
- [ ] `mkfifo tube.txt`, survol avec l'aperçu actif : pas de gel.
- [ ] Copier ou déplacer un dossier contenant un tube : pas de blocage.
- [x] `cargo check --tests` et `cargo test open_regular` sous Linux (modules linux_* non compilés sous Windows). Fait sous WSL le 2026-11-18.

### Lot 3 : racines de l'application et préfixes (`ca25109e`, `630b6e3d`)

- [ ] `~/.cache` remplacé par un lien symbolique vers un autre dossier (`mv ~/.cache ~/cache-reel && ln -s ~/cache-reel ~/.cache`), puis ouvrir un fichier depuis une archive et afficher des aperçus : tout fonctionne, le cache est écrit dans `~/cache-reel/Oxyde`.
- [ ] Recherche de doublons sur un dossier ordinaire : résultats inchangés (le retrait du préfixe `\\?\` ne touche pas les chemins Linux, y compris un nom non UTF-8).
- [x] `cargo check --tests` sous Linux (`app_paths::real_path` et `path_text::without_verbatim_prefix` compilés pour la première fois). Fait sous WSL le 2026-11-18.

### Lot 4 : archives

- [x] `cargo test --no-fail-fast`, `check-linux.sh` et `linux_permissions` sous `unshare` : verts sous WSL le 2026-11-18.
- [x] Test intermittent `pressure_evicts_unmodified_closed_entries_before_expiry` (et `active_workspaces_...`, même cause) : 1 échec sur 40 lancements complets avant `097edead`, 0 sur 120 après (WSL, 2026-11-18).
- [ ] M35 (`a4b70e91`) : un ZIP ou TAR contenant `aux.txt`, `nom:flux`, `fin.` et `a<b` s'ouvre et s'extrait sous Linux ; `../x` et un nom à caractère de contrôle restent refusés.
- [ ] M36 (`2b5468fb`) : un TAR contenant `Readme` et `README` s'extrait avec les deux fichiers.
- [ ] M38 (`33ad76b1`) : extraire un ZIP créé sous Windows : les dates des fichiers sont celles d'origine (fuseau local), y compris dans le bac à sable bubblewrap ; idem avec `TZ=America/New_York` exporté avant de lancer Oxyde.
- [ ] M44 (`0d9a6857`) : archive chiffrée ouverte par un lien symbolique (`ln -s ~/coffre ~/lien`, ouvrir `~/lien/secret.7z`) : le mot de passe est demandé une seule fois, extraction et ouverture d'un membre fonctionnent.
- [ ] E4 (`6dc5ac9b`) : `git archive --format=tar.gz HEAD` d'un dépôt, puis `tar --sparse -cf creux.tar fichier-creux` : les deux s'ouvrent et s'extraient, fichier creux à la bonne taille.
- [ ] M40 (`dc312d8a`) : ISO d'installation de Windows 11 : montée par le système au lieu d'afficher seulement `README.TXT` ; une ISO Linux ordinaire reste parcourue comme dossier virtuel.
- [ ] M41 (`0eb561f8`) : sans objet sous Linux (format v2 du cache), vérifier seulement que l'ouverture d'un fichier depuis une archive marche toujours.

### Lot 5 : composition IME (E10, `4e639422`)

- [x] `check-linux.sh` et tests `ui::components::input` sous WSL le 2026-11-18 (code commun, sans partie propre à Linux).
- [ ] IBus (GNOME, Wayland) avec Anthy ou Mozc : taper `kana` dans le renommage, la barre d'adresse et le filtre : préédition soulignée qui se remplace, puis `かな` (ou sa conversion) inséré une seule fois.
- [ ] Fcitx 5 sous X11 (KDE ou Xfce) avec Pinyin : `nihao` puis Espace : `你好` une seule fois.
- [ ] Échap pendant une préédition : la préédition disparaît, le texte d'avant reste, le renommage n'est pas annulé.
- [ ] Cliquer hors du champ de renommage pendant une préédition : le nom validé contient le texte souligné tel qu'affiché.
- Limite connue : passer d'un champ à un autre de la même fenêtre pendant une préédition laisse la préédition dans le premier comme texte saisi, et le système continue la composition dans le second.

### Lot 6 : moyens restants (en cours)

- [ ] M30 (`667cc2d7`) : ouvrir `smb://serveur/partage/dossier` dans la barre d'adresse, cliquer le segment parent : cible `smb://serveur/...` (et non `smb:/`) ; un dossier local dont le nom finit par deux-points reste un dossier.
- [ ] M20/M21 (sélecteur) : Ctrl+L puis clic sur une entrée : sélection conservée ; taper un filtre puis Entrée tout de suite : rien ne s'ouvre avant la fin du filtrage.
- [ ] M59 (`365dc639`) : lister `/tmp` pendant qu'un programme crée et supprime des fichiers : pas d'erreur d'onglet.
- [ ] M64 : « Tout développer » sur `/` reste borné (20 000 dossiers) ; exclusions faites au lot 7 (voir plus bas).
- M22, M54, M61 : faits au lot 7 (voir plus bas).

## Lot 6 (suite)
- M83 : corrigé au lot 9 (voir plus bas).
- WM8 : `is_cloud_placeholder` renvoie false hors Windows ; la recherche de contenu doit se comporter comme avant.

### Lot 9 : restes du lot 6 (Opus)

- [ ] M83 : ouvrir une grosse archive lente (7z solide de plusieurs Go sur une clé USB), puis aller tout de suite dans un autre dossier du même onglet et ouvrir une autre archive : la seconde s'affiche sans attendre la fin de la première (le worker de la première est arrêté). `linux_archive_list::list` prend maintenant un point d'annulation.
- [ ] WF6 (code commun) : `MountedDiskImage` a un champ `aliases`, vide sous Linux ; une image montée par udisks reste reconnue (menu Démonter proposé).
- [ ] Compiler `cargo check --tests` et lancer `cargo test linux_archive_list` sous WSL.

### Lot 7 : session Linux (Opus)

Fait sous WSL Fedora 45 le 2026-11-18 : `cargo test --no-fail-fast` (798 + 619 tests), `check-linux.sh`, tests de montage sous `unshare` (52), tests udisks2 sous `sudo`, vfat réel, `/mnt/c` (9p, sans bail). Reste à faire sur une vraie machine :

- [ ] E6 (`374c5078`) : le journal indique « open file limit raised from 1024 to … » ; copier un dossier de plus de 1 000 sous-dossiers réussit ; un programme ouvert depuis Oxyde (`ulimit -n` dans un terminal ouvert par Oxyde) voit 1024.
- [ ] E7 et LM4 (`de492644`, `dec384fb`), session Wayland GNOME : copier des fichiers dans Oxyde, coller dans Nautilus ; couper puis coller : déplacés. Même chose vers Dolphin (KDE, `text/uri-list` et coupe KDE) et un sélecteur GTK 4. Coller dans un éditeur de texte : les chemins. Copier dans Nautilus, coller dans Oxyde. Un élément de la corbeille copié ne s'offre qu'en texte.
- [ ] E7 et E9, session X11 : copier puis coller entre deux onglets d'Oxyde ; coller dans un éditeur : les chemins en texte ; aucun processus `wl-copy` ; dans la boîte de conflit, maintenir Entrée sur Ignorer : un seul conflit traité (X11 ne signale pas la répétition).
- [ ] E10 : voir lot 5 (IBus sous GNOME Wayland, Fcitx 5 sous X11).
- [ ] M93 (`2f14fc1a`) : `sudo mount --bind ~/a ~/b/lien` puis supprimer définitivement `~/b` : refus sans toucher à `~/a` ; changer les permissions de `~/b` récursivement : `~/a` inchangé.
- [ ] M94 (`758ddbed`) : rendre `~/.config/gtk-3.0/servers` illisible (`chmod 000`) puis ajouter un serveur : le fichier n'est pas réécrit.
- [ ] M95 (`7303879a`) : monter une ISO réelle par Oxyde, supprimer le fichier ISO, démonter depuis la barre latérale : démontée.
- [ ] M98 (`7ef1e7ce`) : `du -sh ~/.cache/oxyde/previews` (ou le dossier d'aperçus) reste sous 256 Mio après le survol de nombreux fichiers.
- [ ] M99, M54, M61 (`0dfb1121`) : mettre à la corbeille sur une clé USB : entrée dans `.Trash-<uid>` de la clé, visible dans Nautilus ; Annuler la restaure ; recréer un fichier du même nom puis Annuler : l'autre fichier n'est pas touché ; Rétablir la renvoie.
- [ ] LM2 et LF1 (`8bc2c37b`) : dans une toolbox ou distrobox, puis sous NixOS : les aperçus d'archives et de vidéos marchent (bac à sable bubblewrap accepté, sinon repli sans bubblewrap signalé dans le journal).
- [ ] LM3 et LF3 (`b42afd99`) : Btrfs avec sous-volumes, sans UDisks2 (conteneur) : la barre latérale ne montre ni `/home` ni les sous-volumes système en double ; un partage `sshfs` apparaît sans geler.
- [ ] LF2 (`ed27a8ac`) : sous Debian/Ubuntu (paquet .deb) et Arch (paquet AUR) : la mise à jour annonce le bon gestionnaire, pas RPM.
- [ ] LF4 (`5ebf8330`) : copier `Readme` vers une clé exFAT montée par FUSE contenant `README` : conflit signalé avant la copie.
- [ ] LF4 (`0467c4eb`) : copier un dossier setgid (`chmod 2775`) vers une clé FAT : copie complète, sans erreur.
- [ ] LF4 (`6f5f0299`) : avec un dossier personnel NFS, ouvrir un fichier d'une archive : le journal indique « grants no file lease », le fichier s'ouvre depuis `/tmp/oxyde-<uid>` (mode 700).
- [ ] M64 (`00924c95`) : « Tout développer » sur `/` : `/proc`, `/sys`, `/dev`, `/run` et les partages GVfs ou NFS restent fermés, mais s'ouvrent un par un.
- [ ] M22 (`9eb9b039`) : depuis Firefox (portail, filtre « Images »), ouvrir un dossier de dizaines de milliers de fichiers : la liste apparaît tout de suite, les images arrivent par lots ; Retour puis Avant : immédiat.

Paquets installés dans la VM WSL pour ces tests : `wl-clipboard`, `udisks2` (service démarré), `dosfstools`.

### Lot 10 : reportés du lot 8 (Opus)

- [ ] Compiler `cargo check --tests` puis `cargo test` sous WSL (`git pull origin audit-fixes` une fois la branche poussée) ; lancer `check-linux.sh`.
- [ ] F94 (`3f80b0d0`) : supprimer de la corbeille Linux un élément déjà purgé par un autre programme : message « sélection indisponible », pas d'erreur brute.
- [ ] F106 (`dfa3e5bb`) : compresser un dossier qui contient `README` et `readme` : archive créée avec les deux.
- [ ] F134 (`d64b8acb`) : deux instances d'Oxyde (sélecteur de fichiers ouvert par le portail et fenêtre principale) : `oxyde.log` n'est pas tourné pendant que l'autre écrit.
- [ ] F128 (`6de8e9d5`) : `config.json` avec BOM ou un champ mal typé : réglages gardés, copie `config.json.invalid` créée.
- [ ] F91 (`d87732d2`) : rechercher un mot dans un fichier UTF-16 avec BOM (`iconv -t UTF-16`) : trouvé.

### Reprise des reportés (2026-11-19)

- [ ] Compiler et lancer `cargo test` sous WSL (code commun : recherche, annulation, aperçus, barre latérale).
- [ ] M62 (`455c629b`) : rechercher un mot placé après une ligne en Latin-1 (`printf 'caf\xe9\nmot\n'`) : trouvé.
- [ ] M29 (`65b51b17`) : survoler un fichier texte sur un partage NFS/SMB coupé, puis d'autres fichiers après 20 s : leurs aperçus reviennent.
- [ ] M63/M64 (`233c9d7e`) : « Tout développer » sur `/`, replier aussitôt : le dossier reste replié.
