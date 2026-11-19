# CORE-03 : utilitaires, décodage d'images, documents Office, protocole d'aperçu

## Fichiers lus en entier

- [x] src/utils/image_preview_decode.rs
- [x] src/utils/mod.rs
- [x] src/utils/office_document.rs
- [x] src/utils/office_document/search.rs
- [x] src/utils/office_document/sheets.rs
- [x] src/utils/office_document/slides.rs
- [x] src/utils/office_preview.rs
- [x] src/utils/path_security.rs
- [x] src/utils/path_text.rs
- [x] src/utils/preview.rs
- [x] src/utils/preview_protocol.rs
- [x] src/utils/visibility.rs

Aucun constat critique ni élevé. Les limites de taille, de dimensions et de mémoire des décodeurs et du protocole sont cohérentes, et les lectures bornées sont correctes.

## Constats

### Moyen

1. `src/utils/office_document/sheets.rs:92-110` | moyen | confirmé. Au-delà de 64 colonnes, aucune cellule n'est plus créée, mais la valeur de chaque cellule suivante est écrite dans la dernière cellule existante. La 64e colonne affiche donc la valeur de la dernière colonne de la ligne, et non la sienne. Déclenchement : ouvrir dans le Viewer un classeur xlsx de plus de 64 colonnes remplies. La colonne 64 est fausse pour chaque ligne sans aucun avertissement, alors que la limite est présentée comme « les premières colonnes sont conservées ».

### Faible

2. `src/utils/office_document.rs:99-105, 316-324` | faible | confirmé. Même défaut de plafond à 64 cellules pour Word, ODF et PowerPoint : le texte des colonnes excédentaires est concaténé dans la 64e cellule. Déclenchement : tableau docx, odt ou ods de plus de 64 colonnes. La 64e cellule affiche un mélange de colonnes.

3. `src/utils/office_document/sheets.rs:23` | faible | plausible. Une entrée de chaînes partagées vide et auto-fermée n'est pas comptée, contrairement à la variante de recherche qui la traite. Tous les indices suivants sont décalés. Déclenchement : xlsx produit par un générateur qui écrit les chaînes vides sous cette forme. Le Viewer affiche des textes de cellules faux, et seulement ceux-là.

4. `src/utils/office_document.rs:109-121, 127-139` | faible | confirmé. Le plafond de sortie n'est testé qu'avant l'ajout d'une ligne, et une ligne (un paragraphe ou une rangée) n'est pas bornée. La sortie peut dépasser la limite de réponse du worker, que la constante de marge est censée respecter. L'encodage de la réponse refuse alors le texte (« réponse invalide »). Déclenchement : docx contenant un seul paragraphe de plus de 32 Mo de texte (le XML décompressé peut atteindre 128 Mo pour quelques centaines de Ko compressés). Le Viewer échoue au lieu de tronquer.

5. `src/utils/office_document.rs:259-263` | faible | confirmé. Le niveau de plan Word est converti en niveau de titre en ajoutant un. La valeur 255 déborde : panique dans les builds debug (les vérifications de débordement sont actives dans le profil dev), repli silencieux en release. Par ailleurs, le niveau 9 de Word (texte normal) devient un titre h4. Déclenchement : un paragraphe de document.xml avec un niveau de plan égal à 9 ou à 255.

6. `src/utils/office_document.rs:252` (idem `search.rs:207`) | faible | confirmé. Le nom d'élément « tabulation » est traité sans tenir compte de son parent. Les définitions de taquets dans les propriétés de paragraphe ajoutent chacune une espace, et le paragraphe commence par des espaces. Déclenchement : docx dont les paragraphes ont des taquets personnalisés. Les lignes du Viewer sont décalées, et le texte de recherche contient des espaces parasites.

7. `src/utils/office_document.rs:291-300` (idem `search.rs:189-194`) | faible | plausible. Un paragraphe ODF imbriqué (zone de texte ancrée dans un paragraphe) remet à faux l'état « dans le texte » à sa fermeture et vide le paragraphe parent. La suite du texte du paragraphe parent est perdue. Déclenchement : odt ou odp dont un paragraphe contient une zone de texte suivie de texte. Le texte manque dans le Viewer et dans l'index de recherche.

8. `src/utils/office_document.rs:248-274` | faible | plausible. Aucun traitement de l'alternative de contenu (choix et repli) : le texte d'une zone de texte Word existe dans les deux branches. Déclenchement : docx avec zone de texte enregistrée par Word. Le texte apparaît en double dans le Viewer.

9. `src/utils/office_document.rs:50-57` et `office_document/sheets.rs:76-81` | faible | plausible. Chaque partie est décompressée jusqu'à 128 Mo sans budget cumulé ni limite du nombre d'entrées, contrairement à la recherche (64 Mo cumulés, 10 000 entrées). Tant que la sortie n'est pas pleine (feuilles vides ou sans texte), les parties s'enchaînent. Déclenchement : xlsx ou pptx malveillant avec de nombreuses feuilles ou diapositives très compressibles. Le worker consomme son délai complet en CPU avant d'être tué.

10. `src/utils/office_preview.rs:21-24` | faible | plausible. Une seule lecture (non complète) de 8 Ko est faite sur la partie décompressée, puis la boucle s'arrête même si le résultat est vide ou court. Un décompresseur peut rendre moins d'octets que demandé. Les 8 premiers Ko d'un document.xml sont en grande partie des déclarations d'espaces de noms. Déclenchement : infobulle sur un docx, aperçu court ou vide, et essai de la cible suivante impossible.

11. `src/utils/office_preview.rs:41-47` | faible | confirmé. Le nom d'étiquette retenu est tronqué à 10 caractères, puis comparé à l'égalité stricte pour l'ODF. Comme ces éléments portent presque toujours un attribut de style, les sauts de ligne ODF ne sont jamais produits. Les entités (comme l'esperluette) ne sont pas décodées. Déclenchement : infobulle sur un odt, texte en un bloc, avec des entités brutes dans tous les formats.

12. `src/utils/office_document/search.rs:67-71` | faible | confirmé. La recherche ne lit que le corps principal pour Word (pas les en-têtes, pieds de page, notes de bas de page, commentaires) et ne lit pas non plus les feuilles de graphiques ni les commentaires pour Excel. Le contrat annonce pourtant un texte « complet ». Déclenchement : recherche d'un mot présent seulement dans un en-tête ou une note de bas de page. Aucun résultat, sans signal.

13. `src/utils/mod.rs:109-110` | faible | confirmé. La comparaison insensible à la casse convertit chaque octet en caractère puis le met en minuscules. Pour les octets UTF-8 non ASCII, cela transforme les octets de tête 0xC0-0xDE en 0xE0-0xFE et ne replie pas la casse des lettres non ASCII. Le résultat est un ordre incohérent avec le comparateur par caractères utilisé dans l'interface (`src/ui/workspace/sorting.rs`). Déclenchement : le tri des listings côté backend (`src/commands/io.rs:259`) avec des noms accentués, cyrilliques ou en majuscules non ASCII. « É » et « é » sont séparés, et les alphabets sur 2 octets sont mélangés avec les caractères sur 3 octets.

14. `src/utils/image_preview_decode.rs:189` | faible | confirmé. Une erreur de décodage sur n'importe quelle image d'une animation fait échouer tout l'aperçu, même quand les images précédentes sont valides. Déclenchement : gif ou webp animé tronqué (téléchargement interrompu). Aucune image n'est affichée, alors que les navigateurs affichent ce qui est décodé.

15. `src/utils/image_preview_decode.rs:76-81` | faible | plausible. Le SVG est analysé une deuxième fois, avec les options par défaut de usvg, dans le seul but d'obtenir la taille. Cela double le coût CPU de l'analyse d'un document non fiable. Avec les options par défaut, la résolution des images externes lit des chemins de fichiers (pas de dossier de ressources, donc chemins absolus ou relatifs au répertoire courant). Déclenchement : SVG dont les images référencent un fichier local. Le fichier est lu pour une analyse qui n'a besoin que de la taille. Le risque réel dépend du confinement du worker.

16. `src/utils/path_text.rs:29` | faible | plausible. Sous Windows, le texte d'un chemin passe par une conversion avec perte : un nom NTFS valide contenant un surrogate isolé devient U+FFFD. Plusieurs noms distincts partagent alors le même texte, et celui-ci ne rouvre pas le fichier d'origine. Déclenchement : fichier au nom UTF-16 invalide. Renommer, supprimer ou ouvrir échoue, ou vise un autre fichier qui contiendrait réellement U+FFFD.

17. `src/utils/preview.rs:9` | faible | plausible. Le fichier est ouvert sans vérifier qu'il s'agit d'un fichier régulier. Sous Linux, l'ouverture d'un tube nommé sans écrivain bloque indéfiniment. Déclenchement : survoler un tube nommé dont l'extension déclenche l'aperçu texte (appel dans `src/ui/components/tooltip/preview.rs:372`). Le thread du pool d'aperçus est bloqué sans fin, et des survols répétés épuisent le pool.

18. `src/utils/preview_protocol.rs:130, 140` | faible | plausible. Le récepteur accepte jusqu'à 1024 images par réponse (la limite de la politique du Viewer) même pour les aperçus d'infobulle, où la politique du worker s'arrête à 256, et il accepte un délai nul. Le test qui annonce refuser 257 images échoue en réalité sur des données tronquées et ne valide pas la limite. Déclenchement : un worker compromis par une image piégée renvoie des images avec un délai de 0 ms. Une animation sans pause est alors lancée côté interface.

### Optimisation

19. `src/utils/office_document.rs:50-57` | optimisation | plausible. La lecture d'une partie de 128 Mo grossit le tampon par doublement (jusqu'à 256 Mo de capacité), puis la conversion UTF-8 avec perte copie encore le texte (copie quasi systématique, même s'il est valide). Le pic atteint environ 384 Mo pour une limite de worker de 512 Mo (Linux). Déclenchement : docx ou xlsx dont une partie atteint la limite. Le worker peut être arrêté par la limite mémoire, ce que la conversion directe du tampon en texte, avec repli sur la copie, éviterait.

20. `src/utils/preview_protocol.rs:148-156` | optimisation | plausible. Chaque image est recopiée depuis la réponse brute vers un tampon propre, alors que la réponse entière (jusqu'à 256 Mo plus un en-tête) reste en mémoire pendant le décodage. Le pic dans le processus de l'interface atteint donc le double de la taille des pixels. Déclenchement : ouverture d'une image de près de 67 mégapixels dans le Viewer.

21. `src/utils/visibility.rs:34-41` | optimisation | confirmé. Le cache des attributs par dossier n'est jamais purgé : une entrée par dossier visité reste en mémoire, avec son chemin et la liste des noms cachés (lecture du fichier de liste à chaque dossier). Le chemin complet du parent est aussi haché pour chaque entrée. Déclenchement : recherche récursive sur une arborescence de plusieurs centaines de milliers de dossiers (`src/commands/search.rs`). La mémoire croît pendant toute la recherche, alors qu'un parcours en profondeur n'a besoin que du dossier courant.

22. `src/utils/office_document/sheets.rs:104-109` | optimisation | confirmé. Chaque cellule fait une copie de la chaîne partagée ou de la valeur, puis une seconde copie après le retrait des espaces. Déclenchement : feuille de plusieurs centaines de milliers de cellules. Deux allocations par cellule, évitables en déplaçant la valeur ou en empruntant.

<!-- LOT TERMINÉ -->
