<?php
// Administration du dépôt de l'émulateur TRS-80 : retirer un fichier de la liste présentée
// aux utilisateurs (il passe dans la liste « À valider »), l'y remettre, changer sa catégorie
// ou les modèles sur lesquels il est proposé.
//
// POST <dépôt>/admin.php, corps JSON (envoyé en text/plain : pas de requête préalable CORS) :
//   { "password": "...", "action": "login" | "pending" | "hide" | "restore" | "category" | "models",
//     "kind": "cmd" | "bas" | "asm" | "cas" | "disk" | "rom", "file": "model1/x.cas",
//     "category": "games", "models": [1, 3] }
// Réponse : { "ok": true, ... } ou { "ok": false, "error": "..." }.
//
// Données hors du site (DATA) : password.hash (password_hash() du mot de passe, jamais dans
// Git), pending.json (fichiers retirés, avec leur entrée d'index), backup/ (copies des index
// avant chaque modification). Les fichiers eux-mêmes restent en place : seul l'index change.

const DATA = '/var/lib/trs80-admin';
const KINDS = ['rom', 'disk', 'cmd', 'bas', 'asm', 'cas'];
const CATEGORIES = ['games', 'education', 'finance', 'office', 'programming', 'utilities', 'graphics',
    'music', 'communications', 'science', 'systems', 'demo', 'other'];
const BACKUPS_KEPT = 200;

header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store');
if ($_SERVER['REQUEST_METHOD'] === 'OPTIONS') {
    header('Access-Control-Allow-Methods: POST');
    header('Access-Control-Allow-Headers: Content-Type');
    exit;
}

function reply(array $data, int $status = 200): void
{
    http_response_code($status);
    echo json_encode($data, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    exit;
}

function fail(string $error, int $status = 400): void
{
    reply(['ok' => false, 'error' => $error], $status);
}

function read_json(string $path, $default)
{
    if (!is_file($path)) {
        return $default;
    }
    $data = json_decode(file_get_contents($path), true);
    return is_array($data) ? $data : $default;
}

function write_json(string $path, array $data): void
{
    $json = json_encode($data, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    if ($json === false || file_put_contents($path, $json, LOCK_EX) === false) {
        fail("cannot write $path", 500);
    }
}

function index_path(string $kind): string
{
    return __DIR__ . "/$kind/index.json";
}

/** Copie de l'index avant modification; les plus anciennes copies sont effacées. */
function backup(string $kind): void
{
    $dir = DATA . '/backup';
    if (!is_dir($dir)) {
        mkdir($dir, 0750, true);
    }
    copy(index_path($kind), sprintf('%s/%s-%s.json', $dir, $kind, date('Ymd-His')));
    $all = glob("$dir/*.json");
    sort($all);
    foreach (array_slice($all, 0, max(0, count($all) - BACKUPS_KEPT)) as $old) {
        unlink($old);
    }
}

function find_entry(array $list, string $file): ?int
{
    foreach ($list as $i => $e) {
        if (($e['file'] ?? null) === $file) {
            return $i;
        }
    }
    return null;
}

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    fail('POST only', 405);
}
$req = json_decode(file_get_contents('php://input'), true);
if (!is_array($req)) {
    fail('bad request');
}
$hash = trim((string) @file_get_contents(DATA . '/password.hash'));
if ($hash === '' || !password_verify((string) ($req['password'] ?? ''), $hash)) {
    sleep(2); // freine les essais en série
    error_log('trs80 admin: bad password from ' . ($_SERVER['HTTP_X_FORWARDED_FOR'] ?? $_SERVER['REMOTE_ADDR']));
    fail('bad password', 403);
}

$action = (string) ($req['action'] ?? '');
if ($action === 'login') {
    reply(['ok' => true]);
}

// Une modification à la fois.
$lock = fopen(DATA . '/lock', 'c');
flock($lock, LOCK_EX);
$pendingPath = DATA . '/pending.json';
$pending = read_json($pendingPath, []);

if ($action === 'pending') {
    reply(['ok' => true, 'pending' => $pending]);
}

$kind = (string) ($req['kind'] ?? '');
$file = (string) ($req['file'] ?? '');
if (!in_array($kind, KINDS, true) || $file === '') {
    fail('bad kind or file');
}
$index = read_json(index_path($kind), null);
if ($index === null) {
    fail("no index for $kind", 500);
}
$i = find_entry($index, $file);
$p = null;
foreach ($pending as $k => $item) {
    if ($item['kind'] === $kind && ($item['entry']['file'] ?? null) === $file) {
        $p = $k;
    }
}

switch ($action) {
    case 'hide':
        if ($i === null) {
            fail('not in the list', 404);
        }
        backup($kind);
        $pending[] = ['kind' => $kind, 'entry' => $index[$i], 'position' => $i, 'hidden' => date('c'),
            'note' => (string) ($req['note'] ?? '')];
        array_splice($index, $i, 1);
        write_json(index_path($kind), $index);
        write_json($pendingPath, $pending);
        reply(['ok' => true]);

    case 'restore':
        if ($p === null) {
            fail('not pending', 404);
        }
        if ($i === null) {
            backup($kind);
            $item = $pending[$p];
            array_splice($index, min((int) ($item['position'] ?? count($index)), count($index)), 0, [$item['entry']]);
            write_json(index_path($kind), $index);
        }
        array_splice($pending, $p, 1);
        write_json($pendingPath, $pending);
        reply(['ok' => true]);

    case 'category':
        $category = (string) ($req['category'] ?? '');
        if (!in_array($category, CATEGORIES, true)) {
            fail('bad category');
        }
        if ($i !== null) {
            backup($kind);
            $index[$i]['category'] = $category;
            write_json(index_path($kind), $index);
        } elseif ($p !== null) {
            $pending[$p]['entry']['category'] = $category;
            write_json($pendingPath, $pending);
        } else {
            fail('not found', 404);
        }
        reply(['ok' => true]);

    case 'models':
        // Un modèle : un nombre; plusieurs : une liste (comme dans les index).
        $models = array_values(array_unique(array_map('intval', (array) ($req['models'] ?? []))));
        sort($models);
        if (!$models || array_diff($models, [1, 2, 3, 4])) {
            fail('bad models');
        }
        $value = count($models) === 1 ? $models[0] : $models;
        if ($i !== null) {
            backup($kind);
            $index[$i]['model'] = $value;
            write_json(index_path($kind), $index);
        } elseif ($p !== null) {
            $pending[$p]['entry']['model'] = $value;
            write_json($pendingPath, $pending);
        } else {
            fail('not found', 404);
        }
        reply(['ok' => true, 'model' => $value]);
}
fail('unknown action');
