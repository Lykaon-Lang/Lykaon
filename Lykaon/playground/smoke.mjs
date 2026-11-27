import fs from 'fs';

const bytes = fs.readFileSync('target/wasm32-unknown-unknown/release/lykaon_playground.wasm');
const { instance } = await WebAssembly.instantiate(bytes, {});
const ex = instance.exports;
console.log('exports:', Object.keys(ex).filter((k) => k.startsWith('lykaon') || k === 'memory'));
const enc = new TextEncoder();
const cases = [
  ['hello', 'Say Hello World.'],
  ['sum', 'Changeable total is 0.\nFor each i from 1 to 10.\n    Set total to total plus i.\nSay total.'],
  ['factorial', 'Say 25 factorial.'],
  ['record', 'A color is red or green or blue.\nShade is green.\nLook at shade.\n    When red.\n        Say warm.\n    When green.\n        Say fresh.\n    When blue.\n        Say cool.'],
  ['fail', 'To halve n can fail.\n    If n is 0 fail with zero cannot be halved.\n    Give back n divided by 2.\nTry halve 0.\n    Say it.\nIf that fails.\n    Say could not halve.'],
  ['task', 'Start task looker.\n    Give back 11.\nWait for looker.\nSay it.'],
  ['error', 'Say "quoted".'],
  ['json', 'Save 42 as json to file n in temporary folder.\nRead json file n in temporary folder as whole number.\nSay it.'],
];
let bad = 0;
for (const [name, src] of cases) {
  const s = enc.encode(src);
  const ptr = ex.lykaon_alloc(s.length);
  new Uint8Array(ex.memory.buffer, ptr, s.length).set(s);
  const packed = ex.lykaon_run(ptr, s.length);
  const outPtr = Number(BigInt(packed) >> 32n);
  const outLen = Number(BigInt(packed) & 0xFFFFFFFFn);
  const text = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, outPtr, outLen));
  console.log('--- ' + name + ' ---');
  console.log(JSON.stringify(text));
  if (name !== 'error' && name !== 'json' && !text.trim()) bad++;
}
process.exit(bad ? 1 : 0);