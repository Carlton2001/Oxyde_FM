# Documentation d’Oxyde

Oxyde utilise Rust, GPUI et Tokio. Choisir une référence selon le travail à faire :

| Besoin | Document |
| --- | --- |
| Compiler, tester ou découper un module | [Développement](guides/development.md) |
| Développer la version Linux et connaître ce qui reste à faire | [Développement Linux](guides/linux-development.md) |
| Consulter le premier build Linux et la garde Windows | [Validation du 26 octobre](reports/2026-10-26-linux-foundation.md) |
| Consulter les premiers essais graphiques et la corbeille Linux | [Bilan du 26 octobre](reports/2026-10-26-linux-ui-trash.md) |
| Évaluer l'ampleur du portage Linux | [Audit de portabilité, 26 octobre 2026](reports/2026-10-26-linux-portability-audit.md) |
| Construire une version installable ou portable | [Distribution Windows](guides/distribution.md) |
| Comprendre les temps de distribution Windows | [Référence GitHub](reports/2026-10-22-windows-distribution.md), [tests locaux](reports/2026-10-22-distribution-local-tests.md), [mesures v2.0.1 et clé de cache](reports/2026-10-23-distribution-cache.md) |
| Comprendre les temps de la CI native et de l’audit Windows | [Mesures et validations](reports/2026-10-22-windows-ci.md) |
| Mettre à jour les avis de licences tiers | [Licences des dépendances](../licenses/dependencies/README.md) |
| Modifier thèmes, langues ou composants | [Interface](architecture/interface.md) |
| Modifier menus, infobulles ou fenêtres secondaires | [Fenêtres](architecture/windows.md) |
| Comprendre les aperçus et leurs caches | [Aperçus](architecture/previews.md) |
| Modifier les parcours d’archives | [Archives](architecture/archives.md) |
| Comprendre les opérations et leurs bilans | [Opérations](architecture/operations.md) |
| Comprendre le domaine volumes (pilote Linux/UDisks2) | [Volumes](architecture/volumes.md) |
| Modifier copier, couper et coller des fichiers | [Presse-papiers](architecture/clipboard.md) |
| Modifier la surveillance des dossiers (événements ou sondage) | [Surveillance](architecture/watchers.md) |
| Modifier l'accès rapide (favoris) et l'ouverture par le bureau | [Accès rapide](architecture/quick-access.md) |
| Vérifier l’effet de Turbo et les améliorations restantes | [Couverture de Turbo](reports/2026-10-17-turbo-coverage.md) |
| Modifier le listing, les filtres ou la recherche | [Navigation et recherche](architecture/search.md) |
| Diagnostiquer copies, suppressions ou fermeture | [Copie](diagnostics/copy.md), [suppression](diagnostics/delete.md), [fermeture](diagnostics/shutdown.md) |
| Mesurer la mémoire du Viewer et des aperçus | [Mémoire](diagnostics/memory.md) |
| Comparer les moteurs de copie | [Benchmark](guides/copy-benchmark.md) |
| Mesurer la compression et les échanges du worker | [Compression ZIP](diagnostics/archive-compression.md) |
| Connaître les protections et limites restantes | [Sécurité](security/status.md) |
| Réexaminer les dépendances conservées | [Dépendances](security/dependencies.md) |
| Préparer un essai manuel | [Validation visuelle](guides/visual-checks.md) |
| Retrouver un audit ou une preuve datée | [Bilans et historique](reports/README.md) |
| Consulter l’audit du code du 25 octobre 2026 | [Constats, validations et limites](reports/2026-10-25-code-audit.md) |

Les [consignes des agents](../AGENTS.md) restent à la racine. Les chemins de code entre accents graves partent de la racine du dépôt.

## Entretenir cette documentation

- **Nommage :** dossiers et fichiers en anglais, minuscules et `kebab-case` ; contenu technique en français. Exceptions : `README.md` pour les points d’entrée et `AGENTS.md` pour les consignes.
- **Classement :** `guides/` pour une procédure, `architecture/` pour un contrat actuel, `diagnostics/` pour un problème, `security/` pour les garanties/décisions, `reports/` pour un résultat daté.
- **Rapports :** `YYYY-MM-DD-subject.md`. Indiquer révision/périmètre, résultat, validation et limites. Une mesure passée n’est jamais une validation de la version actuelle.
- **Taille :** viser une page courte (environ 40–100 lignes), une question par document. Au-delà, enlever les répétitions avant de scinder ; éviter de remplacer un gros document par une multitude de petites notes.
- **Rédaction :** commencer par la réponse ou la procédure. Décrire l’état actuel, garder les exceptions utiles et les points d’entrée du code. Éviter les récits de chantier, listes exhaustives de tests et valeurs recopiées sans nécessité.
- **Source unique :** chaque règle a une référence ; les autres documents font un lien. Une correction met à jour le contrat concerné plutôt que d’ajouter un paragraphe chronologique.
- **Retrait :** fusionner les doublons, retirer les plans achevés. Conserver dans le bilan la révision Git permettant de retrouver les preuves uniques et les décisions ; maintenir les points encore ouverts dans la référence active.
- **Format :** UTF-8 sans BOM, fins de ligne LF, liens relatifs valides. Mettre à jour cet index lors d’un ajout, déplacement ou retrait.

Le README racine présente la version publiée ; sa mise à jour est réservée à la prochaine version.
