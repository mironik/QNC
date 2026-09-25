//! Wrap view of an edited story (v5 `ViewMode::Wrap`).
//!
//! Source and Wrap are two views with two playheads (v5
//! `EditorialPlaybackSessionState`). Source is the clip chosen in the pool with
//! its own timeline and IN/OUT. Wrap is the PROGRAM on one frame axis: the player
//! plays the program, never the source clip of a segment, and the Source view is
//! left as it was.
//!
//! This component owns only the Wrap side: whether the view is on, the program
//! playhead and what the program player must do. The first point on the program
//! opens the program at that frame; every next one only scrubs the same program
//! (v5 `ensure_wrap_or_scrub`, `OpenProgram`, `ScrubFrame`). The playhead goes
//! to the frame at once (v5 `set_wrap_playhead_frame`); program frames the player
//! confirms take over once it reaches the asked frame. It knows no form, no
//! source clip, no database and no player; it never plays, probes or opens media.

pub const MODULE_ID: &str = "qnc.module.wrap-session";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the program player must do, in program frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapRequest {
    /// Open the program with this frame as its first picture.
    Open(u64),
    /// Show this frame of the already open program.
    Scrub(u64),
}

impl WrapRequest {
    /// The program frame asked for.
    pub fn frame(self) -> u64 {
        match self {
            Self::Open(frame) | Self::Scrub(frame) => frame,
        }
    }

    /// Whether the program must be built and opened again.
    pub fn opens(self) -> bool {
        matches!(self, Self::Open(_))
    }
}

#[derive(Debug, Default)]
pub struct WrapSession {
    active: bool,
    playhead: u64,
    /// Program length from the last repaint.
    total_frames: u64,
    request: Option<WrapRequest>,
    /// Frame asked of the player: older confirmed frames do not move the playhead back.
    awaiting: Option<u64>,
}

impl WrapSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the Wrap view is on (else the Source view is).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Program playhead of the Wrap view; kept while the Source view is on (v5).
    pub fn playhead(&self) -> u64 {
        self.playhead
    }

    /// A program frame the user pointed at: the playhead goes there at once, and
    /// the program is opened (entering Wrap) or scrubbed (already in Wrap).
    pub fn seek(&mut self, frame: u64) {
        self.playhead = frame;
        self.awaiting = Some(frame);
        self.request = Some(match self.request {
            // An open still waiting keeps opening, now at this frame.
            Some(WrapRequest::Open(_)) => WrapRequest::Open(frame),
            _ if self.active => WrapRequest::Scrub(frame),
            _ => {
                self.active = true;
                WrapRequest::Open(frame)
            }
        });
    }

    /// The program changed (a segment or cover was written): in Wrap it is built
    /// and opened again at the playhead, as v5 rebuilds its program playlist.
    pub fn reopen(&mut self) {
        if self.active {
            self.awaiting = Some(self.playhead);
            self.request = Some(WrapRequest::Open(self.playhead));
        }
    }

    /// One frame back or forward from the playhead, in Wrap only.
    pub fn step(&mut self, frames: i64) -> bool {
        if !self.active {
            return false;
        }
        let last = self.total_frames.saturating_sub(1);
        let frame = self.playhead.saturating_add_signed(frames).min(last);
        self.seek(frame);
        true
    }

    /// Back to the Source view: nothing more is asked of the program player.
    pub fn leave(&mut self) {
        self.active = false;
        self.request = None;
        self.awaiting = None;
    }

    /// What the program player must do, once.
    pub fn take_request(&mut self) -> Option<WrapRequest> {
        self.request.take()
    }

    /// A program frame the program player confirmed. Before the asked frame
    /// lands, older frames are ignored; nothing is interpolated.
    pub fn follow_program(&mut self, confirmed: Option<u64>) {
        let Some(frame) = confirmed.filter(|_| self.active) else {
            return;
        };
        if let Some(awaiting) = self.awaiting {
            if frame != awaiting {
                return;
            }
            self.awaiting = None;
        }
        self.playhead = frame;
    }

    /// One repaint: a changed program opens again, a program frame the user
    /// pointed at, if any, then the playhead kept inside the program. Returns what
    /// the program player must do.
    pub fn apply(
        &mut self,
        seek: Option<u64>,
        total_frames: u64,
        program_changed: bool,
    ) -> Option<WrapRequest> {
        if program_changed {
            self.reopen();
        }
        if let Some(frame) = seek {
            self.seek(frame);
        }
        self.clamp(total_frames);
        self.take_request()
    }

    /// Keeps the playhead inside a program that became shorter.
    pub fn clamp(&mut self, total_frames: u64) {
        self.total_frames = total_frames;
        self.playhead = self.playhead.min(total_frames);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_point_opens_the_program_and_the_next_ones_scrub_it() {
        let mut wrap = WrapSession::new();
        assert!(!wrap.is_active());
        wrap.seek(120);
        assert!(wrap.is_active());
        assert_eq!(wrap.playhead(), 120, "v5: the playhead goes there at once");
        assert_eq!(wrap.take_request(), Some(WrapRequest::Open(120)));
        assert_eq!(wrap.take_request(), None);
        wrap.seek(40);
        assert_eq!(wrap.take_request(), Some(WrapRequest::Scrub(40)));
    }

    #[test]
    fn confirmed_frames_take_over_only_after_the_asked_frame() {
        let mut wrap = WrapSession::new();
        wrap.seek(50);
        wrap.follow_program(Some(12));
        assert_eq!(
            wrap.playhead(),
            50,
            "an older picture does not move it back"
        );
        wrap.follow_program(Some(50));
        wrap.follow_program(Some(53));
        assert_eq!(wrap.playhead(), 53);
    }

    #[test]
    fn leaving_wrap_keeps_its_playhead_and_asks_nothing_more() {
        let mut wrap = WrapSession::new();
        wrap.seek(30);
        wrap.leave();
        assert!(!wrap.is_active());
        assert_eq!(wrap.take_request(), None);
        wrap.follow_program(Some(99));
        assert_eq!(wrap.playhead(), 30, "no program frames outside Wrap");
        assert!(!wrap.step(1), "steps belong to the Source view then");
        wrap.seek(30);
        assert_eq!(
            wrap.take_request(),
            Some(WrapRequest::Open(30)),
            "coming back opens the program again"
        );
    }

    #[test]
    fn a_changed_program_opens_again_only_in_wrap() {
        let mut wrap = WrapSession::new();
        wrap.reopen();
        assert_eq!(wrap.take_request(), None, "nothing to reopen in Source");
        wrap.seek(20);
        wrap.take_request();
        wrap.reopen();
        wrap.seek(25);
        assert_eq!(
            wrap.take_request(),
            Some(WrapRequest::Open(25)),
            "a click right after a change still opens the new program"
        );
        assert!(WrapRequest::Open(1).opens() && !WrapRequest::Scrub(1).opens());
    }

    #[test]
    fn steps_stay_inside_the_program() {
        let mut wrap = WrapSession::new();
        assert_eq!(wrap.apply(Some(0), 10, false), Some(WrapRequest::Open(0)));
        wrap.take_request();
        assert!(wrap.step(-1));
        assert_eq!(wrap.playhead(), 0);
        wrap.seek(9);
        assert!(wrap.step(1));
        assert_eq!(wrap.playhead(), 9);
        assert_eq!(wrap.take_request(), Some(WrapRequest::Scrub(9)));
        wrap.clamp(4);
        assert_eq!(wrap.playhead(), 4);
    }
}
