//! The clip the media pool points at, given once when the pointer moved to it (user
//! 2026-10-20: the filmstrip of the pointed clip is read ahead). Moved from `qnc-pool-pointer`
//! (docs/99 step 2): the preview is its only user; the pool pointer stays the gui channel.

/// Gives a pointed clip once, when the pointer moved to it.
#[derive(Debug, Default)]
pub(crate) struct Follow {
    last: Option<String>,
}

impl Follow {
    pub(crate) fn new_clip<'a>(&mut self, pointed: Option<&'a str>) -> Option<&'a str> {
        let pointed = pointed?;
        if self.last.as_deref() == Some(pointed) {
            return None;
        }
        self.last = Some(pointed.to_string());
        Some(pointed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointed_clip_is_given_once_until_the_pointer_moves() {
        let mut follow = Follow::default();
        assert_eq!(follow.new_clip(Some("a")), Some("a"));
        assert_eq!(follow.new_clip(Some("a")), None);
        assert_eq!(follow.new_clip(None), None);
        assert_eq!(follow.new_clip(Some("b")), Some("b"));
        assert_eq!(follow.new_clip(Some("a")), Some("a"));
    }
}
