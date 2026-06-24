'use strict';
// bf-wasm.js — le site devient TURING COMPLET : un INTERPRETE + un COMPILATEUR WASM, sur la page.
//
// José : « il faut que le site soit Turing complet, un interprète ET un compilateur wasm directement
// sur la page. » Brainfuck est le plus petit langage Turing-complet prouvé (8 instructions) — donc le
// PROUVER en compilant BF -> WebAssembly *dans le navigateur* (zero serveur, zero toolchain) suffit à
// rendre la page universelle. Le même encodeur wasm sert ensuite à compiler des ploxions in-browser
// (l'unlock RepoVerse : n'importe qui écrit/compile/lance son ploxion sur la page).
//
//   compileBF(src) -> Uint8Array  (un module .wasm valide)
//   runWasm(bytes, input) -> Promise<string>  (instancie + exécute, sortie via import env.out)
//   interpBF(src, input) -> string  (interprète direct, même résultat)
//
// Zéro dépendance. Marche en navigateur (window.BFWasm) et en node (module.exports).
(function (root) {
  // ---------- encodage wasm (LEB128 + sections) ----------
  function uleb(n) { const b = []; n >>>= 0; do { let x = n & 0x7f; n >>>= 7; if (n) x |= 0x80; b.push(x); } while (n); return b; }
  function sleb(n) { const b = []; let more = 1; while (more) { let x = n & 0x7f; n >>= 7; if ((n === 0 && (x & 0x40) === 0) || (n === -1 && (x & 0x40))) more = 0; else x |= 0x80; b.push(x); } return b; }
  function nm(s) { const u = Array.from(s, c => c.charCodeAt(0)); return [u.length, ...u]; }
  function sec(id, body) { return [id, ...uleb(body.length), ...body]; }

  const OP = {
    block: 0x02, loop: 0x03, end: 0x0b, br: 0x0c, br_if: 0x0d, call: 0x10,
    local_get: 0x20, local_set: 0x21, i32_load8_u: 0x2d, i32_store8: 0x3a,
    i32_const: 0x41, i32_eqz: 0x45, i32_add: 0x6a, i32_sub: 0x6b,
  };

  // Compile Brainfuck -> bytes d'un module wasm. Imports: env.out(i32)->(), env.in()->i32.
  // Exports: run()->() et memory. Pointeur de données = local i32 #0 (init 0).
  function compileBF(src) {
    const code = [];
    const P = 0;
    const get = () => code.push(OP.local_get, P);
    for (const ch of src) {
      switch (ch) {
        case '>': get(); code.push(OP.i32_const, ...sleb(1), OP.i32_add, OP.local_set, P); break;
        case '<': get(); code.push(OP.i32_const, ...sleb(1), OP.i32_sub, OP.local_set, P); break;
        case '+': get(); get(); code.push(OP.i32_load8_u, 0x00, 0x00, OP.i32_const, ...sleb(1), OP.i32_add, OP.i32_store8, 0x00, 0x00); break;
        case '-': get(); get(); code.push(OP.i32_load8_u, 0x00, 0x00, OP.i32_const, ...sleb(1), OP.i32_sub, OP.i32_store8, 0x00, 0x00); break;
        case '.': get(); code.push(OP.i32_load8_u, 0x00, 0x00, OP.call, 0x00); break; // env.out
        case ',': get(); code.push(OP.call, 0x01, OP.i32_store8, 0x00, 0x00); break;   // env.in
        case '[': code.push(OP.block, 0x40, OP.loop, 0x40); get(); code.push(OP.i32_load8_u, 0x00, 0x00, OP.i32_eqz, OP.br_if, ...uleb(1)); break;
        case ']': code.push(OP.br, ...uleb(0), OP.end, OP.end); break;
        default: break; // tout caractère non-BF est un commentaire
      }
    }
    const body = [1, 1, 0x7f, ...code, OP.end];          // 1 groupe de locaux: 1 x i32, puis code, puis end
    const codeSec = [0x01, ...uleb(body.length), ...body]; // 1 fonction
    const types = [0x03, 0x60, 1, 0x7f, 0, 0x60, 0, 1, 0x7f, 0x60, 0, 0]; // (i32)->() , ()->i32 , ()->()
    const imports = [0x02, ...nm('env'), ...nm('out'), 0x00, 0x00, ...nm('env'), ...nm('in'), 0x00, 0x01];
    const funcs = [0x01, 0x02];        // 1 fonction de type #2
    const mems = [0x01, 0x00, 0x01];   // 1 mémoire, min 1 page (64 Ko > 30000 cellules BF)
    const exports = [0x02, ...nm('run'), 0x00, 0x02, ...nm('memory'), 0x02, 0x00];
    const mod = [
      0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm + version 1
      ...sec(1, types), ...sec(2, imports), ...sec(3, funcs), ...sec(5, mems), ...sec(7, exports), ...sec(10, codeSec),
    ];
    return Uint8Array.from(mod);
  }

  async function runWasm(bytes, input) {
    input = input || '';
    let out = '', inPtr = 0;
    const { instance } = await WebAssembly.instantiate(bytes, { env: {
      out: (c) => { out += String.fromCharCode(c & 0xff); },
      in: () => (inPtr < input.length ? input.charCodeAt(inPtr++) : 0),
    }});
    instance.exports.run();
    return out;
  }

  function interpBF(src, input) {
    input = input || '';
    const mem = new Uint8Array(30000); let p = 0, ip = 0, inPtr = 0, out = '', steps = 0;
    const jumps = {}, stack = [];
    for (let i = 0; i < src.length; i++) { if (src[i] === '[') stack.push(i); else if (src[i] === ']') { const j = stack.pop(); jumps[i] = j; jumps[j] = i; } }
    while (ip < src.length) {
      if (++steps > 5e7) throw new Error('trop d etapes (boucle infinie ?)');
      switch (src[ip]) {
        case '>': p++; break; case '<': p--; break;
        case '+': mem[p] = (mem[p] + 1) & 0xff; break; case '-': mem[p] = (mem[p] - 1) & 0xff; break;
        case '.': out += String.fromCharCode(mem[p]); break;
        case ',': mem[p] = inPtr < input.length ? input.charCodeAt(inPtr++) : 0; break;
        case '[': if (mem[p] === 0) ip = jumps[ip]; break;
        case ']': if (mem[p] !== 0) ip = jumps[ip]; break;
      }
      ip++;
    }
    return out;
  }

  const api = { compileBF, runWasm, interpBF };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.BFWasm = api;
})(typeof self !== 'undefined' ? self : this);
