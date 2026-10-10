# Vérification visuelle de sécurité — Windows

Préparation du 9 octobre 2026. **Premier retour visuel reçu : archives, corbeille/historique et lisibilité sans anomalie signalée ; aperçus en échec et deux parcours à reprendre après correction.** Ce protocole est à réaliser par l’utilisateur, sans Computer Use. Il vérifie les changements déjà implémentés ; il ne clôture pas les autres risques de l’audit.

**Revue ciblée à reprendre après correction :** GIF, AVI, MOV x265, remplacement après déplacement et cache indisponible. Le retour utilisateur ne valide pas les codecs non testés. Ce protocole vérifie leur usage, sans valider la course résiduelle Shell, les autres fournisseurs ou les parseurs d’archives.

Exécutable reconstruit après les corrections du retour visuel (lot 57) : `C:\Users\Biluf\Documents\Projects\Oxyde-GPUI\target\debug\oxyde.exe`. Fermer les anciennes instances avant de l’ouvrir pour vérifier la bonne version. Les modifications de langue et de thème suivent les préférences habituelles d’Oxyde ; remettre ensuite celles souhaitées.

Jeu de fichiers préparé :

```text
C:\Users\Biluf\Documents\Projects\Oxyde-GPUI\target\security-visual-check-xusvjigg
```

Ce nouveau jeu contient 20 petits fichiers, 169 036 octets au total. Il conserve les anciens jeux de test ; ses GIF alternent maintenant rouge/bleu de façon nettement visible. Le PNG au-delà du seuil ne fait que 32 769 × 1 pixels ; le GIF limité contient 257 images de 16 × 16 pixels ; le ZIP profond contient un seul octet. Aucun document personnel, exécutable ou volume de décompression massif n’est utilisé.

Pour recréer une session de test indépendante, depuis le dépôt :

```powershell
$taskPython = 'C:/Users/Biluf/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
& $taskPython scripts/prepare-security-visual-check.py
```

Le script affiche un nouveau dossier. Il ne réutilise ni n’écrase les fichiers de la session précédente. Pour le protocole ci-dessous, `$fixtureRoot` désigne ce dossier, initialement celui indiqué plus haut.

## 1. Aperçus — environ trois minutes

Dans Oxyde, ouvrir le sous-dossier `aperçus` via la barre d’adresse. Activer les aperçus dans les préférences si nécessaire, puis survoler les fichiers.

| Fichier | Résultat attendu |
| --- | --- |
| `01-normal.png` | Image verte semi-transparente, petit format et proportions préservés. |
| `02-limite-dimensions.png` | « Cette image dépasse les limites de l’aperçu. Le fichier reste accessible. » Aucun chargement infini. |
| `03-corrompu.png` | Aperçu indisponible, sans spinner permanent ; une image normale reste consultable ensuite. |
| `04-animation.gif` | Carré animé alternant franchement rouge et bleu toutes les 250 ms dans les nouveaux jeux de test. La bulle agrandit le carré ; ce n’est pas sa taille native. |
| `05-animation-limitee.gif` | Première image fixe, avec « Aperçu fixe : l’animation dépasse les limites de l’aperçu. » |
| `06-proportions.svg` | Rectangle haut vert/bleu, rapport largeur/hauteur 1:3. |
| `07-page.pdf` | Page rouge verticale, proportions 1:2, également à 125 %/150 % de mise à l’échelle Windows. |
| `08-office.odt` | Miniature rouge (pas bleue), avec le fond blanc éventuel du fournisseur Windows ; le repli texte éventuel affiche « École : aperçu Office. » |
| `09-video.avi` | Carré rouge puis bleu animé. Fermer le survol arrête la lecture ; un PNG/PDF reste consultable pendant la vidéo. |

Passer à un autre fichier, fermer le survol, puis revenir. L’application reste navigable et les autres aperçus actifs restent utilisables. Fermer aussi l’application après plusieurs aperçus PDF/vidéo : aucune attente de fermeture infinie. Si un vrai document dépasse le délai, le message indique les 15 secondes et que le fichier reste accessible ; ne pas chercher à produire une charge énorme pour ce test. Les délais et l’arrêt d’un worker bloqué ont des tests automatisés. Ce contrôle ne mesure pas le RSS, le GPU ou l’éviction interne de GPUI.

## 2. Archives — environ deux minutes

Ouvrir `archives/01-normal.zip` : navigation, noms Unicode et lecture des fichiers habituels fonctionnent. Ouvrir `02-limite-profondeur.zip` : le refus doit expliquer la limite de **64 niveaux, nom du fichier compris**, sans écran vide ambigu ni boucle de chargement. Ce test couvre le message de profondeur ; les autres seuils sont couverts par les tests automatisés et ne sont pas déclenchés ici par une décompression géante.

## 3. Annuler/Rétablir un déplacement — environ trois minutes

Déplacer `déplacements/source/témoin.txt` vers `déplacements/destination` avec les actions habituelles d’Oxyde. Utiliser Annuler puis Rétablir, deux fois. Le fichier retrouve la bonne place et son contenu reste lisible.

Pour tester le refus après remplacement, utiliser exactement ces noms (aucun fichier `original.txt` n’est nécessaire) :

1. Après le dernier Rétablir dans Oxyde, vérifier que `déplacements/destination/témoin.txt` existe et que `déplacements/source/témoin.txt` est absent.
2. Dans **l’Explorateur Windows**, ouvrir `déplacements/destination` et renommer `témoin.txt` en `original-conservé.txt`. Saisir seulement le nom du fichier, sans chemin.
3. Toujours dans **l’Explorateur Windows**, copier puis coller `original-conservé.txt` dans ce même dossier. Renommer le nouveau fichier copié en `témoin.txt`. On doit obtenir deux fichiers : `original-conservé.txt` et `témoin.txt`.
4. Revenir dans Oxyde et cliquer une seule fois sur Annuler. Oxyde doit refuser le déplacement et expliquer le remplacement. Les deux fichiers doivent rester dans `destination`, et `source/témoin.txt` doit rester absent.

Effectuer les étapes 2 et 3 hors d’Oxyde pour ne pas ajouter une copie ou un renommage à son historique : sinon Annuler ne vise plus le déplacement testé. Fermer le message doit laisser la navigation disponible. Ne réaliser ce scénario que sur les témoins générés.

## 4. Cache indisponible et sélecteur natif — environ trois minutes

Le script a créé un dossier TEMP privé comportant une obstruction inoffensive au chemin du cache d’ouverture des archives. Ce cas produit **Cache indisponible**, et non une simulation de cache plein ou occupé. Le bouton de repli et le sélecteur sont communs à ces trois causes.

Fermer l’instance de test, puis lancer cette session isolée :

```powershell
$taskPython = 'C:/Users/Biluf/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
$fixtureRoot = 'C:/Users/Biluf/Documents/Projects/Oxyde-GPUI/target/security-visual-check-xusvjigg'
& $taskPython 'C:/Users/Biluf/Documents/Projects/Oxyde-GPUI/scripts/prepare-security-visual-check.py' --launch-cache-unavailable $fixtureRoot
```

**Copier les trois lignes du bloc entier.** Le `&` au début de la dernière ligne est l’opérateur d’appel PowerShell : il est obligatoire pour exécuter le chemin stocké dans `$taskPython`. Sans lui, PowerShell produit « Jeton inattendu » avant de lancer le script. Ne pas ajouter de `\` de continuation à la fin des lignes.

Seul le processus enfant reçoit ce TEMP privé. Le TEMP du système, les autres applications et le cache utilisateur existant restent hors de ce test. Oxyde conserve ses préférences habituelles.

Ouvrir `archives/01-normal.zip`, puis `lecture.txt`. Vérifier que la notification explique l’indisponibilité et propose **« Extraire et ouvrir… »**, sans demander de nettoyer `%TEMP%`. Cliquer sur le bouton : le sélecteur Windows doit autoriser un seul dossier. Annuler une première fois : aucun fichier permanent ne doit être ajouté. Recommencer et choisir `export-permanent` dans la fixture : `lecture.txt`, qui contient « TÉMOIN À CONSERVER », reste intact ; le fichier extrait devient `lecture (1).txt`, avec une notification indiquant son emplacement. L’ouverture externe peut dépendre de l’application Windows associée aux fichiers texte ; un échec d’association doit rester expliqué.

Fermer cette instance après le test. Relancer ensuite Oxyde normalement pour retrouver son environnement TEMP habituel. Aucune suppression manuelle dans `%TEMP%` n’est nécessaire.

## 4 bis. Corbeille et historique — cinq minutes

Cette étape utilise uniquement le nouveau sous-dossier `corbeille` des fixtures. Les actions sont réalisées par l’utilisateur dans Oxyde ; aucun résultat n’est considéré obtenu à ce stade. Si la corbeille du volume est désactivée dans Windows, signaler cette configuration : une suppression permanente n’est pas une restauration réussie.

1. Supprimer `Annuler école.txt` vers la corbeille, puis Annuler, Rétablir et Annuler. Le nom Unicode et le texte « ÉLÉMENT ORIGINAL À RESTAURER » doivent rester identiques. Les onglets de la corbeille et du dossier doivent refléter les effets sans élément fantôme. Répéter la suppression/restauration directe depuis la corbeille avec `Dossier école` et `Lien école.url` ; le contenu du dossier et l’extension URL doivent être conservés. Il suffit de vérifier le nom de l’URL, sans l’ouvrir.
2. Supprimer `Conflit école.txt`. Depuis PowerShell, créer le témoin indépendant au même emplacement, sans enregistrer une nouvelle action Oxyde :

```powershell
Copy-Item -LiteralPath (Join-Path $fixtureRoot 'corbeille\témoin-remplacement.txt') -Destination (Join-Path $fixtureRoot 'corbeille\Conflit école.txt')
```

Annuler dans Oxyde : le message doit expliquer qu’une destination existe déjà. Le texte « REMPLACEMENT À CONSERVER » reste intact et l’élément original reste dans la corbeille. La restauration directe depuis la corbeille doit aussi refuser ce conflit sans écrasement.
3. Supprimer `Rétablir école.txt`, puis Annuler. Depuis PowerShell, remplacer son entrée en conservant l’original ailleurs dans la fixture :

```powershell
Move-Item -LiteralPath (Join-Path $fixtureRoot 'corbeille\Rétablir école.txt') -Destination (Join-Path $fixtureRoot 'corbeille\original-conservé.txt')
Copy-Item -LiteralPath (Join-Path $fixtureRoot 'corbeille\témoin-remplacement.txt') -Destination (Join-Path $fixtureRoot 'corbeille\Rétablir école.txt')
```

Rétablir dans Oxyde : le message doit expliquer que l’élément ne correspond plus à l’objet enregistré. Le remplacement et `original-conservé.txt` doivent rester présents et lisibles. Utiliser une fixture neuve pour répéter cette étape ; le script en crée une sans écraser la précédente.

Ce contrôle couvre l’intégration normale et les refus visibles. Il ne reproduit pas une course de remplacement pendant l’appel Shell, ne mesure pas les quotas de Windows et ne teste pas les autres documents de la vraie corbeille.

## 5. Lisibilité et retour attendu

Réaliser au minimum les messages en français dans le thème habituel, puis vérifier le message d’aperçu limité et celui du déplacement remplacé en anglais et dans l’autre thème. Pour couvrir les cinq langues visuellement, répéter ces deux messages en allemand, espagnol et italien. Les cinq catalogues ont déjà été vérifiés automatiquement ; leur présence ne garantit pas leur disposition à l’écran.

Les textes ne doivent pas être coupés, superposés, remplacés par leur clé de traduction ou cachés derrière la notification. Le bouton de repli doit rester visible dans une fenêtre réduite. Le sélecteur doit rester utilisable après annulation et le changement de langue ne doit pas réinterpréter les noms de fichier.

Retour à donner dans le chat : étape réussie ou échouée, langue, thème, et ce qui est visible. Une capture est utile si un texte est tronqué, mais n’est pas indispensable. Aucun résultat visuel ne sera marqué réussi sans ce retour.

## Contrôles préparatoires réalisés

`cargo build --bin oxyde --locked --offline` réussi, sans avertissement du projet. Suite finale du lot 43 : **655 réussites**, aucun échec, 17 tests préexistants ignorés. Fixtures PNG/GIF vérifiées avec Pillow : tailles attendues et 12/257 images lisibles ; profondeur ZIP : 65 composants. Un test ponctuel, retiré ensuite, appelle le chargeur de production avec les six aperçus : image normale, refus de dimension, corruption, animation normale, repli fixe et proportions SVG vérifiés sans fenêtre. Le test ponctuel du cache/export vérifie séparément les wrappers de production sans lancer l’application associée ; son résultat est consigné dans l’audit.

Les scripts n’ont pas lancé l’UI pendant la préparation. L’affichage réel, les clics, le sélecteur Windows et l’application externe restent à vérifier. Lots 45 à 52 : preuves Move/Rename/Delete, sidecars, préparation des sources, compression, restauration et propriété des temporaires ajoutées. Dernière suite : 719 réussites, aucune erreur, 17 tests préexistants ignorés. Les appels Shell par chemin, les fournisseurs non testés et l’isolation des parseurs restent ouverts dans `AUDIT_SECURITE.md`. Aucune conclusion générale de sécurité.

Lots 53–56 : tests du vrai worker et protocole sans interface, PDF à 125 %, Office, icône/cache, AVI MJPEG/RGB32, annulation, délai et paquets invalides vérifiés automatiquement. Les 20 fixtures ont été régénérées sans modifier les sessions précédentes. Aucune validation visuelle n’a été faite par l’agent.

Borne finale du lot 56 : **738 tests permanents réussis**, aucun échec, 17 tests existants ignorés non activés ; compilation sans avertissement. Les neuf aperçus générés ont aussi été contrôlés ponctuellement via les workers réels, sans fenêtre. Le contrôle interactif de ce document reste entièrement à faire.


## Retour utilisateur et reprise ciblée — lot 57

Premier passage : archives, corbeille/historique et lisibilité sans anomalie signalée. GIF ordinaire fixe, AVI sans infobulle et MOV x265 bloqué signalés ; déplacement ambigu ; cache non lancé à cause de la commande PowerShell sans `&`. Les trois corrections d’aperçu sont incluses dans le nouveau binaire.

Ne pas refaire toute la procédure : reprendre seulement les GIF/AVI du nouveau jeu, le même MOV x265, puis les sections 3 et 4 ci-dessus. Le GIF limité doit rester fixe avec son avertissement. Son grand carré provient de l’agrandissement dans la bulle. L’AVI s’active désormais en aperçu, avec les codecs disponibles sur Windows.

Validation avant cette reprise : 739 tests permanents réussis, aucun échec ; compilation et build sans avertissement du projet. Le worker vidéo reçoit 1 Gio au lieu de 512 Mio, le worker fixe reste à 512 Mio. Le MOV utilisateur a donné 24 frames via le vrai worker corrigé, première frame en 0,40 seconde. Ces contrôles sans fenêtre ne remplacent pas la confirmation visuelle des corrections. Les autres chantiers techniques ouverts ne sont pas clôturés par cette reprise.


## Deuxième retour utilisateur et reprise ciblée — lot 58

Annuler/Rétablir après remplacement et cache indisponible/sélecteur : sans anomalie de fonctionnement signalée. Le chemin natif étendu affiché dans le refus et la présentation des notifications sont corrigés. La vidéo était toujours lente et l’AVI semblait fixe ; de nouvelles corrections de timing, boucle, redimensionnement natif et transport sont incluses. Le sélecteur Windows est conservé ; créer un sélecteur Oxyde constitue un chantier distinct reporté à la demande de l’utilisateur.

Contrôles de la reprise ciblée du lot 58, validée par le retour utilisateur ci-dessous :

1. **Vidéo :** dans le même jeu `security-visual-check-xusvjigg`, `09-video.avi` doit alterner rouge/bleu environ chaque seconde, aussi aux reprises de boucle. Le même MOV x265 doit jouer à la vitesse normale. Fermer le survol interrompt toujours la lecture ; PNG/PDF restent consultables pendant une vidéo.
2. **Fenêtre :** déplacer et redimensionner, fermer immédiatement puis relancer. Position et dimensions doivent être retrouvées ; essayer aussi après maximisation, puis restauration. Sur plusieurs écrans, les coordonnées négatives sont permises ; si un écran disparaît, la fenêtre doit rester accessible.
3. **Transferts :** utiliser seulement les témoins générés. Déplacer un fichier dans son dossier actuel doit être ignoré sans dialogue ni nouvelle action à annuler. Copier/coller dans ce dossier doit créer `témoin - Copie.txt`, puis `témoin - Copie (2).txt` en français ; Annuler/Rétablir vise la copie réellement créée. L’original et les copies précédentes restent intacts. Le suffixe suit la langue active. Une copie de dossier utilise aussi un nom distinct.
4. **Notifications :** reprendre le refus après remplacement déjà vérifié, et éventuellement le cache indisponible seulement pour contrôler sa présentation. Le chemin doit apparaître comme `C:\Users\...`, ou `\\serveur\partage\...` pour UNC, sans préfixe `\\?\`. La croix reste en haut à droite ; les messages longs peuvent atteindre 40 rem si la fenêtre le permet. « Extraire et ouvrir… » doit être un bouton rempli lisible. Aucune modification de curseur.

La reprise ciblée est terminée selon le retour utilisateur du 10 octobre 2026 : « tout fonctionne ». Ce retour global porte sur les aperçus AVI/MOV, la géométrie de la fenêtre, les transferts sur soi et la présentation des notifications après le commit `1b4df95`. Aucun détail supplémentaire par scénario ou configuration n’a été fourni. Les autres risques techniques de l’audit ne sont pas clôturés.

Validation automatisée du lot 58 : **751 tests permanents réussis**, aucun échec ; 17 tests préexistants ignorés non activés. Le diagnostic ponctuel du MOV reste distinct de cette suite.

Compilation et reconstruction sans avertissement du projet ; utiliser `target/debug/oxyde.exe` pour cette reprise.

## Reprise après le retour par élément du Shell — lots 59–60

La validation précédente du lot 58 reste acquise pour ses parcours. L’implémentation Windows des nouvelles suppressions suivies et de Rétablir Delete passe maintenant par IFileOperation ; les callbacks et reprises sont contrôlés automatiquement sans vraie suppression. Après les derniers changements de code, contrôler uniquement avec un nouveau fichier et un nouveau dossier jetables dans le jeu généré : suppression vers la corbeille, Annuler, puis Rétablir ; observer la mise à jour de la liste et la possibilité de restaurer. Ne pas vider la corbeille ni utiliser de documents personnels. Vérifier aussi qu’un dossier compagnon non sélectionné reste en place. Les erreurs tardives injectées restent des tests automatisés ; ne pas provoquer une erreur en remplaçant des fichiers personnels. Cette reprise native n’a pas encore été exécutée par l’agent ou l’utilisateur.


## Reprise ciblée de navigation — lot 61

Après la fin des modifications, utiliser uniquement les archives du jeu généré : ouvrir une ZIP et son sous-dossier, puis une 7z, une TAR compressée et l'ISO. Vérifier liste/grille, noms et navigation de retour, avec les aperçus habituels disponibles en parallèle. Les erreurs de worker/délai et de taille des métadonnées restent couvertes automatiquement ; il n'est pas nécessaire de créer une archive énorme pour cette revue. La recherche, l'extraction et les modifications d'archive conservent leurs parcours précédents. Cette reprise visuelle n'a pas été exécutée.


## Reprise ciblée de recherche — lot 62

Après la fin des modifications, lancer une recherche avancée avec recherche dans les archives sur le jeu généré, vérifier un résultat d'un sous-dossier et son emplacement, puis arrêter et relancer la recherche. Vérifier qu'une archive corrompue ou limitée produit un avertissement et laisse apparaître les autres résultats. Les annulations de parseur bloqué restent des tests automatisés ; aucun gros fichier ni document personnel nécessaire. La recherche rapide et les filtres habituels ne changent pas. Revue visuelle non exécutée.
