# Navigation, tri et recherche

Chaque onglet possède ses critères, résultats et annulation. Les workers préparent les données ; la publication vérifie l’identité de l’onglet et la révision de la demande. Une ancienne réponse ne doit jamais remplacer une navigation plus récente.

## Listing et filtres

Les dossiers restent en tête en ordre ascendant comme descendant. L’UI trie les noms en ordre naturel, en minuscules Unicode et sans retrait d’accents : `fichier2` précède `fichier10`. Les suites de chiffres ASCII sont comparées sans limite de taille ; à valeur égale, le moins de zéros initiaux précède (`2`, `02`, `002`). Les extensions restent lexicographiques, avec les noms en départage naturel pour toutes les colonnes et des égalités stables. Le tri naturel public du backend garde ses autres consommateurs.

Nom et emplacement utilisent une sous-chaîne insensible à la casse mais sensible aux accents. Préparer la requête une fois par parcours. La visibilité des fichiers cachés/système doit rester cohérente entre listing et recherche.

Le regroupement par date appartient à l’onglet et est persisté ; son repli reste temporaire. Les six périodes calendaires sont communes aux filtres et groupes. Le changement de jour/fuseau invalide les caches en arrière-plan, y compris pour les onglets inactifs, sans relire les fichiers. Les groupes utilisent la même géométrie pour rendu et sélection.

## Parcours de recherche

| Parcours | Contrat |
| --- | --- |
| Complétion du chemin | Jusqu’à douze dossiers, lecture différée de 150 ms ; Haut/Bas, Tab et clic gardent l’édition active ; pas de complétion virtuelle |
| Recherche rapide | Noms récursifs, insensible à la casse, motifs `*`/`?`, départ après 350 ms ou Entrée ; ni contenu ni archives |
| Recherche avancée | Critères conservés dans sa fenêtre ; lancement dans le panneau d’origine, sans fermer le formulaire |
| Recherche dans les archives | Noms/métadonnées via worker dédié ; critères non pris en charge signalés, sans publier des correspondances trompeuses |
| Doublons | Nom, taille et contenu combinables ; comparer le contenu impose une même taille ; racines recouvrantes dédupliquées |

Les résultats arrivent par lots bornés ; Arrêter conserve ceux déjà reçus. La vue virtuelle ne propose ni création de dossier ni collage. Ouvrir l’emplacement sélectionne et révèle le fichier dans son dossier.

## Contenu texte : mémoire bornée

`commands/search/content.rs` conserve les lignes courtes jusqu’à 64 Kio, puis utilise un automate Thompson incrémental. Les frontières de blocs ne doivent modifier ni ancres, ni Unicode, ni correspondance : pas de troncature ou de chevauchement fixe approximatif.

Valider toute la ligne avant de publier une correspondance ; erreur UTF-8/I/O ou annulation l’invalident. La mémoire de l’automate dépend du motif. La normalisation des séquences combinatoires exceptionnelles peut utiliser un temporaire privé : la borne RAM n’est pas un quota disque.

Cette correction concerne le texte brut. Le parseur Office conserve ses limites et la recherche de contenu dans les archives reste indisponible. Elle ne garantit ni un plafond RSS global ni un instantané du fichier face aux modifications concurrentes. Preuves et incidents : [bilan A09](../reports/2026-10-17-code-review.md).
