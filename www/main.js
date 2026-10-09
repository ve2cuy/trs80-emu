// Page de l'émulateur : charge le module WebAssembly, la ROM et les programmes, puis
// fait tourner la boucle d'affichage. Toute l'émulation est dans Rust (crates/web).
// Les textes de l'interface sont dans i18n.js (anglais, français, espagnol, chinois).

// Version de déploiement (?v=<commit> inscrit par GitHub Actions dans index.html) : ajoutée
// à chaque fichier chargé, pour qu'une mise à jour ne mélange jamais anciens et nouveaux
// fichiers gardés en cache. En développement (?v=dev), on ne met rien en cache.
const BUILD = new URL(import.meta.url).searchParams.get('v') ?? 'dev';
const V = `?v=${BUILD === 'dev' ? Date.now() : BUILD}`;

const { LANGUAGES, browserLanguage, getLanguage, setLanguage, t, localized } = await import(`./i18n.js${V}`);
const { default: init, Emulator } = await import(`./pkg/trs80_web.js${V}`);
const { FONTS, buildAtlas, drawText } = await import(`./fonts.js${V}`);

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
  driveSound: false,    // imiter le bruit des lecteurs de disquettes
  repo: 'https://ve2cuy.com/trs80', // dépôt externe (dossiers rom, disk, cmd, bas)
  repoKind: 'rom',
  lang: null,           // langue de l'interface (null : celle du fureteur)
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
langList.value = setLanguage(langParam in LANGUAGES ? langParam : prefs.lang ?? browserLanguage());
langList.addEventListener('change', () => {
  prefs.lang = setLanguage(langList.value);
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
const WIDTH = Emulator.width();
const HEIGHT = Emulator.height();

let emulator = null;

// ------------------------------------------------------------------ ROM

// IndexedDB conserve la ROM (magasin « roms ») et la bibliothèque de l'utilisateur
// (magasin « files » : disquettes, programmes, listings BASIC). Version 1 : la ROM seulement.
const DB_NAME = 'trs80-emu';
let dbPromise = null;

function openDb() {
  dbPromise ??= new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 2);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains('roms')) db.createObjectStore('roms');
      if (!db.objectStoreNames.contains('files')) {
        db.createObjectStore('files', { keyPath: 'id', autoIncrement: true });
      }
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
async function saveRom(id, bytes) {
  try {
    await dbRequest('roms', 'readwrite', (s) => s.put({ id, bytes }, 'level2'));
  } catch { /* stockage indisponible (navigation privée) : on s'en passe */ }
}

async function loadSavedRom() {
  try {
    const value = await dbRequest('roms', 'readonly', (s) => s.get('level2'));
    if (value instanceof Uint8Array) return { id: 'custom', bytes: value };
    return value ?? null;
  } catch {
    return null;
  }
}

// En développement : une ROM placée dans www/rom/level2.rom (exclue de Git) est chargée d'office.
async function loadDevRom() {
  try {
    const res = await fetch('rom/level2.rom');
    return res.ok ? { id: 'custom', bytes: new Uint8Array(await res.arrayBuffer()) } : null;
  } catch {
    return null;
  }
}

// ROM proposées (roms.json) : téléchargées chez un tiers quand on les choisit, ou au premier
// démarrage pour la ROM par défaut.
const DEFAULT_ROM = 'level2-1.3';
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
  for (const r of roms) {
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
    showStatus(t('download.fail', { name: localized(rom, 'title'), msg: e.message ?? e }), true);
  }
}

romList.addEventListener('change', () => chooseListedRom(romList.value));

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
  resetButton.disabled = !running;
  programList.disabled = !running;
  cmdFile.disabled = !running;
  cmdButton.classList.toggle('disabled', !running);
  typeButton.disabled = !running;
  programsHint.hidden = running;
  diskList.disabled = !running;
  for (const row of driveRows) row.setEnabled(running);
  renderLibrary();
}

function start(bytes) {
  try {
    emulator?.free();
    emulator = new Emulator(bytes);
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
  setRunning(true);
  hideNowInfo();
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
    showStatus(t('download.fail', { name: localized(p, 'title'), msg: e.message ?? e }), true);
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
function driveSelect() {
  const into = element('select');
  setTip(into, 'lib.into.tip');
  const first = new Option(t('lib.drive'), '');
  first.dataset.i18n = 'lib.drive';
  into.append(first, ...[1, 2, 3].map((d) => new Option(t('lib.driveN', { n: d }), d)));
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
  input.accept = '.dsk,.dmk,.jv1,.jv3';
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
    error.textContent = '';
    showStatus(t('drive.blankStatus', { n: drive }));
    refresh();
    focusScreen();
  });
  eject.addEventListener('click', () => {
    emulator?.eject_disk(drive);
    current = null;
    error.textContent = '';
    refresh();
    focusScreen();
  });
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
  };
}

const driveRows = [0, 1, 2, 3].map(makeDriveRow);
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
  for (const d of disks) diskList.append(new Option('', d.id));
  relabelDiskList();
}

function relabelDiskList() {
  for (const option of diskList.options) {
    const d = disks.find((x) => x.id === option.value);
    if (d) option.textContent = `${localized(d, 'title')}${d.year ? ` (${d.year})` : ''}${d.local ? ` — ${t('disk.local')}` : ''}`;
  }
}
languageListeners.push(relabelDiskList);

async function bootDisk(id) {
  const d = disks.find((x) => x.id === id);
  if (!d) return;
  try {
    const res = await fetch(`disks/${d.file}${V}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const name = d.file.split('/').pop();
    driveRows[0].insertAndBoot(name, new Uint8Array(await res.arrayBuffer()), null, entryInfo(d, name));
  } catch (e) {
    showStatus(t('download.fail', { name: localized(d, 'title'), msg: e.message ?? e }), true);
  }
  showScreen();
}

diskList.addEventListener('change', () => bootDisk(diskList.value));

// ------------------------------------------------------------------ bibliothèque

// Fichiers de l'utilisateur, conservés dans IndexedDB (ce fureteur, cet appareil) :
// { id, name, kind, bytes, size, updated }.
const KINDS = [
  { kind: 'disk', pattern: /\.(dsk|dmk|jv1|jv3)$/i, icon: 'disk' },
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

  if (file.kind === 'disk') {
    const boot = textElement('button', 'secondary', 'lib.boot');
    boot.type = 'button';
    setTip(boot, 'lib.boot.tip');
    boot.addEventListener('click', () => {
      if (driveRows[0].insertAndBoot(file.name, file.bytes, file.id)) showScreen();
    });
    const into = driveSelect();
    into.addEventListener('change', () => {
      const d = Number(into.value);
      into.value = '';
      if (d && driveRows[d].insert(file.name, file.bytes, file.id)) focusScreen();
    });
    actions.append(boot, into);
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
// { file, title, year, authors, description, license } ou simplement des noms de fichiers.
// Le serveur doit permettre les requêtes d'une autre origine (CORS).
const DEFAULT_REPO = 'https://ve2cuy.com/trs80';
const REPO_ICONS = { rom: 'cpu', disk: 'disk', cmd: 'file', bas: 'basic' };
const repoList = document.getElementById('repo-list');
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
  repoList.replaceChildren();
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
    repoStatus.textContent = entries.length ? `${prefs.repo}/${kind}/` : t('repo.empty');
    repoList.replaceChildren(...entries.map((e) => repoItem(kind, e)));
  } catch (e) {
    if (prefs.repoKind !== kind) return;
    const reason = e instanceof TypeError ? t('repo.unreachable') : e.message;
    repoStatus.textContent = t('repo.unavailable', { url: indexUrl, reason });
  }
}
// Nouvelle langue : la liste ouverte est redessinée (depuis le cache, sans requête).
languageListeners.push(() => { if (prefs.open.includes('repo')) loadRepo(prefs.repoKind); });

async function fetchRepoFile(kind, entry) {
  const url = repoUrl(`${kind}/${entry.file.split('/').map(encodeURIComponent).join('/')}`);
  showStatus(t('downloading', { name: entry.file }));
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

function repoItem(kind, entry) {
  const name = entry.file.split('/').pop();
  const li = element('li', 'lib-item');
  const meta = element('div', 'lib-meta');
  const title = element('span', 'lib-name', localized(entry, 'title') ?? name);
  title.title = [localized(entry, 'title'), localized(entry, 'description'), localized(entry, 'license')]
    .filter(Boolean).join('\n\n') || name;
  meta.append(title, element('span', 'lib-sub', [name, entry.year, entry.authors].filter(Boolean).join(' · ')));
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
      showStatus(t('download.fail', { name, msg: e.message ?? e }), true);
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
    button('lib.boot', withFile((bytes) => {
      if (driveRows[0].insertAndBoot(name, bytes, null, entryInfo(entry, name))) showScreen();
    }), 'lib.boot.tip');
    const into = driveSelect();
    into.addEventListener('change', () => {
      const d = Number(into.value);
      into.value = '';
      if (d) withFile((bytes) => driveRows[d].insert(name, bytes))();
    });
    actions.append(into);
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
  li.append(icon(REPO_ICONS[kind]), meta, actions);
  return li;
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

// Ctrl+V sur l'écran : le texte du presse-papiers est tapé sur le TRS-80.
window.addEventListener('paste', (e) => {
  if (!emulator || typingInForm(e.target) || e.target instanceof HTMLTextAreaElement) return;
  e.preventDefault();
  typeOnTrs80(e.clipboardData.getData('text'));
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
  if (!emulator || e.ctrlKey || e.altKey || e.metaKey) return;
  if (e.target === touchInput) {
    // Clavier virtuel : le texte (et ← ou ENTRÉE) arrive par l'événement input; seules les
    // touches qui ne produisent pas de texte (flèches, Échap d'un clavier branché) passent ici.
    if (e.isComposing || e.key.length === 1 || ['Unidentified', 'Process', 'Backspace', 'Enter'].includes(e.key)) return;
  } else if (typingInForm(e.target)) {
    return;
  }
  if (e.repeat) { e.preventDefault(); return; }
  const key = pressKey(e.key);
  if (key) {
    pressed.set(e.code, key);
    e.preventDefault();
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
small.width = WIDTH;
small.height = HEIGHT;
const smallCtx = small.getContext('2d');
const image = smallCtx.createImageData(WIDTH, HEIGHT);
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

function screenChanged() {
  const video = new Uint8Array(wasm.memory.buffer, emulator.video_ptr(), 1024);
  const wide = emulator.wide();
  if (lastVideo && wide === lastWide && video.every((b, i) => b === lastVideo[i])) return false;
  lastVideo = video.slice();
  lastWide = wide;
  return true;
}

function draw() {
  const ptr = atlas ? emulator.render_graphics() : emulator.render();
  // Lecture directe de l'image dans la mémoire Wasm, sans copie intermédiaire.
  image.data.set(new Uint8ClampedArray(wasm.memory.buffer, ptr, WIDTH * HEIGHT * 4));
  smallCtx.putImageData(image, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(small, 0, 0, canvas.width, canvas.height);
  if (atlas) {
    const video = new Uint8Array(wasm.memory.buffer, emulator.video_ptr(), 1024);
    drawText(ctx, atlas, video, emulator.wide());
  }
}

function loop(now) {
  if (emulator) {
    // Rythme réel de 60 images/s, même si l'écran de l'hôte rafraîchit à 120 Hz ou plus.
    frameDebt += (now - lastTime) * 60 / 1000;
    frameDebt = Math.min(frameDebt, 5); // pas de rattrapage après une pause d'onglet
    const frames = Math.floor(frameDebt);
    if (frames > 0) {
      frameDebt -= frames;
      const emulated = frames * (turbo.checked ? 10 : emulator.typing() ? 4 : 1);
      emulator.run_frames(emulated);
      playAudio(emulated !== frames);
      driveNoise();
      frameCount += emulated;
      releaseDueKeys();
      if (screenChanged()) draw();
      fpsFrames += frames;
    }
    if (now - fpsTime >= 1000) {
      const mhz = fpsFrames * (turbo.checked ? 10 : 1) * Emulator.clock_hz() / 60 / 1e6;
      speedBox.textContent = `${mhz.toFixed(2)} MHz`;
      fpsFrames = 0;
      fpsTime = now;
    }
  }
  lastTime = now;
  requestAnimationFrame(loop);
}

await Promise.all([loadProgramIndex(), loadRomIndex(), loadDiskIndex(), refreshLibrary()]);
if (prefs.open.includes('repo')) loadRepo(prefs.repoKind); // section ouverte à la dernière visite
const params = new URLSearchParams(location.search);
// Police : ?font=<id>, sinon la dernière choisie.
let savedFont = null;
try { savedFont = localStorage.getItem('trs80-font'); } catch { /* stockage indisponible */ }
await selectFont(params.get('font') ?? savedFont ?? 'trs80');
// Lien direct vers une ROM de la liste : ?rom=level2-1.3 (a priorité sur la ROM conservée).
const romParam = params.get('rom');
if (romParam && roms.some((r) => r.id === romParam && r.source === 'url')) {
  selectRomInList(romParam);
  await chooseListedRom(romParam);
} else {
  const saved = (await loadDevRom()) ?? (await loadSavedRom());
  if (saved) {
    if (start(saved.bytes)) selectRomInList(saved.id);
  } else {
    // Première visite : démarrage avec la ROM Level II 1.3 officielle de la liste.
    selectRomInList(DEFAULT_ROM);
    await chooseListedRom(DEFAULT_ROM);
  }
}
if (emulator) {
  // Lien direct vers une disquette : ?disk=ldos-531
  const diskParam = params.get('disk');
  if (diskParam) {
    diskList.value = diskParam;
    await bootDisk(diskParam);
  }
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
requestAnimationFrame(loop);
