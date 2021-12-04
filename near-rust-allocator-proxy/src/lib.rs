#![deny(
    unused_crate_dependencies,
    unused_extern_crates,
    variant_size_differences
)]
pub mod allocator;

/// Used by benches;
#[cfg(test)]
use criterion as _;
#[cfg(test)]
use tikv_jemallocator as _;
