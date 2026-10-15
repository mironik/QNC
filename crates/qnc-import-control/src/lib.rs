//! Import control: the order of the background work after Select and Import (user
//! 2026-10-15). A wave reads little more than a clip's sound and takes little of the
//! processor, so the filmstrip does not wait for all the waves: as soon as a clip's wave is
//! done, the filmstrip worker is told to make that clip. While waves or records are still
//! being made the filmstrip takes no clip of its own; once both are done it takes the rest
//! (a clip that gets no wave, for one).
//!
//! It only decides; it reads nothing, writes nothing and runs no worker.

pub const MODULE_ID: &str = "qnc.module.import-control";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the filmstrip worker is told; `P` are the key pictures read with the wave, when
/// the wave's one pass over the original gave them (the filmstrip then decodes them and
/// does not read the media again).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order<P> {
    /// Make the filmstrip of this clip now.
    MakeFilmstrip(String, Option<P>),
    /// Make only the clips you are told (waves or records are still being made).
    FilmstripOnOrders,
    /// Take every clip that has no filmstrip yet.
    FilmstripOnItsOwn,
}

#[derive(Debug, Default)]
pub struct ImportControl {
    on_its_own: Option<bool>,
}

impl ImportControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// The clips whose wave is done (made, already stored, or impossible): each gets its
    /// filmstrip next.
    pub fn waves_done<P>(&self, clips: impl IntoIterator<Item = (String, Option<P>)>) -> Vec<Order<P>> {
        clips.into_iter().map(|(clip, pictures)| Order::MakeFilmstrip(clip, pictures)).collect()
    }

    /// How the filmstrip works now; an order only when that changes.
    pub fn filmstrip_mode<P>(&mut self, waves_busy: bool, records_completing: bool) -> Option<Order<P>> {
        let on_its_own = !waves_busy && !records_completing;
        if self.on_its_own == Some(on_its_own) {
            return None;
        }
        self.on_its_own = Some(on_its_own);
        Some(if on_its_own { Order::FilmstripOnItsOwn } else { Order::FilmstripOnOrders })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_gets_its_filmstrip_when_its_wave_is_done_and_the_rest_when_all_is_done() {
        let mut control = ImportControl::new();
        assert_eq!(control.filmstrip_mode::<u8>(true, true), Some(Order::FilmstripOnOrders));
        assert_eq!(control.filmstrip_mode::<u8>(true, false), None, "no change, no order");
        assert_eq!(control.waves_done([("c1".to_string(), Some(7u8))]), [Order::MakeFilmstrip("c1".into(), Some(7))]);
        assert_eq!(control.filmstrip_mode::<u8>(false, true), None, "records still being completed");
        assert_eq!(control.filmstrip_mode::<u8>(false, false), Some(Order::FilmstripOnItsOwn));
        assert_eq!(control.filmstrip_mode::<u8>(true, false), Some(Order::FilmstripOnOrders), "new waves");
    }
}
