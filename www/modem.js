// Modem Hayes virtuel branché sur le port RS-232 du TRS-80.
//
// En mode commande, le modem lit les commandes AT tapées dans le programme de communication
// (LCOMM, COMM...) : ATDT bbs.electrodrome.net compose le « numéro », c'est-à-dire ouvre un
// WebSocket vers le relais telnet (server/telnet-relay), qui se connecte au BBS. En ligne,
// les octets passent tels quels; +++ (précédé et suivi d'une seconde de silence) revient au
// mode commande, ATO retourne en ligne, ATH raccroche.
//
// Commandes : AT, ATZ, ATE0/ATE1 (écho), ATH (raccrocher), ATO (retour en ligne), ATI,
// ATD/ATDT/ATDP hôte[:port] (aussi ATDT"hôte"), AT&F. Les autres répondent OK.

const GUARD_MS = 1000;

export class Modem {
  /**
   * @param {object} io
   * @param {(bytes: Uint8Array) => void} io.send    octets vers le TRS-80
   * @param {() => string} io.relay                  adresse du relais (wss://...)
   * @param {() => number} io.baud                   vitesse choisie par le TRS-80
   * @param {() => boolean} io.filter                retirer les séquences ANSI et les octets > 7Eh
   * @param {(state: object) => void} io.onState     changement d'état (pour la page)
   */
  constructor(io) {
    this.io = io;
    this.echo = true;
    this.line = '';
    this.socket = null;
    this.online = false; // en ligne (sinon : mode commande, éventuellement connecté)
    this.host = '';
    this.lastByte = 0;
    this.plus = 0; // « + » de la séquence d'échappement
    this.plusAt = 0;
    this.ansi = 0; // état du filtre ANSI : 0 texte, 1 après ESC, 2 dans ESC [
  }

  state() {
    return { connected: !!this.socket && this.socket.readyState === WebSocket.OPEN, online: this.online, host: this.host };
  }

  notify() {
    this.io.onState?.(this.state());
  }

  reply(text) {
    this.io.send(new TextEncoder().encode(`\r\n${text}\r\n`));
  }

  /** Octets émis par le TRS-80. */
  write(bytes) {
    const now = performance.now();
    for (const b of bytes) {
      if (this.online) {
        // Séquence d'échappement « +++ » : silence, trois +, silence (vérifié dans tick).
        if (b === 0x2B && (this.plus > 0 || now - this.lastByte >= GUARD_MS) && this.plus < 3) {
          this.plus++;
          this.plusAt = now;
        } else {
          this.flushPlus();
          this.sendRemote([b]);
        }
      } else {
        this.command(b);
      }
      this.lastByte = now;
    }
  }

  /** Appelé régulièrement : fin de la séquence « +++ » après une seconde de silence. */
  tick() {
    if (this.plus === 3 && performance.now() - this.plusAt >= GUARD_MS) {
      this.plus = 0;
      this.online = false;
      this.reply('OK');
      this.notify();
    }
  }

  flushPlus() {
    if (this.plus > 0) {
      this.sendRemote(new Array(this.plus).fill(0x2B));
      this.plus = 0;
    }
  }

  sendRemote(bytes) {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(new Uint8Array(bytes));
  }

  command(b) {
    const c = b & 0x7F;
    if (this.echo) this.io.send(new Uint8Array([c]));
    if (c === 0x0D) {
      const line = this.line.trim();
      this.line = '';
      if (/^AT/i.test(line)) this.execute(line.slice(2));
    } else if (c === 0x08 || c === 0x7F) {
      this.line = this.line.slice(0, -1);
    } else if (c >= 0x20) {
      this.line += String.fromCharCode(c);
    }
  }

  execute(rest) {
    let s = rest.trim();
    while (s.length) {
      const cmd = s[0].toUpperCase();
      if (cmd === 'D') {
        // Composer : le reste de la ligne est l'adresse (T et P ignorés).
        let target = s.slice(1).trim().replace(/^[TP]/i, '').trim().replace(/^"|"$/g, '');
        this.dial(target);
        return;
      }
      const m = /^(&?[A-Z])(\d*)/i.exec(s);
      if (!m) {
        this.reply('ERROR');
        return;
      }
      s = s.slice(m[0].length).trim();
      const name = m[1].toUpperCase();
      const arg = Number(m[2] || 0);
      if (name === 'E') this.echo = arg !== 0;
      else if (name === 'H') this.hangup(false);
      else if (name === 'Z' || name === '&F') {
        this.echo = true;
        this.hangup(false);
      } else if (name === 'O') {
        if (this.state().connected) {
          this.online = true;
          this.reply(`CONNECT ${this.io.baud()}`);
          this.notify();
        } else {
          this.reply('NO CARRIER');
        }
        return;
      } else if (name === 'I') {
        this.reply('TRS-80 EMULATOR TELNET MODEM');
      }
    }
    this.reply('OK');
  }

  dial(target) {
    const [host, port] = target.split(':');
    if (!host) {
      this.reply('NO DIALTONE');
      return;
    }
    this.hangup(false);
    let url;
    try {
      url = new URL(this.io.relay());
      url.searchParams.set('host', host.toLowerCase());
      url.searchParams.set('port', String(Number(port) || 23));
    } catch {
      this.reply('NO DIALTONE');
      return;
    }
    const socket = new WebSocket(url);
    socket.binaryType = 'arraybuffer';
    this.socket = socket;
    this.host = host;
    let opened = false;
    socket.onopen = () => {
      if (this.socket !== socket) return;
      opened = true;
      this.online = true;
      this.ansi = 0;
      this.reply(`CONNECT ${this.io.baud()}`);
      this.notify();
    };
    socket.onmessage = (event) => {
      if (this.socket !== socket) return;
      const bytes = new Uint8Array(event.data);
      this.io.send(this.io.filter() ? this.clean(bytes) : bytes);
    };
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.online = false;
      // Refusé par le relais ou injoignable : BUSY; connexion terminée : NO CARRIER.
      this.reply(opened ? 'NO CARRIER' : 'BUSY');
      this.notify();
    };
    this.notify();
  }

  hangup(report = true) {
    const socket = this.socket;
    this.socket = null;
    this.online = false;
    this.plus = 0;
    if (socket) {
      socket.close();
      if (report) this.reply('NO CARRIER');
    }
    this.notify();
  }

  /** Texte affichable par le TRS-80 : sans séquences ANSI, octets > 7Eh remplacés. */
  clean(bytes) {
    const out = [];
    for (const b of bytes) {
      if (this.ansi === 1) {
        this.ansi = b === 0x5B ? 2 : 0; // ESC [ : séquence CSI; sinon ESC + un caractère
        continue;
      }
      if (this.ansi === 2) {
        if (b >= 0x40 && b <= 0x7E) this.ansi = 0; // lettre finale de la séquence
        continue;
      }
      if (b === 0x1B) {
        this.ansi = 1;
      } else if (b === 0x0D || b === 0x0A || b === 0x08 || b === 0x09 || b === 0x07) {
        out.push(b);
      } else if (b >= 0x20 && b <= 0x7E) {
        out.push(b);
      } else if (b >= 0x80) {
        out.push(0x2E); // caractère graphique (CP437...) : un point
      }
    }
    return new Uint8Array(out);
  }
}
