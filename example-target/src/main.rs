use near_rust_allocator_proxy::ProxyAllocator;
use std::thread::sleep;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::util::SubscriberInitExt;

const MB: usize = 1024 * 1024;

#[global_allocator]
static ALLOC: ProxyAllocator<tikv_jemallocator::Jemalloc> =
    ProxyAllocator::new(tikv_jemallocator::Jemalloc);

#[allow(unused)]
pub struct MyData {
    pub xx: usize,
}

#[inline(never)]
fn something() {
    let mut res = Vec::new();
    for x in 0..1_000_000_000 {
        res.push(MyData { xx: x });
    }
    loop {
        res.push(MyData { xx: 123 });
        std::thread::sleep(Duration::from_millis(10000));
        info!(res_len = ?res.len(), expected_size_mb = res.len() * 8/ MB);
    }
}

#[inline(never)]
fn main() {
    let format = tracing_subscriber::fmt::format()
        .with_level(true) // don't include levels in formatted output
        .with_target(true) // don't include targets
        .without_time();
    tracing_subscriber::fmt().event_format(format).with_writer(std::io::stderr).finish().init();
    info!("init");

    ALLOC.set_report_usage_interval(usize::MAX).enable_stack_trace(true).set_verbose(true);
    sleep(Duration::from_millis(50));

    println!("Hello, world!");
    loop {
        something();
    }
}
