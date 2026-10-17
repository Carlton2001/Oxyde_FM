# Opérations de fichiers et historique

`FileOperationManager` exécute en arrière-plan ; `OxydeApp` possède l’état d’interface. `components/operations/` sépare notifications, fenêtres, cartes, indicateurs et graphe.

## Présentation et persistance

Chaque opération active occupe une notification compacte de progression. Sa fin retire cette ligne et affiche le résultat habituel. Un clic ouvre ou réactive le gestionnaire unique et redimensionnable ; il ne s’ouvre pas automatiquement. Fermer ou masquer le gestionnaire conserve opérations et historique ; la fermeture principale retire ses fenêtres.

Les opérations actives restent dépliées ; les états finaux sont repliés par défaut. Les détails montrent au plus cinq racines sélectionnées. « Éléments concernés » ouvre une liste virtualisée par opération, avec filtre du chemin complet préparé en arrière-plan et rejet des résultats obsolètes. Les icônes réutilisent `render_file_icon`.

Le graphe s’affiche uniquement pendant les opérations actives ayant des mesures de débit. Il disparaît dans les états finaux ; la durée et le débit moyen restent visibles dans les détails. Les résumés de la fenêtre principale passent par `operation_counts()`, `list_active_progress()` et `operation_state()` ; ne pas cloner toutes les sources et tous les bilans pour un compteur ou un simple survol.

La liste des opérations reste uniquement en mémoire pendant la session. Aucun historique d’opérations n’est écrit ni restauré au démarrage ; les anciens fichiers `operation_history.json` sont ignorés.

## Bilans individuels

| État | Signification |
| --- | --- |
| Traité | Résultat conservé confirmé |
| Non traité / ignoré | Aucun traitement, ou décision d’ignorer |
| Interrompu | Commencé, sans effet conservé confirmé |
| Partiellement traité | Effets conservés confirmés : fichier publié ou dossier créé |
| Inconnu | Preuves insuffisantes, ancien bilan ou parcours virtuel non couvert |

Les reçus réels priment sur la progression. Les transferts suivent les descendants et la suppression de la racine déplacée ; les suppressions suivent la confirmation effective. Les anciens bilans Partiel non discriminants sont chargés comme inconnus, avec versionnement JSON. Une erreur finale ne supprime pas les reçus confirmés.

## Contrat Turbo

La préférence fixe le mode initial des six types d’opérations ; le bouton agit sur une seule opération. Turbo ne pilote ni navigation, ni recherche, ni aperçus. Sans Turbo, les threads de travail passent en mode d’arrière-plan Windows.

Les priorités passent toutes par `apply_thread_priority` / `follow_turbo_priority` (`src/platform/thread_priority.rs`). Chaque transition est absolue — y compris le retour à `THREAD_PRIORITY_NORMAL` — et un état déjà appliqué n’est pas renvoyé au système. `RestoreNormalPriority` rend les threads réutilisés à la priorité normale ; un échec natif inattendu est journalisé. Linux n’applique aucune priorité.

| Opération | Priorité Turbo | Concurrence Turbo | Moment d’application |
| --- | --- | --- | --- |
| Copie, déplacement entre volumes | Normale | SSD (sources et destination) : jusqu’à 4 fichiers dès 2 fichiers. Disque rotatif, réseau ou inconnu : jusqu’à 4 seulement si au moins 8 fichiers de moins de 32 Mio en moyenne | Priorité : entre fichiers et pendant la copie. Concurrence : aux frontières de fichiers. |
| Renommage sur le même volume | Normale | Aucune | Au démarrage de l’opération |
| Suppression définitive | Haute, dès le démarrage | Jusqu’à 4 éléments sélectionnés | Entre éléments et pendant le parcours d’un dossier |
| Corbeille | Normale | Aucune | Avant chaque lot et aux points de contrôle du Shell, dossiers compris |
| Compression | Priorité du processus worker | ZIP, 7z, TAR.ZST : [limites](archives.md#compression-et-turbo-sous-windows) | Aux frontières de fichiers ou de blocs ; TAR.ZST au démarrage du flux |
| Extraction | Priorité du processus worker | Aucune ; le décodage reste monothread | Aux points de contrôle |

Le type de stockage vient de `Platform::is_ssd` (pénalité de recherche), interrogé une fois par lecteur ; les chemins `\\?\` sont normalisés. Chaque copie relit sa sortie pour la vérifier : sous Windows, cette lecture attend l’analyse de Defender (15 à 25 ms par fichier mesurés), latence que plusieurs workers recouvrent.

Limites structurelles : le Shell traite la Corbeille de façon séquentielle, TAR n’a rien à compresser, un renommage ne transfère pas de données et un fichier ZIP reste un flux DEFLATE unique. Limites d’implémentation actuelles :

- Un dossier supprimé reste parcouru par un seul thread (environ 35 µs par fichier sur NVMe) ; déléguer chaque fichier à des workers s’est révélé plus lent. Paralléliser des sous-arbres indépendants exigerait un point de contrôle partageable entre threads.
- Le décodage 7z multithread de lzma-rust2 bufferise des segments LZMA2 entiers sans borne : il reste désactivé.
- L’extraction ZIP en parallèle demanderait plusieurs flux simultanés dans le protocole du worker.

## Conflits et intégrité

Le plan réserve toutes les destinations avant le premier effet, y compris entre racines distinctes. L’autorisation de remplacer une destination existante ne permet pas d’écraser une autre source du même lot. Un déplacement sur soi est ignoré ; une copie sur soi choisit un nom libre traduit et le backend garde la publication exclusive.

Le dialogue conserve les métadonnées source/cible et les décisions par chemin. Le mode avancé hérite des décisions des dossiers, avec priorité aux exceptions explicites. Au premier choix individuel, les indécis reçoivent le choix contraire ; ce défaut automatique est identifiable et ne se réapplique pas ensuite. Aucun transfert avec conflit ne démarre avant validation complète. Échap annule l’opération en attente.

Copier/couper depuis la Corbeille conserve les chemins physiques comme identité et résout les noms via `Platform`. Restaurer reste un parcours distinct. La restauration directe traite les éléments indépendants sans écrasement ni création automatique du parent absent ; ses échecs n’empêchent pas les suivants. La suppression Shell garde ses limites de course par chemin : [sécurité R2](../security/status.md).

Pour mesurer les opérations : [copie](../diagnostics/copy.md) et [suppression](../diagnostics/delete.md). Pour les transferts virtuels : [archives](archives.md).
