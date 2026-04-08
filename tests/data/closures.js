// d8 --allow-natives-syntax closures.js
function makeCounter() {
  let count = 0;
  return function counter() { return ++count; };
}
const counter = makeCounter();
counter();

function makeGreeter(greeting, punctuation) {
  const prefix = '[greeter]';
  return function greet(name) {
    return prefix + ' ' + greeting + ', ' + name + punctuation;
  };
}
const greeter = makeGreeter('Hello', '!');

function makeNested() {
  const shared = { tag: 'shared-data' };
  function outer() {
    const innerOnly = 'inner-value';
    return function inner() {
      return shared.tag + ' ' + innerOnly;
    };
  }
  return outer();
}
const nested = makeNested();

const fns = [];
for (let i = 0; i < 3; i++) {
  fns.push(function loopClosure() { return i; });
}

const secret = (function() {
  const hidden = 'super-secret-value';
  return function getSecret() { return hidden; };
})();

class Emitter {
  constructor(name) {
    this.name = name;
    this.listeners = [];
  }
  addListener() {
    const listener = () => this.name;
    this.listeners.push(listener);
  }
}
const emitter = new Emitter('my-emitter');
emitter.addListener();

globalThis.counter = counter;
globalThis.greeter = greeter;
globalThis.nested = nested;
globalThis.fns = fns;
globalThis.secret = secret;
globalThis.emitter = emitter;

%TakeHeapSnapshot('closures.heapsnapshot');
