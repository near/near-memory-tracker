use near_rust_allocator_proxy::allocator::MyAllocator;

#[global_allocator]
static ALLOC: MyAllocator<tikv_jemallocator::Jemalloc> =
    MyAllocator::new(tikv_jemallocator::Jemalloc);

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn small_alloc_benchmark(c: &mut Criterion) {
    c.bench_function("small_alloc_benchmark", |b| {
        b.iter(|| {
            black_box(Vec::<usize>::with_capacity(128));
        })
    });
}

criterion_group!(benches, small_alloc_benchmark);
criterion_main!(benches);
/*
small_alloc_benchmark   time:   [1.8438 us 1.8477 us 1.8528 us]
 */
