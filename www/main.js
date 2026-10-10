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
const soundBox = document.getElementById('sound');
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
  keepFiles: true,      // garder dans la bibliothèque les fichiers ouverts
  keepSession: true,    // retrouver les disques (avec leurs écritures) au rechargement
  modemRelay: null,     // adresse du relais telnet (null : celle par défaut)
  modemFilter: true,    // modem : retirer les séquences ANSI
  bbsPick: 'bbs.electrodrome.net:23', // BBS choisi dans la liste du modem
  driveSound: false,    // imiter le bruit des lecteurs de disquettes
  repo: 'https://ve2cuy.com/trs80', // dépôt externe (dossiers rom, disk, cmd, bas)
  repoKind: 'rom',
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
turbo.addEventListener('change', () => { prefs.turbo = turbo.checked; savePrefs(); focusScreen(); });

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
window.addEventListener('error', (e) => showStatus(t('error', { msg: e.message }), true));
window.addEventListener('unhandledrejection', (e) =>
  showStatus(t('error', { msg: e.reason?.message ?? e.reason }), true));

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
    const res = await fetch(rom.url);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    if (rom.sha256 && (await sha256Hex(bytes)) !== rom.sha256) {
      throw new Error(t('rom.checksum'));
    }
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
  pasteButton.disabled = copyButton.disabled = !running;
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
  if (audioCtx && soundBox.checked) emulator.set_audio_rate(audioCtx.sampleRate);
  // Nouvelle ROM : les disquettes déjà insérées le restent (images d'origine).
  for (const row of driveRows) row.reinsert();
  hardRows.forEach((row, i) => row.reinsert(hardImages[i]));
  setRunning(true);
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
let nowKind = null;
let nowSource = null;

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
  nowInfo.hidden = false;
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
  return `${name}: ${message}`;
}

function diskMessage(desc) {
  const m = desc.match(/^(\S+), (\d+) sectors(, write-protected)?$/);
  return m ? t('disk.desc', { format: m[1], n: m[2] }) + (m[3] ? t('disk.protected') : '') : desc;
}

/**
 * Charge un programme selon son extension : cassette .CAS, listing .BAS ou exécutable .CMD,
 * et affiche son nom et sa description (`info`, sinon cherchée par describe).
 */
function runFile(bytes, name, info = null) {
  if (!emulator) return;
  info ??= describe(name, bytes);
  if (/\.(bas|txt)$/i.test(name)) {
    runBasic(bytes, info);
    return;
  }
  const label = describeSource(info).title;
  try {
    if (/\.cas$/i.test(name)) {
      showStatus(casMessage(label, emulator.load_cas(bytes)));
    } else {
      const entry = emulator.load_cmd(bytes);
      showStatus(t('run.cmd', { name: label, addr: entry.toString(16).toUpperCase().padStart(4, '0') }));
    }
    showNowInfo(info, /\.cas$/i.test(name) && info.basic ? 'basic' : 'program');
  } catch (e) {
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
      if (driveRows[0].insertAndBoot(name, bytes, libraryId, info)) showScreen();
    } else if (driveRows[n].insert(name, bytes, libraryId)) {
      focusScreen();
    }
  });
  return into;
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
  // Erreur d'insertion, affichée sur la ligne du lecteur pour ne pas passer inaperçue.
  const error = element('span', 'drive-error');
  error.setAttribute('role', 'alert');
  actions.append(insertLabel, blank, eject, keep, save);
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
    eject.disabled = keep.disabled = save.disabled = !current || !emulator;
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

function renderDriveBar() {
  const now = performance.now();
  // Format du DOS du lecteur 0 : une disquette d'une autre famille aux lecteurs 1 à 3 est
  // signalée (en rouge).
  const system = driveRows[0].contents() ? diskFamily(diskGeometryOf(0)) : null;
  const chips = [
    ...driveRows.map((r, i) => {
      const c = r.contents();
      const family = c ? diskFamily(diskGeometryOf(i)) : null;
      const clash = i > 0 && system && family && family !== system;
      return {
        no: String(i), c, row: r, geo: c ? diskGeometry(i) : '',
        clash: clash ? t('bar.clash', { disk: t(`family.${family}`), system: t(`family.${system}`) }) : '',
      };
    }),
    ...hardRows.map((r, u) => ({ no: `HD${u + 1}`, c: r.contents(), row: r, geo: hardGeometry(u), clash: '' })).filter((x) => x.c),
  ].map((x) => {
    const a = lastAccess.get(x.no);
    return { ...x, access: a && now - a.at < ACCESS_SHOWN_MS ? a.text : '' };
  });
  const state = JSON.stringify([document.documentElement.lang, !!emulator, chips.map(({ row, ...x }) => x)]);
  if (state === driveBarState) return;
  driveBarState = state;
  driveBar.hidden = !emulator;
  driveBar.replaceChildren(...chips.map(({ no, c, row, geo, clash, access }) => {
    const chip = element('div', 'drive-chip');
    chip.classList.toggle('empty', !c);
    chip.classList.toggle('modified', !!c?.modified);
    chip.classList.toggle('active', !!access);
    chip.classList.toggle('clash', !!clash);
    // Le corps ouvre la section Disquettes; le bouton éjecte.
    const main = element('button', 'chip-main');
    main.type = 'button';
    main.title = [c ? c.name : t('bar.empty'), geo, clash, t('bar.open')].filter(Boolean).join('\n');
    const head = element('span', 'chip-head');
    head.append(element('span', 'no', no), element('span', 'file', c ? c.name : t('bar.empty')));
    main.append(head);
    if (geo) main.append(element('span', 'geo', geo));
    if (clash) main.append(element('span', 'clash-text', clash));
    if (access) main.append(element('span', 'access', access));
    main.addEventListener('click', () => showPanel('disks'));
    chip.append(main);
    if (c) {
      const eject = iconButton('eject', 'drive.eject');
      eject.addEventListener('click', () => row.eject());
      chip.append(eject);
    }
    return chip;
  }));
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
const REPO_ICONS = { rom: 'cpu', disk: 'disk', cmd: 'file', bas: 'basic' };
const repoSelect = document.getElementById('repo-select');
const repoDetail = document.getElementById('repo-detail');
let repoShown = []; // entrées de la liste déroulante (catégorie et modèle courants)
const repoStatus = document.getElementById('repo-status');
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
    const shown = prefs.repoAll ? entries : entries.filter((e) => repoModels(e).includes(prefs.model));
    let summary;
    if (!entries.length) summary = t('repo.empty');
    else if (!shown.length) summary = t('repo.noneForModel', { model, n: entries.length });
    else if (prefs.repoAll) summary = t('repo.countAll', { n: entries.length });
    else summary = t('repo.count', { n: shown.length, total: entries.length, model });
    repoStatus.textContent = shown.length ? `${prefs.repo}/${kind}/ — ${summary}` : summary;
    fillRepoSelect(kind, shown);
  } catch (e) {
    if (prefs.repoKind !== kind) return;
    fillRepoSelect(kind, []);
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
    return new Option(`${localized(e, 'title') ?? name}${models}`, String(i));
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
  meta.append(title, element('span', 'lib-sub', [models, name, entry.year, entry.authors].filter(Boolean).join(' · ')));
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
    actions.append(placeSelect(name, async () => {
      try {
        return await fetchRepoFile(kind, entry);
      } catch (e) {
        showStatus(t('download.fail', { name, msg: failure(e) }), true);
        return null;
      }
    }, { info: entryInfo(entry, name) }));
  } else {
    button('lib.run', withFile((bytes) => runFile(bytes, name, entryInfo(entry, name))));
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

const modem = new Modem({
  send: (bytes) => emulator?.serial_send(bytes),
  relay: () => prefs.modemRelay || DEFAULT_RELAY,
  baud: () => emulator?.serial_baud() ?? 300,
  filter: () => prefs.modemFilter,
  onState: (state) => {
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
  if (!emulator.key_down(name)) return null;
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
  if (!emulator || e.altKey || e.metaKey) return;
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
    pressed.set(e.code, key);
    e.preventDefault();
    if (name === 'CapsLock' && emulator.model() === 2) showStatus(t(emulator.caps() ? 'caps.on' : 'caps.off'));
  }
});

window.addEventListener('keyup', (e) => {
  if (!emulator) return;
  const key = pressed.get(e.code);
  pressed.delete(e.code);
  if (key) {
    releaseKey(key);
  } else if (e.target !== touchInput) {
    emulator.key_up(e.key);
  }
});

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

/** Caractères affichés (64 × 16, ou 80 × 24 sur le Model 4). */
function shownText() {
  const len = emulator.text_cols() * emulator.text_rows();
  return new Uint8Array(wasm.memory.buffer, emulator.video_ptr(), len);
}

function screenChanged() {
  const video = shownText();
  if (lastVideo && lastVideo.length !== video.length) lastVideo = null;
  const wide = emulator.wide();
  if (lastVideo && wide === lastWide && video.every((b, i) => b === lastVideo[i])) return false;
  lastVideo = video.slice();
  lastWide = wide;
  return true;
}

function draw() {
  // Les polices de fonts.js sont prévues pour 64 × 16 : en 80 × 24, la police d'origine.
  const fontText = atlas && emulator.text_cols() === 64;
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
  if (fontText) drawText(ctx, atlas, shownText(), emulator.wide());
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
      const emulated = frames * (turbo.checked ? 10 : emulator.typing() ? 4 : 1);
      emulator.run_frames(emulated);
      // RS-232 : ce que le TRS-80 a émis va au modem.
      const serialOut = emulator.serial_take();
      if (serialOut.length) modem.write(serialOut);
      playAudio(emulated !== frames);
      driveNoise();
      frameCount += emulated;
      releaseDueKeys();
      ide.afterFrames();
      if (screenChanged()) draw();
      fpsFrames += frames;
    }
    if (now - fpsTime >= 1000) {
      const mhz = fpsFrames * (turbo.checked ? 10 : 1) * emulator.current_hz() / 60 / 1e6;
      speedBox.textContent = `${mhz.toFixed(2)} MHz`;
      fpsFrames = 0;
      fpsTime = now;
    }
  }
  lastTime = now;
  requestAnimationFrame(loop);
}

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
