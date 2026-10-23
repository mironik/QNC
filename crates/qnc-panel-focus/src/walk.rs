use super::*;

impl PanelFocus {
    /// The panels Tab and Enter reach: those of the layout that have something to take.
    pub(crate) fn walk(&self) -> Vec<Panel> {
        self.walk.reachable().into_iter().filter_map(Panel::from_name).collect()
    }

    /// Enter with nothing to confirm: on to the next panel of the walk.
    pub(crate) fn advance(&mut self, preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &ProgramSegments) -> bool {
        let next = cycle(self.panel, true, &self.walk());
        self.enter(next, preview, wrap, segments)
    }

    /// A segment edge written or dropped: the mark is let go and the keyboard and
    /// the Wrap playhead go back to the segment panel.
    pub(crate) fn back_to_wrap(&mut self, segments: &mut ProgramSegments, wrap: &mut WrapSession, timeline: &mut TimelineProjection) -> bool {
        let Some(frame) = segments.take_wrap_return() else {
            return false;
        };
        self.source.set_mark_taken(false);
        qnc_source_mark_focus::release(timeline);
        self.panel = Panel::Segments;
        wrap.seek(frame);
        true
    }

    /// Tab into a panel: the source timeline shows the source clip again (v5
    /// `activate_source_keyboard_panel`), the segment panel opens the program at
    /// the Wrap playhead.
    pub(crate) fn enter(&mut self, panel: Panel, preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &ProgramSegments) -> bool {
        self.panel = panel;
        match panel {
            Panel::SourceTimeline if wrap.is_active() => {
                wrap.leave();
                let frame = preview.view().timeline.playhead_frame.unwrap_or(0);
                preview.timeline_intent(&TimelineIntent::CueFrame(frame));
            }
            Panel::Segments if !wrap.is_active() && !segments.view().is_empty() => wrap.seek(wrap.playhead()),
            _ => {}
        }
        true
    }
}
