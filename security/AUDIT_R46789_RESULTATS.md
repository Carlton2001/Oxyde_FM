# R4, R6, R7, R8, R9 — résultats du 13 octobre 2026

Branche `codex/security-r1-r2-r3`, poursuivie depuis `69a9f53` à la demande de l'utilisateur. Cette passe ajoute des corrections et des preuves ; elle ne transforme pas les limites restantes en garanties. R5/Linux demeure exclu. Les droits supplémentaires étudiés pour R1 restent désactivés en production.

| Point | Résultat livré | Limite restante |
| --- | --- | --- |
| R4 | Identités nulles refusées, provenance originale conservée et comparée, essais de liens physiques et d'objets étrangers. | Matrice native validée sur le volume local C: NTFS fixe. Autres volumes/fournisseurs, snapshots immuables et tous les ADS hors preuve. |
| R6 | Tentative d'interruption native sur annulation/délai, étendue aux préparations et aux I/O du courtier ; scopes imbriqués et reprise testés. | Un pilote peut refuser l'interruption ; aucun délai absolu pour toute I/O du parent n'est promis. |
| R7 | Nouveau cache avec preuves persistantes authentifiées des créations et écritures, reprise après crash réel. | Anciennes préparations sans preuve, clé perdue/corrompue, objets modifiés/étrangers et états incomplets sans preuve suffisante conservés. |
| R8 | Toutes les archives sont préparées avant publication ; restauration du groupe sur échec ou annulation entre publications. | Pas d'atomicité de visibilité entre fichiers, ni de transaction durable couvrant crash/panne machine et sources ordinaires. |
| R9 | Vérification indépendante du contenu, de l'inventaire et de métadonnées définies pour Decode/Encode TAR non compressé. | ZIP/7z, conteneurs compressés, ISO, listing/index et pixels/textes des aperçus restent sans vérification indépendante générale. |

## Décisions de portée

L'utilisateur a choisi pour R8 : **« Préparer ensemble et restaurer en cas d'échec »**, avec la limite explicitée qu'un crash peut demander une récupération. Les originaux sont donc conservés jusqu'à la fin des publications du groupe.

Pour R9, la question sur le maintien ou le blocage des fonctions dépourvues de vérification indépendante a été présentée. En l'absence de réponse, leurs comportements existants sont conservés ; cela ne constitue pas une acceptation de leurs limites ni une clôture de R9. Aucun blocage général des formats/aperçus n'a été activé.

## R4 — objets, contenus et matrice réellement exercée

Le résultat natif `FILE_ID_INFO` à identifiant nul est désormais refusé. Le repli historique vers l'identifiant 64 bits reste limité aux erreurs de classe native déjà prévues ; il n'est pas présenté comme une identité stable sur tous les fournisseurs.

Les workspaces mémorisent la provenance Internet `Zone.Identifier` lors de l'ouverture, la contrôlent après lecture, puis la comparent à nouveau sur l'original avant remplacement. Une modification de cette provenance sur le même objet fait échouer la mutation avant backup/publication. Les contrôles existants d'identité, taille, date et BLAKE3 du flux principal sont conservés.

| Scénario Windows local NTFS | Preuve/contrat |
| --- | --- |
| Deux liens physiques d'un fichier | Même identité observée ; écriture via l'autre lien refusée pendant retenue. |
| Renommage d'un lien | Le nom sélectionné est protégé ; un autre lien peut être renommé. Il reste lié au même objet et ne permet pas de contourner le refus d'écriture. |
| Modification des dates seule | Identité inchangée : les dates ne constituent pas une preuve cryptographique du contenu. |
| Remplacement avec mêmes octets | Identité différente ; le contrôle d'admission refuse l'objet étranger et sa fermeture ne le supprime pas. |
| Modification du contenu avec date restaurée | Tests natifs existants conservés ; BLAKE3 refuse la reconstruction conflictuelle. |
| Modification de `Zone.Identifier` | Nouveau test natif : refus, archive originale inchangée, aucun backup publié. |
| Deux archives liées au même original dans un groupe | Refus avant publication, objets et octets originaux conservés. |
| Jonctions/reparse dans les parcours retenus | Essais existants conservés ; ajout du refus d'un ancêtre jonction lors de la consultation de la clé du cache. |

La requête locale `Get-Volume -DriveLetter C` a confirmé **NTFS, Fixed**. FAT/exFAT, ReFS, SMB, volumes amovibles, fournisseurs cloud et points de montage n'ont pas été exécutés dans cette passe. Les refus de capacité/identité/reparse restent actifs, sans convertir une cible non testée en cible garantie.

Les IDs ne sont pas cryptographiques et peuvent être réutilisés après suppression ; garder un handle vivant évite ce problème pour les comparaisons de l'opération courante. Les preuves persistantes du cache ajoutent identité, création et contenu authentifiés, sans prétendre constituer un snapshot général du système de fichiers. Les autres ADS et les modifications via une vue mémoire préexistante restent hors garantie universelle. [Documentation Microsoft sur les identifiants de fichiers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information), [partage et fichiers mappés](https://learn.microsoft.com/en-us/windows/win32/memory/creating-a-file-mapping-object).

## R6 — interruption coopérative du parent

Le mécanisme passe par `Platform::watch_broker_io`, implémenté dans `windows_broker_cancellation.rs`. Un moniteur possède un handle du thread appelant et tente `CancelSynchronousIo` sur annulation ou dépassement du temps actif. La pause suspend ce compteur ; les demandes sont répétées pour couvrir les intervalles entre appels natifs.

La couverture comprend le courtier Decode/Encode, les ouvertures et empreintes de workspaces, leur préparation de reconstruction, la préparation/finalisation du cache et la retenue des sources des nouveaux workers d'image/texte/listing. Les opérations gardent leurs limites et contrôles existants.

Les scopes d'un même thread partagent un moniteur avec leurs propres délais/flags : maximum huit moniteurs physiques et seize scopes imbriqués par moniteur. L'admission est annulable et bornée à trente secondes. La garde ne peut pas être transférée à un autre thread. Retirer un scope attend les demandes natives en cours, pour empêcher un ancien délai d'interrompre l'I/O suivante. La destruction du dernier scope termine le moniteur avant réutilisation du thread.

Les tests utilisent de vraies lectures synchrones de pipe bloquées : annulation, délai, pause, scopes imbriqués et seconde lecture réussie sur le même thread. Un test dans le véritable courtier de compression obtient `ERROR_OPERATION_ABORTED` (995), conserve la source et ne publie aucune sortie.

Le mécanisme tente une interruption et conserve les gardes d'objets jusqu'au retour natif. Il ne tue jamais un thread du parent. Les appels de pilotes/fournisseurs qui refusent la cancellation et les appels hors de ces scopes ne sont pas couverts par une promesse de délai absolu. [Limites documentées de la cancellation Windows](https://learn.microsoft.com/en-us/windows/win32/fileio/canceling-pending-i-o-operations).

## R7 — récupération attestée des nouvelles préparations

Les nouvelles entrées Windows utilisent `oxyde-archive-open/v2` et le protocole de propriétaire version 4. Le stockage de la clé et des journaux est privé au parent. La clé aléatoire Oxyde de 32 octets est protégée à repos par DPAPI ; son fichier a une DACL protégée donnant accès à l'utilisateur et SYSTEM, vérifiée à chaque lecture. La création/lecture refuse atomiquement toute traversée reparse. Une clé inconnue n'est ni remplacée ni réinitialisée.

Chaque preuve combine une enveloppe DPAPI et un MAC BLAKE3 avec cette clé. Un blob DPAPI créé avec les mêmes credentials, mais sans la clé Oxyde, ne fournit pas d'autorité de nettoyage. La sonde LPAC lit le fichier témoin dans un dossier explicitement lisible pour ARAP, mais refuse la clé protégée de ce même dossier. Une autre sonde observe un échec DPAPI 14 avec témoin positif hors sandbox ; c'est une observation d'initialisation, pas une preuve de refus universel de l'API.

Les enregistrements portent seulement sur les créations capturées par `OwnedTree` : chemin relatif, identité et création du root, des dossiers et des fichiers. Deux slots de preuve sont alternés et synchronisés. Avant d'écrire un bloc de payload, le parent persiste le hash/la taille du préfixe antérieur et les octets attendus du bloc suivant. Après un crash au milieu d'une écriture, la reprise peut ainsi vérifier un préfixe exact du bloc, sans adopter un fichier par son seul nom.

La récupération ouvre les objets de façon exclusive et vérifie toute l'arborescence survivante avant la première suppression : racine, marker, slots, identities, création, préfixe/hash, taille et provenance. L'énumération sert à refuser les étrangers, jamais à leur attribuer une propriété. Un fichier modifié/remplacé, une provenance changée, un lecteur actif, une preuve falsifiée/rejouée ou un enfant étranger empêche le nettoyage du lot. Les plafonds sont 128 preuves, 2 Mio par slot et un bloc de 256 Kio ; les quotas d'entrée comptent les slots.

Les tests couvrent zéro octet, écriture partielle, bloc complet, modifications/remplacements/provenance, lecteur actif, falsification et rejeu. Un sous-processus réel sort brutalement au checkpoint d'écriture, sans Drop ; après le délai de grâce normal de soixante secondes, un autre processus récupère son entrée attestée puis peut ouvrir à nouveau le document.

Les anciens caches `v1` ne sont pas purgés ou adoptés : leurs préparations version 3 ne contiennent pas ces preuves. Leur espace physique n'est pas inclus dans le quota logique du nouveau stockage. Une clé manquante/corrompue et les crashs laissant un état sans preuve complète restent conservateurs et peuvent nécessiter une récupération manuelle. Les credentials complets de l'utilisateur, un administrateur hostile et un parent compromis sont hors base de confiance ; les workers natifs encore non confinés restent le reliquat R1. [Portée et intégrité de DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata).

## R8 — publication groupée et restauration

`archive_workspace/native_commit.rs` a d'abord été extrait sans changement fonctionnel. Le parcours natif distingue désormais préparation et publication. Chaque reconstruction est terminée et synchronisée, et son original contrôlé/retenu, avant que le premier original soit déplacé. Les archives sont traitées dans un ordre stable, destination d'abord. Un groupe contenant plusieurs chemins vers le même objet original est refusé.

Les backups restent retenus jusqu'au succès de toutes les publications. Un échec ou une annulation entre publications restaure les membres déjà modifiés en ordre inverse. Il n'y a aucun checkpoint entre le backup et sa publication, ni pendant la restauration. Une restauration déplace uniquement le nouvel objet retenu puis restaure l'ancien par handles, sans écraser un remplacement étranger. Les gardes de renommage de la préparation sont fermées avant son nettoyage.

Une restauration refusée conserve et signale l'original au backup ; les autres membres sont tout de même restaurés. Après succès du groupe, une erreur de suppression d'un backup produit un rapport de récupération avec `completed=true`. Les tests vérifient trois archives, un échec tardif, l'annulation entre publications, un remplacement étranger et des noms liés au même original. Le test historique de transfert interrompu vérifie maintenant que l'échec de préparation laisse **les deux originaux exactement inchangés**, conformément au contrat choisi.

Le groupe n'est pas une transaction atomique observable par tous les lecteurs. Un crash ou une panne machine pendant la séquence peut laisser des versions publiées et des backups à récupérer ; aucune récupération multi-archives automatique après crash n'est annoncée. Le retrait ultérieur de sources ordinaires du filesystem reste distinct du groupe d'archives. Aucune dépendance TxF n'a été ajoutée ; Microsoft recommande des alternatives. [Alternatives à Transactional NTFS](https://learn.microsoft.com/en-us/windows/win32/fileio/deprecation-of-txf).

## R9 — premier périmètre de fidélité vérifiable

`utils/tar_fidelity.rs` interprète directement les enregistrements TAR bornés dans le parent, sans appeler le codec `tar`. Il contrôle les headers/checksums, noms relatifs, types, tailles, modes, mtime, inventaire complet et digests de payloads. Les budgets d'entrée, de temps, de profondeur et de noms sont conservés ; les vérifications tiennent compte de la pression du parent. Ce petit vérificateur entre explicitement dans la base de confiance.

Pour **Decode TAR**, le parent dérive la référence depuis le fichier source retenu avant de lancer le worker. Les métadonnées annoncées sont comparées à cet inventaire ; les octets reçus sont hashés et comparés avant succès. Pour une sélection, seuls ses payloads sont hashés, les headers des voisins restant contrôlés. Omettre une entrée ou un dossier requis, changer un nom/type/mode/date ou renvoyer d'autres octets échoue.

Pour **Encode TAR**, la référence provient des métadonnées des sources retenues et des octets réellement envoyés par le parent. Après la fin du worker, le parent vérifie indépendamment le TAR produit avant publication : même inventaire, mêmes digests et métadonnées définies. Les propriétaires, noms de liens, données de padding et extensions inattendues de la sortie sont également refusés.

Le sous-ensemble accepté couvre TAR ordinaire, V7/USTAR/GNU, noms longs GNU et extensions PAX `path`/`size`. Liens/sparse et extensions non prises en charge sont refusés ; il n'y a pas de repli vers le seul résultat du codec. Une source TAR corrompue conserve la catégorie d'erreur de traitement d'archive ; une falsification de sortie produit l'échec de vérification de fidélité.

Les tests comprennent un véritable exécutable hostile qui consomme toutes les sources de l'encodeur puis renvoie `evil` dans un TAR valide, et qui renvoie `evil` au décodeur avec des trames valides. Les deux échouent sans publication. Les parcours ordinaires POSIX/PAX et noms longs passent. Les essais unitaires portent aussi sur omission, métadonnées, checksums, troncature, données terminales et noms ambigus/dangereux.

**Cela ne clôture pas R9 général.** Les autres formats et les inventaires du worker de listing/index, ainsi que les pixels/textes des aperçus, ne bénéficient pas d'une référence indépendante générale. Aucun round-trip par le même codec ni hash annoncé par le worker n'est présenté comme une preuve. Le choix de maintien ou de blocage de ces fonctions reste à confirmer ; aucune acceptation n'est déduite de l'absence de réponse.

## Validation et commits

Dernière exécution complète Windows, configuration de développement, sans filtre ni exclusion et avec TEMP/TMP privé : **1 023 réussis, 0 échec, 17 ignorés**. Répartition : bibliothèque 691/16 ignorés, application 198/1 ignoré, pipeline archives 29, worker archive-I/O 40, listing 30, preview 35. Les binaires auxiliaires et l'exemple découvrent zéro test ; le sous-harnais déjà compté par son test parent n'est pas ajouté une seconde fois.

`cargo check --all-targets --offline` et `cargo build --bins --offline` réussissent sans avertissement du projet ; `git diff --check` passe. Clippy n'a pas été relancé. Journaux privés : `target/security-validation/r46789-final-4f49704ab92746cdbb32d3706335f207/{check-complete,tests-complete,build-complete}.log`.

Les passages intermédiaires ont repéré les hypothèses d'ancien emplacement du cache et la catégorie d'erreur du TAR tronqué. Les tests de nettoyage vérifient désormais exactement les deux fichiers d'infrastructure attendus, sans résidu d'entrée ; le TAR tronqué conserve sa catégorie d'erreur de traitement. Une mesure temporelle de sonde incluait compilation/admission : son délai est maintenant mesuré après readiness réelle, en conservant le délai du superviseur de trois secondes et l'obligation d'échec.

Commits de code :

- R4 : `8ba197e` (provenance/alias/identités), `5e626a1` (rejet d'objet étranger).
- Structure R8 : `9831aa8` (extraction native sans changement fonctionnel).
- R6 : `5c2f418` (interruption du courtier), `1f364e7` (préparations et scopes imbriqués).
- R7 : `6c72198` (journal authentifié/reprise), `d3450e0` (clé sans reparse et témoins de permissions).
- R8 : `273135e` (publication/restauration groupée), `da6e848` (originaux liés refusés).
- R9 TAR : `e209a68` (références indépendantes, vérification et worker hostile).
- Validation temporelle : `2c9e26d` (mesure après disponibilité native).

La borne R1/R2/R3 de 986 réussis/17 ignorés et les bornes visuelles antérieures restent historiques. La checklist visuelle de cette branche n'a pas été exécutée. Linux n'a été ni compilé ni lancé ; `paste`, `rustybuzz`, `ttf-parser` et `Cargo.lock` restent inchangés. Aucun Computer Use, push ou suivi automatique.

Vérification manuelle utile : ouverture/export depuis des archives, déplacements entre deux archives puis reprise après erreur, images/texte Office/PDF/vidéo ordinaires, messages de refus pour les TAR non pris en charge. Le packaging conserve les quatre exécutables côte à côte, avec `cargo build --bins`.
