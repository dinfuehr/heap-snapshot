const obj = {
  t: true,
  f: false,
  n: null,
  u: undefined,
  i: 42,
  d: 12.75,
  s: "hello",
  nested: { a: 1, b: "two" },
};

const arr = [true, false, null, undefined, 42, 12.75, "hello", { a: 1, b: "two" }];

// Keep references alive
globalThis.obj = obj;
globalThis.arr = arr;

%TakeHeapSnapshot("primitives.heapsnapshot");
