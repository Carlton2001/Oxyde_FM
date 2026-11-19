# Audit UI-APP-09 : visionneuse (Markdown, médias, PDF, texte, arbre, fenêtre)

## Fichiers lus en entier

- [x] src/ui/app/viewer/markdown/parse.rs
- [x] src/ui/app/viewer/markdown/selection.rs
- [x] src/ui/app/viewer/media.rs
- [x] src/ui/app/viewer/media_bar.rs
- [x] src/ui/app/viewer/media_labels.rs
- [x] src/ui/app/viewer/media_overlay.rs
- [x] src/ui/app/viewer/pdf.rs
- [x] src/ui/app/viewer/render.rs
- [x] src/ui/app/viewer/scrollbar.rs
- [x] src/ui/app/viewer/text.rs
- [x] src/ui/app/viewer/text/selection.rs
- [x] src/ui/app/viewer/text/syntax.rs
- [x] src/ui/app/viewer/tree.rs
- [x] src/ui/app/viewer/tree/loading.rs
- [x] src/ui/app/viewer/tree/render.rs
- [x] src/ui/app/viewer/video_layout.rs
- [x] src/ui/app/viewer/window.rs

Fichiers de contexte lus pour suivre un appel : markdown/loading.rs (appelant du parseur) et viewer.rs (chargement courant), par lectures ciblées.

## Constats

### Moyen

1. `src/ui/app/viewer/markdown/parse.rs:483` : moyen, confirmé. Le découpage d'une balise HTML d'un bloc `<pre>` sans `>` de fermeture coupe un octet avant la fin de la valeur ; si le dernier caractère est multi-octet, la coupe tombe à l'intérieur d'un caractère et provoque une panique du thread de parsing. Scénario : un fichier .md dont la dernière ligne, sans retour à la ligne final, se trouve dans un bloc `<pre>` et contient un `<` sans `>` ensuite, en finissant par un caractère non ASCII (par exemple une ligne se terminant par le symbole euro). Le chargement du Markdown échoue (ou la panique remonte selon le hook de panique).

2. `src/ui/app/viewer/markdown/parse.rs:138` (avec 273-275 et 300) : moyen, confirmé. Chaque image en ligne vide le bloc courant puis remet le type de bloc à « paragraphe » ou « citation » : le contexte titre, ligne de tableau ou puce est perdu pour la suite du bloc. Scénario : un titre contenant un badge puis du texte, une ligne de tableau avec une image dans une cellule, ou un élément de liste commençant par une image. Le texte qui suit n'est plus un titre ou une ligne de tableau, et la puce se retrouve seule dans son propre bloc avant l'image.

3. `src/ui/app/viewer/pdf.rs:83-89` (avec 17 et 68-79) : moyen, plausible. La plage de pages à décoder se calcule à partir de la hauteur de page et de la taille du volet, sans plafond, alors que le cache n'en garde que 6. Quand la plage dépasse 6, chaque rendu évince une page encore visible (annulation et suppression de l'image) puis la redemande, ce qui provoque un décodage en boucle. Scénario : PDF de diapositives paysage dans un volet Viewer étroit et haut (largeur 350 px, hauteur 900 px, soit 8 pages dans la plage). Charge CPU et mémoire continue, pages qui clignotent.

4. `src/ui/app/viewer/text.rs:128-137` : moyen, confirmé. Le fichier est lu en UTF-8 strict, sinon en conversion avec perte, sans détection de BOM ni d'UTF-16 ni de page de code ANSI. Aucune détection d'encodage n'existe dans le dossier viewer. Scénario : ouvrir un .reg, un .csv Excel ou une sortie PowerShell en UTF-16 LE, très courants sous Windows, ou un fichier cp1252. Le contenu s'affiche en caractères de remplacement ou avec des NUL. Un BOM UTF-8 reste aussi en tête comme caractère invisible.

### Faible

5. `src/ui/app/viewer/markdown/parse.rs:112` : faible, plausible. Le rendu coloré des blocs de code dépend du temps écoulé depuis le début du parsing (200 ms), donc du matériel et de la charge. Scénario : au premier document ouvert, l'initialisation paresseuse du jeu de syntaxes consomme le budget, et tous les blocs de code suivants du même document restent sans couleur, alors qu'ils sont colorés à la réouverture.

6. `src/ui/app/viewer/markdown/parse.rs:51-66` : faible, plausible. Les images locales relatives acceptent les composantes `..` et les noms de périphériques Windows réservés. Scénario : un Markdown référence une image hors du dossier du document ou un nom réservé (NUL, COM1), et le chargeur d'images les ouvre. Un fichier de périphérique peut bloquer la lecture. Aucune sortie de données, mais la confinement au dossier du document n'est pas appliqué.

7. `src/ui/app/viewer/markdown/selection.rs:134-137` : faible, plausible. Ctrl+A sélectionne jusqu'au dernier bloc présent à cet instant ; pendant le parsing progressif, Ctrl+A puis Ctrl+C copie un document tronqué sans indication. Scénario : gros Markdown encore en cours de chargement.

8. `src/ui/app/viewer/media.rs:238-244` : faible, confirmé. Appuyer sur lecture après la fin d'un média détruit la session sans enregistrer de reprise ; la nouvelle session démarre alors en pause si la lecture automatique est désactivée. Scénario : lecture automatique désactivée, un média se termine, l'utilisateur clique sur lecture : rien ne joue, un second clic est nécessaire (et l'icône affiche brièvement « pause »).

9. `src/ui/app/viewer/media_overlay.rs:29` : faible, confirmé. Pour un nom sans point, le nom entier est pris pour l'extension. Même logique dans le chargement courant du Viewer. Scénario : un fichier sans extension nommé « mp3 », « mp4 » ou « avi » est traité comme média natif.

10. `src/ui/app/viewer/media_overlay.rs:409-479` : faible, plausible. Si l'ouverture des fenêtres popup échoue, l'indicateur d'attente est remis à zéro puis la notification relance le rendu, qui recrée les popups tant que le pointeur reste dessus : boucle d'erreurs journalisées et de tentatives. Si seules certaines fenêtres s'ouvrent, le trio reste incomplet et n'est jamais retenté.

11. `src/ui/app/viewer/text/selection.rs:141-143` : faible, plausible. Ctrl+A puis Ctrl+C sur un fichier de plusieurs centaines de Mio copie la sélection entière dans une chaîne sur le thread UI (plafond de 512 Mio), puis la convertit pour le presse-papiers. Gel et pic mémoire.

12. `src/ui/app/viewer/tree.rs:230-243` : faible, confirmé. Replier puis redéplier un dossier réutilise le niveau déjà présent, sans relecture. Un niveau en échec n'est jamais retenté, et les modifications disque survenues entre-temps ne sont pas vues. Scénario : lecture échouée à cause d'un verrou transitoire ou d'un partage réseau ; le dossier reste en « échec » jusqu'à la fermeture du Viewer.

13. `src/ui/app/viewer/tree.rs:54` (avec tree/loading.rs:75 et tree.rs:129) : faible, plausible. Les sommes de tailles utilisent des additions non protégées ; les tailles d'une archive forgée peuvent déborder (panique en debug, valeur fausse en release). Scénario : archive dont les en-têtes déclarent des tailles proches de la limite de u64.

14. `src/ui/app/viewer/tree/loading.rs:155-157` : faible, confirmé. Si la création du thread de comptage échoue, les totaux restent à l'état « comptage en cours » indéfiniment. Cas rare (épuisement de ressources).

### Optimisation

15. `src/ui/app/viewer/media.rs:189-220` : optimisation, confirmé. La tâche de suivi se réveille toutes les 80 ms tant que la session n'est pas annulée, y compris quand le média est terminé ou en pause, et clone les informations du média à chaque réveil. Scénario : un onglet Viewer laissé sur un média terminé garde un réveil permanent.

16. `src/ui/app/viewer/text.rs:135` : optimisation, confirmé. Si le fichier contient un seul octet invalide, ou si la troncature à 512 Mio coupe un caractère, tout le tampon est recopié par la conversion avec perte (pic mémoire d'environ le double). Scénario : très gros journal tronqué à 512 Mio.

17. `src/ui/app/viewer/text.rs:281-308` : optimisation, plausible. Chaque changement de largeur lance un réencadrage complet sur le fichier entier, non interruptible une fois démarré ; l'ancien résultat est abandonné mais son calcul continue. Scénario : redimensionner la fenêtre sur un fichier de plusieurs centaines de Mio empile des calculs CPU.

18. `src/ui/app/viewer/text.rs:334` : optimisation, plausible. L'ajout de chaque lot de coloration passe par un copier-sur-écriture ; si un instantané du rendu précédent détient encore la liste, la liste de tous les segments (jusqu'à un million) est recopiée à chaque lot. Scénario : gros fichier source colorié pendant l'affichage, copie quadratique.

19. `src/ui/app/viewer/tree/loading.rs:47-55` : optimisation, plausible. Avant de publier un niveau, jusqu'à 1000 sous-dossiers sont ouverts un par un pour savoir s'ils ont du contenu. Aucun affichage partiel. Scénario : dossier de 1000 sous-dossiers sur un lecteur réseau : attente prolongée à « chargement ».

20. `src/ui/app/viewer/scrollbar.rs:60` : optimisation, confirmé. L'identifiant de piste (chaîne) est cloné à chaque événement de déplacement de souris pendant le glissement de l'ascenseur. Allocation évitable.

21. `src/ui/app/viewer/window.rs:153` : optimisation, plausible. Le titre de la fenêtre détachée est réécrit à chaque rendu, donc à chaque tick de progression média (80 ms) et à chaque survol, ce qui produit un appel système par rendu.

<!-- LOT TERMINÉ -->
