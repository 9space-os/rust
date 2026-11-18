// SPDX-License-Identifier: MIT
//
// The windows of a program: the process's own (its namespace's /dev, as rio
// gives one) and more made through wsys(4)'s clone file: a second main
// window, a dialog (modal or not) or a popup (a menu, a tooltip) of another
// window. Such a window lives while its clone file is open (Window's drop
// closes it); it comes on the screen with its first present or top (show).
// rio has no clone file: there a program has its one window (create fails).
//
// A window of the clone file has a number of the program's own (its key,
// from 1) and a directory that stays (/mnt/wsyswin.key: wsys' /dev/wsys/n
// bound there, /mnt being mntgen's). When its wsys is gone the process's
// window comes again in the next one (wsys.rs, the mouse's reader), then
// each such window is made again there as it was (parent, role, modal,
// askclose, its place, its label) and bound at the same directory: its
// readers, draw(3) and Mesa's WSI find it where it was, as the process's
// window is again at /dev. raw-window-handle's Plan9WindowHandle.window is
// the key.

use crate::sys::{self, Fd, MREPL, ORDWR, OREAD, OTRUNC, OWRITE};
use std::io;
use std::sync::{Arc, Mutex};

/// what a window made by create is to its parent
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// a main window of its own (no parent)
    Toplevel,
    /// a dialog of the parent: above it, moving and hidden with it; modal: the
    /// parent takes no clicks while it is shown
    Dialog { modal: bool },
    /// a popup of the parent (a menu, a tooltip): no border; grab: it takes
    /// the focus (a menu), else leaves it where it is (a tooltip)
    Popup { grab: bool },
}

/// a window of the clone file, as it is now and as it is to be made again
struct Made {
    key: u32,
    parent: u32, // its parent's key, 0: the process's window, none: Toplevel
    role: Role,
    id: u32,     // its number in the wsys it is in now
    ctl: Option<Fd>,
    askclose: bool,
    rect: Option<(i32, i32, i32, i32)>, // its contents as last set or seen (ctl's rect)
    label: Option<String>,
    srv: String, // the wsys it is in ($wsys when it was made)
}

/// the windows of the clone file there are, in the order they were made
static MADE: Mutex<Vec<Arc<Mutex<Made>>>> = Mutex::new(Vec::new());
static NEXTKEY: Mutex<u32> = Mutex::new(0);

/// the directory a key's window stays at
pub fn keydir(key: u32) -> String {
    if key == 0 { "/dev".into() } else { format!("/mnt/wsyswin.{key}") }
}

/// a window: the process's (key 0, /dev), or one of the clone file
pub struct Window {
    key: u32,
    dir: String,
    made: Option<Arc<Mutex<Made>>>, // a window of the clone file: what makes it (its owner holds it)
}

fn readid(ctl: &Fd) -> io::Result<u32> {
    let mut b = [0u8; 256];
    let n = ctl.pread(&mut b, 0)?;
    String::from_utf8_lossy(&b[..n])
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::other("wsys: clone: no window number"))
}

/// window m made in the process's wsys (again): its clone file opened from its
/// parent's directory, its role and state written, bound at its key's directory
fn make(m: &mut Made) -> io::Result<()> {
    let clone = if m.role == Role::Toplevel { "/dev/clone".to_string() } else { format!("{}/clone", keydir(m.parent)) };
    let ctl = Fd::open(&clone, ORDWR)?;
    let id = readid(&ctl)?;
    let msg = |s: &str| ctl.write1(s.as_bytes());
    match m.role {
        // /dev/clone makes a dialog of the process's window: none here
        Role::Toplevel => msg("parent -")?,
        Role::Dialog { modal } => {
            if modal {
                msg("modal")?;
            }
        },
        Role::Popup { grab } => msg(if grab { "popup grab" } else { "popup" })?,
    }
    if m.askclose {
        msg("askclose")?;
    }
    if let Some((x0, y0, x1, y1)) = m.rect {
        let _ = msg(&format!("rect {x0} {y0} {x1} {y1}"));
    }
    let dir = keydir(m.key);
    // where the old one was (gone with its wsys) the new one
    let _ = sys::unmount_all(&dir);
    sys::bind(&format!("/dev/wsys/{id}"), &dir, MREPL)?;
    if let Some(l) = &m.label {
        let _ = Fd::open(&format!("{dir}/label"), OWRITE | OTRUNC).and_then(|f| f.write1(l.as_bytes()));
    }
    m.id = id;
    m.ctl = Some(ctl);
    m.srv = std::env::var("wsys").unwrap_or_default().trim_end_matches('\0').to_string();
    Ok(())
}

/// the window at dir is gone with its wsys (not deleted in it): that wsys'
/// process is no more, or its files hung up
pub fn lost_at(dir: &str) -> bool {
    let srv = MADE.lock().unwrap().iter().find_map(|w| {
        let m = w.lock().unwrap();
        (keydir(m.key) == dir).then(|| m.srv.clone())
    });
    let Some(srv) = srv else { return false };
    crate::wsys::gone(&srv) || Fd::open(&format!("{dir}/winname"), OREAD).is_err_and(|e| e.to_string().contains("hungup"))
}

/// the windows of the clone file made again in the process's next wsys (its
/// window there already: wsys::reattach), each once, parents first; by any
/// window's reader that found its wsys gone, the others find them made
pub fn remake() -> io::Result<()> {
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let all: Vec<_> = MADE.lock().unwrap().clone();
    for w in all {
        let mut m = w.lock().unwrap();
        // made already in this wsys (another reader did): its files answer
        if m.ctl.as_ref().is_some_and(|c| readid(c).is_ok()) {
            continue;
        }
        m.ctl = None;
        make(&mut m)?;
    }
    Ok(())
}

/// the place of the window at dir now, kept for the next wsys: its contents
/// as its ctl says (id x0 y0 x1 y1 ...), what the ctl message rect wants
/// (wctl's is the frame: border, title bar)
pub fn seen_at(dir: &str) {
    for w in MADE.lock().unwrap().iter() {
        let mut m = w.lock().unwrap();
        if keydir(m.key) != dir {
            continue;
        }
        let Some(ctl) = &m.ctl else { continue };
        let mut b = [0u8; 256];
        let Ok(n) = ctl.pread(&mut b, 0) else { continue };
        let v: Vec<i32> = String::from_utf8_lossy(&b[..n]).split_whitespace().skip(1).take(4).filter_map(|x| x.parse().ok()).collect();
        if v.len() == 4 {
            m.rect = Some((v[0], v[1], v[2], v[3]));
        }
    }
}

impl Window {
    /// the process's window: its namespace's /dev
    pub fn process() -> Window {
        Window { key: 0, dir: "/dev".into(), made: None }
    }

    /// the window of key n (raw-window-handle's window: 0, the process's) as
    /// another window's parent; it does not hold the window
    pub fn of(n: u32) -> Window {
        Window { key: n, dir: keydir(n), made: None }
    }

    /// a window of the process's wsys: a dialog or popup of parent, or a
    /// main window of its own; off the screen until show (or a present)
    pub fn create(parent: Option<&Window>, role: Role) -> io::Result<Window> {
        let key = {
            let mut k = NEXTKEY.lock().unwrap();
            *k += 1;
            *k
        };
        let mut m = Made {
            key,
            parent: parent.map_or(0, |p| p.key),
            role,
            id: 0,
            ctl: None,
            askclose: false,
            rect: None,
            label: None,
            srv: String::new(),
        };
        if role != Role::Toplevel && parent.is_none() {
            m.parent = 0; // a dialog or popup of the process's window
        }
        make(&mut m)?;
        let made = Arc::new(Mutex::new(m));
        MADE.lock().unwrap().push(made.clone());
        Ok(Window { key, dir: keydir(key), made: Some(made) })
    }

    /// its key (raw-window-handle's window): 0 the process's window
    pub fn key(&self) -> u32 {
        self.key
    }

    /// its number in wsys now (winid), 0 if there is none
    pub fn id(&self) -> u32 {
        match &self.made {
            Some(m) => m.lock().unwrap().id,
            None => std::fs::read_to_string(self.path("winid")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0),
        }
    }

    /// its directory: /dev, or the one its key's window stays at
    pub fn dir(&self) -> &str {
        &self.dir
    }

    /// whether it is the process's window
    pub fn is_process(&self) -> bool {
        self.key == 0
    }

    /// file f of the window (mouse, kbd, winname, ...)
    pub fn path(&self, f: &str) -> String {
        format!("{}/{f}", self.dir)
    }

    /// a message to its ctl (wsys(4): rect, top, label, popup, parent, modal,
    /// askclose, delete, ...)
    pub fn ctl(&self, msg: &str) -> io::Result<()> {
        if let Some(m) = &self.made {
            let m = m.lock().unwrap();
            if let Some(fd) = &m.ctl {
                return fd.write1(msg.as_bytes());
            }
        }
        Fd::open(&self.path("ctl"), OWRITE)?.write1(msg.as_bytes())
    }

    /// on the screen with the focus (a window drawn through draw(3): it has no
    /// present that would bring it)
    pub fn show(&self) -> io::Result<()> {
        self.ctl("top")
    }

    /// its contents on the screen (wsys' ctl rect: the frame, border and
    /// title bar, around them); kept for the next wsys
    pub fn set_rect(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> io::Result<()> {
        if let Some(m) = &self.made {
            m.lock().unwrap().rect = Some((x0, y0, x1, y1));
        }
        self.ctl(&format!("rect {x0} {y0} {x1} {y1}"))
    }

    /// its place now: kept for the next wsys (seen_at)
    pub fn seen(&self) {
        seen_at(&self.dir);
    }

    /// the user's Delete asks (the event close) instead of deleting it
    pub fn ask_close(&self, on: bool) -> io::Result<()> {
        if let Some(m) = &self.made {
            m.lock().unwrap().askclose = on;
        }
        self.ctl(if on { "askclose" } else { "askclose 0" })
    }

    /// a message to its wctl (rio(4)); without a window system nothing
    pub fn wctl(&self, msg: &str) -> io::Result<()> {
        match Fd::open(&self.path("wctl"), OWRITE) {
            Ok(fd) => fd.write1(msg.as_bytes()),
            Err(_) => Ok(()),
        }
    }

    pub fn set_label(&self, s: &str) -> io::Result<()> {
        if let Some(m) = &self.made {
            m.lock().unwrap().label = Some(s.to_string());
        }
        Fd::open(&self.path("label"), OWRITE | OTRUNC)?.write1(s.as_bytes())
    }

    /// its dots per inch (wsys' dpi file), 96 without one
    pub fn dpi(&self) -> i32 {
        std::fs::read_to_string(self.path("dpi")).ok().and_then(|s| s.trim().parse().ok()).filter(|&d: &i32| d > 0).unwrap_or(96)
    }

    /// its contents on the screen, as its ctl says (id x0 y0 x1 y1 ...): what
    /// set_rect sets; none for the process's window (no ctl of its own)
    pub fn contents(&self) -> Option<(i32, i32, i32, i32)> {
        let m = self.made.as_ref()?.lock().unwrap();
        let ctl = m.ctl.as_ref()?;
        let mut b = [0u8; 256];
        let n = ctl.pread(&mut b, 0).ok()?;
        let v: Vec<i32> = String::from_utf8_lossy(&b[..n]).split_whitespace().skip(1).take(4).filter_map(|x| x.parse().ok()).collect();
        (v.len() == 4).then(|| (v[0], v[1], v[2], v[3]))
    }

    /// its place on the screen, as its wctl says (x0 y0 x1 y1, border and all)
    pub fn rect(&self) -> Option<(i32, i32, i32, i32)> {
        let fd = Fd::open(&self.path("wctl"), OREAD).ok()?;
        let mut b = [0u8; 256];
        let n = fd.pread(&mut b, 0).ok()?;
        let s = String::from_utf8_lossy(&b[..n]);
        let v: Vec<i32> = s.split_whitespace().take(4).filter_map(|x| x.parse().ok()).collect();
        (v.len() == 4).then(|| (v[0], v[1], v[2], v[3]))
    }
}

impl Drop for Window {
    /// a window of the clone file goes: out of the ones made again, its clone
    /// file closed (wsys deletes it), its directory let go
    fn drop(&mut self) {
        let Some(made) = self.made.take() else { return };
        MADE.lock().unwrap().retain(|w| !Arc::ptr_eq(w, &made));
        made.lock().unwrap().ctl = None;
        let _ = sys::unmount_all(&self.dir);
    }
}
