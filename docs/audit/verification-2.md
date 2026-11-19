# Vérification 2 : constats critiques et élevés (ARCHIVES-01 à 04, FILEOPS-01 à 03, PLATFORM-01 à 04)

Rapports sans constat critique ni élevé : ARCHIVES-02, ARCHIVES-03, ARCHIVES-04, FILEOPS-02, PLATFORM-02, PLATFORM-03, PLATFORM-04. Cinq constats retenus, aucun critique.

## ARCHIVES-01 n° 1 : `archive_metadata.rs:229`, `archive_io_codec.rs:514` (élevé)

Verdict : confirmé, élevé maintenu.
Dans tar 0.4.46, l'itérateur d'entrées absorbe les en-têtes `L`, `K` et `x`, mais renvoie l'en-tête global `g` comme une entrée ordinaire, et `is_file` n'accepte que le type `Regular` (ni `S` ni `7`). Les quatre parseurs du projet (`archive_metadata.rs`, `archive_io_codec.rs`, `archive.rs:455`, `archive_tar_open.rs:75`) appliquent le même filtre et refusent l'archive entière ; aucun code ne traite ces types. Scénario : l'archive « Source code (tar.gz) » d'une version GitHub, produite par `git archive`, commence par `pax_global_header` ; son listage, sa recherche, l'ouverture d'un membre et l'extraction échouent.

## FILEOPS-01 : `compress.rs:268` (élevé)

Verdict : confirmé, élevé maintenu.
`unique_archive_path` renvoie le chemin inchangé quand `Path::exists()` est faux, ce qui arrive pour un lien symbolique cassé, alors que la publication exclusive (`renameat` NOREPLACE sous Linux, renommage sans remplacement sous Windows) rencontre bien l'objet et renvoie `AlreadyExists`. La boucle de publication n'a ni point d'annulation ni limite. Scénario : compresser « photos » dans un dossier qui contient un lien cassé `photos.zip` fait tourner la boucle sans fin, un cœur reste à 100 % et l'opération n'est pas annulable. La publication d'un membre extrait (`extract.rs:45-50`) suit le même motif et a le même défaut.

## FILEOPS-01 : `destination_tree.rs:150` (élevé)

Verdict : confirmé sous Linux, élevé maintenu.
`copy.rs` planifie tous les fichiers dans `files_to_process` avant de copier, et chaque `SelectedFile` garde un `Arc` de son `BoundDirectory`, lequel retient toute sa chaîne d'ancêtres avec leurs descripteurs ouverts, jusqu'à `copy.rs:685`. Le cache de `DestinationTree` garde un descripteur par dossier de destination jusqu'à la fin de l'opération, et rien ne relève `RLIMIT_NOFILE` (seul `Resource::Data` est réglé, dans `linux_worker.rs`). Scénario : sous Linux, avec la limite souple habituelle de 1024, copier une arborescence d'environ 500 dossiers non vides dépasse les 1024 descripteurs une fois les dossiers créés, et l'opération échoue en EMFILE. Sans effet notable sous Windows. Ce constat a la même cause racine que celui de FILEOPS-03 ci-dessous.

## FILEOPS-03 : `source_selection.rs:197` (élevé)

Verdict : confirmé sous Linux, élevé maintenu (doublon partiel du précédent).
`SelectedEntry::child` crée pour chaque dossier un `BoundDirectory` qui tient son descripteur et celui de son ancêtre, et chaque fichier planifié le garde vivant. Le seul plafond du parcours (`bound_pending`) limite le nombre d'entrées d'archive, pas le nombre de descripteurs. Scénario : sous Linux, la planification d'une copie ou d'un déplacement de `node_modules`, avec plus d'environ 1000 dossiers contenant des fichiers, échoue en EMFILE avant même la création des dossiers de destination. Une seule correction des deux constats suffit : relever la limite, ou ne pas retenir les descripteurs des dossiers sources pendant toute la planification.

## PLATFORM-01 n° 1 : `linux_file_clipboard/wayland.rs:16` (élevé)

Verdict : confirmé, élevé maintenu.
`offer` lance `wl-copy` sans vérifier qu'une session Wayland existe (aucun test de `WAYLAND_DISPLAY` dans ce chemin). L'écriture dans le tube réussit presque toujours avant que le processus se termine, donc `set_files` renvoie faux et l'interface (`ui/app/clipboard.rs`) n'écrit pas les chemins en texte. Ensuite, `served()` voit le processus terminé : `current()` libère la sélection, `has_file_format()` renvoie faux, et le repli `wl-paste` échoue lui aussi. Scénario : session X11 (GNOME sur Xorg, Xfce, MATE, Cinnamon) avec wl-clipboard installé ; copier ou couper puis coller des fichiers ne fait rien, même entre deux fenêtres d'Oxyde.

## Bilan

| Constat | Verdict | Gravité |
| --- | --- | --- |
| ARCHIVES-01 n° 1 (TAR `g`, `S`, `7`) | confirmé | élevé |
| FILEOPS-01 `compress.rs:268` (boucle de publication) | confirmé | élevé |
| FILEOPS-01 `destination_tree.rs:150` (descripteurs) | confirmé (Linux) | élevé |
| FILEOPS-03 `source_selection.rs:197` (descripteurs) | confirmé (Linux), même cause que le précédent | élevé |
| PLATFORM-01 n° 1 (`wl-copy` sous X11) | confirmé | élevé |

<!-- LOT TERMINÉ -->
