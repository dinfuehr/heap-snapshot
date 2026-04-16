use super::*;

#[test]
fn get_summary() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(2, "get_summary", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    assert_eq!(
        text,
        r#"Constructor                                           Count   Shallow size  Retained size
Function                                                843          24212          60808
(object shape)                                          926          48544          49456
system / NativeContext                                    1           1240          23708
(array)                                                  13          14792          18276
{constructor}                                            71           1988          13556
system / FunctionTemplateInfo                            96           6144           6796
(string)                                                137           4516           4516
system / PropertyArray                                  119           4388           4388
Object                                                    7            192           3512
system / Context                                          2             40           2276
system / PropertyCell                                   110           2200           2224
TypedArray                                               12            336           2196
system / ObjectTemplateInfo                              27            756           2088
Array                                                     2             32           2072
Error                                                    12            336           1968
Math                                                      1             28           1860
InitialObject [script_id=3:L3:23]                         3            180           1696
String                                                    1             16           1612
system / DoubleStringCache                                1           1544           1544
(concatenated string)                                    66           1320           1432
system / ArrayList                                       19           1352           1380
DataView                                                  1             28           1376
system / AccessorPair                                   101           1212           1340
console                                                   1             12           1248
{file, log, dom, test, promise, debugger, serializer, wasm}        2             56           1208
global                                                    2             32           1168
Intl.Locale                                               1             28           1168
Intl                                                      1             28           1048
system / FunctionTemplateRareData                        15            600            852
system / WeakFixedArray                                   2            784            784
Atomics                                                   1             28            764
AsyncDisposableStack                                      1             28            760
DisposableStack                                           1             28            760
(shared function info)                                   10            480            756
system / BytecodeArray                                    2            480            744
Reflect                                                   1             28            620
ArrayBuffer                                               1             28            540
WebAssembly                                               1             12            440
EventTarget                                               4            112            424
Set                                                       1             28            404
SharedArrayBuffer                                         1             28            388
Intl.NumberFormat                                         1             28            384
Intl.DateTimeFormat                                       1             28            384
Map                                                       1             28            376
Number                                                    1             16            372
Symbol                                                    1             28            344
InitialObject                                             1             48            320
Intl.PluralRules                                          1             28            292
Intl.RelativeTimeFormat                                   1             28            292
Intl.ListFormat                                           1             28            292
Intl.DurationFormat                                       1             28            292
BigInt                                                    1             28            292
JSON                                                      1             28            280
[JSGlobalObject]                                          1             24            276
Promise                                                   1             28            264
Generator                                                 1             28            252
AsyncGenerator                                            1             28            252
(constant pool)                                           3            184            248
Intl.Collator                                             1             28            244
WeakMap                                                   1             28            240
FinalizationRegistry                                      1             28            232
Intl.DisplayNames                                         1             28            232
Intl.Segmenter                                            1             28            232
(number)                                                 19            228            228
grow                                                      2             96            224
Async-from-Sync Iterator                                  1             28            220
{current, owner, global, create, createAllowCrossRealmAccess}        3             36            192
WeakRef                                                   1             28            192
WebAssembly.Table                                         1             28            192
WebAssembly.Memory                                        1             28            192
Boolean                                                   1             16            188
Module                                                    1             48            180
Instance                                                  1             48            180
Table                                                     1             48            180
Memory                                                    1             48            180
Global                                                    1             48            180
Tag                                                       1             48            180
Exception                                                 1             48            180
Array Iterator                                            1             28            180
Iterator Helper                                           1             28            180
Map Iterator                                              1             28            180
Set Iterator                                              1             28            180
String Iterator                                           1             28            180
{chdir, setenv, unsetenv, umask, mkdirp, rmdir, name, d8Path}        3             84            180
Segmenter String Iterator                                 1             28            180
RegExp String Iterator                                    1             28            180
Suspending                                                1             48            180
(code)                                                    2            152            168
AsyncGeneratorFunction                                    1             28            164
GeneratorFunction                                         1             28            164
AsyncFunction                                             1             28            152
WebAssembly.Global                                        1             28            148
WebAssembly.Suspending                                    1             28            148
get disposed                                              2             96            144
WebAssembly.Instance                                      1             28            136
WebAssembly.Exception                                     1             28            136
compile                                                   1             48            112
validate                                                  1             48            112
instantiate                                               1             48            112
imports                                                   1             48            112
exports                                                   1             48            112
customSections                                            1             48            112
get exports                                               1             48            112
get length                                                1             48            112
get                                                       1             48            112
set                                                       1             48            112
get buffer                                                1             48            112
get value                                                 1             48            112
set value                                                 1             48            112
valueOf                                                   1             48            112
getArg                                                    1             48            112
is                                                        1             48            112
WebAssembly.Module                                        1             28            112
WebAssembly.Tag                                           1             28            112
promising                                                 1             48            112
toFixedLengthBuffer                                       1             48            112
toResizableBuffer                                         1             48            112
WeakSet                                                   1             28            104
system / ScriptContextTable                               1             80            100
snapshot_diffs.js                                         1             76             96
read                                                      2             96             96
quit                                                      2             96             96
dispose                                                   2             96             96
terminate                                                 2             96             96
use                                                       2             96             96
adopt                                                     2             96             96
defer                                                     2             96             96
move                                                      2             96             96
getOrInsert                                               2             96             96
getOrInsertComputed                                       2             96             96
system / ScopeInfo                                        2             84             84
{getAndStop}                                              3             84             84
{EventTarget, Div}                                        3             84             84
{now, mark, measure, measureMemory}                       3             84             84
{setOnProfileEndListener, triggerSample}                  3             84             84
{maxFixedArrayCapacity, maxFastArrayLength}               3             84             84
{setHooks}                                                3             84             84
{enable, disable}                                         3             84             84
{serialize, deserialize}                                  3             84             84
{serializeModule, deserializeModule}                      3             84             84
{read, execute}                                           3             84             84
{verifySourcePositions, installConditionalFeatures}        3             84             84
{createdAt}                                               3             48             84
system / FeedbackMetadata                                 2             80             80
system / AccessorInfo                                     3             72             72
NewObject                                                 1             48             68
system / ObjectBoilerplateDescription                     2             64             64
[JSGlobalProxy]                                           1             16             56
system / FeedbackCell                                     3             48             48
console                                                   1             48             48
Arguments                                                 1             48             48
Array Iterator                                            1             48             48
StringIterator                                            1             48             48
Segments                                                  1             48             48
Segment Iterator                                          1             48             48
MapIterator                                               1             48             48
RegExpStringIterator                                      1             48             48
SetIterator                                               1             48             48
WebAssembly                                               1             48             48
version                                                   1             48             48
print                                                     1             48             48
printErr                                                  1             48             48
write                                                     1             48             48
writeFile                                                 1             48             48
readbuffer                                                1             48             48
readline                                                  1             48             48
load                                                      1             48             48
setTimeout                                                1             48             48
current                                                   1             48             48
owner                                                     1             48             48
global                                                    1             48             48
create                                                    1             48             48
createAllowCrossRealmAccess                               1             48             48
navigate                                                  1             48             48
navigateSameOrigin                                        1             48             48
detachGlobal                                              1             48             48
switch                                                    1             48             48
eval                                                      1             48             48
now                                                       1             48             48
mark                                                      1             48             48
measure                                                   1             48             48
measureMemory                                             1             48             48
Worker                                                    1             48             48
terminateAndWait                                          1             48             48
postMessage                                               1             48             48
getMessage                                                1             48             48
chdir                                                     1             48             48
setenv                                                    1             48             48
unsetenv                                                  1             48             48
umask                                                     1             48             48
mkdirp                                                    1             48             48
rmdir                                                     1             48             48
execute                                                   1             48             48
getAndStop                                                1             48             48
EventTarget                                               1             48             48
Div                                                       1             48             48
verifySourcePositions                                     1             48             48
installConditionalFeatures                                1             48             48
setFlushDenormals                                         1             48             48
setHooks                                                  1             48             48
enable                                                    1             48             48
disable                                                   1             48             48
serialize                                                 1             48             48
deserialize                                               1             48             48
serializeModule                                           1             48             48
deserializeModule                                         1             48             48
setOnProfileEndListener                                   1             48             48
triggerSample                                             1             48             48
getContinuationPreservedEmbedderDataViaAPIForTesting        1             48             48
terminateNow                                              1             48             48
getExtrasBindingObject                                    1             48             48
try                                                       1             48             48
pause                                                     1             48             48
isError                                                   1             48             48
escape                                                    1             48             48
SuppressedError                                           1             48             48
DisposableStack                                           1             48             48
AsyncDisposableStack                                      1             48             48
disposeAsync                                              1             48             48
[Symbol.dispose]                                          1             48             48
[Symbol.asyncDispose]                                     1             48             48
f16round                                                  1             48             48
getFloat16                                                1             48             48
setFloat16                                                1             48             48
Float16Array                                              1             48             48
fromBase64                                                1             48             48
fromHex                                                   1             48             48
toBase64                                                  1             48             48
setFromBase64                                             1             48             48
toHex                                                     1             48             48
setFromHex                                                1             48             48
concat                                                    1             48             48
SuspendError                                              1             48             48
system / RegExpMatchInfo                                  1             44             44
system / WeakFixedArray                                   2             36             36
Tag                                                       1             24             32
system / WasmSuspenderObject                              1             28             28
system / WeakArrayList                                    1             24             24
system / UncompiledDataWithoutPreparseData                1             20             20
(host-defined options)                                    1             16             16
system / ClosureFeedbackCellArray                         1             16             16
system / EmbedderDataArray                                1             16             16
system / BytecodeWrapper                                  2             16             16
system / TrustedByteArray                                 1              8              8
system / TrustedFixedArray                                1              8              8
system / TrustedWeakFixedArray                            1              8              8
system / ProtectedFixedArray                              1              8              8
system / ProtectedWeakFixedArray                          1              8              8
system / Managed<d8::ModuleEmbedderData>                  1              8              8
system / WasmExceptionTag                                 1              8              8
system / Foreign                                          1              8              8"#
    );
}

#[test]
fn get_summary_expand_constructor() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "Function", "limit": 3 }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        "\
Function: 843 objects, 24212 shallow bytes, 60808 retained bytes
Showing 1-3 of 843:
  @16265 Date (self_size: 32, retained_size: 2544)
  @16467 RegExp (self_size: 32, retained_size: 1892)
  @16337 Locale (self_size: 32, retained_size: 1368)"
    );
}

#[test]
fn get_summary_expand_sorted_by_retained_size() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "Function" }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        "\
Function: 843 objects, 24212 shallow bytes, 60808 retained bytes
Showing 1-20 of 843:
  @16265 Date (self_size: 32, retained_size: 2544)
  @16467 RegExp (self_size: 32, retained_size: 1892)
  @16337 Locale (self_size: 32, retained_size: 1368)
  @16449 Object (self_size: 32, retained_size: 1228)
  @16257 CallSite (self_size: 32, retained_size: 1184)
  @16385 DisposableStack (self_size: 32, retained_size: 1008)
  @16387 AsyncDisposableStack (self_size: 32, retained_size: 1008)
  @16447 Number (self_size: 32, retained_size: 900)
  @16551 Symbol (self_size: 32, retained_size: 852)
  @18025 v8BreakIterator (self_size: 32, retained_size: 680)
  @16497 SharedArrayBuffer (self_size: 32, retained_size: 640)
  @16329 DateTimeFormat (self_size: 32, retained_size: 624)
  @16335 NumberFormat (self_size: 32, retained_size: 624)
  @16635 SuppressedError (self_size: 32, retained_size: 624)
  @16647 SuspendError (self_size: 32, retained_size: 624)
  @16589 EvalError (self_size: 32, retained_size: 576)
  @16591 AggregateError (self_size: 32, retained_size: 576)
  @16625 RangeError (self_size: 32, retained_size: 576)
  @16627 ReferenceError (self_size: 32, retained_size: 576)
  @16637 SyntaxError (self_size: 32, retained_size: 576)"
    );
}

#[test]
fn get_summary_expand_invalid_constructor() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "NoSuchConstructor" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No constructor group"),
        "expected not-found error, got: {err}"
    );
}
