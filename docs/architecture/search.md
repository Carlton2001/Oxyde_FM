# Navigation, tri et recherche

Chaque onglet possède ses critères, résultats et annulation. Les workers préparent les données ; la publication vérifie l’identité de l’onglet et la révision de la demande. Une ancienne réponse ne doit jamais remplacer une navigation plus récente.

Les résultats de doublons utilisent une liste virtualisée commune aux en-têtes
de groupes et aux fichiers. Filtre, tailles minimales/maximales et plages de lignes
sont préparés en arrière-plan ; replier un groupe invalide la préparation précédente.
Le rendu ne parcourt que les lignes demandées par le viewport, même pour un groupe
de 100 000 fichiers. Une taille commune est affichée si tous les membres ont la
même taille ; sinon, le groupe affiche leur plage de tailles.

## Listing et filtres

Les dossiers restent en tête en ordre ascendant comme descendant. L’UI trie les noms en ordre naturel, en minuscules Unicode et sans retrait d’accents : `fichier2` précède `fichier10`. Les suites de chiffres ASCII sont comparées sans limite de taille ; à valeur égale, le moins de zéros initiaux précède (`2`, `02`, `002`). Les extensions restent lexicographiques, avec les noms en départage naturel pour toutes les colonnes et des égalités stables. Le tri naturel public du backend garde ses autres consommateurs.

Nom et emplacement utilisent une sous-chaîne insensible à la casse mais sensible aux accents. Préparer la requête une fois par parcours. La visibilité des fichiers cachés/système doit rester cohérente entre listing et recherche.

Le regroupement par date appartient à l’onglet et est persisté ; son repli reste temporaire. Les six périodes calendaires sont communes aux filtres et groupes. Le changement de jour/fuseau invalide les caches en arrière-plan, y compris pour les onglets inactifs, sans relire les fichiers. Les groupes utilisent la même géométrie pour rendu et sélection.

## Tailles des dossiers

Le calcul fournit une taille indicative des éléments accessibles, adaptée à un explorateur de fichiers. L’exhaustivité d’un outil spécialisé d’analyse disque n’est pas un objectif : des éléments illisibles peuvent être omis sans demander de droits supplémentaires ni afficher un statut d’incomplétude dédié. Les watchers et recalculs différés améliorent l’actualisation, sans garantir une mesure exhaustive ni un instantané. Cette limite est acceptée dans le [suivi A04 de l’audit](../reports/2026-10-25-code-audit.md#a04--taille-des-éléments-accessibles--limite-acceptée).

Le bouton de la barre de statut active le calcul pour le chemin de l’onglet jusqu’à sa prochaine navigation. Un rafraîchissement conserve les tailles non invalidées et ne relance que les dossiers signalés, non mesurés ou dont une mesure invalidée a échoué. Un parcours déjà en cours ou prêt à publier n’est pas dupliqué s’il n’a reçu aucun nouveau changement. Le listing actualisé ajoute les nouveaux dossiers à mesurer. La surveillance devient récursive pendant ce mode ; un changement profond ne recalcule que le sous-dossier immédiat concerné. Les liens symboliques et jonctions sont exclus du calcul, les vues virtuelles et archives de ce mode.

Les événements sont regroupés par racine et enfant, avec repli sur une invalidation complète en cas de débordement ou de perte d’événements. Le refresh prend aussi en compte les événements déjà reçus avant le traitement périodique de 500 ms, sans les réappliquer deux fois ni perdre les demandes de listing ou les changements des autres onglets sur la même racine. Conserver une taille sans nouveau parcours nécessite un watcher natif récursif sans erreur ; en son absence, pendant une installation, après une erreur ou avec du polling, un refresh réinvalide toutes les tailles par précaution.

La surveillance reste active pendant le premier passage pour mémoriser les changements. La file conserve au plus une demande en attente par dossier ; une modification pendant son parcours ou avant publication conserve la mesure affichée et ajoute un seul nouveau passage. Une valeur à actualiser change seulement de couleur (`accent` du thème actif), sans préfixe ni changement de police ou de largeur. En grille, le libellé de type du dossier utilise la même couleur. Les trois phrases explicatives figurent sur trois lignes dans un cadre de l’infobulle normale du fichier : bordure et texte accent, fond accent légèrement teinté. Le calcul de largeur inclut les marges et la bordure du cadre. Aucune infobulle n’est imbriquée sur la taille ou le libellé. La session de survol commune reste seule responsable de l’affichage et de la fermeture. La couleur normale revient après publication d’un parcours sans nouveau changement.

L’activation explicite demande l’installation du watcher récursif avant de démarrer les premières mesures, puis démarre sans temporisation fixe. Un échec d’installation journalisé laisse le calcul initial démarrer. L’installation ne réinvalide pas les demandes initiales encore intactes ; un remplacement de watcher conserve une vérification prudente pour les mesures déjà commencées ou disponibles. La mise en file initiale seule ne marque pas une taille provisoire. L’infobulle indique une actualisation en cours ou en attente, qui peut aussi venir d’une vérification prudente, sans affirmer qu’un changement a été observé.

Les événements de surveillance sont regroupés sur 500 ms. Les dossiers sans première mesure passent avant les recalculs éligibles du même onglet. Après une mesure, un recalcul automatique attend au moins cinq secondes après la fin du parcours et cinq secondes sans nouvel événement. L’activité continue reporte donc les recalculs, en conservant la dernière valeur affichée : les grandes arborescences actives ne sont plus reparcourues en boucle. La première mesure reste publiable même sous des écritures continues. Actualiser ou recliquer sur le bouton des tailles force les demandes nécessaires sans attendre l’accalmie, en conservant les tailles à jour ; les événements suivants ne repoussent pas cette demande explicite et ne transforment pas les passages suivants en recalculs forcés.

Un résultat achevé attend sa publication avant qu’un autre parcours du même dossier puisse commencer. L’ordonnanceur reste actif pendant ces attentes, sans occuper de place physique, et remplit deux places sans attendre les dossiers plus lents ni la préparation de leur affichage ; le moteur conserve sa limite globale de deux parcours physiques, y compris après annulation. Les changements survenus pendant un listing demandent un seul listing supplémentaire. La surveillance native reste événementielle. Les supports utilisant le polling conservent deux secondes pour les listings non récursifs ; leurs instantanés récursifs sont espacés de trente secondes, car chacun parcourt le sous-arbre. Leur instantané initial et leurs parcours périodiques gardent un coût disque et une détection profonde moins immédiate.

La préférence Turbo générale s’applique aussi aux Propriétés. La priorité Windows suit le mode pendant le parcours ; le budget est choisi au prochain parcours sans redémarrer les travaux en cours. Chaque parcours SSD utilise jusqu’à 2 workers sans Turbo et jusqu’à 8 avec Turbo, avec un plafond adapté aux processeurs logiques ; HDD, réseau ou stockage inconnu restent à 1 worker par parcours. Les pools sont réutilisés, indépendants entre les deux parcours, et leurs threads reviennent à la priorité normale en fin de travail. Les compteurs sont agrégés par répertoire avant fusion. Les résultats attendent dans une file distincte et sont publiés par lots de 128 au plus, avec 50 ms de regroupement avant chaque lot ; leurs révisions sont revérifiées avant publication. Une préparation devenue obsolète est retentée sur le sous-ensemble encore valide, sans bloquer sur un dossier modifié. Les exclusions `System Volume Information` et `Microsoft\Windows\Containers` restent actives dans les deux modes, sur des composants de chemin complets sans distinction de casse. Mesures et portée des tests : [bilan du 25 octobre](../reports/2026-10-25-folder-sizes-turbo.md).

Chaque résultat vérifie l’identité de l’onglet, le chemin, la génération du mode et la révision du dossier avant publication. Tri, filtres et résumés des colonnes sont préparés en arrière-plan. Une navigation annule le parcours et vide la file, y compris vers une recherche ou après retrait du disque. Les erreurs de surveillance sont journalisées ; un rafraîchissement manuel permet toujours de relancer le calcul. Le résultat reste une mesure du parcours, sans garantie d’instantané face aux écritures concurrentes.

## Parcours de recherche

| Parcours | Contrat |
| --- | --- |
| Complétion du chemin | Jusqu’à douze dossiers, lecture différée de 150 ms ; Haut/Bas, Tab et clic gardent l’édition active ; pas de complétion virtuelle |
| Recherche rapide | Noms récursifs, insensible à la casse, motifs `*`/`?`, départ après 350 ms ou Entrée ; ni contenu ni archives |
| Recherche avancée | Critères conservés dans sa fenêtre ; lancement dans le panneau d’origine, sans fermer le formulaire |
| Recherche dans les archives | Noms/métadonnées via worker dédié ; critères non pris en charge signalés, sans publier des correspondances trompeuses |
| Doublons | Nom, taille et contenu combinables ; comparer le contenu impose une même taille ; racines recouvrantes dédupliquées ; dossiers `$Recycle.Bin` exclus sans distinction de casse, y compris les racines situées dans une corbeille |

Les résultats arrivent par lots bornés ; Arrêter conserve ceux déjà reçus. La vue virtuelle ne propose ni création de dossier ni collage. Ouvrir l’emplacement sélectionne et révèle le fichier dans son dossier.

## Contenu texte : mémoire bornée

`commands/search/content.rs` conserve les lignes courtes jusqu’à 64 Kio, puis utilise un automate Thompson incrémental. Les frontières de blocs ne doivent modifier ni ancres, ni Unicode, ni correspondance : pas de troncature ou de chevauchement fixe approximatif.

Valider toute la ligne avant de publier une correspondance ; erreur UTF-8/I/O ou annulation l’invalident. La mémoire de l’automate dépend du motif. La normalisation des séquences combinatoires exceptionnelles peut utiliser un temporaire privé : la borne RAM n’est pas un quota disque.

Cette borne concerne le texte brut ; le parcours Office est décrit ci-dessous.
La recherche de contenu dans les archives reste indisponible. Aucun de ces parcours
ne garantit un plafond RSS global ni un instantané face aux modifications concurrentes.
Preuves du texte brut : [bilan A09](../reports/2026-10-17-code-review.md).

## Recherche de contenu Office

`Platform::office_search_text` produit du texte entier dans son périmètre ou une
erreur explicite. Sous Windows, un mode dédié d’`oxyde-image-worker` reçoit seulement
le handle source en lecture et son extension. Il est lancé en LPAC sans capacité,
avec le superviseur et l’admission agrégée existants, sans repli local si le worker
est absent, en erreur ou hors délai. Son admission est distincte des aperçus Office ;
l’annulation est contrôlée pendant l’attente et la supervision.

Limites : source encodée 128 Mio, 10 000 entrées, 1 Mio de noms cumulés, 16 Mio par
partie XML et 64 Mio de XML cumulés, 8 Mio de texte en réponse, job de 512 Mio et
délai de 15 secondes incluant l’attente d’admission. Le parent ne décompresse pas
le document et valide une réponse texte bornée avant d’utiliser le moteur de
recherche en flux. Linux utilise le même extracteur dans un [worker confiné](linux-previews.md#images-office-et-audio), sans garantie LPAC.

Le corps Word, les parties des diapositives et les cellules des feuilles sont
parcourus sans limite arbitraire aux trois premières diapositives ou aux 64 premières
colonnes. Les cellules inline et les références de chaînes partagées sont prises
en charge ; les entités XML partagent leur décodage avec le Viewer. La recherche
ne calcule pas les formules ou les formats de nombres et ignore les notes de
présentation OpenDocument, comme le texte principal du Viewer.

Une limite de décompression ou de sortie, un document illisible ou un échec du
worker produit `ContentSkipped` : le bandeau indique dans les cinq langues que
certains contenus n’ont pas été vérifiés et donne le premier chemin concerné.
Aucune troncature Office n’est présentée comme une recherche complète réussie.
