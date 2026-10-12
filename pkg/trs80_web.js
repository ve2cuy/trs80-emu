/* @ts-self-types="./trs80_web.d.ts" */

/**
 * Résultat de [`assemble`].
 */
export class Assembled {
    static __wrap(ptr) {
        const obj = Object.create(Assembled.prototype);
        obj.__wbg_ptr = ptr;
        AssembledFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        AssembledFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_assembled_free(ptr, 0);
    }
    /**
     * Blocs : JSON [[adresse, [octets...]], ...].
     * @returns {string}
     */
    blocks_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.assembled_blocks_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Première et dernière adresse ([] si le programme est vide).
     * @returns {Uint16Array}
     */
    bounds() {
        const ret = wasm.assembled_bounds(this.__wbg_ptr);
        var v1 = getArrayU16FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 2, 2);
        return v1;
    }
    /**
     * Symboles prédéfinis utilisés : JSON ["@DSPLY", ...].
     * @returns {string}
     */
    builtins_used_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.assembled_builtins_used_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Fichier .CMD.
     * @returns {Uint8Array}
     */
    cmd() {
        const ret = wasm.assembled_cmd(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Diagnostics : JSON [{"line","warning","code","args","fix","insert"}].
     * @returns {string}
     */
    diagnostics_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.assembled_diagnostics_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    entry() {
        const ret = wasm.assembled_entry(this.__wbg_ptr);
        return ret;
    }
    /**
     * Pour chaque ligne : adresse et nombre d'octets, à plat ([a0, n0, a1, n1, ...]).
     * @returns {Uint16Array}
     */
    lines() {
        const ret = wasm.assembled_lines(this.__wbg_ptr);
        var v1 = getArrayU16FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 2, 2);
        return v1;
    }
    /**
     * @returns {boolean}
     */
    ok() {
        const ret = wasm.assembled_ok(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    size() {
        const ret = wasm.assembled_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Symboles du programme : JSON [["NOM", valeur], ...].
     * @returns {string}
     */
    symbols_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.assembled_symbols_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Le programme appelle-t-il des services de LDOS ?
     * @returns {boolean}
     */
    uses_ldos() {
        const ret = wasm.assembled_uses_ldos(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) Assembled.prototype[Symbol.dispose] = Assembled.prototype.free;

export class Emulator {
    static __wrap(ptr) {
        const obj = Object.create(Emulator.prototype);
        obj.__wbg_ptr = ptr;
        EmulatorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EmulatorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_emulator_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    audio_len() {
        const ret = wasm.emulator_audio_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Adresse et nombre des échantillons audio produits depuis `clear_audio`
     * (f32 dans la mémoire Wasm, lus sans copie par la page).
     * @returns {number}
     */
    audio_ptr() {
        const ret = wasm.emulator_audio_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Image vierge de disque dur : `cylinders` × `heads` (les valeurs proposées par le
     * pilote RSHARD : 306 × 4, 10 Mo). À formater par RSFORM.
     * @param {number} cylinders
     * @param {number} heads
     * @returns {Uint8Array}
     */
    static blank_hard_disk(cylinders, heads) {
        const ret = wasm.emulator_blank_hard_disk(cylinders, heads);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    cancel_typing() {
        wasm.emulator_cancel_typing(this.__wbg_ptr);
    }
    /**
     * Model II : touche CAPS enfoncée (lettres en majuscules).
     * @returns {boolean}
     */
    caps() {
        const ret = wasm.emulator_caps(this.__wbg_ptr);
        return ret !== 0;
    }
    clear_audio() {
        wasm.emulator_clear_audio(this.__wbg_ptr);
    }
    /**
     * Fréquence du Z80 (pour l'affichage d'informations).
     * @returns {number}
     */
    static clock_hz() {
        const ret = wasm.emulator_clock_hz();
        return ret >>> 0;
    }
    /**
     * Copie un fichier d'un lecteur à un autre (4 et 5 : disques durs), avec son type entre
     * disques TRSDOS-II.
     * @param {number} from
     * @param {number} to
     * @param {string} name
     */
    copy_disk_file(from, to, name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_copy_disk_file(this.__wbg_ptr, from, to, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Fréquence actuelle du processeur (pour l'affichage de la vitesse).
     * @returns {number}
     */
    current_hz() {
        const ret = wasm.emulator_current_hz(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {number} drive
     * @param {string} name
     */
    delete_disk_file(drive, name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_delete_disk_file(this.__wbg_ptr, drive, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Accès aux lecteurs (commandes, sélections) depuis le démarrage : le moteur tourne.
     * @returns {number}
     */
    disk_accesses() {
        const ret = wasm.emulator_disk_accesses(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * DOS de la disquette du lecteur : JSON {"name","family","system","label","files","writable"}, ou
     * "" (vide, vierge ou format inconnu).
     * @param {number} drive
     * @returns {string}
     */
    disk_dos(drive) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.emulator_disk_dos(this.__wbg_ptr, drive);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Fichiers de la disquette : JSON [{"name","size","system"}]. Erreur : son code.
     * @param {number} drive
     * @returns {string}
     */
    disk_files(drive) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.emulator_disk_files(this.__wbg_ptr, drive);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Formats proposés pour un modèle : JSON [{"id","name"}].
     * @param {number} model
     * @returns {string}
     */
    static disk_formats(model) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.emulator_disk_formats(model);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Espace libre (octets) de la disquette.
     * @param {number} drive
     * @returns {number}
     */
    disk_free(drive) {
        const ret = wasm.emulator_disk_free(this.__wbg_ptr, drive);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Géométrie de la disquette du lecteur, en JSON (vide sans disquette) : pistes, faces,
     * puis [secteurs, taille, double densité, premier secteur] de la piste 0 et des autres pistes.
     * @param {number} drive
     * @returns {string}
     */
    disk_geometry(drive) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.emulator_disk_geometry(this.__wbg_ptr, drive);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Image (modifiée) à enregistrer : le fichier d'origine (JV1, JV3) ou, après un
     * formatage ou pour une image DMK, une image JV3.
     * @param {number} drive
     * @returns {Uint8Array | undefined}
     */
    disk_image(drive) {
        const ret = wasm.emulator_disk_image(this.__wbg_ptr, drive);
        let v1;
        if (ret[0] !== 0) {
            v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Format de l'image enregistrée par `disk_image` (« JV1 » ou « JV3 »).
     * @param {number} drive
     * @returns {string | undefined}
     */
    disk_image_format(drive) {
        const ret = wasm.emulator_disk_image_format(this.__wbg_ptr, drive);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]);
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * @param {number} drive
     * @returns {boolean}
     */
    disk_modified(drive) {
        const ret = wasm.emulator_disk_modified(this.__wbg_ptr, drive);
        return ret !== 0;
    }
    /**
     * Dernier accès aux disquettes : [n° de commande, lecteur, piste (tête), secteur,
     * commande] (vide avant le premier). Un nouveau n° signale un nouvel accès.
     * @returns {Uint32Array}
     */
    disk_position() {
        const ret = wasm.emulator_disk_position(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * La disquette du lecteur a-t-elle été modifiée par le TRS-80 ?
     * Pas de la tête des lecteurs depuis le démarrage (compteur cumulatif).
     * @returns {number}
     */
    disk_steps() {
        const ret = wasm.emulator_disk_steps(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Écritures sur la disquette du lecteur depuis son insertion (pour l'enregistrer).
     * @param {number} drive
     * @returns {number}
     */
    disk_writes(drive) {
        const ret = wasm.emulator_disk_writes(this.__wbg_ptr, drive);
        return ret >>> 0;
    }
    /**
     * @param {number} drive
     */
    eject_disk(drive) {
        wasm.emulator_eject_disk(this.__wbg_ptr, drive);
    }
    /**
     * @param {number} unit
     */
    eject_hard_disk(unit) {
        wasm.emulator_eject_hard_disk(this.__wbg_ptr, unit);
    }
    eject_tape() {
        wasm.emulator_eject_tape(this.__wbg_ptr);
    }
    /**
     * Image d'une disquette de données vierge (format `id`, nom, date « MM/DD/YY »).
     * @param {string} id
     * @param {string} name
     * @param {string} date
     * @returns {Uint8Array}
     */
    static format_image(id, name, date) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(date, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_format_image(ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v4 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v4;
    }
    /**
     * Image du disque dur, avec les écritures du DOS.
     * @param {number} unit
     * @returns {Uint8Array | undefined}
     */
    hard_disk_image(unit) {
        const ret = wasm.emulator_hard_disk_image(this.__wbg_ptr, unit);
        let v1;
        if (ret[0] !== 0) {
            v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Le DOS a-t-il écrit sur le disque dur ?
     * @param {number} unit
     * @returns {boolean}
     */
    hard_disk_modified(unit) {
        const ret = wasm.emulator_hard_disk_modified(this.__wbg_ptr, unit);
        return ret !== 0;
    }
    /**
     * Écritures sur le disque dur depuis son insertion (pour l'enregistrer).
     * @param {number} unit
     * @returns {number}
     */
    hard_disk_writes(unit) {
        const ret = wasm.emulator_hard_disk_writes(this.__wbg_ptr, unit);
        return ret >>> 0;
    }
    /**
     * Géométrie du disque dur : [cylindres, têtes] (vide sans disque).
     * @param {number} unit
     * @returns {Uint32Array}
     */
    hard_geometry(unit) {
        const ret = wasm.emulator_hard_geometry(this.__wbg_ptr, unit);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Dernier accès au disque dur : [n° de commande, unité, cylindre, tête, secteur, commande].
     * @returns {Uint32Array}
     */
    hard_position() {
        const ret = wasm.emulator_hard_position(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Graphique haute résolution affiché : l'image fait 640 × 240, texte compris.
     * @returns {boolean}
     */
    hires_active() {
        const ret = wasm.emulator_hires_active(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Changements de l'image haute résolution (pour ne redessiner qu'au besoin).
     * @returns {number}
     */
    hires_changes() {
        const ret = wasm.emulator_hires_changes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * DOS d'une image de disquette (même JSON), sans l'insérer.
     * @param {Uint8Array} image
     * @returns {string}
     */
    static image_dos(image) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passArray8ToWasm0(image, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.emulator_image_dos(ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Disquette vierge dans le lecteur (à formater par le DOS, ex. FORMAT :1).
     * @param {number} drive
     */
    insert_blank_disk(drive) {
        wasm.emulator_insert_blank_disk(this.__wbg_ptr, drive);
    }
    /**
     * Insère une image de disquette (JV1, JV3 ou DMK) dans le lecteur `drive` (0 à 3).
     * Retourne une description (format, nombre de secteurs) pour la page.
     * @param {number} drive
     * @param {Uint8Array} image
     * @returns {string}
     */
    insert_disk(drive, image) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(image, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.emulator_insert_disk(this.__wbg_ptr, drive, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Branche une image de disque dur (Reed / HDV) sur l'unité `unit` (0 à 3) du contrôleur
     * Radio Shack. Retourne sa description (taille, têtes).
     * @param {number} unit
     * @param {Uint8Array} image
     * @returns {string}
     */
    insert_hard_disk(unit, image) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(image, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.emulator_insert_hard_disk(this.__wbg_ptr, unit, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Met une cassette au magnétophone (lue par CLOAD, SYSTEM ou le chargeur d'un programme).
     * @param {Uint8Array} data
     */
    insert_tape(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_insert_tape(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Vidéo inversée (bit 7 : caractère inversé, sinon bloc graphique).
     * @returns {boolean}
     */
    inverse_video() {
        const ret = wasm.emulator_inverse_video(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Touche enfoncée, selon `KeyboardEvent.key`. Retourne `true` si la touche
     * existe sur le TRS-80 (la page annule alors l'action par défaut du fureteur).
     * @param {string} name
     * @returns {boolean}
     */
    key_down(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_key_down(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {string} name
     */
    key_up(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_key_up(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Lance un programme déjà en mémoire (retour à LDOS ou au BASIC); retourne l'adresse
     * de retour (402DH ou 1A19H).
     * @param {number} entry
     * @returns {number}
     */
    launch(entry) {
        const ret = wasm.emulator_launch(this.__wbg_ptr, entry);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0];
    }
    /**
     * Charge une cassette `.CAS`. Un programme BASIC est aussitôt lancé (RUN tapé au
     * clavier). Retourne une description pour la barre d'état (en anglais).
     * @param {Uint8Array} data
     * @returns {string}
     */
    load_cas(data) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.emulator_load_cas(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Charge et lance un programme `.CMD`; retourne son adresse de lancement.
     * @param {Uint8Array} data
     * @returns {number}
     */
    load_cmd(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_load_cmd(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0];
    }
    /**
     * Modèle émulé : 1, 2, 3 ou 4.
     * @returns {number}
     */
    model() {
        const ret = wasm.emulator_model(this.__wbg_ptr);
        return ret;
    }
    /**
     * Crée l'émulateur à partir d'une ROM : 12 Ko (Model I), 14 Ko (Model III) ou 2 Ko (Model II).
     * @param {Uint8Array} rom
     */
    constructor(rom) {
        const ptr0 = passArray8ToWasm0(rom, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        EmulatorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * `len` octets de la mémoire à partir de `addr`.
     * @param {number} addr
     * @param {number} len
     * @returns {Uint8Array}
     */
    peek_range(addr, len) {
        const ret = wasm.emulator_peek_range(this.__wbg_ptr, addr, len);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @param {number} addr
     * @param {Uint8Array} bytes
     */
    poke(addr, bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_poke(this.__wbg_ptr, addr, ptr0, len0);
    }
    /**
     * Imprimante : octets reçus depuis le dernier appel.
     * @returns {Uint8Array}
     */
    printer_take() {
        const ret = wasm.emulator_printer_take(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @param {number} drive
     * @param {string} name
     * @returns {Uint8Array}
     */
    read_disk_file(drive, name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_read_disk_file(this.__wbg_ptr, drive, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Registres : AF BC DE HL IX IY SP PC AF' BC' DE' HL' I R IFF1 IM, arrêté (HALT).
     * @returns {Uint16Array}
     */
    registers() {
        const ret = wasm.emulator_registers(this.__wbg_ptr);
        var v1 = getArrayU16FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 2, 2);
        return v1;
    }
    release_all_keys() {
        wasm.emulator_release_all_keys(this.__wbg_ptr);
    }
    /**
     * Dessine l'écran et retourne l'adresse de l'image RGBA dans la mémoire Wasm.
     * @returns {number}
     */
    render() {
        const ret = wasm.emulator_render(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Comme `render`, sans le texte (la page le dessine avec une autre police).
     * @returns {number}
     */
    render_graphics() {
        const ret = wasm.emulator_render_graphics(this.__wbg_ptr);
        return ret >>> 0;
    }
    reset() {
        wasm.emulator_reset(this.__wbg_ptr);
    }
    restart_basic() {
        const ret = wasm.emulator_restart_basic(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Reprend un état (même modèle, même ROM); l'erreur est un code (`badFormat`,
     * `otherModel`, `otherRom`, `corrupt`).
     * @param {Uint8Array} data
     */
    restore_state(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_restore_state(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Reprend après un arrêt (l'instruction courante s'exécute même sur un point d'arrêt).
     */
    resume() {
        wasm.emulator_resume(this.__wbg_ptr);
    }
    /**
     * Exécute `count` images de 1/60 s (plus d'une en mode turbo).
     * @param {number} count
     */
    run_frames(count) {
        wasm.emulator_run_frames(this.__wbg_ptr, count);
    }
    /**
     * Redémarre en BASIC (RESET avec BREAK, ENTRÉE aux questions de la ROM), même avec un
     * DOS en marche : avant de taper un listing BASIC.
     * État complet de la machine (sans la ROM), à reprendre avec `restore_state`.
     * @returns {Uint8Array}
     */
    save_state() {
        const ret = wasm.emulator_save_state(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @returns {number}
     */
    screen_height() {
        const ret = wasm.emulator_screen_height(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Nombre de caractères par ligne et de lignes affichés (64 × 16, ou 80 × 24).
     * Texte de l'écran (lignes sans espaces de fin), pour le copier dans le presse-papiers.
     * Les blocs graphiques deviennent des espaces.
     * @returns {string}
     */
    screen_text() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.emulator_screen_text(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Taille de l'image produite par `render` (pixels).
     * @returns {number}
     */
    screen_width() {
        const ret = wasm.emulator_screen_width(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * RS-232 : vitesse choisie par le programme (bauds).
     * @returns {number}
     */
    serial_baud() {
        const ret = wasm.emulator_serial_baud(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * RS-232 : octets reçus pas encore lus par le TRS-80.
     * @returns {number}
     */
    serial_pending() {
        const ret = wasm.emulator_serial_pending(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * RS-232 : octets venus d'Internet (par le modem de la page), reçus par le TRS-80.
     * @param {Uint8Array} bytes
     */
    serial_send(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_serial_send(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * RS-232 : octets émis par le TRS-80 depuis le dernier appel.
     * @returns {Uint8Array}
     */
    serial_take() {
        const ret = wasm.emulator_serial_take(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Active le son à la fréquence `rate` (celle de l'AudioContext); 0 le désactive.
     * @param {number} rate
     */
    set_audio_rate(rate) {
        wasm.emulator_set_audio_rate(this.__wbg_ptr, rate);
    }
    /**
     * @param {Uint16Array} addresses
     */
    set_breakpoints(addresses) {
        const ptr0 = passArray16ToWasm0(addresses, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_set_breakpoints(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {Uint16Array} addresses
     */
    set_exit_points(addresses) {
        const ptr0 = passArray16ToWasm0(addresses, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.emulator_set_exit_points(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Interface d'expansion (horloge à 40 Hz).
     * @param {boolean} present
     */
    set_expansion_interface(present) {
        wasm.emulator_set_expansion_interface(this.__wbg_ptr, present);
    }
    /**
     * @param {boolean} present
     */
    set_graphics_board(present) {
        wasm.emulator_set_graphics_board(this.__wbg_ptr, present);
    }
    /**
     * Carte graphique haute résolution (Model III et 4) : branchée ou non.
     * Model I : modification « minuscules ».
     * @param {boolean} on
     */
    set_lowercase_mod(on) {
        wasm.emulator_set_lowercase_mod(this.__wbg_ptr, on);
    }
    /**
     * Arrêt dès que PC revient dans `lo..=hi` (pas à pas); `lo > hi` : plus de surveillance.
     * @param {number} lo
     * @param {number} hi
     */
    set_stop_range(lo, hi) {
        wasm.emulator_set_stop_range(this.__wbg_ptr, lo, hi);
    }
    /**
     * Modèle d'un état (1 à 4), ou 0 si ce n'est pas un état de l'émulateur.
     * @param {Uint8Array} data
     * @returns {number}
     */
    static state_model(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_state_model(ptr0, len0);
        return ret;
    }
    /**
     * Exécute une seule instruction (sans tenir compte des points d'arrêt).
     */
    step_instruction() {
        wasm.emulator_step_instruction(this.__wbg_ptr);
    }
    /**
     * @returns {boolean}
     */
    stopped() {
        const ret = wasm.emulator_stopped(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Arrêt survenu : 0 aucun, 1 point d'arrêt, 2 retour dans la plage, 3 fin du programme.
     * @returns {number}
     */
    take_stop() {
        const ret = wasm.emulator_take_stop(this.__wbg_ptr);
        return ret;
    }
    /**
     * Magnétophone : [octets lus, taille, moteur en marche (0 ou 1), octets par seconde];
     * vide sans cassette.
     * @returns {Uint32Array}
     */
    tape_progress() {
        const ret = wasm.emulator_tape_progress(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @returns {number}
     */
    text_cols() {
        const ret = wasm.emulator_text_cols(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    text_rows() {
        const ret = wasm.emulator_text_rows(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Tape un texte au clavier (ex. : programme BASIC collé); retourne le nombre de
     * caractères acceptés (ceux qui n'existent pas sur le TRS-80 sont ignorés).
     * @param {string} text
     * @returns {number}
     */
    type_text(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_type_text(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Une frappe automatique est-elle en cours ?
     * @returns {boolean}
     */
    typing() {
        const ret = wasm.emulator_typing(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Copie les caractères affichés (`text_cols × text_rows` octets) et retourne leur
     * adresse dans la mémoire Wasm.
     * @returns {number}
     */
    video_ptr() {
        const ret = wasm.emulator_video_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Mode 32 caractères par ligne actif.
     * @returns {boolean}
     */
    wide() {
        const ret = wasm.emulator_wide(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Crée un modèle donné : 1, 2, 3 ou 4 (le Model 4 utilise la ROM du Model III).
     * @param {Uint8Array} rom
     * @param {number} model
     * @returns {Emulator}
     */
    static with_model(rom, model) {
        const ptr0 = passArray8ToWasm0(rom, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_with_model(ptr0, len0, model);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return Emulator.__wrap(ret[0]);
    }
    /**
     * @param {number} drive
     * @param {string} name
     * @param {Uint8Array} data
     */
    write_disk_file(drive, name, data) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.emulator_write_disk_file(this.__wbg_ptr, drive, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
}
if (Symbol.dispose) Emulator.prototype[Symbol.dispose] = Emulator.prototype.free;

/**
 * Symboles prédéfinis (services de LDOS, routines de la ROM) : JSON [["@DSPLY", 17511, true], ...].
 * @returns {string}
 */
export function asm_builtins_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.asm_builtins_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

/**
 * Assemble un source Z80 (syntaxe des assembleurs TRS-80).
 * @param {string} source
 * @returns {Assembled}
 */
export function assemble(source) {
    const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.assemble(ptr0, len0);
    return Assembled.__wrap(ret);
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_30c8987f7c2ed4e2: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg___wbindgen_throw_41e9ee4f547fc59a: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./trs80_web_bg.js": import0,
    };
}

const AssembledFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_assembled_free(ptr, 1));
const EmulatorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_emulator_free(ptr, 1));

function getArrayU16FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint16ArrayMemory0 = null;
function getUint16ArrayMemory0() {
    if (cachedUint16ArrayMemory0 === null || cachedUint16ArrayMemory0.byteLength === 0) {
        cachedUint16ArrayMemory0 = new Uint16Array(wasm.memory.buffer);
    }
    return cachedUint16ArrayMemory0;
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray16ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 2, 2) >>> 0;
    getUint16ArrayMemory0().set(arg, ptr / 2);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedUint16ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('trs80_web_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
