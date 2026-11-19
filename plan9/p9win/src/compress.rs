// SPDX-License-Identifier: MIT
//
// image(6)'s compressed form, as libdraw's writeimage writes it (9front,
// MIT): the bytes of a block of lines as runs of literals (a byte
// 128+n-1, then n bytes, n up to 128) and matches of 3 to 34 bytes with
// what came up to 1024 bytes before in the block (two bytes: (n-3)<<2 |
// offset>>8, offset&255, the offset counted from 0 for the byte just
// before). A match ends within its line; the decoder (draw(3)'s 'Y',
// memdraw's cloadmemimage) knows no lines, only the block.

const NMATCH: usize = 3;
const NRUN: usize = NMATCH + 31;
const NMEM: usize = 1024;
const NDUMP: usize = 128;
const NHASH: usize = 1 << 12;
const CHAIN: usize = 32; // candidates looked at for a match

fn hash(b: &[u8]) -> usize {
    ((b[0] as usize) << 8 ^ (b[1] as usize) << 4 ^ b[2] as usize) & (NHASH - 1)
}

/// as many whole lines of bpl bytes from data as fit in nout bytes,
/// compressed, onto out (cleared first): the lines (0: not even one fits)
pub fn lines(data: &[u8], bpl: usize, out: &mut Vec<u8>, nout: usize) -> usize {
    out.clear();
    if bpl == 0 {
        return 0;
    }
    let nline = data.len() / bpl;
    let mut head = vec![usize::MAX; NHASH];
    let mut prev = vec![usize::MAX; NMEM];
    let mut dump: Vec<u8> = Vec::with_capacity(NDUMP);
    let mut done = 0;
    let mut kept = 0; // out's length after the last whole line
    let flush = |out: &mut Vec<u8>, dump: &mut Vec<u8>| {
        if !dump.is_empty() {
            out.push((dump.len() - 1 + 128) as u8);
            out.extend_from_slice(dump);
            dump.clear();
        }
    };
    let add = |head: &mut Vec<usize>, prev: &mut Vec<usize>, p: usize| {
        if p + NMATCH <= data.len() {
            let h = hash(&data[p..]);
            prev[p % NMEM] = head[h];
            head[h] = p;
        }
    };
    for l in 0..nline {
        let (line, eline) = (l * bpl, (l + 1) * bpl);
        let mut p = line;
        while p < eline {
            let es = eline.min(p + NRUN);
            let (mut best, mut at) = (0, 0);
            if p + NMATCH <= es {
                let mut c = head[hash(&data[p..])];
                let mut n = 0;
                while c != usize::MAX && p - c <= NMEM && n < CHAIN {
                    let mut k = 0;
                    while p + k < es && data[c + k] == data[p + k] {
                        k += 1;
                    }
                    if k > best {
                        (best, at) = (k, c);
                        if p + k == es {
                            break;
                        }
                    }
                    let next = prev[c % NMEM];
                    // the ring's entry is another position's once it wrapped
                    if next == usize::MAX || next >= c {
                        break;
                    }
                    c = next;
                    n += 1;
                }
            }
            if best < NMATCH {
                if dump.len() == NDUMP {
                    flush(out, &mut dump);
                }
                dump.push(data[p]);
                add(&mut head, &mut prev, p);
                p += 1;
            } else {
                flush(out, &mut dump);
                let offs = p - at - 1;
                out.push((((best - NMATCH) << 2) | (offs >> 8)) as u8);
                out.push((offs & 255) as u8);
                for q in p..p + best {
                    add(&mut head, &mut prev, q);
                }
                p += best;
            }
        }
        flush(out, &mut dump);
        if out.len() > nout {
            out.truncate(kept);
            break;
        }
        kept = out.len();
        done += 1;
    }
    done
}

/// image(6)'s decoder, for the tests: the bytes of the block
#[cfg(test)]
pub fn expand(c: &[u8]) -> Vec<u8> {
    let mut o: Vec<u8> = vec![];
    let mut i = 0;
    while i < c.len() {
        let b = c[i] as usize;
        i += 1;
        if b >= 128 {
            let n = b - 128 + 1;
            o.extend_from_slice(&c[i..i + n]);
            i += n;
        } else {
            let n = (b >> 2) + NMATCH;
            let offs = ((b & 3) << 8 | c[i] as usize) + 1;
            i += 1;
            let s = o.len() - offs;
            for k in 0..n {
                o.push(o[s + k]);
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        // a window's pixels: flat colours, a gradient, noise
        let (w, h) = (300usize, 40usize);
        let mut px = vec![0u8; w * h * 4];
        let mut seed = 1u32;
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                let v = match y % 3 {
                    0 => [0xee, 0xee, 0xff, 0],
                    1 => [x as u8, y as u8, 7, 0],
                    _ => {
                        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                        (seed >> 8).to_le_bytes()
                    }
                };
                px[o..o + 4].copy_from_slice(&v);
            }
        }
        let bpl = w * 4;
        let mut out = vec![];
        let mut at = 0;
        let mut back = vec![];
        while at < h {
            let n = lines(&px[at * bpl..], bpl, &mut out, 6000);
            assert!(n > 0);
            assert!(out.len() <= 6000);
            back.extend(expand(&out));
            at += n;
        }
        assert_eq!(back, px);
    }
}
