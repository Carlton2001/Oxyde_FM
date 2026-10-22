# Audit des changements mémoire du Viewer — 22 octobre 2026

Périmètre : changements non commités de retrait des textures du Viewer,
initialisation paresseuse des syntaxes, instrumentation mémoire Windows et
copie locale du backend GPUI Windows. Les corrections des tooltips font l'objet
d'un [audit distinct](2026-10-22-tooltip-memory.md).

## Constats et corrections

### Pages PDF évincées : retrait de l'atlas incomplet

Le changement de `Viewer::replace_content` différait correctement le retrait
des images pour attendre le retour de la fenêtre courante dans `App.windows`.
`request_pdf_page` utilisait encore `drop_image(image, None)` pendant le rendu
des items de liste. Cette fenêtre était donc précisément absente du parcours
de nettoyage, et les textures des pages déjà évincées du cache pouvaient
rester dans son atlas. La fermeture du document ne pouvait plus les retrouver
dans les six pages du cache.

Le helper privé `content::retire_images` applique désormais le même retrait
différé au remplacement du contenu et aux évictions PDF. Il enregistre aussi
l'état Retired dans les diagnostics. Les résultats non publiés n'ont pas de
texture à retirer et sont simplement libérés.

### PDF fermé ou page remplacée : demandes encore actives

Les demandes PDF possédaient des drapeaux d'annulation, mais détruire le contenu
PDF ne les activait pas. Les tâches en attente pouvaient donc encore acquérir
le verrou de session et rendre un ancien document. La destruction du PDF et
l'éviction d'une demande activent désormais son drapeau. Un appel natif déjà
engagé reste non préemptif.

Une demande pouvait également être annulée en quittant le viewport, puis
relancée pour le même numéro de page. L'ancienne réponse supprimait alors
l'entrée pending de la nouvelle demande et pouvait publier à sa place.
La publication vérifie désormais l'identité de la demande, en plus de la
génération du document.

### Diagnostic incomplet pour les contenus progressifs

Le registre était alimenté au décodage d'une image complète et lors de
`replace_content`. PDF et Markdown publient cependant leurs pixels après la
création de leur contenu initialement vide, sans rappeler cette méthode.
Leurs pixels pouvaient donc manquer aux compteurs live/pending/retired.

Les décodeurs PDF et Markdown enregistrent maintenant les résultats dès leur
décodage ; les callbacks de publication les passent à Shown et le nettoyage
les passe à Retired. Les références du registre restent faibles. Les périmètres
partiels, notamment les aperçus de repli, restent explicites dans le
[guide de diagnostic](../diagnostics/memory.md).

Le guide comportait aussi une section ajoutée en Windows-1252 après du texte
UTF-8. Il est maintenant entièrement en UTF-8, sans modifier son contenu
technique historique.

## Changements validés sans correction supplémentaire

- Le retrait différé du contenu inclut les fenêtres détachées présentes dans
  `App.windows`. Les images ne sont pas recopiées : les listes de nettoyage
  partagent les pixels par Arc le temps du nettoyage.
- Les grammaires Syntect ne sont plus préchargées à la création du Viewer.
  Leur durée de vie reste celle du processus une fois utilisées.
- Les sondages de mémoire système et de caches s'exécutent hors UI. Les appels
  Windows de production passent par `Platform::process_memory_snapshot`.
  Le registre est borné, ne possède pas les pixels et signale les admissions
  manquées. Les compteurs ne constituent pas un instantané atomique.
- Comparaison de la copie GPUI avec la révision épinglée : seuls le manifeste,
  la déclaration du module et `directx_atlas.rs` divergent parmi les fichiers
  amont ; les ajouts sont le helper de validation, ses tests et la provenance.
  Les autres sources sont identiques. Le lien relatif de licence du crate est
  remplacé par le texte de licence amont de cette même révision pour que la copie
  reste autonome.
- Le parcours initial-data du backend ne s'applique qu'à une nouvelle texture
  dont les deux côtés correspondent à la source, de 1024 à 16384 px. Longueur,
  pitch, formats et nombre de sous-ressources sont cohérents. Les textures
  existantes et sous-régions conservent leur upload usuel. Les règles des API
  sont décrites par Microsoft : [CreateTexture2D](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11device-createtexture2d),
  [D3D11_SUBRESOURCE_DATA](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_subresource_data).

## Vérifications et limites

Les nouveaux tests couvrent l'annulation des demandes à la fermeture,
l'éviction sur 20 pages avec six conservées au plus, la libération du cache et
le rejet d'une ancienne réponse sans suppression de la demande qui la remplace.
Un test GPUI natif peint une page de 1024 × 1024, l'évince depuis un callback
de cette même fenêtre et vérifie à la fois la disparition de ses pixels et un
événement réel de retrait de l'atlas avant fermeture de la fenêtre. Il échoue
si le retrait immédiat est réintroduit.

Résultats locaux :

| Vérification | Résultat |
| --- | --- |
| Viewer et registre mémoire, tests ordinaires ciblés | 41 réussis ; 8 probes ignorés dans ce passage |
| Validation pure des dimensions et buffers d'upload | 2 réussis |
| Éviction PDF pendant le callback de la fenêtre | 1 réussi ; retrait et destruction réels de la texture constatés |
| Libération des pixels après remplacement du contenu | 1 réussi, en processus natif isolé |
| Sélection, rendu et détachement du Viewer | 1 réussi, en processus natif isolé |
| Huit grandes images dans GPUI, puis repos de 30 s | 1 réussi ; huit textures de 64 000 000 octets retirées/détruites |
| D3D11 initial-data et relecture GPU couleur/alpha | 1 réussi, en processus isolé |

Soit 48 tests/probes réussis. Les probes natifs, ignorés par défaut, ont bien
été exécutés individuellement. La sonde GPUI mesurait 104 476 672 Private Bytes
avant les images et 133 447 680 après nettoyage/repos de 30 s (environ 127,3 Mio),
sans purge du pilote. La sonde D3D11 mesurait 45 715 456 octets après déliaison
et 30 s de repos, avant son appel diagnostique à Trim. Ces valeurs ne sont pas
des seuils portables ni des tailles de pixels encore possédés par le Viewer.

Le contrôle final `cargo check --offline --locked --all-targets` est passé sans
avertissement du projet. `cargo build --offline --locked --bins` a également
réussi et reconstruit les exécutables. Les changements restent non commités.

Le Viewer conserve volontairement l'image à pleine résolution pour le zoom,
avec les limites existantes de 16384 px / 256 Mio de pixels. Cet audit ne
remplace pas le Viewer par une miniature. Les anciennes frames peuvent encore
conserver brièvement des références ; les Private Bytes et le RSS ne se
déduisent pas des seuls compteurs de pixels. Les mesures graphiques restent
propres au pilote testé. Linux n'est pas testé par les probes Windows.
