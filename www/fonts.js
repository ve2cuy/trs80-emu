// Polices de l'écran.
//
// - « TRS-80 (original pixels) » : la police 5 × 7 dessinée par Rust, pixel par pixel.
// - Les autres : polices à chasse fixe (monospace) dessinées par la page. L'écran du TRS-80
//   est une grille de 64 × 16 (ou 80 × 24) cases étroites : chaque police est condensée
//   horizontalement d'un même facteur pour toutes ses lettres, calculé sur sa largeur de
//   caractère.
//
// Les glyphes d'une police sont rendus une seule fois par disposition dans une « feuille »
// (96 glyphes, codes 20h-7Fh); l'affichage copie ensuite le glyphe voulu dans chaque case.

export const FONTS = [
  { id: 'trs80', label: 'TRS-80 (original pixels)', group: 'Original' },

  { id: 'vt323', label: 'VT323 (DEC terminal)', family: 'VT323', weight: 400, google: true, group: 'Retro' },
  { id: 'press-start', label: 'Press Start 2P (arcade)', family: 'Press Start 2P', weight: 400, google: true, group: 'Retro' },
  { id: 'share-tech-mono', label: 'Share Tech Mono', family: 'Share Tech Mono', weight: 400, google: true, group: 'Retro' },
  { id: 'major-mono', label: 'Major Mono Display', family: 'Major Mono Display', weight: 400, google: true, group: 'Retro' },

  { id: 'ibm-plex-mono', label: 'IBM Plex Mono', family: 'IBM Plex Mono', weight: 500, google: true, group: 'Modern' },
  { id: 'jetbrains-mono', label: 'JetBrains Mono', family: 'JetBrains Mono', weight: 500, google: true, group: 'Modern' },
  { id: 'fira-mono', label: 'Fira Mono', family: 'Fira Mono', weight: 500, google: true, group: 'Modern' },
  { id: 'source-code-pro', label: 'Source Code Pro', family: 'Source Code Pro', weight: 500, google: true, group: 'Modern' },
  { id: 'roboto-mono', label: 'Roboto Mono', family: 'Roboto Mono', weight: 500, google: true, group: 'Modern' },
  { id: 'space-mono', label: 'Space Mono', family: 'Space Mono', weight: 400, google: true, group: 'Modern' },
  { id: 'inconsolata', label: 'Inconsolata', family: 'Inconsolata', weight: 600, google: true, group: 'Modern' },
  { id: 'courier-prime', label: 'Courier Prime (typewriter)', family: 'Courier Prime', weight: 400, google: true, group: 'Modern' },

  { id: 'consolas', label: 'Consolas (Windows)', family: 'Consolas', weight: 400, group: 'System' },
  { id: 'lucida-console', label: 'Lucida Console (Windows)', family: 'Lucida Console', weight: 400, group: 'System' },
  { id: 'courier', label: 'Courier New', family: 'Courier New', weight: 400, group: 'System' },
];

// Disposition des glyphes selon l'écran. Le canvas fait 1536 × 1152 : en 64 × 16, une case
// de 24 × 72; en 80 × 24, de 19,2 × 48, où l'on copie des glyphes de 24 × 60 réduits à 0,8
// (mêmes proportions). Ligne de base et taille du texte : comme le glyphe 5 × 7 d'origine.
const LAYOUTS = {
  64: { cellW: 24, cellH: 72, baseline: 56, size: 60 },
  80: { cellW: 24, cellH: 60, baseline: 47, size: 50 },
};
// Facteur de condensation maximal.
const MAX_SCALE_X = 0.62;
const TEXT_COLOR = '#e6eeff';
// Fond de l'écran (caractères en vidéo inversée).
const BACK_COLOR = '#08080a';

// Comme sur les Model I, III et 4, 5Bh-5Eh sont des flèches (le Model II, en ASCII, montre
// [ \ ] ^).
const SPECIAL = { 0x5b: '↑', 0x5c: '↓', 0x5d: '←', 0x5e: '→' };

const loadedStylesheets = new Map();

/** Charge la feuille de style Google Fonts d'une police (une seule fois). */
function loadGoogleStylesheet(font) {
  if (!loadedStylesheets.has(font.id)) {
    const family = font.family.replace(/ /g, '+');
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = `https://fonts.googleapis.com/css2?family=${family}:wght@${font.weight}&display=block`;
    loadedStylesheets.set(font.id, new Promise((resolve, reject) => {
      link.onload = resolve;
      link.onerror = () => reject(new Error(`cannot load ${font.family} from Google Fonts`));
    }));
    document.head.append(link);
  }
  return loadedStylesheets.get(font.id);
}

/**
 * Prépare une police : la charge, puis rend ses glyphes à la demande dans des « feuilles »
 * (une rangée de 96 cases, codes 20h-7Fh), une par disposition, jeu de caractères et couleur.
 */
export async function buildAtlas(font) {
  const sizes = [...new Set(Object.values(LAYOUTS).map((l) => l.size))];
  if (font.google) await loadGoogleStylesheet(font);
  for (const size of sizes) await document.fonts.load(`${font.weight} ${size}px "${font.family}"`, 'ABCabc0123');
  return { font, sheets: new Map() };
}

function sheet(atlas, cols, arrows, color) {
  const key = `${cols}/${arrows}/${color}`;
  if (atlas.sheets.has(key)) return atlas.sheets.get(key);
  const l = LAYOUTS[cols];
  const canvas = document.createElement('canvas');
  canvas.width = 96 * l.cellW;
  canvas.height = l.cellH;
  const g = canvas.getContext('2d');
  g.font = `${atlas.font.weight} ${l.size}px "${atlas.font.family}", sans-serif`;
  g.fillStyle = color;
  g.textAlign = 'center';
  g.textBaseline = 'alphabetic';
  // Chasse fixe : un seul facteur pour toute la police, d'après la largeur de « M ».
  const scaleX = Math.min(MAX_SCALE_X, (l.cellW - 2) / (g.measureText('M').width || 1));
  for (let i = 0; i < 95; i++) {
    const code = 0x20 + i;
    const ch = (arrows && SPECIAL[code]) || String.fromCharCode(code);
    g.save();
    g.translate(i * l.cellW + l.cellW / 2, l.baseline);
    g.scale(scaleX, 1);
    g.fillText(ch, 0, 0);
    g.restore();
  }
  atlas.sheets.set(key, canvas);
  return canvas;
}

/**
 * Dessine le texte de la mémoire vidéo par-dessus l'image des blocs graphiques (et des fonds
 * inversés). `screen` : { cols, rows, wide (32 caractères par ligne), lowercase, inverse
 * (bit 7 : caractère inversé, sinon bloc graphique), triangles (Model II : 00h-03h), arrows }.
 */
export function drawText(ctx, atlas, video, screen) {
  const { cols, rows, wide, lowercase, inverse, triangles, arrows } = screen;
  const l = LAYOUTS[cols];
  if (!l) return;
  const fg = sheet(atlas, cols, arrows, TEXT_COLOR);
  const bg = inverse ? sheet(atlas, cols, arrows, BACK_COLOR) : null;
  const dw = ctx.canvas.width / cols;
  const dh = ctx.canvas.height / rows;
  ctx.imageSmoothingEnabled = true;
  const step = wide ? 2 : 1;
  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col += step) {
      const code = video[row * cols + col];
      const inverted = (code & 0x80) !== 0;
      if (inverted && !inverse) continue; // bloc semi-graphique, déjà dessiné
      if (triangles && (code & 0x7c) === 0) continue; // triangle, déjà dessiné
      let c = code & 0x7f;
      if (c < 0x20) c += 0x40;
      else if (c >= 0x60 && !lowercase) c -= 0x20;
      if (c === 0x20 || c === 0x7f) continue;
      ctx.drawImage(inverted ? bg : fg, (c - 0x20) * l.cellW, 0, l.cellW, l.cellH,
        col * dw, row * dh, dw * step, dh);
    }
  }
}
