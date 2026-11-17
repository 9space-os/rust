// SPDX-License-Identifier: MIT
//
// The windows of a program: the process's own (its namespace's /dev, as rio
// gives one) and more made through wsys(4)'s clone file, each a directory of
// files (/dev/wsys/n under wsys): a second main window, a dialog (modal or
// not) or a popup (a menu, a tooltip) of another window. Such a window lives
// while its clone file is open (Window's drop closes it); it comes on the
// screen with its first present or top (show). rio has no clone file: there
// a program has its one window (create fails).

use crate::sys::{Fd, ORDWR, OREAD, OTRUNC, OWRITE};
use std::io;

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

/// a window: its number in wsys (0: unknown, rio's), its directory, and for
/// one made by create the clone file that keeps it
pub struct Window {
    id: u32,
    dir: String,
    ctl: Option<Fd>,
}

impl Window {
    /// the process's window: its namespace's /dev
    pub fn process() -> Window {
        let id = std::fs::read_to_string("/dev/winid").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        Window { id, dir: "/dev".into(), ctl: None }
    }

    /// window n of the process's wsys as another window's parent (0: the
    /// process's window); it does not hold the window
    pub fn of(n: u32) -> Window {
        if n == 0 {
            return Window::process();
        }
        Window { id: n, dir: format!("/dev/wsys/{n}"), ctl: None }
    }

    /// a window of the process's wsys: a dialog or popup of parent, or a
    /// main window of its own; off the screen until show (or a present)
    pub fn create(parent: Option<&Window>, role: Role) -> io::Result<Window> {
        let clone = match parent {
            Some(p) => format!("{}/clone", p.dir),
            None => "/dev/clone".to_string(),
        };
        let ctl = Fd::open(&clone, ORDWR)?;
        // the ctl's first field: the window's number
        let mut b = [0u8; 256];
        let n = ctl.pread(&mut b, 0)?;
        let id: u32 = String::from_utf8_lossy(&b[..n])
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("wsys: clone: no window number"))?;
        let w = Window { id, dir: format!("/dev/wsys/{id}"), ctl: Some(ctl) };
        match role {
            // /dev/clone makes a dialog of the process's window: none here
            Role::Toplevel => w.ctl("parent -")?,
            Role::Dialog { modal } => {
                if parent.is_none() {
                    w.ctl("parent -")?;
                }
                if modal {
                    w.ctl("modal")?;
                }
            }
            Role::Popup { grab } => w.ctl(if grab { "popup grab" } else { "popup" })?,
        }
        Ok(w)
    }

    /// its number in wsys (winid), 0 if there is none
    pub fn id(&self) -> u32 {
        self.id
    }

    /// its directory: /dev, or /dev/wsys/n
    pub fn dir(&self) -> &str {
        &self.dir
    }

    /// whether it is the process's window
    pub fn is_process(&self) -> bool {
        self.dir == "/dev"
    }

    /// file f of the window (mouse, kbd, winname, ...)
    pub fn path(&self, f: &str) -> String {
        format!("{}/{f}", self.dir)
    }

    /// a message to its ctl (wsys(4): rect, top, label, popup, parent, modal,
    /// askclose, delete, ...)
    pub fn ctl(&self, msg: &str) -> io::Result<()> {
        match &self.ctl {
            Some(fd) => fd.write1(msg.as_bytes()),
            None => Fd::open(&self.path("ctl"), OWRITE)?.write1(msg.as_bytes()),
        }
    }

    /// on the screen with the focus (a window drawn through draw(3): it has no
    /// present that would bring it)
    pub fn show(&self) -> io::Result<()> {
        self.ctl("top")
    }

    /// its frame on the screen (border and all, as wctl says)
    pub fn set_rect(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> io::Result<()> {
        self.ctl(&format!("rect {x0} {y0} {x1} {y1}"))
    }

    /// the user's Delete asks (the event close) instead of deleting it
    pub fn ask_close(&self, on: bool) -> io::Result<()> {
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
        Fd::open(&self.path("label"), OWRITE | OTRUNC)?.write1(s.as_bytes())
    }

    /// its dots per inch (wsys' dpi file), 96 without one
    pub fn dpi(&self) -> i32 {
        std::fs::read_to_string(self.path("dpi")).ok().and_then(|s| s.trim().parse().ok()).filter(|&d: &i32| d > 0).unwrap_or(96)
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
