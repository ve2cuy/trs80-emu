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
const typeButton = document.getElementById('type-button');
const expansion = document.getElementById('expansion');
const soundBox = document.getElementById('sound');

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
  typeButton.disabled = !running;
  programsHint.hidden = running;
  diskList.disabled = !running;
  for (const row of driveRows) row.setEnabled(running);
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
  emulator.set_expansion_interface(expansion.checked);
  if (audioCtx && soundBox.checked) emulator.set_audio_rate(audioCtx.sampleRate);
  // Nouvelle ROM : les disquettes déjà insérées le restent (images d'origine).
  for (const row of driveRows) row.reinsert();
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

/** Charge un programme selon son extension : cassette .CAS ou exécutable .CMD. */
function runFile(bytes, name, label = name) {
  if (!emulator) return;
  try {
    if (/\.cas$/i.test(name)) {
      showStatus(`${label}: ${emulator.load_cas(bytes)}`);
    } else {
      const entry = emulator.load_cmd(bytes);
      showStatus(`${label} loaded, started at ${entry.toString(16).toUpperCase().padStart(4, '0')}h`);
    }
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
    runFile(new Uint8Array(await res.arrayBuffer()), p.file, p.title);
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
  runFile(new Uint8Array(await file.arrayBuffer()), file.name);
  event.target.value = '';
});

// ------------------------------------------------------------------ disquettes

const diskList = document.getElementById('disk-list');
const diskInfo = document.getElementById('disk-info');
let disks = [];

/** Une rangée de l'interface par lecteur : nom, Insert…, Eject, Save. */
function makeDriveRow(drive) {
  const row = document.createElement('div');
  row.className = 'drive';
  const name = document.createElement('span');
  name.className = 'drive-name';
  const insertLabel = document.createElement('label');
  insertLabel.className = 'button secondary';
  insertLabel.textContent = 'Insert…';
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.dsk,.dmk,.jv1,.jv3';
  input.hidden = true;
  insertLabel.append(input);
  const eject = document.createElement('button');
  eject.type = 'button';
  eject.className = 'secondary';
  eject.textContent = 'Eject';
  const save = document.createElement('button');
  save.type = 'button';
  save.className = 'secondary';
  save.textContent = 'Save';
  // Erreur d'insertion, affichée sur la ligne du lecteur pour ne pas passer inaperçue.
  const error = document.createElement('span');
  error.className = 'drive-error';
  error.setAttribute('role', 'alert');
  row.append(name, insertLabel, eject, save, error);
  document.getElementById('drives').append(row);

  let current = null; // { name, bytes } : image d'origine

  function refresh() {
    name.textContent = `Drive ${drive}: ${current ? current.name : '(empty)'}`;
    name.classList.toggle('modified', !!(current && emulator?.disk_modified(drive)));
    eject.disabled = !current || !emulator;
    save.disabled = !current || !emulator;
  }

  function insert(fileName, bytes) {
    if (!emulator) return false;
    try {
      const desc = emulator.insert_disk(drive, bytes);
      current = { name: fileName, bytes };
      error.textContent = '';
      showStatus(`Drive ${drive}: ${fileName} (${desc}).`);
      refresh();
      return true;
    } catch (e) {
      const message = `${fileName} not inserted: ${e.message ?? e}`;
      error.textContent = current ? `${message} (${current.name} is still in the drive)` : message;
      showStatus(`Cannot insert ${fileName}: ${e.message ?? e}`, true);
      return false;
    }
  }

  input.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    event.target.value = '';
    if (file) insert(file.name, new Uint8Array(await file.arrayBuffer()));
    canvas.focus();
  });
  eject.addEventListener('click', () => {
    emulator?.eject_disk(drive);
    current = null;
    error.textContent = '';
    refresh();
    canvas.focus();
  });
  save.addEventListener('click', () => {
    const image = emulator?.disk_image(drive);
    if (!image) {
      showStatus('Saving is only supported for JV1 and JV3 images.', true);
      return;
    }
    const link = document.createElement('a');
    link.href = URL.createObjectURL(new Blob([image], { type: 'application/octet-stream' }));
    link.download = current.name;
    link.click();
    URL.revokeObjectURL(link.href);
  });

  refresh();
  return {
    insert,
    refresh,
    setEnabled(on) {
      insertLabel.classList.toggle('disabled', !on);
      input.disabled = !on;
      refresh();
    },
    reinsert() {
      if (current) insert(current.name, current.bytes);
      refresh();
    },
  };
}

const driveRows = [0, 1, 2, 3].map(makeDriveRow);
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
  for (const d of disks) {
    const label = `${d.title}${d.year ? ` (${d.year})` : ''}${d.local ? ' — local' : ''}`;
    diskList.append(new Option(label, d.id));
  }
}

async function bootDisk(id) {
  const d = disks.find((x) => x.id === id);
  diskInfo.replaceChildren();
  diskInfo.hidden = !d;
  if (!d) return;
  for (const [tag, text] of [['strong', `${d.title} — ${d.authors ?? ''}`], ['span', d.description], ['span', d.license]]) {
    if (!text) continue;
    const el = document.createElement(tag);
    el.textContent = text;
    diskInfo.append(el);
  }
  try {
    const res = await fetch(`disks/${d.file}${V}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    if (driveRows[0].insert(d.file, new Uint8Array(await res.arrayBuffer()))) {
      emulator.reset();
      showStatus(`Booting ${d.title} from drive 0…`);
    }
  } catch (e) {
    showStatus(`Cannot download ${d.title}: ${e.message ?? e}`, true);
  }
  canvas.focus();
}

diskList.addEventListener('change', () => bootDisk(diskList.value));

// ------------------------------------------------------------------ frappe de texte

const typePanel = document.getElementById('type-panel');
const typeText = document.getElementById('type-text');

function typeOnTrs80(text) {
  if (!emulator || !text) return;
  const accepted = emulator.type_text(text);
  const skipped = [...text.replace(/\r/g, '')].length - accepted;
  showStatus(`Typing ${accepted} characters…` + (skipped > 0 ? ` (${skipped} without a TRS-80 key skipped)` : ''));
  canvas.focus();
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
document.getElementById('type-stop').addEventListener('click', () => {
  emulator?.cancel_typing();
  showStatus('Typing stopped.');
});

// Ctrl+V sur l'écran : le texte du presse-papiers est tapé sur le TRS-80.
window.addEventListener('paste', (e) => {
  if (!emulator || typingInForm(e.target) || e.target instanceof HTMLTextAreaElement) return;
  e.preventDefault();
  typeOnTrs80(e.clipboardData.getData('text'));
});

expansion.addEventListener('change', () => {
  emulator?.set_expansion_interface(expansion.checked);
  canvas.focus();
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

// ------------------------------------------------------------------ son

// Le son vient de la sortie cassette du TRS-80 (port FFh), échantillonnée par Rust.
// Les fureteurs n'autorisent l'audio qu'après une action de l'utilisateur : l'AudioContext
// est créé à la première touche ou au premier clic.
let audioCtx = null;
let nextAudioTime = 0;
const AUDIO_LATENCY = 0.06; // secondes d'avance pour éviter les coupures

function ensureAudio() {
  if (!soundBox.checked) return;
  if (!audioCtx) {
    try {
      audioCtx = new AudioContext();
    } catch {
      return; // pas de WebAudio : l'émulateur fonctionne sans son
    }
    emulator?.set_audio_rate(audioCtx.sampleRate);
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
  canvas.focus();
});

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
      const emulated = frames * (turbo.checked ? 10 : emulator.typing() ? 4 : 1);
      emulator.run_frames(emulated);
      playAudio(emulated !== frames);
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

await Promise.all([loadProgramIndex(), loadRomIndex(), loadDiskIndex()]);
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
