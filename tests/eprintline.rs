// eprintline: eprintln! lines of several programs on one stderr stay whole. std's Stderr
// is unbuffered and io::Write's write_fmt writes each piece of a format by itself, so two
// programs on one console (wsys, ui, the drivers) cut into each other's lines; plan9's
// Stderr::write_fmt (sys/stdio/plan9.rs) formats a line into one write.
// Without an argument: 4 copies of itself as children, their stderr one pipe, each 2000
// lines of 40 pieces; every line read back must be one child's, whole. "eprintline ok".
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::{env, io};

const KIDS: u32 = 4;
const LINES: u32 = 2000;
const PIECES: u32 = 40;

fn child(id: u32) {
    for n in 0..LINES {
        // 40 pieces: "{id}:{n}:{k} " each, then the count at the end
        eprintln!(
            "line {id} {n} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} end",
            id, n, 0, id, n, 1, id, n, 2, id, n, 3, id, n, 4, id, n, 5, id, n, 6, id, n, 7, id, n,
            8, id, n, 9, id, n, 10, id, n, 11, id, n, 12, PIECES
        );
    }
}

// the line child id would write as its n-th
fn expect(id: u32, n: u32) -> String {
    let v = [
        id, n, 0, id, n, 1, id, n, 2, id, n, 3, id, n, 4, id, n, 5, id, n, 6, id, n, 7, id, n, 8,
        id, n, 9, id, n, 10, id, n, 11, id, n, 12, PIECES,
    ];
    let mut s = format!("line {id} {n}");
    for x in v {
        s.push_str(&format!(" {x}"));
    }
    s.push_str(" end");
    s
}

fn main() -> io::Result<()> {
    let a: Vec<String> = env::args().collect();
    if a.len() > 1 {
        child(a[1].parse().unwrap());
        return Ok(());
    }
    let (r, w) = io::pipe()?;
    let mut kids = Vec::new();
    for id in 0..KIDS {
        kids.push(
            Command::new(&a[0])
                .arg(id.to_string())
                .stderr(Stdio::from(w.try_clone()?))
                .spawn()?,
        );
    }
    drop(w);
    // whole: the line child id writes as its n-th, seen once (a cut line counts once,
    // it does not hold up the child's later lines)
    let mut seen = vec![false; (KIDS * LINES) as usize];
    let mut whole = 0u32;
    let mut bad = 0;
    for l in BufReader::new(r).lines() {
        let l = l?;
        let f: Vec<&str> = l.splitn(4, ' ').collect();
        let ok = f.len() > 3 && f[0] == "line" && {
            match (f[1].parse::<u32>(), f[2].parse::<u32>()) {
                (Ok(id), Ok(n)) if id < KIDS && n < LINES && l == expect(id, n) => {
                    let i = (id * LINES + n) as usize;
                    !std::mem::replace(&mut seen[i], true)
                }
                _ => false,
            }
        };
        if ok {
            whole += 1;
        } else {
            bad += 1;
            if bad <= 3 {
                println!("eprintline: cut line: {:?}", &l[..l.len().min(120)]);
            }
        }
    }
    for mut k in kids {
        k.wait()?;
    }
    if bad == 0 && whole == KIDS * LINES {
        println!("eprintline ok");
    } else {
        println!("eprintline FAIL {bad} cut lines, {whole} of {} whole", KIDS * LINES);
        std::process::exit(1);
    }
    Ok(())
}
