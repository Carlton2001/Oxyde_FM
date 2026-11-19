# Plan de remédiation de l'audit

Ce plan découle de [99-synthese.md](99-synthese.md). Les identifiants (E, M, F, O, WM, WF, LM, LF, WI) sont ceux de la synthèse. Les lots se traitent dans l'ordre ci-dessous, en mode normal (sans sous-agents), un constat à la fois.

## Règles communes

- Branche de travail `audit-fixes`, fusionnée en local dans `main` à la fin d'un lot. Aucun push sans demande explicite pour ce push (règle du projet).
- Lot 1 fusionné le 2026-11-18 (`c7745898`, fusion sans avance rapide), `audit-fixes` repartie de `main`.
- Lot 4 fusionné le 2026-11-18 (`2ebc41a7`, fusion sans avance rapide), `audit-fixes` réalignée sur `main`. Validé à la main : ZIP de 5,7 Go, clé exFAT (copie, compression, extraction, Oxyde portable). La marque Internet disparaît en copiant vers exFAT, comme avec l'Explorateur. Puis validés le même jour : ISO de Windows 11 montée, dates des ZIP dans l'Explorateur, archive chiffrée sur lecteur mappé, tar.gz de sources GitHub. `docs/audit/` reste hors Git (décision du 2026-11-18).
- Lot 6 fusionné le 2026-11-18 (`bd63e890`, non poussé). Restes dans le lot 9.
- Lot 9 fusionné le 2026-11-18 (`bfdc563b`, fusion sans avance rapide, non poussé), `audit-fixes` réalignée sur `main`. F11 validé à la main sous Windows 11 le 2026-11-19 (fichier verrouillé dans un lot de 137 : seul lui échoue, les autres partent à la corbeille)  ; F25 validé à la main le 2026-11-19 avec un ZIP de l'Explorateur, une fois les fichiers en flux acceptés au dépôt ; WM6 hors périmètre (2026-11-19).
- Revue des lots du profil C fusionnée le 2026-11-19 (`a8fc0417`, fusion sans avance rapide, non poussée), `audit-fixes` réalignée sur `main`.
- Lot 5 fusionné le 2026-11-18 (`cb29ff1e`, fusion sans avance rapide), `audit-fixes` réalignée sur `main`. IME japonais validé sous Windows le 2026-11-19.
- Un commit par constat, message en anglais sur une seule ligne.
- Avant de corriger : relire le code actuel (l'audit date du 2026-11-17 et ne couvre pas les 5 commits de la section Fraîcheur) et reproduire le défaut quand c'est possible. Si le constat n'est plus vrai, le noter dans le suivi sans modifier le code.
- Pour chaque correction : un test de régression dans le dossier `tests/` du module (sans élargir la visibilité), puis `cargo check` sans avertissement du projet.
- Tout message ou libellé nouveau est ajouté aux catalogues des cinq langues, avec des couleurs sémantiques. Aucun changement de curseur, aucune icône hors Lucide.
- Quand un worker ou son code partagé change : rappeler qu'il faut fermer Oxyde, lancer `cargo build --offline --bins`, puis `cargo run --offline`. Pour `shell/oxyde-file-dialog` : `cargo build --offline --manifest-path shell/oxyde-file-dialog/Cargo.toml --target-dir target`.
- Les corrections qui ne se testent que sous Linux ou sur une configuration Windows particulière sont regroupées (lots 6 et 7) et marquées « à tester » dans le suivi.

## Modèles et efforts

| Profil | Modèle | Effort | Usage |
| --- | --- | --- | --- |
| B | Opus 5.5 | high | perte de données, sécurité, concurrence, COM et noyau Windows, archives, E10 |
| C | Sonnet 5.5 | medium | correctifs locaux validés par un test, moyens, faibles, optimisations |

Pas de xhigh dans ce plan : high suffit pour tous les lots.

Changer avec `/model` puis `/effort` au début de chaque lot.

## Ordre des lots

### Lot 0 : préparation (Sonnet 5.5, medium)

- Créer la branche `audit-fixes` et remplir le suivi ci-dessous.
- Décider si `docs/audit/` est committé.
- Option : exécuter les tests manuels WI1 (lecteur SUBST, réseau mappé, `\\?\UNC` sous Windows 11) pour savoir si le lot 3 contient un constat élevé.

### Lot 1 : perte de données irréversible (profil B)

1. E8 : confirmation de la purge de la corbeille (Suppr, menu contextuel, barre, propriétés, vidange Windows).
2. E9 : Ignorer et Remplacer ne comptent ni les clics répétés ni la répétition de touche.
3. E2 et WM7 : avertissement de suppression définitive à la mise à la corbeille Windows, stratégies `NoRecycleFiles` et `RecycleBinSize`, reçu accepté sans destination.
4. M1 : élaguer la sélection (filtre, groupes repliés, fichiers disparus) avant Supprimer, Copier, Couper, Compresser.
5. E5 : nom libre testé sans suivre les liens, plus de boucle infinie sur un lien symbolique cassé (compress.rs, extract.rs, et le constat faible sur les conflits par existence).

### Lot 2 : plantages, blocages et incohérences d'interface (profil C)

M4, M5, M7, M8 (paniques), M2 et M3 (renommage), M6 (champ masqué), M9 à M12, M16 à M19, M23 à M25, M100 (tubes nommés, à tester sous Linux pour la partie FIFO). Faibles voisins de ces fichiers traités au passage.

### Lot 3 : chemins Windows et famille 4395 (profil B)

- M70 (points et espaces finaux), M71 (préfixe verbatim UNC et Volume), M79 (explorer.exe et virgules), M72, M73.
- Famille 4395 : WM1, WF2, WI1 (selon le test du lot 0), WM2 (volumes absents du gestionnaire de montage), WM5 (sémantique POSIX de suppression), WF3, WF4.
- Workers touchés : rebuild obligatoire.

### Lot 4 : archives (profil B)

- E3, WM3, WM4 (FAT32, exFAT, flux alternatifs absents).
- E4, M33, M34 (TAR PAX, sparse, liens, noms courts non UTF-8).
- M35, M36 (règles Windows et casse appliquées sous Linux), M44 (clé du magasin de mots de passe), M39, M41, M42, M43, M45, M37, M38, M40.
- Hors audit, trouvé sous WSL le 2026-11-18 : `systems::archive_open_cache::tests::pressure_evicts_unmodified_closed_entries_before_expiry` échoue par intermittence sous Linux (`Full`, environ 1 lancement complet sur 15, jamais seul). Cause probable : un descripteur hérité par un processus lancé en parallèle tient le fichier au-delà des deux reprises de 5 ms de `linux_cache::open_cleanup`. Reproduire, puis décider entre le test et la logique de reprise.
- Workers touchés : rebuild obligatoire.

### Lot 5 : composition IME (profil B, une session dédiée)

- E10 : suivi de la composition IME dans `ui/components/input.rs` (plage marquée, préédition, fin de composition). Tester sous Windows ; Linux (IBus, Fcitx) dans le lot 7.

E1 (DLL x64 inscrite sur Windows ARM64) et LM1 (valeurs `O_NOFOLLOW` x86 codées en dur, builds arm et aarch64) sont **hors périmètre** : seules les architectures x86-64 sont prises en charge. Ils restent dans la synthèse à titre d'information et ne sont pas corrigés.

### Lot 6 : moyens restants (profil C), par module

1. Interface : M13 à M15, M20 à M22, M26 à M32, M46 à M51, M101.
2. Fichiers et opérations : M52 à M56, M59 à M67.
3. Windows (plateforme) : M68, M69, M74 à M78, M80 à M92, WM6, WM8.
4. Faibles Windows : F1 à F31 et WF1, WF5 à WF8, par fichier.

### Lot 7 : session Linux (profil B)

E6 (descripteurs, `RLIMIT_NOFILE`), E7 (`wl-copy` sans session Wayland), M93 (frontière de système de fichiers, à reproduire d'abord : non revérifié), M57, M58, M94, M95, M98, M99, LM2 à LM4, LF1 à LF4, test IME sous IBus et Fcitx (E10). Tout se vérifie sur une machine Linux.

### Lot 8 : faibles et optimisations restants (profil C)

Faibles hors Windows et optimisations (O1 à O103), traités au fil de l'eau quand un fichier est déjà ouvert pour un autre lot. Les optimisations sur le thread UI (M31, M32, M67, O58 et voisines) passent en premier. À relire avant : la vue tuiles et le tri par profondeur de `popup/window.rs`, non audités.

### Lot 10 : reportés du lot 8 (décisions, mesures ou Linux)

Rien n'est en cours dans ce lot : il rassemble ce que le lot 8 n'a pas traité, avec la raison.

- Demande une décision : F133 (borne de la limite de recherche, valeur affichée dans les réglages), O51 (enregistrer les préférences à chaque cran de molette : temporisation ou non).
- Demande un changement de protocole entre processus ou de type partagé : O1, O2, O35 (pochette en tableau JSON), O37, O92 (aperçu cloné à chaque contrôle d'état, texte à passer en `Arc`), O95, O97.
- Demande une mesure ou un index nouveau, risque sur la sécurité du moteur de copie : O13, O14, O15, O17, O19, O20, O21, O22, O28 (noms de restauration résolus avant la sélection), O82 (ordre de propagation des choix aux enfants), O85 (résumé de sélection à mettre en cache par révision), O86, O100, O101.
- Archives, workers et IPC : O3 à O11, O24 à O26, O36, O39, O41 à O47 (le reste de O49).
- Linux seulement, à compiler et tester dans WSL : O29, O30, O40, O41, O42, et les faibles Linux (section « Plateforme Linux et services communs » de la synthèse).
- Interface à revoir une par une avec une mesure : O50, O52 à O57, O59 à O74, O76 à O78, O81, O83, O84, O87, O89 à O94, O96, O98, O102, O103. O60 laissé : fenêtre du sélecteur, rare.
- Faibles non examinés : F70 à F89 (hors F90, F97, F100, F101 faits), F91 à F96, F98 à F99, F102 à F123, F124 à F128, F130 à F136, F138 à F148 et suivants, ainsi que la section « Interface : application, fenêtres et composants ».

## Suivi

Statuts : à faire, en cours, corrigé (hash), obsolète, reporté. Mettre à jour après chaque commit.

| Lot | Constat | Statut | Commit | Remarque |
| --- | --- | --- | --- | --- |
| 1 | E8 | corrigé | 28e32428 | validé le 2026-11-18 ; vider la corbeille demande toujours ; purge d'une sélection selon la préférence (comme Maj+Suppr) ; Linux non compilé ici |
| 1 | E9 | corrigé | 1cc5172d | clic simple au relâchement, répétition ignorée ; X11 ne signale pas la répétition (lot 7) |
| 1 | E2 + WM7 | corrigé | 18d5b708, b64aa907 | SUBST vérifié (refus, fichier conservé) ; fichier trop gros : avertissement natif de Windows vu le 2026-11-18 ; partage réseau bloqué par l'erreur 4395 (famille WI1, lot 3), à retester après le lot 3 ; stratégie NoRecycleFiles non testée ; RecycleBinSize non lu (capacité affichée seulement) |
| 1 | M1 | corrigé | d4743871 | filtre et fichiers disparus élagués à l'action ; groupes repliés gardés (choix existant, comme l'Explorateur) |
| 1 | E5 | corrigé | 2da636fa, a16779c0 | + constat faible conflits par existence et compress.rs:271 |
| 2 | M4, M5, M7, M8 | corrigé | 59ae979a, f21edafa, 0f3b7790, dab77623 | M4 test unitaire, M8 test unitaire, M5 et M7 sans test automatique (UI), à vérifier à la main |
| 2 | M9, M10, M11, M12 | corrigé | 4 derniers commits | sans test automatique, à vérifier à la main (éjection avec dossier surveillé supprimé, glisser depuis la corbeille puis changer d onglet) |
| 2 | M16, M18, M19, M23, M24, M25 | corrigé | 6 derniers commits | sans test automatique, à vérifier à la main ; M23 Linux seulement (invite smb refermée en changeant d'adresse) |
| 2 | M17 | reporté | | exige que le moteur de copie, déplacement et suppression dise quelles sources sont terminées ; une heuristique risquerait d'omettre des sources en silence |
| 2 | M2, M3, M6 | corrigé | HEAD~1, HEAD | sans test automatique (UI), à vérifier à la main : F2 avec plusieurs éléments, F2 et Ctrl+Maj+N dans la corbeille, copier/couper/double-clic dans le champ mot de passe |
| 2 | M100 | corrigé | HEAD | test unitaire du tube sous Linux et modules linux_* non compilés sous Windows : à vérifier sous Linux |
| 3 | WI1 | corrigé | c684ac02 | reproduit par un test (`\\localhost\C$`) ; repli par composants sur 4395 quel que soit le noyau ; à retester : lecteur mappé, `\\?\UNC`, et E2 sur partage réseau |
| 3 | WF2 | corrigé | 87e8266d | noyau sans OBJ_DONT_REPARSE : parcours par composants, refus par OPEN_REPARSE_POINT et contrôle final ; non testable sans ancienne build |
| 3 | WM1 | corrigé en partie | 45d0f1a8, ca25109e | volume monté dans un dossier (enregistré par le gestionnaire de montage) atteint par sa racine de volume ; racines de l'application (exe, cache, temp, config, `data` portable) résolues une fois (jonctions de profil, Scoop) ; **reporté** : étiquettes qui ne sont pas des liens (OneDrive, WOF, déduplication), refus conservé car OPEN_REPARSE_POINT lirait un fichier WOF vide |
| 3 | WM2 | corrigé | 246d4bd3 | chemin final hors gestionnaire de montage (ImDisk, redirecteurs) : nom NT traduit par la lettre ou `\Device\Mup`, gardé seulement s'il rouvre le même objet |
| 3 | WM5 | corrigé | a8255fa6 | sémantique POSIX, repli hérité sur FAT, exFAT, réseau ; lecture seule toujours refusée |
| 3 | WF3 | corrigé | 644cd3ae | STATUS_NO_SUCH_FILE à la première requête = dossier vide |
| 3 | WF4 | obsolète | | `set_hidden_attribute` n'a aucun appelant hors tests |
| 3 | M70 | corrigé | c6ca52bc | les actions Shell refusent un nom finissant par un point ou un espace (au lieu d'agir sur son jumeau) ; `validate_path` ne rogne plus un chemin `\\?\` |
| 3 | M71 | corrigé | 630b6e3d | fonction commune `path_text::without_verbatim_prefix` : reçus de suppression, PDF, doublons |
| 3 | M79 | corrigé | 77527cb7 | guillemets pour virgule, point-virgule, égal, espace |
| 3 | M72 | corrigé | 5e2e66db | racine reconnue par identité quand le Shell la rapporte sous une autre forme, puis preuve exigée |
| 3 | copie vers la racine d'un partage | corrigé | 0e8e2529 | trouvé au test manuel du 2026-11-18 (« Invalid planned destination component ») : Windows nomme la racine `\\?\UNC\serveur\partage` sans séparateur final ; défaut antérieur au lot 3 |
| 3 | suppression sur un partage en 3 étapes | corrigé | 72f1595f | trouvé au test manuel du 2026-11-18 : partage, SUBST, lecteur amovible ou optique, ou volume réglé en suppression définitive → confirmation de suppression définitive d'emblée et opération définitive ; demandée même si les confirmations sont désactivées (Maj+Suppr excepté) ; le contrôle du Shell (E2) reste en filet ; libellés et icônes (menu contextuel, barre, info-bulle) suivent la même règle (commit suivant) |
| 3 | M73 | corrigé | 79e6fcd9 | au-delà du budget ou sans preuve, le descendant n'est pas enregistré mais la suppression continue ; l'erreur d'un échec partiel le signale |
| 4 | E3 + WM3 | corrigé | 5c5e76f0 | volume sans flux nommés (FAT32, exFAT, partage `\\wsl.localhost`) : aucune marque lue ni écrite, au lieu d'un fichier frère `nom:Zone.Identifier` ; test sur un partage WSL ; rebuild workers |
| 4 | WM4 | corrigé | 1a678d2e | dossier temporaire sans droits d'accès (portable sur FAT32/exFAT) : le bac à sable des workers et le cache des fichiers ouverts depuis une archive passent au dossier temporaire du système (fichiers transitoires seulement), décision du 2026-11-18 ; détection testée sur un partage WSL ; à tester sur une vraie clé exFAT ; rebuild workers |
| 4 | E4 + M33 + M34 | corrigé | 6dc5ac9b, 08f41123, 442dd862 | en-têtes PAX globaux ('g', git archive) ignorés ; membres contigus ('7') et GNU sparse lus comme fichiers ; PAX `GNU.sparse.*` refusé ; vérificateur : clés PAX informatives et 'K' acceptés, nom d'en-tête décodé seulement sans nom long ; rebuild workers. Liens TAR : refus de toute l'archive **conservé** (décision du 2026-11-18) : politique documentée (`archive-link-refused`) ; les ignorer demanderait un changement de protocole, du vérificateur indépendant et un avis visible pour ne pas livrer en silence une arborescence incomplète |
| 4 | M35 | corrigé | a4b70e91, a59d47c2, f6a3a3c0 | règles de nommage Windows seulement sous Windows ; `..` et caractères de contrôle refusés partout |
| 4 | M36 | corrigé | 2b5468fb | doublons sans casse seulement sous Windows |
| 4 | M44 | corrigé | 0d9a6857, d4af0154 | mot de passe mémorisé sous le chemin affiché et le chemin résolu ; résolution hors du fil d'interface |
| 4 | M37 | corrigé | 0b81ed36 | en-têtes ZIP64 dès 0xF000_0000 octets |
| 4 | ZIP de plus de 4 Gio en build de développement | corrigé | 96bda4dc | trouvé au test manuel du 2026-11-18 (fichier de 5,7 Go, « Le traitement de l'archive a échoué ») : zlib-rs garde des compteurs 32 bits sous Windows et les additionne sans bouclage ; avec les contrôles de débordement du profil dev, le worker paniquait à 4 Gio de sortie (et paniquerait à 4 Gio d'entrée à l'extraction) ; release non touchée ; contrôles coupés pour zlib-rs en dev ; test ignoré `a_zip_member_compressing_past_4_gib_is_written` ; aller-retour de 4,4 Go vérifié par le worker (octets identiques, `unzip -l` correct) ; rebuild workers |
| 4 | coller à la racine d'une clé exFAT | corrigé | b934ef74 | trouvé au test manuel du 2026-11-18 (« Filesystem did not provide a stable file identity ») : FAT32 et exFAT donnent l'ID 0 à leur dossier racine ; une racine de volume, qui ne peut être ni renommée ni remplacée, est identifiée par son volume ; copie, déplacement, compression et extraction vérifiés sur G:\ (exFAT), sans fichier `:Zone.Identifier` |
| 4 | M38 | corrigé | 33ad76b1 | dates ZIP écrites et lues (NTFS, UT, sinon DOS en heure locale) ; le bac à sable Linux voit `/etc/localtime` et `TZ` ; modes ZIP non écrits |
| 4 | M39 | corrigé | 66bddc16, e1f54be5 | ouverture d'un membre : flux décodé borné par l'allocation de listage (TAR compressé, 7z solide) ; protocole broker/worker modifié, rebuild workers |
| 4 | M40 | corrigé | dc312d8a | descripteur UDF (NSR02/NSR03) après le jeu ISO 9660 : image refusée en lecture virtuelle, donc montée |
| 4 | M41 | corrigé | 0eb561f8 | une préparation de plus d'une heure n'occupe plus de place « en préparation » |
| 4 | M42 | corrigé | 3de24609 | « existe déjà » garde son genre : « Nouveau dossier (1) » est créé |
| 4 | M43 | corrigé en partie | 2b6aa0ae | une seule extraction et une seule reconstruction quel que soit le nombre de noms pris ; **reporté** : progression et annulation, qui demandent un nouveau genre d'opération dans le panneau |
| 4 | M45 | corrigé | d1a83246 | cellules au-delà de 64 colonnes ignorées au lieu d'écraser la 64e (xlsx, docx, odt, ods, pptx) |
| 4 | test Linux intermittent | corrigé | 097edead | cause : un processus en cours de lancement partage les descripteurs ; reprises du bail portées à 5, 10, 20, 40 ms ; 0 échec sur 120 lancements (1 sur 40 avant) |
| 5 | E10 | corrigé | 4e639422 | plage marquée suivie (`input/composition.rs`), préédition soulignée et remplacée à chaque mise à jour puis par le résultat ; annulation (message vide du IME japonais) sans effacer la sélection ; coller, couper, effacer décalent la composition ; complétions de chemin suspendues pendant la composition ; quitter le champ garde la préédition comme texte saisi ; IME japonais validé sous Windows le 2026-11-19 (chinois, coréen non testés) ; Linux (IBus, Fcitx) au lot 7 |
| 5 | E1 | hors périmètre | | ARM64 non pris en charge |
| 6 | M13 | corrigé | 804543e7 | réorganisation ou appui terminé si le dossier est désépinglé ; sans test automatique, à vérifier à la main |
| 6 | M14 | reporté | | comportement voulu et testé (premier choix en mode avancé applique l'opposé aux autres) : décision de conception |
| 6 | M15 | corrigé | a1832d9c | filtre ou tri saisi pendant le chargement réappliqué |
| 6 | M20, M21 | corrigé | ddd51e69, 2dbfb29f | champ de chemin du sélecteur : texte identique au dossier courant = fermeture sans rechargement ; Entrée ignorée pendant le tri/filtre en arrière-plan |
| 6 | M22 | corrigé (lot 7) | 9eb9b039 | liste publiée d'abord, types MIME lus ensuite par lots de 256, arrêtés à la navigation, gardés en cache (date et taille) pour Retour et F5 |
| 6 | M26 | corrigé | 8501e8e6 | mise à jour reproposée si la fermeture est refusée (l'installeur déjà lancé attend) |
| 6 | M27 | corrigé | 07d4f7bc | plus de 20 éléments : refus avec message (5 langues), pas de confirmation |
| 6 | M28 | corrigé en partie | b6789b89 | tâche de l'aperçu gardée : un travail en file disparaît à la fermeture de l'infobulle ; M29 corrigé à la reprise (lecture bloquante qui occupe l'emplacement unique : exige un délai) |
| 6 | M30 | corrigé | 667cc2d7 | `smb://` gardé dans les cibles du fil d'Ariane ; deux-points final lu comme lecteur seulement sous Windows ; Linux non compilé ici |
| 6 | M31 | corrigé en partie | cd347c81 | sélection de glisser partagée (Rc) au lieu d'une copie par ligne ; **reporté** : copie de `selected_paths` par rendu (type partagé par ~90 sites) |
| 6 | M32 | corrigé en partie | e7f47d6c | un seul parcours au lieu de deux ; toujours sur le thread UI |
| 6 | M46, M48 | corrigé | 7894cd05 | BOM, UTF-16, cp1252 (visionneuse texte et Markdown) ; Markdown plafonné à 16 Mio |
| 6 | M47, M49, M50, M51 | corrigé | bd5c876a, 33981a1c, 19d780f0, 4921c8a5 | tableau, autolien linéaire, image en ligne (test), plage de pages PDF plafonnée |
| 6 | M101 | corrigé | 1146aec9 | plus de balayage des miniatures par fichier au vidage du cache |
| 6 | M52, M53 | corrigé | 1a672efd | réservations indexées (linéaire) ; garde qui libère la réservation même sur panique |
| 6 | M54, M61 | corrigé (lot 7) | 0dfb1121 | voir M99 |
| 6 | M55 | corrigé | 5769f3b0 | lignes par fichier au niveau debug (`RUST_LOG=oxyde_lib::copy_timing=debug`), doc mise à jour |
| 6 | M56 | corrigé | 95de45ca | un dossier qui contient l'archive cible est refusé |
| 6 | M59, M60 | corrigé | 365dc639, 7e3722fb | entrée illisible ignorée au listage ; conflits : profondeur 128 et sous-dossier illisible ignoré |
| 6 | M62 | corrigé (reprise) | | comportement testé (ligne non UTF-8 = fichier abandonné) : choix à faire entre binaires et cp1252 |
| 6 | M63, M64 | corrigé (annulation à la reprise) | 81be1175, 00924c95 | vérification de la racine hors thread UI, plafond de 20 000 dossiers ; exclusions Linux (lot 7) : `/proc`, `/sys`, `/dev`, `/run`, systèmes pseudo (statfs) et montages réseau ou FUSE ; **reporté** : annulation |
| 6 | M65 | corrigé | 7c096532 | `themes.json` illisible mis de côté en `themes.json.invalid`, réglages chargés |
| 6 | M66 | corrigé | 0482d2b8 | jonctions et liens de dossier traités comme des dossiers (liste et arbre) ; à vérifier à la main sous Windows |
| 6 | M67 | reporté | | instantané complet par rendu : refonte du panneau des opérations |
| 6 | M77, M78, M84, M85, M89, M91 | corrigé | 928f7c98, 61a4a60a, a1021869, 8190cc52, ef8f8c04, 13d21893 | souris inversée, valeurs DWORD de la corbeille, clé de cache réparée (test), icônes sans alpha (test), presse-papiers fermé avant les stat, GetFileName nul |
| 6 | M86 | voir lot 9 | | traité au lot 9 (96be2854) |
| 6 | M68, M69, M75, M76, M80 à M82, M87, M90, M92, WM8 | corrigé | 6327bb78, f5ceb5da, ff167822, 3f0e28cb, e8695edb, 1bc563b7, 5847c923, 164624ea, aa00f6bd, eab2fc6f, c29b7617 | délai de 3 s pour l'Explorateur, requête d'instance résolue hors du thread du pipe, en-têtes vidéo sur un autre thread, cible de raccourci inchangée non réécrite, liste de la corbeille sur son thread, collage Shell pompé tant que la fenêtre de progression existe (à tester à la main), session PDF fermée sur erreur de transport, balayage des copies LPAC, lecture du lanceur bornée, OnOverwrite relayé, fichiers cloud exclus de la recherche de contenu |
| 6 | M74, M83, M88 | voir lot 9 | | traités au lot 9 (M74 obsolète, M83 corrigé, M88 obsolète en partie) |
| 6 | WM6 | voir lot 9 | | corrigé au lot 9 (1c16de6a) ; test sur Win10 hors périmètre (décision du 2026-11-19) |
| 6 | F5, F7, F9, F10, F12, F13, F14, F17, F19, F20, F21, F22, F23, F24, F28, F29, F30, F34 | corrigé | | commits de la session du 2026-11-18 (tampon des arguments du jump list, relâchement mémorisé, DPI nul, délai vidéo 60 s, .lnk atomique, lecture seule en suppression définitive, presse-papiers avec réessais, images disque, HICON détruits, Corbeille déjà vide, journal Accès rapide) |
| 6 | F8 | obsolète | | « Exécuter en tant qu'administrateur » retiré du menu de la barre des tâches (df1ff6ba) |
| 6 | F1 à F4, F6, F16, F18, F27, WF5, WF7 | corrigé | | message tronqué ignoré, requête de dialogue retirée si le lanceur est introuvable, chemins non transmissibles journalisés, menu de la barre des tâches sérialisé, manifeste de propriétés dans le dossier d'Oxyde en repli, attente de 150 ms entre éléments de sélection, attribut lecture seule vraiment effacé (corrige aussi F19), seuil de mémoire proportionnel, lecteur système et SystemRoot |
| 6 | WF1 | obsolète | | seule Windows 10 1507 est concernée (les trois imports DPI sont de 1607) : hors périmètre |
| 9 | M86 | corrigé | 96be2854 | 6 workers ; l'échec « monitor budget » ne venait que des tests d'intégration lancés en parallèle : verts avec `--test-threads=1` (comme le script officiel) et unitaires verts en parallèle |
| 9 | F11 | corrigé | b3c62fa6 | fichier disparu ou refusé : échec isolé ; un refus du puits annule la suite du lot natif, les fichiers non atteints sont relancés dans une nouvelle opération |
| 9 | F15 | corrigé | 1777f69e | l'appelant reçoit le résultat dès l'ouverture ; le thread natif attend seul la fermeture (pas de borne : elle couperait une feuille ouverte) |
| 9 | F31 | corrigé | 99440c20 | contrôle préalable de tout le lot sans garder les handles, puis restauration par blocs de 64 recontrôlés |
| 9 | F25 | corrigé | 876cb664 | chemins lus une fois par objet glissé (identité COM), réutilisés à la ré-entrée et au dépôt ; pas de GetData hors thread UI (objet lié à son appartement) |
| 9 | F26 | corrigé en partie | aa79fa36 | Windows n'a pas de remplacement atomique : révocation tolérée si rien n'est enregistré, une nouvelle tentative, message clair |
| 9 | WF8 | corrigé | a6e13f8c | le verbe « openas » du sous-menu est lu par GetMenuItemInfo (vérifié sur ce poste) ; libellés gardés en repli |
| 9 | M74 | obsolète | ca79e66c | vérifié : Windows ne détruit pas les fenêtres possédées d'un autre thread, il leur retire le propriétaire ; le placement suivant les rend à Oxyde (test ajouté) |
| 9 | WM6 | corrigé, hors périmètre | 1c16de6a | `\wsl$` pour les builds < 22000, les deux racines reconnues ; WSL sous Windows 10 déclaré hors périmètre par l'utilisateur le 2026-11-19, non testé |
| 9 | M83 | corrigé | d1302a19, bed6a16c | listage abandonné quand l'onglet change de dossier (Windows et Linux) ; vérification de mot de passe inchangée |
| 9 | M88 | obsolète en partie | | Viewer : les requêtes périmées sont déjà sautées avant leur tour (`wanted`, ed41a5a3) ; icônes : assets GPUI sans notion de visibilité, requêtes courtes, laissé tel quel |
| 9 | WF6 | corrigé | 5eb3a624, 9bb31de9, f7a68393, ffe8b181 | lecteur mappé hors ligne nommé par son partage sans attente réseau, WebDAV nommé par dossier/hôte, occupation de la corbeille sur fixes et amovibles, WebDAV exclu des serveurs, images reconnues via SUBST/lecteur mappé/point de montage, lettres réseau sautées ; à tester à la main |
| 7 | E6 | corrigé | 374c5078 | limite douce portée à la limite dure au démarrage, limite reçue rendue aux programmes lancés (Command et GIO) ; EMFILE reproduit à 256 puis corrigé (WSL) |
| 7 | M57, M58 | atténué | 374c5078 | couverts par le relèvement de E6 (plus de descripteurs disponibles), structure inchangée |
| 7 | E7 | corrigé | de492644 | `WAYLAND_DISPLAY` vide ou absent : ni `wl-copy` ni `wl-paste` ; `wl-copy` ensuite remplacé par LM4 |
| 7 | M93 | corrigé | 2f14fc1a | frontière par statx `STATX_MNT_ID` (montages liés), point de montage sélectionné refusé ; reproduit (3 tests) puis corrigé sous `unshare` |
| 7 | M94 | corrigé | 758ddbed | fichier des serveurs illisible et non vide : jamais réécrit |
| 7 | M95 | corrigé | 7303879a | image supprimée (« (deleted) ») encore démontée par son fichier ; testé avec udisks2 sous WSL |
| 7 | M98 | corrigé | 7ef1e7ce | quota du cache d'aperçus comme Windows (256 Mio, 4096 entrées, 4 Mio par entrée), noms partagés dans `preview_cache_names.rs` |
| 7 | M99 (M54, M61) | corrigé | 0dfb1121 | corbeille freedesktop propre (parent tenu, identité vérifiée, `RENAME_NOREPLACE`, corbeille du montage), entrée gardée comme reçu pour Annuler et Rétablir |
| 7 | LM1 | hors périmètre | | aarch64 non pris en charge |
| 7 | LM2, LF1 | corrigé | 8bc2c37b | sonde bubblewrap avec le bac à sable complet ; `/nix/store` et `/gnu/store` liés et permis à Landlock |
| 7 | LM3, LF3 | corrigé | b42afd99 | sans UDisks2, règles d'affichage de GIO ; FUSE réseau et 9p hors statvfs |
| 7 | LM4 | corrigé | dec384fb | patch GPUI : fichiers offerts en `text/uri-list`, `x-special/gnome-copied-files` et `application/x-kde-cutselection` ; `wl-copy` retiré, `wl-paste` garde la lecture ; X11 : texte seul |
| 7 | LF2 | corrigé | ed27a8ac | paquet système reconnu par dpkg, pacman ou la base RPM |
| 7 | LF4 | corrigé | 5ebf8330, 0467c4eb, 6f5f0299 | FUSE exFAT, NTFS et GVfs insensibles à la casse ; bits spéciaux abandonnés sur FAT/exFAT (EPERM reproduit) ; cache d'archives dans `/tmp/oxyde-<uid>` privé sans bail (NFS, 9p) |
| 7 | E9 (X11), E10 (IBus, Fcitx) | à tester | | machine Linux réelle : voir tests-linux.md |
| 8 | O70, O80, O79 | corrigé | 7a6293f7, 527034ff, 3a8805d9 | arbre des volets emprunté au lieu d'être cloné ; titre du Viewer réécrit seulement s'il change ; état de glissement de la barre emprunté ; sans test automatique (UI). O51, O102 vus et laissés : gain faible ou comportement à discuter |
| 8 | O58, O75, O99, O16, O27, O18, O31, O38, O49, O48, O12, O88 (tri) | corrigé | 4 derniers commits d'optimisation | O58 positions par tranches ; O75 sans vecteur par image ; O99 chemins identiques court-circuités ; O16 et O27 tables de recherche ; O18 propriétaire de destination par ancêtres ; O31 requête littérale préparée une fois ; O38 cache d'attributs borné ; O49 (corbeille, ensembles normalisés, restauration d'historique laissée) ; O48 un registre lu par racine ; O12 chemin analysé une fois ; O88 clé de tri en minuscules. Tests : 1111 unitaires verts ; sans test automatique pour les optimisations UI |
| 8 | F90, F97, F101, F129, F137 (+ F100 obsolète : déjà couvert par E5) | corrigé | derniers commits | mot de passe refusé effacé ; date de suppression saturée ; suffixes TAR composés insensibles à la casse (fonction commune `split_compound_tar_suffix`) ; numéro de thème en u64 ; niveau de plan Word 9 = corps de texte ; tests de régression ajoutés |
| 8 | O34 | obsolète | | une seule `fs::metadata` par lien symbolique dans le code actuel |
| 8 | reste | reporté au lot 10 | | voir le lot 10 |
| 10 | F133, O51 (décisions) | corrigé | 8da43f74, e6587cce | F133 : limite de recherche plafonnée à 100 000 (valeur éditée à la main ramenée) ; O51 : une écriture en file sautée quand une plus récente la suit (couvre aussi la rafale de O33), recalcul des lignes en grille laissé |
| 10 | F212, F123 | corrigé | f34018de, b700f7a3 | opérations revérifiées après le téléchargement, installeur non lancé si occupé ; paquet téléchargé supprimé s'il n'est pas lancé |
| 10 | F208 | corrigé | de7a67cf | contrôle de la politique de corbeille borné à 5 s (le préfixe verbatim était déjà traité) ; au-delà, corbeille tentée, le Shell avertit |
| 10 | F119 | corrigé en partie | 40b32267 | l'erreur de retrait n'est plus masquée ; remplacement avant vérification laissé : l'utilisateur a demandé le remplacement |
| 10 | F121, F102, F122 | corrigé | 594876eb, 9279d872, 0dc16335 | enfant disparu ignoré (NotFound seulement) ; cible de remplacement disparue = création ; nom de copie raccourci sous 255 |
| 10 | F128 | corrigé | 6de8e9d5 | BOM accepté ; champ mal typé = valeur par défaut, copie `config.json.invalid` ; texte non JSON toujours refusé ; 4 réessais du remplacement ; UTF-16 non traité |
| 10 | F134 | corrigé | d64b8acb | verrou partagé `oxyde.log.lock` : rotation seulement si aucun autre processus n'écrit le journal |
| 10 | F76 | corrigé | 28ad1a8a | élément étranger ignoré à la racine du cache ; F77 **laissé** : les copies modifiées par l'utilisateur ne sont jamais supprimées (choix de sécurité des données) |
| 10 | F94, F120 | corrigé | 3f80b0d0, ba91edd7 | sélections disparues signalées en erreur ; note de staging gardée 90 jours si le volume est absent |
| 10 | F240, F204, F230, F231, F234, F247, F256 | corrigé | caab4e20, 072d3ebd, e26665a0, a617469f, 7eba1ce6, aecea229, 66b4cc9c | Entrée sans ligne clavier après mouvement du pointeur ; Instant sans soustraction qui panique ; index de clic sur frontière de caractère ; peinture sans unwrap ; clic réseau borné ; saut de ligne remplacé à la mise en forme ; stepper arrêté à la perte du focus |
| 10 | F72 | corrigé en partie | 7a996e47 | une erreur de somme après un membre déjà décodé = archive abîmée ; un seul membre corrompu reste lu comme mauvais mot de passe (indiscernable en ZipCrypto) ; rebuild workers |
| 10 | F135, F139, F145, F147 | corrigé | 0e1ab912, 5d1a3c05, f16051e4, 33f21c2e | `<si/>` compté ; zone de texte ODF dans un paragraphe (Viewer et recherche) ; animation abîmée gardée jusqu'au dommage ; délai nul = 100 ms côté interface ; rebuild workers |
| 10 | F187, F186, F185 (en partie) | corrigé | e5b1b339, 0efd5eeb | rafraîchissement en échec passager garde la liste ; changement d'onglet attend le listage en vol ; requête retirée si la préparation échoue |
| 10 | F205, F199 | obsolète | | marque web déjà écrite à l'ouverture depuis une archive (archive_io::decode) ; onglets suivis au renommage (`follow_renamed_item`) |
| 10 | F103 | obsolète | | le renommage d'un dossier entier n'existe que sous Linux (`same_mount` faux sous Windows), où les noms sont sensibles à la casse |
| 10 | F111 | corrigé (reprise) | | une annulation réussie n'a pas de canal d'avertissement : changer l'API d'historique |
| 10 | revue du lot 8 | vérifié | | les 21 commits relus : aucun défaut trouvé |
| 10 | F161 | corrigé | cecf5544 | nom d'archive proposé : nom entier d'un dossier (« v1.2 » n'est plus coupé) ; test unitaire |
| 10 | F132 | corrigé | 295d6a69 | identité d'historique lue à travers un verrou empoisonné, plus de panique dans Drop |
| 10 | F213 | corrigé | 927062e4 | extension média lue après un point seulement ; test unitaire |
| 10 | F232 | corrigé | e15aad56 | Retour arrière au début, Suppr à la fin : aucun changement annoncé |
| 10 | F229 | corrigé | a5510bc7 | date de suppression vide quand la Corbeille n'en donne pas (liste et info-bulle) |
| 10 | F206 | corrigé | 2647d08f | au-delà de cinq toasts, le plus ancien passager part, pas celui qui attend lecture |
| 10 | F211 | corrigé | 19772445 | espace initial ignoré après expiration de la recherche par frappe |
| 10 | F92 | corrigé | bb92d75c | échantillon de hachage partiel lu en entier (read_exact borné) |
| 10 | F167, F166, F164 | corrigé | f68b084d, c43b59be, c4fc5238 | doublons : indication de repos après annulation ; une seule fenêtre pour deux demandes ; emplacement injoignable écarté au lieu d'un échec (test unitaire) |
| 10 | F91 | corrigé | d87732d2 | UTF-16 avec BOM (.reg, PowerShell) décodé en flux pour la recherche de contenu ; UTF-16 sans BOM reste binaire ; tests unitaires |
| 10 | F108 | corrigé | 1bc3272c | reçu terminé suivi d'une erreur : compté une fois, terminé |
| 10 | F117, F115, F116 | corrigé | c6c01976, 9100f05b, 95fede51 | dedup après tri complet ; cause d'extraction gardée (nettoyage seulement journalisé) ; erreur typée gardée au retour arrière des renommages |
| 10 | F106 | corrigé | dfa3e5bb | racine de lecteur nommée par sa lettre ; casse repliée sous Windows seulement ; tests unitaires ; **Linux** : README et readme compressés ensemble |
| 10 | F222 | corrigé | 3ad095bd | tolérance de 2 s (FAT) pour « identique » et « plus récent » ; un dossier n'est jamais « identique » ; tests |
| 10 | F225 | corrigé | 7d7c2024 | nom finissant par un point : pas d'extension (colonne Type, filtre, renommage « supprimée ») ; test |
| 10 | F171, F172 | corrigé | b09c5c01 | sélecteur : erreur de listage gardée au tri ; Entrée n'accepte que si le bouton le peut ; à vérifier à la main (dossier refusé puis tri ; Entrée sans sélection en choix de fichiers) |
| 10 | série O | reporté | | chaque O demande une mesure ou un changement de protocole (liste ci-dessus) : non traité sans mesure, aucun gain prouvé |
| 10 | faibles restants | reporté | | F70-F75, F78-F89 (archives), F93, F96, F98, F99, F107, F109, F110, F112-F114, F118, F124-F127, F129-F131, F136-F146 (hors F139, F145), F148, reste de F149-F264 : non examinés dans ce lot |
| revue C | F10 | corrigé | 4f35253f | revue des lots 2 et 6 (profil C) le 2026-11-19 : le correctif du lot 6 (5953856e) bornait le multiplicateur de `point_to_owner`, pas les diviseurs ; `work_area_size` (ligne du constat) et `owner_scale` bornés à 96 ; pas de test : une fenêtre détruite fait déjà échouer `GetMonitorInfoW`, le cas reste une course |
| revue C | M76 (forme) | corrigé | c7b45666 | `windows_shortcuts.rs` contenait un octet NUL brut (Git le traitait en binaire) et était passé en CRLF ; échappement `'\0'` et LF rétablis, comportement inchangé |
| revue C | décodage du Viewer | corrigé | 68bbf3b8 | régression du lot 6 (7894cd05) : un fichier UTF-8 avec un seul octet abîmé s'affichait entier en Windows-1252 (mojibake) ; reste UTF-8 avec un caractère de remplacement ; test |
| revue C | commentaires `OxydeApp` | corrigé | 64b1723e | lot 2 : commentaire de la file des mots de passe posé sur `clipboard_gate` |
| revue C | file des mots de passe | corrigé | 58260e25 | lot 2 (cc96dced) : `dismiss_archive_password` (fenêtre impossible à ouvrir) relance maintenant la file ; pas de test (contexte GPUI) |
| revue C | listage tronqué | corrigé | db103739 | lot 6 (365dc639) : au-delà de 64 entrées illisibles, le listage s'arrêtait et passait pour complet ; il échoue maintenant ; test |
| revue C | ouverture de plus de 20 éléments | corrigé | b04faa1e | lot 6 (07d4f7bc) refusait ; remplacé par une confirmation au-delà de 15 éléments, comme l'Explorateur (dialogue fixe `OpenMany`, 5 langues) ; validé à la main le 2026-11-19 |
| revue C | indentation | corrigé | a37f34b8 | `remote.rs`, `windows_shortcuts.rs` |
| revue C | ordre de deux lancements | laissé | | lot 6 (f5ceb5da) : un fil par lancement, deux lancements à quelques ms d'écart peuvent arriver inversés ; voulu pour qu'un partage bloqué ne retienne pas les suivants |
| reprise | tableau du lot 6 | corrigé | | lignes M86, M74/M83/M88 et WM6 du lot 6 renvoyées au lot 9, où elles ont été traitées (2026-11-19) |
| reprise | M62 | corrigé | 455c629b | recherche de contenu : un octet hors UTF-8 se lit en Windows-1252 (ligne courte et ligne longue en flux), la lecture continue ; table cp1252 partagée avec le Viewer (`utils::legacy_text`) ; binaires toujours écartés par l'octet nul ; tests |
| reprise | F111 | corrigé | 2b251e82 | annulation réussie mais copies en attente non supprimées : notification d'erreur « Opération annulée, mais… » avec leurs emplacements (5 langues) ; `undo_last_action_report` ; test |
| reprise | M29 | corrigé | 65b51b17, 6a87b783, be4d67c2 | lecture d'aperçu au survol bloquée : l'emplacement est rendu après 20 s (au-delà des 15 s des workers), jusqu'à 4 lectures bloquées tolérées ensuite l'attente reprend ; test ; test intermittent des files d’aperçu corrigé (verrou partagé) ; plantage au premier survol (minuterie créée hors de Tokio) trouvé au lancement et corrigé, test |
| reprise | M63, M64 (annulation) | corrigé | 233c9d7e | « Tout développer » arrêté quand on replie le dossier (ou un parent) ou qu'on relance sur le même dossier ; le résultat n'ouvre plus un dossier replié entre-temps ; test |
| catégorie 1 | M96 | corrigé | 7d56f6b6 | miniature avec transparence mise en cache en PNG (`<clé>.png`, reconnu par le quota), opaque toujours en JPEG ; clé de cache en v2 ; test |
| catégorie 1 | M97 | corrigé | 007930d8 | page de taille invalide gardée à sa place, avec la taille de la page valide précédente (sinon suivante, sinon Letter) ; tests |
| catégorie 1 | F32 | corrigé | ce0f4004 | élément purgé pendant l'énumération (`$I` disparu) ignoré ; après un échec du flux, repli direct sur les propriétés Shell (`list_trash_fallback`) au lieu de relire la corbeille deux fois ; test |
| catégorie 1 | F33 | corrigé | 44e3588c | métadonnées de la corbeille lues sans suivre les liens (lecteur natif et repli Shell) ; une jonction reste un dossier ; test |
| catégorie 1 | F35 | corrigé | 1971dafc | pause confirmée tardivement : plus comptée dans le délai (Windows et Linux) |
| catégorie 1 | F36 | corrigé | e464eb90 | 10 s au lieu de 1 s pour la sortie du worker après Finished (Windows et Linux) |
| catégorie 1 | F37 | corrigé | 7361767c, 98d239df | emplacements d'admission empoisonnés repris (archives, liste, recherche Office, aperçus, vidéo ; Windows et Linux) ; test |
| catégorie 1 | F38 | corrigé | d11dae72 | clé d'icône sans casse ; .exe, .lnk, .ico… : taille et date d'écriture dans la clé (mémoire, disque, worker) ; tests ; les anciennes icônes en cache deviennent orphelines |
| catégorie 1 | F39 | corrigé | 16313ded | mesure des pages bornée à 3 s ; les suivantes listées `0 0` et prennent la taille de la dernière mesurée (M97) ; test |
| catégorie 1 | F40 | corrigé | d89c0e11 | bail du cache d'aperçus attendu jusqu'à 1 s (Windows et Linux) ; test Windows |
| catégorie 1 | F41 | corrigé | 63d1c8c2 | horloge « aucune image depuis 15 s » démarrée après le lancement du worker vidéo ; Linux déjà correct |
| catégorie 1 | F42 | corrigé | 628c9941 | repli PDF (chemin long ou non Unicode) admis par l'emplacement partagé |
| catégorie 1 | F43 | corrigé | e455bbe8 | libération de la session du Viewer gardée par un drapeau quand la file est pleine ; sans test (session réelle) |
| catégorie 1 | F44 | corrigé | daad4df9 | profils `Oxyde.Archive.<uuid>` de plus de 10 min supprimés une fois par processus avant la création d'un profil ; vérifié sur cette machine : 2 profils orphelins trouvés ; tests |
| release | RUSTSEC-2026-0548 | corrigé | 5d5e5680 | zip 9.0.4 → 9.7.3 ; `cargo audit` passe ; précondition d'un test de recherche adaptée (méthode inconnue refusée à la lecture, plus à l'ouverture) |
| release | toolchain Windows | corrigé | 53fb5c5d | distribution Windows alignée sur Rust 1.100.0 (1.98.1 restait de 0945330e) |
| release | Win10 LTSC | validé | | 2026-11-19, VM Win10 LTSC, portable 2.2.2 d'`audit-fixes` : pas d'erreur 4395 (création, copie, déplacement, renommage, suppression), workers isolés OK (miniatures, icône d'exe, 7z) ; mode portable validé (réglages à côté de l'exécutable, extraction d'archive, WM4) |
| release | WF6 (test manuel) | corrigé | 633a295d, e68d3382 | ISO montée + SUBST vers un de ses dossiers : nom, éjection, lecture validés ; Y: proposait la corbeille (GetVolumePathNameW échoue sur ce SUBST) et les paramètres de la corbeille listaient une clé USB exFAT : lettre SUBST lue sur le chemin, paramètres et occupation limités aux lecteurs qui ont une corbeille ; revérifié ; lecteur mappé hors ligne : message « introuvable » sans blocage ; WF6 validé le 2026-11-19 |
| release | lecture seule | ajouté | 0b79d399 | demande de l'utilisateur : couper, coller, nouveau, renommer, supprimer, compresser, extraire ici masqués dans les menus et la barre sur un volume en lecture seule (ISO, DVD, carte verrouillée) ou dans une archive non réinscriptible ; « Nouveau dossier » reste visible, désactivé ; raccourcis clavier ignorés aussi ; à tester |
