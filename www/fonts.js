// Polices de l'écran.
//
// - « TRS-80 (original pixels) » : la police 5 × 7 dessinée par Rust, pixel par pixel.
// - Les autres : polices à chasse fixe (monospace) dessinées par la page. L'écran du TRS-80
//   est une grille de 64 × 16 cases étroites : chaque police est condensée horizontalement
//   d'un même facteur pour toutes ses lettres, calculé sur sa largeur de caractère.
//
// Chaque police est rendue une seule fois dans un « atlas » (une image de 64 glyphes, codes
// 20h-5Fh); l'affichage copie ensuite le glyphe voulu dans chaque case.

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

// Taille d'une case dans le canvas (1536 × 1152 = 64 × 16 cases, affiché en 4:3).
export const CELL_W = 24;
export const CELL_H = 72;
// Ligne de base et taille du texte : comme le glyphe 5 × 7 d'origine (lignes 2 à 8 sur 12).
const BASELINE = 56;
const FONT_SIZE = 60;
// Facteur de condensation maximal (la case fait 24 × 72 pixels).
const MAX_SCALE_X = 0.62;
const TEXT_COLOR = '#e6eeff';

// Comme sur le Model I, 5Bh-5Eh sont des flèches.
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

/** Code ASCII réellement affiché (20h-5Fh) pour un octet de texte de la mémoire vidéo. */
export function displayAscii(code) {
  const c = code & 0x7f;
  if (c < 0x20) return c + 0x40;
  if (c >= 0x60) return c - 0x20;
  return c;
}

/** Rend les 64 glyphes d'une police dans un atlas (une rangée de cases). */
export async function buildAtlas(font) {
  const css = `${font.weight} ${FONT_SIZE}px "${font.family}"`;
  if (font.google) await loadGoogleStylesheet(font);
  await document.fonts.load(css, 'ABCabc0123');

  const atlas = document.createElement('canvas');
  atlas.width = 64 * CELL_W;
  atlas.height = CELL_H;
  const g = atlas.getContext('2d');
  g.font = `${css}, sans-serif`;
  g.fillStyle = TEXT_COLOR;
  g.textAlign = 'center';
  g.textBaseline = 'alphabetic';
  // Chasse fixe : un seul facteur pour toute la police, d'après la largeur de « M ».
  const scaleX = Math.min(MAX_SCALE_X, (CELL_W - 2) / (g.measureText('M').width || 1));
  for (let i = 0; i < 64; i++) {
    const code = 0x20 + i;
    const ch = SPECIAL[code] ?? String.fromCharCode(code);
    g.save();
    g.translate(i * CELL_W + CELL_W / 2, BASELINE);
    g.scale(scaleX, 1);
    g.fillText(ch, 0, 0);
    g.restore();
  }
  return atlas;
}

/** Dessine le texte de la mémoire vidéo par-dessus l'image des blocs graphiques. */
export function drawText(ctx, atlas, video, wide) {
  ctx.imageSmoothingEnabled = true;
  const step = wide ? 2 : 1;
  for (let row = 0; row < 16; row++) {
    for (let col = 0; col < 64; col += step) {
      const code = video[row * 64 + col];
      if (code & 0x80) continue; // bloc semi-graphique, déjà dessiné
      const ascii = displayAscii(code);
      if (ascii === 0x20) continue;
      ctx.drawImage(atlas, (ascii - 0x20) * CELL_W, 0, CELL_W, CELL_H,
        col * CELL_W, row * CELL_H, CELL_W * step, CELL_H);
    }
  }
}
