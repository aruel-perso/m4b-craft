# m4b-Craft — requirements

## Phase autorisée — prérelease Windows et macOS Apple Silicon

Alex a validé le récapitulatif et ses amendements : développement, tests, commits/push et publication d’une prérelease avec MSI Windows et DMG macOS ARM64 (hypothèse Mac M2). Le lot comprend l’ensemble des demandes ci-dessous, avec vérification GitHub obligatoire au démarrage sans interrupteur de désactivation, installation uniquement sur consentement, langue de l’OS par défaut FR/EN (repli anglais) et choix manuel mémorisé. Signature Authenticode et signature/notarisation Apple exclues. Les anciennes mentions « à valider » documentent le cadrage ; cette autorisation vaut pour ce lot uniquement. Toute extension substantielle reste soumise à une nouvelle validation.

## Mode de travail actuel : projet / conception

À la demande d’Alex, consigner les idées et recommandations sans lancer leur développement. Une idée, une question ou une recommandation ne vaut pas autorisation d’implémentation. Attendre une demande explicite de développement avant toute modification du code, compilation, déploiement ou publication. La prise de notes et la clarification du périmètre restent autorisées.

Avant chaque phase de développement, même lorsqu’elle est demandée explicitement, présenter un récapitulatif du périmètre prévu : fonctionnalités et changements retenus, exclusions, points à trancher, risques et vérifications prévues. Demander une validation explicite de ce récapitulatif et attendre la réponse avant de commencer. Toute extension substantielle du périmètre nécessite une nouvelle validation.

## Idées et recommandations à cadrer — non autorisées au développement

### Visibilité de la version et du téléchargement sur GitHub
- Demande : afficher clairement la version avec un accès au téléchargement, à l’image de https://github.com/boykopovar/AnyPS5.
- Constat vérifié : AnyPS5 a une release stable `v0.1.5` reconnue par `/releases/latest` ; m4b-Craft a une prérelease `v0.2.0-alpha.1`, mais `/releases/latest` renvoie 404. Ne pas transformer artificiellement cette alpha en version stable pour obtenir un indicateur Latest.
- Recommandation à valider : badge de version compatible avec les préreleases et bouton/lien de téléchargement bien visible en tête du README ; vérifier également la présentation Releases sur la page d’accueil du dépôt. Le lien `/releases/latest` seul ne convient pas tant qu’aucune stable n’existe.

### Version macOS pour un testeur
- Demande : prévoir une distribution macOS testable, en complément de Windows.
- À préciser : modèle/architecture du Mac (Apple Silicon ou Intel) et version de macOS.
- Recommandation à valider : application `.app` livrée en `.dmg`, backend FFmpeg/ffprobe inclus, build et vérifications sur macOS, tests d’import/export/lecture et conservation des projets. Évaluer la signature/notarisation et le parcours Gatekeeper ; ne pas promettre une distribution signée sans identité Apple disponible. L’installation et les mises à jour macOS nécessitent un mécanisme adapté, pas le MSI Windows.
- Aucun développement, build, changement GitHub ou publication autorisé par cette prise de notes. Présenter le périmètre récapitulatif et obtenir validation avant exécution.

### Aperçu de la couverture dans Metadata
- Demande : afficher visuellement la cover dans la section `Metadata`, et pas seulement sa référence ou son chemin.
- Recommandations à valider : aperçu de la couverture courante (importée du M4B ou choisie dans le projet), proportions conservées, actualisation après remplacement et état explicite lorsqu’aucune couverture n’est définie ou que l’image est illisible.
- Statut : besoin consigné uniquement ; à intégrer au récapitulatif soumis à validation avant développement.

### Barre de lecture audio en bas de fenêtre
- Demande : pendant la lecture audio, afficher en bas de la fenêtre une barre avec les commandes Précédent / Play / Pause / Stop / Suivant.
- Recommandations à valider : garder la barre visible en pause ; afficher le titre en cours, la position/durée et une progression permettant le déplacement ; commandes cohérentes avec l’état du lecteur.
- À préciser au cadrage : navigation Précédent/Suivant par piste ou par chapitre selon le contenu ; comportement de la barre après Stop.

### Réglages dans le menu supérieur
- Demande : ajouter une entrée Réglages / Settings dans le menu haut, avec libellé adapté à la langue choisie.

### Débit d’export par défaut
- Demande : débit d’export par défaut de 160 kb/s (interprétation de « débit 160 »).
- Recommandation à valider : appliquer ce défaut aux nouveaux projets ; préserver le débit explicitement enregistré dans les projets existants et ne pas forcer un réencodage lors d’un remux sans perte.

### Écran de démarrage et projets récents
- Demande : écran d’accueil de type Android Studio, proposant création ou sélection d’un projet ; ce choix est obligatoire avant d’accéder à l’éditeur.
- Demande : liste persistante des projets précédemment créés ou sélectionnés ; une croix retire le lien de la liste uniquement, sans supprimer le projet ni ses médias.
- Recommandations à valider : nom et emplacement lisibles, détection des liens devenus invalides, retour à l’accueil à la fermeture d’un projet et respect des modifications non enregistrées au changement de projet.

### Extension de fichier projet dédiée
- Demande : remplacer l’extension utilisateur `.json` par une extension identifiant clairement un projet m4b-Craft.
- Proposition à valider : `.m4bproj`. Le format interne peut rester JSON versionné ; une extension dédiée ne nécessite pas de réécrire le stockage.
- Recommandations à valider : conserver l’ouverture des anciens projets `.json` et leurs références `.assets`, proposer le nouveau suffixe lors de l’enregistrement sans migration destructive ; distinguer projet `.m4bproj` et livre audio exporté `.m4b`.

### README multilingue, français prioritaire
- Demande : proposer un README multilingue ; Alex est francophone.
- Recommandation à valider : `README.md` en français par défaut et `README.en.md` en anglais, avec liens de langue visibles en tête des deux fichiers.
- Maintenir les mêmes informations essentielles dans les deux langues : présentation, téléchargement/version, installation, utilisation, limites et statut des fonctionnalités. Conserver les commandes et identifiants techniques exacts.
- Statut : besoin consigné uniquement ; ne pas réécrire ou publier les README avant validation du récapitulatif.

### Notification et installation intégrée des mises à jour GitHub
- Demande : lorsqu’une mise à jour GitHub est disponible, afficher une popup proposant de la réaliser ; un clic déclenche la chaîne téléchargement/vérification/installation intégrée, sans navigateur ni téléchargement manuel.
- Proposition à valider : popup indiquant versions actuelle et disponible, avec « Mettre à jour » et « Plus tard », progression et erreurs visibles ; recherche automatique au démarrage si activée dans les réglages, respect du canal stable/préversions et de la plateforme/architecture.
- Sécurité et données : vérification obligatoire de la signature et des octets téléchargés avant installation ; aucune perte de projet non enregistré ni interruption d’un export/opération média. Si nécessaire, demander l’enregistrement ou différer l’installation ; ne pas fermer silencieusement le projet.
- Distinguer automatisation depuis l’application et absence totale de dialogue système : les confirmations Windows/macOS éventuellement requises restent applicables. Prévoir une reprise de l’application après succès sans boucle de mise à jour ; adapter l’installateur à chaque plateforme.
- Statut : besoin consigné uniquement ; ce parcours remplace la succession manuelle des actions de mise à jour dans le périmètre futur, sous réserve de validation préalable du récapitulatif.

### Signature éditeur Windows différée
- Décision : remettre à plus tard la signature éditeur Authenticode (dite « signature Microsoft »). Aucune inscription, acquisition de certificat ou intégration de ce service dans la prochaine phase sans nouvelle validation.
- Cette décision ne retire pas la vérification cryptographique obligatoire des mises à jour (signature Ed25519 et contrôle des octets téléchargés).

### Menu latéral redimensionnable
- Demande : rendre déplaçable par glisser-déposer la séparation verticale entre le menu gauche et le contenu, pour ajuster la largeur du menu.
- Recommandations à valider : curseur de redimensionnement au survol, limites de largeur protégeant le menu et le contenu, mémorisation de la largeur entre les sessions.

### Nom du projet sur une seconde ligne
- Demande : afficher le nom du projet sélectionné sur une seconde ligne distincte, sous le menu supérieur, et non sur la première ligne réservée aux menus.
- Recommandations à valider : conserver un nom lisible quand il est long et un indicateur des modifications non enregistrées ; ne pas mélanger nom de projet et titre du livre audio.
- Statut de ces ajustements : notes de cadrage uniquement, à inclure dans le récapitulatif soumis à validation avant développement.

## Périmètre et exigences historiques
Rust desktop audiobook creator/editor. Windows 11 x86_64 first, Linux x86_64 next; Android future: portable core, no UI coupling. Libre GPL-3.0-or-later proposed. No remote repository yet; never publish or push without request.
User-approved scope: egui/eframe proposed native Rust UI; FR/EN selectable language, system/light/dark themes; M4B primary open/export format; import audio tracks, natural ordering and manual reorder; edit title, author, narrator, description, chapters and embedded cover; AAC default with codec/bitrate selection based on actual encoder availability, source properties, estimated output size, lossless remux where possible; ZIP/RAR/7z safe import with limits/password support eventually; playback tracks, transitions, output and encoded samples eventually; shared Rust core and stdio MCP API, optional future MCP client; save project, autosave/recovery; async export, real progress, cancel; no overwrite originals, stage temp output and validate before finalize; one chapter per track default; no DRM bypass and no advanced waveform editing in v1.
Future GitHub Releases updates: optional checks, stable/prerelease channels, signatures mandatory before automatic installation, no updates during export; platform-specific installers. Windows exe + installer goal, Linux AppImage later. Future Android must not depend on spawning desktop FFmpeg executable: isolate media backend and filesystem abstractions.
Start with a real working vertical slice and tests; document gaps honestly, never present roadmap as implemented. Use low resource builds (CARGO_BUILD_JOBS=1, no debug info). All tasks/implementation within this project, no unrelated system changes, no commits unless requested.

Mandatory alpha acceptance: native in-app pre-listening (tracks and opened M4B, play/pause/stop, seek, volume, chapter jump) is included, not deferred. Windows distribution must be self-contained: no FFmpeg/ffprobe install, PATH change, external player, runtime installer, or first-launch download. Bundle the media backend and applicable licenses/corresponding source. Discover packaged tools relative to the application executable, not the working directory. A portable directory/ZIP is acceptable; a single monolithic EXE is not required.
