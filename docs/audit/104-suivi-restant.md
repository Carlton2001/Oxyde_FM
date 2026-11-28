# Audit — Remédiations et validations restantes

État au 28 novembre 2026. Les validations citées proviennent des comptes rendus du dépôt. Les lignes « à confirmer » correspondent à un scénario pour lequel le suivi ne consigne pas clairement le résultat.

## Corrections et mesures

| ID | État de la remédiation | Travail restant | Référence |
|---|---|---|---|
| **M17** | Corrigé (de83ab1d, 3c3193ea), validé à la main le 28 novembre | Copier depuis un ZIP aux fichiers chiffrés (mot de passe pas encore saisi) vers un dossier : le mot de passe doit être demandé (la recherche de conflits l’exigeait sans le demander), puis la copie se fait ; en cas de conflit, la boîte habituelle s’affiche et ses choix s’appliquent sans doublon « (2) ». Le cas partiel (suppression dans deux archives) a un test automatique. | [Constat](101-restes.md#L46) · [Code](../../src/systems/file_ops/retry.rs) |
| **M43** | Partiellement corrigé | Ajouter progression et annulation à la création d’un dossier dans une grosse archive. Vérifier que l’archive d’origine reste exploitable après annulation. | [Suivi](100-plan-remediation.md#L144) · [Code](../../src/systems/archive_workspace.rs#L457) |
| **M67** | Corrigé (e36fb857, ae156245, 5b1d17ad, 67aac9fa), testé à l’écran le 28 novembre ; essai de l’utilisateur à refaire | Copier 50 000 fichiers de 4 Kio (`C:\Users\Biluf\OxydeM67`), fenêtre des opérations ouverte : elle reste fluide, états, compteurs, tri par État et filtre suivent. Le premier essai de l’utilisateur a révélé que la copie elle-même ne pouvait pas se faire : chaque source tenait toute la chaîne de ses dossiers (12 handles par fichier, 600 000 pour 50 000 fichiers), et la recherche de conflits tenait les dossiers de chaque nom libre. Avant : 25 s avant l’apparition de l’opération, 272 s de sélection, 159 s de parcours, 1 061 s au total ; après : 2 s, 1,8 s, 2,2 s, 56 s au total (build de développement), 1 200 handles. Suppression définitive des 50 000 : 14 s. Double-clic sur une copie renommée dans son propre dossier (« rapport - Copie (2).txt ») : c’est elle qui est sélectionnée. Mesure du rendu (50 000 éléments, release) : 24 ms par rendu sur le thread UI avant, 1 µs après. | [Suivi](100-plan-remediation.md#L172) · [Code](../../src/systems/source_selection.rs) |
| **M31 / O85** | Corrigé (2ca699e9), mesuré le 28 novembre | 100 000 fichiers, Ctrl+A, défilement : 11,3 ms par image avant, 3,2 ms après, comme sans sélection. Chaque image comparait encore deux fois la sélection entière ; elle compare maintenant un tampon que prend toute modification. Seule l’image du Ctrl+A coûte 25 ms. | [O85](103-optimisations.md#L92) · [Code](../../src/ui/workspace/tab.rs#L192) |
| **M32 / O101** | Corrigé (e9373e4d), mesuré le 28 novembre | 300 000 fichiers. Clic sur une case : deux images de 50 ms avant (les comptes étaient refaits à chaque révision du filtre), 7 ms au plus après (comptes gardés tant que les fichiers et les filtres des autres colonnes ne changent pas). Restent sur le thread UI, une image chacun : ouverture du menu Type (70 ms), case « Tout » (50 ms, l’indicateur de l’en-tête relit tous les types). | [O101](103-optimisations.md#L106) · [Code](../../src/ui/components/file_list/filters.rs#L38) |
| **M57 / E6** | Atténué, test de charge ouvert | Déplacer un gros lot vers ou depuis une archive sous Linux et relever le pic de descripteurs. E6 a été validé sur une copie ordinaire. | [Suivi](100-plan-remediation.md#L195) · [Test E6](tests-linux.md#L104) |
| **M58 / E6** | Atténué, test de charge ouvert | Annuler via l’historique une copie de plusieurs dizaines de milliers de fichiers. Vérifier le résultat et le pic de descripteurs. | [Constat](99-synthese.md#L131) · [Suivi](100-plan-remediation.md#L195) |

## Windows — Validations ouvertes

Les trois premières lignes sont explicitement ouvertes dans le [compte rendu du 27 novembre](100-plan-remediation.md#L286).

| ID | État de la remédiation | Scénario restant | Référence |
|---|---|---|---|
| **M13** | Corrigé, test manuel ouvert | Désépingler un favori pendant sa réorganisation. Vérifier que le glisser se termine et que les interactions reprennent. | [Suivi](100-plan-remediation.md#L149) |
| **M26** | Corrigé, test manuel ouvert | Refuser la fermeture après le lancement de l’installeur. Oxyde doit reproposer la mise à jour. | [Suivi](100-plan-remediation.md#L154) |
| **M77** | Corrigé, test manuel ouvert | Inverser les boutons de la souris et glisser d’Oxyde vers l’Explorateur avec le bouton principal configuré. | [Constat](99-synthese.md#L154) |
| **F25 / dépôt en flux** | ZIP de l’Explorateur validé, variante Outlook ouverte | Déposer une pièce jointe Outlook dans Oxyde et vérifier le contenu de la copie. | [Suivi](100-plan-remediation.md#L282) |
| **F53** | Corrigé, test matériel ouvert | Monter un volume dans un dossier et vérifier que les informations de stockage correspondent au volume monté. | [Suivi](102-faibles.md#L21) |
| **F163** | Corrigé, test manuel ouvert | Glisser entre deux volumes dont l’un est monté dans un dossier sous la même lettre. L’action par défaut doit être Copier. | [Suivi](102-faibles.md#L113) |
| **E2 / WM7** | Partiellement validé | Vérifier la suppression sur partage réseau et avec la stratégie NoRecycleFiles. Le 28 novembre, ni la clé de stratégie HKCU ni le partage `so39i Agde` n’étaient accessibles en écriture sans élévation : à faire par l’utilisateur (`reg add HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer /v NoRecycleFiles /t REG_DWORD /d 1` en administrateur, puis relancer Oxyde). | [Suivi](100-plan-remediation.md#L107) |
| **F109** | Corrigé, test passé sous Windows le 28 novembre ; test manuel ouvert | Annuler une mise à la corbeille mêlant des éléments avec et sans identité. Vérifier la restauration des premiers et le signalement des autres. | [Suivi](102-faibles.md#L69) |
| **E10** | IME japonais validé, variantes ouvertes | Tester la composition avec les IME chinois et coréen sous Windows. | [Suivi](100-plan-remediation.md#L147) |
| **WF2** | Corrigé, repli spécifique sans résultat consigné | Vérifier le parcours par composants sur une ancienne build sans OBJ_DONT_REPARSE. | [Suivi](100-plan-remediation.md#L117) |

## Linux — Neuf tests de copie directe

Ces tests ont pour référence leur ligne dans la checklist.

| Scénario | État de la remédiation | Résultat à vérifier | Référence |
|---|---|---|---|
| Copie vers ext4 et FAT/exFAT | Implémenté, test ouvert | Nom final visible dès le début, contenu identique et métadonnées prises en charge conservées. | [Checklist](tests-linux.md#L225) |
| Annulation d’une nouvelle copie | Implémenté, test ouvert | Le fichier partiel disparaît. | [Checklist](tests-linux.md#L226) |
| Annulation d’un remplacement | Implémenté, test ouvert | L’ancien contenu est intact et le fichier temporaire est nettoyé. | [Checklist](tests-linux.md#L227) |
| Copie sur NTFS via ntfs-3g | Implémenté, test ouvert | Copie, remplacement et annulation sans résidu .fuse_hidden. | [Checklist](tests-linux.md#L228) |
| Copie sur SMB via GVfs | Implémenté, test ouvert | Création d’un fichier et nettoyage du partiel à l’annulation. | [Checklist](tests-linux.md#L229) |
| Dossiers 0333 et 1733 | Test de comportement ouvert | Vérifier la création dans un dossier tiers sans droit de lecture. Si l’annulation ne peut pas supprimer le partiel, vérifier qu’il est vidé et signalé. | [Checklist](tests-linux.md#L230) |
| Arrêt forcé et relance | Implémenté, test ouvert | Sur ext4, suppression du partiel et notification. Sur ntfs-3g sans date de création exploitable, conservation du partiel et trace dans le journal. | [Checklist](tests-linux.md#L231) |
| Compression ZIP et TAR.ZST | Implémenté, test ouvert | Nom final visible dès le début, suffixe en cas de conflit et nettoyage à l’annulation. | [Checklist](tests-linux.md#L232) |
| Déplacement USB puis Annuler | Implémenté, test ouvert | Retour du fichier à son emplacement d’origine sans dossier .oxyde-history- résiduel. | [Checklist](tests-linux.md#L233) |

## Linux — Intégrations et variantes de test

| ID | État de la remédiation | Scénario restant | Référence |
|---|---|---|---|
| **LM2 / LF1** | Tests du bac à sable passés en conteneur | Ouvrir des aperçus d’archives et de vidéos dans une vraie Toolbox ou Distrobox, puis sous NixOS. | [Checklist](tests-linux.md#L112) |
| **LF2** | Détection partiellement vérifiée | Contrôler le message de mise à jour dans Oxyde avec un paquet .deb et avec le paquet AUR. | [Checklist](tests-linux.md#L114) |
| **LF4** | Corrigé, environnement nécessaire | Avec un dossier personnel NFS ou équivalent sans bail, ouvrir un fichier d’archive et vérifier le repli vers le répertoire temporaire privé. | [Checklist](tests-linux.md#L117) |
| **F45** | Validé en session imbriquée, bureaux réels à tester | Copier depuis Nautilus sous GNOME Wayland, puis copier et couper depuis Dolphin sous KDE Wayland. | [Résultat](tests-linux.md#L170) |
| **F55** | Report du redémarrage observé, cycle complet à tester | Modifier le réglage pendant un vrai partage d’écran. Vérifier que le partage continue et que le portail redémarre ensuite. | [Résultat](tests-linux.md#L174) |
| **M22** | Filtre testé via le portail, Firefox à tester | Depuis Firefox, ouvrir le sélecteur avec le filtre Images dans un gros dossier. Vérifier le premier affichage et Retour/Avant. | [Résultat](tests-linux.md#L119) |
| **F56** | Appels directs validés, Firefox à tester | Ouvrir puis fermer plusieurs fois de suite le sélecteur depuis Firefox. | [Résultat](tests-linux.md#L175) |
| **E10** | IBus validé, Fcitx ouvert sans échéance | Vérifier la composition Pinyin avec Fcitx 5 sous X11. | [Checklist](tests-linux.md#L240) |
| **M100** | Copie validée, déplacement à tester | Déplacer un dossier contenant un FIFO et vérifier l’absence de blocage et de sortie partielle. | [Résultat](tests-linux.md#L53) |
| **M40** | ISO ordinaire et UDF vérifiées | Essayer une ISO d’installation de Windows 11 et vérifier qu’elle passe par le montage système. | [Résultat](tests-linux.md#L71) |
| **M64** | Arbre local validé, montages réseau à tester | Vérifier que « Tout développer » laisse les montages GVfs/NFS fermés et que leur ouverture individuelle fonctionne. | [Résultat](tests-linux.md#L118) |
| **M99 / M54 / M61** | Corbeille USB et Annuler validés en partie | Vérifier la visibilité de l’entrée dans Nautilus et son retour en corbeille via Rétablir. | [Résultat](tests-linux.md#L111) |

## Interface et opérations — Validations à confirmer

| ID | État de la remédiation | Scénario à confirmer | Référence |
|---|---|---|---|
| **M9** | Corrigé, validation à confirmer | Éjecter un support après la disparition d’un dossier surveillé. Sous Windows, une ISO est en lecture seule : il faut une clé USB ou un VHD (élévation), ou le faire sous Linux comme dans le scénario d’origine. | [Constat](99-synthese.md#L76) |
| **M23** | Corrigé, validation Linux à confirmer | Changer d’adresse pendant une invite SMB et vérifier que l’invite disparaît. | [Suivi](100-plan-remediation.md#L112) |
| **M24** | Corrigé, validation Linux à confirmer | Fermer la fenêtre des propriétés pendant l’application récursive des droits et vérifier qu’aucune interruption partielle ne passe sous silence. | [Constat](99-synthese.md#L91) |
| **F202** | Corrigé, validation Linux à confirmer | Modifier les droits après ouverture des propriétés. Vérifier la relecture des droits et le comportement au deuxième OK. | [Suivi](102-faibles.md#L143) |

## Validations déjà consignées et limites actées

- **Windows, à l’écran**. F67, F93, M70 et O89, notification de récupération NTFS/exFAT, et Annuler/Rétablir sur USB. [Compte rendu](100-plan-remediation.md#L286)
- **Windows, côté moteur**. M66 et M79, notamment. [Compte rendu](100-plan-remediation.md#L285)
- **Windows, à l’écran, 28 novembre**. Oxyde piloté par clavier, souris et captures :
  - M2 (F2 renomme l’élément du curseur), M3 (F2 et Ctrl+Maj+N sans effet dans la corbeille), M5 (croix, Maj+clic puis saisie : « xy », sans panique), M6 (Ctrl+C et Ctrl+X refusés dans le champ masqué, Couper et Copier grisés dans son menu), M7 (menu « Exécuter » de repli, sans panique).
  - M12 : un glisser depuis la corbeille déposé dans l’Explorateur sortait l’élément de la corbeille. Corrigé (4b9b722c) : rien n’est offert hors d’Oxyde, vérifié.
  - M16 (deux invites de mot de passe à la suite, deux archives extraites), M18 (nouvelle fenêtre, racine de l’autre volet, résultats dans ce volet), M19 (Ctrl+X A, Ctrl+C B, Ctrl+V : B copié ; en local, pas sur un partage lent), M25 (Tab saute la capacité masquée, Échap ferme).
  - F171 (l’erreur reste après un tri) et F172 (Entrée sans sélection ne fait rien, avec sélection répond le chemin), par le lanceur `--choose`.
  - M10 : 20 fichiers créés pendant le démontage d’une ISO depuis Oxyde, tous affichés ensuite.
  - M11 : la disparition passagère d’un lecteur (SUBST retiré environ une seconde, pendant les relevés déclenchés par le montage d’une ISO) redirigeait l’onglet : une notification amène deux relevés en 150 ms. Corrigé (38156904) : un lecteur doit manquer deux secondes. Vérifié : onglet et historique gardés ; un vrai retrait est pris en compte vers 3 s.
  - M85 : un .ico 32 bits à alpha nul s’affichait vide dans la grille et les aperçus (décodeur `image`, pas le chemin HICON corrigé en 8190cc52). Corrigé (354625e0) : le masque AND décide de la transparence. Vérifié avec une icône sans masque et une icône à quart transparent.
  - F68 : serveur muet simulé (port 445 qui accepte sans répondre, dans WSL) : l’onglet abandonne à 30 s et dit que le serveur n’a pas répondu ; l’énumération de `\\PCS` liste ses partages.
- **Windows, tests automatiques, 28 novembre**. M92 : un écouteur qui refuse l’écrasement dans OnOverwrite fait redemander Oxyde, et le fichier existant est conservé (2c958470).
- **Limites actées**. Les décisions sur M14, F77 et WM1 figurent dans la liste des choix volontaires. [Décisions](101-restes.md#L87)
