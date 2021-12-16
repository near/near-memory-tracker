use near_rust_allocator_proxy::allocator::MyAllocator;

#[global_allocator]
static ALLOC: MyAllocator<tikv_jemallocator::Jemalloc> =
    MyAllocator::new(tikv_jemallocator::Jemalloc);

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn alloc_1024(c: &mut Criterion) {
    c.bench_function("alloc_1024", |b| {
        b.iter(|| {
            black_box(Vec::<u8>::with_capacity(1024));
        })
    });
}

fn alloc_32(c: &mut Criterion) {
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
