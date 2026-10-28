
/// A panel of an editorial surface that takes the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    /// The tabs of the pool head (All, SubClip, B-roll, Segment).
    PoolTabs,
    Pool,
    /// v5: choosing a clip puts the keyboard on its source timeline.
    #[default]
    SourceTimeline,
    /// The action buttons in the head of the source dock.
    SourceButtons,
    Segments,
}

impl Panel {
    /// The name of the panel in a layout contract and in the focus walk.
    pub fn name(self) -> &'static str {
        match self {
            Self::PoolTabs => qnc_focus_walk::POOL_TABS,
            Self::Pool => qnc_focus_walk::CARDS,
            Self::SourceTimeline => qnc_focus_walk::SOURCE_TIMELINE,
            Self::SourceButtons => qnc_focus_walk::SOURCE_BUTTONS,
            Self::Segments => "segments",
        }
    }

    /// The panel a layout contract names in its `tab_order`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "pool_tabs" => Self::PoolTabs,
            "pool" => Self::Pool,
            "source_timeline" => Self::SourceTimeline,
            "source_buttons" => Self::SourceButtons,
            "segments" => Self::Segments,
            _ => return None,
        })
    }
}

/// The Tab walk of one form (user 2026-10-13: it belongs to the form, from its layout
/// contract): its panels in order, the action ids of its pool tabs and of its source
/// dock buttons, left to right.
#[derive(Debug, Clone)]
pub struct FocusLayout {
    pub order: Vec<Panel>,
    pub tabs: Vec<String>,
    pub buttons: Vec<String>,
}

impl FocusLayout {
    pub fn new((order, tabs, buttons): (Vec<&str>, Vec<&str>, Vec<&str>)) -> Self {
        let owned = |names: Vec<&str>| names.into_iter().map(str::to_string).collect();
        Self { order: order.into_iter().filter_map(Panel::from_name).collect(), tabs: owned(tabs), buttons: owned(buttons) }
    }
}

impl Default for FocusLayout {
    /// The walk without a layout: pool, source timeline, segments.
    fn default() -> Self {
        Self { order: vec![Panel::Pool, Panel::SourceTimeline, Panel::Segments], tabs: Vec::new(), buttons: Vec::new() }
    }
}

/// A key on the media pool: the card focus moves, or Enter opens the focused card (the
/// public card grid applies it).
pub use qnc_key_list::GridStep as PoolStep;

/// What a form draws of the focus: the panel, on the source dock buttons the action of
/// the one the arrows are on, and the last key on the media pool with its number (the
/// card grid applies each number once, however often the view is read).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FocusView {
    pub panel: Panel,
    pub button: Option<String>,
    pub pool_step: Option<(PoolStep, u64)>,
    /// The transcript of the clip on the preview.
    pub transcript: qnc_transcript::TranscriptView,
    /// The shots of the clip on the preview (Video AI).
    pub shots: qnc_clip_shots::ShotsView,
}

impl FocusView {
    /// The catalog scope that reads the keys first: the source timeline's own while it (or
    /// its buttons) has the keyboard (piece `qnc-source-timeline`).
    pub fn key_scope(&self) -> Option<&'static str> {
        matches!(self.panel, Panel::SourceTimeline | Panel::SourceButtons).then_some(qnc_source_timeline::KEY_SCOPE)
    }
}
