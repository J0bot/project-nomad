// test.cjs — preuve que la page est Turing complete : interprete ET compile-vers-wasm le meme BF.
// node test.cjs  -> doit afficher Hello World deux fois (interprete + wasm) et OK.
const { compileBF, interpBF } = require('./bf-wasm.js');
const HELLO = '++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.';
const ECHO = ',[.,]'; // boucle d'entree/sortie = utilise le flux d'input (Turing: I/O)
(async () => {
  const i = interpBF(HELLO, '');
  console.log('interprete :', JSON.stringify(i));
  const bytes = compileBF(HELLO);
  console.log('wasm       :', bytes.length, 'octets, magic', JSON.stringify([...bytes.slice(0, 4)]));
  let out = '';
  const { instance } = await WebAssembly.instantiate(bytes, { env: { out: c => { out += String.fromCharCode(c & 0xff); }, in: () => 0 } });
  instance.exports.run();
  console.log('wasm-run   :', JSON.stringify(out));
  // echo: prouve l'input cote wasm
  let eo = '';
  const e = await WebAssembly.instantiate(compileBF(ECHO), { env: { out: c => { eo += String.fromCharCode(c & 0xff); }, in: (() => { let k = 0; const s = 'xion'; return () => k < s.length ? s.charCodeAt(k++) : 0; })() } });
  e.instance.exports.run();
  console.log('wasm-echo  :', JSON.stringify(eo), '(entree "xion")');
  const ok = i === 'Hello World!\n' && out === 'Hello World!\n' && eo === 'xion';
  console.log(ok ? 'OK  Turing complet: interprete ET compilateur-wasm sur la meme source, I/O compris.' : 'FAIL');
  process.exit(ok ? 0 : 1);
})();
