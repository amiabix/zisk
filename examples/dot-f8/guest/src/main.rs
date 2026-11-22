//! Calls `dot_f8` with every format pair, block sums, empty calls, negative totals and
//! `DOT_F8_REPEAT_Y`, checks each result against a plain decode loop and commits them.

#![no_main]

ziskos::entrypoint!(main);

use ziskos::syscalls::{
    DOT_F8_DECODE, DOT_F8_E2M1, DOT_F8_E4M3, DOT_F8_E4M3_FNUZ, DOT_F8_I8, DOT_F8_REPEAT_Y,
    SyscallDotF8Params, dot_f8_bits, dot_f8_formats, syscall_dot_f8,
};

fn value(fmt: u8, bytes: &[u8], i: usize) -> i64 {
    let code = if fmt == DOT_F8_E2M1 { bytes[i / 2] >> (4 * (i % 2)) & 15 } else { bytes[i] };
    DOT_F8_DECODE[fmt as usize][code as usize] as i64
}

fn words(seed: u64, n: usize) -> Vec<u64> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            s
        })
        .collect()
}

fn bytes(w: &[u64]) -> Vec<u8> {
    w.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn check(fx: u8, x: &[u64], fy: u8, y: &[u64], n: usize, block: usize, with_out: bool) -> i64 {
    check_with(fx, x, fy, y, n, block, with_out, false)
}

/// With `repeat_y`, y holds one block, multiplied by every block of x.
#[allow(clippy::too_many_arguments)]
fn check_with(
    fx: u8,
    x: &[u64],
    fy: u8,
    y: &[u64],
    n: usize,
    block: usize,
    with_out: bool,
    repeat_y: bool,
) -> i64 {
    let blocks = if n == 0 { 0 } else { n / if block == 0 { n } else { block } };
    let mut out = vec![0i64; blocks.max(1)];
    let params = SyscallDotF8Params {
        x: x.as_ptr().cast(),
        y: y.as_ptr().cast(),
        n: n as u64,
        formats: dot_f8_formats(fx, fy) | if repeat_y { DOT_F8_REPEAT_Y } else { 0 },
        block: block as u64,
        out: if with_out { out.as_mut_ptr() } else { core::ptr::null_mut() },
    };
    // SAFETY: x and y hold n values of their formats, out holds the block sums.
    let total = unsafe { syscall_dot_f8(&params) };
    let (xb, yb) = (bytes(x), bytes(y));
    let size = if block == 0 { n } else { block };
    let mut sum = 0;
    for b in 0..blocks {
        let y0 = if repeat_y { 0 } else { b * size };
        let s: i64 = (0..size).map(|i| value(fx, &xb, b * size + i) * value(fy, &yb, y0 + i)).sum();
        if with_out {
            assert_eq!(out[b], s, "block sum");
        }
        sum += s;
    }
    assert_eq!(total, sum, "total");
    total
}

fn main() {
    let fmts = [DOT_F8_I8, DOT_F8_E4M3, DOT_F8_E4M3_FNUZ, DOT_F8_E2M1];
    let mut results = Vec::new();
    let mut seed = 1;
    for &fx in &fmts {
        for &fy in &fmts {
            let n = 64;
            let x = words(seed, n * dot_f8_bits(fx) as usize / 64);
            let y = words(seed + 7, n * dot_f8_bits(fy) as usize / 64);
            results.push(check(fx, &x, fy, &y, n, 16, true));
            results.push(check(fx, &x, fy, &y, n, 0, false));
            seed += 1;
        }
    }
    for &fx in &fmts {
        for &fy in &fmts {
            let (rows, cols) = (8, 32);
            let x = words(seed, rows * cols * dot_f8_bits(fx) as usize / 64);
            let y = words(seed + 7, cols * dot_f8_bits(fy) as usize / 64);
            results.push(check_with(fx, &x, fy, &y, rows * cols, cols, true, true));
            results.push(check_with(fx, &x, fy, &y, rows * cols, cols, false, true));
            seed += 1;
        }
    }
    // One block: the same as a plain call.
    let (x, y) = (words(90, 8), words(91, 8));
    assert_eq!(
        check_with(DOT_F8_E4M3, &x, DOT_F8_E4M3, &y, 64, 64, true, true),
        check(DOT_F8_E4M3, &x, DOT_F8_E4M3, &y, 64, 64, true)
    );
    results.push(check_with(DOT_F8_I8, &[], DOT_F8_I8, &[], 0, 16, true, true));
    // Largest E4M3 magnitudes, -448 * 448 each: a negative total.
    let neg = [0xfefe_fefe_fefe_fefe_u64; 4];
    let pos = [0x7e7e_7e7e_7e7e_7e7e_u64; 4];
    results.push(check(DOT_F8_E4M3, &neg, DOT_F8_E4M3, &pos, 32, 8, true));
    results.push(check(DOT_F8_I8, &[], DOT_F8_I8, &[], 0, 0, true));
    assert!(results.iter().any(|&v| v < 0));
    let (sum, min) = (results.iter().sum::<i64>(), *results.iter().min().unwrap());
    ziskos::io::commit(&(results.len() as u32, sum, min));
    println!("dot_f8 calls: {}", results.len());
}
