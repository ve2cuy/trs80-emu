// Atelier d'assemblage Z80 : éditeur, assemblage (crates/z80asm), exécution avec points
// d'arrêt et pas à pas, registres, fichiers sur la disquette (LDOS).
//
// main.js fournit `api` : l'émulateur courant, les traductions, la ligne d'état et quelques
// outils d'interface. En retour, la boucle d'affichage demande à l'atelier s'il est en
// pause (paused) et lui signale chaque groupe d'images exécutées (afterFrames).

const MNEMONICS = new Set(('ADC ADD AND BIT CALL CCF CP CPD CPDR CPI CPIR CPL DAA DEC DI DJNZ EI EX EXX HALT IM IN INC '
  + 'IND INDR INI INIR JP JR LD LDD LDDR LDI LDIR NEG NOP OR OTDR OTIR OUT OUTD OUTI POP PUSH RES RET RETI RETN RL RLA '
  + 'RLC RLCA RLD RR RRA RRC RRCA RRD RST SBC SCF SET SLA SLL SRA SRL SUB XOR').split(' '));
const DIRECTIVES = new Set('ORG EQU DEFL DB DEFB BYTE DM DEFM TEXT ASCII DW DEFW WORD DS DEFS BLOCK RMB END'.split(' '));
const REGISTERS = new Set("A B C D E H L I R AF AF' BC DE HL SP IX IY NZ Z NC PO PE P M".split(' '));
const STORE_KEY = 'trs80-ide';

const EXAMPLE = `; HELLO/ASM - LDOS services from a Z80 program
; Boot an LDOS disk (Disks menu), then Run or Debug.
        ORG     5200H

START:  LD      HL,MSG
        CALL    @DSPLY          ; display the message (ends with 0DH)
        LD      HL,BUFFER
        CALL    @TIME           ; current time, HH:MM:SS
        LD      HL,BUFFER
        CALL    @DSPLY
        LD      HL,BUFFER
        CALL    @DATE           ; current date, MM/DD/YY
        LD      HL,BUFFER
        CALL    @DSPLY
        JP      @EXIT           ; back to LDOS

MSG:    DB      'HELLO FROM THE Z80!',0DH
BUFFER: DS      8
        DB      0DH
        END     START
`;

function hex(v, digits = 4) {
  return v.toString(16).toUpperCase().padStart(digits, '0');
}

function escapeHtml(s) {
  return s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));
}

/** Coloration d'une ligne de source (HTML). */
function highlight(line) {
  if (line.startsWith('*')) return `<span class="tk-comment">${escapeHtml(line)}</span>`;
  // Séparer le commentaire (hors chaînes; l'apostrophe de AF' n'ouvre pas de chaîne).
  let code = line;
  let comment = '';
  let quote = null;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quote) {
      if (c === quote) quote = null;
    } else if (c === ';') {
      code = line.slice(0, i);
      comment = line.slice(i);
      break;
    } else if (c === '"' || (c === "'" && !/AF$/i.test(line.slice(0, i)))) {
      quote = c;
    }
  }
  let out = '';
  let first = true;
  const re = /('(?:[^']|'')*'?|"[^"]*"?)|([A-Za-z_@?.$][\w@?.$]*'?:?)|(\d[\dA-Fa-fHhXxBbOoQq]*|\$[\dA-Fa-f]+|%[01]+)|(\s+)|(.)/g;
  let m;
  while ((m = re.exec(code))) {
    const [text, str, word, num, space] = m;
    if (str) out += `<span class="tk-string">${escapeHtml(text)}</span>`;
    else if (word) {
      const upper = word.toUpperCase().replace(/:$/, '');
      const atStart = first && m.index === 0;
      let cls = '';
      if (word.endsWith(':') || (atStart && !MNEMONICS.has(upper) && !DIRECTIVES.has(upper))) cls = 'tk-label';
      else if (MNEMONICS.has(upper)) cls = 'tk-op';
      else if (DIRECTIVES.has(upper)) cls = 'tk-dir';
      else if (REGISTERS.has(upper)) cls = 'tk-reg';
      else if (upper.startsWith('@')) cls = 'tk-svc';
      out += cls ? `<span class="${cls}">${escapeHtml(text)}</span>` : escapeHtml(text);
    } else if (num) out += `<span class="tk-num">${escapeHtml(text)}</span>`;
    else out += escapeHtml(text);
    if (!space) first = false;
  }
  if (comment) out += `<span class="tk-comment">${escapeHtml(comment)}</span>`;
  return out;
}

/** Texte d'un fichier source : texte simple (CR ou CR LF) ou format EDTASM (numéros de ligne). */
function decodeSource(bytes) {
  const latin = (b) => new TextDecoder('latin1').decode(b);
  if (bytes[0] === 0xD3) {
    // EDTASM : D3h, nom (6), puis par ligne 5 chiffres (bit 7 levé), une espace, le texte, 0Dh.
    const lines = [];
    let i = 7;
    while (i < bytes.length && bytes[i] !== 0x1A) {
      i += 5;
      if (bytes[i] === 0x20 || bytes[i] === 0x09) i += 1;
      const start = i;
      while (i < bytes.length && bytes[i] !== 0x0D) i += 1;
      lines.push(latin(bytes.subarray(start, i)));
      i += 1;
    }
    return lines.join('\n') + '\n';
  }
  let end = bytes.length;
  while (end > 0 && (bytes[end - 1] === 0x1A || bytes[end - 1] === 0)) end -= 1;
  return latin(bytes.subarray(0, end)).replace(/\r\n?/g, '\n');
}

/** Source à enregistrer sur la disquette : fins de ligne CR, comme sur le TRS-80. */
function encodeSource(text) {
  const t = text.replace(/\r\n?/g, '\n').replace(/\n*$/, '\n').replace(/\n/g, '\r');
  return Uint8Array.from(t, (c) => (c.charCodeAt(0) < 256 ? c.charCodeAt(0) : 0x3F));
}

export function createIde(api) {
  const { t, element, textElement, setTip, iconButton } = api;
  const $ = (id) => document.getElementById(id);
  const panel = $('ide');
  const source = $('ide-source');
  const hl = $('ide-highlight');
  const gutter = $('ide-gutter');
  const nameInput = $('ide-name');
  const driveSelect = $('ide-drive');
  const messages = $('ide-messages');
  const summary = $('ide-summary');
  const filesBox = $('ide-files');
  const regsBox = $('ide-regs');
  const stateBox = $('ide-state');

  let store = {};
  try { store = JSON.parse(localStorage.getItem(STORE_KEY)) ?? {}; } catch { /* stockage indisponible */ }
  source.value = store.source ?? EXAMPLE;
  nameInput.value = store.name ?? 'HELLO';
  driveSelect.value = String(store.drive ?? 0);
  const breakpoints = new Set(store.breakpoints ?? []);

  let assembled = null; // { ok, entry, lo, hi, blocks, lineAddr: [[addr, len]], addrLine: Map, diagnostics, cmd, usesLdos }
  let diagnostics = [];
  let session = null;   // { lo, hi, back, paused, waiting }
  let lastRegs = null;
  let saveTimer = 0;

  function persist() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      try {
        localStorage.setItem(STORE_KEY, JSON.stringify({
          source: source.value, name: nameInput.value, drive: Number(driveSelect.value), breakpoints: [...breakpoints],
        }));
      } catch { /* stockage plein ou indisponible */ }
    }, 400);
  }

  // ---------------------------------------------------------------- éditeur

  function currentLine() {
    if (!session?.paused || !assembled) return -1;
    const pc = api.emulator()?.registers()[7];
    return assembled.addrLine.get(pc) ?? -1;
  }

  function render() {
    const lines = source.value.split('\n');
    const byLine = new Map();
    for (const d of diagnostics) {
      if (!byLine.has(d.line) || !d.warning) byLine.set(d.line, d.warning ? 'warn' : 'error');
    }
    const cur = currentLine();
    hl.innerHTML = lines.map((l, i) => {
      const n = i + 1;
      const cls = [n === cur ? 'cur' : '', byLine.has(n) ? byLine.get(n) : ''].filter(Boolean).join(' ');
      return `<div class="hl-line ${cls}">${highlight(l) || ' '}</div>`;
    }).join('');
    gutter.innerHTML = lines.map((_, i) => {
      const n = i + 1;
      const cls = [breakpoints.has(n) ? 'bp' : '', n === cur ? 'cur' : '', byLine.has(n) ? byLine.get(n) : ''].filter(Boolean).join(' ');
      return `<div class="gl ${cls}" data-line="${n}">${n}</div>`;
    }).join('');
    syncScroll();
  }

  function syncScroll() {
    hl.style.transform = `translate(${-source.scrollLeft}px, ${-source.scrollTop}px)`;
    gutter.style.transform = `translateY(${-source.scrollTop}px)`;
  }

  // Les points d'arrêt suivent les lignes insérées ou supprimées au-dessus d'eux.
  let lineCount = source.value.split('\n').length;
  source.addEventListener('input', () => {
    const count = source.value.split('\n').length;
    if (count !== lineCount && breakpoints.size) {
      const caret = source.value.slice(0, source.selectionStart).split('\n').length;
      const delta = count - lineCount;
      const moved = [...breakpoints].map((b) => (b >= caret - (delta > 0 ? 0 : delta) ? b + delta : b)).filter((b) => b >= 1);
      breakpoints.clear();
      moved.forEach((b) => breakpoints.add(b));
    }
    lineCount = count;
    assembled = null;
    render();
    persist();
  });
  source.addEventListener('scroll', syncScroll);
  // Tabulation : jusqu'à la colonne suivante (multiple de 8), comme dans EDTASM.
  source.addEventListener('keydown', (e) => {
    if (e.key !== 'Tab' || e.shiftKey || e.ctrlKey || e.altKey) return;
    e.preventDefault();
    const pos = source.selectionStart;
    const col = pos - (source.value.lastIndexOf('\n', pos - 1) + 1);
    source.setRangeText(' '.repeat(8 - (col % 8)), pos, source.selectionEnd, 'end');
    source.dispatchEvent(new Event('input'));
  });
  gutter.addEventListener('click', (e) => {
    const n = Number(e.target.closest('[data-line]')?.dataset.line);
    if (!n) return;
    if (breakpoints.has(n)) breakpoints.delete(n);
    else breakpoints.add(n);
    applyBreakpoints();
    render();
    persist();
  });
  nameInput.addEventListener('input', () => {
    nameInput.value = nameInput.value.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 8);
    persist();
  });
  driveSelect.addEventListener('change', persist);

  function goToLine(n) {
    const lines = source.value.split('\n');
    const start = lines.slice(0, n - 1).reduce((a, l) => a + l.length + 1, 0);
    source.focus();
    source.setSelectionRange(start, start + (lines[n - 1]?.length ?? 0));
    const lineHeight = parseFloat(getComputedStyle(source).lineHeight) || 18;
    source.scrollTop = Math.max(0, (n - 4) * lineHeight);
    syncScroll();
  }

  // ---------------------------------------------------------------- assemblage

  function message(d) {
    const values = Object.assign({}, d.args);
    const key = `asm.e.${d.code}`;
    const suggest = ['unknownMnemonic', 'undefinedSymbol'].includes(d.code) && d.args.length > 1;
    return t(suggest ? `${key}.suggest` : key, values);
  }

  function assembleNow() {
    const r = api.assemble(source.value);
    const lines = r.lines();
    const lineAddr = [];
    const addrLine = new Map();
    for (let i = 0; i < lines.length; i += 2) {
      lineAddr.push([lines[i], lines[i + 1]]);
      if (lines[i + 1] > 0 && !addrLine.has(lines[i])) addrLine.set(lines[i], i / 2 + 1);
    }
    const bounds = r.bounds();
    // Services de LDOS sans équivalent quand aucun DOS n'est chargé (voir Trs80::launch).
    const withoutDos = new Set(['@DSPLY', '@EXIT', '@ABORT']);
    const ldosOnly = JSON.parse(r.builtins_used_json()).filter((n) => !withoutDos.has(n)
      && api.builtins().some(([name, , ldos]) => name === n && ldos));
    assembled = {
      ok: r.ok(), entry: r.entry(), size: r.size(), usesLdos: r.uses_ldos(), ldosOnly,
      lo: bounds[0], hi: bounds[1], blocks: JSON.parse(r.blocks_json()), cmd: r.cmd(),
      lineAddr, addrLine,
    };
    diagnostics = JSON.parse(r.diagnostics_json());
    r.free();
    renderMessages();
    render();
    return assembled;
  }

  function renderMessages() {
    const errors = diagnostics.filter((d) => !d.warning).length;
    const warnings = diagnostics.length - errors;
    if (!assembled) {
      summary.textContent = '';
    } else if (assembled.ok) {
      summary.className = 'ide-summary ok';
      summary.textContent = assembled.size
        ? t('ide.ok', { size: assembled.size, lo: hex(assembled.lo), hi: hex(assembled.hi), entry: hex(assembled.entry) })
          + (warnings ? ` ${t('ide.warnings', { n: warnings })}` : '')
        : t('ide.empty');
    } else {
      summary.className = 'ide-summary error';
      summary.textContent = t('ide.errors', { n: errors }) + (warnings ? ` ${t('ide.warnings', { n: warnings })}` : '');
    }
    messages.replaceChildren(...diagnostics.map((d) => {
      const li = element('li', d.warning ? 'warn' : 'error');
      const where = element('button', 'ide-line', t('ide.line', { n: d.line }));
      where.type = 'button';
      where.addEventListener('click', () => goToLine(d.line));
      const text = element('span', 'ide-msg', message(d));
      li.append(where, text);
      if (d.fix != null) {
        const fix = element('button', 'secondary ide-fix');
        fix.type = 'button';
        fix.append(element('span', '', d.insert ? t('ide.insertLine') : t('ide.applyFix')), element('code', '', d.fix.trim()));
        fix.addEventListener('click', () => applyFix(d));
        li.append(fix);
      }
      return li;
    }));
  }

  function applyFix(d) {
    const lines = source.value.split('\n');
    if (d.insert) lines.splice(d.line - 1, 0, d.fix);
    else lines[d.line - 1] = d.fix;
    source.value = lines.join('\n');
    lineCount = lines.length;
    if (d.insert) {
      const moved = [...breakpoints].map((b) => (b >= d.line ? b + 1 : b));
      breakpoints.clear();
      moved.forEach((b) => breakpoints.add(b));
    }
    persist();
    assembleNow();
  }

  // ---------------------------------------------------------------- exécution

  function lineBreakpoints() {
    if (!assembled) return [];
    return [...breakpoints].map((n) => assembled.lineAddr[n - 1]).filter((l) => l && l[1] > 0).map((l) => l[0]);
  }

  function applyBreakpoints() {
    const emu = api.emulator();
    if (session && emu) emu.set_breakpoints(Uint16Array.from(lineBreakpoints()));
  }

  function setState(text) {
    stateBox.textContent = text;
  }

  function updateButtons() {
    const active = !!session;
    $('ide-step').disabled = !session?.paused;
    $('ide-continue').disabled = !active;
    $('ide-stop').disabled = !active;
    const cont = $('ide-continue');
    cont.querySelector('span').textContent = session && !session.paused ? t('ide.pause') : t('ide.continue');
    regsBox.classList.toggle('live', active);
  }

  function endSession(text) {
    const emu = api.emulator();
    if (emu) {
      emu.set_breakpoints(new Uint16Array());
      emu.set_exit_points(new Uint16Array());
      emu.set_stop_range(1, 0);
      if (emu.stopped()) emu.take_stop();
    }
    session = null;
    if (text) setState(text);
    updateButtons();
    render();
  }

  function start(debug) {
    const emu = api.emulator();
    if (!emu) {
      api.showStatus(t('prog.hint'), true);
      return;
    }
    const asm = assembleNow();
    if (!asm.ok) return;
    if (!asm.size) {
      setState(t('ide.noCode'));
      return;
    }
    if (session) endSession();
    if (!api.hasDos() && asm.ldosOnly.length) {
      // Sans DOS, ces services n'existent pas : le programme partirait dans la mémoire vide.
      setState(t('ide.ldosNeeded'));
      return;
    }
    for (const [addr, bytes] of asm.blocks) emu.poke(addr, Uint8Array.from(bytes));
    let back;
    try {
      back = emu.launch(asm.entry);
    } catch (e) {
      api.showStatus(t('run.fail', { name: nameInput.value, msg: e.message ?? e }), true);
      return;
    }
    session = { lo: asm.lo, hi: asm.hi, back, paused: debug, waiting: false };
    emu.set_exit_points(Uint16Array.from([back]));
    applyBreakpoints();
    if (debug) {
      setState(pausedText());
      lastRegs = null;
    } else {
      emu.resume();
      setState(t('ide.running'));
    }
    api.redraw();
    api.showScreen();
    updateButtons();
    showRegisters();
    render();
  }

  function pausedText() {
    const pc = api.emulator().registers()[7];
    const line = assembled?.addrLine.get(pc);
    return line ? t('ide.paused', { addr: hex(pc), line }) : t('ide.pausedOutside', { addr: hex(pc) });
  }

  function step() {
    const emu = api.emulator();
    if (!session?.paused || !emu) return;
    lastRegs = emu.registers();
    emu.step_instruction();
    const pc = emu.registers()[7];
    api.redraw();
    if (pc >= session.lo && pc <= session.hi) {
      setState(pausedText());
      showRegisters();
      render();
      return;
    }
    if (pc === session.back) {
      // Le programme vient de rendre la main (RET ou JP @EXIT).
      emu.resume();
      endSession(t('ide.ended', { where: t(session.back === 0x402D ? 'ide.whereDos' : 'ide.whereBasic') }));
      return;
    }
    // Appel au système ou interruption : on court jusqu'au retour dans le programme.
    emu.set_stop_range(session.lo, session.hi);
    session.paused = false;
    session.waiting = true;
    setState(t('ide.waitRange'));
    updateButtons();
    render();
  }

  function continueOrPause() {
    const emu = api.emulator();
    if (!session || !emu) return;
    if (session.paused) {
      emu.set_stop_range(1, 0);
      emu.resume();
      session.paused = false;
      session.waiting = false;
      lastRegs = emu.registers();
      setState(t('ide.running'));
      api.showScreen();
    } else {
      // Pause à la prochaine instruction du programme.
      emu.set_stop_range(session.lo, session.hi);
      session.waiting = true;
      setState(t('ide.waitRange'));
    }
    updateButtons();
    render();
  }

  /** Appelé par la boucle d'affichage après chaque groupe d'images. */
  function afterFrames() {
    const emu = api.emulator();
    if (!session || !emu) return;
    if (emu.stopped()) {
      const kind = emu.take_stop();
      if (kind === 3) {
        endSession(t('ide.ended', { where: t(session.back === 0x402D ? 'ide.whereDos' : 'ide.whereBasic') }));
        return;
      }
      emu.set_stop_range(1, 0);
      session.paused = true;
      session.waiting = false;
      setState(pausedText());
      updateButtons();
      showRegisters();
      render();
      const line = currentLine();
      if (line > 0) goToLineQuietly(line);
    } else if (performance.now() - (afterFrames.last ?? 0) > 250) {
      afterFrames.last = performance.now();
      showRegisters();
    }
  }

  function goToLineQuietly(n) {
    const lineHeight = parseFloat(getComputedStyle(source).lineHeight) || 18;
    const top = (n - 1) * lineHeight;
    if (top < source.scrollTop || top > source.scrollTop + source.clientHeight - lineHeight * 2) {
      source.scrollTop = Math.max(0, top - lineHeight * 4);
      syncScroll();
    }
  }

  // ---------------------------------------------------------------- registres

  const REG_NAMES = ['AF', 'BC', 'DE', 'HL', 'IX', 'IY', 'SP', 'PC', "AF'", "BC'", "DE'", "HL'"];
  const FLAGS = [['S', 0x80], ['Z', 0x40], ['H', 0x10], ['P/V', 0x04], ['N', 0x02], ['C', 0x01]];

  function showRegisters() {
    const emu = api.emulator();
    if (!emu || panel.hidden) return;
    const r = emu.registers();
    const cells = REG_NAMES.map((name, i) => {
      const changed = lastRegs && lastRegs[i] !== r[i] && session?.paused;
      return `<div class="reg${changed ? ' changed' : ''}"><span>${name}</span><code>${hex(r[i])}</code></div>`;
    });
    cells.push(`<div class="reg"><span>I</span><code>${hex(r[12], 2)}</code></div>`,
      `<div class="reg"><span>R</span><code>${hex(r[13], 2)}</code></div>`,
      `<div class="reg"><span>IM</span><code>${r[15]}</code></div>`,
      `<div class="reg"><span>IFF</span><code>${r[14] ? 'EI' : 'DI'}</code></div>`);
    const f = r[0] & 0xFF;
    const flags = FLAGS.map(([n, bit]) => `<span class="flag${f & bit ? ' on' : ''}">${n}</span>`).join('');
    const bytes = (addr, n) => [...emu.peek_range(addr, n)].map((b) => hex(b, 2)).join(' ');
    const stack = emu.peek_range(r[6], 8);
    const words = [0, 2, 4, 6].map((i) => hex(stack[i] | (stack[i + 1] << 8))).join(' ');
    $('ide-reg-grid').innerHTML = cells.join('');
    $('ide-flags').innerHTML = flags;
    $('ide-mem').innerHTML = `<div><span>PC</span><code>${bytes(r[7], 6)}</code></div>`
      + `<div><span>(HL)</span><code>${bytes(r[3], 8)}</code></div>`
      + `<div><span>${escapeHtml(t('ide.stack'))}</span><code>${words}</code></div>`;
  }

  // ---------------------------------------------------------------- fichiers

  function fsError(e, name) {
    const code = String(e?.message ?? e);
    return t(`ide.fs.${code}`, { d: driveSelect.value, name }) || code;
  }

  function fileName(ext) {
    return `${nameInput.value || 'NONAME'}/${ext}`;
  }

  async function saveToDisk() {
    const emu = api.emulator();
    if (!emu) return api.showStatus(t('prog.hint'), true);
    const drive = Number(driveSelect.value);
    if (!/^[A-Z][A-Z0-9]{0,7}$/.test(nameInput.value)) return setState(t('ide.fs.badName'));
    let existing = [];
    try {
      existing = JSON.parse(emu.disk_files(drive)).map((f) => f.name);
    } catch (e) {
      return setState(fsError(e, fileName('ASM')));
    }
    const asm = assembleNow();
    const files = [[fileName('ASM'), encodeSource(source.value)]];
    if (asm.ok && asm.size) files.push([fileName('CMD'), asm.cmd]);
    const clash = files.map((f) => f[0]).filter((n) => existing.includes(n));
    if (clash.length && !confirm(t('ide.overwrite', { name: clash.join(', ') }))) return;
    try {
      for (const [name, data] of files) emu.write_disk_file(drive, name, data);
    } catch (e) {
      return setState(fsError(e, files[0][0]));
    }
    setState(t('ide.saved', { files: files.map((f) => f[0]).join(', '), d: drive }));
    api.diskChanged();
  }

  function listFiles() {
    const emu = api.emulator();
    if (!emu) return api.showStatus(t('prog.hint'), true);
    if (!filesBox.hidden) {
      filesBox.hidden = true;
      return;
    }
    const drive = Number(driveSelect.value);
    let files;
    try {
      files = JSON.parse(emu.disk_files(drive)).filter((f) => !f.system);
    } catch (e) {
      return setState(fsError(e, ''));
    }
    const text = files.filter((f) => /\/(ASM|SRC|TXT|Z80|MAC|EDT|BAS)$/.test(f.name) || !f.name.includes('/'));
    const list = (text.length ? text : files).map((f) => {
      const b = element('button', 'secondary ide-file');
      b.type = 'button';
      b.append(element('code', '', f.name), element('span', '', `${f.size} B`));
      b.addEventListener('click', () => openFromDisk(drive, f.name));
      return b;
    });
    filesBox.replaceChildren(element('p', 'hint', t('ide.filesTitle', { d: drive })), ...list);
    if (!list.length) filesBox.append(element('p', 'hint', t('ide.noFiles')));
    filesBox.hidden = false;
  }

  function openFromDisk(drive, name) {
    try {
      const bytes = api.emulator().read_disk_file(drive, name);
      loadSource(decodeSource(bytes), name);
      filesBox.hidden = true;
      setState(t('ide.opened', { name, d: drive }));
    } catch (e) {
      setState(fsError(e, name));
    }
  }

  function loadSource(text, name) {
    if (session) endSession();
    source.value = text;
    lineCount = text.split('\n').length;
    breakpoints.clear();
    if (name) nameInput.value = name.split(/[/.]/)[0].toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 8);
    diagnostics = [];
    assembled = null;
    renderMessages();
    render();
    persist();
  }

  // ---------------------------------------------------------------- aide

  function renderHelp() {
    const rows = api.builtins().map(([name, value, ldos]) => {
      const tr = document.createElement('tr');
      const b = element('button', 'linklike', name);
      b.type = 'button';
      setTip(b, 'ide.insertName');
      b.addEventListener('click', () => {
        source.focus();
        source.setRangeText(name, source.selectionStart, source.selectionEnd, 'end');
        source.dispatchEvent(new Event('input'));
      });
      const td = document.createElement('td');
      td.append(b);
      tr.append(td, element('td', 'mono', `${hex(value)}H`), element('td', ldos ? 'svc-ldos' : 'svc-rom', ldos ? 'LDOS' : 'ROM'),
        element('td', '', t(`asm.svc.${name}`)));
      return tr;
    });
    $('ide-svc').replaceChildren(...rows);
  }

  // ---------------------------------------------------------------- boutons

  $('ide-asm').addEventListener('click', assembleNow);
  $('ide-run').addEventListener('click', () => start(false));
  $('ide-debug').addEventListener('click', () => start(true));
  $('ide-step').addEventListener('click', step);
  $('ide-continue').addEventListener('click', continueOrPause);
  $('ide-stop').addEventListener('click', () => endSession(t('ide.stopped')));
  $('ide-save').addEventListener('click', saveToDisk);
  $('ide-open').addEventListener('click', listFiles);
  $('ide-new').addEventListener('click', () => {
    if (confirm(t('ide.confirmNew'))) loadSource(EXAMPLE, 'HELLO');
  });
  $('ide-dl-cmd').addEventListener('click', () => {
    const asm = assembleNow();
    if (asm.ok && asm.size) api.download(asm.cmd, `${nameInput.value || 'PROGRAM'}.CMD`);
  });
  $('ide-dl-asm').addEventListener('click', () => api.download(encodeSource(source.value), `${nameInput.value || 'PROGRAM'}.ASM`));
  $('ide-local').addEventListener('change', async (e) => {
    const file = e.target.files[0];
    e.target.value = '';
    if (file) loadSource(decodeSource(new Uint8Array(await file.arrayBuffer())), file.name);
  });
  // F5 : exécuter, F9 : assembler, F10 : pas, quand le focus est dans l'éditeur.
  panel.addEventListener('keydown', (e) => {
    const action = { F5: () => (session ? continueOrPause() : start(false)), F9: assembleNow, F10: step }[e.key];
    if (action) {
      e.preventDefault();
      action();
    }
  });

  render();
  renderHelp();
  updateButtons();

  return {
    /** La boucle d'affichage doit-elle suspendre l'émulation ? */
    paused: () => !!session?.paused,
    afterFrames,
    show() {
      render();
      showRegisters();
    },
    onLanguage() {
      renderMessages();
      renderHelp();
      updateButtons();
      if (session?.paused) setState(pausedText());
      showRegisters();
    },
    /** Ouvre une source (fichier du dépôt, par exemple). */
    open(bytes, name) {
      loadSource(decodeSource(bytes), name);
    },
    /** Nouvel émulateur (autre ROM) : la session de débogage n'a plus de sens. */
    reset() {
      if (session) endSession(t('ide.stopped'));
    },
  };
}
