/* tslint:disable */
/* eslint-disable */

/**
 * Résultat de [`assemble`].
 */
export class Assembled {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Blocs : JSON [[adresse, [octets...]], ...].
     */
    blocks_json(): string;
    /**
     * Première et dernière adresse ([] si le programme est vide).
     */
    bounds(): Uint16Array;
    /**
     * Symboles prédéfinis utilisés : JSON ["@DSPLY", ...].
     */
    builtins_used_json(): string;
    /**
     * Fichier .CMD.
     */
    cmd(): Uint8Array;
    /**
     * Diagnostics : JSON [{"line","warning","code","args","fix","insert"}].
     */
    diagnostics_json(): string;
    entry(): number;
    /**
     * Pour chaque ligne : adresse et nombre d'octets, à plat ([a0, n0, a1, n1, ...]).
     */
    lines(): Uint16Array;
    ok(): boolean;
    size(): number;
    /**
     * Symboles du programme : JSON [["NOM", valeur], ...].
     */
    symbols_json(): string;
    /**
     * Le programme appelle-t-il des services de LDOS ?
     */
    uses_ldos(): boolean;
}

export class Emulator {
    free(): void;
    [Symbol.dispose](): void;
    audio_len(): number;
    /**
     * Adresse et nombre des échantillons audio produits depuis `clear_audio`
     * (f32 dans la mémoire Wasm, lus sans copie par la page).
     */
    audio_ptr(): number;
    /**
     * Image vierge de disque dur : `cylinders` × `heads` (les valeurs proposées par le
     * pilote RSHARD : 306 × 4, 10 Mo). À formater par RSFORM.
     */
    static blank_hard_disk(cylinders: number, heads: number): Uint8Array;
    cancel_typing(): void;
    /**
     * Model II : touche CAPS enfoncée (lettres en majuscules).
     */
    caps(): boolean;
    clear_audio(): void;
    /**
     * Fréquence du Z80 (pour l'affichage d'informations).
     */
    static clock_hz(): number;
    /**
     * Copie un fichier d'un lecteur à un autre (4 et 5 : disques durs), avec son type entre
     * disques TRSDOS-II.
     */
    copy_disk_file(from: number, to: number, name: string): void;
    /**
     * Fréquence actuelle du processeur (pour l'affichage de la vitesse).
     */
    current_hz(): number;
    delete_disk_file(drive: number, name: string): void;
    /**
     * Accès aux lecteurs (commandes, sélections) depuis le démarrage : le moteur tourne.
     */
    disk_accesses(): number;
    /**
     * DOS de la disquette du lecteur : JSON {"name","family","system","label","files","writable"}, ou
     * "" (vide, vierge ou format inconnu).
     */
    disk_dos(drive: number): string;
    /**
     * Fichiers de la disquette : JSON [{"name","size","system"}]. Erreur : son code.
     */
    disk_files(drive: number): string;
    /**
     * Formats proposés pour un modèle : JSON [{"id","name"}].
     */
    static disk_formats(model: number): string;
    /**
     * Espace libre (octets) de la disquette.
     */
    disk_free(drive: number): number;
    /**
     * Géométrie de la disquette du lecteur, en JSON (vide sans disquette) : pistes, faces,
     * puis [secteurs, taille, double densité, premier secteur] de la piste 0 et des autres pistes.
     */
    disk_geometry(drive: number): string;
    /**
     * Image (modifiée) à enregistrer : le fichier d'origine (JV1, JV3) ou, après un
     * formatage ou pour une image DMK, une image JV3.
     */
    disk_image(drive: number): Uint8Array | undefined;
    /**
     * Format de l'image enregistrée par `disk_image` (« JV1 » ou « JV3 »).
     */
    disk_image_format(drive: number): string | undefined;
    disk_modified(drive: number): boolean;
    /**
     * Dernier accès aux disquettes : [n° de commande, lecteur, piste (tête), secteur,
     * commande] (vide avant le premier). Un nouveau n° signale un nouvel accès.
     */
    disk_position(): Uint32Array;
    /**
     * La disquette du lecteur a-t-elle été modifiée par le TRS-80 ?
     * Pas de la tête des lecteurs depuis le démarrage (compteur cumulatif).
     */
    disk_steps(): number;
    /**
     * Écritures sur la disquette du lecteur depuis son insertion (pour l'enregistrer).
     */
    disk_writes(drive: number): number;
    eject_disk(drive: number): void;
    eject_hard_disk(unit: number): void;
    eject_tape(): void;
    /**
     * Image d'une disquette de données vierge (format `id`, nom, date « MM/DD/YY »).
     */
    static format_image(id: string, name: string, date: string): Uint8Array;
    /**
     * Image du disque dur, avec les écritures du DOS.
     */
    hard_disk_image(unit: number): Uint8Array | undefined;
    /**
     * Le DOS a-t-il écrit sur le disque dur ?
     */
    hard_disk_modified(unit: number): boolean;
    /**
     * Écritures sur le disque dur depuis son insertion (pour l'enregistrer).
     */
    hard_disk_writes(unit: number): number;
    /**
     * Géométrie du disque dur : [cylindres, têtes] (vide sans disque).
     */
    hard_geometry(unit: number): Uint32Array;
    /**
     * Dernier accès au disque dur : [n° de commande, unité, cylindre, tête, secteur, commande].
     */
    hard_position(): Uint32Array;
    /**
     * Graphique haute résolution affiché : l'image fait 640 × 240, texte compris.
     */
    hires_active(): boolean;
    /**
     * Changements de l'image haute résolution (pour ne redessiner qu'au besoin).
     */
    hires_changes(): number;
    /**
     * DOS d'une image de disquette (même JSON), sans l'insérer.
     */
    static image_dos(image: Uint8Array): string;
    /**
     * Disquette vierge dans le lecteur (à formater par le DOS, ex. FORMAT :1).
     */
    insert_blank_disk(drive: number): void;
    /**
     * Insère une image de disquette (JV1, JV3 ou DMK) dans le lecteur `drive` (0 à 3).
     * Retourne une description (format, nombre de secteurs) pour la page.
     */
    insert_disk(drive: number, image: Uint8Array): string;
    /**
     * Branche une image de disque dur (Reed / HDV) sur l'unité `unit` (0 à 3) du contrôleur
     * Radio Shack. Retourne sa description (taille, têtes).
     */
    insert_hard_disk(unit: number, image: Uint8Array): string;
    /**
     * Met une cassette au magnétophone (lue par CLOAD, SYSTEM ou le chargeur d'un programme).
     */
    insert_tape(data: Uint8Array): void;
    /**
     * Vidéo inversée (bit 7 : caractère inversé, sinon bloc graphique).
     */
    inverse_video(): boolean;
    /**
     * Touche enfoncée, selon `KeyboardEvent.key`. Retourne `true` si la touche
     * existe sur le TRS-80 (la page annule alors l'action par défaut du fureteur).
     */
    key_down(name: string): boolean;
    key_up(name: string): void;
    /**
     * Lance un programme déjà en mémoire (retour à LDOS ou au BASIC); retourne l'adresse
     * de retour (402DH ou 1A19H).
     */
    launch(entry: number): number;
    /**
     * Charge une cassette `.CAS`. Un programme BASIC est aussitôt lancé (RUN tapé au
     * clavier). Retourne une description pour la barre d'état (en anglais).
     */
    load_cas(data: Uint8Array): string;
    /**
     * Charge et lance un programme `.CMD`; retourne son adresse de lancement.
     */
    load_cmd(data: Uint8Array): number;
    /**
     * Modèle émulé : 1, 2, 3 ou 4.
     */
    model(): number;
    /**
     * Crée l'émulateur à partir d'une ROM : 12 Ko (Model I), 14 Ko (Model III) ou 2 Ko (Model II).
     */
    constructor(rom: Uint8Array);
    /**
     * `len` octets de la mémoire à partir de `addr`.
     */
    peek_range(addr: number, len: number): Uint8Array;
    poke(addr: number, bytes: Uint8Array): void;
    /**
     * Imprimante : octets reçus depuis le dernier appel.
     */
    printer_take(): Uint8Array;
    read_disk_file(drive: number, name: string): Uint8Array;
    /**
     * Registres : AF BC DE HL IX IY SP PC AF' BC' DE' HL' I R IFF1 IM, arrêté (HALT).
     */
    registers(): Uint16Array;
    release_all_keys(): void;
    /**
     * Dessine l'écran et retourne l'adresse de l'image RGBA dans la mémoire Wasm.
     */
    render(): number;
    /**
     * Comme `render`, sans le texte (la page le dessine avec une autre police).
     */
    render_graphics(): number;
    reset(): void;
    restart_basic(): void;
    /**
     * Reprend un état (même modèle, même ROM); l'erreur est un code (`badFormat`,
     * `otherModel`, `otherRom`, `corrupt`).
     */
    restore_state(data: Uint8Array): void;
    /**
     * Reprend après un arrêt (l'instruction courante s'exécute même sur un point d'arrêt).
     */
    resume(): void;
    /**
     * Exécute `count` images de 1/60 s (plus d'une en mode turbo).
     */
    run_frames(count: number): void;
    /**
     * Redémarre en BASIC (RESET avec BREAK, ENTRÉE aux questions de la ROM), même avec un
     * DOS en marche : avant de taper un listing BASIC.
     * État complet de la machine (sans la ROM), à reprendre avec `restore_state`.
     */
    save_state(): Uint8Array;
    screen_height(): number;
    /**
     * Nombre de caractères par ligne et de lignes affichés (64 × 16, ou 80 × 24).
     * Texte de l'écran (lignes sans espaces de fin), pour le copier dans le presse-papiers.
     * Les blocs graphiques deviennent des espaces.
     */
    screen_text(): string;
    /**
     * Taille de l'image produite par `render` (pixels).
     */
    screen_width(): number;
    /**
     * RS-232 : vitesse choisie par le programme (bauds).
     */
    serial_baud(): number;
    /**
     * RS-232 : octets reçus pas encore lus par le TRS-80.
     */
    serial_pending(): number;
    /**
     * RS-232 : octets venus d'Internet (par le modem de la page), reçus par le TRS-80.
     */
    serial_send(bytes: Uint8Array): void;
    /**
     * RS-232 : octets émis par le TRS-80 depuis le dernier appel.
     */
    serial_take(): Uint8Array;
    /**
     * Active le son à la fréquence `rate` (celle de l'AudioContext); 0 le désactive.
     */
    set_audio_rate(rate: number): void;
    set_breakpoints(addresses: Uint16Array): void;
    set_exit_points(addresses: Uint16Array): void;
    /**
     * Interface d'expansion (horloge à 40 Hz).
     */
    set_expansion_interface(present: boolean): void;
    set_graphics_board(present: boolean): void;
    /**
     * Carte graphique haute résolution (Model III et 4) : branchée ou non.
     * Model I : modification « minuscules ».
     */
    set_lowercase_mod(on: boolean): void;
    /**
     * Arrêt dès que PC revient dans `lo..=hi` (pas à pas); `lo > hi` : plus de surveillance.
     */
    set_stop_range(lo: number, hi: number): void;
    /**
     * Modèle d'un état (1 à 4), ou 0 si ce n'est pas un état de l'émulateur.
     */
    static state_model(data: Uint8Array): number;
    /**
     * Exécute une seule instruction (sans tenir compte des points d'arrêt).
     */
    step_instruction(): void;
    stopped(): boolean;
    /**
     * Arrêt survenu : 0 aucun, 1 point d'arrêt, 2 retour dans la plage, 3 fin du programme.
     */
    take_stop(): number;
    /**
     * Magnétophone : [octets lus, taille, moteur en marche (0 ou 1), octets par seconde];
     * vide sans cassette.
     */
    tape_progress(): Uint32Array;
    text_cols(): number;
    text_rows(): number;
    /**
     * Tape un texte au clavier (ex. : programme BASIC collé); retourne le nombre de
     * caractères acceptés (ceux qui n'existent pas sur le TRS-80 sont ignorés).
     */
    type_text(text: string): number;
    /**
     * Une frappe automatique est-elle en cours ?
     */
    typing(): boolean;
    /**
     * Copie les caractères affichés (`text_cols × text_rows` octets) et retourne leur
     * adresse dans la mémoire Wasm.
     */
    video_ptr(): number;
    /**
     * Mode 32 caractères par ligne actif.
     */
    wide(): boolean;
    /**
     * Crée un modèle donné : 1, 2, 3 ou 4 (le Model 4 utilise la ROM du Model III).
     */
    static with_model(rom: Uint8Array, model: number): Emulator;
    write_disk_file(drive: number, name: string, data: Uint8Array): void;
}

/**
 * Symboles prédéfinis (services de LDOS, routines de la ROM) : JSON [["@DSPLY", 17511, true], ...].
 */
export function asm_builtins_json(): string;

/**
 * Assemble un source Z80 (syntaxe des assembleurs TRS-80).
 */
export function assemble(source: string): Assembled;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_assembled_free: (a: number, b: number) => void;
    readonly __wbg_emulator_free: (a: number, b: number) => void;
    readonly asm_builtins_json: () => [number, number];
    readonly assemble: (a: number, b: number) => number;
    readonly assembled_blocks_json: (a: number) => [number, number];
    readonly assembled_bounds: (a: number) => [number, number];
    readonly assembled_builtins_used_json: (a: number) => [number, number];
    readonly assembled_cmd: (a: number) => [number, number];
    readonly assembled_diagnostics_json: (a: number) => [number, number];
    readonly assembled_entry: (a: number) => number;
    readonly assembled_lines: (a: number) => [number, number];
    readonly assembled_ok: (a: number) => number;
    readonly assembled_size: (a: number) => number;
    readonly assembled_symbols_json: (a: number) => [number, number];
    readonly assembled_uses_ldos: (a: number) => number;
    readonly emulator_audio_len: (a: number) => number;
    readonly emulator_audio_ptr: (a: number) => number;
    readonly emulator_blank_hard_disk: (a: number, b: number) => [number, number];
    readonly emulator_cancel_typing: (a: number) => void;
    readonly emulator_caps: (a: number) => number;
    readonly emulator_clear_audio: (a: number) => void;
    readonly emulator_clock_hz: () => number;
    readonly emulator_copy_disk_file: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly emulator_current_hz: (a: number) => number;
    readonly emulator_delete_disk_file: (a: number, b: number, c: number, d: number) => [number, number];
    readonly emulator_disk_accesses: (a: number) => number;
    readonly emulator_disk_dos: (a: number, b: number) => [number, number];
    readonly emulator_disk_files: (a: number, b: number) => [number, number, number, number];
    readonly emulator_disk_formats: (a: number) => [number, number];
    readonly emulator_disk_free: (a: number, b: number) => [number, number, number];
    readonly emulator_disk_geometry: (a: number, b: number) => [number, number];
    readonly emulator_disk_image: (a: number, b: number) => [number, number];
    readonly emulator_disk_image_format: (a: number, b: number) => [number, number];
    readonly emulator_disk_modified: (a: number, b: number) => number;
    readonly emulator_disk_position: (a: number) => [number, number];
    readonly emulator_disk_steps: (a: number) => number;
    readonly emulator_disk_writes: (a: number, b: number) => number;
    readonly emulator_eject_disk: (a: number, b: number) => void;
    readonly emulator_eject_hard_disk: (a: number, b: number) => void;
    readonly emulator_eject_tape: (a: number) => void;
    readonly emulator_format_image: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly emulator_hard_disk_image: (a: number, b: number) => [number, number];
    readonly emulator_hard_disk_modified: (a: number, b: number) => number;
    readonly emulator_hard_disk_writes: (a: number, b: number) => number;
    readonly emulator_hard_geometry: (a: number, b: number) => [number, number];
    readonly emulator_hard_position: (a: number) => [number, number];
    readonly emulator_hires_active: (a: number) => number;
    readonly emulator_hires_changes: (a: number) => number;
    readonly emulator_image_dos: (a: number, b: number) => [number, number];
    readonly emulator_insert_blank_disk: (a: number, b: number) => void;
    readonly emulator_insert_disk: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly emulator_insert_hard_disk: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly emulator_insert_tape: (a: number, b: number, c: number) => void;
    readonly emulator_inverse_video: (a: number) => number;
    readonly emulator_key_down: (a: number, b: number, c: number) => number;
    readonly emulator_key_up: (a: number, b: number, c: number) => void;
    readonly emulator_launch: (a: number, b: number) => [number, number, number];
    readonly emulator_load_cas: (a: number, b: number, c: number) => [number, number, number, number];
    readonly emulator_load_cmd: (a: number, b: number, c: number) => [number, number, number];
    readonly emulator_model: (a: number) => number;
    readonly emulator_new: (a: number, b: number) => [number, number, number];
    readonly emulator_peek_range: (a: number, b: number, c: number) => [number, number];
    readonly emulator_poke: (a: number, b: number, c: number, d: number) => void;
    readonly emulator_printer_take: (a: number) => [number, number];
    readonly emulator_read_disk_file: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly emulator_registers: (a: number) => [number, number];
    readonly emulator_release_all_keys: (a: number) => void;
    readonly emulator_render: (a: number) => number;
    readonly emulator_render_graphics: (a: number) => number;
    readonly emulator_reset: (a: number) => void;
    readonly emulator_restart_basic: (a: number) => [number, number];
    readonly emulator_restore_state: (a: number, b: number, c: number) => [number, number];
    readonly emulator_resume: (a: number) => void;
    readonly emulator_run_frames: (a: number, b: number) => void;
    readonly emulator_save_state: (a: number) => [number, number];
    readonly emulator_screen_height: (a: number) => number;
    readonly emulator_screen_text: (a: number) => [number, number];
    readonly emulator_screen_width: (a: number) => number;
    readonly emulator_serial_baud: (a: number) => number;
    readonly emulator_serial_pending: (a: number) => number;
    readonly emulator_serial_send: (a: number, b: number, c: number) => void;
    readonly emulator_serial_take: (a: number) => [number, number];
    readonly emulator_set_audio_rate: (a: number, b: number) => void;
    readonly emulator_set_breakpoints: (a: number, b: number, c: number) => void;
    readonly emulator_set_exit_points: (a: number, b: number, c: number) => void;
    readonly emulator_set_expansion_interface: (a: number, b: number) => void;
    readonly emulator_set_graphics_board: (a: number, b: number) => void;
    readonly emulator_set_lowercase_mod: (a: number, b: number) => void;
    readonly emulator_set_stop_range: (a: number, b: number, c: number) => void;
    readonly emulator_state_model: (a: number, b: number) => number;
    readonly emulator_step_instruction: (a: number) => void;
    readonly emulator_stopped: (a: number) => number;
    readonly emulator_take_stop: (a: number) => number;
    readonly emulator_tape_progress: (a: number) => [number, number];
    readonly emulator_text_cols: (a: number) => number;
    readonly emulator_text_rows: (a: number) => number;
    readonly emulator_type_text: (a: number, b: number, c: number) => number;
    readonly emulator_typing: (a: number) => number;
    readonly emulator_video_ptr: (a: number) => number;
    readonly emulator_wide: (a: number) => number;
    readonly emulator_with_model: (a: number, b: number, c: number) => [number, number, number];
    readonly emulator_write_disk_file: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
