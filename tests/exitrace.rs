// exitrace: the program ends (process::exit from main) while its threads allocate, free
// and finish. std's end_program (sys/pal/plan9/proc.rs) kills the other threads one by
// one; a thread killed inside malloc dies holding its lock. A thread that was finishing
// had taken itself off the list of threads before its last frees (free_thread_data), was
// not killed, and slept on that lock for ever: servoshot's procs stayed after its exit
// (browser/NOTES.md, 2026-11-21). One run is a few milliseconds; tests/run.sh runs it
// many times and then looks for procs of it that are left.
// usage: exitrace [MS]   (main ends after MS ms; default: 1 to 8 by the pid)
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{env, process, thread};

static STOP: AtomicBool = AtomicBool::new(false);

// allocations of mixed sizes, kept a while and freed: malloc's lock taken often
fn churn() {
    let mut keep: Vec<Vec<u8>> = Vec::new();
    let mut n = 0usize;
    while !STOP.load(Ordering::Relaxed) {
        keep.push(vec![n as u8; 16 + (n * 37) % 4000]);
        if keep.len() > 64 {
            keep.drain(..32);
        }
        n = n.wrapping_add(1);
        black_box(&keep);
    }
}

fn main() {
    let ms = env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1 + process::id() as u64 % 8);
    for _ in 0..4 {
        thread::spawn(churn);
    }
    // threads that finish all the time: each ends through free_thread_data
    thread::spawn(|| {
        while !STOP.load(Ordering::Relaxed) {
            for _ in 0..4 {
                thread::spawn(|| black_box(vec![1u8; 512]).len());
            }
            thread::sleep(Duration::from_micros(200));
        }
    });
    thread::sleep(Duration::from_millis(ms));
    process::exit(0);
}
