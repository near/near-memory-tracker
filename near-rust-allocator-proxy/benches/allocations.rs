use near_rust_allocator_proxy::allocator::MyAllocator;

#[global_allocator]
static ALLOC: MyAllocator<tikv_jemallocator::Jemalloc> =
    MyAllocator::new(tikv_jemallocator::Jemalloc);

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn small_alloc_test(n: usize) -> usize {
    let mut x = Vec::<usize>::new();
    x.resize(n, 0);
    let mut sum = 0;
    for val in x.iter() {
        sum += val;
    }
    sum
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("alloc test", |b| b.iter(|| small_alloc_test(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
/*
/// without #[global_allocator]
alloc test              time:   [22.098 ns 22.180 ns 22.259 ns]
/// with #[global_allocator]
alloc test              time:   [55.122 ns 55.707 ns 56.387 ns]
 */
