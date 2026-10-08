// Page de l'émulateur : charge le module WebAssembly, la ROM et les programmes, puis
// fait tourner la boucle d'affichage. Toute l'émulation est dans Rust (crates/web).
// Les textes de l'interface sont en anglais.

// Version de déploiement (?v=<commit> inscrit par GitHub Actions dans index.html) : ajoutée
// à chaque fichier chargé, pour qu'une mise à jour ne mélange jamais anciens et nouveaux
// fichiers gardés en cache. En développement (?v=dev), on ne met rien en cache.
const BUILD = new URL(import.meta.url).searchParams.get('v') ?? 'dev';
const V = `?v=${BUILD === 'dev' ? Date.now() : BUILD}`;

const { default: init, Emulator } = await import(`./pkg/trs80_web.js${V}`);

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

async function saveRom(bytes) {
  try {
    const db = await openDb();
    db.transaction('roms', 'readwrite').objectStore('roms').put(bytes, 'level2');
  } catch { /* stockage indisponible (navigation privée) : on s'en passe */ }
}

async function loadSavedRom() {
  try {
    const db = await openDb();
    return await new Promise((resolve) => {
      const req = db.transaction('roms').objectStore('roms').get('level2');
      req.onsuccess = () => resolve(req.result ?? null);
      req.onerror = () => resolve(null);
    });
  } catch {
    return null;
  }
}

// En développement : une ROM placée dans www/rom/level2.rom (exclue de Git) est chargée d'office.
async function loadDevRom() {
  try {
    const res = await fetch('rom/level2.rom');
    return res.ok ? new Uint8Array(await res.arrayBuffer()) : null;
  } catch {
    return null;
  }
}

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

async function onRomFile(event) {
  const file = event.target.files[0];
  if (!file) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (start(bytes)) saveRom(bytes);
  event.target.value = '';
}

document.getElementById('rom-file').addEventListener('change', onRomFile);
document.getElementById('rom-change').addEventListener('change', onRomFile);
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
const pressed = new Map();

function typingInForm(target) {
  return target instanceof HTMLInputElement || target instanceof HTMLSelectElement;
}

window.addEventListener('keydown', (e) => {
  if (!emulator || e.ctrlKey || e.altKey || e.metaKey || typingInForm(e.target)) return;
  if (e.repeat) { e.preventDefault(); return; }
  if (emulator.key_down(e.key)) {
    pressed.set(e.code, e.key);
    e.preventDefault();
  }
});

window.addEventListener('keyup', (e) => {
  if (!emulator) return;
  const name = pressed.get(e.code) ?? e.key;
  pressed.delete(e.code);
  emulator.key_up(name);
});

window.addEventListener('blur', () => {
  pressed.clear();
  emulator?.release_all_keys();
});

// ------------------------------------------------------------------ boucle

const image = ctx.createImageData(WIDTH, HEIGHT);
let lastTime = performance.now();
let frameDebt = 0;
let fpsFrames = 0;
let fpsTime = lastTime;

function draw() {
  const ptr = emulator.render();
  // Lecture directe de l'image dans la mémoire Wasm, sans copie intermédiaire.
  image.data.set(new Uint8ClampedArray(wasm.memory.buffer, ptr, WIDTH * HEIGHT * 4));
  ctx.putImageData(image, 0, 0);
}

function loop(now) {
  if (emulator) {
    // Rythme réel de 60 images/s, même si l'écran de l'hôte rafraîchit à 120 Hz ou plus.
    frameDebt += (now - lastTime) * 60 / 1000;
    frameDebt = Math.min(frameDebt, 5); // pas de rattrapage après une pause d'onglet
    const frames = Math.floor(frameDebt);
    if (frames > 0) {
      frameDebt -= frames;
      emulator.run_frames(frames * (turbo.checked ? 10 : 1));
      draw();
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

await loadProgramIndex();
const saved = (await loadDevRom()) ?? (await loadSavedRom());
if (saved && start(saved)) {
  const params = new URLSearchParams(location.search);
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
