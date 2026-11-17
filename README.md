# m4b-Craft

[![Dernière version, préversions incluses](https://img.shields.io/github/v/release/aruel-perso/m4b-craft?include_prereleases&label=version%20%28alpha%20incluse%29)](https://github.com/aruel-perso/m4b-craft/releases)

**Français** · [English](README.en.md)

Créez et éditez des livres audio M4B localement : import de pistes, ordre naturel,
chapitres, métadonnées, couverture, lecture et export AAC. Interface graphique,
CLI et serveur MCP en lecture seule. Aucun service audio distant requis.

**Version décrite : 0.3.0-alpha.1 (préversion).** Le badge et la page Releases
indiquent les versions effectivement disponibles.
Consultez les [versions GitHub, préversions incluses](https://github.com/aruel-perso/m4b-craft/releases)
et choisissez les fichiers d'une même version :

| Plateforme | Téléchargement prévu pour 0.3.0-alpha.1 | Installation |
| --- | --- | --- |
| Windows x86_64 | [MSI](https://github.com/aruel-perso/m4b-craft/releases/download/v0.3.0-alpha.1/m4b-Craft-0.3.0-alpha.1-windows-x86_64.msi) | Ouvrir le MSI ; installation par utilisateur, raccourci dans Démarrer. |
| macOS AppleSilicon ARM64 | [DMG](https://github.com/aruel-perso/m4b-craft/releases/download/v0.3.0-alpha.1/m4b-Craft-0.3.0-alpha.1-macos-aarch64.dmg) | Monter le DMG, glisser `m4b-Craft.app` dans Applications, éjecter le disque. |

Ces liens seront disponibles seulement après publication réussie. Pas de version
Mac Intel, Linux installable ou Android dans cette livraison. FFmpeg et ffprobe
sont inclus : aucune installation multimédia ni dépendance Homebrew à l'exécution.
Le ZIP Windows est une alternative portable ; le ZIP `-media-sources` contient les
sources correspondantes et n'est pas nécessaire pour lancer l'application.

**Avertissements de signature :** la signature Ed25519 des mises à jour ne constitue
pas une signature d'éditeur Windows ni une notarisation Apple. Windows peut afficher
« éditeur inconnu » ou SmartScreen. L'application Mac est signée ad hoc, sans
Developer ID ni notarisation ; Gatekeeper peut la bloquer. N'autorisez via
Confidentialité et sécurité / Ouvrir quand même qu'un téléchargement dont vous
avez vérifié l'origine et conformément à votre politique de sécurité.

L'historique de validation contient des tests Linux et Wine ; le nouveau travail
CI macOS reste **non testé tant qu'une exécution réelle n'a pas réussi**. Aucun test
interactif de l'interface Mac n'est revendiqué. Voir [VALIDATION.md](VALIDATION.md)
et les [exécutions CI](https://github.com/aruel-perso/m4b-craft/actions).

## Interface

Au démarrage, créez ou ouvrez un projet, ou sélectionnez un projet récent. La croix
retire seulement son lien. La langue suit celle de l’OS au premier lancement
(FR/EN, repli anglais) et votre choix manuel est mémorisé dans Réglages.
Le menu latéral se redimensionne par sa séparation verticale ; le nom du projet
figure sous les menus. Métadonnées affiche la couverture. La barre inférieure
regroupe lecture/pause, arrêt, précédent/suivant et déplacement dans la lecture.
Les nouveaux projets utilisent 160 kb/s pour l’export AAC ; les projets existants
conservent leur réglage. Le remux AAC compatible évite le réencodage.

## Projets et transport

Créez un projet `.m4bproj`, importez vos fichiers, réordonnez les pistes et ajustez
chapitres, métadonnées et couverture, puis exportez un nouveau `.m4b`. Les imports
copiés dans le dossier d'actifs du projet doivent rester avec le projet. Pour
transporter un travail éditable, enregistrez, fermez et copiez **tout le dossier
projet avec ses actifs**, en conservant les chemins relatifs. Gardez une sauvegarde ;
ne placez jamais vos projets dans le dossier d'installation ou dans le `.app`.
Les anciens projets peuvent référencer des chemins absolus : vérifiez les sources
sur la machine destinataire. Pour un lecteur audio, transportez le M4B exporté.
« Enregistrer sous » ne garantit pas le déplacement de tous les actifs.

L'import M4B récupère audio, chapitres et métadonnées disponibles. ZIP accepte les
entrées stockées/Deflate non chiffrées ; RAR/7Z nécessite un `7z`/`7zz` compatible
fourni séparément (non inclus). Pas de contournement DRM. L'export utilise une étape
PCM, environ **11,5 Mo par minute source**, plus le résultat AAC. Pas de précontrôle
d'espace libre ; conservez assez d'espace et une destination acceptant les liens
physiques. L'export refuse d'écraser un fichier existant. Les longues durées, pertes
d'alimentation et mutations des sources restent des cas à valider.

## Mises à jour intégrées

Le contrat 0.3 prévoit une vérification automatique au démarrage via GitHub Releases,
ainsi qu'une vérification manuelle dans les paramètres. Cette vérification contacte
GitHub pour les informations de version ; elle n'autorise pas l'installation.
Le canal stable exclut les préversions ; activez leur inclusion pour recevoir les
alphas. La signature du manifeste, la taille et le SHA-256 du fichier téléchargé
sont vérifiés avec la clé publique embarquée, inchangée pour cette version.

**Aucune installation sans votre consentement explicite.** Enregistrez le projet et
arrêtez lecture/export avant de confirmer. Le clic sur « Mettre à jour » télécharge
et vérifie le paquet sans navigateur, puis passe le relais à l’installation Windows
ou au remplacement de l’application macOS. Des confirmations système peuvent rester
nécessaires. La vérification au démarrage ne déclenche jamais seule l’installation.

Sur macOS, le remplacement automatique exige une `.app` installée dans un dossier parent accessible en écriture par votre utilisateur. Utilisez `~/Applications` (à créer si nécessaire), copiez-y `m4b-Craft.app`, éjectez le DMG puis ouvrez cette copie. Si `/Applications` n’est pas accessible en écriture, une erreur est affichée ; aucune élévation administrateur n’est demandée. Cette alpha ne dispose pas de signature Apple Developer ID ni de notarisation.

## Développement

Rust 1.100.0, Python 3.11+ et Cargo.lock. Compilation native :

```sh
cargo test --workspace --locked
cargo run -p m4b-desktop
cargo run -p m4b-cli -- --help
```

FFmpeg/ffprobe doivent être voisins des exécutables ou dans PATH pendant le
développement. Les bibliothèques système de développement GUI/audio sont nécessaires
sur Linux. Les recettes Windows GNU et Mac ARM64, sources épinglées, licences,
manifestes de mise à jour et limites sont décrits dans [RELEASE.md](RELEASE.md).

Licence [GPL-3.0-or-later](LICENSE). Les outils FFmpeg/zlib fournis conservent leurs
licences ; chaque livraison binaire doit inclure ses notices, configurations et
l'archive correspondante `-media-sources.zip`.
