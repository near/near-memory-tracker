Utilities developed to measure memory usage in projects with mixed Rust/C++ code
while having minimizing performance/memory overhead.

# Goals
* Track memory usage/leaks in large Rust projects, where memory leak could
  potencially practially everywhere: for example: application itself, any of hundreds
  imported Rust libraries or even with liked C/C++ code.
* Low performance overhead - existing tools like Valdrid can slow down program
  by a factor of 25-50 times, using such approach would be impractical.
* Low memory overhead - Adds extra 32 bytes per each memory allocation on heap.
  While it's easy to add extra memory to a machine when needed, adding extra CPU cores will not help with applications limited by a single core performance.
  This can be optimized if needed by either reducing header size or by
  doing random sampling for small allocations.
* Ability to dump memory while process is running without affecting it.

# Requirements

* Linux operating system - `dump` script uses linux proc filesystem to read
  process information. This can be extended to other platform if needed in the
  future.

# Design

Rust allocator proxy:
Tracking Rust allocation is done by adding a proxy, which uses jemalloc and
add 32 bytes header to all allocations.
See https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html

C allocator proxy:
Tracking C allocator is done in by overriding dynamic links to malloc/free/etc.
This can be overriden by providing an enviroment variable while running an
executable `LD_PRELOAD=./mtrace.so path'.

`dump` script:
* Reads process memory mapping from `/proc/<PID>/smaps`.
* It's able to identify which pages are present by reading from `/proc/<PID>/pagemap`.
* It reads memory using `/proc/<PID>/pagemap`
* Once memory is read, regions of memory allocated by Rust/C code can be
  identified by looking for `MAGIC` keyword, which is part of the header.

The core tool is in `dump.cpp` file. It dumps the program memory and it prints
memory statistics.

# Modules
* near-rust-allocator-proxy inside `near-rust-allocator-proxy` folder
* near-c-allocator-proxy `near-c-allocator-proxy.c` inside
  `near-dump-analyzer` folder.
* near-dump-analyzer `dump.cpp` inside `near-dump-analyzer` folder

