// Page de l'émulateur : charge le module WebAssembly, la ROM, puis fait tourner
// la boucle d'affichage. Toute l'émulation est dans Rust (crates/web).

import init, { Emulator } from './pkg/trs80_web.js';

const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const overlay = document.getElementById('overlay');
const errorBox = document.getElementById('error');
const statusBox = document.getElementById('status');
const resetButton = document.getElementById('reset');
const turbo = document.getElementById('turbo');

// Toute erreur imprévue est affichée sous l'écran plutôt que de figer la page en silence.
function showError(message) {
  statusBox.textContent = `Erreur : ${message}`;
  statusBox.classList.add('error');
}
window.addEventListener('error', (e) => showError(e.message));
window.addEventListener('unhandledrejection', (e) => showError(e.reason?.message ?? e.reason));

const wasm = await init();
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

function start(bytes) {
  try {
    emulator?.free();
    emulator = new Emulator(bytes);
  } catch (e) {
    emulator = null;
    errorBox.textContent = `ROM refusée : ${e.message ?? e}`;
    overlay.hidden = false;
    return false;
  }
  errorBox.textContent = '';
  overlay.hidden = true;
  resetButton.disabled = false;
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

// ------------------------------------------------------------------ clavier

// La touche relâchée peut avoir un autre nom que la touche enfoncée (ex. : « a » puis « A »
// si MAJ a été enfoncée entre-temps) : on mémorise le nom par touche physique.
const pressed = new Map();

window.addEventListener('keydown', (e) => {
  if (!emulator || e.ctrlKey || e.altKey || e.metaKey) return;
  if (e.target instanceof HTMLInputElement) return;
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
      const mhz = (fpsFrames * (turbo.checked ? 10 : 1) * Emulator.clock_hz() / 60 / 1e6).toFixed(2);
      statusBox.textContent = `${mhz} MHz émulés`;
      fpsFrames = 0;
      fpsTime = now;
    }
  }
  lastTime = now;
  requestAnimationFrame(loop);
}

const saved = (await loadDevRom()) ?? (await loadSavedRom());
if (saved && start(saved)) {
  // Développement : ?frames=N exécute N images dès le chargement (captures d'écran, tests).
  const warmup = Number(new URLSearchParams(location.search).get('frames') ?? 0);
  if (warmup > 0) {
    emulator.run_frames(warmup);
    draw();
  }
}
requestAnimationFrame(loop);
