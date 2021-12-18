use near_rust_allocator_proxy::ProxyAllocator;

#[global_allocator]
static ALLOC: ProxyAllocator<tikv_jemallocator::Jemalloc> =
    ProxyAllocator::new(tikv_jemallocator::Jemalloc);
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tracing_subscriber::util::SubscriberInitExt;

fn alloc_1024(c: &mut Criterion) {
    let format = tracing_subscriber::fmt::format()
        .with_level(true) // don't include levels in formatted output
        .with_target(true) // don't include trgets
        .without_time();
    tracing_subscriber::fmt().event_format(format).finish().try_init().ok();
    ALLOC.set_verbose(false).enable_stack_trace(true);
    c.bench_function("alloc_2048", |b| {
        b.iter(|| {
            black_box(Vec::<u8>::with_capacity(1024));
        })
    });
}

fn alloc_32(c: &mut Criterion) {
    let format = tracing_subscriber::fmt::format()
        .with_level(true) // don't include levels in formatted output
        .with_target(true) // don't include trgets
        .without_time();
    let _ = tracing_subscriber::fmt().event_format(format).finish().try_init();
    ALLOC.set_verbose(false).enable_stack_trace(true);
    c.bench_function("alloc_32", |b| {
        b.iter(|| {
            black_box(Vec::<u8>::with_capacity(32));
        })
    });
}

criterion_group!(benches, alloc_32, alloc_1024);
criterion_main!(benches);
/*
alloc_32                time:   [38.494 ns 38.525 ns 38.557 ns]
alloc_1024              time:   [2.0461 us 2.0477 us 2.0494 us]
 */
