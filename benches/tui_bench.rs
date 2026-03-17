use criterion::{Criterion, criterion_group, criterion_main};
use heap_snapshot::parser;
use heap_snapshot::snapshot::HeapSnapshot;
use heap_snapshot::tui::bench::BenchApp;
use std::fs::File;

fn load_snapshot() -> HeapSnapshot {
    let file = File::open("tests/data/heap-1.heapsnapshot").unwrap();
    let raw = parser::parse(file).unwrap();
    HeapSnapshot::new(raw)
}

fn load_weakrefs_snapshot() -> HeapSnapshot {
    let file = File::open("tests/data/weakrefs.heapsnapshot").unwrap();
    let raw = parser::parse(file).unwrap();
    HeapSnapshot::new(raw)
}

fn bench_app_new(c: &mut Criterion) {
    let snap = load_snapshot();
    c.bench_function("App::new", |b| {
        b.iter(|| BenchApp::new(&snap));
    });
}

fn bench_rebuild_rows_summary(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_summary(&snap);
    app.rebuild_rows(&snap);

    c.bench_function("rebuild_rows/summary", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_rebuild_rows_containment(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_containment(&snap);
    app.rebuild_rows(&snap);

    c.bench_function("rebuild_rows/containment", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_rebuild_rows_dominators(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_dominators(&snap);
    app.rebuild_rows(&snap);

    c.bench_function("rebuild_rows/dominators", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_summary_expand_groups(c: &mut Criterion) {
    let snap = load_snapshot();

    c.bench_function("summary/expand_first_5_groups", |b| {
        b.iter_batched(
            || {
                let mut app = BenchApp::new(&snap);
                app.set_view_summary(&snap);
                app.rebuild_rows(&snap);
                app
            },
            |mut app| {
                // Expand first 5 summary groups by pressing Enter on each
                for _ in 0..5 {
                    app.key_enter(&snap);
                    app.rebuild_rows(&snap);
                    // Move cursor past the expanded children to the next group
                    let count = app.row_count();
                    let cur = app.cursor();
                    for _ in cur..count {
                        app.key_down(&snap);
                    }
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_summary_filter(c: &mut Criterion) {
    let snap = load_snapshot();

    c.bench_function("summary/filter_rebuild", |b| {
        b.iter_batched(
            || {
                let mut app = BenchApp::new(&snap);
                app.set_view_summary(&snap);
                app
            },
            |mut app| {
                app.set_summary_filter("string");
                app.rebuild_rows(&snap);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_summary_navigate(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_summary(&snap);
    // Expand a few groups first
    app.rebuild_rows(&snap);
    app.key_enter(&snap);
    app.rebuild_rows(&snap);

    c.bench_function("summary/navigate_20_rows", |b| {
        b.iter(|| {
            for _ in 0..20 {
                app.key_down(&snap);
            }
            for _ in 0..20 {
                app.key_up(&snap);
            }
        });
    });
}

fn bench_containment_expand_deep(c: &mut Criterion) {
    let snap = load_snapshot();

    c.bench_function("containment/expand_5_levels", |b| {
        b.iter_batched(
            || {
                let mut app = BenchApp::new(&snap);
                app.set_view_containment(&snap);
                app.rebuild_rows(&snap);
                app
            },
            |mut app| {
                for _ in 0..5 {
                    app.key_enter(&snap);
                    app.rebuild_rows(&snap);
                    app.key_down(&snap);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_paging(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_containment(&snap);
    app.rebuild_rows(&snap);
    // Expand root to get paged children
    app.key_enter(&snap);
    app.rebuild_rows(&snap);

    c.bench_function("containment/page_next_prev", |b| {
        b.iter(|| {
            app.key('n', &snap);
            app.rebuild_rows(&snap);
            app.key('p', &snap);
            app.rebuild_rows(&snap);
        });
    });
}

fn bench_view_switching(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.rebuild_rows(&snap);

    c.bench_function("switch_views", |b| {
        b.iter(|| {
            app.key('2', &snap); // containment
            app.rebuild_rows(&snap);
            app.key('1', &snap); // summary
            app.rebuild_rows(&snap);
            app.key('3', &snap); // dominators
            app.rebuild_rows(&snap);
            app.key('1', &snap); // back to summary
            app.rebuild_rows(&snap);
        });
    });
}

fn bench_flatten_summary_expanded(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_summary(&snap);
    app.rebuild_rows(&snap);
    // Expand first 5 groups
    for _ in 0..5 {
        app.key_enter(&snap);
        app.rebuild_rows(&snap);
        let count = app.row_count();
        let cur = app.cursor();
        for _ in cur..count {
            app.key_down(&snap);
        }
    }

    c.bench_function("flatten/summary_5_groups_expanded", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_flatten_summary_filtered(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_summary(&snap);
    app.set_summary_filter("string");
    app.rebuild_rows(&snap);

    c.bench_function("flatten/summary_filtered", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_flatten_containment_expanded(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_containment(&snap);
    app.rebuild_rows(&snap);
    // Expand 5 levels deep
    for _ in 0..5 {
        app.key_enter(&snap);
        app.rebuild_rows(&snap);
        app.key_down(&snap);
    }

    c.bench_function("flatten/containment_5_levels_expanded", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_flatten_dominators_expanded(c: &mut Criterion) {
    let snap = load_snapshot();
    let mut app = BenchApp::new(&snap);
    app.set_view_dominators(&snap);
    app.rebuild_rows(&snap);
    // Expand root + a few children
    app.key_enter(&snap);
    app.rebuild_rows(&snap);
    for _ in 0..3 {
        app.key_down(&snap);
        app.key_enter(&snap);
        app.rebuild_rows(&snap);
    }

    c.bench_function("flatten/dominators_expanded", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_retainers_weaktarget_rebuild(c: &mut Criterion) {
    let snap = load_weakrefs_snapshot();
    // Find the WeakTarget instance by class name
    let ordinal = snap
        .node_for_snapshot_object_id(heap_snapshot::types::NodeId(7207))
        .expect("WeakTarget @7207 should exist in weakrefs.heapsnapshot");

    let mut app = BenchApp::new(&snap);
    app.set_view_retainers_with_plan(ordinal.0, &snap);

    c.bench_function("retainers/weaktarget_rebuild_rows", |b| {
        b.iter(|| app.rebuild_rows(&snap));
    });
}

fn bench_retainers_weaktarget_navigate(c: &mut Criterion) {
    let snap = load_weakrefs_snapshot();
    let ordinal = snap
        .node_for_snapshot_object_id(heap_snapshot::types::NodeId(7207))
        .expect("WeakTarget @7207 should exist in weakrefs.heapsnapshot");

    let mut app = BenchApp::new(&snap);
    app.set_view_retainers_with_plan(ordinal.0, &snap);

    c.bench_function("retainers/weaktarget_navigate_20", |b| {
        b.iter(|| {
            for _ in 0..20 {
                app.key_down(&snap);
            }
            for _ in 0..20 {
                app.key_up(&snap);
            }
        });
    });
}

criterion_group!(
    benches,
    bench_app_new,
    bench_rebuild_rows_summary,
    bench_rebuild_rows_containment,
    bench_rebuild_rows_dominators,
    bench_summary_expand_groups,
    bench_summary_filter,
    bench_summary_navigate,
    bench_containment_expand_deep,
    bench_paging,
    bench_view_switching,
    bench_flatten_summary_expanded,
    bench_flatten_summary_filtered,
    bench_flatten_containment_expanded,
    bench_flatten_dominators_expanded,
    bench_retainers_weaktarget_rebuild,
    bench_retainers_weaktarget_navigate,
);
criterion_main!(benches);
