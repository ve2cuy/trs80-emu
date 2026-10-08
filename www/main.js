// Page de l'émulateur : charge le module WebAssembly, la ROM et les programmes, puis
// fait tourner la boucle d'affichage. Toute l'émulation est dans Rust (crates/web).
// Les textes de l'interface sont en anglais.

// Version de déploiement (?v=<commit> inscrit par GitHub Actions dans index.html) : ajoutée
// à chaque fichier chargé, pour qu'une mise à jour ne mélange jamais anciens et nouveaux
// fichiers gardés en cache. En développement (?v=dev), on ne met rien en cache.
const BUILD = new URL(import.meta.url).searchParams.get('v') ?? 'dev';
const V = `?v=${BUILD === 'dev' ? Date.now() : BUILD}`;

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
const programInfo = document.getElementById('program-info');
const cmdFile = document.getElementById('cmd-file');
const cmdButton = document.getElementById('cmd-button');
const programsHint = document.getElementById('programs-hint');

// Toute erreur imprévue est affichée sous l'écran plutôt que de figer la page en silence.
function showStatus(message, isError = false) {
  statusBox.textContent = message;
  statusBox.classList.toggle('error', isError);
}
window.addEventListener('error', (e) => showStatus(`Error: ${e.message}`, true));
window.addEventListener('unhandledrejection', (e) =>
  showStatus(`Error: ${e.reason?.message ?? e.reason}`, true));

const wasm = await init({ module_or_path: `pkg/trs80_web_bg.wasm${V}` });
const WIDTH = Emulator.width();
const HEIGHT = Emulator.height();

let emulator = null;

// ------------------------------------------------------------------ ROM

// La ROM est conservée dans IndexedDB pour les visites suivantes.
const DB_NAME = 'trs80-emu';

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore('roms');
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

// Valeur conservée : { id, bytes } (id = entrée de roms.json, ou 'custom' pour un fichier
// de l'utilisateur). Les versions précédentes conservaient seulement les octets.
async function saveRom(id, bytes) {
  try {
    const db = await openDb();
    db.transaction('roms', 'readwrite').objectStore('roms').put({ id, bytes }, 'level2');
  } catch { /* stockage indisponible (navigation privée) : on s'en passe */ }
}

async function loadSavedRom() {
  try {
    const db = await openDb();
    const value = await new Promise((resolve) => {
      const req = db.transaction('roms').objectStore('roms').get('level2');
      req.onsuccess = () => resolve(req.result ?? null);
      req.onerror = () => resolve(null);
    });
    if (value instanceof Uint8Array) return { id: 'custom', bytes: value };
    return value;
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
    romList.append(new Option(r.title, r.id));
  }
}

/** Affiche la ROM courante dans la liste; un fichier personnel y apparaît comme « Your ROM file ». */
function selectRomInList(id) {
  if (id === 'custom' && !romList.querySelector('option[value="custom"]')) {
    romList.append(new Option('Your ROM file', 'custom'));
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
    showStatus(`${rom.title}: open the release archive to continue.`);
    return;
  }
  showStatus(`Downloading ${rom.title}…`);
  try {
    const res = await fetch(rom.url);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    if (rom.sha256 && (await sha256Hex(bytes)) !== rom.sha256) {
      throw new Error('the downloaded file does not match the expected checksum');
    }
    if (start(bytes)) {
      saveRom(rom.id, bytes);
      showStatus(`${rom.title} loaded.`);
    }
  } catch (e) {
    showStatus(`Cannot download ${rom.title}: ${e.message ?? e}`, true);
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
    showStatus(`${rom.member} was not found in ${file.name}.`, true);
    return;
  }
  if (start(bytes)) {
    romTar.hidden = true;
    saveRom(rom.id, bytes);
    showStatus(`${rom.title} loaded from ${file.name}.`);
  }
});

function setRunning(running) {
  overlay.hidden = running;
  resetButton.disabled = !running;
  programList.disabled = !running;
  cmdFile.disabled = !running;
  cmdButton.classList.toggle('disabled', !running);
  programsHint.hidden = running;
}

function start(bytes) {
  try {
    emulator?.free();
    emulator = new Emulator(bytes);
  } catch (e) {
    emulator = null;
    errorBox.textContent = `ROM rejected: ${e.message ?? e}`;
    setRunning(false);
    return false;
  }
  errorBox.textContent = '';
  setRunning(true);
  canvas.focus();
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
    showStatus(`${file.name} loaded.`);
  }
});
resetButton.addEventListener('click', () => { emulator?.reset(); canvas.focus(); });

// ------------------------------------------------------------------ programmes

let programs = [];

async function loadProgramIndex() {
  try {
    const res = await fetch(`programs/index.json${V}`);
    programs = res.ok ? await res.json() : [];
  } catch {
    programs = [];
  }
  for (const p of programs) {
    const option = document.createElement('option');
    option.value = p.id;
    option.textContent = `${p.title} (${p.year})`;
    programList.append(option);
  }
}

function runCmd(bytes, label) {
  if (!emulator) return;
  try {
    const entry = emulator.load_cmd(bytes);
    showStatus(`${label} loaded, started at ${entry.toString(16).toUpperCase().padStart(4, '0')}h`);
  } catch (e) {
    showStatus(`Cannot run ${label}: ${e.message ?? e}`, true);
  }
  canvas.focus();
}

function showProgramInfo(p) {
  programInfo.replaceChildren();
  if (!p) {
    programInfo.hidden = true;
    return;
  }
  const lines = [
    ['strong', `${p.title} (${p.year}) — ${p.authors}`],
    ['span', p.description],
    ['span', `Controls: ${p.controls}`],
    ['span', p.license],
  ];
  for (const [tag, text] of lines) {
    const el = document.createElement(tag);
    el.textContent = text;
    programInfo.append(el);
  }
  programInfo.hidden = false;
}

async function runProgram(id) {
  const p = programs.find((x) => x.id === id);
  showProgramInfo(p);
  if (!p) return;
  try {
    const res = await fetch(`programs/${p.file}${V}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    runCmd(new Uint8Array(await res.arrayBuffer()), p.title);
  } catch (e) {
    showStatus(`Cannot download ${p.title}: ${e.message ?? e}`, true);
  }
}

programList.addEventListener('change', () => runProgram(programList.value));

cmdFile.addEventListener('change', async (event) => {
  const file = event.target.files[0];
  if (!file) return;
  programList.value = '';
  showProgramInfo(null);
  runCmd(new Uint8Array(await file.arrayBuffer()), file.name);
  event.target.value = '';
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
  return target instanceof HTMLInputElement || target instanceof HTMLSelectElement;
}

window.addEventListener('keydown', (e) => {
  if (!emulator || e.ctrlKey || e.altKey || e.metaKey || typingInForm(e.target)) return;
  if (e.repeat) { e.preventDefault(); return; }
  if (emulator.key_down(e.key)) {
    pendingReleases = pendingReleases.filter((r) => r.name !== e.key);
    pressed.set(e.code, { name: e.key, at: frameCount });
    e.preventDefault();
  }
});

window.addEventListener('keyup', (e) => {
  if (!emulator) return;
  const key = pressed.get(e.code) ?? { name: e.key, at: -Infinity };
  pressed.delete(e.code);
  if (frameCount - key.at >= MIN_HOLD_FRAMES) {
    emulator.key_up(key.name);
  } else {
    pendingReleases.push(key);
  }
});

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

for (const group of [...new Set(FONTS.map((f) => f.group))]) {
  const optgroup = document.createElement('optgroup');
  optgroup.label = group;
  for (const f of FONTS.filter((x) => x.group === group)) optgroup.append(new Option(f.label, f.id));
  fontList.append(optgroup);
}

async function selectFont(id) {
  const chosen = FONTS.find((f) => f.id === id) ?? FONTS[0];
  try {
    atlas = chosen.family ? await buildAtlas(chosen) : null;
    font = chosen;
    try { localStorage.setItem('trs80-font', chosen.id); } catch { /* stockage indisponible */ }
  } catch (e) {
    showStatus(`Font ${chosen.label}: ${e.message ?? e}`, true);
  }
  fontList.value = font.id;
  lastVideo = null; // force le redessin
  if (emulator) draw();
}

fontList.addEventListener('change', () => { selectFont(fontList.value); canvas.focus(); });

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
      const emulated = frames * (turbo.checked ? 10 : 1);
      emulator.run_frames(emulated);
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

await Promise.all([loadProgramIndex(), loadRomIndex()]);
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
