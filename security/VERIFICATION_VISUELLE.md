# Vérification visuelle de sécurité — Windows

Préparation du 9 octobre 2026, actualisée jusqu’au lot 56. **La vérification visuelle n’a pas été effectuée.** Ce protocole est à réaliser par l’utilisateur, sans Computer Use. Il vérifie les changements déjà implémentés ; il ne clôture pas les autres risques de l’audit.

**Revue à effectuer par l’utilisateur :** les corrections de cette passe sont codées. Le binaire final contient les lots 53–56 : chemins Shell littéraux, workers d’aperçu, caches/icônes et vidéo isolés. Ce protocole vérifie leur usage, sans valider la course résiduelle Shell, les autres fournisseurs ou les parseurs d’archives.

Exécutable final du lot 56 : `C:\Users\Biluf\Documents\Projects\Oxyde-GPUI\target\debug\oxyde.exe`. Fermer les anciennes instances avant de l’ouvrir pour vérifier la bonne version. Les modifications de langue et de thème suivent les préférences habituelles d’Oxyde ; remettre ensuite celles souhaitées.

Jeu de fichiers préparé :

```text
C:\Users\Biluf\Documents\Projects\Oxyde-GPUI\target\security-visual-check-0_qkhpsg
```

Il contient 20 petits fichiers, 169 036 octets au total. Le PNG au-delà du seuil ne fait que 32 769 × 1 pixels ; le GIF limité contient 257 images de 16 × 16 pixels ; le ZIP profond contient un seul octet. Aucun document personnel, exécutable ou volume de décompression massif n’est utilisé.

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
| `04-animation.gif` | Petit carré animé qui change de couleur. |
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

Après Rétablir, dans l’Explorateur Windows, renommer le témoin présent dans `destination` en `original-conservé.txt`. Copier ce dernier puis renommer **la copie** en `témoin.txt`. Les deux fichiers contiennent ainsi les mêmes octets mais sont des objets distincts. Dans Oxyde, Annuler doit refuser le déplacement, expliquer que le fichier a été remplacé et conserver les deux fichiers. Fermer le message doit laisser la navigation disponible. Ce scénario ne touche que les témoins générés ; ne pas le réaliser sur un document personnel.

## 4. Cache indisponible et sélecteur natif — environ trois minutes

Le script a créé un dossier TEMP privé comportant une obstruction inoffensive au chemin du cache d’ouverture des archives. Ce cas produit **Cache indisponible**, et non une simulation de cache plein ou occupé. Le bouton de repli et le sélecteur sont communs à ces trois causes.

Fermer l’instance de test, puis lancer cette session isolée :

```powershell
$taskPython = 'C:/Users/Biluf/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
$fixtureRoot = 'C:/Users/Biluf/Documents/Projects/Oxyde-GPUI/target/security-visual-check-0_qkhpsg'
& $taskPython scripts/prepare-security-visual-check.py --launch-cache-unavailable $fixtureRoot
```

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
