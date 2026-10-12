// Écran couleur ANSI du modem : ce que le BBS envoie, avant tout filtrage, interprété comme
// par un terminal ANSI de PC (80 × 25, jeu de caractères CP437, 16 couleurs). Pendant la
// connexion, il remplace l'écran du TRS-80 (main.js recopie son canvas, 720 × 400 comme le
// mode texte VGA); on tape toujours sur le TRS-80 (LCOMM, COMM, OMNITERM...).
//
// Séquences comprises : CSI n A/B/C/D/E/F/G (curseur), H et f (position), J et K
// (effacement), m (couleurs : 0, 1, 2, 5, 7, 22, 25, 27, 30-37, 39, 40-47, 49, 90-97,
// 100-107), s/u et ESC 7/8 (sauver, rétablir le curseur), S/T (défilement), L/M (lignes),
// P/@ (caractères), n (6n : position du curseur, réponse au BBS), h/l (ignorés).

/** CP437 : 80h-FFh en Unicode (cadres, blocs, lettres accentuées). */
const CP437_HIGH =
  'ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»' +
  '░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀' +
  'αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■ ';
/** 00h-1Fh affichables (visages, flèches...) quand ils ne sont pas des commandes. */
const CP437_LOW = ' ☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼';

/** Les 16 couleurs d'un PC (VGA). */
const PALETTE = [
  '#000000', '#aa0000', '#00aa00', '#aa5500', '#0000aa', '#aa00aa', '#00aaaa', '#aaaaaa',
  '#555555', '#ff5555', '#55ff55', '#ffff55', '#5555ff', '#ff55ff', '#55ffff', '#ffffff',
];
/** Ordre ANSI (30-37) : noir, rouge, vert, jaune, bleu, magenta, cyan, blanc. */
const ANSI_TO_VGA = [0, 1, 2, 3, 4, 5, 6, 7];

export class AnsiScreen {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {(bytes: Uint8Array) => void} reply  réponses au BBS (position du curseur)
   */
  constructor(canvas, reply) {
    this.canvas = canvas;
    this.reply = reply;
    this.cols = 80;
    this.rows = 25;
    this.cellW = 9;
    this.cellH = 16;
    canvas.width = this.cols * this.cellW;
    canvas.height = this.rows * this.cellH;
    this.ctx = canvas.getContext('2d');
    this.reset();
  }

  reset() {
    this.cells = Array.from({ length: this.rows }, () => this.blankRow());
    this.x = 0;
    this.y = 0;
    this.saved = [0, 0];
    this.fg = 7;
    this.bg = 0;
    this.bold = false;
    this.blink = false;
    this.reverse = false;
    this.state = 0; // 0 texte, 1 après ESC, 2 dans ESC [
    this.params = '';
    this.dirty = true;
    this.draw();
  }

  blankRow() {
    return Array.from({ length: this.cols }, () => ({ ch: ' ', fg: 7, bg: 0 }));
  }

  /** Attributs du caractère suivant. */
  attrs() {
    let fg = this.fg + (this.bold ? 8 : 0);
    let bg = this.bg + (this.blink ? 8 : 0); // clignotement : fond clair (iCE colors)
    if (this.reverse) [fg, bg] = [bg, fg];
    return { fg, bg };
  }

  /** Octets reçus du BBS. */
  write(bytes) {
    for (const b of bytes) this.byte(b);
    this.dirty = true;
  }

  byte(b) {
    if (this.state === 1) {
      if (b === 0x5B) {
        this.state = 2;
        this.params = '';
      } else {
        this.state = 0;
        if (b === 0x37) this.saved = [this.x, this.y]; // ESC 7
        else if (b === 0x38) [this.x, this.y] = this.saved; // ESC 8
        else if (b === 0x63) this.reset(); // ESC c
      }
      return;
    }
    if (this.state === 2) {
      if (b >= 0x40 && b <= 0x7E) {
        this.state = 0;
        this.csi(String.fromCharCode(b), this.params);
      } else {
        this.params += String.fromCharCode(b);
        if (this.params.length > 32) this.state = 0;
      }
      return;
    }
    switch (b) {
      case 0x1B: this.state = 1; return;
      case 0x0D: this.x = 0; return;
      case 0x0A: this.lineFeed(); return;
      case 0x08: this.x = Math.max(0, this.x - 1); return;
      case 0x09: this.x = Math.min(this.cols - 1, (this.x & ~7) + 8); return;
      case 0x0C: this.clear(2); this.x = this.y = 0; return; // saut de page : écran effacé
      case 0x07: case 0x00: return;
      default:
        this.put(b < 0x20 ? CP437_LOW[b] : b < 0x7F ? String.fromCharCode(b) : b === 0x7F ? '⌂' : CP437_HIGH[b - 0x80]);
    }
  }

  put(ch) {
    if (this.x >= this.cols) {
      this.x = 0;
      this.lineFeed();
    }
    this.cells[this.y][this.x] = { ch, ...this.attrs() };
    this.x++;
  }

  lineFeed() {
    if (this.y === this.rows - 1) this.scroll(1);
    else this.y++;
  }

  scroll(n) {
    for (let i = 0; i < n; i++) {
      this.cells.shift();
      this.cells.push(this.blankRow());
    }
  }

  /** Efface : 0 du curseur à la fin, 1 du début au curseur, 2 tout. */
  clear(mode) {
    const blank = { ch: ' ', fg: 7, bg: this.attrs().bg };
    for (let y = 0; y < this.rows; y++) {
      for (let x = 0; x < this.cols; x++) {
        const after = y > this.y || (y === this.y && x >= this.x);
        if (mode === 2 || (mode === 0 && after) || (mode === 1 && !after)) this.cells[y][x] = { ...blank };
      }
    }
  }

  clearLine(mode) {
    const blank = { ch: ' ', fg: 7, bg: this.attrs().bg };
    for (let x = 0; x < this.cols; x++) {
      if (mode === 2 || (mode === 0 && x >= this.x) || (mode === 1 && x <= this.x)) this.cells[this.y][x] = { ...blank };
    }
  }

  csi(cmd, text) {
    const priv = text.startsWith('?');
    const p = (priv ? text.slice(1) : text).split(';').map((v) => (v === '' ? null : Number(v)));
    const n = (i, d = 1) => (p[i] == null || Number.isNaN(p[i]) ? d : p[i]);
    const clampX = (x) => Math.max(0, Math.min(this.cols - 1, x));
    const clampY = (y) => Math.max(0, Math.min(this.rows - 1, y));
    switch (cmd) {
      case 'A': this.y = clampY(this.y - n(0)); break;
      case 'B': this.y = clampY(this.y + n(0)); break;
      case 'C': this.x = clampX(this.x + n(0)); break;
      case 'D': this.x = clampX(this.x - n(0)); break;
      case 'E': this.y = clampY(this.y + n(0)); this.x = 0; break;
      case 'F': this.y = clampY(this.y - n(0)); this.x = 0; break;
      case 'G': this.x = clampX(n(0) - 1); break;
      case 'H': case 'f': this.y = clampY(n(0) - 1); this.x = clampX(n(1) - 1); break;
      case 'J': this.clear(n(0, 0)); if (n(0, 0) === 2) this.x = this.y = 0; break;
      case 'K': this.clearLine(n(0, 0)); break;
      case 'S': this.scroll(n(0)); break;
      case 'T':
        for (let i = 0; i < n(0); i++) {
          this.cells.pop();
          this.cells.unshift(this.blankRow());
        }
        break;
      case 'L':
        for (let i = 0; i < n(0); i++) {
          this.cells.splice(this.y, 0, this.blankRow());
          this.cells.pop();
        }
        break;
      case 'M':
        for (let i = 0; i < n(0); i++) {
          this.cells.splice(this.y, 1);
          this.cells.push(this.blankRow());
        }
        break;
      case 'P': this.cells[this.y].splice(this.x, n(0)); while (this.cells[this.y].length < this.cols) this.cells[this.y].push({ ch: ' ', fg: 7, bg: 0 }); break;
      case '@':
        for (let i = 0; i < n(0); i++) this.cells[this.y].splice(this.x, 0, { ch: ' ', fg: 7, bg: 0 });
        this.cells[this.y].length = this.cols;
        break;
      case 's': this.saved = [this.x, this.y]; break;
      case 'u': [this.x, this.y] = this.saved; break;
      case 'n':
        // 6n : le BBS demande la position du curseur (détection d'un terminal ANSI).
        if (n(0, 0) === 6) this.reply(new TextEncoder().encode(`\x1b[${this.y + 1};${this.x + 1}R`));
        break;
      case 'm': this.sgr(p.length ? p : [0]); break;
      default: break; // h, l (modes), r (zone de défilement)... ignorés
    }
  }

  sgr(codes) {
    for (const c of codes.map((v) => v ?? 0)) {
      if (c === 0) {
        this.fg = 7; this.bg = 0; this.bold = this.blink = this.reverse = false;
      } else if (c === 1) this.bold = true;
      else if (c === 2 || c === 22) this.bold = false;
      else if (c === 5 || c === 6) this.blink = true;
      else if (c === 25) this.blink = false;
      else if (c === 7) this.reverse = true;
      else if (c === 27) this.reverse = false;
      else if (c >= 30 && c <= 37) this.fg = ANSI_TO_VGA[c - 30];
      else if (c === 39) this.fg = 7;
      else if (c >= 40 && c <= 47) this.bg = ANSI_TO_VGA[c - 40];
      else if (c === 49) this.bg = 0;
      else if (c >= 90 && c <= 97) { this.fg = ANSI_TO_VGA[c - 90]; this.bold = true; }
      else if (c >= 100 && c <= 107) { this.bg = ANSI_TO_VGA[c - 100]; this.blink = true; }
    }
  }

  /** Redessine si quelque chose a changé (appelé à chaque image). */
  draw(cursorOn = true) {
    if (!this.dirty) return;
    this.dirty = false;
    const { ctx, cellW: w, cellH: h } = this;
    ctx.font = `${h - 2}px "Cascadia Mono", Consolas, "DejaVu Sans Mono", monospace`;
    ctx.textBaseline = 'middle';
    ctx.textAlign = 'center';
    for (let y = 0; y < this.rows; y++) {
      for (let x = 0; x < this.cols; x++) {
        const c = this.cells[y][x];
        ctx.fillStyle = PALETTE[c.bg];
        ctx.fillRect(x * w, y * h, w, h);
        if (c.ch !== ' ' && c.ch !== ' ') {
          // Blocs pleins et demi-blocs : dessinés (une police les laisse souvent avec des
          // jours entre les cellules).
          if (!this.block(c, x * w, y * h)) {
            ctx.fillStyle = PALETTE[c.fg];
            ctx.fillText(c.ch, x * w + w / 2, y * h + h / 2 + 1);
          }
        }
      }
    }
    if (cursorOn) {
      ctx.fillStyle = PALETTE[7];
      ctx.fillRect(this.x * w, this.y * h + h - 3, w, 2);
    }
  }

  block(c, x, y) {
    const { ctx, cellW: w, cellH: h } = this;
    const rect = { '█': [0, 0, w, h], '▀': [0, 0, w, h / 2], '▄': [0, h / 2, w, h / 2], '▌': [0, 0, w / 2, h], '▐': [w / 2, 0, w / 2, h] }[c.ch];
    if (!rect) return false;
    ctx.fillStyle = PALETTE[c.fg];
    ctx.fillRect(x + rect[0], y + rect[1], rect[2], rect[3]);
    return true;
  }

  /** Le texte de l'écran (lignes sans espaces de fin). */
  text() {
    return this.cells.map((row) => row.map((c) => c.ch).join('').trimEnd()).join('\n').trimEnd();
  }
}
