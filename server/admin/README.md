# Administration du dépôt (`admin.php`)

Le panneau **Admin** de l'émulateur (section Settings) permet, avec un mot de passe vérifié
par le serveur :

- de tester le programme en cours sur les Model I, III et 4 ;
- de **retirer un fichier de la liste** présentée aux utilisateurs : son entrée quitte
  `<genre>/index.json` et passe dans la liste « À valider » (visible seulement en admin),
  d'où on peut la remettre ;
- de **changer sa catégorie**.

Les fichiers eux-mêmes restent sur le serveur; seul l'index change. Chaque modification
garde une copie de l'index dans `/var/lib/trs80-admin/backup/` (les 200 dernières).

## Installation (Apache avec PHP)

```sh
# Script à côté des dossiers du dépôt (rom/, disk/, cmd/...).
sudo cp admin.php /var/www/ve2cuy.com/trs80/

# Données hors du site : mot de passe (haché), liste « À valider », copies des index.
sudo install -d -o www-data -g www-data -m 750 /var/lib/trs80-admin
php -r 'echo password_hash($argv[1], PASSWORD_DEFAULT), "\n";' 'MOT-DE-PASSE' \
  | sudo tee /var/lib/trs80-admin/password.hash >/dev/null
sudo chown www-data:www-data /var/lib/trs80-admin/password.hash
sudo chmod 640 /var/lib/trs80-admin/password.hash

# PHP (www-data) doit pouvoir réécrire les index.
sudo chown www-data /var/www/ve2cuy.com/trs80/*/index.json
sudo chmod 664 /var/www/ve2cuy.com/trs80/*/index.json
```

Pour changer le mot de passe, refaire la commande `password_hash`. Le mot de passe n'est
jamais dans Git ni dans la page : la page l'envoie à chaque action et le serveur le
vérifie (pause de 2 s après un refus).

## Demandes de retrait (ayants droit)

La fiche d'un fichier du dépôt, sous l'écran, invite l'ayant droit à demander le retrait
du programme : le lien ouvre le formulaire GitHub « Demande de retrait / Removal request »
(`.github/ISSUE_TEMPLATE/removal-request.yml`), avec le programme et le chemin du fichier
déjà remplis, et l'étiquette `removal-request`.

Traitement : lancer le fichier depuis le dépôt (en admin), **Retirer de la liste**
(il passe dans « À valider »), répondre dans l'issue, puis la fermer.
