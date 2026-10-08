# Corrections et optimisations à traiter après le nettoyage

Audit du 7 octobre 2026, base `3cc7d14`, branche `codex/nettoyage-code-mort`. Ces observations proviennent de la revue du code actuel. Les corrections réalisées ensuite sont indiquées dans les entrées concernées. Le nettoyage est documenté séparément dans [AUDIT_CODE_MORT_2.md](AUDIT_CODE_MORT_2.md).

Priorités proposées : P1 pour un risque sur les opérations de fichiers à reproduire en premier, P2 pour les comportements et coûts à examiner ensuite, P3 pour l'entretien. Une chaîne de code établie n'est pas une reproduction utilisateur ; les limites sont indiquées pour chaque entrée.

## A01 — Filtres de date différents des groupes de date — P2

**Corrigé le 7 octobre 2026, après reproduction déterministe.** Un test à 00 h 10 avec un fichier modifié la veille à 23 h 50 échouait avant correction : le filtre Hier l'excluait, alors que le regroupement le classait dans Hier. Les fenêtres glissantes de 24 heures ont été retirées.

Le filtrage de `TabState`, les compteurs du menu, l'indicateur de filtre et Tout sélectionner utilisent désormais `DateGroup`, déjà employé par le regroupement. Les jours suivent le calendrier local, les semaines commencent le lundi, et la priorité des périodes reste Aujourd'hui, Hier, Cette semaine, La semaine dernière, Ce mois-ci, Plus ancien. Ce mois-ci est ajouté au filtre, avec les traductions existantes ; seules les périodes présentes dans le dossier sont proposées. Les dates futures restent dans Aujourd'hui, comme dans le regroupement existant ; les timestamps invalides rejoignent Plus ancien. Une date de référence unique est utilisée pendant chaque parcours du cache ou des compteurs.

Validation : le test de minuit passe après correction. Les fixtures couvrent lundi, frontières de mois et d'année, jours de 23/25 heures avec décalages explicites, conversion du timestamp vers le jour local, dates futures/invalides, compteurs des six périodes, Tout sélectionner, ordre des fichiers et combinaison avec le filtre de nom. Les cas de décalage horaire ne valident pas la base de règles de fuseau du système. Aucun Computer Use ni Linux. Le rafraîchissement d'un filtre déjà ouvert lors d'un changement de jour reste à examiner en A15.

## A02 — Annuler/Rétablir peuvent démarrer plusieurs fois en parallèle — P1

**Corrigé le 7 octobre 2026, après reproduction déterministe.** Avant correction, sélectionner la dernière transaction, ajouter une nouvelle entrée puis valider Annuler fait échouer `complete_undo`. Le test de régression a échoué sur cette assertion avant modification. Deux workers pouvaient également sélectionner la même transaction sans réservation commune.

`HistoryManager::begin_undo` / `begin_redo` réservent désormais une transaction jusqu'à la fin du traitement. Une seconde demande est refusée avant les effets sur les fichiers. Les ajouts d'historique arrivant pendant ce traitement sont différés, puis intégrés dans leur ordre d'arrivée ; une nouvelle action invalide Rétablir après la validation de la transaction réservée. Le verrou du modèle reste libéré pendant les opérations de fichiers. La réservation se libère aussi sur erreur ou panique du worker, et le nettoyage attend sa fin pour préserver les sauvegardes utilisées.

GPUI partage un état d'exécution entre Annuler et Rétablir : les clics et raccourcis répétés sont ignorés pendant le travail, les deux boutons sont désactivés, et l'état est rétabli au retour du worker, y compris sur erreur. Le rejeu de l'ancienne API de restauration avait été adapté, puis cette chaîne sans appelant GPUI a été retirée après revalidation d'A14. La sortie finale du processus et le délai d'arrêt Tokio restent ceux documentés dans `SHUTDOWN_DIAGNOSTICS.md` ; si un worker dépasse ce délai à la fermeture, son nettoyage différé reste soumis à cette limite existante.

Validation : demandes concurrentes sur deux threads, entrées ajoutées pendant Annuler et Rétablir, abandon/erreur/panique, ordre et limite de 100 entrées, nettoyage différé des sauvegardes. Une fixture de renommage vérifie le refus avant mutation, puis l'aller-retour Annuler/Rétablir et la conservation du contenu. Ces contrôles portent sur le modèle et les commandes ; aucune validation interactive GPUI ni Linux.

**Limite de portée :** cette réservation protège l'historique ; elle ne réserve pas les chemins auprès du planificateur de copies/déplacements. La coordination des effets physiques d'une opération de fichiers indépendante avec Annuler/Rétablir reste à examiner séparément. L'observation A14 a ensuite été requalifiée : son API n'avait pas d'appelant GPUI et a été retirée.

## A03 — Marqueur d'onglet potentiellement périmé sur la zone libre — P2

**Corrigé le 7 octobre 2026, après reproduction déterministe du handler.** Une fixture entre panneaux sélectionne une limite d'onglet, puis passe à la zone libre du même panneau. Avant correction, le test obtient l'index 1 au lieu de l'index 3, alors que le dépôt du wrapper insère réellement à la fin.

Le wrapper et les deux moitiés d'onglet mettent désormais à jour la cible dans `on_drag_move`, avant la construction du rendu, au lieu de modifier cet état dans `drag_over`. Le wrapper choisit la fin à chaque événement dans la barre ; la moitié d'onglet survolée affine ensuite cet index. L'ordre de dispatch Capture (parent avant enfants) a été vérifié dans les sources de la révision GPUI utilisée par Cargo (`33c9585`). Les zones d'onglet sont limitées au viewport défilant pour ignorer leurs portions masquées en débordement. Les mouvements sans effet effacent les deux champs de ciblage, et les changements notifient GPUI depuis l'événement.

Validation : quatre tests couvrent limite → zone libre/bouton +, correspondance avec l'insertion réelle entre panneaux et dans un même panneau, priorité des moitiés d'onglet, déplacements sans effet et zones masquées par le défilement. Les marqueurs visuels existants et la position avant le bouton + restent conservés ; aucun nouveau témoin ou changement de curseur. Aucun Computer Use ni validation visuelle interactive. La validation automatique porte sur les calculs et transitions de cible, pas sur un rendu de fenêtre GPUI simulé.

## A04 — Propriétés d'un élément de corbeille : nom et emplacement physiques — P2

**Corrigé le 7 octobre 2026, après reproduction déterministe de la présentation.** Avant correction, une fixture de fichier supprimé affiche `$Rfixture` au lieu de son nom d'origine. Le dialogue affichait également le dossier physique de corbeille sans lire `original_path` et `deleted_time`.

La présentation utilise désormais le nom et le dossier du chemin d'origine, avec les libellés existants Emplacement d'origine et Date de suppression. Les métadonnées partagent les règles de nommage de la liste et de la restauration : lecture du fichier `$I` via Platform et conservation des extensions `.lnk` / `.url` lorsque le Shell les masque. Le chemin physique reste utilisé pour les icônes natives, les calculs de taille et les actions système. En l'absence d'un chemin d'origine absolu exploitable, le dialogue conserve la présentation physique.

La date de suppression du crate `trash`, exprimée en secondes Unix, est convertie en millisecondes comme les autres dates du modèle et le formateur. Les dates négatives ou débordantes ne sont pas publiées comme dates valides. La largeur des libellés tient compte des deux lignes supplémentaires seulement lorsqu'elles sont présentes, avec les traductions existantes dans les cinq langues.

Validation : six nouveaux tests couvrent l'identité affichée, la conservation du chemin physique, dossiers et raccourcis, le repli sans métadonnées, les unités et dates invalides. Une fixture locale Windows `$I` vérifie le nom Unicode d'origine malgré un libellé Shell différent. Aucun élément n'est envoyé à la corbeille par ces tests ; aucune validation visuelle interactive, aucun Computer Use ni Linux.

## A05 — Durée de vie des ressources COM/Shell — P2

**Corrigé le 8 octobre 2026 sur les parcours actifs relus.** Les initialisations ignorées de l'énumération réseau, des propriétés virtuelles, de l'accès rapide et des verbes Shell pouvaient être suivies d'une désinitialisation sans référence COM acquise. Les noms et chemins réseau n'étaient pas libérés ; la valeur native du property store ne recevait pas `PropVariantClear`. Aucun profil mémoire n'a été effectué.

Une garde commune lie chaque initialisation réussie (`S_OK` ou `S_FALSE`) à une désinitialisation sur le même thread. Les traitements qui fonctionnaient déjà dans un appartement existant acceptent explicitement `RPC_E_CHANGED_MODE`, sans acquérir ni libérer de référence. Les autres échecs arrêtent le traitement natif ; les propriétés et l'accès rapide conservent leurs replis existants. Les menus, raccourcis et ouvertures virtuelles gardent leur exigence STA. La garde est déclarée avant les interfaces pour que celles-ci soient détruites avant COM, y compris sur erreur ou panique.

Les chaînes Shell et PIDL possédés sont libérés par `CoTaskMemFree`, les valeurs de propriétés par `PropVariantClear`, les menus par `DestroyMenu`. Les sous-menus et PIDL relatifs empruntés ne reçoivent pas de libération indépendante. Les aperçus vidéo équilibrent également chaque démarrage réussi de Media Foundation après la destruction des lecteurs et échantillons. Les gardes OLE du presse-papiers A18 restent séparées. La sortie finale documentée dans `SHUTDOWN_DIAGNOSTICS.md` est conservée.

Huit régressions vérifient les résultats COM injectés, les références natives imbriquées, un worker MTA conservé après refus STA, les erreurs/paniques, le décodage de chaînes allouées Unicode et invalides, les erreurs des parcours actifs de propriétés/PDF/menu, la destruction des menus natifs et deux conversions indépendantes d'une propriété Unicode réelle. Ces tests ne mesurent pas une fuite mémoire globale et ne valident pas le rendu d'un aperçu PDF/vidéo ou les extensions Shell installées.

Contrats natifs : [CoInitializeEx](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex), [CoUninitialize](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-couninitialize), [IShellItem::GetDisplayName](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitem-getdisplayname).

## A06 — Presse-papiers : allocations et erreurs du format Couper — P2

**Corrigé le 8 octobre 2026 dans le parcours actif de fichiers ordinaires et de texte.** La relecture confirme les sorties sans `GlobalFree` après un verrouillage ou une publication refusés, et les résultats ignorés pour `Preferred DropEffect`. Le parcours de corbeille A18 garde son objet Shell.

Un module Windows privé prépare les deux buffers avant de vider le presse-papiers. Chaque buffer reste possédé par une garde jusqu'à la réussite de `SetClipboardData` ; toute sortie avant cette réussite le libère. L'effet Copier/Couper est publié avant les chemins. Un refus de publication est propagé et le contenu partiel est vidé, sans libérer localement les buffers déjà possédés par Windows. La session est fermée avant de détruire sa fenêtre propriétaire, y compris sur erreur.

L'ouverture en écriture utilise une fenêtre réservée aux messages, créée et détruite sur le thread appelant. L'ancien `OpenClipboard(NULL)` suivi de `EmptyClipboard` laisse un propriétaire nul, ce qui ne respecte pas le contrat de publication documenté par Windows. Aucun rendu visible, aucune modification de curseur ou du parcours GPUI.

Les formats restent CF_HDROP Unicode, Preferred DropEffect (1 pour Copier, 2 pour Couper) et CF_UNICODETEXT. La préparation ou l'ouverture refusée conserve l'ancien contenu. Une publication partiellement acceptée ne permet pas de garantir le retour à l'ancien contenu ; elle est nettoyée et signalée. Les tests injectent les erreurs sans ouvrir le presse-papiers utilisateur. La publication réelle et le collage interactif de fichiers ordinaires restent à confirmer manuellement.

Contrats natifs : [SetClipboardData](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setclipboarddata), [OpenClipboard](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard), [GlobalFree](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-globalfree).

## A07 — API Windows encore dans les commandes — P2

**Partiellement traité le 8 octobre 2026 avec A05.** Les menus natifs, l'énumération/épinglage de l'accès rapide et le nom convivial des propriétés virtuelles passent maintenant par `Platform`, avec leurs implémentations dans des modules Windows privés. Les wrappers publics sans autre appelant des menus dans `commands/system.rs` sont retirés ; la plateforme ne redélègue plus à ces commandes pour ces traitements. Les résultats et replis existants restent conservés, hormis les échecs d'initialisation COM désormais équilibrés.

Restent notamment les appels natifs de vidage/suppression permanente de corbeille dans `commands/ops.rs`, les usages de `trash::os_limited` dans les commandes et les détails de filtrage Windows dans `commands/sidebar.rs`. Déplacer ces responsabilités dans un lot distinct, en revalidant leurs appelants et erreurs. Linux reste hors validation ; ses conditions de compilation et interfaces sont conservées.

## A08 — Annulation de recherche et détection SSD — P2

**Limite structurelle établie, latence non mesurée.** La recherche avancée contrôle son annulation entre les entrées. Les lecteurs de contenu texte et Office (`commands/search.rs:800`, `:886`, `:918`) n'ont pas de paramètre d'annulation pour arrêter une lecture en cours. Un fichier volumineux peut retarder l'arrêt du travail après fermeture/annulation de la recherche.

Propager l'annulation jusqu'aux boucles de lecture et vérifier un arrêt pendant un gros fichier texte et une archive Office. Le seul appel de `file_contains_content` passe aussi `ssd_hint = false` (`:200`) : la branche permettant d'éviter le verrou de disque sur SSD n'est jamais choisie. Décider de raccorder une détection effective ou de simplifier ce paramètre ; ne pas activer la concurrence SSD sans mesure.

## A09 — Calcul des tailles : tâches et pools cumulés — P2

**Organisation établie, coût à mesurer.** `ui/app/sizes.rs:26` crée un travail bloquant pour chaque chemin à calculer. `commands/io.rs:603` crée un pool jwalk par calcul, avec davantage de threads sur SSD. Le parcours ne reçoit pas de signal d'annulation interne ; abandonner la tâche GPUI ne garantit pas l'arrêt des travaux bloquants déjà lancés.

Mesurer un dossier contenant beaucoup de sous-dossiers, puis une navigation rapide entre dossiers. Envisager une limite globale de concurrence, un pool partagé et une annulation propagée. Garder les calculs hors du thread UI et préserver le cache et les mises à jour des résultats.

## A10 — Tri et normalisation répétés — P2

**Calculs répétés établis, gain à mesurer.** `commands/io.rs:140` trie par nom naturel ; `ui/workspace.rs` retrie les listes et normalise les noms dans les comparateurs/filtres. La complétion effectue également son propre tri. Les règles de comparaison ne sont pas toutes identiques.

Clarifier les règles attendues pour noms numériques, accents, dossiers et ordre descendant avant factorisation. Préparer les clés normalisées une fois par entrée/filtre, et éviter les tris répétés lorsqu'un consommateur partage réellement le même ordre. Le tri backend reste utile au fil d'Ariane et ne doit pas être supprimé aveuglément.

## A11 — Copie de l'historique pendant le rendu — P2

**Coût structurel établi, impact non mesuré.** `ui/components/top_bar.rs:121` appelle `HistoryManager::get_state`, qui clone les piles de transactions et leurs détails (`models/history.rs:102`). Le rendu peut être répété pour chaque panneau, alors que les boutons n'ont besoin que d'un résumé de l'état et des transactions disponibles.

Exposer un petit état de présentation plutôt que recopier les chemins et journaux de fichiers de tout l'historique. Mesurer le rendu avec un historique chargé et plusieurs panneaux ; conserver un accès de test à l'état détaillé si nécessaire.

## A12 — Polling et notifications sans changement — P2

**Parcours établis, fréquence utile à mesurer.** `ui/app/shell_features.rs` réévalue régulièrement Peek et notifie l'interface après chaque résultat, même identique. La surveillance du presse-papiers (`ui/app/mod.rs:2268`) lit périodiquement ses chemins pour déduire la disponibilité du collage.

Notifier uniquement lors d'un changement pertinent. Examiner un indicateur natif de présence de fichiers ou de changement du presse-papiers pour éviter la lecture répétée complète, en gardant les formats nécessaires aux actions actuelles. Comparer coûts et réactivité avant remplacement du polling.

## A13 — Suggestions Clippy et simplifications locales — P3

`cargo clippy --all-targets --offline --message-format=json` termine avec succès. Ses diagnostics comprennent des doublons entre cibles et des suggestions concernant uniquement les tests ; ils ne signalent pas tous du code mort ni des défauts.

Exemples à revoir après les retraits : emprunts inutiles dans `commands/duplicates.rs` et `conflict_dialog.rs`, copies clonées dans `file_list.rs` et `tooltip.rs`, comparaisons avec allocation dans `commands/ops.rs`, casts superflus dans `ui/app/sizes.rs`, remplacement de caractères dans `input.rs`, organisation de modules de tests. La branche de couleur identique des onglets est déjà recensée en N05.

Traiter les simplifications utiles dans des changements limités ; ne pas convertir automatiquement tous les diagnostics ni modifier les tests uniquement pour satisfaire une règle stylistique.

## A14 — Rétablir une restauration déplacée depuis la corbeille — P2

**Requalifié le 7 octobre 2026 : chaîne sans appelant dans l'application, retirée après A14.** La relecture des appelants établit que `move_from_trash` était invoqué uniquement par le test natif ajouté en A14. GPUI utilise `execute_coller` → `execute_file_move` → le gestionnaire `file_ops`, qui ne produit aucune transaction Restore. L'action Restaurer passe directement par `restore_items`.

Le commit A14 `b8c7523` validait le rejeu de cette API isolée, pas une fonctionnalité raccordée à GPUI. La variante Restore, `move_from_trash`, le journal et ses tests dédiés, ses nouvelles méthodes Platform et son libellé traduit d'historique sont donc retirés dans le lot suivant. Les tests et gardes d'A02 pour les opérations actives restent conservés. La restauration ordinaire et ses deux fixtures natives restent actives.

L'affirmation initiale d'un défaut Annuler/Rétablir de restauration dans l'application était incorrecte. Le parcours réel de Couper/Coller depuis la corbeille fait l'objet de l'observation distincte A18 ; aucune nouvelle intégration de l'ancienne API n'est ajoutée implicitement.

## A15 — Cache de filtrage lors d'un changement de jour — P2

**Corrigé le 7 octobre 2026, avec validation déterministe.** Le cache de chaque onglet mémorise désormais le jour local, le décalage UTC et la signature des règles du fuseau Windows utilisés lors de sa construction. Une tâche dédiée vérifie cette référence toutes les cinq secondes, indépendamment du watcher de fichiers et du presse-papiers, et prépare uniquement les caches avec un filtre Date actif et une référence différente.

Les règles de filtrage sont partagées avec le calcul ordinaire du cache. Le recalcul calendaire s'effectue sur un worker à partir d'un snapshot des fichiers et critères ; chaque réponse conserve l'identifiant de l'onglet et la révision de son cache. Une modification de filtre, de tri, une navigation ou une réception de fichiers invalide les anciennes réponses. Un changement de référence pendant le calcul fait aussi abandonner les résultats. Les onglets inactifs et ceux déplacés entre panneaux sont traités, les onglets fermés sont ignorés.

Les fichiers ne sont pas relus, et les filtres, sélection, position de défilement et mesures des colonnes sont conservés. Les onglets regroupés sans filtre Date demandent uniquement un nouveau rendu lorsque la référence change. Aucune notification périodique si la référence et les caches sont déjà à jour. Un retour de veille ou un saut d'horloge est pris en compte à la prochaine vérification ; la conversion des dates continue d'utiliser Chrono et le fuseau natif. Les menus reprennent leurs compteurs lors de leur ouverture, comme auparavant.

Validation : six fixtures déterministes couvrant minuit dans plusieurs panneaux et onglets actifs/inactifs, sauts d'horloge, changements de décalage et de signature de fuseau, résultats obsolètes, fermeture/déplacement d'onglet, conservation de l'état et absence de recalcul sans filtre Date. Aucun Computer Use ni modification de l'horloge ou du fuseau de la machine ; Linux reste hors validation.

## A16 — Conflits et espace temporaire du déplacement initial depuis la corbeille — P1

**Clos par suppression de la chaîne inutilisée, après revalidation des appelants.** Les conflits et sauvegardes partagées recensés appartenaient à `move_from_trash`, sans appelant de production GPUI. La fonction et sa chaîne Restore sont retirées avec la correction de qualification A14. Il n'y a donc pas de nouvel espace temporaire ou de nouveau dialogue de conflit à implémenter pour cette API. Le parcours actif de Couper/Coller doit être examiné séparément en A18.

## A17 — Énumérations de corbeille lors du rejeu des restaurations — P2

**Clos par retrait du journal A14 inutilisé.** Les énumérations par effet appartenaient au journal d'une API sans appelant de l'application. Ce journal a été supprimé ; aucun benchmark ou optimisation n'est nécessaire pour ce parcours retiré.

## A18 — Couper/Coller depuis la corbeille dans le parcours GPUI actuel — P1

**Corrigé le 7 octobre 2026 dans le parcours actif.** Une fixture native a reproduit le déplacement sous le nom `$R…` dans `FileOperationManager::perform_copy`, appelé par la file d'opérations utilisée par GPUI. La correction ne réintroduit pas l'ancienne commande `move_from_trash`.

La détection native vérifie l'appartenance à la corbeille et lit le nom original du fichier `$I`, par lot de sources et en arrière-plan. Le plan de copie/déplacement et les vérifications de conflits, y compris après attente dans la file, utilisent ce nom. Les clés de résolution et les chemins sources restent physiques. Deux éléments sélectionnés portant le même nom original sont refusés avant mutation et peuvent être transférés séparément.

Un déplacement complet conserve ses métadonnées dans une sauvegarde cachée sur le volume source, ajoutée au journal Move existant. Annuler remet les métadonnées et les données à leur emplacement de corbeille ; Rétablir rejoue le même journal. Une sauvegarde manquante bloque Annuler avant mutation. Les éléments ignorés et les dossiers partiellement déplacés gardent leur `$I` actif. La finalisation traite aussi les éléments entièrement déplacés avant une erreur ou une annulation ultérieure. Les sauvegardes expirent avec l'historique. Les transferts vers une archive gardent leur politique actuelle sans historique et ne conservent pas de sauvegarde de métadonnées après publication.

Le presse-papiers actif utilise désormais l'objet de données natif du dossier Corbeille pour ses éléments. Il publie l'identité Shell et l'intention Copier/Couper avec `OleSetClipboard`, puis rend les formats avec `OleFlushClipboard` sur un thread STA dédié. Les gardes libèrent les PIDL, les textes et les buffers en cas d'erreur ; les interfaces COM quittent leur portée avant la désinitialisation OLE. Les fichiers ordinaires utilisent toujours le parcours existant. Le format CF_HDROP conserve les chemins physiques nécessaires au collage dans Oxyde.

La comparaison avec le projet Tauri local `Biluf_FM_Clean` confirme ses appelants : `src/actions/fileActions.ts:152` publiait un objet Shell pour Couper, et `:191` redirigeait le collage vers `move_from_trash`. GPUI avait perdu ce branchement. Le principe du presse-papiers est repris ; le détour Tauri par l'emplacement original, ses écrasements et ses sauvegardes temporaires communes ne sont pas repris.

Les cycles natifs ont aussi révélé que `create_dir_all_tracked` tentait de créer le préfixe de lecteur d'un chemin Windows canonique `\\?\…` lors du redo de dossiers. Il remonte maintenant aux parents existants et ne suit que les dossiers réellement créés.

Validation : fichiers Unicode, `.lnk`, `.url`, dossiers avec contenu et dossiers vides ; copie et déplacement par la file active ; conflits et déplacements partiels ; sauvegarde absente et conflit au redo ; deux cycles Annuler/Rétablir ; copie et déplacement dans une archive temporaire. Le consommateur natif `IFileOperation` vérifie les noms obtenus depuis des formats Shell rendus puis détachés de l'objet source, ainsi que les chemins physiques CF_HDROP. Aucun remplacement du presse-papiers utilisateur pendant les tests. La publication réelle et le collage interactif dans l'Explorateur restent à confirmer manuellement. Le test de cible de dépôt qui bloquait a été arrêté et n'est pas conservé ; ses fixtures identifiées ont été récupérées. Aucun Computer Use ni validation Linux.

Références natives : [transfert Shell par presse-papiers](https://learn.microsoft.com/en-us/windows/win32/shell/dragdrop), [IFileOperation::MoveItems](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-moveitems).

## A19 — Annuler/Rétablir un déplacement entre volumes — P1

**Corrigé le 8 octobre 2026 après reproduction native entre C: et D:.** Avant correction, un dossier déplacé par la file active provoque l'erreur Windows 17 dès Annuler : le journal et son rollback utilisaient `std::fs::rename`. La fixture du parcours GPUI/backend actif échoue avant modification, puis passe deux cycles Annuler/Rétablir après correction.

Le rejeu des fichiers Move et son rollback passent par un transfert commun. Sous Windows, la source est ouverte avec accès Lecture/Suppression, partage Lecture et ouverture du point de réanalyse lui-même. La garde bloque les écritures et remplacements concurrents du fichier principal. Un renommage du fichier ouvert refuse tout écrasement. Seul le refus natif `CrossesDevices` déclenche une copie dans un dossier temporaire caché sur le volume cible ; taille et hash BLAKE3 du contenu principal sont vérifiés avant publication exclusive. La source ouverte n'est marquée pour suppression qu'après publication réussie, puis retirée à la fermeture de sa garde.

La copie utilise `std::fs::copy`, qui appelle la copie native sous Windows. Un test distinct vérifie les flux NTFS et la date de modification lors de l'aller-retour du transfert A19. Cela ne valide pas leur conservation par la copie initiale de la file active, qui possède une autre implémentation (voir A28). Les fichiers symboliques et points de réanalyse sont refusés avant de les suivre. Un fichier utilisé par un writer incompatible est refusé ; aucun déverrouillage forcé. Le nettoyage d'une copie temporaire en lecture seule rend uniquement cette copie possédée modifiable, sans changer les attributs de la source.

Une copie partielle, corrompue ou une destination apparue avant publication laisse la source intacte. Si la suppression de source est refusée, la copie publiée possédée est retirée ; un échec de ce retrait est signalé avec son chemin. Les fichiers déjà transférés sont ramenés en sens inverse avec le même helper lorsqu'un fichier suivant échoue. Le test natif bloque le deuxième fichier au redo, vérifie le rollback entre volumes et la conservation de l'entrée d'historique, puis réussit après libération du writer.

Huit tests automatiques couvrent le transfert forcé entre volumes, les erreurs de copie, la corruption, le conflit de publication, le refus de suppression, les attributs en lecture seule, un hash attendu périmé, l'identité conservée sur un volume et le verrou natif. Deux tests natifs exécutés explicitement couvrent la file active et le transfert avec flux/dates entre C: et D:. Un contrôle a montré que `CopyFile` refuse un fichier cible caché déjà existant : le staging est donc placé dans un dossier caché, sans imposer cet attribut au fichier cible. Les assertions d'erreurs injectées vérifient le motif effectivement atteint.

Limites : pas de validation Linux, réseau/SMB, crash ou coupure d'alimentation. Les API Windows sont privées derrière `Platform`. Les phases ultérieures de dossiers/remplacements de l'Undo ne deviennent pas transactionnelles dans ce lot (A27). Les hashes du journal portent sur le contenu principal ; ils ne certifient pas les métadonnées, ACL ou l'absence d'une modification concurrente d'un flux alternatif.

Contrats natifs : [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info), [SetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle), [copie Rust](https://doc.rust-lang.org/std/fs/fn.copy.html).

## A20 — Métadonnées de corbeille après un déplacement natif Shell — P2

**Observation native, sans conclusion sur l'annulation de l'Explorateur.** Le consommateur Shell utilisé dans le test A18 conserve les noms et retire les données de la corbeille, mais les fichiers `$I` restent présents. L'absence de `$R` retire bien les éléments de l'énumération. Vérifier le comportement du collage réel et de l'historique Windows avant de modifier ou purger ces métadonnées ; aucune purge automatique de métadonnées utilisateur n'est ajoutée. Les sidecars capturés pour les fixtures du test sont nettoyés séparément.

## A21 — Historique de suppression dans la référence Tauri — observation de comparaison

L'utilisateur confirme que son application Tauri annule une suppression, puis que le redo disparaît après une première tentative sans erreur affichée. La lecture montre une branche Delete prévue pour supprimer à nouveau. La disparition sans message est compatible avec une erreur masquée suivie de la perte de la transaction ; la cause native de l'échec n'est pas établie. Le hook React `useFileOperations` n'affiche pas les erreurs de redo, et l'ancien backend retire la transaction de sa pile avant l'opération : un échec peut donc perdre l'entrée. Le journal GPUI utilise déjà la réservation A02, qui garde les entrées en cas d'échec.

Un défaut distinct est établi pour l'ancienne transaction Restore de Tauri : `move_from_trash` enregistre `paths: vec![]`, puis le redo réutilise cette liste vide. Cette observation ne doit pas être confondue avec le refus de redo Delete signalé par l'utilisateur. Le projet Tauri est consulté en lecture seule. Le cycle Delete natif de GPUI réussit deux tours Supprimer → Annuler → Rétablir dans ce lot, avec les noms Unicode d'origine.

## A22 — Plafond arbitraire du redo de suppression — P1

**Corrigé le 7 octobre 2026, puis confirmé par le test utilisateur.** Le toast `TrashFull("Items potentially too large for Recycle Bin.")` venait de `fast_trash`, et non d'un refus de Windows. Cette ancienne fonction refusait plus de 4 Gio ou plus de 1 000 chemins ; la suppression initiale ne contenait pas ces limites. Les cycles natifs A18 précédents ne couvraient que de petits fichiers.

Le rejeu Delete et l'annulation NewFolder utilisent désormais `Platform::shell_delete` avec la même politique que la file de suppression active. Le duplicata des appels natifs et ses estimations de taille sont retirés. La décision appartient au système ; une annulation native est également vérifiée par ce parcours commun. Le journal A02 conserve l'entrée si le système refuse réellement l'opération.

Deux régressions natives ciblent les anciens seuils : un fichier creux de 4 Gio + 1 octet et 1 001 petits fichiers, avec Supprimer → Annuler → Rétablir → Annuler et contrôle du nom et du contenu. Les fixtures sont générées puis récupérées ; le test de grand fichier exige une marge de capacité avant mutation et ne change pas la configuration de corbeille. Les deux tests natifs exécutés explicitement passent. Le fichier de 4 Gio + 1 octet conserve sa taille et ses marqueurs de contenu ; les 1 001 fichiers conservent leurs noms et leur contenu après les deux restaurations.

## A23 — Lecture non bornée des buffers de presse-papiers — P2

**Corrigé le 8 octobre 2026 dans le lecteur actif de `Platform::get_clipboard_files`.** La relecture des appelants confirme que Coller et le watcher GPUI utilisent ce lecteur en arrière-plan. Il interprétait l'en-tête CF_HDROP, son décalage et ses chaînes sans vérifier la taille du bloc ; l'effet Couper était déréférencé sans vérifier la présence de quatre octets. Aucun crash utilisateur n'a été reproduit.

La taille native est vérifiée avant de verrouiller le bloc. Le parseur n'accède qu'à cette plage de mémoire, valide l'en-tête et le décalage puis recherche les deux terminateurs dans les octets disponibles. Il refuse une liste tronquée ou un nom UTF-16 mal formé en entier, avant de retourner des chemins au collage. Un effet présent mais illisible ou tronqué est une erreur, au lieu d'être interprété comme Copier. Un effet absent garde le comportement Copier.

Les chaînes et champs sont décodés depuis des octets, sans déréférencement aligné supposé. Les chemins Unicode et réseau restent pris en charge, les listes ANSI restent ignorées comme auparavant, et le padding inutilisé après le terminateur reste autorisé car `GlobalSize` peut dépasser la taille demandée. Le presse-papiers occupé garde son repli vide.

Des gardes déverrouillent les blocs puis ferment la session sur toute sortie. Les allocations empruntées au système ne sont jamais libérées par le lecteur ; les noms retournés sont des chaînes Rust indépendantes. Les fixtures de validation ne lisent ni ne remplacent le presse-papiers utilisateur.

Contrats natifs : [GetClipboardData](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboarddata), [GlobalSize](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-globalsize), [DROPFILES](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/ns-shlobj_core-dropfiles).

## A24 — Erreurs de menus et d'épinglage Shell ignorées — P2

**Points de code établis pendant A05, sans reproduction utilisateur.** Les fonctions actives de `platform/windows_shell.rs` ignorent le résultat de `QueryContextMenu` ; `execute_native_menu_item` ignore aussi celui de `InvokeCommand`. Le repli PowerShell de désépinglage d'un chemin absent retourne une réussite sans vérifier le résultat du processus. Une erreur native peut donc être présentée comme une réussite. Propager les erreurs utiles et distinguer une commande absente d'un échec d'exécution dans un lot dédié ; ne pas modifier les épingles utilisateur pour les tests.

## A25 — Lecture de verbes Shell hors du buffer si terminateur absent — P2

**Point de code établi, sans crash reproduit.** Les trois lectures `GetCommandString` de `platform/windows_shell.rs` convertissent un tableau de 128 octets via `CStr::from_ptr`. Si une extension remplit le tableau sans terminateur, la conversion cherche au-delà de la mémoire disponible. Borne explicite et politique de refus à introduire séparément, avec fixture de réponse tronquée ; les conversions restent inchangées dans A05.

## A26 — Échecs du lecteur vidéo masqués — P2

**Points de code établis, sans reproduction native.** `platform/thumbnails.rs::stream_video_frames_sync` abandonne la boucle sur certains échecs de création/type puis retourne `Ok(())`. Une erreur de lecture termine seulement la boucle interne : le lecteur est relancé après 200 ms. Si aucune image n'arrive, le callback ne peut pas signaler l'arrêt. Examiner la propagation des erreurs et l'annulation indépendante des images dans un lot distinct ; A05 équilibre seulement les ressources COM/Media Foundation.

## A27 — Phases d'Undo Move après le transfert des fichiers — P1

**Corrigé.** Une fixture reproduit l'échec de création d'un dossier après restauration d'un fichier : avant correction, le fichier reste à la source malgré l'échec d'Annuler et la conservation de l'entrée d'historique. Un dossier source remplacé par un fichier était aussi accepté, et une destination déjà absente ignorée.

`systems/history_move_undo.rs` valide les fichiers et leur contenu avant mutation, puis conserve le journal des transferts, des parents créés et des dossiers retirés jusqu'à la fin. Les sorties copiées dont la source n'a pas été supprimée sont mises en attente dans un dossier caché du volume de destination, hors des dossiers à retirer. Elles ne sont purgées qu'après réussite des phases réversibles. Une erreur ou un unwind rejoue les effets en sens inverse, y compris avec le transfert entre volumes A19. Les fichiers extérieurs et les dossiers devenus non vides sont préservés ; l'historique reste réessayable après un rollback réussi.

Si un conflit extérieur empêche le rollback, les données restent à leur emplacement courant ou en attente et l'erreur indique les chemins concernés. Aucun nettoyage récursif des dossiers d'attente ne peut supprimer ces données. Une purge finale impossible conserve la sortie en attente avec un avertissement dans les logs ; elle ne transforme pas un Undo déjà terminé en échec après une suppression irréversible.

Limites : ce journal en mémoire ne garantit pas une transaction de système de fichiers face à un crash, une coupure ou toute mutation extérieure concurrente. Les dossiers recréés retrouvent leurs permissions de base et leur attribut caché ; leur identité, leurs dates et leurs ACL spécifiques ne sont pas restaurées. Le contenu principal est vérifié par hash, avec les limites des flux alternatifs décrites dans A19. A28 reste distinct pour la copie initiale.

La chaîne de sauvegardes de remplacements évoquée initialement est inactive et retirée après vérification de ses producteurs (A29).

## A28 — Métadonnées et flux natifs absents de la copie initiale — P1

**Point de code établi pendant A19, sans reproduction native dédiée de la file initiale.** Le worker de `systems/file_ops.rs::perform_copy` crée un fichier de destination et copie le contenu principal via `Read`/`Write`. Il ne recopie pas les flux NTFS, dates ou attributs de la source. Cela concerne Copier et les déplacements initiaux qui nécessitent une copie ; les déplacements sur un volume par renommage conservent leur identité. Le journal ne peut pas restaurer des données absentes de la copie initiale. Étudier une copie native avec conservation des métadonnées, progression et annulation, en gardant les politiques de conflits et de fichiers partiels. Le transfert A19 utilise déjà la copie native, mais ne corrige pas ce worker initial.

## A29 — Ancien journal de sauvegardes de remplacement — code mort retiré

`replaced_files` n'était alimenté par aucun parcours de production : les deux constructeurs l'initialisaient vide et seuls des tests synthétiques créaient des sauvegardes. Le champ, sa propagation dans l'historique, les restaurations et la suppression des sauvegardes sont retirés. Les tests de réservation et de nettoyage portent désormais sur les sauvegardes de métadonnées de corbeille réellement produites.

Les écrasements actifs restent suivis par `overwritten_files` : une copie ayant écrasé un fichier bloque Annuler, et le redo de remplacement demande une nouvelle résolution de conflit. Aucune sauvegarde des anciens contenus n'est ajoutée.

## A30 — Annuler une copie peut laisser une suppression partielle — P1

**Point de code établi pendant A27, sans nouvelle reproduction dédiée.** Le parcours Copy d'`undo_last_action` vérifie les sorties, puis `remove_undo_copy_files` les supprime l'une après l'autre avant le retrait des dossiers. Une erreur sur un fichier ultérieur ou un dossier conserve l'entrée d'historique alors que les premières sorties sont déjà supprimées ; la reprise peut alors échouer sur une sortie absente. La réservation A02 ne rend pas ces effets réversibles. Examiner une mise en attente des sorties et des erreurs injectées dans un lot distinct, comme pour les phases Move A27. Ce parcours Copy reste inchangé dans A27, hors retrait de la branche de sauvegardes inactive A29.

## Validation et suite

La compilation Windows actuelle passe sans avertissement du projet. Les observations ci-dessus sont issues de la lecture des sources et de Clippy ; aucun nouveau test de comportement ou benchmark n'a été exécuté pendant cet audit. Snap Layouts et le témoin de déplacement actuel sont confirmés par l'utilisateur. Les validations des lots précédents restent décrites dans [AUDIT_CODE_MORT.md](AUDIT_CODE_MORT.md).

Après A01/A02/A03/A04/A05/A06/A15/A19/A23/A27, le raccordement A18 et le retrait A29, examiner A28 et A30 ; conserver A20 pour la validation native interactive. Les optimisations demandent des mesures avant modification. Cet ordre est proposé, pas une liste de corrections déjà autorisées ou réalisées.

### État après les lots N01 à N16

Le nettoyage retire les anciennes sérialisations, les estimations et plusieurs lectures natives dont les résultats étaient abandonnés. Ce nettoyage ne corrige pas les observations A01–A13. A01, A02, A03, A04 et A15 sont traités dans les lots suivants ; A14/A16/A17 ont été requalifiés après relecture des appelants, puis leur chaîne inutilisée a été retirée. Ces suites sont décrites plus haut. Les références de lignes ci-dessus restent celles de `3cc7d14` ; utiliser les noms de fonctions pour retrouver leur nouvel emplacement.

Depuis A04, `FileProperties::original_path` / `deleted_time` et leur collecte alimentent les propriétés des éléments de corbeille. Pour A05, l'énumération de diagnostic du property store et les lectures de métadonnées d'appareil sans affichage disparaissent ; la récupération du nom convivial et son traitement COM restent actifs. Pour A10, les variantes de tri backend inaccessibles disparaissent, mais le tri naturel actif et les tris GPUI restent distincts. Pour A13, la branche de couleur identique des onglets disparaît avec N05 ; les autres suggestions ne font pas l'objet d'une correction générale.

### Validation du lot A02

`cargo test --all-targets --offline` : 101 tests backend et 89 tests du binaire passent, soit 190 ; les trois tests natifs déjà ignorés restent ignorés. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Neuf tests de régression ont été ajoutés, et le test existant de déplacement de l'historique a été adapté à la réservation. Aucun Computer Use et aucune validation Linux.

### Validation du lot A01

`cargo test --bin oxyde --offline` : 95 tests passent, dont six nouveaux tests de régression. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Les validations portent sur les règles de calcul et les parcours de filtrage ; aucune validation visuelle interactive, aucun Computer Use ni Linux.

### Validation du lot A03

`cargo test --bin oxyde --offline` : 99 tests passent, dont quatre nouveaux tests de régression. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Aucun Computer Use ni test visuel de fenêtre GPUI ; Linux reste hors validation.

### Validation du lot A04

`cargo test --all-targets --offline` : 104 tests backend et 102 tests du binaire passent, soit 206 ; les trois tests natifs déjà ignorés restent ignorés. Six tests de régression ont été ajoutés. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Les traductions des deux libellés existent dans les cinq catalogues. Aucun Computer Use, aucune validation visuelle interactive ni Linux.

### Validation historique du lot A14 — API isolée, retirée ensuite

Ces résultats valident l’API isolée testée dans `b8c7523`, sans prouver son usage par GPUI. Ses huit tests dédiés sont retirés avec cette chaîne ; les trois tests natifs antérieurs restent conservés.

`cargo test --all-targets --offline` : 111 tests backend et 102 tests du binaire passent, soit 213. Quatre tests natifs restent ignorés dans cette invocation ; le nouveau test `native_restore_history_roundtrip_and_conflict` est exécuté explicitement avec `--ignored` et passe, soit 214 tests distincts validés dans le lot. Les trois anciens tests natifs ignorés n'ont pas été relancés. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. Aucun Computer Use ni Linux ; aucun fichier utilisateur n'est utilisé par les fixtures natives.

### Validation du retrait de la chaîne Restore et du lot A15

`cargo test --all-targets --offline` : 104 tests backend et 108 tests du binaire passent, soit 212 ; les trois tests natifs antérieurs restent ignorés. Les huit tests du journal A14 isolé disparaissent avec sa chaîne, et six tests A15 sont ajoutés. Les tests des catalogues et de l'historique actif passent. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` réussit. La vérification des appelants confirme que la tâche A15 est démarrée par `start_watcher_task`, appelé à l'initialisation de la fenêtre principale. Aucun Computer Use ni validation Linux, aucune modification du fuseau ou de l'horloge système.

### Validation du lot A18

`cargo test --all-targets --offline` : 106 tests backend et 108 tests du binaire passent, soit 214. Deux tests automatiques couvrent les dossiers canoniques et l'expiration des métadonnées. Sept scénarios natifs exécutés explicitement passent : les deux restaurations natives antérieures, le consommateur Shell, la file Copier/Déplacer, les conflits, les archives et deux cycles Delete Annuler/Rétablir. Cela représente 221 tests distincts validés. Les tests natifs restent ignorés par défaut car ils utilisent uniquement des fixtures générées envoyées à la vraie corbeille Windows.

`cargo check --all-targets --offline` passe sans avertissement du projet et `cargo build --bin oxyde --offline` réussit. Aucun Computer Use, aucune validation Linux ; le presse-papiers utilisateur n'est pas remplacé par les tests.

### Validation de la correction A22

Les deux tests `native_delete_redo_above_old_size_limit` et `native_delete_redo_above_old_item_limit` passent avec `cargo test --lib --offline native_delete_redo_above_old_ -- --ignored --test-threads=1`. Ils utilisent uniquement des fixtures générées ; le grand fichier est creux et la capacité de la corbeille est vérifiée avant mutation. Les fichiers sont restaurés puis retirés avec leur dossier temporaire. Aucun Computer Use ni modification de la configuration système.

`cargo test --all-targets --offline` : 106 tests backend et 108 tests du binaire passent (214), avec 10 tests natifs ignorés par défaut. Les deux régressions natives A22 passent séparément. `cargo check --all-targets --offline` passe sans avertissement du projet.

Le lien du nouveau binaire est produit dans `target/debug/deps/oxyde.exe`, mais `cargo build --bin oxyde --offline` échoue ensuite à remplacer `target/debug/oxyde.exe` avec « Accès refusé ». Le nouveau binaire est copié sous `target/debug/oxyde-corrige.exe` pour permettre le test sans arrêter l’application ouverte. Le remplacement du nom habituel reste à refaire après fermeture.

### Validation du lot A06

Sept tests ciblés passent : injection d'erreurs dans les parcours Copier/Couper et texte, transfert de propriété après réussite, nettoyage partiel, conservation de l'ancien contenu avant publication, vidage sans allocation, texte Unicode, lecture native CF_HDROP Unicode/UNC et durée de vie de la fenêtre propriétaire invisible. Les échecs de fermeture sont aussi injectés. Aucun test n'ouvre le presse-papiers utilisateur ; aucun Computer Use ni validation Linux. La publication réelle et le collage interactif restent à confirmer manuellement.

`cargo test --all-targets --offline` : 113 tests backend et 108 tests du binaire passent, soit 221 ; les 10 tests natifs déjà ignorés restent ignorés. Les sept nouveaux tests A06 sont inclus dans cette invocation. `cargo check --all-targets --offline` passe sans avertissement du projet.

`cargo build --bin oxyde --offline` réussit ; le binaire habituel `target/debug/oxyde.exe` est reconstruit. Le nouveau lot A06 est séparé du commit corbeille `f081c4d`.

### Validation du lot A23

Sept tests ciblés passent : chaque préfixe tronqué d'une liste valide, décalages dans l'en-tête ou hors du bloc, Unicode/UNC avec décalage non aligné, padding inutilisé, UTF-16 invalide après un premier chemin correct, liste vide et politique ANSI conservée, effets tronqués et valeurs nominales. Le test natif vérifie le compteur de verrous avant/après un décodage réussi ou refusé, et la conservation de l'allocation empruntée. Une assertion de libération de la fixture a été ajustée au retour nul de réussite de `GlobalFree`, que le binding Rust représente comme `Err(S_OK)`. Aucun accès au presse-papiers utilisateur, aucun Computer Use ni validation Linux.

`cargo test --all-targets --offline` : 120 tests backend et 108 tests du binaire passent, soit 228 ; les 10 tests natifs ignorés restent ignorés. Les sept tests A23 sont inclus. `cargo check --all-targets --offline` passe sans avertissement du projet.

`cargo build --bin oxyde --offline` réussit ; `target/debug/oxyde.exe` est reconstruit. Le lot A23 reste séparé du commit A06 `03d90e6`.

### Validation du lot A05 et du déplacement partiel A07

`cargo test --all-targets --offline` : 128 tests backend et 108 tests du binaire passent, soit 236 ; les 10 tests natifs ignorés restent ignorés. Les huit nouvelles régressions A05 sont incluses et ont aussi été exécutées dans le filtre des modules Windows. Le test natif existant de modification des raccourcis passe avec la garde commune. Les fixtures ne modifient ni les épingles, ni le presse-papiers utilisateur, ni les fichiers utilisateur.

`cargo check --all-targets --offline` passe sans avertissement du projet ; le formatage des trois nouveaux modules et `git diff --check` sont vérifiés. Aucun Computer Use ni validation Linux. La lecture réseau réelle, les extensions de menus, les aperçus PDF/vidéo et leur fermeture restent à confirmer manuellement ; le contournement de sortie finale du processus est conservé.

`cargo build --bin oxyde --offline` réussit ; `target/debug/oxyde.exe` est reconstruit. Le lot A05/A07 reste séparé du commit A23 `f555f30`.

### Validation du lot A19

La fixture native de la file active échoue avant correction avec l'erreur Windows 17 au premier Annuler. Après correction, elle passe deux cycles entre C: et D:, ainsi qu'un échec sur le deuxième fichier suivi d'un rollback entre volumes et d'un redo réussi après libération du writer. Le test distinct de transfert conserve le flux principal, un flux NTFS alternatif et la date de modification à l'aller et au retour ; la visibilité finale reste celle de la source. Le contrôle natif existant `native_trash_transfer_names_metadata_and_history_roundtrip` passe aussi. Tous utilisent seulement des fixtures générées, récupérées ou nettoyées à la fin.

`cargo test --all-targets --offline` : 136 tests backend et 108 tests du binaire passent, soit 244 ; 12 tests natifs restent ignorés par défaut. Les huit nouveaux tests automatiques A19 sont inclus. Les deux nouveaux tests natifs et le scénario de corbeille sont exécutés explicitement séparément, soit 247 tests distincts validés dans le lot. Les assertions d'erreurs injectées vérifient leur motif pour éviter qu'un refus plus précoce masque le parcours réellement visé.

`cargo check --all-targets --offline` passe sans avertissement du projet. Les deux nouveaux modules sont formatés et `git diff --check` passe. Les appelants GPUI Annuler/Rétablir exécutent toujours les commandes dans `spawn_blocking` ; les copies et hashes restent en arrière-plan. Aucun Computer Use ni validation Linux, aucune modification de volume, de montage ou de configuration système. Les interfaces de plateforme sont maintenues ; les garanties de verrouillage testées sont celles de Windows.

`cargo build --bin oxyde --offline` réussit ; `target/debug/oxyde.exe` est reconstruit. Le lot A19 est commité dans `c2f92b5`, après le commit COM/Shell `f0c39fe`.

### Validation du lot A27 et du retrait A29

Les trois régressions des commandes couvrent l'échec tardif de création d'un dossier, le remplacement d'un dossier par un fichier extérieur et une destination absente, avec conservation de l'historique et reprise. Sept tests du nouveau journal couvrent la création partielle des parents, la suppression d'un dossier suivie d'un échec, la restauration d'une sortie mise en attente, un unwind, un conflit de rollback avec rétention des données, une sortie modifiée ou sans hash et la préservation du contenu extérieur. Le test de copie de remplacement synthétique sans producteur actif est retiré ; les tests de réservation et de nettoyage des sauvegardes restent conservés sur les métadonnées de corbeille.

`cargo test --all-targets --offline` : 145 tests backend et 108 tests du binaire passent, soit 253 ; les 12 tests natifs restent ignorés par défaut. La fixture native de la file active entre C: et D: passe après ajout d'un échec tardif d'Undo : les fichiers reviennent à destination, le fichier extérieur est préservé et l'Undo peut être réessayé avant deux cycles Annuler/Rétablir. Le test natif de flux NTFS et date de modification passe aussi. Le scénario actif de corbeille est vérifié séparément. Tous utilisent des fixtures générées.

Ces trois scénarios natifs portent le total à 256 tests distincts validés pour ce lot. `cargo check --all-targets --offline` passe sans avertissement du projet, et `cargo build --bin oxyde --offline` reconstruit `target/debug/oxyde.exe`. Le nouveau module est formaté et `git diff --check` passe. Les commandes GPUI restent exécutées dans `spawn_blocking`. Aucun Computer Use, aucune validation Linux. A27/A29 restent non commités après `c2f92b5` ; A30 est seulement consigné pour la suite.
