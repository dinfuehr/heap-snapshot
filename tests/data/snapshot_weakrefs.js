// Run with: d8 --expose-gc --allow-natives-syntax snapshot_weakrefs.js
//
// Creates a snapshot with both strong and weak references:
//   - StrongTarget: held via a normal property (strongly reachable)
//   - WeakTarget: held via both a direct ref and a WeakRef

class StrongTarget {
  constructor(label) {
    this.label = label;
    this.payload = 'strong-' + 'x'.repeat(1000);
  }
}

class WeakTarget {
  constructor(label) {
    this.label = label;
    this.payload = 'weak-' + 'y'.repeat(1000);
  }
}

const strong = new StrongTarget('kept-alive');
const weakObj = new WeakTarget('weakly-held');

const weakRef = new WeakRef(weakObj);
const registry = new FinalizationRegistry((value) => {
  print('Cleaned up: ' + value);
});
registry.register(weakObj, 'weak-target-cleaned');

const holder = {
  strong: strong,
  weakRef: weakRef,
  registry: registry,
};

globalThis.holder = holder;
globalThis.weakObj = weakObj;

%TakeHeapSnapshot('weakrefs.heapsnapshot');
