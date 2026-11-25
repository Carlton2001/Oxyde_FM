# Tests à réaliser sous Linux

Liste tenue pendant la remédiation, à exécuter à la fin de l'audit. Chaque lot ajoute ses tests ; cocher au fur et à mesure.

## Préalable

- [x] `scripts/check-linux.sh` : compilation et tests Linux (le code Linux du lot 1 n'a pas été compilé sous Windows). Fait le 2026-11-18 sous Fedora 45 WSL, Rust 1.100.0 : aucun avertissement ; `cargo test` : 1 échec intermittent préexistant (`pressure_evicts_unmodified_closed_entries_before_expiry`, environ 1 lancement sur 15), le reste passe ; `linux_permissions` sous `unshare` : 15 tests passés.

## Environnement WSL (historique)

Les tests prévus sous WSL datent du développement sous Windows. Ils se font désormais en natif sur la machine Fedora ; seules les lignes datées « WSL » ci-dessous ont été faites là.

Distribution `FedoraLinux-45`, utilisateur `biluf`, clone dans `~/oxyde` (ext4, pas `/mnt/c` : `mkfifo` et les liens n'y ont pas le comportement Linux). Mettre à jour avec `git pull origin audit-fixes` depuis le dépôt Windows. Lancer l'interface sous WSLg (Wayland, rendu llvmpipe) sur un profil isolé :

```bash
cd ~/oxyde && export CARGO_TARGET_DIR=~/.cache/oxyde-gpui/target && cargo build --offline --bins
P=~/oxyde-test; XDG_CONFIG_HOME=$P/config XDG_CACHE_HOME=$P/cache XDG_DATA_HOME=$P/data TMPDIR=$P/tmp $CARGO_TARGET_DIR/debug/oxyde
```

Restent sur une vraie machine : propriétés de corbeille GNOME/KDE, répétition de touches Wayland/X11 (E9), volumes, GPU.

## Lot 1

### E8 : confirmation de la purge de la corbeille (`28e32428`)

- [x] Vue corbeille, sélection puis Suppr : confirmation affichée si « Confirmer la suppression » est actif, purge directe sinon.
- [x] Menu contextuel du fond de la corbeille, « Vider la corbeille » : confirmation toujours affichée, même préférence désactivée.
- [x] Propriétés de la corbeille (GNOME ou KDE), bouton « Vider la corbeille » : confirmation affichée ; Annuler ne supprime rien ; fermer les propriétés pendant la question puis confirmer vide quand même.

### E9 : choix de conflit (`1cc5172d`)

- [x] Plusieurs conflits en mode simple, double-clic sur Remplacer : seul le premier conflit est remplacé, le suivant reste affiché.
- [x] Wayland : Entrée maintenue sur Remplacer ne répond qu'au premier conflit.

### M1 : sélection masquée par un filtre (`d4743871`)

- [x] Ctrl+A, filtre de nom qui ne laisse qu'un fichier, puis Couper/Coller, Copier, Compresser et Suppr : seul le fichier visible est concerné.
- [x] Sélectionner un fichier, le supprimer depuis un terminal, puis Copier : le chemin disparu n'est pas copié. `commentaire : Le fichier disparait dans oxyde puisque le watcher fait son travail. Par contre l'icone Copier reste actif dans la navbar, Si je copie/colle, là j'ai le message d'erreur que le fichier n'existe plus. Est ce qu'on désactiverai pas les icones tout de suite pour un fichier qui n'existe plus ?`

### E5 : liens symboliques cassés (`2da636fa`, `a16779c0`)

- [x] `ln -s absent photos.zip` puis compresser le dossier `photos` au même endroit : archive publiée sous `photos (1).zip`, pas de processeur à 100 %, opération terminée. `Fait (nid mutter, 2026-11-19) : `photos (1).zip` créé, lien intact.`
- [x] `ln -s absent notes` à côté de `notes.gz`, puis extraire : fichier publié sous `notes (1)`. `Fait (nid mutter, 2026-11-19) : `notes (1)`.`
- [x] `ln -s absent dossier` à côté de `dossier.zip`, puis extraire : dossier créé sous `dossier (1)`. `Fait (nid mutter, 2026-11-19) : dossier `dossier (1)`.`
- [x] Copier `rapport.pdf` vers un dossier qui contient un lien cassé `rapport.pdf` : la boîte de conflit s'ouvre ; Ignorer garde le lien ; Remplacer est refusé proprement (« pas un fichier ordinaire »), sans arrêter le reste de la copie. `Fait (nid mutter, 2026-11-19) : boîte de conflit ouverte ; Remplacer refusé (« Copy target is not an ordinary file », en anglais dans l'interface, et la boîte l'appelle « Opération interrompue »), lien conservé, `autre.txt` copié avant. Le lien cassé est présenté comme « Plus volumineux » (6 o, taille du lien).`

### M100 : tubes nommés (lot 2)

- [x] `mkfifo .hidden` dans un dossier, l'ouvrir : le listage s'affiche, sans blocage. `Listage affiché sans blocage (nid mutter, 2026-11-19) ; l'affichage du fichier caché lui-même n'a pas été vérifié.`
- [x] `mkfifo x.zip` dans un dossier, l'ouvrir et le survoler : pas de blocage, refus propre. `Fait (nid mutter, 2026-11-19) : « Impossible d'ouvrir ce dossier », pas de blocage ; survol de `tube.txt` : infobulle « Impossible de charger cet aperçu ».`
- [x] `mkfifo` dans l'arborescence d'une recherche de contenu : la recherche se termine (Finished). `Fait (nid mutter, 2026-11-19) : recherche de contenu terminée, 1 résultat.`
- [x] `mkfifo tube.txt`, survol avec l'aperçu actif : pas de gel. `Fait (nid mutter, 2026-11-19) : pas de gel, mais le panneau Visionneuse reste sur « Chargement du fichier… » (12 s et plus) alors que l'infobulle indique déjà l'échec.`
- [x] Copier ou déplacer un dossier contenant un tube : pas de blocage. `Copie faite (nid mutter, 2026-11-19) : pas de blocage, arrêt avec « La source a été remplacée ou n'est plus un fichier ou dossier ordinaire », rien de laissé ; déplacement non testé.`
- [x] `cargo check --tests` et `cargo test open_regular` sous Linux (modules linux_* non compilés sous Windows). Fait sous WSL le 2026-11-18.

### Lot 3 : racines de l'application et préfixes (`ca25109e`, `630b6e3d`)

- [x] `~/.cache` remplacé par un lien symbolique vers un autre dossier (`mv ~/.cache ~/cache-reel && ln -s ~/cache-reel ~/.cache`), puis ouvrir un fichier depuis une archive et afficher des aperçus : tout fonctionne, le cache est écrit dans `~/cache-reel/Oxyde`. `Fait (nid mutter, 2026-11-19) avec `XDG_CACHE_HOME` pointant un lien symbolique (le vrai `~/.cache` n'a pas été touché) : cache écrit dans `cache-real/Oxyde/oxyde-archive-open`.`
- [x] Recherche de doublons sur un dossier ordinaire : résultats inchangés (le retrait du préfixe `\\?\` ne touche pas les chemins Linux, y compris un nom non UTF-8). Vérifié : nom non UTF-8, résultats inchangés.
- [x] `cargo check --tests` sous Linux (`app_paths::real_path` et `path_text::without_verbatim_prefix` compilés pour la première fois). Fait sous WSL le 2026-11-18.

### Lot 4 : archives

- [x] `cargo test --no-fail-fast`, `check-linux.sh` et `linux_permissions` sous `unshare` : verts sous WSL le 2026-11-18.
- [x] Test intermittent `pressure_evicts_unmodified_closed_entries_before_expiry` (et `active_workspaces_...`, même cause) : 1 échec sur 40 lancements complets avant `097edead`, 0 sur 120 après (WSL, 2026-11-18).
- [x] M35 (`a4b70e91`) : un ZIP ou TAR contenant `aux.txt`, `nom:flux`, `fin.` et `a<b` s'ouvre et s'extrait sous Linux ; `../x` et un nom à caractère de contrôle restent refusés. `Fait (nid mutter, 2026-11-19) : `aux.txt`, `nom:flux`, `fin.`, `a<b` listés et extraits ; `../x` et nom à caractère de contrôle refusés (« Le traitement de l'archive a échoué »).`
- [x] M36 (`2b5468fb`) : un TAR contenant `Readme` et `README` s'extrait avec les deux fichiers. `Fait (nid mutter, 2026-11-19) : `Readme` et `README` extraits.`
- [x] M38 (`33ad76b1`) : extraire un ZIP créé sous Windows : les dates des fichiers sont celles d'origine (fuseau local), y compris dans le bac à sable bubblewrap ; idem avec `TZ=America/New_York` exporté avant de lancer Oxyde. `Fait (nid mutter, 2026-11-19) : 2020-01-02 03:04 local conservé en fuseau par défaut et sous `TZ=America/New_York` ; bac à sable bubblewrap non vérifié séparément.`
- [x] M44 (`0d9a6857`) : archive chiffrée ouverte par un lien symbolique (`ln -s ~/coffre ~/lien`, ouvrir `~/lien/secret.7z`) : le mot de passe est demandé une seule fois, extraction et ouverture d'un membre fonctionnent. `Fait (nid mutter, 2026-11-19) : `lien -> coffre` avec un 7z à en-têtes chiffrés : un seul mot de passe, le membre s'ouvre. À noter : l'infobulle du membre dit « This preview could not be loaded ».`
- [x] E4 (`6dc5ac9b`) : `git archive --format=tar.gz HEAD` d'un dépôt, puis `tar --sparse -cf creux.tar fichier-creux` : les deux s'ouvrent et s'extraient, fichier creux à la bonne taille. `Fait (nid mutter, 2026-11-19) : `git archive` et TAR creux extraits, fichier à la bonne taille (50 Mio, mais écrit plein, sans trous).`
- [x] M40 (`dc312d8a`) : ISO d'installation de Windows 11 : montée par le système au lieu d'afficher seulement `README.TXT` ; une ISO Linux ordinaire reste parcourue comme dossier virtuel. `Partiel (nid mutter + clé exfat, 2026-11-19) : une ISO Joliet/Rock Ridge ordinaire est parcourue en dossier virtuel, avec les noms ISO 9660 courts (`IN.TXT`) ; ISO Windows 11 non disponible.` Vérifié : image UDF montée par le système, non listée comme ISO.
- [x] M41 (`0eb561f8`) : sans objet sous Linux (format v2 du cache), vérifier seulement que l'ouverture d'un fichier depuis une archive marche toujours. `Fait (nid mutter, 2026-11-19) : un membre de ZIP s'ouvre depuis l'archive.`

### Lot 5 : composition IME (E10, `4e639422`)

- [x] `check-linux.sh` et tests `ui::components::input` sous WSL le 2026-11-18 (code commun, sans partie propre à Linux).
- [ ] IBus (GNOME, Wayland) avec Anthy ou Mozc : taper `kana` dans le renommage, la barre d'adresse et le filtre : préédition soulignée qui se remplace, puis `かな` (ou sa conversion) inséré une seule fois.
- [ ] Échap pendant une préédition : la préédition disparaît, le texte d'avant reste, le renommage n'est pas annulé.
- [ ] Cliquer hors du champ de renommage pendant une préédition : le nom validé contient le texte souligné tel qu'affiché.
- Limite connue : passer d'un champ à un autre de la même fenêtre pendant une préédition laisse la préédition dans le premier comme texte saisi, et le système continue la composition dans le second.

### Lot 6 : moyens restants (en cours)

- [x] M30 (`667cc2d7`) : ouvrir `smb://serveur/partage/dossier` dans la barre d'adresse, cliquer le segment parent : cible `smb://serveur/...` (et non `smb:/`) ; un dossier local dont le nom finit par deux-points reste un dossier. `Fait (nid mutter, 2026-11-19) avec un partage local : serveur SMB en espace utilisateur (impacket smbserver, port 1445) monté par GVfs ; onglet ouvert sur smb://127.0.0.1:1445/share/dossier/sous, clic sur le segment « dossier » : le dossier parent du partage s'affiche (schéma conservé). Seconde moitié refaite : un dossier local `dossier:` (avec un sous-dossier) reste un dossier, le clic sur son nom dans le fil d'Ariane le rouvre, sans devenir `dossier://`. Refait avec le vrai FUSE de GVfs (Oxyde du nid lancé sur le bus de la session, nom `.Devel`, profil isolé : le `gvfsd-fuse` d'un bus imbriqué ne peut pas se monter, Oxyde retombait sur le listage GIO sans tailles ni dates, ce qui n'est pas un défaut). Taille, date, aperçu et visionneuse corrects. Deux défauts réels corrigés : (1) copier dans le partage échouait avec « Operation not supported (os error 95) » car `fchmod` est refusé par le FUSE de GVfs : `set_mode` l'accepte maintenant (copie, Annuler vérifiés) ; (2) supprimer proposait la corbeille puis échouait sur `/run/user/1000/gvfs/.Trash-1000` : Linux déclare maintenant `has_recycle_bin` faux sous le FUSE de GVfs, la boîte propose « Delete permanently » (vérifié) ; (3) les onglets dont le chemin contient `://` ne sont plus surveillés (avertissement répété).`
- [x] M20/M21 (sélecteur) : Ctrl+L puis clic sur une entrée : sélection conservée ; taper un filtre puis Entrée tout de suite : rien ne s'ouvre avant la fin du filtrage. Vérifié via le portail (OpenFile) : Ctrl+L puis clic garde la sélection ; filtre + Entrée valide le filtre, un second Entrée renvoie le fichier.
- [x] M59 (`365dc639`) : lister `/tmp` pendant qu'un programme crée et supprime des fichiers : pas d'erreur d'onglet. `Fait (nid mutter + clé exfat, 2026-11-19) : 40 cycles de création/suppression de 60 fichiers pendant l'affichage, aucune erreur d'onglet. Anomalie annexe : un onglet Corbeille restauré journalise « Cannot watch trash:// » toutes les 500 ms.`
- [x] M64 : « Tout développer » sur `/` reste borné (20 000 dossiers) ; exclusions faites au lot 7 (voir plus bas). `Fait (nid mutter, 2026-11-25) : « Tout développer » sur `/` se termine, borné ; il épuisait les instances inotify de l'utilisateur (une par dossier) : corrigé, une seule instance partagée par processus. Les dossiers d'un volume sondé (NFS, FUSE) partagent aussi un seul fil de sondage (test unitaire ; pas de NFS ici).`
- M22, M54, M61 : faits au lot 7 (voir plus bas).

## Lot 6 (suite)
- M83 : corrigé au lot 9 (voir plus bas).
- WM8 : `is_cloud_placeholder` renvoie false hors Windows ; la recherche de contenu doit se comporter comme avant.

### Lot 9 : restes du lot 6 (Opus)

- [ ] M83 : ouvrir une grosse archive lente (7z solide de plusieurs Go sur une clé USB), puis aller tout de suite dans un autre dossier du même onglet et ouvrir une autre archive : la seconde s'affiche sans attendre la fin de la première (le worker de la première est arrêté). `linux_archive_list::list` prend maintenant un point d'annulation. PARTIEL : listings rapides seulement.
- [ ] WF6 (code commun) : `MountedDiskImage` a un champ `aliases`, vide sous Linux ; une image montée par udisks reste reconnue (menu Démonter proposé).
- [x] Compiler `cargo check --tests` et lancer `cargo test linux_archive_list` sous WSL. Fait en natif sur cette machine (Fedora 45) le 2026-11-19 : `check-linux.sh --offline` propre, `cargo test` 829 + 632 tests verts (un échec isolé sur un des cinq lancements, non reproduit en trois relances, probablement l'intermittent connu), `linux_archive_list` 7 tests verts.

### Lot 7 : session Linux (Opus)

Fait sous WSL Fedora 45 le 2026-11-18 : `cargo test --no-fail-fast` (798 + 619 tests), `check-linux.sh`, tests de montage sous `unshare` (52), tests udisks2 sous `sudo`, vfat réel, `/mnt/c` (9p, sans bail). Reste à faire sur une vraie machine :

- [x] E6 (`374c5078`) : le journal indique « open file limit raised from 1024 to … » ; copier un dossier de plus de 1 000 sous-dossiers réussit ; un programme ouvert depuis Oxyde (`ulimit -n` dans un terminal ouvert par Oxyde) voit 1024. `Partiel (nid mutter, 2026-11-19) : avec `ulimit -Sn 1024`, journal « open file limit raised from 1024 to 1048576 » et `/proc/<pid>/limits` à 1048576 ; copie de 1 500 sous-dossiers réussie dans ces conditions ; limite vue par un programme lancé non vérifiée.` Vérifié : un processus enfant voit la limite souple à 1024.
- [x] E7 et LM4 (`de492644`, `dec384fb`), session Wayland GNOME : copier des fichiers dans Oxyde, coller dans Nautilus ; couper puis coller : déplacés. Même chose vers Dolphin (KDE, `text/uri-list` et coupe KDE) et un sélecteur GTK 4. Coller dans un éditeur de texte : les chemins. Copier dans Nautilus, coller dans Oxyde. Un élément de la corbeille copié ne s'offre qu'en texte. Vérifié avec wl-paste/wl-copy : export uri-list + gnome-copied-files + texte, sans processus wl-copy ; import copy et cut de type Nautilus OK. Pas de session GNOME réelle.
- [ ] E10 : voir lot 5 (IBus sous GNOME Wayland ; Fcitx 5 sous X11 : voir la section X11).
- [x] M93 (`2f14fc1a`) : `sudo mount --bind ~/a ~/b/lien` puis supprimer définitivement `~/b` : refus sans toucher à `~/a` ; changer les permissions de `~/b` récursivement : `~/a` inchangé. `Fait (nid mutter, 2026-11-25) : `~/a` n'est pas touché. Trois défauts trouvés puis corrigés : la suppression de `~/b` était partielle (refus maintenant avant de rien supprimer, d'après la table des montages), la cause s'affichait en anglais (traduite dans les cinq langues), et le conseil « Fermez les programmes… » avec Réessayer ne s'affiche plus pour ce refus. Vérifié dans le nid : `~/b` intact, boîte « Suppression incomplète » traduite.`
- [x] M94 (`758ddbed`) : rendre `~/.config/gtk-3.0/servers` illisible (`chmod 000`) puis ajouter un serveur : le fichier n'est pas réécrit. Vérifié : fichier laissé intact (même inode, même mtime).
- [x] M95 (`7303879a`) : monter une ISO réelle par Oxyde, supprimer le fichier ISO, démonter depuis la barre latérale : démontée. `Fait (nid mutter + clé exfat, 2026-11-19) : ISO montée par le menu, fichier ISO supprimé, Démonter depuis la barre latérale : démontée, périphérique loop libéré, entrée retirée.`
- [x] M98 (`7ef1e7ce`) : `du -sh ~/.cache/oxyde/previews` (ou le dossier d'aperçus) reste sous 256 Mio après le survol de nombreux fichiers. `Fait (nid mutter, 2026-11-19) : 968 Ko de cache après 240 miniatures (test petit : 256 Mio non approchés).`
- [x] M99, M54, M61 (`0dfb1121`) : mettre à la corbeille sur une clé USB : entrée dans `.Trash-<uid>` de la clé, visible dans Nautilus ; Annuler la restaure ; recréer un fichier du même nom puis Annuler : l'autre fichier n'est pas touché ; Rétablir la renvoie. `Fait (nid mutter + clé exfat, 2026-11-19) : corbeille dans `.Trash-1000` de la clé ; Annuler restaure ; avec le nom repris par un autre fichier, Annuler refuse (« A file or folder already exists at the restore location… ») sans toucher au fichier. Visibilité dans Nautilus et Rétablir non vérifiés.`
- [ ] LM2 et LF1 (`8bc2c37b`) : dans une toolbox ou distrobox, puis sous NixOS : les aperçus d'archives et de vidéos marchent (bac à sable bubblewrap accepté, sinon repli sans bubblewrap signalé dans le journal).
- [ ] LM3 et LF3 (`b42afd99`) : Btrfs avec sous-volumes, sans UDisks2 (conteneur) : la barre latérale ne montre ni `/home` ni les sous-volumes système en double ; un partage `sshfs` apparaît sans geler.
- [ ] LF2 (`ed27a8ac`) : sous Debian/Ubuntu (paquet .deb) et Arch (paquet AUR) : la mise à jour annonce le bon gestionnaire, pas RPM.
- [x] LF4 (`5ebf8330`) : copier `Readme` vers une clé exFAT montée par FUSE contenant `README` : conflit signalé avant la copie. `Fait (nid mutter + clé exfat, 2026-11-19) : conflit `Readme`/`README` signalé avant la copie (noyau exfat, pas FUSE).`
- [x] LF4 (`0467c4eb`) : copier un dossier setgid (`chmod 2775`) vers une clé FAT : copie complète, sans erreur. `Fait (nid mutter + clé exfat, 2026-11-19) : dossier `2775` copié sur exfat, « Copy completed », sans erreur.`
- [ ] LF4 (`6f5f0299`) : avec un dossier personnel NFS, ouvrir un fichier d'une archive : le journal indique « grants no file lease », le fichier s'ouvre depuis `/tmp/oxyde-<uid>` (mode 700).
- [x] M64 (`00924c95`) : « Tout développer » sur `/` : `/proc`, `/sys`, `/dev`, `/run` et les partages GVfs ou NFS restent fermés, mais s'ouvrent un par un. `Fait (nid mutter + clé exfat, 2026-11-19) : « Tout développer » sur Root se termine, `/dev`, `/etc`, `/boot` restent fermés, mémoire stable ; replier aussitôt : reste replié (M63/M64). Partages GVfs/NFS non testés.`
- [x] M22 (`9eb9b039`) : depuis Firefox (portail, filtre « Images »), ouvrir un dossier de dizaines de milliers de fichiers : la liste apparaît tout de suite, les images arrivent par lots ; Retour puis Avant : immédiat. PARTIEL : le dossier de 30 000 fichiers s'ouvre en ~1,3 s via le portail, mais mon appel gdbus n'a pas appliqué le filtre Images ; à refaire depuis Firefox. `Fait (nid mutter, 2026-11-25) par le portail avec le filtre Images : 2 000 PNG parmi 30 000 fichiers affichés en 2,5 s ; Monter, Retour et Avant immédiats. Firefox non essayé.`

Paquets installés dans la VM WSL pour ces tests : `wl-clipboard`, `udisks2` (service démarré), `dosfstools`.

### Lot 10 : reportés du lot 8 (Opus)

- [x] Compiler `cargo check --tests` puis `cargo test` sous WSL (`git pull origin audit-fixes` une fois la branche poussée) ; lancer `check-linux.sh`. Fait en natif sur cette machine (Fedora 45) le 2026-11-19 : `check-linux.sh --offline` propre, `cargo test` 829 + 632 tests verts (un échec isolé sur un des cinq lancements, non reproduit en trois relances, probablement l'intermittent connu), `linux_archive_list` 7 tests verts.
- [x] F94 (`3f80b0d0`) : supprimer de la corbeille Linux un élément déjà purgé par un autre programme : message « sélection indisponible », pas d'erreur brute. `Non fait : la vue Corbeille du nid lit la vraie corbeille de l'utilisateur (listage périmé affiché) ; un Suppr dans cette vue a été bloqué par précaution. À faire à la main.` Vérifié : refus, rien de supprimé ; message désormais traduit (cinq langues, test ajouté).
- [x] F106 (`dfa3e5bb`) : compresser un dossier qui contient `README` et `readme` : archive créée avec les deux. `Fait (nid mutter, 2026-11-19) : `proj.zip` contient `README` et `readme`.`
- [x] F134 (`d64b8acb`) : deux instances d'Oxyde (sélecteur de fichiers ouvert par le portail et fenêtre principale) : `oxyde.log` n'est pas tourné pendant que l'autre écrit. `Fait (nid mutter, 2026-11-25), cas du passage de main : une seconde instance lancée avec un journal de 2,6 Mo ne le tourne pas et écrit dans le même fichier.`
- [x] F128 (`6de8e9d5`) : `config.json` avec BOM ou un champ mal typé : réglages gardés, copie `config.json.invalid` créée. `Fait (nid mutter, 2026-11-19) : BOM + `ui_rem_size` mal typé : seul ce champ remis par défaut, langue et `show_hidden` gardés, `config.json.invalid` créé.`
- [x] F91 (`d87732d2`) : rechercher un mot dans un fichier UTF-16 avec BOM (`iconv -t UTF-16`) : trouvé. `Fait (nid mutter, 2026-11-19) : le mot est trouvé dans le fichier UTF-16 avec BOM.`

### Reprise des reportés (2026-11-19)

- [x] Compiler et lancer `cargo test` sous WSL (code commun : recherche, annulation, aperçus, barre latérale). Fait en natif sur cette machine (Fedora 45) le 2026-11-19 : `check-linux.sh --offline` propre, `cargo test` 829 + 632 tests verts (un échec isolé sur un des cinq lancements, non reproduit en trois relances, probablement l'intermittent connu), `linux_archive_list` 7 tests verts.
- [x] M62 (`455c629b`) : rechercher un mot placé après une ligne en Latin-1 (`printf 'caf\xe9\nmot\n'`) : trouvé. `Fait (nid mutter, 2026-11-19) : trouvé. À noter : la boîte de recherche reste ouverte après « Rechercher », et un onglet de recherche sur le contenu seul s'intitule « Rechercher «  » dans … » (requête de nom vide).`
- [x] M29 (`65b51b17`) : survoler un fichier texte sur un partage NFS/SMB coupé, puis d'autres fichiers après 20 s : leurs aperçus reviennent. `Fait (nid mutter, 2026-11-19) sur le partage SMB local (serveur impacket) figé par SIGSTOP : survol de f1 puis f2 pendant le gel, interface fluide, pas d'infobulle bloquée ; après 25 s et reprise (SIGCONT), l'aperçu de f3 (« contenu 3 ») revient. Un partage NFS n'a pas été essayé.`
- [x] M63/M64 (`233c9d7e`) : « Tout développer » sur `/`, replier aussitôt : le dossier reste replié. `Fait (nid mutter + clé exfat, 2026-11-19).`

### Catégorie 1 de 101-restes (2026-11-19)

- [x] Compiler et lancer `cargo test` sous WSL : check-linux.sh propre, 828 + 625 tests verts (2026-11-19).
- [x] M96 (`7d56f6b6`) : miniature d'un PNG transparent en grille, quitter puis revenir dans le dossier : toujours transparente. `Fait (nid mutter, 2026-11-19) : miniature transparente à la première génération et après retour dans le dossier (cache : 1 PNG, 239 JPEG).`
- [x] M97 (`007930d8`) : PDF dont une page a une MediaBox nulle : les pages suivantes gardent leurs proportions, la dernière reste accessible. `Fait (nid mutter, 2026-11-19) (panneau Visionneuse) : la page 2 à MediaBox nulle prend la taille de la page 1, la page 3 portrait reste accessible avec ses proportions.`
- [x] F35/F36 (`1971dafc`, `e464eb90`) : compression 7z d'un gros dossier, pause de quelques minutes puis reprise : se termine sans « délai dépassé ». `Fait (nid mutter, 2026-11-25) : 7z de 1,6 Go mis en pause 4 min puis repris, terminé en 501 s, `7z t` sans erreur.`
- [x] F40 (`d89c0e11`) : dossier de nombreuses images ouvert dans deux onglets : les miniatures sont toutes mises en cache (le retour au dossier ne les régénère pas). Vérifié : 307 miniatures en cache pour 302 images, inchangé au retour.

## Lecture seule (0b79d399)

- [x] Compilation Linux : `volume_is_read_only` (statvfs, `ST_RDONLY`) dans `linux_impl.rs`. `Fait (nid mutter, 2026-11-25) : compilé et utilisé (`cargo check` propre).`
- [x] ISO montée par udisks ou clé montée `ro` : couper, coller, renommer, supprimer, nouveau, compresser, extraire ici absents des menus ; « Nouveau dossier » grisé dans la barre. `Fait sur ISO udisks (nid mutter + clé exfat, 2026-11-19) : menu fichier sans Couper/Coller/Renommer/Supprimer/Compresser/Extraire, « Nouveau dossier » grisé. Le menu propose encore « Annuler : Supprimer » (historique d'un autre volume). Clé montée `ro` non testée (pas de root).`
- [x] Lecteur le plus profond (`f96c3172`) : avec `/` en lecture seule et `/home` monté à part (SteamOS, ou un montage `ro` contenant un montage `rw`), les actions d'écriture restent disponibles dans `/home`. `Fait (nid mutter, 2026-11-25) avec un montage `ro` contenant un montage `rw`.`
- [x] Dépôt (`cac4d642`) : glisser des fichiers sur un dossier d'une ISO montée est refusé ; Maj + glisser depuis l'ISO vers `/home` copie sans erreur. `Fait (nid mutter + clé exfat, 2026-11-19) : Maj+glisser depuis l'ISO vers la clé copie (« Copy completed ») ; un glisser vers l'ISO est ignoré sans message, et l'indicateur du glisser affiche toujours « Copy ».`

## Anomalies trouvées pendant ces tests (2026-11-19)

- [x] Visionneuse bloquée sur « Chargement du fichier… » pour un tube nommé : `File::open` attendait un écrivain pour toujours. Texte, Markdown et image passent par `open_regular` ; test `a_named_pipe_is_refused_instead_of_blocking_the_viewer` ; vérifié dans le nid (tubes `.hidden` et `tube.txt` : « This file cannot be displayed »).
- [x] « Copy target is not an ordinary file » affiché en anglais : message reconnu et traduit dans les cinq langues (`transfer-target-not-replaceable`) ; test dans `format/tests/transfer_collision.rs`.
- [x] Onglet de recherche sur le contenu seul intitulé « Rechercher «  » … » : le titre reprend le contenu cherché quand le nom est vide ; test `search/tests/label.rs`.
- [x] Journal : « Cannot watch trash:// » toutes les 500 ms avec un onglet Corbeille ou Réseau ouvert : ces emplacements virtuels ne sont plus surveillés ; vérifié (0 occurrence dans une session avec onglets restaurés).
- Boîte de recherche avancée qui reste ouverte après « Rechercher » : voulu (la fenêtre principale est réactivée), pas modifié.
- [ ] Dépôt sur une ISO montée ignoré sans retour, indicateur « Copy » affiché : le fantôme du glisser interne ne connaît pas la cible survolée (il faudrait un état central de cible de dépôt) ; non corrigé. `2026-11-25 : à revoir, le dépôt peut désormais afficher « Dépôt indisponible ».`
- [ ] « Annuler : Supprimer » proposé sur un volume en lecture seule : l'historique est global ; non corrigé, à décider.
- [ ] Infobulle « This preview could not be loaded » sur un membre d'archive (7z chiffré, ISO) : probablement aperçus d'archive non pris en charge ; non examiné.

## Faibles Linux de 102-faibles (2026-11-24)

Corrigés sous WSL Fedora 45 (`51ec43ca..552e8bb6`). Reste à faire sur une vraie machine :

- [x] Compiler et lancer `cargo test` sous WSL après `cargo build --bins` (workers changés : F57, F58). Fait le 2026-11-25 : `check-linux.sh --offline` propre, 860 + 656 tests verts, intégration verte (`--test-threads=1`), `linux_permissions` sous `unshare` 16 verts. Un échec trouvé et corrigé (`200c20c7`) : F233 gardait l'espace final d'une URI tapée (`network:/// `).
- [x] F45 (`ab2a45b0`, patch GPUI `clipboard_offers`) : sous GNOME Wayland puis KDE Wayland, copier des fichiers dans Nautilus (puis Dolphin) : « Coller » est actif dans Oxyde avant tout collage, et colle les fichiers ; copier du texte seul : « Coller » des fichiers reste inactif. `Fait (nid mutter, 2026-11-25) avec un propriétaire GTK 4 : « Coller » actif avant tout collage. Défaut trouvé et corrigé : seul `x-special/gnome-copied-files` était lu ; `text/uri-list` (Dolphin) et la coupe KDE (`application/x-kde-cutselection`) le sont aussi. Texte seul : pas de fichiers collés. GNOME et KDE réels non essayés.`
- [x] F46 (`6bb4475e`) : coller dans Oxyde pendant qu'une application qui possède le presse-papiers est figée (`kill -STOP`) : Oxyde reprend la main en 3 s environ, sans processus `wl-paste` restant. `Fait (nid mutter, 2026-11-25) : `wl-paste` tué vers 3 s, aucun processus restant. Défaut trouvé et corrigé : Coller lisait d'abord le presse-papiers par GPUI sur le fil de l'interface (4 s de gel). Coller ne le lit plus quand une autre application le possède (fichiers, texte ou image annoncés) ; vérifié avec un propriétaire figé qui n'offre que du texte pendant qu'Oxyde garde des fichiers copiés : interface immédiate, rien collé.`
- [ ] F49 (`02a00de6`) : partage SMB ou SFTP monté par GVfs, serveur figé ou débranché : ouvrir le partage ou le démonter échoue au bout de 30 s au plus, sans geler la fenêtre.
- [x] F50 (`424b7f35`) : « Ouvrir avec » une application dont le lancement échoue (fichier `.desktop` dont `Exec` est introuvable) : elle n'est pas retenue comme application par défaut ou récente de ce type. `Partiel (nid mutter, 2026-11-25) : une entrée sans `Exec` n'est pas proposée (GIO) ; une entrée disparue donne une erreur et rien n'est retenu. Limite partagée avec Nautilus : une application qui échoue après son lancement par `gio-launch-desktop` est retenue, le lancement ayant réussi.`
- [ ] F55 (`299a84a0`) : sans `/usr/share/xdg-desktop-portal/*-portals.conf` propre à Oxyde, définir Oxyde comme sélecteur puis le retirer : le fichier de `~/.config/xdg-desktop-portal/` qu'Oxyde avait écrit est supprimé ; pendant un partage d'écran (visio), changer ce réglage ne coupe pas le partage (redémarrage du portail différé).
- [x] F56 (`14d85fa3`) : ouvrir le sélecteur de fichiers d'Oxyde depuis Firefox plusieurs fois de suite : il répond toujours, sans délai. `Fait (nid mutter, 2026-11-25) par appels directs au portail (`FileChooser.OpenFile`) : 4 appels de suite, chacun répond et s'annule en 1 s environ. Firefox non essayé.`
- [ ] F57 (`8d6ef65d`) : vidéo dans l'aperçu, déplacer la tête de lecture tout de suite après l'ouverture puis plusieurs fois vite : la lecture va à la dernière position demandée.
- [ ] F59 (`cce830d8`) : ouvrir un fichier d'une clé USB dont le chemin contient des espaces dans un programme qui le garde ouvert, puis éjecter : le message nomme ce programme.
- [x] F61 (`44dbf2e9`) : un dossier ordinaire nommé `Trash` ou `.Trash-1000` dans un sous-dossier n'est pas traité comme une corbeille (pas de vue Corbeille, suppression normale). `Fait (nid mutter, 2026-11-25) : dossiers `Trash` et `.Trash-1000` ordinaires listés comme des dossiers, suppression normale.`
- [x] F62 (`d8f449b4`) : profil neuf sans `~/.config/gtk-3.0` : ajouter un signet dans Nautilus : il apparaît dans la barre latérale d'Oxyde sans redémarrage. `Fait (nid mutter, 2026-11-25) : profil sans `~/.config/gtk-3.0`, signet ajouté au fichier : il apparaît sans redémarrage.`
- [x] F65 (`532c6ffe`) : document Office dont la première miniature intégrée est endommagée et une seconde valide : miniature affichée. `Fait (nid mutter, 2026-11-25) : première miniature intégrée endommagée, l'aperçu au survol montre la seconde.`
- [x] F66 (`34c2cb3a`) : pendant une grosse copie en priorité basse, `ionice -p <tid>` du thread de copie indique `best-effort: prio 7` (BFQ seulement pour un effet réel). `Fait (nid mutter, 2026-11-25) : Turbo désactivé, les threads de copie tokio sont en `best-effort: prio 7`, tous revenus à `none` après la copie.`
- [ ] F86 (`e77d6bf5`) : ouvrir puis extraire une archive RAR en plusieurs volumes : contenu complet, comme avant.
- [x] F87 (`70b5bdb8`) : renommer un membre d'une grosse archive et, pendant la reconstruction, modifier l'archive avec un autre programme : la reconstruction est annulée et l'archive modifiée est gardée telle quelle. `Fait (nid mutter, 2026-11-25) : ajout dans l'archive pendant une reconstruction de 16 s : « archive-source-conflict », archive modifiée gardée, fichiers temporaires nettoyés. Défaut trouvé et corrigé : le message d'erreur sous le champ de renommage était coupé à gauche (`field_error`).`
- [x] F124 (`2d015312`) : lien symbolique vers un dossier dans l'arbre de la barre latérale : affiché avec sa flèche et dépliable ; « Tout développer » sur le parent n'entre pas dedans. `Fait (nid mutter, 2026-11-25) après correction : le lien garde sa flèche et se déplie, mais « Tout développer » n'entre plus dedans.`
- [x] F209 (`d6ac0f67`) : sous KDE, avec un pourcentage de corbeille non entier dans `ktrashrc` (12,34), ouvrir les réglages de la corbeille et valider sans toucher au champ : `ktrashrc` garde 12.34. `Fait (nid mutter, 2026-11-25) : `ktrashrc` inchangé (12.34, fichier non réécrit) après OK ; le champ affiche 12,3.`

## Racine en lecture seule (2026-11-25)

- [ ] Fedora Silverblue : dans `~` (`/var/home/<user>`) puis via `/home/<user>`, Nouveau dossier, Renommer, Couper, Coller et Supprimer (vers la corbeille) sont proposés et fonctionnent ; dans `/usr`, ils restent masqués. Même chose sur SteamOS dans `/home`.
- [x] Dossier du système en écriture mais qui appartient à root (`/etc`, `/var/lib`) : Nouveau dossier grisé dans la barre, Nouveau, Coller, Couper, Renommer et Supprimer absents des menus, comme Nautilus ; `/tmp` garde ces actions. `Fait (nid mutter, 2026-11-25). Défaut trouvé puis corrigé : dans `/tmp` (sticky), un élément de root proposait encore Couper, Renommer et Supprimer ; comme GIO, ils ne sont plus proposés pour un élément ni à soi ni dans un dossier à soi (menu et barre vérifiés ; ses propres éléments les gardent).`
- [x] Glisser un fichier depuis `~` sur la ligne `etc` en listant `/` : dépôt refusé, sans surbrillance ; sur `tmp` : accepté. `Fait (nid mutter, 2026-11-25).`

## Optimisations Linux de 103-optimisations (2026-11-25)

Corrigés et mesurés sous WSL Fedora 45 (`c6409fba..6cc13488`), mesures dans `103-optimisations.md`.

- [x] Compiler et lancer les tests sous WSL : fait le 2026-11-25, `check-linux.sh --offline` propre, `cargo test` 874 + 671 tests verts et intégration verte (`--test-threads=1`), `linux_permissions` sous `unshare --map-auto --map-root-user` 17 verts. Un échec intermittent trouvé et corrigé (`99aac9ae`) : le test de suppression d'arbre `an_entry_the_listing_passed_is_found_by_another_pass` écrivait dans un dossier qu'un autre worker venait de retirer (0 échec sur 300 lancements après).
- [x] O29 (`c6409fba`), corbeille : mettre une centaine de dossiers à la corbeille depuis Dolphin (KDE écrit `directorysizes`), puis les supprimer définitivement depuis la vue Corbeille d'Oxyde : leurs lignes disparaissent de `~/.local/share/Trash/directorysizes`, les autres restent, et la taille de la corbeille affichée par Dolphin reste juste. Même chose avec « Vider la corbeille ». `Fait (nid mutter, 2026-11-25) avec 103 entrées et un `directorysizes` écrit à la main (pas de Dolphin) : après suppression définitive de 100 d'entre elles, seules les 3 autres lignes restent ; « Vider la Corbeille » laisse un `directorysizes` vide. Affichage de la taille dans Dolphin non vérifié.`
- [x] O29, propriétés : sélectionner plusieurs centaines d'éléments dans la vue Corbeille, ouvrir Propriétés : la fenêtre s'ouvre sans attente sensible ; ouvrir aussi les propriétés d'un seul élément : emplacement d'origine et date de suppression justes. Remettre à la corbeille un fichier du même nom qu'un élément purgé, puis ses propriétés : ce sont celles du nouvel élément. Refaire sur la corbeille d'une clé USB (`.Trash-1000`). `Fait (nid mutter, 2026-11-25) : Propriétés de 100 éléments ouvertes en moins de 0,6 s ; un élément du même nom qu'un élément purgé, remis à la corbeille : emplacement d'origine et date de suppression du nouvel élément. Défaut trouvé et corrigé : « 1 fichiers, 0 dossiers » (pluriel de `properties-counts` dans les cinq langues). Corbeille de clé USB non refaite.`
- [x] O29, restauration : restaurer une sélection, puis Annuler la mise à la corbeille d'un élément (sélection par chemin d'origine) : inchangé. `Fait (nid mutter, 2026-11-25) : deux éléments restaurés à leur place, leurs lignes `directorysizes` retirées ; mettre `keep1` à la corbeille puis Annuler le restaure, les autres éléments de la corbeille restent.`
- [x] O30 (`b94f17d0`), recherche avancée : sans « fichiers cachés », un dossier caché (`.cache`) n'est pas parcouru et ses fichiers n'apparaissent pas ; avec, ils apparaissent. Recherche de contenu dans un dossier mêlant texte, UTF-16 (`iconv -t UTF-16`) et binaires : les textes et l'UTF-16 sont trouvés, pas les binaires ; un document Office (`.docx`, `.odt`) reste trouvé par son texte. `Fait (nid mutter, 2026-11-25) : le dossier caché suit « fichiers cachés » ; UTF-16, `.docx` et `.odt` trouvés, binaire non.`
- [x] O40 (`610a122b`), « Ouvrir avec… » sur un fichier : les icônes des applications s'affichent comme avant, y compris celles des Flatpak (`/var/lib/flatpak/exports/share`, `~/.local/share/flatpak/exports/share`) et d'une application qui n'a qu'une icône dans `/usr/share/pixmaps`. `Fait (nid mutter, 2026-11-25) : icône du Flatpak Zed et d'une application qui n'a qu'une icône dans `pixmaps` affichées.`
- [x] O41 (`b7df8b25`), Propriétés > Permissions d'un fichier à soi : la liste des groupes nomme tous les groupes de l'utilisateur (`id -Gn`), le propriétaire et le groupe du fichier aussi. Si possible avec un annuaire (SSSD, LDAP) : la fenêtre s'ouvre sans attendre un processus `getent` par groupe. `Fait (nid mutter, 2026-11-25) : groupes `biluf` et `wheel`, comme `id -Gn`. Pas d'annuaire disponible.`
- [x] O42 (`6cc13488`), types de fichiers : colonne Type et Propriétés pour `photo.PNG`, `archive.tar.gz`, `ARCHIVE.TAR.GZ`, `Makefile`, `README`, `script.sh`, un fichier sans extension (texte, puis binaire) et un fichier vide : mêmes types que `gio info -a standard::content-type`. Sélecteur de fichiers ouvert par Firefox avec le filtre « Images » sur un dossier de milliers de fichiers : la liste filtrée apparaît sans gel. `Fait (nid mutter, 2026-11-25) pour Propriétés : mêmes types que `gio info` pour les 9 fichiers ; la colonne Type affiche les libellés d'extension. Partie Firefox : voir M22.`

## Anomalies trouvées pendant ces tests (2026-11-25)

- [x] « Tout développer » sur `/` épuisait les instances inotify de l'utilisateur (128, partagées avec toute la session) : une seule instance pour tout le processus, comme GIO ; test `changes/tests/shared.rs`.
- [x] Lien symbolique vers un dossier dans l'arbre : « Tout développer » entrait dedans (F124) ; test `sidebar/tests/links.rs`.
- [x] Fichiers offerts en `text/uri-list` (Dolphin) ou coupés par KDE non collés (F45) ; test `linux_file_clipboard/tests.rs`.
- [x] Coller gelait l'interface 4 s quand le propriétaire des fichiers annoncés était figé (F46) : lecture sur le worker.
- [x] Message d'erreur sous un champ coupé à gauche quand il est long (F87) : il passe à la ligne dans la largeur du champ.
- [x] « 1 fichiers, 0 dossiers » dans Propriétés : pluriel dans les cinq langues.
- [x] M93 : cause du refus en anglais, conseil « Fermez les programmes… » trompeur, suppression partielle de `~/b` : refus avant toute suppression, message traduit, pas de Réessayer ; tests `linux_file_tree/tests/remove.rs` et `format/tests/transfer_collision.rs`.
- [x] Propriétés et colonne Taille : « Contenu » comptait à travers les montages (`mount --bind`, autre volume) : comme Nautilus, un dossier monté est compté mais pas parcouru ; test `folder_sizes/tests/mounts.rs` (sous `unshare`), vérifié dans le nid (4 fichiers, 2 dossiers, 16 o).
- [x] Élément de root dans `/tmp` (sticky) : Couper, Renommer et Supprimer encore proposés ; test `linux_write_access/tests/access.rs`.
- [x] Surveillances par sondage (NFS, FUSE) : une instance et un fil par dossier déplié ; maintenant une seule instance partagée ; test `changes/tests/shared.rs`. À mesurer sur un vrai partage NFS.
- [x] Propriétaire du presse-papiers figé qui n'offre que du texte : Coller gelait 4 s (lecture GPUI) ; Oxyde ne lit plus le presse-papiers d'une autre application sur le fil de l'interface. Reste hors Oxyde : coller du texte dans un champ (barre d'adresse) depuis un propriétaire figé attend encore GPUI, comme dans Zed.
- Clic droit juste après une navigation par la barre d'adresse : une fois, menu réduit à « Tout sélectionner » ; non reproduit. C'est le menu d'un onglet de résultats de recherche (Coller et Propriétés y sont retirés) : le chemin avait sans doute été tapé dans la recherche par le banc de test. Pas un défaut.

## Tests X11 (non prévus)

Sortis de leurs lots : l'utilisateur ne compte pas les faire. Ils restent ouverts, sans échéance.

- [x] E9 (`1cc5172d`), X11 : même test que sous Wayland (Entrée maintenue sur Remplacer). Limite connue : X11 ne signale pas la répétition de touche dans GPUI, Entrée maintenue peut encore enchaîner les conflits. Noter le résultat.
- [ ] Fcitx 5 sous X11 (KDE ou Xfce) avec Pinyin : `nihao` puis Espace : `你好` une seule fois.
- [ ] E7 et E9, session X11 : copier puis coller entre deux onglets d'Oxyde ; coller dans un éditeur : les chemins en texte ; aucun processus `wl-copy` ; dans la boîte de conflit, maintenir Entrée sur Ignorer : un seul conflit traité (X11 ne signale pas la répétition).
