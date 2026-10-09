# Vérification visuelle de sécurité — Windows

Préparation du 9 octobre 2026, après les lots 42 et 43. **La vérification visuelle n’a pas été effectuée.** Ce protocole est à réaliser par l’utilisateur, sans Computer Use. Il vérifie les changements déjà implémentés ; il ne clôture pas les autres risques de l’audit.

Exécutable construit : `C:\Users\Biluf\Documents\Projects\Oxyde-GPUI\target\debug\oxyde.exe`. Fermer les anciennes instances avant de l’ouvrir pour vérifier la bonne version. Les modifications de langue et de thème suivent les préférences habituelles d’Oxyde ; remettre ensuite celles souhaitées.

Jeu de fichiers préparé :

```text
C:\Users\Biluf\Documents\Projects\Oxyde-GPUI\target\security-visual-check-7197tp8o
```

Il contient onze petits fichiers, environ 161 Kio au total. Le PNG au-delà du seuil ne fait que 32 769 × 1 pixels ; le GIF limité contient 257 images de 16 × 16 pixels ; le ZIP profond contient un seul octet. Aucun document personnel, exécutable ou volume de décompression massif n’est utilisé.

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

Passer à un autre fichier, fermer le survol, puis revenir. L’application reste navigable et les autres aperçus actifs restent utilisables. Ce contrôle ne mesure pas le RSS, le GPU ou l’éviction interne de GPUI.

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
$fixtureRoot = 'C:/Users/Biluf/Documents/Projects/Oxyde-GPUI/target/security-visual-check-7197tp8o'
& $taskPython scripts/prepare-security-visual-check.py --launch-cache-unavailable $fixtureRoot
```

Seul le processus enfant reçoit ce TEMP privé. Le TEMP du système, les autres applications et le cache utilisateur existant restent hors de ce test. Oxyde conserve ses préférences habituelles.

Ouvrir `archives/01-normal.zip`, puis `lecture.txt`. Vérifier que la notification explique l’indisponibilité et propose **« Extraire et ouvrir… »**, sans demander de nettoyer `%TEMP%`. Cliquer sur le bouton : le sélecteur Windows doit autoriser un seul dossier. Annuler une première fois : aucun fichier permanent ne doit être ajouté. Recommencer et choisir `export-permanent` dans la fixture : `lecture.txt`, qui contient « TÉMOIN À CONSERVER », reste intact ; le fichier extrait devient `lecture (1).txt`, avec une notification indiquant son emplacement. L’ouverture externe peut dépendre de l’application Windows associée aux fichiers texte ; un échec d’association doit rester expliqué.

Fermer cette instance après le test. Relancer ensuite Oxyde normalement pour retrouver son environnement TEMP habituel. Aucune suppression manuelle dans `%TEMP%` n’est nécessaire.

## 5. Lisibilité et retour attendu

Réaliser au minimum les messages en français dans le thème habituel, puis vérifier le message d’aperçu limité et celui du déplacement remplacé en anglais et dans l’autre thème. Pour couvrir les cinq langues visuellement, répéter ces deux messages en allemand, espagnol et italien. Les cinq catalogues ont déjà été vérifiés automatiquement ; leur présence ne garantit pas leur disposition à l’écran.

Les textes ne doivent pas être coupés, superposés, remplacés par leur clé de traduction ou cachés derrière la notification. Le bouton de repli doit rester visible dans une fenêtre réduite. Le sélecteur doit rester utilisable après annulation et le changement de langue ne doit pas réinterpréter les noms de fichier.

Retour à donner dans le chat : étape réussie ou échouée, langue, thème, et ce qui est visible. Une capture est utile si un texte est tronqué, mais n’est pas indispensable. Aucun résultat visuel ne sera marqué réussi sans ce retour.

## Contrôles préparatoires réalisés

`cargo build --bin oxyde --locked --offline` réussi, sans avertissement du projet. Suite finale du lot 43 : **655 réussites**, aucun échec, 17 tests préexistants ignorés. Fixtures PNG/GIF vérifiées avec Pillow : tailles attendues et 12/257 images lisibles ; profondeur ZIP : 65 composants. Un test ponctuel, retiré ensuite, appelle le chargeur de production avec les six aperçus : image normale, refus de dimension, corruption, animation normale, repli fixe et proportions SVG vérifiés sans fenêtre. Le test ponctuel du cache/export vérifie séparément les wrappers de production sans lancer l’application associée ; son résultat est consigné dans l’audit.

Les scripts n’ont pas lancé l’UI pendant la préparation. L’affichage réel, les clics, le sélecteur Windows et l’application externe restent à vérifier. Pas de conclusion générale de sécurité : dossiers de Move, Rename/Delete, sidecars de corbeille, fixation initiale des sources, fournisseurs de volumes et isolation des parseurs restent ouverts dans `AUDIT_SECURITE.md`.
