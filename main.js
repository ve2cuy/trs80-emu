// Page de l'émulateur : charge le module WebAssembly, la ROM et les programmes, puis
// fait tourner la boucle d'affichage. Toute l'émulation est dans Rust (crates/web).
// Les textes de l'interface sont dans i18n.js (anglais, français, espagnol, chinois).

// Version de déploiement (?v=<commit> inscrit par GitHub Actions dans index.html) : ajoutée
// à chaque fichier chargé, pour qu'une mise à jour ne mélange jamais anciens et nouveaux
// fichiers gardés en cache. En développement (?v=dev), on ne met rien en cache.
const BUILD = new URL(import.meta.url).searchParams.get('v') ?? 'dev';
const V = `?v=${BUILD === 'dev' ? Date.now() : BUILD}`;

const { LANGUAGES, browserLanguage, getLanguage, setLanguage, t, localized } = await import(`./i18n.js${V}`);
const { default: init, Emulator, assemble, asm_builtins_json } = await import(`./pkg/trs80_web.js${V}`);
const { createIde } = await import(`./ide.js${V}`);
const { FONTS, buildAtlas, drawText } = await import(`./fonts.js${V}`);
const { Modem } = await import(`./modem.js${V}`);
const { AnsiScreen } = await import(`./ansi.js${V}`);
const { CHEATSHEET, defaultCheatsheet } = await import(`./cheatsheet.js${V}`);

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const overlay = document.getElementById('overlay');
const errorBox = document.getElementById('error');
const statusBox = document.getElementById('status');
const speedBox = document.getElementById('speed');
const resetButton = document.getElementById('reset');
const turbo = document.getElementById('turbo');
const programList = document.getElementById('program-list');
const cmdFile = document.getElementById('cmd-file');
const cmdButton = document.getElementById('cmd-button');
const programsHint = document.getElementById('programs-hint');
const typeButton = document.getElementById('type-button');
const expansion = document.getElementById('expansion');
const lowercaseBox = document.getElementById('lowercase');
const hiresBoard = document.getElementById('hires');
const soundBox = document.getElementById('sound');
const fastTapeBox = document.getElementById('fast-tape');
const driveSound = document.getElementById('drive-sound');
const app = document.getElementById('app');

// ------------------------------------------------------------------ préférences

// Conservées dans localStorage (petit objet JSON); les fichiers vont dans IndexedDB.
const PREFS_KEY = 'trs80-prefs';
const prefs = {
  theme: 'system',      // 'system', 'light' ou 'dark'
  sidebar: null,        // 'expanded' ou 'collapsed' (null : selon la largeur de l'écran)
  open: ['machine'],    // sections ouvertes du menu
  turbo: false,
  sound: true,
  expansion: true,
  lowercase: true,      // Model I : modification « minuscules »
  hires: true,          // carte graphique haute résolution (Model III et 4)
  fastTape: true,       // magnétophone en marche : émuler aussi vite que possible
  keepFiles: true,      // garder dans la bibliothèque les fichiers ouverts
  keepSession: true,    // retrouver les disques (avec leurs écritures) au rechargement
  modemRelay: null,     // adresse du relais telnet (null : celle par défaut)
  modemFilter: true,    // modem : retirer les séquences ANSI
  modemColor: false,    // modem : écran couleur ANSI pendant la connexion
  bbsPick: 'bbs.electrodrome.net:23', // BBS choisi dans la liste du modem
  driveSound: false,    // imiter le bruit des lecteurs de disquettes
  repo: 'https://ve2cuy.com/trs80', // dépôt externe (dossiers rom, disk, cmd, bas)
  repoKind: 'rom',
  repoCat: {},          // dépôt : catégorie choisie, par genre de fichier
  repoQuery: '',        // dépôt : texte recherché
  repoAll: false, // dépôt : afficher les fichiers de tous les modèles
  repoPick: {},   // dépôt : dernier fichier choisi par catégorie (rom, disk...)
  lang: null,           // langue de l'interface (null : celle du fureteur)
  model: 1,             // modèle émulé : 1, 2, 3 ou 4
  ...readPrefs(),
};

function readPrefs() {
  try {
    return JSON.parse(localStorage.getItem(PREFS_KEY)) ?? {};
  } catch {
    return {};
  }
}

function savePrefs() {
  try { localStorage.setItem(PREFS_KEY, JSON.stringify(prefs)); } catch { /* stockage indisponible */ }
}

// ------------------------------------------------------------------ langue

// Langue : ?lang=<code> (pour ce chargement seulement), sinon le choix conservé, sinon celle
// du fureteur. Les textes créés par ce module se redessinent par les fonctions inscrites
// dans languageListeners.
const languageListeners = [];
const langList = document.getElementById('lang-list');
for (const [code, name] of Object.entries(LANGUAGES)) langList.append(new Option(name, code));
const langParam = new URLSearchParams(location.search).get('lang');
const langCode = document.getElementById('lang-code');
langList.value = setLanguage(langParam in LANGUAGES ? langParam : prefs.lang ?? browserLanguage());
langCode.textContent = langList.value.toUpperCase();
langList.addEventListener('change', () => {
  prefs.lang = setLanguage(langList.value);
  langCode.textContent = prefs.lang.toUpperCase();
  savePrefs();
  for (const listener of languageListeners) listener();
});

turbo.checked = prefs.turbo;
soundBox.checked = prefs.sound;
expansion.checked = prefs.expansion;
lowercaseBox.checked = prefs.lowercase;
hiresBoard.checked = prefs.hires;
turbo.addEventListener('change', () => { prefs.turbo = turbo.checked; savePrefs(); focusScreen(); });
fastTapeBox.checked = prefs.fastTape;
fastTapeBox.addEventListener('change', () => { prefs.fastTape = fastTapeBox.checked; savePrefs(); focusScreen(); });

// ------------------------------------------------------------------ thème

const darkSystem = matchMedia('(prefers-color-scheme: dark)');

function applyTheme(theme) {
  prefs.theme = theme;
  if (theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  for (const radio of document.querySelectorAll('input[name="theme"]')) radio.checked = radio.value === theme;
  savePrefs();
}

for (const radio of document.querySelectorAll('input[name="theme"]')) {
  radio.addEventListener('change', () => applyTheme(radio.value));
}
// Bouton rapide : passe à l'inverse du thème affiché.
document.getElementById('theme-toggle').addEventListener('click', () => {
  const dark = prefs.theme === 'dark' || (prefs.theme === 'system' && darkSystem.matches);
  applyTheme(dark ? 'light' : 'dark');
});
applyTheme(prefs.theme);

// ------------------------------------------------------------------ menu latéral

// Ordinateur et tablette : menu fixe, déplié ou réduit à ses icônes. Téléphone : tiroir.
const phone = matchMedia('(max-width: 760px)');
const backdrop = document.getElementById('backdrop');
const navItems = [...document.querySelectorAll('.nav-item')];

function applySidebar() {
  const collapsed = !phone.matches
    && (prefs.sidebar ?? (innerWidth < 1100 ? 'collapsed' : 'expanded')) === 'collapsed';
  app.classList.toggle('collapsed', collapsed);
  const button = document.getElementById('sb-collapse');
  button.title = t(collapsed ? 'menu.expand' : 'menu.collapse');
  button.setAttribute('aria-label', button.title);
  if (!phone.matches) setDrawer(false);
}
languageListeners.push(applySidebar);

function setDrawer(open) {
  app.classList.toggle('drawer-open', open);
  backdrop.hidden = !open;
}

function openPanel(item, open) {
  item.setAttribute('aria-expanded', String(open));
  document.getElementById(item.getAttribute('aria-controls')).hidden = !open;
  prefs.open = navItems.filter((i) => i.getAttribute('aria-expanded') === 'true')
    .map((i) => i.getAttribute('aria-controls').replace('panel-', ''));
  savePrefs();
  if (open) item.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  if (open && item.getAttribute('aria-controls') === 'panel-repo') loadRepo(prefs.repoKind);
}

/** Ouvre une section du menu (et le menu lui-même s'il est réduit ou fermé). */
function showPanel(name) {
  const item = navItems.find((i) => i.getAttribute('aria-controls') === `panel-${name}`);
  if (phone.matches) setDrawer(true);
  else if (app.classList.contains('collapsed')) { prefs.sidebar = 'expanded'; applySidebar(); }
  openPanel(item, true);
}

for (const item of navItems) {
  const name = item.getAttribute('aria-controls').replace('panel-', '');
  item.setAttribute('aria-expanded', String(prefs.open.includes(name)));
  document.getElementById(`panel-${name}`).hidden = !prefs.open.includes(name);
  item.addEventListener('click', () => {
    // Menu réduit : un clic sur une icône le déplie sur sa section.
    if (app.classList.contains('collapsed')) showPanel(name);
    else openPanel(item, item.getAttribute('aria-expanded') !== 'true');
  });
}

// Menu réduit : le nom de la section en infobulle, à droite de l'icône survolée.
const tooltip = document.createElement('div');
tooltip.className = 'tooltip';
tooltip.hidden = true;
document.body.append(tooltip);
for (const el of document.querySelectorAll('.sidebar [data-tip]')) {
  el.addEventListener('mouseenter', () => {
    if (!app.classList.contains('collapsed') || !matchMedia('(hover: hover)').matches) return;
    const r = el.getBoundingClientRect();
    tooltip.textContent = el.dataset.tip;
    tooltip.style.left = `${r.right + 10}px`;
    tooltip.style.top = `${r.top + r.height / 2}px`;
    tooltip.hidden = false;
  });
  el.addEventListener('mouseleave', () => { tooltip.hidden = true; });
  el.addEventListener('click', () => { tooltip.hidden = true; });
}

document.getElementById('sb-collapse').addEventListener('click', () => {
  prefs.sidebar = app.classList.contains('collapsed') ? 'expanded' : 'collapsed';
  savePrefs();
  applySidebar();
});
document.getElementById('menu-button').addEventListener('click', () => setDrawer(true));
document.getElementById('sb-close').addEventListener('click', () => setDrawer(false));
backdrop.addEventListener('click', () => setDrawer(false));
document.getElementById('overlay-menu').addEventListener('click', () => showPanel('machine'));
phone.addEventListener('change', applySidebar);
applySidebar();

/** Rend le clavier à l'écran du TRS-80 (pas sur un écran tactile : le clavier virtuel surgirait). */
function focusScreen() {
  if (!matchMedia('(pointer: coarse)').matches) canvas.focus({ preventScroll: true });
}

/** Après une action qui démarre quelque chose : ferme le tiroir pour montrer l'écran. */
function showScreen() {
  if (phone.matches) setDrawer(false);
  focusScreen();
}

// Toute erreur imprévue est affichée sous l'écran plutôt que de figer la page en silence.
function showStatus(message, isError = false) {
  statusBox.textContent = message;
  statusBox.classList.toggle('error', isError);
}
// Erreur inattendue : le message, et où elle s'est produite (fonction, fichier, ligne), pour
// qu'un signalement permette de la retrouver. La pile complète va dans la console.
function errorPlace(error, fallback = '') {
  const frame = String(error?.stack ?? '').split('\n').find((l) => /\.js|wasm/.test(l) && !/errorPlace/.test(l));
  const where = frame ? frame.trim().replace(/^at\s+/, '').replace(/https?:\/\/[^)\s]*\//, '').replace(/\?v=[^:)]*/, '') : fallback;
  return where ? ` — ${where}` : '';
}
window.addEventListener('error', (e) => {
  console.error(e.error ?? e.message);
  const fallback = e.filename ? `${e.filename.split('/').pop().replace(/\?.*/, '')}:${e.lineno}` : '';
  showStatus(t('error', { msg: `${e.message}${errorPlace(e.error, fallback)}` }), true);
});
window.addEventListener('unhandledrejection', (e) => {
  console.error(e.reason);
  showStatus(t('error', { msg: `${e.reason?.message ?? e.reason}${errorPlace(e.reason)}` }), true);
});

const wasm = await init({ module_or_path: `pkg/trs80_web_bg.wasm${V}` });
// Scripts et WebAssembly fonctionnent : le message « scripts bloqués » de index.html n'a
// plus lieu d'être (même s'il s'est affiché après un chargement très lent).
document.documentElement.classList.remove('no-js', 'js-loading', 'js-failed');
document.documentElement.classList.add('js-ready');

/**
 * Cause d'un téléchargement raté. Une TypeError de fetch() : requête bloquée (bloqueur de
 * publicité, boucliers de Brave) ou réseau indisponible.
 */
function failure(e) {
  return e instanceof TypeError ? t('net.blocked') : (e.message ?? e);
}

let emulator = null;

// ------------------------------------------------------------------ ROM

// IndexedDB conserve la ROM (magasin « roms ») et la bibliothèque de l'utilisateur
// (magasin « files » : disquettes, programmes, listings BASIC). Version 1 : la ROM seulement.
const DB_NAME = 'trs80-emu';
let dbPromise = null;

function openDb() {
  dbPromise ??= new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 3);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains('roms')) db.createObjectStore('roms');
      if (!db.objectStoreNames.contains('files')) {
        db.createObjectStore('files', { keyPath: 'id', autoIncrement: true });
      }
      // Reprise de session : disques en place, par modèle.
      if (!db.objectStoreNames.contains('session')) db.createObjectStore('session');
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  dbPromise.catch(() => { dbPromise = null; });
  return dbPromise;
}

/** Une opération sur un magasin; résolue quand la transaction est terminée. */
async function dbRequest(store, mode, operation) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(store, mode);
    const req = operation(tx.objectStore(store));
    tx.oncomplete = () => resolve(req.result);
    tx.onerror = tx.onabort = () => reject(tx.error);
  });
}

// Valeur conservée : { id, bytes } (id = entrée de roms.json, ou 'custom' pour un fichier
// de l'utilisateur). Les versions précédentes conservaient seulement les octets.
/** Famille de ROM du modèle choisi : 1 (Model I), 2 (Model II) ou 3 (Model III, aussi pour le Model 4). */
function romFamily() {
  return prefs.model === 4 ? 3 : prefs.model;
}

/** Clé de la ROM conservée (la ROM du Model I garde sa clé d'origine). */
function romKey() {
  return { 1: 'level2', 2: 'model2', 3: 'model3' }[romFamily()];
}

async function saveRom(id, bytes) {
  try {
    await dbRequest('roms', 'readwrite', (s) => s.put({ id, bytes }, romKey()));
  } catch { /* stockage indisponible (navigation privée) : on s'en passe */ }
}

async function loadSavedRom() {
  try {
    const value = await dbRequest('roms', 'readonly', (s) => s.get(romKey()));
    if (value instanceof Uint8Array) return { id: 'custom', bytes: value };
    return value ?? null;
  } catch {
    return null;
  }
}

// En développement : une ROM placée dans www/rom/level2.rom (exclue de Git) est chargée d'office.
async function loadDevRom() {
  try {
    const res = await fetch(`rom/${romKey()}.rom`);
    return res.ok ? { id: 'custom', bytes: new Uint8Array(await res.arrayBuffer()) } : null;
  } catch {
    return null;
  }
}

// ROM proposées (roms.json) : téléchargées chez un tiers quand on les choisit, ou au premier
// démarrage pour la ROM par défaut. Aucune ROM du Model II n'est proposée : l'utilisateur
// charge la sienne.
const DEFAULT_ROMS = { 1: 'level2-1.3', 3: 'model3-revc' };
const defaultRom = () => DEFAULT_ROMS[romFamily()];
const romList = document.getElementById('rom-list');
const romTar = document.getElementById('rom-tar');
const romTarMember = document.getElementById('rom-tar-member');
let roms = [];

async function loadRomIndex() {
  try {
    const res = await fetch(`roms.json${V}`);
    roms = res.ok ? await res.json() : [];
  } catch {
    roms = [];
  }
  fillRomList();
}

/** Liste des ROM du modèle choisi (le Model 4 utilise celles du Model III). */
function fillRomList() {
  romList.replaceChildren(romList.options[0]);
  for (const r of roms.filter((x) => (x.model ?? 1) === romFamily())) {
    romList.append(new Option(localized(r, 'title'), r.id));
  }
}

/** Titres de la liste des ROM dans la langue courante. */
function relabelRomList() {
  for (const option of romList.options) {
    const rom = roms.find((r) => r.id === option.value);
    if (rom) option.textContent = localized(rom, 'title');
    else if (option.value === 'custom') option.textContent = t('rom.custom');
  }
}
languageListeners.push(relabelRomList);

/** Affiche la ROM courante dans la liste; un fichier personnel y apparaît comme « Your ROM file ». */
function selectRomInList(id) {
  if (id === 'custom' && !romList.querySelector('option[value="custom"]')) {
    romList.append(new Option(t('rom.custom'), 'custom'));
  }
  romList.value = id ?? '';
}

async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** Contenu d'un membre (comparé sans le chemin) d'une archive tar, ou null. */
function extractFromTar(tar, wanted) {
  const decoder = new TextDecoder();
  for (let off = 0; off + 512 <= tar.length;) {
    const header = tar.subarray(off, off + 512);
    if (header.every((b) => b === 0)) break; // fin de l'archive
    const name = decoder.decode(header.subarray(0, 100)).replace(/\0.*$/s, '');
    const size = parseInt(decoder.decode(header.subarray(124, 136)).replace(/\0.*$/s, '').trim() || '0', 8);
    if (name.split('/').pop().toUpperCase() === wanted.toUpperCase()) {
      return tar.slice(off + 512, off + 512 + size);
    }
    off += 512 + Math.ceil(size / 512) * 512;
  }
  return null;
}

/** Télécharge une ROM de la liste (vérifiée par son empreinte); lève une erreur sinon. */
async function downloadListedRom(rom) {
  const res = await fetch(rom.url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const bytes = new Uint8Array(await res.arrayBuffer());
  if (rom.sha256 && (await sha256Hex(bytes)) !== rom.sha256) throw new Error(t('rom.checksum'));
  return bytes;
}

/**
 * Model I : passe au BASIC Level I (1) ou Level II (2) pour une cassette de l'autre famille.
 * La ROM Level I est conservée à part (clé « level1 »), téléchargée la première fois; la
 * ROM Level II est celle de l'utilisateur, sinon celle par défaut. La ROM choisie dans la
 * liste ne change pas.
 */
let levelSwitch = null; // bascule Level I / Level II en cours (promesse)

async function switchLevel(level) {
  const wanted = roms.find((r) => r.id === (level === 1 ? 'level1' : DEFAULT_ROMS[1]));
  let bytes = null;
  try {
    if (level === 1) {
      bytes = await dbRequest('roms', 'readonly', (s) => s.get('level1')).catch(() => null);
      if (!(bytes instanceof Uint8Array)) {
        showStatus(t('downloading', { name: localized(wanted, 'title') }));
        bytes = await downloadListedRom(wanted);
        dbRequest('roms', 'readwrite', (s) => s.put(bytes, 'level1')).catch(() => {});
      }
    } else {
      const saved = (await loadSavedRom()) ?? (await loadDevRom());
      bytes = saved && saved.bytes.length !== 4096 ? saved.bytes : await downloadListedRom(wanted);
    }
  } catch (e) {
    showStatus(t('download.fail', { name: wanted ? localized(wanted, 'title') : 'ROM', msg: failure(e) }), true);
    return false;
  }
  if (!start(bytes)) return false;
  showStatus(t(level === 1 ? 'rom.toLevel1' : 'rom.toLevel2'));
  return true;
}

async function chooseListedRom(id) {
  const rom = roms.find((r) => r.id === id);
  romTar.hidden = true;
  if (!rom) return;
  if (rom.source === 'tar') {
    romTarMember.textContent = rom.member;
    romTar.hidden = false;
    showStatus(t('rom.tarStatus', { name: localized(rom, 'title') }));
    return;
  }
  showStatus(t('downloading', { name: localized(rom, 'title') }));
  try {
    const bytes = await downloadListedRom(rom);
    if (start(bytes)) {
      saveRom(rom.id, bytes);
      showStatus(t('rom.loaded', { name: localized(rom, 'title') }));
    }
  } catch (e) {
    showStatus(t('download.fail', { name: localized(rom, 'title'), msg: failure(e) }), true);
  }
}

romList.addEventListener('change', () => chooseListedRom(romList.value));

// ------------------------------------------------------------------ modèle

const modelList = document.getElementById('model-list');

/** Titres de la page, listes de ROM et de disquettes du modèle choisi. */
function applyModel() {
  modelList.value = String(prefs.model);
  // L'interface d'expansion (horloge à 40 Hz) n'existe que sur le Model I.
  expansion.closest('label').hidden = prefs.model !== 1;
  lowercaseBox.closest('label').hidden = prefs.model !== 1;
  // Touches à l'écran (appareils sans clavier) : ESC et F1-F2 du Model II, F1-F3 du Model 4.
  for (const b of document.querySelectorAll('#touch-keys [data-models]')) {
    b.hidden = !b.dataset.models.split(' ').includes(String(prefs.model));
  }
  // La carte graphique Radio Shack : Model III et 4.
  hiresBoard.closest('label').hidden = prefs.model !== 3 && prefs.model !== 4;
  // Disque dur et port RS-232 : l'aide du Model II (TRSDOS-HD, SIO) diffère des autres.
  const two = prefs.model === 2 ? '2' : '';
  for (const [id, key] of [['hd-hint', 'hd.hint'], ['modem-help', 'modem.help']]) {
    const el = document.getElementById(id);
    el.dataset.i18nHtml = key + two;
    el.innerHTML = t(key + two);
  }
  hardRows.forEach((r) => r.setModel());
  const name = t(`model.${prefs.model}`);
  document.title = name;
  for (const el of document.querySelectorAll('.machine-name')) el.textContent = name;
  // Message d'accueil (sans ROM) propre au modèle : ROM attendue et sa taille.
  const start = document.getElementById('overlay-start');
  start.dataset.i18nHtml = { 1: 'overlay.start', 2: 'overlay.start2' }[prefs.model] ?? 'overlay.start3';
  start.innerHTML = t(start.dataset.i18nHtml);
  fillRomList();
  if (disks.length) fillDiskList();
}
languageListeners.push(() => {
  const name = t(`model.${prefs.model}`);
  document.title = name;
  for (const el of document.querySelectorAll('.machine-name')) el.textContent = name;
});

/** Autre modèle : la ROM de sa famille (conservée, sinon celle de la liste) le démarre. */
async function changeModel(model) {
  // Les disques du modèle quitté sont enregistrés; ceux du nouveau modèle seront remis.
  await saveSession(true);
  sessionReady = false;
  prefs.model = model;
  savePrefs();
  applyModel();
  romTar.hidden = true;
  hideNowInfo();
  for (const row of driveRows) row.clear();
  showStatus('');
  if (prefs.open.includes('repo')) loadRepo(prefs.repoKind); // fichiers du nouveau modèle
  const saved = (await loadDevRom()) ?? (await loadSavedRom());
  if (saved) {
    if (start(saved.bytes)) selectRomInList(saved.id);
  } else if (!defaultRom()) {
    // Model II : pas de ROM à télécharger; l'émulateur s'arrête jusqu'au choix d'un fichier.
    emulator?.free();
    emulator = null;
    selectRomInList('');
    setRunning(false);
    showStatus(t('rom.ownM2'));
  } else {
    selectRomInList(defaultRom());
    await chooseListedRom(defaultRom());
  }
  await restoreSession();
}
modelList.addEventListener('change', () => changeModel(Number(modelList.value)));

document.getElementById('tar-file').addEventListener('change', async (event) => {
  const file = event.target.files[0];
  event.target.value = '';
  const rom = roms.find((r) => r.id === romList.value);
  if (!file || !rom) return;
  const bytes = extractFromTar(new Uint8Array(await file.arrayBuffer()), rom.member);
  if (!bytes) {
    showStatus(t('rom.notInTar', { member: rom.member, file: file.name }), true);
    return;
  }
  if (start(bytes)) {
    romTar.hidden = true;
    saveRom(rom.id, bytes);
    showStatus(t('rom.loadedFrom', { name: localized(rom, 'title'), file: file.name }));
  }
});

function setRunning(running) {
  overlay.hidden = running;
  pasteButton.disabled = copyButton.disabled = historyButton.disabled = !running;
  resetButton.disabled = !running;
  programList.disabled = !running;
  cmdFile.disabled = !running;
  cmdButton.classList.toggle('disabled', !running);
  typeButton.disabled = !running;
  programsHint.hidden = running;
  diskList.disabled = !running;
  for (const row of driveRows) row.setEnabled(running);
  for (const row of hardRows) row.setEnabled(running);
  renderLibrary();
}

function start(bytes) {
  modem.hangup(false);
  // Images des disques durs (avec les écritures du DOS), à rebrancher sur la nouvelle machine.
  const hardImages = hardRows.map((r) => r.image());
  try {
    emulator?.free();
    emulator = Emulator.with_model(bytes, prefs.model);
  } catch (e) {
    emulator = null;
    errorBox.textContent = t('rom.rejected', { msg: e.message ?? e });
    setRunning(false);
    return false;
  }
  errorBox.textContent = '';
  emulator.set_expansion_interface(expansion.checked);
  emulator.set_lowercase_mod(lowercaseBox.checked);
  emulator.set_graphics_board(hiresBoard.checked);
  if (audioCtx && soundBox.checked) emulator.set_audio_rate(audioCtx.sampleRate);
  // Nouvelle ROM : les disquettes déjà insérées le restent (images d'origine).
  for (const row of driveRows) row.reinsert();
  hardRows.forEach((row, i) => row.reinsert(hardImages[i]));
  setRunning(true);
  refreshStateInfo();
  forgetLine(); // nouvelle machine : la ligne en cours repart de zéro
  hideNowInfo();
  ideReset();
  showScreen();
  return true;
}

document.getElementById('rom-file').addEventListener('change', async (event) => {
  const file = event.target.files[0];
  event.target.value = '';
  if (!file) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (start(bytes)) {
    romTar.hidden = true;
    selectRomInList('custom');
    saveRom('custom', bytes);
    showStatus(t('rom.loaded', { name: file.name }));
  }
});
resetButton.addEventListener('click', () => {
  emulator?.reset();
  forgetLine();
  if (nowKind !== 'disk') hideNowInfo(); // le programme chargé est perdu
  focusScreen();
});

// ------------------------------------------------------------------ programmes

let programs = [];

async function loadProgramIndex() {
  try {
    const res = await fetch(`programs/index.json${V}`);
    programs = res.ok ? await res.json() : [];
  } catch {
    programs = [];
  }
  for (const p of programs) programList.append(new Option('', p.id));
  relabelProgramList();
}

function relabelProgramList() {
  for (const option of programList.options) {
    const p = programs.find((x) => x.id === option.value);
    if (p) option.textContent = `${localized(p, 'title')} (${p.year})`;
  }
}
languageListeners.push(relabelProgramList);

// ------------------------------------------------------------------ programme en cours

// Carte sous l'écran : nom du programme (ou de la disquette) chargé et, si on la connaît,
// sa description. Une « source » ({ name, entry } d'un index, ou { name, header } d'un .CMD)
// est conservée : la carte se recompose dans la langue choisie.
const nowInfo = document.getElementById('now-info');
const ISSUES_URL = 'https://github.com/ve2cuy/trs80-emu/issues';
let nowKind = null;
let nowSource = null;
// Administration (voir plus bas) : état lu par showNowInfo et runFile dès le démarrage.
const ADMIN_CATEGORIES = ['games', 'education', 'finance', 'office', 'programming', 'utilities', 'graphics',
  'music', 'communications', 'science', 'systems', 'demo', 'other'];
const nowAdmin = document.getElementById('now-admin');
let adminPassword = null;
try { adminPassword = sessionStorage.getItem('trs80-admin'); } catch { /* stockage indisponible */ }
let adminPending = [];
let lastRun = null; // dernier programme lancé : { bytes, name, info, disk }
let testing = false;
const testShots = new Map(); // fichier -> [{ model, url }]

/** Textes d'une source : { title, sub, lines: [{ text, muted }] }. */
function describeSource(source) {
  const e = source.entry;
  if (e) {
    const title = localized(e, 'title');
    const controls = localized(e, 'controls');
    return {
      title: title ?? source.name,
      sub: [title ? source.name : null, e.year, e.authors].filter(Boolean).join(' · '),
      lines: [
        { text: localized(e, 'description') },
        { text: controls && t('now.controls', { c: controls }) },
        { text: localized(e, 'license'), muted: true },
      ],
    };
  }
  const header = source.header ?? {};
  return {
    title: source.name,
    sub: header.name ? t('now.module', { name: header.name }) : '',
    lines: [{ text: header.copyright, muted: true }],
  };
}

function showNowInfo(source, kind = 'program') {
  nowKind = kind;
  nowSource = source;
  const info = describeSource(source);
  document.getElementById('now-icon').firstElementChild
    .setAttribute('href', `#i-${kind === 'disk' ? 'disk' : kind === 'basic' ? 'basic' : 'file'}`);
  document.getElementById('now-title').textContent = info.title;
  const sub = document.getElementById('now-sub');
  sub.textContent = info.sub ?? '';
  sub.hidden = !info.sub;
  document.getElementById('now-lines').replaceChildren(...info.lines.filter((l) => l?.text)
    .map((l) => element('p', l.muted ? 'muted' : '', l.text)));
  // Fichier du dépôt : signaler qu'il ne fonctionne pas, ou (ayant droit) demander son
  // retrait. Les liens ouvrent les formulaires de .github/ISSUE_TEMPLATE, avec le programme,
  // le fichier (et le modèle) déjà remplis.
  if (source.repo) {
    const title = localized(source.entry, 'title') ?? source.name;
    const file = `${source.repo.kind}/${source.repo.file}`;
    const issueLink = (key, linkText, fields) => {
      const [before, after = ''] = t(key).split('{link}');
      const link = element('a', '', linkText);
      link.href = `${ISSUES_URL}/new?${new URLSearchParams({ ...fields, program: title, file })}`;
      link.target = '_blank';
      link.rel = 'noopener';
      const p = element('p', 'muted rights');
      p.append(before, link, after);
      return p;
    };
    document.getElementById('now-lines').append(
      issueLink('now.broken', t('now.brokenLink'), {
        template: 'broken-program.yml',
        title: `Ne fonctionne pas / Does not work: ${title}`,
        model: { 1: 'Model I', 2: 'Model II', 3: 'Model III', 4: 'Model 4' }[prefs.model],
      }),
      issueLink('now.rights', ISSUES_URL, {
        template: 'removal-request.yml',
        title: `Demande de retrait / Removal request: ${title}`,
      }),
    );
  }
  nowInfo.hidden = false;
  renderNowAdmin();
}
languageListeners.push(() => { if (nowKind) showNowInfo(nowSource, nowKind); });

function hideNowInfo() {
  nowInfo.hidden = true;
  nowKind = null;
}
document.getElementById('now-close').addEventListener('click', hideNowInfo);

/** Source d'une entrée de programs/index.json, de disks/index.json ou d'un dépôt. */
function entryInfo(e, name = e.file) {
  return { name, entry: e };
}

/** Source d'un fichier du dépôt, avec son genre et son chemin (actions de l'admin). */
function repoInfo(kind, e, name = e.file.split('/').pop()) {
  return { name, entry: e, repo: { kind, file: e.file } };
}

/**
 * Enregistrements d'un fichier .CMD qui le décrivent : en-tête (05h, nom du module) et
 * avis de droit d'auteur (1Fh). Les blocs de chargement (01h) sont sautés.
 */
function cmdHeader(bytes) {
  const found = {};
  const text = (a, b) => new TextDecoder('latin1').decode(bytes.subarray(a, b)).replace(/[\x00-\x1F]+/g, ' ').trim();
  for (let i = 0; i + 1 < bytes.length;) {
    const type = bytes[i];
    let len = bytes[i + 1];
    if (type === 0x01 && len < 3) len += 256; // 0, 1, 2 : 256, 257, 258 octets
    if (type === 0x02) break; // adresse de départ : fin du fichier
    if (type === 0x05) found.name = text(i + 2, i + 2 + len);
    if (type === 0x1F) found.copyright = text(i + 2, i + 2 + len);
    i += 2 + len;
  }
  return found;
}

/** Ce qu'on sait d'un fichier : liste intégrée, dépôt déjà parcouru, sinon le fichier lui-même. */
function describe(name, bytes) {
  const lower = name.toLowerCase();
  const known = programs.find((p) => p.file.toLowerCase() === lower);
  if (known) return entryInfo(known, name);
  for (const entries of repoCache.values()) {
    const e = entries.find((x) => x.file.split('/').pop().toLowerCase() === lower);
    if (e) return entryInfo(e, name);
  }
  if (!/\.cmd$/i.test(name)) return { name };
  const header = cmdHeader(bytes);
  // Un nom de module identique au nom du fichier n'apprend rien.
  if (header.name?.toLowerCase() === lower.replace(/\.cmd$/, '')) delete header.name;
  return { name, header };
}

/**
 * Messages de Rust (en anglais, de forme fixe) dans la langue courante : description d'une
 * cassette chargée ou d'une disquette insérée.
 */
function casMessage(name, message) {
  let m = message.match(/^machine-language tape, started at ([0-9A-F]+)h$/);
  if (m) return t('run.casSystem', { name, addr: m[1] });
  m = message.match(/^BASIC tape \((\d+) bytes\), running$/);
  if (m) return t('run.casBasic', { name, size: m[1] });
  if (message === 'Level I tape, CLOAD typed') return t('run.casLevel1', { name });
  if (message === 'machine-language blocks loaded, no entry point') return t('run.casBlocks', { name });
  if (message === 'data tape put in the recorder') return t('run.casData', { name });
  return `${name}: ${message}`;
}

function diskMessage(desc) {
  const m = desc.match(/^(\S+), (\d+) sectors(, write-protected)?$/);
  return m ? t('disk.desc', { format: m[1], n: m[2] }) + (m[3] ? t('disk.protected') : '') : desc;
}

/**
 * Source EDTASM enregistrée sur cassette (amorce, A5h, D3h, nom de 6 caractères, puis des
 * lignes numérotées en chiffres au bit 7 levé) : le texte à ouvrir dans l'atelier, ou null.
 */
function edtasmTape(bytes) {
  let i = 0;
  while (i < bytes.length && bytes[i] === 0) i += 1;
  const digit = (b) => b >= 0xB0 && b <= 0xB9;
  if (bytes[i] !== 0xA5 || bytes[i + 1] !== 0xD3 || !digit(bytes[i + 8]) || !digit(bytes[i + 12])) return null;
  return bytes.subarray(i + 1);
}

/**
 * Charge un programme selon son extension : cassette .CAS, listing .BAS ou exécutable .CMD,
 * et affiche son nom et sa description (`info`, sinon cherchée par describe).
 */
function runFile(bytes, name, info = null) {
  if (!emulator) return;
  info ??= describe(name, bytes);
  lastRun = { bytes, name, info };
  if (/\.(bas|txt)$/i.test(name)) {
    runBasic(bytes, info);
    return;
  }
  const label = describeSource(info).title;
  const source = /\.cas$/i.test(name) ? edtasmTape(bytes) : null;
  if (source) {
    // Source assembleur sur cassette : ouverte dans l'atelier.
    showIde(true);
    ide.open(source, name.replace(/\.cas$/i, '.asm'));
    showStatus(t('run.casSource', { name: label }));
    return;
  }
  try {
    if (/\.cas$/i.test(name)) {
      showStatus(casMessage(label, emulator.load_cas(bytes)));
      tapeName = label;
    } else {
      const entry = emulator.load_cmd(bytes);
      showStatus(t('run.cmd', { name: label, addr: entry.toString(16).toUpperCase().padStart(4, '0') }));
    }
    showNowInfo(info, /\.cas$/i.test(name) && info.basic ? 'basic' : 'program');
  } catch (e) {
    // Model I : cassette de l'autre BASIC (Level I ou Level II) : on change de ROM, puis on
    // recommence.
    const level = /^Level I tape/.test(e.message) ? 1 : /^Level II tape/.test(e.message) ? 2 : 0;
    if (level && prefs.model === 1 && !info.levelSwitched) {
      levelSwitch = switchLevel(level)
        .then((ok) => ok && runFile(bytes, name, { ...info, levelSwitched: true }))
        .finally(() => { levelSwitch = null; });
      return;
    }
    showStatus(t('run.fail', { name: label, msg: e.message ?? e }), true);
  }
  showScreen();
}

/**
 * Programme BASIC : un fichier enregistré par le BASIC disque (FFh puis les lignes
 * tokenisées) devient une cassette BASIC, chargée comme par CLOAD puis lancée; un listing
 * en texte est tapé au clavier (NEW, les lignes, RUN).
 */
function runBasic(bytes, info) {
  if (bytes[0] === 0xFF) {
    const cas = new Uint8Array(255 + 5 + bytes.length - 1); // amorce, A5h, D3h × 3, nom
    cas.set([0xA5, 0xD3, 0xD3, 0xD3, 0x50], 255);
    cas.set(bytes.subarray(1), 260);
    runFile(cas, 'basic.cas', { ...info, basic: true });
    return;
  }
  const text = new TextDecoder('latin1').decode(bytes).replace(/\r\n?/g, '\n').trim();
  // Comme un .CMD ou une cassette : redémarrage en BASIC (même à « Cass? » ou sous un DOS).
  try {
    emulator.restart_basic();
  } catch (e) {
    showStatus(t('run.fail', { name: describeSource(info).title, msg: e.message ?? e }), true);
    return;
  }
  emulator.type_text(`NEW\n${text}\nRUN\n`);
  showStatus(t('run.basText', { name: describeSource(info).title }));
  showNowInfo(info, 'basic');
  showScreen();
}

async function runProgram(id) {
  const p = programs.find((x) => x.id === id);
  if (!p) return;
  try {
    const res = await fetch(`programs/${p.file}${V}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    runFile(new Uint8Array(await res.arrayBuffer()), p.file, entryInfo(p));
  } catch (e) {
    showStatus(t('download.fail', { name: localized(p, 'title'), msg: failure(e) }), true);
  }
}

programList.addEventListener('change', () => runProgram(programList.value));

cmdFile.addEventListener('change', async (event) => {
  const file = event.target.files[0];
  if (!file) return;
  programList.value = '';
  const bytes = new Uint8Array(await file.arrayBuffer());
  runFile(bytes, file.name);
  event.target.value = '';
  if (prefs.keepFiles) keepFile(file.name, bytes);
});

// ------------------------------------------------------------------ disquettes

const diskList = document.getElementById('disk-list');
let disks = [];

/** Élément créé avec sa classe et son texte. */
function element(tag, className, text) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (text) el.textContent = text;
  return el;
}

/** Élément dont le texte (clé de i18n.js) suit la langue choisie. */
function textElement(tag, className, key) {
  const el = element(tag, className, t(key));
  el.dataset.i18n = key;
  return el;
}

/** Infobulle (title) qui suit la langue choisie. */
function setTip(el, key) {
  el.title = t(key);
  el.dataset.i18nTitle = key;
}

/** Icône du sprite de index.html. */
function icon(name) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', 'icon');
  const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
  use.setAttribute('href', `#i-${name}`);
  svg.append(use);
  return svg;
}

// Infobulles des icônes et des cases des lecteurs : affichées par la page plutôt que par le
// navigateur, dont l'infobulle (attribut title) ne revient pas toujours quand on passe d'une
// icône à sa voisine. Le texte reste dans title (lecteurs d'écran, traduction); il est mis
// de côté pendant le survol pour que le navigateur n'affiche pas aussi la sienne.
const TIP_SELECTOR = '.icon-button, .drive-chip .chip-main';
const TIP_DELAY_MS = 350;
const tipBox = element('div', 'hover-tip');
tipBox.setAttribute('role', 'tipBox');
tipBox.hidden = true;
document.body.append(tipBox);
let tipTarget = null;
let tipTimer = 0;

function tipText(el) {
  return el.dataset.tipText ?? el.title ?? '';
}

function placeTooltip(el) {
  const r = el.getBoundingClientRect();
  const t = tipBox.getBoundingClientRect();
  const margin = 6;
  let top = r.bottom + margin;
  if (top + t.height > window.innerHeight - 4) top = r.top - t.height - margin;
  const left = Math.min(Math.max(4, r.left + r.width / 2 - t.width / 2), window.innerWidth - t.width - 4);
  tipBox.style.top = `${Math.max(4, top)}px`;
  tipBox.style.left = `${left}px`;
}

function showTooltip(el) {
  const text = tipText(el);
  if (!text || !el.isConnected) return;
  tipBox.textContent = text;
  tipBox.hidden = false;
  placeTooltip(el);
}

function releaseTip() {
  if (tipTarget && tipTarget.dataset.tipText != null) {
    // Le texte a pu changer pendant le survol (langue, état) : celui de title l'emporte.
    if (!tipTarget.title) tipTarget.title = tipTarget.dataset.tipText;
    delete tipTarget.dataset.tipText;
  }
  tipTarget = null;
}

// Moment où la dernière infobulle a été cachée : la souris passe de l'icône à sa voisine
// (sortie de l'une, puis entrée dans l'autre) sans attendre de nouveau le délai.
let tipHiddenAt = 0;
const TIP_CHAIN_MS = 400;

function hideTooltip() {
  clearTimeout(tipTimer);
  releaseTip();
  if (!tipBox.hidden) tipHiddenAt = performance.now();
  tipBox.hidden = true;
}

document.addEventListener('pointerover', (event) => {
  const el = event.target.closest?.(TIP_SELECTOR);
  if (el === tipTarget) return;
  const chained = !tipBox.hidden || performance.now() - tipHiddenAt < TIP_CHAIN_MS;
  hideTooltip();
  if (!el || el.disabled || !el.title) return;
  tipTarget = el;
  el.dataset.tipText = el.title;
  el.title = '';
  // D'une icône à sa voisine : tout de suite; sinon après un court délai.
  if (chained) showTooltip(el);
  else tipTimer = setTimeout(() => { if (tipTarget === el) showTooltip(el); }, TIP_DELAY_MS);
});
document.addEventListener('pointerout', (event) => {
  if (tipTarget && !tipTarget.contains(event.relatedTarget)) hideTooltip();
});
for (const type of ['pointerdown', 'keydown', 'scroll', 'blur']) window.addEventListener(type, hideTooltip, true);

/** Bouton icône; `key` (i18n.js) donne son infobulle et son nom pour les lecteurs d'écran. */
function iconButton(name, key, extra = '') {
  const button = element('button', `icon-button small ${extra}`);
  button.type = 'button';
  setTip(button, key);
  button.setAttribute('aria-label', button.title);
  button.dataset.i18nAria = key;
  button.append(icon(name));
  return button;
}

/** Liste « Lecteur… » pour insérer une disquette dans les lecteurs 1 à 3. */
/**
 * Commande « Mettre dans… », la même partout (disquettes de la liste, bibliothèque, dépôt) :
 * lecteur 0 (le TRS-80 redémarre dessus), lecteurs 1 à 3, ou disque dur HD1 / HD2 pour une
 * image .hdv. `getBytes` fournit le contenu (téléchargé au besoin), ou null en cas d'échec.
 */
function placeSelect(name, getBytes, { libraryId = null, info = null } = {}) {
  const hard = /\.hdv$/i.test(name);
  const into = element('select', 'place');
  setTip(into, hard ? 'place.hard.tip' : 'place.tip');
  const keys = hard ? ['place.h0', 'place.h1'] : ['place.d0', 'place.d1', 'place.d2', 'place.d3'];
  const option = (key, value) => {
    const o = new Option(t(key), value);
    o.dataset.i18n = key;
    return o;
  };
  into.append(option('place', ''), ...keys.map((k) => option(k, k.slice(-2))));
  into.addEventListener('change', async () => {
    const target = into.value;
    into.value = '';
    if (!target) return;
    if (!emulator) {
      showStatus(t('prog.hint'), true);
      return;
    }
    const bytes = await getBytes();
    if (!bytes) return;
    const n = Number(target[1]);
    if (target[0] === 'h') {
      if (hardRows[n].insert(name, bytes, libraryId)) focusScreen();
    } else if (n === 0) {
      if (driveRows[0].insertAndBoot(name, bytes, libraryId, info)) {
        lastRun = { bytes, name, info, disk: true };
        showScreen();
      }
    } else if (driveRows[n].insert(name, bytes, libraryId)) {
      focusScreen();
    }
  });
  return into;
}

function downloadFile(bytes, fileName) {
  download(bytes, fileName);
}

function download(bytes, fileName) {
  const link = document.createElement('a');
  link.href = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
  link.download = fileName;
  link.click();
  setTimeout(() => URL.revokeObjectURL(link.href), 1000);
}

/** Une carte par lecteur : nom de l'image, Insert…, Blank, Eject, Keep, Download. */
function makeDriveRow(drive) {
  const row = element('div', 'drive');
  const head = element('div', 'drive-head');
  const name = element('span', 'drive-name');
  head.append(element('span', 'drive-no', String(drive)), name);
  const actions = element('div', 'drive-actions');
  const insertLabel = element('label', 'button secondary');
  insertLabel.append(textElement('span', '', 'drive.insert'));
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.dsk,.dmk,.jv1,.jv3,.imd';
  input.hidden = true;
  insertLabel.append(input);
  const blank = textElement('button', 'secondary', 'drive.blank');
  blank.type = 'button';
  setTip(blank, 'drive.blank.tip');
  const eject = iconButton('eject', 'drive.eject');
  const keep = iconButton('folder-plus', 'drive.keep');
  const save = iconButton('download', 'drive.download');
  // Fichiers et formatage (les mêmes outils que dans la barre des lecteurs, avec un texte).
  const filesButton = textElement('button', 'secondary', 'drive.files');
  filesButton.type = 'button';
  setTip(filesButton, 'files.tip');
  filesButton.addEventListener('click', () => openFilesDialog(drive));
  const formatButton = textElement('button', 'secondary', 'drive.format');
  formatButton.type = 'button';
  setTip(formatButton, 'fmt.tip');
  formatButton.addEventListener('click', () => openFormatDialog(drive));
  // Erreur d'insertion, affichée sur la ligne du lecteur pour ne pas passer inaperçue.
  const error = element('span', 'drive-error');
  error.setAttribute('role', 'alert');
  actions.append(insertLabel, blank, formatButton, filesButton, eject, keep, save);
  row.append(head, actions, error);
  document.getElementById('drives').append(row);

  let current = null; // { name, bytes, libraryId } : image d'origine
  let version = 0; // change à chaque insertion ou éjection (reprise de session)

  function refresh() {
    name.textContent = current ? current.name : t('drive.empty');
    name.dataset.modified = t('drive.modified');
    name.title = current ? current.name : '';
    name.classList.toggle('empty', !current);
    name.classList.toggle('modified', !!(current && emulator?.disk_modified(drive)));
    eject.disabled = keep.disabled = save.disabled = filesButton.disabled = !current || !emulator;
    formatButton.disabled = !emulator || !JSON.parse(Emulator.disk_formats(prefs.model)).length;
  }

  /** Image actuelle (avec les écritures du DOS) et son nom de fichier, ou null. */
  function currentImage() {
    const image = emulator?.disk_image(drive);
    if (!image) {
      if (current?.bytes && !emulator?.disk_modified(drive)) return { bytes: current.bytes, name: current.name };
      showStatus(t('drive.saveOnly'), true);
      return null;
    }
    // Une disquette reformatée ou une image DMK est enregistrée en JV3 : extension .dsk.
    const format = emulator.disk_image_format(drive);
    const fileName = format === 'JV3' ? current.name.replace(/\.(dmk|jv1)$/i, '.dsk') : current.name;
    return { bytes: image, name: fileName };
  }

  function insert(fileName, bytes, libraryId = null) {
    if (!emulator) return false;
    try {
      const desc = emulator.insert_disk(drive, bytes);
      current = { name: fileName, bytes, libraryId };
      version++;
      error.textContent = '';
      showStatus(t('drive.inserted', { n: drive, name: fileName, desc: diskMessage(desc) }));
      refresh();
      return true;
    } catch (e) {
      const message = t('drive.notInserted', { name: fileName, msg: e.message ?? e });
      error.textContent = current ? t('drive.stillIn', { msg: message, name: current.name }) : message;
      showStatus(t('drive.insertFail', { name: fileName, msg: e.message ?? e }), true);
      return false;
    }
  }

  /** Insère une disquette; au lecteur 0, redémarre aussitôt dessus. */
  function insertAndBoot(fileName, bytes, libraryId = null, info = null) {
    // Lecteurs 1 à 3 : des disquettes de données, insérées pendant que le DOS tourne.
    if (!insert(fileName, bytes, libraryId)) return false;
    if (drive === 0) {
      emulator.reset();
      showStatus(t('drive.booting', { name: fileName }));
      const known = disks.find((d) => d.file.split('/').pop().toLowerCase() === fileName.toLowerCase());
      showNowInfo(info ?? (known ? entryInfo(known, fileName) : { name: fileName }), 'disk');
    }
    return true;
  }

  input.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    event.target.value = '';
    if (!file) return;
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (insertAndBoot(file.name, bytes) && prefs.keepFiles) {
      const id = await keepFile(file.name, bytes);
      if (current?.bytes === bytes) current.libraryId = id;
    }
    showScreen();
  });
  blank.addEventListener('click', () => {
    if (!emulator) return;
    emulator.insert_blank_disk(drive);
    current = { name: `blank-${drive}.dsk`, bytes: null, libraryId: null };
    version++;
    error.textContent = '';
    showStatus(t('drive.blankStatus', { n: drive }));
    refresh();
    focusScreen();
  });
  function ejectDisk() {
    emulator?.eject_disk(drive);
    current = null;
    version++;
    error.textContent = '';
    refresh();
    focusScreen();
  }
  eject.addEventListener('click', ejectDisk);
  keep.addEventListener('click', async () => {
    const image = currentImage();
    if (!image) return;
    // La disquette vient de la bibliothèque : son entrée est mise à jour.
    current.libraryId = await keepFile(image.name, image.bytes, { id: current.libraryId });
    if (current.libraryId != null) showStatus(t('lib.kept', { name: image.name }));
  });
  save.addEventListener('click', () => {
    const image = currentImage();
    if (image) download(image.bytes, image.name);
  });

  refresh();
  return {
    insert,
    insertAndBoot,
    refresh,
    setEnabled(on) {
      insertLabel.classList.toggle('disabled', !on);
      input.disabled = !on;
      blank.disabled = !on;
      refresh();
    },
    reinsert() {
      if (current?.bytes) insert(current.name, current.bytes, current.libraryId);
      else if (current) emulator.insert_blank_disk(drive);
      refresh();
    },
    hasDisk: () => !!current,
    eject: ejectDisk,
    /** Barre des lecteurs : nom de la disquette et écritures du DOS, ou null. */
    contents: () => (current ? { name: current.name, modified: !!emulator?.disk_modified(drive) } : null),
    /** Vide le lecteur (autre modèle : ses disquettes ne conviennent plus). */
    clear() {
      current = null;
      version++;
      error.textContent = '';
      refresh();
    },
    /** État restauré : la disquette est déjà dans l'émulateur, on reprend son nom. */
    adopt(fileName) {
      const bytes = fileName && emulator ? emulator.disk_image(drive) : null;
      current = bytes ? { name: fileName, bytes, libraryId: null } : null;
      version++;
      error.textContent = '';
      refresh();
    },
    /** Reprise de session : ce qui identifie le contenu actuel (nom, insertion, écritures). */
    signature: () => (current ? `${current.name}#${version}#${emulator?.disk_writes(drive) ?? 0}` : '-'),
    /** Reprise de session : l'image actuelle, avec les écritures du DOS. */
    snapshot() {
      if (!current || !emulator) return null;
      const modified = emulator.disk_modified(drive) || !current.bytes;
      const bytes = modified ? emulator.disk_image(drive) : current.bytes;
      return bytes ? { name: current.name, bytes } : null;
    },
  };
}

const driveRows = [0, 1, 2, 3].map(makeDriveRow);

// ------------------------------------------------------------------ disques durs

// Contrôleur Radio Shack (WD1010) : deux unités, les adresses 1 et 2 du pilote RSHARD (Model
// I, III et 4) ou les unités 0 et 1 de TRSDOS-HD (Model II). Images au format Reed (.hdv),
// comme xtrs, trs80gp et FreHD.

function makeHardRow(unit) {
  const row = element('div', 'drive');
  const head = element('div', 'drive-head');
  const name = element('span', 'drive-name');
  head.append(element('span', 'drive-no', `HD${unit + 1}`), name);
  const actions = element('div', 'drive-actions');
  const insertLabel = element('label', 'button secondary');
  insertLabel.append(textElement('span', '', 'drive.insert'));
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.hdv';
  input.hidden = true;
  insertLabel.append(input);
  const blank = textElement('button', 'secondary', 'hd.new');
  blank.type = 'button';
  const eject = iconButton('eject', 'drive.eject');
  const keep = iconButton('folder-plus', 'hd.keep');
  const save = iconButton('download', 'hd.download');
  const error = element('span', 'drive-error');
  error.setAttribute('role', 'alert');
  actions.append(insertLabel, blank, eject, keep, save);
  row.append(head, actions, error);
  document.getElementById('hard-drives').append(row);

  let current = null; // { name, bytes, libraryId, model2 } : image d'origine (null pour un disque neuf)
  let version = 0; // change à chaque insertion ou éjection (reprise de session)

  function refresh() {
    name.textContent = current ? current.name : t('drive.empty');
    name.dataset.modified = t('hd.modified');
    name.title = current ? current.name : '';
    name.classList.toggle('empty', !current);
    name.classList.toggle('modified', !!(current && emulator?.hard_disk_modified(unit)));
    eject.disabled = keep.disabled = save.disabled = !current || !emulator;
  }

  function insert(fileName, bytes, libraryId = null) {
    if (!emulator) return false;
    try {
      const desc = emulator.insert_hard_disk(unit, bytes);
      // Model II : secteurs de 512 octets, une image incompatible avec les autres modèles.
      current = { name: fileName, bytes, libraryId, model2: prefs.model === 2 };
      version++;
      error.textContent = '';
      showStatus(t('hd.inserted', { n: unit + 1, name: fileName, desc }));
      refresh();
      return true;
    } catch (e) {
      error.textContent = t('drive.notInserted', { name: fileName, msg: e.message ?? e });
      showStatus(t('drive.insertFail', { name: fileName, msg: e.message ?? e }), true);
      return false;
    }
  }

  /** Image actuelle, avec les écritures du DOS. */
  function currentImage() {
    const image = emulator?.hard_disk_image(unit);
    return image ? { bytes: image, name: current.name } : null;
  }

  input.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    event.target.value = '';
    if (!file) return;
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (insert(file.name, bytes) && prefs.keepFiles) current.libraryId = await keepFile(file.name, bytes);
    focusScreen();
  });
  blank.addEventListener('click', () => {
    if (!emulator) return;
    // 306 cylindres, 4 têtes : les valeurs que propose RSHARD (10 Mo). Model II : le disque
    // Tandy de 8,4 Mo (256 cylindres, 4 têtes), que TRSDOS-HD formate avec INIT.
    const two = prefs.model === 2;
    const bytes = two ? Emulator.blank_hard_disk(256, 4) : Emulator.blank_hard_disk(306, 4);
    if (insert(`hard${unit + 1}.hdv`, bytes)) showStatus(t(two ? 'hd.newStatus2' : 'hd.newStatus', { n: unit + 1 }));
    focusScreen();
  });
  function ejectHard() {
    emulator?.eject_hard_disk(unit);
    current = null;
    version++;
    error.textContent = '';
    refresh();
    focusScreen();
  }
  eject.addEventListener('click', ejectHard);
  keep.addEventListener('click', async () => {
    const image = currentImage();
    if (!image) return;
    current.libraryId = await keepFile(image.name, image.bytes, { id: current.libraryId });
    if (current.libraryId != null) showStatus(t('lib.kept', { name: image.name }));
  });
  save.addEventListener('click', () => {
    const image = currentImage();
    if (image) download(image.bytes, image.name);
  });

  refresh();
  return {
    insert,
    refresh,
    setEnabled(on) {
      insertLabel.classList.toggle('disabled', !on);
      input.disabled = !on;
      blank.disabled = !on;
      refresh();
    },
    /** Nouvelle machine (autre ROM, autre modèle) : le disque dur y reste branché, avec
     *  ses écritures, sauf entre le Model II et les autres (formats différents). */
    reinsert(image) {
      if (current && image && current.model2 === (prefs.model === 2)) {
        try { emulator.insert_hard_disk(unit, image); } catch { current = null; }
      } else if (current) {
        current = null;
        version++;
      }
      refresh();
    },
    /** Infobulle du bouton Nouveau selon le modèle. */
    setModel() {
      setTip(blank, prefs.model === 2 ? 'hd.new.tip2' : 'hd.new.tip');
    },
    image: () => (current ? emulator?.hard_disk_image(unit) : null),
    /** État restauré : le disque est déjà branché, on reprend son nom. */
    adopt(fileName) {
      current = fileName ? { name: fileName, bytes: null, libraryId: null, model2: prefs.model === 2 } : null;
      version++;
      error.textContent = '';
      refresh();
    },
    contents: () => (current ? { name: current.name, modified: !!emulator?.hard_disk_modified(unit) } : null),
    eject: ejectHard,
    signature: () => (current ? `${current.name}#${version}#${emulator?.hard_disk_writes(unit) ?? 0}` : '-'),
    snapshot() {
      const bytes = current ? emulator?.hard_disk_image(unit) : null;
      return bytes ? { name: current.name, bytes } : null;
    },
  };
}

const hardRows = [0, 1].map(makeHardRow);
languageListeners.push(() => hardRows.forEach((r) => r.refresh()));
setInterval(() => hardRows.forEach((r) => r.refresh()), 1000);

// ------------------------------------------------------------------ barre des lecteurs

// Sous l'écran : ce qui est dans les lecteurs 0 à 3 et les disques durs branchés (● : écrit
// par le DOS), leur géométrie, et pendant un accès l'opération, la piste et le secteur. Un
// clic ouvre la section Disquettes.
const driveBar = document.getElementById('drive-bar');
let driveBarState = '';
/** Dernier accès vu, par lecteur (« 0 » à « 3 », « HD1 », « HD2 ») : { text, at }. */
const lastAccess = new Map();
let lastDiskCommand = null;
let lastHardCommand = null;
const ACCESS_SHOWN_MS = 1500;

/** Opération d'une commande du contrôleur de disquettes (clé geo.*), ou null. */
function fdcOperation(cmd) {
  if (cmd < 0x10) return 'restore';
  if (cmd < 0x80) return 'seek';
  if (cmd < 0xA0) return 'read';
  if (cmd < 0xC0) return 'write';
  if (cmd < 0xD0) return 'id';
  if (cmd < 0xE0) return null; // interruption forcée
  return cmd < 0xF0 ? 'read' : 'format';
}

/** Opération d'une commande du WD1010 (disque dur), ou null. */
function hardOperation(cmd) {
  return { 0x10: 'restore', 0x20: 'read', 0x30: 'write', 0x40: 'read', 0x50: 'format', 0x70: 'seek' }[cmd & 0xF0] ?? null;
}

/** Relève le dernier accès des contrôleurs (souvent : un accès est bref). */
function watchAccess() {
  if (!emulator) return;
  const d = emulator.disk_position();
  if (d.length && d[0] !== lastDiskCommand) {
    if (lastDiskCommand !== null) {
      const [, drive, track, sector, cmd] = d;
      const op = fdcOperation(cmd);
      if (op) {
        // Déplacement de la tête : la piste seulement.
        const where = op === 'seek' ? t('geo.track', { t: track }) : t('geo.pos', { t: track, s: sector });
        lastAccess.set(String(drive), { text: op === 'restore' ? t('geo.restore') : `${t(`geo.${op}`)} · ${where}`, at: performance.now() });
      }
    }
    lastDiskCommand = d[0];
  }
  const h = emulator.hard_position();
  if (h.length && h[0] !== lastHardCommand) {
    if (lastHardCommand !== null) {
      const [, unit, cyl, head, sector, cmd] = h;
      const op = hardOperation(cmd);
      if (op) {
        const where = op === 'seek' ? t('geo.cyl', { c: cyl }) : t('geo.hpos', { c: cyl, h: head, s: sector });
        lastAccess.set(`HD${unit + 1}`, { text: op === 'restore' ? t('geo.restore') : `${t(`geo.${op}`)} · ${where}`, at: performance.now() });
      }
    }
    lastHardCommand = h[0];
  }
}
setInterval(watchAccess, 50);

/**
 * Famille de format d'une disquette, d'après sa géométrie (null : vierge ou inconnue). Deux
 * familles différentes ne se lisent pas : TRSDOS-II 2.0 et 4.x (8 pouces, secteurs de 256
 * ou 512 octets), TRSDOS 1.3 du Model III et 2.7DD du Model I (secteurs numérotés à partir
 * de 1), LDOS / LS-DOS / TRSDOS 6 et les DOS du Model I qui leur ressemblent (à partir de 0).
 */
function diskFamily(g) {
  if (!g?.tracks) return null;
  const [, size, , first] = g.t;
  const [n0, size0] = g.t0;
  if (g.tracks >= 70 || (n0 === 26 && size0 === 128)) return size === 512 ? 'trsdos2-4' : 'trsdos2-2';
  return first === 1 ? 'trsdos13' : 'ldos';
}

/** Géométrie d'une disquette (objet), ou null sans disquette. */
function diskGeometryOf(drive) {
  const json = emulator?.disk_geometry(drive);
  return json ? JSON.parse(json) : null;
}

/** Géométrie d'une disquette (texte), ou '' sans disquette. */
function diskGeometry(drive) {
  const g = diskGeometryOf(drive);
  if (!g) return '';
  if (!g.tracks) return t('geo.blank');
  const dens = (dd) => (dd ? 'DD' : 'SD');
  const [n, size, dd] = g.t;
  let text = t(g.sides > 1 ? 'geo.sides' : 'geo.disk', { tracks: g.tracks, sides: g.sides, n, size, dens: dens(dd) });
  const [n0, size0, dd0] = g.t0;
  if (n0 !== n || size0 !== size || dd0 !== dd) text += t('geo.t0', { n: n0, size: size0, dens: dens(dd0) });
  return text;
}

/** Géométrie d'un disque dur (texte), ou ''. */
function hardGeometry(unit) {
  const g = emulator?.hard_geometry(unit);
  return g?.length ? t('geo.hard', { cyl: g[0], heads: g[1] }) : '';
}

// Magnétophone : une cassette dont le programme lit lui-même la suite (chargeur à plusieurs
// étapes) reste en place; la barre montre l'avancement et le temps qui reste (500 bauds :
// environ 62 octets par seconde, 31 à 250 bauds pour le Level I).
let tapeName = null;

function tapeChip() {
  const p = emulator?.tape_progress();
  if (!p?.length) return null;
  const [pos, len, motor, rate] = p;
  // Cassette lue en entier (ou presque : quelques octets de remplissage) : plus rien à montrer.
  if (!motor && len - pos <= 16) return null;
  const left = Math.ceil((len - pos) / rate);
  const time = `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`;
  const pct = Math.floor((pos * 100) / len);
  return {
    no: 'CAS', c: { name: tapeName ?? t('bar.tape') }, tape: pct,
    geo: t('bar.tapeLeft', { pct, time }) + (motor ? '' : ` · ${t('bar.tapeStopped')}`),
    clash: '', access: motor ? t('bar.tapeRead') : '',
    eject: () => emulator.eject_tape(),
  };
}

// DOS de chaque disquette (module dos du cœur), recalculé quand son contenu change.
const dosCache = new Map();

// Supports des fichiers : lecteurs 0 à 3 (disquettes), 4 et 5 (disques durs HD1 et HD2).
const HARD_BASE = 4;
function volumeRow(i) {
  return i < HARD_BASE ? driveRows[i] : hardRows[i - HARD_BASE];
}
/** « :0 » pour une disquette, « HD1 » pour un disque dur. */
function volumeLabel(i) {
  return i < HARD_BASE ? `:${i}` : `HD${i - HARD_BASE + 1}`;
}

/** DOS de la disquette du lecteur (4 et 5 : disques durs) : { name, family, system, label,
 *  files, writable }, ou null. Disque dur : recalculé au plus toutes les 2 s pendant que le
 *  DOS y écrit (il faut relire tout le disque). */
function driveDos(i) {
  const sig = volumeRow(i).signature();
  const cached = dosCache.get(i);
  if (cached?.sig === sig) return cached.dos;
  if (cached && i >= HARD_BASE && performance.now() - cached.at < 2000) return cached.dos;
  const json = emulator?.disk_dos(i);
  const dos = json ? JSON.parse(json) : null;
  dosCache.set(i, { sig, dos, at: performance.now() });
  return dos;
}

/** « LDOS 5.3 · système » (ou données). */
function dosText(dos) {
  if (!dos) return '';
  const kind = dos.system === true ? t('dos.system') : dos.system === false ? t('dos.data') : '';
  return kind ? `${dos.name} · ${kind}` : dos.name;
}

function renderDriveBar() {
  const now = performance.now();
  // Famille du DOS du lecteur 0 : une disquette d'une autre famille aux lecteurs 1 à 3 est
  // signalée (en rouge).
  const familyOf = (i) => driveDos(i)?.family ?? diskFamily(diskGeometryOf(i));
  const system = driveRows[0].contents() ? familyOf(0) : null;
  const canFormat = !!emulator && JSON.parse(Emulator.disk_formats(prefs.model)).length > 0;
  const chips = [
    ...driveRows.map((r, i) => {
      const c = r.contents();
      const dos = c ? driveDos(i) : null;
      const family = c ? familyOf(i) : null;
      const clash = i > 0 && system && family && family !== system;
      return {
        no: String(i), c, eject: () => r.eject(), geo: c ? diskGeometry(i) : '',
        // Une disquette vierge (non formatée) n'a pas de DOS : sa géométrie le dit déjà.
        dos: c && (dos || diskGeometryOf(i)?.tracks) ? (dos ? dosText(dos) : t('dos.unknown')) : '',
        system: dos?.system === true,
        clash: clash ? t('bar.clash', { disk: t(`family.${family}`), system: t(`family.${system}`) }) : '',
        // Outils : DOS compatible (disquette sans système au lecteur 0), fichiers, formatage.
        boot: i === 0 && !!c && dos?.system !== true && !!compatibleDos(dos),
        files: !!c && !!dos?.files,
        format: canFormat,
        drive: i,
      };
    }),
    ...hardRows.map((r, u) => {
      const c = r.contents();
      const dos = c ? driveDos(HARD_BASE + u) : null;
      return {
        no: `HD${u + 1}`, c, eject: () => r.eject(), geo: hardGeometry(u), clash: '',
        dos: dos ? dosText(dos) : '', system: dos?.system === true,
        // Outil : les fichiers du disque dur (LDOS, LS-DOS, TRSDOS 6; TRSDOS-II en lecture).
        boot: false, files: !!dos?.files, format: false, drive: HARD_BASE + u,
      };
    }).filter((x) => x.c),
  ].map((x) => {
    const a = lastAccess.get(x.no);
    return { ...x, access: a && now - a.at < ACCESS_SHOWN_MS ? a.text : '' };
  });
  const tape = tapeChip();
  if (tape) chips.push(tape);
  const state = JSON.stringify([document.documentElement.lang, !!emulator, chips]);
  if (state === driveBarState) return;
  driveBarState = state;
  driveBar.hidden = !emulator;
  // Les cases restent en place d'une mise à jour à l'autre (seuls leurs textes changent) :
  // recréer les boutons sous la souris empêcherait leur infobulle de s'afficher.
  for (const data of chips) {
    let rec = chipElements.get(data.no);
    if (!rec) {
      rec = makeChip(data.no);
      chipElements.set(data.no, rec);
    }
    updateChip(rec, data);
  }
  const wanted = chips.map((x) => chipElements.get(x.no).chip);
  const current = [...driveBar.children];
  if (wanted.length !== current.length || wanted.some((el, i) => el !== current[i])) driveBar.replaceChildren(...wanted);
}

/** Cases de la barre des lecteurs, par numéro (« 0 » à « 3 », « HD1 », « CAS »). */
const chipElements = new Map();

function makeChip(no) {
  const chip = element('div', 'drive-chip');
  // Le corps ouvre la section Disquettes; les outils et le bouton d'éjection suivent.
  const main = element('button', 'chip-main');
  main.type = 'button';
  const head = element('span', 'chip-head');
  const file = element('span', 'file');
  head.append(element('span', 'no', no), file);
  const lines = element('span', 'chip-lines');
  main.append(head, lines);
  const tools = element('span', 'chip-tools');
  const eject = iconButton('eject', 'drive.eject');
  const rec = { chip, main, file, lines, tools, eject, data: null, toolsKey: null };
  main.addEventListener('click', () => { if (rec.data?.tape === undefined) showPanel('disks'); });
  eject.addEventListener('click', () => {
    rec.data?.eject();
    renderDriveBar();
  });
  chip.append(main, tools, eject);
  return rec;
}

function updateChip(rec, data) {
  const { c, tape, geo, dos, system: isSystem, clash, access, boot, files, format, drive } = data;
  rec.data = data;
  const { chip, main } = rec;
  chip.classList.toggle('tape', tape !== undefined);
  if (tape !== undefined) chip.style.setProperty('--progress', `${tape}%`);
  chip.classList.toggle('empty', !c);
  chip.classList.toggle('modified', !!c?.modified);
  chip.classList.toggle('active', !!access);
  chip.classList.toggle('clash', !!clash);
  const title = [c ? c.name : t('bar.empty'), dos, geo, clash, tape === undefined && t('bar.open')].filter(Boolean).join('\n');
  // Pendant le survol, le texte de l'infobulle est mis de côté (voir les infobulles).
  if (main.dataset.tipText != null) {
    if (main.dataset.tipText !== title) {
      main.dataset.tipText = title;
      if (tipTarget === main && !tipBox.hidden) tipBox.textContent = title;
    }
  } else if (main.title !== title) main.title = title;
  const name = c ? c.name : t('bar.empty');
  if (rec.file.textContent !== name) rec.file.textContent = name;
  // Lignes : DOS, géométrie, incompatibilité, accès en cours.
  const lines = [[dos, `dos${isSystem ? ' system' : ''}`], [geo, 'geo'], [clash, 'clash-text'], [access, 'access']].filter(([text]) => text);
  const linesKey = JSON.stringify(lines);
  if (rec.lines.dataset.key !== linesKey) {
    rec.lines.dataset.key = linesKey;
    rec.lines.replaceChildren(...lines.map(([text, cls]) => element('span', cls, text)));
  }
  // Outils : recréés seulement quand ils changent (insertion, éjection, autre DOS).
  const toolsKey = JSON.stringify([drive, boot, files, format, document.documentElement.lang]);
  if (rec.toolsKey !== toolsKey) {
    rec.toolsKey = toolsKey;
    const buttons = [];
    const tool = (icon, key, handler) => {
      const b = iconButton(icon, key);
      b.addEventListener('click', handler);
      buttons.push(b);
    };
    if (drive !== undefined) {
      if (boot) tool('power', 'dos.boot', () => bootWithDos());
      if (files) tool('files', 'files.tip', () => openFilesDialog(drive));
      if (format) tool('format', 'fmt.tip', () => openFormatDialog(drive));
    }
    rec.tools.replaceChildren(...buttons);
    rec.tools.hidden = !buttons.length;
    // Infobulle de l'éjection dans la langue choisie.
    setTip(rec.eject, 'drive.eject');
    rec.eject.setAttribute('aria-label', rec.eject.title);
  }
  rec.eject.hidden = !c;
  // Sans outil ni éjection, le corps garde une marge à droite.
  chip.classList.toggle('bare', rec.tools.hidden && rec.eject.hidden);
}
setInterval(renderDriveBar, 200);
languageListeners.push(renderDriveBar);

// ------------------------------------------------------------------ reprise de session

// Les disquettes et les disques durs en place, avec les écritures du DOS (SYSGEN, fichiers
// copiés...), sont enregistrés dans IndexedDB pour chaque modèle (clé « model-<n> ») au fil
// de l'utilisation. Au rechargement de la page (ou au retour sur ce modèle), ils sont remis
// dans les lecteurs et le TRS-80 redémarre sur le lecteur 0.
const sessionBox = document.getElementById('keep-session');
sessionBox.checked = prefs.keepSession;
sessionBox.addEventListener('change', () => {
  prefs.keepSession = sessionBox.checked;
  savePrefs();
});
let sessionReady = false; // faux pendant un changement de modèle ou une restauration
let savedSignature = null;

function sessionSignature() {
  return [...driveRows, ...hardRows].map((r) => r.signature()).join('|');
}

async function saveSession(force = false) {
  if (!sessionReady || !emulator || !prefs.keepSession) return;
  const signature = sessionSignature();
  if (!force && signature === savedSignature) return;
  savedSignature = signature;
  const value = { drives: driveRows.map((r) => r.snapshot()), hard: hardRows.map((r) => r.snapshot()), saved: Date.now() };
  try {
    await dbRequest('session', 'readwrite', (s) => s.put(value, `model-${prefs.model}`));
  } catch { /* stockage indisponible (navigation privée) : on s'en passe */ }
}
setInterval(saveSession, 3000);
document.addEventListener('visibilitychange', () => { if (document.hidden) saveSession(); });
addEventListener('pagehide', () => saveSession());

/** Remet les disques de la session précédente de ce modèle; vrai s'il y en avait. */
async function restoreSession() {
  let value = null;
  if (emulator && prefs.keepSession) {
    try {
      value = await dbRequest('session', 'readonly', (s) => s.get(`model-${prefs.model}`));
    } catch { value = null; }
  }
  const any = !!value && [...(value.drives ?? []), ...(value.hard ?? [])].some(Boolean);
  if (any) {
    value.hard?.forEach((h, i) => { if (h && hardRows[i]) hardRows[i].insert(h.name, h.bytes); });
    value.drives?.forEach((d, i) => { if (d && i > 0) driveRows[i].insert(d.name, d.bytes); });
    const boot = value.drives?.[0];
    if (boot) driveRows[0].insertAndBoot(boot.name, boot.bytes);
    showStatus(t('session.restored'));
  }
  savedSignature = sessionSignature();
  sessionReady = true;
  return any;
}
languageListeners.push(() => driveRows.forEach((r) => r.refresh()));

// ------------------------------------------------------------------ état de la machine

// Tout le TRS-80 (processeur, mémoire, périphériques, disquettes et disques durs avec leurs
// écritures, cassette) : enregistré dans le fureteur (une place par modèle, magasin
// « session », clé « state-<n> ») ou dans un fichier .t80s. Fichier : « T80P », la longueur
// d'un en-tête JSON (nom des disques, date), l'en-tête, puis l'état de l'émulateur (sans la
// ROM : la reprise exige la même).
const stateInfo = document.getElementById('state-info');
const stateRestoreButton = document.getElementById('state-restore');
const STATE_MAGIC = [0x54, 0x38, 0x30, 0x50]; // « T80P »

function stateKey(model = prefs.model) {
  return `state-${model}`;
}

/** L'état actuel, avec le nom des disques. */
function captureState() {
  return {
    model: prefs.model,
    saved: Date.now(),
    drives: driveRows.map((r) => r.contents()?.name ?? null),
    hard: hardRows.map((r) => r.contents()?.name ?? null),
    bytes: emulator.save_state(),
  };
}

function packState(s) {
  const head = new TextEncoder().encode(JSON.stringify({ model: s.model, saved: s.saved, drives: s.drives, hard: s.hard }));
  const out = new Uint8Array(8 + head.length + s.bytes.length);
  out.set(STATE_MAGIC);
  new DataView(out.buffer).setUint32(4, head.length, true);
  out.set(head, 8);
  out.set(s.bytes, 8 + head.length);
  return out;
}

/** Fichier .t80s (ou état brut de l'émulateur) → { model, saved, drives, hard, bytes }, ou null. */
function unpackState(data) {
  if (STATE_MAGIC.every((b, i) => data[i] === b) && data.length > 8) {
    const n = new DataView(data.buffer, data.byteOffset).getUint32(4, true);
    try {
      const head = JSON.parse(new TextDecoder().decode(data.subarray(8, 8 + n)));
      const bytes = data.slice(8 + n);
      return { ...head, model: Emulator.state_model(bytes), bytes };
    } catch {
      return null;
    }
  }
  const model = Emulator.state_model(data);
  return model ? { model, saved: null, drives: [], hard: [], bytes: data } : null;
}

function stateDate(ms) {
  return ms ? new Date(ms).toLocaleString(getLanguage()) : '';
}

async function refreshStateInfo() {
  let s = null;
  try { s = await dbRequest('session', 'readonly', (st) => st.get(stateKey())); } catch { s = null; }
  stateInfo.textContent = s ? t('state.savedAt', { date: stateDate(s.saved) }) : t('state.none');
  stateRestoreButton.disabled = !s || !emulator;
}

/** Remet un état (changeant de modèle au besoin); vrai si c'est fait. */
async function applyState(s) {
  if (!s?.model) {
    showStatus(t('state.e.badFormat'), true);
    return false;
  }
  if (s.model !== prefs.model) {
    modelList.value = String(s.model);
    await changeModel(s.model);
  }
  if (!emulator) return false;
  try {
    emulator.restore_state(s.bytes);
  } catch (e) {
    showStatus(t(`state.e.${e.message ?? e}`), true);
    return false;
  }
  // Les options de la page s'appliquent à la machine reprise.
  emulator.set_expansion_interface(expansion.checked);
  emulator.set_lowercase_mod(lowercaseBox.checked);
  emulator.set_graphics_board(hiresBoard.checked);
  driveRows.forEach((r, i) => r.adopt(s.drives?.[i] ?? (emulator.disk_geometry(i) ? `disk-${i}.dsk` : null)));
  hardRows.forEach((r, i) => r.adopt(s.hard?.[i] ?? null));
  modem.hangup(false);
  forgetLine();
  hideNowInfo();
  saveSession(true);
  showScreen();
  focusScreen();
  showStatus(t('state.restored', { date: stateDate(s.saved) }));
  return true;
}

document.getElementById('state-save').addEventListener('click', async () => {
  if (!emulator) return;
  const s = captureState();
  try {
    await dbRequest('session', 'readwrite', (st) => st.put(s, stateKey()));
    showStatus(t('state.saved', { kb: Math.round(s.bytes.length / 1024) }));
  } catch {
    showStatus(t('state.noStorage'), true);
  }
  refreshStateInfo();
  focusScreen();
});
stateRestoreButton.addEventListener('click', async () => {
  let s = null;
  try { s = await dbRequest('session', 'readonly', (st) => st.get(stateKey())); } catch { s = null; }
  if (s) await applyState(s);
});
document.getElementById('state-download').addEventListener('click', () => {
  if (!emulator) return;
  const s = captureState();
  const d = new Date(s.saved);
  const stamp = `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, '0')}${String(d.getDate()).padStart(2, '0')}-${String(d.getHours()).padStart(2, '0')}${String(d.getMinutes()).padStart(2, '0')}`;
  download(packState(s), `trs80-model${s.model}-${stamp}.t80s`);
});
document.getElementById('state-file').addEventListener('change', async (event) => {
  const file = event.target.files[0];
  event.target.value = '';
  if (!file) return;
  await applyState(unpackState(new Uint8Array(await file.arrayBuffer())));
});
languageListeners.push(refreshStateInfo);

// Le DOS peut écrire sur la disquette : on met à jour l'indication « modified ».
setInterval(() => driveRows.forEach((r) => r.refresh()), 1000);

async function fetchJson(url) {
  try {
    const res = await fetch(url);
    return res.ok ? await res.json() : [];
  } catch {
    return [];
  }
}

// Disquettes publiées (disks/index.json), puis, en développement, une liste locale
// (disks/local/index.json, exclue de Git) pour des disquettes qu'on ne peut pas publier.
async function loadDiskIndex() {
  const published = await fetchJson(`disks/index.json${V}`);
  const local = (await fetchJson(`disks/local/index.json${V}`))
    .map((d) => ({ ...d, file: `local/${d.file}`, local: true }));
  disks = [...published, ...local];
  fillDiskList();
}

/** Disquettes du modèle choisi (champ « model » de l'index : un nombre ou une liste). */
function fillDiskList() {
  diskList.replaceChildren(diskList.options[0]);
  for (const d of disks) {
    const models = [d.model ?? 1].flat();
    if (models.includes(prefs.model)) diskList.append(new Option('', d.id));
  }
  relabelDiskList();
}

function relabelDiskList() {
  for (const option of diskList.options) {
    const d = disks.find((x) => x.id === option.value);
    if (d) option.textContent = `${localized(d, 'title')}${d.year ? ` (${d.year})` : ''}${d.data ? ` — ${t('disk.data')}` : ''}${d.local ? ` — ${t('disk.local')}` : ''}`;
  }
}
languageListeners.push(relabelDiskList);

/** Contenu d'une disquette de la liste (null en cas d'échec, signalé dans la ligne d'état). */
async function fetchListedDisk(d) {
  try {
    const res = await fetch(`disks/${d.file}${V}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return new Uint8Array(await res.arrayBuffer());
  } catch (e) {
    showStatus(t('download.fail', { name: localized(d, 'title'), msg: failure(e) }), true);
    return null;
  }
}

/** Lien direct ?disk= : démarre la disquette (une disquette de données va au lecteur 1). */
async function bootDisk(id) {
  const d = disks.find((x) => x.id === id);
  if (!d) return;
  const bytes = await fetchListedDisk(d);
  if (!bytes) return;
  const name = d.file.split('/').pop();
  if (d.data) driveRows[1].insert(name, bytes);
  else driveRows[0].insertAndBoot(name, bytes, null, entryInfo(d, name));
  showScreen();
}

// Disquette choisie dans la liste : sa fiche, et « Mettre dans… » comme dans la bibliothèque
// et le dépôt.
const diskDetail = document.getElementById('disk-detail');

function showDiskDetail() {
  const d = disks.find((x) => x.id === diskList.value);
  diskDetail.hidden = !d;
  if (!d) return;
  const name = d.file.split('/').pop();
  const meta = element('div', 'lib-meta');
  meta.append(element('span', 'lib-name', localized(d, 'title')),
    element('span', 'lib-sub', [name, d.year, d.authors].filter(Boolean).join(' · ')));
  const about = localized(d, 'description');
  if (about) meta.append(element('span', 'lib-about', about));
  const actions = element('div', 'lib-actions');
  actions.append(placeSelect(name, () => fetchListedDisk(d), { info: entryInfo(d, name) }));
  diskDetail.replaceChildren(icon('disk'), meta, actions);
}

diskList.addEventListener('change', showDiskDetail);
languageListeners.push(showDiskDetail);

// ------------------------------------------------------------------ bibliothèque

// Fichiers de l'utilisateur, conservés dans IndexedDB (ce fureteur, cet appareil) :
// { id, name, kind, bytes, size, updated }.
const KINDS = [
  { kind: 'disk', pattern: /\.(dsk|dmk|jv1|jv3|imd)$/i, icon: 'disk' },
  { kind: 'hard', pattern: /\.hdv$/i, icon: 'disk' },
  { kind: 'program', pattern: /\.(cmd|cas)$/i, icon: 'file' },
  { kind: 'basic', pattern: /\.(bas|txt)$/i, icon: 'basic' },
];
const libraryList = document.getElementById('library-list');
const libraryCount = document.getElementById('library-count');
const keepFiles = document.getElementById('keep-files');
let library = [];

keepFiles.checked = prefs.keepFiles;
keepFiles.addEventListener('change', () => { prefs.keepFiles = keepFiles.checked; savePrefs(); });

function kindOf(name) {
  return KINDS.find((k) => k.pattern.test(name));
}

function sameBytes(a, b) {
  return a.length === b.length && a.every((x, i) => x === b[i]);
}

/**
 * Garde un fichier dans la bibliothèque; avec `id`, remplace cette entrée. Un fichier
 * identique déjà présent n'est pas dupliqué. Retourne l'identifiant, ou null.
 */
async function keepFile(name, bytes, { id = null } = {}) {
  const kind = kindOf(name);
  if (!kind) {
    showStatus(t('lib.notSupported', { name }), true);
    return null;
  }
  const twin = library.find((f) => f.name === name && sameBytes(f.bytes, bytes));
  if (twin && (id == null || twin.id === id)) return twin.id;
  const item = { name, kind: kind.kind, bytes: bytes.slice(), size: bytes.length, updated: Date.now() };
  if (id != null && library.some((f) => f.id === id)) item.id = id;
  try {
    const newId = await dbRequest('files', 'readwrite', (s) => s.put(item));
    // Demande au fureteur de ne pas effacer ces données quand l'espace manque.
    navigator.storage?.persist?.().catch(() => {});
    await refreshLibrary();
    return newId;
  } catch (e) {
    showStatus(t('lib.keepFail', { name, msg: e.message ?? e }), true);
    return null;
  }
}

async function refreshLibrary() {
  try {
    library = await dbRequest('files', 'readonly', (s) => s.getAll());
  } catch {
    library = [];
  }
  library.sort((a, b) => b.updated - a.updated);
  renderLibrary();
}

function formatSize(n) {
  return n < 1024 ? `${n} B` : `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
}

function renderLibrary() {
  libraryCount.hidden = library.length === 0;
  libraryCount.textContent = library.length;
  document.getElementById('library-empty').hidden = library.length > 0;
  libraryList.replaceChildren(...library.map(libraryItem));
  navigator.storage?.estimate?.().then(({ usage, quota }) => {
    document.getElementById('library-usage').textContent = library.length
      ? t('lib.usage', { used: formatSize(usage), quota: formatSize(quota) }) : '';
  }).catch(() => {});
}
languageListeners.push(renderLibrary);

function libraryItem(file) {
  const kind = KINDS.find((k) => k.kind === file.kind) ?? KINDS[1];
  const li = element('li', 'lib-item');
  const meta = element('div', 'lib-meta');
  const nameEl = element('span', 'lib-name', file.name);
  nameEl.title = file.name;
  meta.append(nameEl, element('span', 'lib-sub',
    `${t(`kind.${kind.kind}`)} · ${formatSize(file.size)} · ${new Date(file.updated).toLocaleDateString(document.documentElement.lang)}`));
  const actions = element('div', 'lib-actions');

  if (file.kind === 'disk' || file.kind === 'hard') {
    actions.append(placeSelect(file.name, async () => file.bytes, { libraryId: file.id }));
  } else {
    const run = textElement('button', 'secondary', 'lib.run');
    run.type = 'button';
    run.addEventListener('click', () => runFile(file.bytes, file.name));
    actions.append(run);
  }
  for (const b of actions.querySelectorAll('button, select')) b.disabled = !emulator;

  const get = iconButton('download', 'lib.download');
  get.addEventListener('click', () => download(file.bytes, file.name));
  const del = iconButton('trash', 'lib.delete', 'danger');
  del.addEventListener('click', async () => {
    if (!confirm(t('lib.confirmDelete', { name: file.name }))) return;
    await dbRequest('files', 'readwrite', (s) => s.delete(file.id)).catch(() => {});
    await refreshLibrary();
  });
  actions.append(element('span', 'spacer'), get, del);
  li.append(icon(kind.icon), meta, actions);
  return li;
}

document.getElementById('library-file').addEventListener('change', async (event) => {
  const files = [...event.target.files];
  event.target.value = '';
  for (const file of files) await keepFile(file.name, new Uint8Array(await file.arrayBuffer()));
  if (files.length) showStatus(t(files.length > 1 ? 'lib.addedN' : 'lib.added1', { n: files.length }));
});

// ------------------------------------------------------------------ dépôt externe

// Un dépôt sur le Web (par défaut ve2cuy.com/trs80) : dossiers rom/, disk/, cmd/ et bas/,
// chacun avec un index.json qui liste ses fichiers, comme disks/index.json : des objets
// { file, title, year, authors, description, license, model } ou simplement des noms de
// fichiers. `model` (1, 2, 3, 4 ou une liste) : seuls les fichiers du modèle choisi sont
// listés, sauf si « Tous les modèles » est coché; sans `model`, le fichier vaut pour tous.
// `file` peut inclure un sous-dossier (ex. model3/trsdos13.dsk).
// Le serveur doit permettre les requêtes d'une autre origine (CORS).
const DEFAULT_REPO = 'https://ve2cuy.com/trs80';
const REPO_ICONS = { rom: 'cpu', disk: 'disk', cmd: 'file', bas: 'basic', asm: 'code', cas: 'tape' };
const repoCat = document.getElementById('repo-cat');
const repoSearch = document.getElementById('repo-search');
const repoSelect = document.getElementById('repo-select');
const repoDetail = document.getElementById('repo-detail');
let repoShown = []; // entrées de la liste déroulante (catégorie et modèle courants)
const repoStatus = document.getElementById('repo-status');
const repoLegend = document.getElementById('repo-legend');
const repoInput = document.getElementById('repo-url');
const repoCache = new Map(); // adresse d'un index.json -> ses entrées

function repoUrl(path) {
  return `${prefs.repo.replace(/\/+$/, '')}/${path}`;
}

async function loadRepo(kind) {
  prefs.repoKind = kind;
  savePrefs();
  for (const radio of document.querySelectorAll('input[name="repo-kind"]')) radio.checked = radio.value === kind;
  const indexUrl = repoUrl(`${kind}/index.json`);
  // L'ancienne liste reste affichée pendant le chargement : la vider raccourcirait le menu,
  // qui sauterait (défilement ramené plus haut) puis s'allongerait de nouveau.
  repoStatus.textContent = t('repo.loading');
  try {
    let entries = repoCache.get(indexUrl);
    if (!entries) {
      const res = await fetch(indexUrl, { cache: 'no-cache' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const list = await res.json();
      if (!Array.isArray(list)) throw new Error(t('repo.notList'));
      entries = list.map((e) => (typeof e === 'string' ? { file: e } : e)).filter((e) => e?.file);
      repoCache.set(indexUrl, entries);
    }
    if (prefs.repoKind !== kind) return; // une autre catégorie a été choisie entre-temps
    const model = t(`model.${prefs.model}`);
    const forModel = prefs.repoAll ? entries : entries.filter((e) => repoModels(e).includes(prefs.model));
    // Catégories des fichiers du modèle (avec leur nombre), puis recherche dans le texte.
    const counts = new Map();
    for (const e of forModel) if (e.category) counts.set(e.category, (counts.get(e.category) ?? 0) + 1);
    let cat = prefs.repoCat[kind] ?? '';
    if (cat && !counts.has(cat)) cat = '';
    const allOption = new Option(t('repo.allCats'), '');
    repoCat.replaceChildren(allOption, ...[...counts].sort((a, b) => b[1] - a[1])
      .map(([c, n]) => new Option(`${t(`cat.${c}`)} (${n})`, c)));
    repoCat.value = cat;
    repoCat.hidden = !counts.size;
    const words = (prefs.repoQuery ?? '').toLowerCase().split(/\s+/).filter(Boolean);
    const shown = forModel.filter((e) => (!cat || e.category === cat) && (!words.length || words.every((w) =>
      `${localized(e, 'title') ?? ''} ${e.file} ${localized(e, 'description') ?? ''} ${e.authors ?? ''}`.toLowerCase().includes(w))));
    let summary;
    if (!entries.length) summary = t('repo.empty');
    else if (!forModel.length) summary = t('repo.noneForModel', { model, n: entries.length });
    else if (prefs.repoAll) summary = t('repo.countAll', { n: entries.length });
    else summary = t('repo.count', { n: forModel.length, total: entries.length, model });
    if (shown.length !== forModel.length) summary += ` — ${t('repo.filtered', { n: shown.length })}`;
    // Disquettes : légende des symboles de la liste, sous celle-ci.
    repoLegend.hidden = kind !== 'disk';
    // Un symbole par ligne (les éléments de la légende sont séparés par « · »).
    repoLegend.textContent = kind === 'disk' ? t('repo.dosLegend').split(' · ').join('\n') : '';
    repoStatus.textContent = forModel.length ? `${prefs.repo}/${kind}/ — ${summary}` : summary;
    fillRepoSelect(kind, shown);
  } catch (e) {
    if (prefs.repoKind !== kind) return;
    fillRepoSelect(kind, []);
    repoLegend.hidden = true;
    const reason = e instanceof TypeError ? t('repo.unreachable') : e.message;
    repoStatus.textContent = t('repo.unavailable', { url: indexUrl, reason });
  }
}
// Nouvelle langue : la liste ouverte est redessinée (depuis le cache, sans requête).
languageListeners.push(() => { if (prefs.open.includes('repo')) loadRepo(prefs.repoKind); });

/** Liste déroulante des fichiers; la fiche du fichier choisi (actions) s'affiche dessous. */
function fillRepoSelect(kind, shown) {
  repoShown = shown;
  repoSelect.replaceChildren(...shown.map((e, i) => {
    const name = e.file.split('/').pop();
    // Avec « Tous les modèles », chaque fichier indique ses modèles (ex. « M3/M4 »).
    const models = prefs.repoAll && e.model != null ? ` — ${repoModels(e).map((m) => `M${m}`).join('/')}` : '';
    // Disquettes : ⚙ si elle démarre (DOS), ▫ disquette de données, ? DOS non reconnu.
    const mark = kind === 'disk' ? (e.boot === true ? '⚙ ' : e.boot === false ? '▫ ' : '? ') : '';
    return new Option(`${mark}${localized(e, 'title') ?? name}${models}`, String(i));
  }));
  // Liste vide : la liste et la fiche restent en place (hauteur stable du menu).
  if (!shown.length) repoSelect.append(new Option('—', ''));
  repoSelect.disabled = !shown.length;
  const picked = shown.findIndex((e) => e.file === prefs.repoPick[kind]);
  repoSelect.value = String(Math.max(picked, 0));
  showRepoDetail(kind);
}

function showRepoDetail(kind) {
  const entry = repoSelect.value === '' ? null : repoShown[Number(repoSelect.value)];
  repoDetail.classList.toggle('empty', !entry);
  repoDetail.replaceChildren(...(entry ? repoItem(kind, entry) : []));
}

repoSelect.addEventListener('change', () => {
  const entry = repoShown[Number(repoSelect.value)];
  if (entry) prefs.repoPick[prefs.repoKind] = entry.file;
  savePrefs();
  showRepoDetail(prefs.repoKind);
});

/** Modèles d'une entrée du dépôt (tous si elle n'en précise pas). */
function repoModels(entry) {
  return entry.model == null ? [1, 2, 3, 4] : [entry.model].flat().map(Number);
}

repoCat.addEventListener('change', () => {
  prefs.repoCat[prefs.repoKind] = repoCat.value;
  savePrefs();
  loadRepo(prefs.repoKind);
});
repoSearch.value = prefs.repoQuery ?? '';
let repoSearchTimer = 0;
repoSearch.addEventListener('input', () => {
  clearTimeout(repoSearchTimer);
  repoSearchTimer = setTimeout(() => {
    prefs.repoQuery = repoSearch.value.trim();
    savePrefs();
    loadRepo(prefs.repoKind);
  }, 250);
});

const repoAllBox = document.getElementById('repo-all');
repoAllBox.checked = prefs.repoAll;
repoAllBox.addEventListener('change', () => {
  prefs.repoAll = repoAllBox.checked;
  loadRepo(prefs.repoKind);
});

async function fetchRepoFile(kind, entry) {
  const url = repoUrl(`${kind}/${entry.file.split('/').map(encodeURIComponent).join('/')}`);
  showStatus(t('downloading', { name: entry.file }));
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

/** Contenu de la fiche d'un fichier du dépôt : titre, détails, description et actions. */
function repoItem(kind, entry) {
  const name = entry.file.split('/').pop();
  const meta = element('div', 'lib-meta');
  const title = element('span', 'lib-name', localized(entry, 'title') ?? name);
  title.title = [localized(entry, 'title'), localized(entry, 'description'), localized(entry, 'license')]
    .filter(Boolean).join('\n\n') || name;
  const models = entry.model != null ? repoModels(entry).map((m) => `M${m}`).join('/') : null;
  const category = entry.category ? t(`cat.${entry.category}`) : null;
  meta.append(title, element('span', 'lib-sub', [category, models, name, entry.year, entry.authors].filter(Boolean).join(' · ')));
  if (kind === 'disk' && entry.dos) {
    meta.append(element('span', 'lib-sub', `${t('dos.label')} ${dosText({ name: entry.dos, system: entry.boot })}`));
  }
  const about = localized(entry, 'description');
  if (about) meta.append(element('span', 'lib-about', about));
  const actions = element('div', 'lib-actions');

  /** Télécharge le fichier puis applique `use`; les erreurs vont dans la ligne d'état. */
  const withFile = (use, needsEmulator = true) => async () => {
    if (needsEmulator && !emulator) {
      showStatus(t('prog.hint'), true);
      return;
    }
    try {
      await use(await fetchRepoFile(kind, entry));
    } catch (e) {
      showStatus(t('download.fail', { name, msg: failure(e) }), true);
    }
  };
  const button = (key, handler, tip) => {
    const b = textElement('button', 'secondary', key);
    b.type = 'button';
    if (tip) setTip(b, tip);
    b.addEventListener('click', handler);
    actions.append(b);
  };

  if (kind === 'rom') {
    button('repo.load', withFile((bytes) => {
      if (!start(bytes)) return;
      romTar.hidden = true;
      selectRomInList('custom');
      saveRom('custom', bytes);
      showStatus(t('repo.romLoaded', { name: localized(entry, 'title') ?? name }));
    }, false));
  } else if (kind === 'disk') {
    // « Mettre dans… » et, à sa droite, l'icône Démarrer (lecteur 0, puis redémarrage).
    const place = element('span', 'place-row');
    const boot = iconButton('power', 'repo.bootTip');
    boot.addEventListener('click', withFile((bytes) => {
      if (driveRows[0].insertAndBoot(name, bytes, null, repoInfo(kind, entry, name))) showScreen();
    }));
    place.append(placeSelect(name, async () => {
      try {
        return await fetchRepoFile(kind, entry);
      } catch (e) {
        showStatus(t('download.fail', { name, msg: failure(e) }), true);
        return null;
      }
    }, { info: repoInfo(kind, entry, name) }), boot);
    actions.append(place);
    // Tout sur une ligne : « Mettre dans… » rétrécit au besoin.
    actions.classList.add('one-line');
  } else if (kind === 'asm') {
    // Source assembleur : ouverte dans l'atelier.
    button('repo.openIde', withFile((bytes) => {
      showIde(true);
      ide.open(bytes, name);
      showStatus(t('repo.opened', { name }));
    }, false));
  } else {
    button('lib.run', withFile((bytes) => runFile(bytes, name, repoInfo(kind, entry, name))));
  }
  actions.append(element('span', 'spacer'));
  if (kind !== 'rom') {
    const keep = iconButton('folder-plus', 'repo.keep');
    keep.addEventListener('click', withFile(async (bytes) => {
      if ((await keepFile(name, bytes)) != null) showStatus(t('lib.kept', { name }));
    }, false));
    actions.append(keep);
  }
  const get = iconButton('download', 'repo.download');
  get.addEventListener('click', withFile((bytes) => { download(bytes, name); showStatus(''); }, false));
  actions.append(get);
  return [icon(REPO_ICONS[kind]), meta, actions];
}

for (const radio of document.querySelectorAll('input[name="repo-kind"]')) {
  radio.addEventListener('change', () => loadRepo(radio.value));
}
// Raccourcis des sections Machine, Programs et Disks.
for (const b of document.querySelectorAll('[data-repo]')) {
  b.addEventListener('click', () => {
    prefs.repoKind = b.dataset.repo;
    showPanel('repo'); // ouvre la section, qui charge la liste
  });
}

function useRepo(address) {
  let url;
  try {
    url = new URL(address);
    if (!/^https?:$/.test(url.protocol)) throw new Error();
  } catch {
    repoStatus.textContent = t('repo.badUrl');
    return;
  }
  prefs.repo = repoInput.value = url.href.replace(/\/+$/, '');
  repoCache.clear();
  loadRepo(prefs.repoKind);
}

repoInput.value = prefs.repo;
document.getElementById('repo-save').addEventListener('click', () => useRepo(repoInput.value.trim()));
repoInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') useRepo(repoInput.value.trim()); });
document.getElementById('repo-default').addEventListener('click', () => useRepo(DEFAULT_REPO));

// Glisser-déposer sur l'écran : disquette au lecteur 0, programme ou listing lancé.
const monitor = document.getElementById('monitor');
const dropHint = document.getElementById('drop-hint');
monitor.addEventListener('dragover', (e) => {
  if (!emulator || !e.dataTransfer.types.includes('Files')) return;
  e.preventDefault();
  dropHint.hidden = false;
});
monitor.addEventListener('dragleave', (e) => {
  if (!monitor.contains(e.relatedTarget)) dropHint.hidden = true;
});
monitor.addEventListener('drop', async (e) => {
  e.preventDefault();
  dropHint.hidden = true;
  const file = e.dataTransfer.files[0];
  if (!file || !emulator) return;
  const kind = kindOf(file.name);
  if (!kind) {
    showStatus(t('lib.notSupported', { name: file.name }), true);
    return;
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  const id = prefs.keepFiles ? await keepFile(file.name, bytes) : null;
  if (kind.kind === 'disk') driveRows[0].insertAndBoot(file.name, bytes, id);
  else if (kind.kind === 'hard') hardRows[0].insert(file.name, bytes, id);
  else runFile(bytes, file.name);
  focusScreen();
});

// ------------------------------------------------------------------ frappe de texte

const typePanel = document.getElementById('type-panel');
const typeText = document.getElementById('type-text');

function typeOnTrs80(text) {
  if (!emulator || !text) return;
  forgetLine(); // texte collé : la ligne en cours n'est plus connue
  const accepted = emulator.type_text(text);
  const skipped = [...text.replace(/\r/g, '')].length - accepted;
  showStatus(t('type.typing', { n: accepted }) + (skipped > 0 ? t('type.skipped', { n: skipped }) : ''));
  showScreen();
}

// ------------------------------------------------------------------ modem telnet

// Le port RS-232 du TRS-80 est relié à un modem Hayes virtuel (modem.js) : ATDT hôte ouvre
// un WebSocket vers le relais telnet (server/telnet-relay), qui se connecte au BBS.
const DEFAULT_RELAY = 'wss://ve2cuy.com/trs80/relay';
const modemStatus = document.getElementById('modem-status');
const modemHangup = document.getElementById('modem-hangup');
const modemFilter = document.getElementById('modem-filter');
const modemRelay = document.getElementById('modem-relay');
let modemState = { connected: false, online: false, host: '' };

function showModemState(state = modemState) {
  modemState = state;
  const key = !state.host ? 'modem.idle' : !state.connected ? 'modem.dialing'
    : state.online ? 'modem.online' : 'modem.command';
  modemStatus.textContent = t(key, { host: state.host });
  modemStatus.classList.toggle('online', state.connected);
  modemHangup.disabled = !state.connected && !state.host;
}
languageListeners.push(() => showModemState());

// Écran couleur : un terminal ANSI qui reçoit tout ce que le BBS envoie; pendant la
// connexion, il prend la place de l'écran du TRS-80 (voir draw).
const ansiCanvas = document.createElement('canvas');
const ansiScreen = new AnsiScreen(ansiCanvas, (bytes) => modem.sendRemote(bytes));
const modemColor = document.getElementById('modem-color');
let ansiShown = false;

/** L'écran couleur est-il affiché (option active, connecté au BBS) ? */
function ansiActive() {
  return prefs.modemColor && modemState.connected;
}

const modem = new Modem({
  send: (bytes) => emulator?.serial_send(bytes),
  onRemote: (bytes) => ansiScreen.write(bytes),
  relay: () => prefs.modemRelay || DEFAULT_RELAY,
  baud: () => emulator?.serial_baud() ?? 300,
  filter: () => prefs.modemFilter,
  onState: (state) => {
    // Nouvelle connexion : l'écran couleur repart d'un écran vide.
    if (state.connected && !modemState.connected) ansiScreen.reset();
    // Fin de connexion : l'hôte n'est plus affiché.
    showModemState(state.connected ? state : { ...state, host: state.online ? state.host : '' });
  },
});
setInterval(() => modem.tick(), 200);
showModemState();

modemHangup.addEventListener('click', () => {
  modem.hangup();
  focusScreen();
});
modemColor.checked = prefs.modemColor;
modemColor.addEventListener('change', () => {
  prefs.modemColor = modemColor.checked;
  savePrefs();
  focusScreen();
});
modemFilter.checked = prefs.modemFilter;
modemFilter.addEventListener('change', () => {
  prefs.modemFilter = modemFilter.checked;
  savePrefs();
});
modemRelay.value = prefs.modemRelay || DEFAULT_RELAY;
function useRelay(address) {
  try {
    const url = new URL(address);
    if (!/^wss?:$/.test(url.protocol)) throw new Error();
    prefs.modemRelay = url.href === DEFAULT_RELAY ? null : url.href;
    modemRelay.value = url.href;
    savePrefs();
    showStatus(t('modem.relaySet', { url: url.href }));
  } catch {
    showStatus(t('modem.badRelay'), true);
  }
}
document.getElementById('modem-relay-save').addEventListener('click', () => useRelay(modemRelay.value.trim()));
modemRelay.addEventListener('keydown', (e) => { if (e.key === 'Enter') useRelay(modemRelay.value.trim()); });
document.getElementById('modem-relay-default').addEventListener('click', () => useRelay(DEFAULT_RELAY));

// Liste des BBS (bbs.json, tirée de telnetbbsguide.com; le relais permet les mêmes) : le
// bouton Composer tape ATDT hôte[:port] sur le TRS-80, qui doit être dans LCOMM ou COMM.
const bbsFilter = document.getElementById('bbs-filter');
const bbsSelect = document.getElementById('bbs-select');
const bbsDial = document.getElementById('bbs-dial');
const bbsInfo = document.getElementById('bbs-info');
let bbsList;          // undefined : en chargement; null : non disponible

async function loadBbs() {
  try {
    const res = await fetch(`bbs.json${V}`);
    if (!res.ok) throw new Error(res.status);
    bbsList = (await res.json()).bbs.map(([name, host, port]) => ({ name, host, port, key: `${host}:${port}` }));
  } catch {
    bbsList = null;
  }
  fillBbs();
}

function bbsTarget(b) {
  return b.port === 23 ? b.host : b.key;
}

function fillBbs() {
  const words = bbsFilter.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const shown = (bbsList || []).filter((b) => words.every((w) => `${b.name} ${b.key}`.toLowerCase().includes(w)));
  bbsSelect.replaceChildren(...shown.map((b) => new Option(b.name, b.key, false, b.key === prefs.bbsPick)));
  showBbs();
}

function showBbs() {
  const b = bbsList?.find((x) => x.key === bbsSelect.value);
  bbsDial.disabled = !b;
  bbsInfo.textContent = bbsList === undefined ? '' : !bbsList ? t('modem.listError')
    : b ? t('modem.pick', { target: bbsTarget(b) }) : t('modem.count', { n: bbsList.length });
}

function dialBbs() {
  const b = bbsList?.find((x) => x.key === bbsSelect.value);
  if (!b) return;
  typeOnTrs80(`ATDT ${bbsTarget(b)}\n`);
  focusScreen();
}

bbsFilter.addEventListener('input', fillBbs);
bbsSelect.addEventListener('change', () => { prefs.bbsPick = bbsSelect.value; savePrefs(); showBbs(); });
bbsSelect.addEventListener('dblclick', dialBbs);
bbsSelect.addEventListener('keydown', (e) => { if (e.key === 'Enter') dialBbs(); });
bbsDial.addEventListener('click', dialBbs);
languageListeners.push(showBbs);
loadBbs();

// ------------------------------------------------------------------ copier-coller

// Coller (CTRL+V hors d'un champ de saisie, ou bouton) : le texte est tapé sur le TRS-80.
// Copier (CTRL+C sans sélection dans la page, ou bouton) : le texte de l'écran.
const pasteButton = document.getElementById('paste-button');
const copyButton = document.getElementById('copy-button');
const historyButton = document.getElementById('history-button');

document.addEventListener('paste', (e) => {
  if (!emulator || typingInForm(e.target)) return;
  const text = e.clipboardData?.getData('text/plain');
  if (!text) return;
  e.preventDefault();
  typeOnTrs80(text);
  focusScreen();
});

document.addEventListener('copy', (e) => {
  if (!emulator || typingInForm(e.target) || !getSelection().isCollapsed) return;
  e.preventDefault();
  e.clipboardData.setData('text/plain', emulator.screen_text());
  showStatus(t('clip.copied'));
});

pasteButton.addEventListener('click', async () => {
  try {
    typeOnTrs80(await navigator.clipboard.readText());
  } catch {
    showStatus(t('clip.denied'), true);
  }
  focusScreen();
});

copyButton.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(emulator.screen_text());
    showStatus(t('clip.copied'));
  } catch {
    showStatus(t('clip.denied'), true);
  }
  focusScreen();
});

typeButton.addEventListener('click', () => {
  typePanel.hidden = !typePanel.hidden;
  if (!typePanel.hidden) typeText.focus();
});
document.getElementById('type-send').addEventListener('click', () => {
  let text = typeText.value;
  if (text && !text.endsWith('\n')) text += '\n';
  typeOnTrs80(text);
});
document.getElementById('type-keep').addEventListener('click', async () => {
  const text = typeText.value.trim();
  if (!text) return;
  let name = prompt(t('type.prompt'), 'program.bas');
  if (!name?.trim()) return;
  name = /\.(bas|txt)$/i.test(name.trim()) ? name.trim() : `${name.trim()}.bas`;
  if ((await keepFile(name, new TextEncoder().encode(`${text}\n`))) != null) {
    showStatus(t('lib.kept', { name }));
  }
});
document.getElementById('type-stop').addEventListener('click', () => {
  emulator?.cancel_typing();
  showStatus(t('type.stopped'));
});

expansion.addEventListener('change', () => {
  emulator?.set_expansion_interface(expansion.checked);
  prefs.expansion = expansion.checked;
  savePrefs();
  focusScreen();
});

lowercaseBox.addEventListener('change', () => {
  emulator?.set_lowercase_mod(lowercaseBox.checked);
  prefs.lowercase = lowercaseBox.checked;
  savePrefs();
  lastVideo = null;
  focusScreen();
});

hiresBoard.addEventListener('change', () => {
  emulator?.set_graphics_board(hiresBoard.checked);
  prefs.hires = hiresBoard.checked;
  savePrefs();
  lastVideo = null;
  focusScreen();
});

// ------------------------------------------------------------------ clavier

// La touche relâchée peut avoir un autre nom que la touche enfoncée (ex. : « a » puis « A »
// si MAJ a été enfoncée entre-temps) : on mémorise le nom par touche physique.
const pressed = new Map(); // code physique -> { name, at: image où la touche a été enfoncée }

// La ROM lit le clavier pendant que l'émulateur tourne : une touche enfoncée puis relâchée
// entre deux images ne serait jamais vue (frappe rapide). Chaque touche reste donc enfoncée
// au moins MIN_HOLD_FRAMES images; un relâchement trop rapide est différé.
const MIN_HOLD_FRAMES = 3;
let frameCount = 0;
let pendingReleases = [];

function typingInForm(target) {
  return target instanceof HTMLInputElement || target instanceof HTMLSelectElement
    || target instanceof HTMLTextAreaElement;
}

/** Enfonce une touche; retourne ce qu'il faut passer à releaseKey, ou null si inconnue. */
function pressKey(name) {
  if (typeof name !== 'string' || !emulator.key_down(name)) return null;
  pendingReleases = pendingReleases.filter((r) => r.name !== name);
  return { name, at: frameCount };
}

function releaseKey(key) {
  if (frameCount - key.at >= MIN_HOLD_FRAMES) {
    emulator.key_up(key.name);
  } else {
    pendingReleases.push(key);
  }
}

window.addEventListener('keydown', (e) => {
  // Remplissage automatique (mots de passe) : des événements sans touche (key absent).
  if (!emulator || e.altKey || e.metaKey || typeof e.key !== 'string') return;
  // Historique des commandes : Ctrl+↑ / Ctrl+↓ (les flèches seules restent au TRS-80).
  if (e.ctrlKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown') && !typingInForm(e.target)) {
    e.preventDefault();
    recallHistory(e.key === 'ArrowUp' ? -1 : 1);
    return;
  }
  // Model II : CTRL + lettre donne un code de contrôle (CTRL-C, etc.).
  const ctrlKey = e.ctrlKey && emulator.model() === 2 && e.key.length === 1 && !typingInForm(e.target)
    && e.key.toLowerCase() !== 'v';
  if (e.ctrlKey && !ctrlKey) return;
  if (e.target === touchInput) {
    // Clavier virtuel : le texte (et ← ou ENTRÉE) arrive par l'événement input; seules les
    // touches qui ne produisent pas de texte (flèches, Échap d'un clavier branché) passent ici.
    if (e.isComposing || e.key.length === 1 || ['Unidentified', 'Process', 'Backspace', 'Enter'].includes(e.key)) return;
  } else if (typingInForm(e.target)) {
    return;
  }
  // Model II : clavier ASCII avec répétition; les autres répètent d'eux-mêmes.
  if (e.repeat && emulator.model() !== 2) { e.preventDefault(); return; }
  // Model II : la casse des lettres vient de MAJ et de sa touche CAPS (Verr. Maj), pas du
  // verrouillage des majuscules de l'ordinateur.
  let name = e.key;
  if (emulator.model() === 2 && /^[a-z]$/i.test(name) && e.getModifierState('CapsLock')) {
    name = name === name.toLowerCase() ? name.toUpperCase() : name.toLowerCase();
  }
  const key = pressKey(ctrlKey ? `Ctrl+${e.key.toUpperCase()}` : name);
  if (key) {
    trackLine(name, e.shiftKey);
    pressed.set(e.code, key);
    e.preventDefault();
    if (name === 'CapsLock' && emulator.model() === 2) showStatus(t(emulator.caps() ? 'caps.on' : 'caps.off'));
  }
});

window.addEventListener('keyup', (e) => {
  if (!emulator || typeof e.key !== 'string') return;
  const key = pressed.get(e.code);
  pressed.delete(e.code);
  if (key) {
    releaseKey(key);
  } else if (e.target !== touchInput) {
    emulator.key_up(e.key);
  }
});

// ------------------------------------------------------------------ imprimante

// Ce que le TRS-80 envoie à l'imprimante (LPRINT, LIST (P), COPY *PR...), sur du papier en
// continu d'époque, sous les contrôles de l'écran. Retour chariot : nouvelle ligne (LF qui
// suit un CR ignoré); saut de page (FF) : nouvelle feuille; tabulation : colonne suivante
// multiple de 8; ESC : la séquence de commande de l'imprimante est ignorée.
const PRINTER_MAX_LINES = 5000;
const PRINTER_PAGE_LINES = 66;
const printerBox = document.getElementById('printer');
const printerFeed = document.getElementById('printer-feed');
const printerCount = document.getElementById('printer-count');
let printerPages = [['']]; // pages de lignes (la dernière ligne : celle en cours)
let printerLastByte = 0;
let printerEscape = false;
let printerHidden = false;

function printerLine() {
  const page = printerPages[printerPages.length - 1];
  return page;
}

function printerAppend(bytes) {
  for (const b of bytes) {
    const page = printerLine();
    if (printerEscape) {
      printerEscape = false;
    } else if (b === 0x1B) {
      printerEscape = true;
    } else if (b === 0x0D) {
      page.push('');
    } else if (b === 0x0A) {
      if (printerLastByte !== 0x0D) page.push('');
    } else if (b === 0x0C) {
      printerPages.push(['']);
    } else if (b === 0x09) {
      const line = page[page.length - 1];
      page[page.length - 1] = line.padEnd((Math.floor(line.length / 8) + 1) * 8);
    } else if (b === 0x08) {
      page[page.length - 1] = page[page.length - 1].slice(0, -1);
    } else if (b >= 0x20 && b < 0x7F) {
      page[page.length - 1] += String.fromCharCode(b);
    }
    printerLastByte = b;
  }
  // Au plus PRINTER_MAX_LINES lignes : les plus anciennes pages tombent.
  let total = printerPages.reduce((n, p) => n + p.length, 0);
  while (total > PRINTER_MAX_LINES && printerPages.length > 1) total -= printerPages.shift().length;
  renderPrinter(true);
}

/** Le texte imprimé (pages séparées par un saut de page). */
function printerText() {
  return printerPages.map((p) => p.join('\n').replace(/\s+$/, '')).join('\n\f\n').replace(/^\s+$/, '');
}

function renderPrinter(fresh = false) {
  const lines = printerPages.reduce((n, p) => n + p.length, 0) - 1;
  const empty = printerPages.length === 1 && printerPages[0].length === 1 && !printerPages[0][0];
  // Feuilles de 66 lignes (11 pouces à 6 lignes par pouce, comme le compte la ROM pour le
  // saut de page), et une nouvelle feuille à chaque saut de page (FF).
  const sheets = [];
  for (const page of printerPages) {
    for (let i = 0; i < page.length || i === 0; i += PRINTER_PAGE_LINES) sheets.push(page.slice(i, i + PRINTER_PAGE_LINES));
  }
  printerFeed.replaceChildren(...sheets.map((lines) => {
    const sheet = element('div', 'sheet');
    sheet.append(element('pre', '', lines.join('\n')));
    return sheet;
  }));
  printerCount.textContent = empty ? '' : t('printer.count', { lines, pages: printerPages.length });
  if (fresh && !empty) {
    printerHidden = false;
    printerBox.hidden = false;
    printerFeed.scrollTop = printerFeed.scrollHeight;
  }
  if (empty || printerHidden) printerBox.hidden = true;
}
languageListeners.push(() => renderPrinter());

document.getElementById('printer-copy').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(printerText());
    showStatus(t('printer.copied'));
  } catch {
    showStatus(t('clip.denied'), true);
  }
});
document.getElementById('printer-save').addEventListener('click', () => {
  download(new TextEncoder().encode(printerText().replace(/\n/g, '\r\n')), 'trs80-printout.txt');
});
document.getElementById('printer-clear').addEventListener('click', () => {
  printerPages = [['']];
  printerLastByte = 0;
  renderPrinter();
});
document.getElementById('printer-hide').addEventListener('click', () => {
  printerHidden = true;
  printerBox.hidden = true;
});

// ------------------------------------------------------------------ historique des commandes

// Comme l'historique de bash : chaque ligne tapée au clavier et validée par ENTRÉE (à
// l'invite du DOS, du BASIC...) est gardée, par modèle, dans le fureteur. Ctrl+↑ et
// Ctrl+↓ la rappellent : la ligne en cours est effacée (←) et la commande retapée, sans
// ENTRÉE. Le bouton Historique de la barre d'outils en donne la liste.
const HISTORY_MAX = 100;
let lineBuffer = ''; // ligne en cours, telle que tapée au clavier
let historyPos = -1; // rang dans l'historique pendant un rappel (-1 : aucun)
let historyDraft = ''; // ligne en cours au début du rappel

const historyKey = () => `trs80-history-${prefs.model}`;

function loadHistory() {
  try {
    const list = JSON.parse(localStorage.getItem(historyKey()) ?? '[]');
    return Array.isArray(list) ? list.filter((x) => typeof x === 'string') : [];
  } catch {
    return [];
  }
}

function saveHistory(list) {
  try {
    localStorage.setItem(historyKey(), JSON.stringify(list.slice(-HISTORY_MAX)));
  } catch { /* stockage indisponible */ }
}

/** Ligne validée : elle passe en fin d'historique (une seule fois). */
function rememberLine() {
  const line = lineBuffer.trim();
  forgetLine();
  if (!line) return;
  saveHistory([...loadHistory().filter((x) => x !== line), line]);
}

function forgetLine() {
  lineBuffer = '';
  historyPos = -1;
}

/** Suit la ligne en cours d'après les touches tapées. */
function trackLine(name, shift) {
  if (name === 'Enter') rememberLine();
  else if (name === 'Backspace' || name === 'ArrowLeft') lineBuffer = shift ? '' : lineBuffer.slice(0, -1);
  else if (name === 'Escape' || name === 'Home' || name === 'Clear') forgetLine();
  else if (name.length === 1) {
    lineBuffer += name;
    historyPos = -1;
  }
}

/** Remplace la ligne en cours par `text` (retours arrière, puis le texte, sans ENTRÉE). */
function replaceLine(text) {
  if (!emulator) return;
  emulator.type_text('\b'.repeat(lineBuffer.length) + text);
  lineBuffer = text;
}

function recallHistory(direction) {
  const list = loadHistory();
  if (!list.length) {
    showStatus(t('history.empty'));
    return;
  }
  if (historyPos < 0) {
    historyPos = list.length;
    historyDraft = lineBuffer;
  }
  const pos = Math.min(Math.max(historyPos + direction, 0), list.length);
  if (pos === historyPos && pos !== list.length) return;
  historyPos = pos;
  const keep = historyPos;
  replaceLine(pos === list.length ? historyDraft : list[pos]);
  historyPos = keep;
  showScreen();
}

/** Fenêtre de l'historique : les dernières commandes, à taper ou à exécuter. */
function openHistoryDialog() {
  if (!emulator) return;
  const list = loadHistory().reverse();
  const title = element('h2', '', t('history.title', { model: t(`model.${prefs.model}`) }));
  const hint = element('p', 'hint', t('history.hint'));
  const ul = element('ul', 'history-list');
  for (const command of list) {
    const li = element('li');
    li.append(element('code', '', command));
    li.title = command;
    const type = dialogButton('history.type', () => {
      diskDialog.close();
      replaceLine(command);
      historyPos = -1;
      focusScreen();
    });
    const run = dialogButton('history.run', () => {
      diskDialog.close();
      replaceLine(command);
      emulator.type_text('\n');
      rememberLine();
      focusScreen();
    });
    li.append(type, run);
    ul.append(li);
  }
  const scroll = element('div', 'files-scroll');
  scroll.append(ul);
  const actions = element('div', 'dialog-actions');
  const clear = dialogButton('history.clear', () => {
    if (!confirm(t('history.confirmClear', { model: t(`model.${prefs.model}`) }))) return;
    saveHistory([]);
    forgetLine();
    openHistoryDialog();
  }, 'danger');
  clear.disabled = !list.length;
  actions.append(clear, element('span', 'spacer'), dialogButton('files.close', () => diskDialog.close()));
  showDialog(title, hint, list.length ? scroll : element('p', 'hint', t('history.empty')), actions);
}
historyButton.addEventListener('click', openHistoryDialog);

// Aide-mémoire : les commandes système de chaque DOS (lecteurs, fichiers, disque dur, port
// série, imprimante); un clic tape la commande sur le TRS-80, sans ENTRÉE, pour l'adapter.
let cheatChoice = null;

function openCheatsheet(id = null) {
  const chosen = id ?? cheatChoice ?? defaultCheatsheet(emulator ? driveDos(0)?.name : null, prefs.model);
  const sheet = CHEATSHEET.find((x) => x.id === chosen) ?? CHEATSHEET[0];
  cheatChoice = sheet.id;
  const title = element('h2', '', t('cheat.title'));
  const select = element('select');
  for (const x of CHEATSHEET) select.append(new Option(x.title, x.id));
  select.value = sheet.id;
  select.addEventListener('change', () => openCheatsheet(select.value));
  const pick = element('label', 'field');
  pick.append(element('span', '', t('cheat.dos')), select);
  const body = element('div', 'cheat-sections');
  for (const [section, items] of sheet.sections) {
    body.append(element('h3', '', t(`cheat.sec.${section}`)));
    for (const [command, key] of items) {
      const row = element('div', 'cheat-row');
      const b = element('button', 'cheat-cmd', command);
      b.type = 'button';
      b.title = t('cheat.typeIt');
      b.addEventListener('click', () => {
        if (!emulator) return;
        diskDialog.close();
        replaceLine(command);
        showStatus(t('cheat.typed', { command }));
        focusScreen();
      });
      row.append(b, element('span', 'cheat-desc', t(`cheat.${key}`)));
      body.append(row);
    }
  }
  const actions = element('div', 'dialog-actions');
  actions.append(element('span', 'spacer'), dialogButton('files.close', () => diskDialog.close()));
  showDialog(title, element('p', 'hint', t('cheat.hint')), pick, body, actions);
}
document.getElementById('cheat-button').addEventListener('click', () => openCheatsheet());

// ------------------------------------------------------------------ écran tactile

// Une tablette n'affiche son clavier virtuel que pour un champ de texte : toucher l'écran
// donne le focus à un champ invisible. Ce qu'on y tape est comparé au contenu précédent
// (les claviers Android n'envoient pas de touches, seulement du texte, parfois corrigé
// en cours de mot) et retapé sur le TRS-80 : ajouts, et ← pour les caractères effacés.
const touchInput = document.getElementById('touch-input');
const touchKeyboard = document.getElementById('touch-keyboard');
// Le champ n'est jamais vide : la touche ← du clavier virtuel a toujours quoi effacer.
const TOUCH_FILL = '  ';
let touchPrevious = TOUCH_FILL;
let touchComposing = false;

function resetTouchInput() {
  touchInput.value = touchPrevious = TOUCH_FILL;
  touchInput.setSelectionRange(TOUCH_FILL.length, TOUCH_FILL.length);
}

function syncTouchInput() {
  const value = touchInput.value;
  let common = 0;
  while (common < value.length && common < touchPrevious.length
         && value[common] === touchPrevious[common]) common++;
  const text = '\b'.repeat(touchPrevious.length - common) + value.slice(common).replace(/\r/g, '');
  touchPrevious = value;
  if (text && emulator) emulator.type_text(text);
  // Pendant la composition d'un mot, modifier le champ dérouterait le clavier virtuel.
  if (!touchComposing) resetTouchInput();
}

touchInput.addEventListener('input', syncTouchInput);
touchInput.addEventListener('compositionstart', () => { touchComposing = true; });
touchInput.addEventListener('compositionend', () => {
  touchComposing = false;
  setTimeout(syncTouchInput); // certains fureteurs envoient le dernier input après
});
touchInput.addEventListener('focus', () => { resetTouchInput(); touchKeyboard.classList.add('active'); });
touchInput.addEventListener('blur', () => touchKeyboard.classList.remove('active'));

// Au click, après le mousedown qui donne le focus au canvas.
let canvasPointer = 'mouse';
canvas.addEventListener('pointerdown', (e) => { canvasPointer = e.pointerType; });
canvas.addEventListener('click', () => {
  if (canvasPointer !== 'mouse') touchInput.focus({ preventScroll: true });
});

// Les touches à l'écran ne doivent pas prendre le focus : le clavier virtuel resterait fermé.
const touchKeys = document.getElementById('touch-keys');
touchKeys.addEventListener('pointerdown', (e) => e.preventDefault());
touchKeyboard.addEventListener('click', () => {
  if (document.activeElement === touchInput) touchInput.blur();
  else touchInput.focus({ preventScroll: true });
});
for (const button of touchKeys.querySelectorAll('[data-key]')) {
  let key = null;
  const release = () => {
    if (key) releaseKey(key);
    key = null;
    button.classList.remove('active');
  };
  button.addEventListener('pointerdown', (e) => {
    if (!emulator || key) return;
    button.setPointerCapture(e.pointerId);
    key = pressKey(button.dataset.key);
    button.classList.add('active');
  });
  button.addEventListener('pointerup', release);
  button.addEventListener('pointercancel', release);
}

/** Applique les relâchements différés dont la durée minimale est atteinte. */
function releaseDueKeys() {
  pendingReleases = pendingReleases.filter((r) => {
    if (frameCount - r.at < MIN_HOLD_FRAMES) return true;
    emulator.key_up(r.name);
    return false;
  });
}

window.addEventListener('blur', () => {
  pressed.clear();
  pendingReleases = [];
  emulator?.release_all_keys();
});

// ------------------------------------------------------------------ son

// Le son vient de la sortie cassette du TRS-80 (port FFh), échantillonnée par Rust.
// Les fureteurs n'autorisent l'audio qu'après une action de l'utilisateur : l'AudioContext
// est créé à la première touche ou au premier clic.
let audioCtx = null;
let nextAudioTime = 0;
const AUDIO_LATENCY = 0.06; // secondes d'avance pour éviter les coupures

function ensureAudio() {
  if (!soundBox.checked && !driveSound.checked) return;
  if (!audioCtx) {
    try {
      audioCtx = new AudioContext();
    } catch {
      return; // pas de WebAudio : l'émulateur fonctionne sans son
    }
    if (soundBox.checked) emulator?.set_audio_rate(audioCtx.sampleRate);
  }
  if (audioCtx.state === 'suspended') audioCtx.resume();
}
window.addEventListener('keydown', ensureAudio, { capture: true });
window.addEventListener('pointerdown', ensureAudio, { capture: true });

soundBox.addEventListener('change', () => {
  if (soundBox.checked) {
    ensureAudio();
    if (audioCtx) emulator?.set_audio_rate(audioCtx.sampleRate);
  } else {
    emulator?.set_audio_rate(0);
  }
  prefs.sound = soundBox.checked;
  savePrefs();
  focusScreen();
});

// ------------------------------------------------------------------ bruit des lecteurs

// Le bruit des lecteurs de disquettes est synthétisé : un ronronnement de moteur (bruit
// filtré, modulé à 5 Hz comme une disquette à 300 tr/min) tant que le DOS accède au lecteur,
// et un clic par pas de la tête. Rust ne fournit que deux compteurs (pas, accès).
const MOTOR_TIMEOUT = 2.5; // s : le moteur du Model I s'arrête quelques secondes après le dernier accès
let motor = null;          // { gain, on }
let motorUntil = 0;
let clickTime = 0;
let lastSteps = 0;
let lastAccesses = 0;
let noiseBuffer = null;

driveSound.checked = prefs.driveSound;
driveSound.addEventListener('change', () => {
  prefs.driveSound = driveSound.checked;
  savePrefs();
  if (driveSound.checked) ensureAudio();
  else if (motor) setMotor(false);
  focusScreen();
});

/** Une seconde de bruit blanc, partagée par le moteur et les clics. */
function noise() {
  if (!noiseBuffer) {
    const n = audioCtx.sampleRate;
    noiseBuffer = audioCtx.createBuffer(1, n, n);
    const data = noiseBuffer.getChannelData(0);
    for (let i = 0; i < n; i++) data[i] = Math.random() * 2 - 1;
  }
  return noiseBuffer;
}

function createMotor() {
  const source = audioCtx.createBufferSource();
  source.buffer = noise();
  source.loop = true;
  const filter = audioCtx.createBiquadFilter();
  filter.type = 'lowpass';
  filter.frequency.value = 220;
  // Rotation de la disquette : légère modulation à 5 Hz.
  const wobble = audioCtx.createGain();
  const lfo = audioCtx.createOscillator();
  lfo.frequency.value = 5;
  const depth = audioCtx.createGain();
  depth.gain.value = 0.3;
  lfo.connect(depth).connect(wobble.gain);
  const gain = audioCtx.createGain();
  gain.gain.value = 0;
  source.connect(filter).connect(wobble).connect(gain).connect(audioCtx.destination);
  source.start();
  lfo.start();
  return { gain, on: false };
}

function setMotor(on) {
  if (motor.on === on) return;
  motor.on = on;
  motor.gain.gain.setTargetAtTime(on ? 0.5 : 0, audioCtx.currentTime, on ? 0.08 : 0.25);
}

/** Un pas de la tête : claquement bref (bruit filtré) et coup sourd. */
function stepClick(at) {
  const source = audioCtx.createBufferSource();
  source.buffer = noise();
  const filter = audioCtx.createBiquadFilter();
  filter.type = 'bandpass';
  filter.frequency.value = 2200;
  filter.Q.value = 1.5;
  const gain = audioCtx.createGain();
  gain.gain.setValueAtTime(0.5, at);
  gain.gain.exponentialRampToValueAtTime(0.001, at + 0.02);
  source.connect(filter).connect(gain).connect(audioCtx.destination);
  source.start(at, Math.random() * 0.9, 0.03);
  const thump = audioCtx.createOscillator();
  thump.frequency.setValueAtTime(140, at);
  thump.frequency.exponentialRampToValueAtTime(60, at + 0.03);
  const thumpGain = audioCtx.createGain();
  thumpGain.gain.setValueAtTime(0.35, at);
  thumpGain.gain.exponentialRampToValueAtTime(0.001, at + 0.035);
  thump.connect(thumpGain).connect(audioCtx.destination);
  thump.start(at);
  thump.stop(at + 0.04);
}

/** Appelé à chaque image : fait entendre l'activité des lecteurs depuis l'image précédente. */
function driveNoise() {
  const steps = emulator.disk_steps();
  const accesses = emulator.disk_accesses();
  // Compteurs plus petits : nouvel émulateur (autre ROM), on repart de là.
  const newSteps = steps >= lastSteps ? steps - lastSteps : 0;
  const newAccesses = accesses >= lastAccesses ? accesses - lastAccesses : 0;
  lastSteps = steps;
  lastAccesses = accesses;
  if (!driveSound.checked || !audioCtx || audioCtx.state !== 'running') return;
  const now = audioCtx.currentTime;
  if (newAccesses > 0 || newSteps > 0) {
    motor ??= createMotor();
    setMotor(true);
    motorUntil = now + MOTOR_TIMEOUT;
  } else if (motor?.on && now > motorUntil) {
    setMotor(false);
  }
  // Les pas d'une image sont étalés (environ 6 à 20 ms par pas sur un vrai lecteur).
  clickTime = Math.max(clickTime, now + 0.02);
  for (let i = 0; i < Math.min(newSteps, 40); i++) {
    stepClick(clickTime);
    clickTime += turbo.checked ? 0.004 : 0.012;
  }
}

/** Joue les échantillons produits pendant les dernières images. */
function playAudio(fast) {
  const len = emulator.audio_len();
  if (len === 0) return;
  if (!audioCtx || audioCtx.state !== 'running' || fast) {
    emulator.clear_audio(); // Turbo ou frappe automatique : son accéléré, on le jette
    return;
  }
  const samples = new Float32Array(wasm.memory.buffer, emulator.audio_ptr(), len);
  const duration = len / audioCtx.sampleRate;
  const now = audioCtx.currentTime;
  if (nextAudioTime < now + 0.01 || nextAudioTime > now + 0.3) nextAudioTime = now + AUDIO_LATENCY;
  if (samples.some((x) => Math.abs(x) > 1e-4)) {
    const buffer = audioCtx.createBuffer(1, len, audioCtx.sampleRate);
    buffer.copyToChannel(samples, 0);
    const source = audioCtx.createBufferSource();
    source.buffer = buffer;
    source.connect(audioCtx.destination);
    source.start(nextAudioTime);
  }
  nextAudioTime += duration;
  emulator.clear_audio();
}

// ------------------------------------------------------------------ boucle

// L'émulateur produit une image de 384 × 192; elle est agrandie dans le canvas (1536 × 1152)
// sans lissage. Avec une police autre que celle d'origine, Rust ne dessine que les blocs
// graphiques et la page dessine le texte par-dessus (fonts.js).
const small = document.createElement('canvas');
const smallCtx = small.getContext('2d');
let image = null;
let lastTime = performance.now();
let frameDebt = 0;
let fpsFrames = 0;
let fpsTime = lastTime;

// ------------------------------------------------------------------ polices

const fontList = document.getElementById('font-list');
let font = FONTS[0];
let atlas = null; // glyphes de la police courante (null : police d'origine, dessinée par Rust)

/** Nom d'une police dans la langue courante (les noms propres restent tels quels). */
function fontLabel(f) {
  const key = `font.${f.id}`;
  const text = t(key);
  return text === key ? f.label : text;
}

function relabelFontList() {
  for (const optgroup of fontList.querySelectorAll('optgroup')) {
    optgroup.label = t(`font.group.${optgroup.dataset.group}`);
  }
  for (const option of fontList.options) option.textContent = fontLabel(FONTS.find((f) => f.id === option.value));
}
languageListeners.push(relabelFontList);

for (const group of [...new Set(FONTS.map((f) => f.group))]) {
  const optgroup = document.createElement('optgroup');
  optgroup.dataset.group = group;
  for (const f of FONTS.filter((x) => x.group === group)) optgroup.append(new Option('', f.id));
  fontList.append(optgroup);
}
relabelFontList();

async function selectFont(id) {
  const chosen = FONTS.find((f) => f.id === id) ?? FONTS[0];
  try {
    atlas = chosen.family ? await buildAtlas(chosen) : null;
    font = chosen;
    try { localStorage.setItem('trs80-font', chosen.id); } catch { /* stockage indisponible */ }
  } catch (e) {
    showStatus(t('font.fail', { name: fontLabel(chosen), msg: e.message ?? e }), true);
  }
  fontList.value = font.id;
  lastVideo = null; // force le redessin
  if (emulator) draw();
}

fontList.addEventListener('change', () => { selectFont(fontList.value); focusScreen(); });

// Dernier contenu dessiné : on ne redessine que si l'écran du TRS-80 a changé.
let lastVideo = null;
let lastWide = false;
let lastScreenState = '';

/** Caractères affichés (64 × 16, ou 80 × 24 sur le Model 4). */
function shownText() {
  const len = emulator.text_cols() * emulator.text_rows();
  return new Uint8Array(wasm.memory.buffer, emulator.video_ptr(), len);
}

function screenChanged() {
  const video = shownText();
  if (lastVideo && lastVideo.length !== video.length) lastVideo = null;
  const wide = emulator.wide();
  // Graphique haute résolution et vidéo inversée : hors de la mémoire texte.
  const state = `${emulator.hires_active()}/${emulator.hires_changes()}/${emulator.inverse_video()}`;
  if (lastVideo && wide === lastWide && state === lastScreenState && video.every((b, i) => b === lastVideo[i])) return false;
  lastVideo = video.slice();
  lastWide = wide;
  lastScreenState = state;
  return true;
}

function draw() {
  // Police choisie (fonts.js) : la page dessine le texte par-dessus les blocs graphiques.
  // Le graphique haute résolution (640 × 240) inclut son texte, en police d'origine.
  const fontText = atlas && !emulator.hires_active();
  const ptr = fontText ? emulator.render_graphics() : emulator.render();
  const w = emulator.screen_width();
  const h = emulator.screen_height();
  if (!image || image.width !== w || image.height !== h) {
    small.width = w;
    small.height = h;
    image = smallCtx.createImageData(w, h);
  }
  // Lecture directe de l'image dans la mémoire Wasm, sans copie intermédiaire.
  image.data.set(new Uint8ClampedArray(wasm.memory.buffer, ptr, w * h * 4));
  smallCtx.putImageData(image, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(small, 0, 0, canvas.width, canvas.height);
  if (fontText) {
    const model = emulator.model();
    drawText(ctx, atlas, shownText(), {
      cols: emulator.text_cols(),
      rows: emulator.text_rows(),
      wide: emulator.wide(),
      lowercase: model !== 1,
      inverse: emulator.inverse_video(),
      triangles: model === 2,
      arrows: model !== 2,
    });
  }
}

// ------------------------------------------------------------------ atelier d'assemblage

/** Nouvel émulateur : fin de la session de débogage. (start ne s'exécute qu'après la création
 * de l'atelier, plus bas : premier démarrage en fin de module, puis choix de l'utilisateur.) */
function ideReset() {
  ide.reset();
}

const ide = createIde({
  t,
  element,
  textElement,
  setTip,
  iconButton,
  download,
  showStatus,
  showScreen,
  emulator: () => emulator,
  assemble,
  builtins: () => JSON.parse(asm_builtins_json()),
  hasDos: () => driveRows[0].hasDisk(),
  diskChanged: () => driveRows.forEach((r) => r.refresh()),
  redraw: () => {
    lastVideo = null;
    if (emulator) draw();
  },
});
languageListeners.push(() => ide.onLanguage());

const ideToggle = document.getElementById('ide-toggle');
function showIde(on) {
  app.classList.toggle('ide-mode', on);
  document.getElementById('ide').hidden = !on;
  document.getElementById('ide-regs').hidden = !on;
  ideToggle.setAttribute('aria-pressed', String(on));
  prefs.ide = on;
  savePrefs();
  if (on) ide.show();
}
ideToggle.addEventListener('click', () => showIde(!app.classList.contains('ide-mode')));
showIde(!!prefs.ide || new URLSearchParams(location.search).has('ide'));

function loop(now) {
  if (emulator && ide.paused()) {
    frameDebt = 0; // débogueur en pause : la machine attend
  } else if (emulator) {
    // Rythme réel de 60 images/s, même si l'écran de l'hôte rafraîchit à 120 Hz ou plus.
    frameDebt += (now - lastTime) * 60 / 1000;
    frameDebt = Math.min(frameDebt, 5); // pas de rattrapage après une pause d'onglet
    const frames = Math.floor(frameDebt);
    if (frames > 0) {
      frameDebt -= frames;
      let emulated = frames * (turbo.checked ? 10 : emulator.typing() ? 4 : 1);
      emulator.run_frames(emulated);
      // Cassette rapide : tant que le moteur tourne, toute la machine va aussi vite que le
      // permet le fureteur (environ 12 ms de calcul par image). Les impulsions gardent leur
      // durée en temps émulé : les chargeurs qui les mesurent ne voient pas la différence.
      if (fastTapeBox.checked && emulator.tape_progress()[2] === 1) {
        const until = performance.now() + 12;
        while (performance.now() < until && emulator.tape_progress()[2] === 1) {
          emulator.run_frames(10);
          emulated += 10;
        }
      }
      // RS-232 : ce que le TRS-80 a émis va au modem.
      const printed = emulator.printer_take();
      if (printed.length) printerAppend(printed);
      const serialOut = emulator.serial_take();
      if (serialOut.length) modem.write(serialOut);
      playAudio(emulated !== frames);
      driveNoise();
      frameCount += emulated;
      releaseDueKeys();
      ide.afterFrames();
      if (!ansiActive() && screenChanged()) draw();
      fpsFrames += emulated;
    }
    if (now - fpsTime >= 1000) {
      const mhz = fpsFrames * emulator.current_hz() / 60 / 1e6;
      speedBox.textContent = `${mhz.toFixed(2)} MHz`;
      fpsFrames = 0;
      fpsTime = now;
    }
  }
  drawAnsi();
  lastTime = now;
  requestAnimationFrame(loop);
}

/** Écran couleur du modem : recopié sur l'écran quand il change; au raccrochage, l'écran du
 *  TRS-80 revient. */
function drawAnsi() {
  const active = !!emulator && ansiActive();
  if (active) {
    if (!ansiShown) ansiScreen.dirty = true;
    if (ansiScreen.dirty) {
      ansiScreen.draw();
      ctx.imageSmoothingEnabled = true;
      ctx.drawImage(ansiCanvas, 0, 0, canvas.width, canvas.height);
    }
  } else if (ansiShown && emulator) {
    lastVideo = null;
    draw();
  }
  ansiShown = active;
}

// ------------------------------------------------------------------ outils des disquettes

// Depuis la barre des lecteurs : démarrer un DOS compatible avec la disquette du lecteur 0
// (elle passe au lecteur 1), formater une disquette, gérer ses fichiers.
const diskDialog = document.getElementById('disk-dialog');

/**
 * DOS système compatible avec une disquette, selon le modèle : une disquette publiée
 * (`local`, dans disks/) ou du dépôt (`repo`, chemin dans disk/). null : aucun.
 */
function compatibleDos(dos) {
  const family = dos?.family;
  const name = dos?.name ?? '';
  switch (prefs.model) {
    case 1:
      if (family === 'trsdos13' || family?.startsWith('trsdos2-')) return null;
      return name.startsWith('TRSDOS 2')
        ? { title: 'TRSDOS 2.3', repo: 'model1/trsdos-21.dmk' }
        : { title: 'LDOS 5.3.1', local: 'ldos-531-dd.dsk' };
    case 3:
      if (family?.startsWith('trsdos2-')) return null;
      return family === 'trsdos13'
        ? { title: 'TRSDOS 1.3', repo: 'model3/trsdos13-m3.dsk' }
        : { title: 'LDOS 5.3.1', local: 'ldos-531-m3.dsk' };
    case 4:
      if (family?.startsWith('trsdos2-')) return null;
      if (family === 'trsdos13') return { title: 'TRSDOS 1.3', repo: 'model3/trsdos13-m3.dsk' };
      return name.startsWith('LDOS 5')
        ? { title: 'LDOS 5.3.1', local: 'ldos-531-m3.dsk' }
        : { title: 'TRSDOS 6.2.1', repo: 'model4/trsdos621-m4.dsk' };
    case 2:
      return family === 'trsdos2-4'
        ? { title: 'TRSDOS-II 4.2', repo: 'model2/trsdos42-m2.imd' }
        : { title: 'TRSDOS-II 2.0a', local: 'trsdos20a-m2.imd' };
    default:
      return null;
  }
}

async function fetchBytes(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

/** La disquette du lecteur 0 passe au lecteur 1; un DOS compatible démarre au lecteur 0. */
async function bootWithDos() {
  if (!emulator) return;
  const target = compatibleDos(driveDos(0));
  const data = driveRows[0].snapshot();
  if (!target || !data) {
    showStatus(t('dos.none', { model: t(`model.${prefs.model}`) }), true);
    return;
  }
  // Le lecteur 1 occupé : sa disquette va au premier lecteur libre (2 ou 3).
  if (driveRows[1].hasDisk()) {
    const free = [2, 3].find((i) => !driveRows[i].hasDisk());
    const occupant = driveRows[1].snapshot();
    if (free !== undefined && occupant) driveRows[free].insert(occupant.name, occupant.bytes);
    else if (!confirm(t('dos.replace1', { name: driveRows[1].contents()?.name ?? '' }))) return;
  }
  showStatus(t('dos.fetching', { name: target.title }));
  let bytes;
  try {
    bytes = await fetchBytes(target.local ? `disks/${target.local}${V}` : repoUrl(`disk/${target.repo}`));
  } catch (e) {
    showStatus(t('download.fail', { name: target.title, msg: failure(e) }), true);
    return;
  }
  driveRows[1].insert(data.name, data.bytes);
  const dosFile = (target.local ?? target.repo).split('/').pop();
  if (driveRows[0].insertAndBoot(dosFile, bytes, null, { name: dosFile })) {
    showStatus(t('dos.booted', { dos: target.title, name: data.name }));
    showScreen();
  }
}

/** Ouvre la fenêtre des disquettes avec ce contenu. */
function showDialog(...children) {
  diskDialog.replaceChildren(...children);
  if (!diskDialog.open) diskDialog.showModal();
}

function dialogButton(key, handler, extra = 'secondary') {
  const b = textElement('button', extra, key);
  b.type = 'button';
  b.addEventListener('click', handler);
  return b;
}

/** Date du jour au format des DOS : MM/DD/YY. */
function dosDate() {
  const d = new Date();
  const two = (n) => String(n).padStart(2, '0');
  return `${two(d.getMonth() + 1)}/${two(d.getDate())}/${two(d.getFullYear() % 100)}`;
}

/** Formatage : choix du format (selon le modèle) et du nom, puis confirmation. */
function openFormatDialog(drive) {
  if (!emulator) return;
  const formats = JSON.parse(Emulator.disk_formats(prefs.model));
  const title = element('h2', '', t('fmt.title', { n: drive }));
  if (!formats.length) {
    const close = element('div', 'dialog-actions');
    close.append(element('span', 'spacer'), dialogButton('fmt.cancel', () => diskDialog.close()));
    showDialog(title, element('p', 'hint', t('fmt.none', { model: t(`model.${prefs.model}`) })), close);
    return;
  }
  const select = element('select');
  for (const f of formats) select.append(new Option(f.name, f.id));
  const kindField = element('label', 'field');
  kindField.append(element('span', '', t('fmt.format')), select);
  const nameInput = element('input');
  nameInput.maxLength = 8;
  nameInput.value = 'DATADISK';
  nameInput.spellcheck = false;
  const nameField = element('label', 'field');
  nameField.append(element('span', '', t('fmt.name')), nameInput);
  const current = driveRows[drive].contents();
  const warning = element('p', 'warning', current ? t('fmt.warn', { n: drive, name: current.name }) : '');
  warning.hidden = !current;
  const actions = element('div', 'dialog-actions');
  actions.append(element('span', 'spacer'), dialogButton('fmt.cancel', () => diskDialog.close()), dialogButton('fmt.go', () => {
    const name = nameInput.value.trim().toUpperCase().replace(/[^A-Z0-9-]/g, '').slice(0, 8) || 'DATADISK';
    try {
      const bytes = Emulator.format_image(select.value, name, dosDate());
      if (driveRows[drive].insert(`${name.toLowerCase()}.dsk`, bytes)) {
        showStatus(t('fmt.done', { n: drive, format: select.selectedOptions[0].textContent, name }));
      }
    } catch (e) {
      showStatus(t('fmt.fail', { msg: failure(e) }), true);
    }
    diskDialog.close();
    renderDriveBar();
  }, 'danger'));
  showDialog(title, element('p', 'hint', t('fmt.hint')), kindField, nameField, warning, actions);
  select.focus();
}

/** Nom DOS (NOM/EXT, 8 et 3 caractères) d'un fichier du PC. */
function dosFileName(fileName) {
  const base = fileName.split(/[\\/]/).pop();
  const dot = base.lastIndexOf('.');
  const clean = (s, n) => s.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, n);
  let name = clean(dot > 0 ? base.slice(0, dot) : base, 8);
  const ext = dot > 0 ? clean(base.slice(dot + 1), 3) : '';
  if (!/^[A-Z]/.test(name)) name = `F${name}`.slice(0, 8);
  return ext ? `${name}/${ext}` : name;
}

/** Message d'une erreur du système de fichiers (code du cœur). */
function fsError(e, drive) {
  const code = e?.message ?? String(e);
  const key = `ide.fs.${code}`;
  const text = t(key, { d: drive < HARD_BASE ? drive : volumeLabel(drive) });
  return text === key ? code : text;
}

/** Fichiers d'une disquette : liste, téléchargement, ajout depuis le PC, copie, effacement. */
/** Message d'une action de la fenêtre des fichiers : ligne d'état, puis en tête de la fenêtre. */
let filesNote = null;
let filesNoteElement = null;
function report(text, error = false) {
  showStatus(text, error);
  filesNote = filesNote ? { text: `${filesNote.text} · ${text}`, error: filesNote.error || error } : { text, error };
  if (filesNoteElement?.isConnected) {
    filesNoteElement.textContent = filesNote.text;
    filesNoteElement.className = filesNote.error ? 'warning' : 'hint';
    filesNoteElement.hidden = false;
  }
}

function openFilesDialog(drive) {
  if (!emulator) return;
  const note = element('p', filesNote?.error ? 'warning' : 'hint', filesNote?.text ?? '');
  note.hidden = !filesNote;
  filesNote = null;
  filesNoteElement = note;
  const dos = driveDos(drive);
  const title = element('h2', '', t('files.title', { n: volumeLabel(drive), dos: dosText(dos) || t('dos.unknown') }));
  let files;
  try {
    files = JSON.parse(emulator.disk_files(drive));
  } catch (e) {
    const close = element('div', 'dialog-actions');
    close.append(element('span', 'spacer'), dialogButton('files.close', () => diskDialog.close()));
    showDialog(title, element('p', 'warning', dos && !dos.files ? t('files.unsupported', { dos: dos.name }) : fsError(e, drive)), close);
    return;
  }
  let free = null;
  try { free = emulator.disk_free(drive); } catch { /* espace libre inconnu */ }
  const boxes = [];
  const table = element('table', 'files-table');
  const head = element('tr');
  head.append(element('th', '', t('files.name')), element('th', '', t('files.size')), element('th', '', ''));
  table.append(head);
  for (const f of files) {
    const tr = element('tr', f.system ? 'system' : '');
    const box = element('input');
    box.type = 'checkbox';
    box.value = f.name;
    boxes.push(box);
    const label = element('label');
    label.append(box, f.name);
    const cell = element('td');
    cell.append(label);
    tr.append(cell, element('td', 'size', String(f.size)), element('td', '', f.system ? t('files.system') : ''));
    table.append(tr);
  }
  const scroll = element('div', 'files-scroll');
  scroll.append(table);
  const selected = () => boxes.filter((b) => b.checked).map((b) => b.value);
  const needSelection = (names) => {
    if (!names.length) report(t('files.select'), true);
    return names.length > 0;
  };
  const afterChange = () => {
    volumeRow(drive).refresh();
    renderDriveBar();
    openFilesDialog(drive);
  };

  const download = dialogButton('files.download', () => {
    filesNote = null;
    const names = selected();
    if (!needSelection(names)) return;
    for (const name of names) {
      try {
        downloadFile(emulator.read_disk_file(drive, name), name.replace('/', '.').toLowerCase());
      } catch (e) {
        report(`${name} : ${fsError(e, drive)}`, true);
      }
    }
  });
  const picker = element('input');
  picker.type = 'file';
  picker.multiple = true;
  picker.hidden = true;
  picker.addEventListener('change', async () => {
    filesNote = null;
    let n = 0;
    for (const file of picker.files) {
      const name = dosFileName(file.name);
      if (files.some((f) => f.name === name) && !confirm(t('files.confirmReplace', { name, d: volumeLabel(drive) }))) continue;
      try {
        emulator.write_disk_file(drive, name, new Uint8Array(await file.arrayBuffer()));
        n++;
      } catch (e) {
        report(`${name} : ${fsError(e, drive)}`, true);
      }
    }
    if (n) report(t('files.added', { n, d: volumeLabel(drive) }));
    afterChange();
  });
  const add = dialogButton('files.add', () => picker.click());
  // Copier vers un autre lecteur (avec une disquette dont on lit les fichiers).
  // Tous les lecteurs qui ont une disquette, avec leur DOS : si on ne sait pas écrire sur
  // l'une d'elles, l'écriture le dit (plutôt qu'une option absente sans explication).
  // Disquettes et disques durs : la copie va de l'un à l'autre dans les deux sens.
  const targets = [0, 1, 2, 3, HARD_BASE, HARD_BASE + 1].filter((i) => i !== drive && volumeRow(i)?.contents());
  const target = element('select');
  for (const i of targets) {
    const dos = driveDos(i);
    const label = dos ? dosText(dos) : t('dos.unknown');
    target.append(new Option(`${volumeLabel(i)} ${volumeRow(i).contents()?.name ?? ''} — ${label}`, String(i)));
  }
  const copy = dialogButton('files.copy', () => {
    filesNote = null;
    const names = selected();
    if (!needSelection(names) || !targets.length) return;
    const to = Number(target.value);
    let n = 0;
    for (const name of names) {
      try {
        emulator.copy_disk_file(drive, to, name);
        n++;
      } catch (e) {
        report(`${name} : ${fsError(e, to)}`, true);
      }
    }
    if (n) report(t('files.copied', { n, d: volumeLabel(to) }));
    volumeRow(to).refresh();
    afterChange();
  });
  copy.disabled = target.disabled = !targets.length;
  const remove = dialogButton('files.delete', () => {
    filesNote = null;
    const names = selected();
    if (!needSelection(names) || !confirm(t('files.confirmDelete', { n: names.length, d: volumeLabel(drive) }))) return;
    let n = 0;
    for (const name of names) {
      try {
        emulator.delete_disk_file(drive, name);
        n++;
      } catch (e) {
        report(`${name} : ${fsError(e, drive)}`, true);
      }
    }
    if (n) report(t('files.deleted', { n }));
    afterChange();
  }, 'danger');
  // Lecture seule (TRSDOS-II 4.x) : on télécharge et on copie vers un autre lecteur.
  const readOnly = dos?.writable === false;
  add.disabled = remove.disabled = readOnly;
  const actions = element('div', 'dialog-actions');
  actions.append(download, add, picker, copy, target, remove, element('span', 'spacer'), dialogButton('files.close', () => diskDialog.close()));
  const summary = [
    t('files.count', { n: files.length }),
    free != null ? t('files.free', { n: (free / 1024).toFixed(1) }) : null,
    readOnly ? t('files.readOnly', { dos: dos.name }) : null,
  ].filter(Boolean).join(' · ');
  showDialog(title, element('p', 'hint', summary), note, files.length ? scroll : element('p', 'hint', t('files.none')), actions);
}

// ------------------------------------------------------------------ administration

// Panneau Admin (Settings) : avec le mot de passe, vérifié par le serveur du dépôt
// (admin.php, voir server/admin/), on peut retirer un fichier de la liste présentée aux
// utilisateurs (il passe dans « À valider »), l'y remettre, changer sa catégorie, et tester
// le programme en cours sur les Model I, III et 4. Le mot de passe reste dans l'onglet
// (sessionStorage) et part avec chaque action.
const TEST_MODELS = [1, 3, 4];
const TEST_WAIT_MS = 5000;
const adminLogin = document.getElementById('admin-login');
const adminPasswordInput = document.getElementById('admin-password');
const adminTools = document.getElementById('admin-tools');
const adminStatus = document.getElementById('admin-status');
const adminPendingSelect = document.getElementById('admin-pending');
const adminPendingTitle = document.getElementById('admin-pending-title');
const adminTestButton = document.getElementById('admin-test');

async function adminCall(action, extra = {}) {
  // text/plain : requête « simple », sans demande préalable CORS.
  const res = await fetch(repoUrl('admin.php'), {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body: JSON.stringify({ password: adminPassword, action, ...extra }),
  });
  let data = null;
  try { data = await res.json(); } catch { /* réponse qui n'est pas du JSON */ }
  if (!data?.ok) throw new Error(data?.error ?? `HTTP ${res.status}`);
  return data;
}

function adminMessage(text, error = false) {
  adminStatus.textContent = text;
  adminStatus.classList.toggle('error', error);
  if (text) showStatus(text, error);
}

function applyAdmin() {
  adminLogin.hidden = !!adminPassword;
  adminTools.hidden = !adminPassword;
  renderNowAdmin();
}

adminLogin.addEventListener('submit', async (event) => {
  event.preventDefault();
  adminPassword = adminPasswordInput.value;
  try {
    await adminCall('login');
    try { sessionStorage.setItem('trs80-admin', adminPassword); } catch { /* stockage indisponible */ }
    adminPasswordInput.value = '';
    adminMessage(t('admin.welcome'));
    applyAdmin();
    await refreshPending();
  } catch (e) {
    adminPassword = null;
    adminMessage(e.message === 'bad password' ? t('admin.bad') : t('admin.fail', { msg: failure(e) }), true);
    applyAdmin();
  }
});

document.getElementById('admin-logout').addEventListener('click', () => {
  adminPassword = null;
  try { sessionStorage.removeItem('trs80-admin'); } catch { /* stockage indisponible */ }
  adminPending = [];
  adminMessage('');
  applyAdmin();
});

/** Liste « À valider » (fichiers retirés), lue sur le serveur. */
async function refreshPending() {
  if (!adminPassword) return;
  try {
    adminPending = (await adminCall('pending')).pending ?? [];
  } catch (e) {
    adminMessage(t('admin.fail', { msg: failure(e) }), true);
  }
  fillPending();
  renderNowAdmin();
}

function fillPending() {
  adminPendingTitle.textContent = t('admin.pending', { n: adminPending.length });
  adminPendingSelect.replaceChildren(...adminPending.map((p, i) => new Option(
    `${p.kind.toUpperCase()} · ${localized(p.entry, 'title') ?? p.entry.file}`, String(i))));
  if (!adminPending.length) adminPendingSelect.append(new Option(t('admin.pendingEmpty'), ''));
  adminPendingSelect.disabled = !adminPending.length;
  for (const id of ['admin-pending-run', 'admin-pending-restore']) {
    document.getElementById(id).disabled = !adminPending.length;
  }
}
languageListeners.push(fillPending);

const pickedPending = () => adminPending[Number(adminPendingSelect.value)];

/** Lance un fichier du dépôt (une disquette démarre au lecteur 0). */
async function runRepoFile(kind, entry) {
  if (!emulator) {
    showStatus(t('prog.hint'), true);
    return;
  }
  const name = entry.file.split('/').pop();
  try {
    const bytes = await fetchRepoFile(kind, entry);
    const info = repoInfo(kind, entry, name);
    if (kind === 'disk') {
      if (driveRows[0].insertAndBoot(name, bytes, null, info)) {
        lastRun = { bytes, name, info, disk: true };
        showScreen();
      }
    } else if (kind === 'asm') {
      showIde(true);
      ide.open(bytes, name);
    } else if (kind !== 'rom') {
      runFile(bytes, name, info);
    }
  } catch (e) {
    showStatus(t('download.fail', { name, msg: failure(e) }), true);
  }
}

document.getElementById('admin-pending-run').addEventListener('click', () => {
  const p = pickedPending();
  if (p) runRepoFile(p.kind, p.entry);
});
document.getElementById('admin-pending-restore').addEventListener('click', () => {
  const p = pickedPending();
  if (p) restoreFile({ kind: p.kind, file: p.entry.file }, localized(p.entry, 'title') ?? p.entry.file);
});

/** Après une modification de l'index : listes du dépôt relues, « À valider » aussi. */
async function afterAdminChange() {
  repoCache.clear();
  if (prefs.open.includes('repo')) loadRepo(prefs.repoKind);
  await refreshPending();
}

async function hideFile(repo, label) {
  if (!confirm(t('admin.hideConfirm', { name: label }))) return;
  try {
    await adminCall('hide', repo);
    adminMessage(t('admin.hidden', { name: label }));
    await afterAdminChange();
  } catch (e) {
    adminMessage(t('admin.fail', { msg: failure(e) }), true);
  }
}

async function restoreFile(repo, label) {
  try {
    await adminCall('restore', repo);
    adminMessage(t('admin.restored', { name: label }));
    await afterAdminChange();
  } catch (e) {
    adminMessage(t('admin.fail', { msg: failure(e) }), true);
  }
}

/** Modèles sur lesquels le fichier est proposé (au moins un). */
async function setModels(repo, entry, models, label) {
  try {
    const { model } = await adminCall('models', { ...repo, models });
    entry.model = model;
    adminMessage(t('admin.modelsChanged', { name: label, models: models.map((m) => `M${m}`).join('/') }));
    await afterAdminChange();
  } catch (e) {
    adminMessage(t('admin.fail', { msg: failure(e) }), true);
  }
  renderNowAdmin();
}

async function setCategory(repo, entry, category, label) {
  try {
    await adminCall('category', { ...repo, category });
    entry.category = category;
    adminMessage(t('admin.categoryChanged', { name: label, cat: t(`cat.${category}`) }));
    await afterAdminChange();
  } catch (e) {
    adminMessage(t('admin.fail', { msg: failure(e) }), true);
  }
}

const isPending = (repo) => adminPending.some((p) => p.kind === repo.kind && p.entry.file === repo.file);
const testKey = (run) => run.info?.repo?.file ?? run.name;

/** Actions de l'admin sous la description du programme en cours (fichier du dépôt). */
function renderNowAdmin() {
  const repo = nowSource?.repo;
  nowAdmin.hidden = !adminPassword || !repo || nowInfo.hidden;
  if (nowAdmin.hidden) return;
  const entry = nowSource.entry;
  const label = localized(entry, 'title') ?? nowSource.name;
  const row = element('div', 'admin-row');
  const button = (key, handler, disabled = false) => {
    const b = textElement('button', 'secondary', key);
    b.type = 'button';
    b.disabled = disabled;
    b.addEventListener('click', handler);
    row.append(b);
  };
  const testable = lastRun && testKey(lastRun) === repo.file && ['cmd', 'cas', 'bas', 'disk'].includes(repo.kind);
  button(testing ? 'admin.testRunning' : 'admin.test', testAllModels, testing || !testable);
  if (isPending(repo)) {
    row.append(element('span', 'admin-badge', t('admin.isPending')));
    button('admin.restore', () => restoreFile(repo, label));
  } else {
    button('admin.hide', () => hideFile(repo, label));
  }
  const cat = element('select');
  setTip(cat, 'admin.category');
  cat.setAttribute('aria-label', t('admin.category'));
  cat.append(...(entry.category ? [] : [new Option('—', '')]),
    ...ADMIN_CATEGORIES.map((c) => new Option(t(`cat.${c}`), c)));
  cat.value = entry.category ?? '';
  cat.addEventListener('change', () => cat.value && setCategory(repo, entry, cat.value, label));
  row.append(cat);
  // Modèles proposés : une case par modèle; la dernière cochée ne peut pas être décochée.
  const models = element('span', 'admin-models');
  setTip(models, 'admin.models');
  const current = repoModels(entry);
  for (const m of [1, 2, 3, 4]) {
    const box = element('input');
    box.type = 'checkbox';
    box.checked = current.includes(m);
    box.disabled = box.checked && current.length === 1;
    box.addEventListener('change', () => {
      const next = [1, 2, 3, 4].filter((x) => (x === m ? box.checked : current.includes(x)));
      setModels(repo, entry, next, label);
    });
    const lab = element('label');
    lab.append(box, `M${m}`);
    models.append(lab);
  }
  row.append(models);
  const parts = [row];
  const shots = testShots.get(repo.file);
  if (shots?.length) {
    const strip = element('div', 'test-shots');
    for (const s of shots) {
      const fig = element('figure');
      if (s.url) {
        const img = element('img');
        img.src = s.url;
        img.alt = t(`model.${s.model}`);
        fig.append(img);
      }
      fig.append(element('figcaption', '', s.url ? t(`model.${s.model}`) : t('admin.testNoRom', { model: t(`model.${s.model}`) })));
      strip.append(fig);
    }
    parts.push(strip);
  }
  nowAdmin.replaceChildren(...parts);
}
languageListeners.push(renderNowAdmin);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Vignette de l'écran. */
function snapshot() {
  const c = document.createElement('canvas');
  c.width = 384;
  c.height = 288;
  c.getContext('2d').drawImage(canvas, 0, 0, c.width, c.height);
  return c.toDataURL('image/png');
}

/** Attend la fin du chargement (bascule Level I, magnétophone, frappe), puis 5 s. */
async function settle() {
  const until = performance.now() + 240_000;
  await sleep(1000);
  while (performance.now() < until) {
    if (levelSwitch) await levelSwitch;
    const tape = emulator?.tape_progress() ?? [];
    if (!(tape[2] === 1 || emulator?.typing())) break;
    await sleep(250);
  }
  await sleep(TEST_WAIT_MS);
}

/** Lance le dernier programme sur les Model I, III et 4, avec une vignette de chaque écran. */
async function testAllModels() {
  const run = lastRun;
  if (testing || !run) {
    if (!run) adminMessage(t('admin.noRun'), true);
    return;
  }
  testing = true;
  const key = testKey(run);
  const shots = [];
  testShots.set(key, shots);
  adminTestButton.disabled = true;
  renderNowAdmin();
  try {
    for (const model of TEST_MODELS) {
      showStatus(t('admin.testing', { model: t(`model.${model}`) }));
      if (prefs.model !== model) {
        modelList.value = String(model);
        await changeModel(model);
      }
      if (!emulator) {
        shots.push({ model, url: null });
        continue;
      }
      if (run.disk) {
        if (driveRows[0].insertAndBoot(run.name, run.bytes, null, run.info)) showScreen();
      } else {
        runFile(run.bytes, run.name, run.info);
      }
      await settle();
      shots.push({ model, url: snapshot() });
      renderNowAdmin();
    }
    lastRun = run;
    showStatus(t('admin.tested'));
  } finally {
    testing = false;
    adminTestButton.disabled = false;
    if (!nowSource?.repo && run.info) showNowInfo(run.info, run.disk ? 'disk' : 'program');
    renderNowAdmin();
  }
}
adminTestButton.addEventListener('click', testAllModels);

applyAdmin();
if (adminPassword) refreshPending();

await Promise.all([loadProgramIndex(), loadRomIndex(), loadDiskIndex(), refreshLibrary()]);
const params = new URLSearchParams(location.search);
// Police : ?font=<id>, sinon la dernière choisie.
let savedFont = null;
try { savedFont = localStorage.getItem('trs80-font'); } catch { /* stockage indisponible */ }
await selectFont(params.get('font') ?? savedFont ?? 'trs80');
// Lien direct vers une ROM de la liste : ?rom=level2-1.3 (a priorité sur la ROM conservée).
const modelParam = Number(params.get('model'));
if ([1, 2, 3, 4].includes(modelParam)) prefs.model = modelParam;
applyModel();
// Section ouverte à la dernière visite : fichiers du modèle choisi (après ?model=).
if (prefs.open.includes('repo')) loadRepo(prefs.repoKind);
const romParam = params.get('rom');
if (romParam && roms.some((r) => r.id === romParam && r.source === 'url')) {
  selectRomInList(romParam);
  await chooseListedRom(romParam);
} else {
  const saved = (await loadDevRom()) ?? (await loadSavedRom());
  if (saved) {
    if (start(saved.bytes)) selectRomInList(saved.id);
  } else if (!defaultRom()) {
    showStatus(t('rom.ownM2'));
  } else {
    // Première visite : démarrage avec la ROM officielle de la liste (Level II 1.3 ou Model III).
    selectRomInList(defaultRom());
    await chooseListedRom(defaultRom());
  }
}
if (emulator) {
  // Lien direct vers une disquette : ?disk=ldos-531
  const diskParam = params.get('disk');
  if (diskParam) {
    diskList.value = diskParam;
    await bootDisk(diskParam);
  }
  // Sinon, les disques de la session précédente (avec leurs écritures).
  if (!diskParam && !params.get('program')) await restoreSession();
  // Lien direct vers un programme : ?program=seadragon
  const program = params.get('program');
  if (program) {
    programList.value = program;
    await runProgram(program);
  }
  // Développement : ?frames=N exécute N images dès le chargement (captures d'écran, tests).
  const warmup = Number(params.get('frames') ?? 0);
  if (warmup > 0) {
    emulator.run_frames(warmup);
    draw();
  }
}
sessionReady ||= !emulator || !!params.get('disk') || !!params.get('program');
requestAnimationFrame(loop);
