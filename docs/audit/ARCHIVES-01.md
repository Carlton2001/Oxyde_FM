# Audit ARCHIVES-01

Résumé : 17 constats, dont 1 élevé, 5 moyens, 6 faibles et 5 optimisations. Aucune faille de confinement (zip slip, liens, noms réservés) n'a été trouvée dans ce lot. Les défauts principaux sont fonctionnels : des archives TAR courantes sont refusées, les ZIP de plus de 4 Gio ne peuvent pas être créés, les dates des membres ZIP sont perdues, et l'IPC du worker fait de nombreux allers-retours.

## Fichiers lus en entier

- [x] `src/bin/oxyde-archive-io-worker.rs`
- [x] `src/bin/oxyde-archive-list-worker.rs`
- [x] `src/commands/archive.rs`
- [x] `src/commands/archive_handle_worker.rs`
- [x] `src/commands/archive_io_codec.rs`
- [x] `src/commands/archive_io_codec/encode.rs`
- [x] `src/commands/archive_io_codec/parallel.rs`
- [x] `src/commands/archive_io_worker.rs`
- [x] `src/commands/archive_iso_open.rs`
- [x] `src/commands/archive_metadata.rs`

## Constats

### 1. `src/commands/archive_metadata.rs:229` et `src/commands/archive_io_codec.rs:514`
- Gravité : élevé. Confiance : confirmé.
- Défaut : tar 0.4.46 renvoie l'en-tête PAX global (type `g`) comme une entrée ordinaire. Les deux parseurs le rejettent comme « fichier spécial » et refusent donc toute l'archive. Le même refus touche les fichiers GNU sparse (`S`) et contigus (`7`), qui sont pourtant des fichiers réguliers. Aucun code du projet ne traite ces types.
- Scénario : un `.tar.gz` produit par `git archive`, comme les archives « Source code » de GitHub, commence par `pax_global_header`. Son listage, son index de recherche, l'ouverture d'un membre et l'extraction échouent tous.

### 2. `src/commands/archive_io_codec/encode.rs:69`
- Gravité : moyen. Confiance : confirmé.
- Défaut : les options ZIP ne fixent jamais `large_file(true)`, et rien n'appelle `set_auto_large_file`. zip 9.0.4 abandonne l'écriture d'un membre dès qu'il atteint 4 Gio (« Large file option has not been set »).
- Scénario : compresser en ZIP une vidéo ou un disque virtuel de 5 Go échoue après 4 Gio de compression. Modifier un ZIP existant qui contient un tel membre échoue de la même façon, puisque la reconstruction passe par ce chemin.

### 3. `src/commands/archive_io_codec/encode.rs:69` et `src/commands/archive_io_codec/parallel.rs:101`
- Gravité : moyen. Confiance : confirmé.
- Défaut : l'encodage ZIP, séquentiel comme parallèle, ignore `entry.modified` et `entry.mode` fournis par le broker. Les deux options utilisent les valeurs par défaut de zip. TAR et 7z, eux, conservent les dates.
- Scénario : chaque fichier d'un ZIP créé reçoit la date de création de l'archive. Après un simple ajout ou une suppression dans un ZIP existant, tous les membres conservés perdent leur date d'origine.

### 4. `src/commands/archive_io_codec.rs:264` et `:299`
- Gravité : moyen. Confiance : confirmé.
- Défaut : le décodage ZIP construit le manifeste et les entrées sans date de modification (`modified: None`). Or le broker (`systems/archive_io.rs:265`) n'applique une date que si elle est présente.
- Scénario : les fichiers extraits ou ouverts depuis un ZIP portent l'heure de l'extraction, pas leur date d'archive. Ce n'est pas le cas pour 7z et TAR.

### 5. `src/commands/archive_io_codec.rs:544` et `:458`
- Gravité : moyen. Confiance : plausible.
- Défaut : pour une sélection dans un TAR compressé, la première passe décode tout le flux. Dans un bloc 7z solide, les voisins sont décodés vers un puits. Dans les deux cas, ces octets sont comptés par le `BudgetReader` de la sélection, dont la limite `Decoded` vaut max_bytes + 4096 × entrées, soit environ 646 Mio avec `OPEN_FILE`.
- Scénario : ouvrir un petit fichier dans un `.tar.xz` de 2 Go décompressés, ou à la fin d'un bloc solide 7z de 1 Go (taille courante avec 7-Zip), échoue sur la limite Decoded. Le fichier demandé ne fait pourtant que quelques Ko.

### 6. `src/commands/archive_iso_open.rs:131`
- Gravité : moyen. Confiance : plausible.
- Défaut : seul le descripteur primaire ISO9660 est lu. Les descripteurs UDF qui suivent le terminateur ne sont pas détectés, si bien qu'une image « pont UDF » est acceptée avec son arbre ISO9660 factice. `ui/app/disk_images.rs:55-69` choisit alors l'ouverture virtuelle plutôt que le montage natif.
- Scénario : l'ISO d'installation de Windows 11 s'ouvre comme dossier virtuel qui ne contient que `README.TXT`, au lieu d'être montée.

### 7. `src/commands/archive_iso_open.rs:208`
- Gravité : faible. Confiance : confirmé.
- Défaut : ni le descripteur supplémentaire Joliet ni les extensions Rock Ridge ne sont lus. Seuls les identifiants ISO9660 bruts sont affichés et extraits.
- Scénario : une ISO gravée en Joliet niveau 1 affiche et extrait `MYDOCU_1.TXT` au lieu de « Mes documents.txt ». Une ISO Linux hybride perd ses noms longs et sa casse.

### 8. `src/commands/archive_iso_open.rs:181` et `:195`
- Gravité : faible. Confiance : confirmé.
- Défaut : la limite de fin d'enregistrement et le saut de bourrage utilisent la taille de bloc logique, alors que la norme les définit sur le secteur logique de 2048 octets.
- Scénario : une image valide en blocs de 512 ou 1024 octets, dont un enregistrement de répertoire franchit une frontière de 512 octets, est refusée comme corrompue.

### 9. `src/commands/archive_io_codec.rs:307`
- Gravité : faible. Confiance : confirmé.
- Défaut : toute erreur « Invalid checksum » sur un membre ZipCrypto est traduite en mot de passe erroné, même lorsque le mot de passe est juste et que les données sont corrompues.
- Scénario : un ZIP chiffré tronqué ou abîmé fait redemander le mot de passe sans fin au lieu de signaler une archive corrompue.

### 10. `src/commands/archive.rs:239` et `src/commands/archive_handle_worker.rs:32`
- Gravité : faible. Confiance : confirmé.
- Défaut : quand le dossier interne demandé n'existe pas ou désigne un fichier, `directory_entries`, `directory` et `list_seven_zip` renvoient une liste vide. `list_iso` sous Linux renvoie au contraire une erreur.
- Scénario : après le remplacement d'une archive, un onglet resté sur un dossier interne disparu affiche un dossier vide sans erreur. Le comportement diffère aussi entre Windows et Linux pour les ISO.

### 11. `src/commands/archive.rs:143`
- Gravité : faible. Confiance : plausible.
- Défaut : le listage n'offre aucun point d'annulation. `list_archive_directory` ne prend pas de checkpoint, et `index_rar_set` reçoit un checkpoint vide.
- Scénario : un utilisateur entre dans plusieurs gros `.tar.xz` ou RAR multivolumes puis en ressort aussitôt. Chaque listage occupe un thread bloquant et un worker jusqu'à 120 s (LISTING).

### 12. `src/commands/archive_io_codec/parallel.rs:65`
- Gravité : faible. Confiance : plausible.
- Défaut : le `Drop` de `Job` attend la fin du thread d'encodage. Sur erreur ou annulation dans `drain_one`, chaque tâche en cours est attendue jusqu'au bout.
- Scénario : l'utilisateur annule une compression ZIP en Turbo qui a 16 fichiers de 200 Mio en vol, au niveau Best. Le worker ne se termine qu'après la fin de ces compressions inutiles.

### 13. `src/commands/archive_io_worker.rs:57` et `src/commands/archive_io_codec.rs:77`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : chaque requête non-checkpoint est précédée d'un rendez-vous Checkpoint. `ReportedReader` envoie en plus un `Decoded` à chaque `read`, et `BudgetReader` y ajoute encore un checkpoint. Cela fait environ 5 allers-retours IPC par lecture.
- Scénario : décoder un `.tar.gz` de 100 000 petits fichiers coûte plusieurs centaines de milliers d'allers-retours par passe, alors qu'un `Decoded` agrégé suffirait.

### 14. `src/commands/archive_iso_open.rs:178` et `src/commands/archive_io_codec.rs:336`, `:505`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : ISO et TAR lisent le `Reader` distant sans tampon. L'index ISO lit chaque enregistrement en deux fois (1 octet, puis le reste), avec un checkpoint et une requête IPC à chaque fois. Le TAR lit chaque en-tête de 512 octets par une requête séparée.
- Scénario : l'index d'une ISO de 50 000 entrées dans le worker de décodage déclenche environ 300 000 allers-retours IPC. Un `WindowReader`, déjà utilisé pour ZIP, les réduirait d'un ordre de grandeur.

### 15. `src/commands/archive_io_codec.rs:257`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : la boucle du manifeste ZIP appelle `by_index_raw`, qui relit l'en-tête local de chaque membre. Au travers de `WindowReader`, chaque en-tête hors de la fenêtre courante déclenche une lecture de 256 Kio.
- Scénario : ouvrir un seul fichier dans un ZIP de 10 000 photos de 3 Mo transfère environ 2,5 Go par IPC avant de décoder quoi que ce soit.

### 16. `src/commands/archive_io_codec/parallel.rs:94`
- Gravité : optimisation. Confiance : plausible.
- Défaut : en Turbo, chaque petit fichier ZIP reçoit son propre thread OS, un `ZipWriter` préalloué d'au moins 64 Kio, puis un nouveau `ZipArchive` est analysé pour la copie brute.
- Scénario : compresser un dossier de 100 000 petits fichiers crée 100 000 threads et autant d'archives temporaires. Un pool fixe de N threads suffirait.

### 17. `src/commands/archive.rs:231` et `src/commands/archive_handle_worker.rs:108`
- Gravité : optimisation. Confiance : confirmé.
- Défaut : chaque navigation dans un sous-dossier relance un worker qui réindexe toute l'archive puis filtre un seul niveau. Aucun cache n'existe dans `commands/io.rs`. Pour un TAR compressé, tout le contenu est décompressé à chaque fois.
- Scénario : parcourir cinq niveaux d'un `.tar.zst` de 4 Go décompresse 20 Go au total.

<!-- LOT TERMINÉ -->
