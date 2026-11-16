// Rust reference for benchmarks/recursion.lykaon (naive Fibonacci, n = 42).
// Build: rustc -O recursion.rs -o recursion   (or cargo build --release)
// Then place the exe at reference/bin/recursion.exe — lykaon bench looks there.
use std::hint::black_box;

fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    let a = black_box(fib(black_box(n - 1)));
    let b = black_box(fib(black_box(n - 2)));
    a + b
}

fn main() {
    println!("{}", fib(black_box(42)));
}