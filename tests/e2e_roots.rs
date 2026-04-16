mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_roots(file: &str) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let output = heap_snapshot_bin()
        .arg("roots")
        .arg(&path)
        .output()
        .expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn roots_heap1() {
    let output = run_roots("heap-1.heapsnapshot");
    let expected = "\
System roots (4):
  @3 (GC roots)  (self: 0 B, retained: 125 kB, 28 children)
    @5 (Bootstrapper)  (self: 0 B, retained: 0 B, 0 children)
    @7 (Builtins)  (self: 0 B, retained: 0 B, 2426 children)
    @9 (Client heap)  (self: 0 B, retained: 0 B, 0 children)
    @11 (Code flusher)  (self: 0 B, retained: 0 B, 0 children)
    @13 (Compilation cache)  (self: 0 B, retained: 0 B, 5 children)
    @15 (Debugger)  (self: 0 B, retained: 0 B, 0 children)
    @17 (Extensions)  (self: 0 B, retained: 0 B, 1 children)
    @19 (Eternal handles)  (self: 0 B, retained: 0 B, 0 children)
    @21 (External strings)  (self: 0 B, retained: 0 B, 0 children)
    @23 (Global handles)  (self: 0 B, retained: 0 B, 4 children)
    @25 (Handle scope)  (self: 0 B, retained: 0 B, 226 children)
    @27 (Identity map)  (self: 0 B, retained: 0 B, 0 children)
    @29 (Micro tasks)  (self: 0 B, retained: 0 B, 0 children)
    @31 (Read-only roots)  (self: 0 B, retained: 0 B, 1049 children)
    @33 (Relocatable)  (self: 0 B, retained: 0 B, 0 children)
    @35 (Retain maps)  (self: 0 B, retained: 0 B, 0 children)
    @37 (Shareable object cache)  (self: 0 B, retained: 0 B, 1 children)
    @39 (SharedStruct type registry)  (self: 0 B, retained: 0 B, 0 children)
    @41 (Smi roots)  (self: 0 B, retained: 0 B, 0 children)
    @43 (Stack roots)  (self: 0 B, retained: 28 B, 21 children)
    @45 (Startup object cache)  (self: 0 B, retained: 96 B, 64 children)
    @47 (Internalized strings)  (self: 0 B, retained: 0 B, 1769 children)
    @49 (Strong root list)  (self: 0 B, retained: 7 kB, 99 children)
    @51 (Strong roots)  (self: 0 B, retained: 0 B, 0 children)
    @53 (Thread manager)  (self: 0 B, retained: 0 B, 0 children)
    @55 (Traced handles)  (self: 0 B, retained: 0 B, 0 children)
    @57 (Weak roots)  (self: 0 B, retained: 0 B, 0 children)
    @59 (Write barrier)  (self: 0 B, retained: 0 B, 0 children)
  @2 C++ Persistent roots  (self: 0 B, retained: 0 B, 0 children)
  @4 C++ CrossThreadPersistent roots  (self: 0 B, retained: 0 B, 0 children)
  @6 C++ native stack roots  (self: 0 B, retained: 0 B, 0 children)

User roots (0):
";
    assert_eq!(output, expected);
}
