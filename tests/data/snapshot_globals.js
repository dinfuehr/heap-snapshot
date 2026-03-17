// Run with: d8 --expose-gc --allow-natives-syntax snapshot_globals.js

var globalVar = { kind: 'var', payload: 'vvvvv' };
let globalLet = { kind: 'let', payload: 'lllll' };
const globalConst = { kind: 'const', payload: 'ccccc' };

%TakeHeapSnapshot('globals.heapsnapshot');
