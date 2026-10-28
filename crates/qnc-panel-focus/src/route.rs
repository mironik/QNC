use super::*;

impl PanelFocus {
    /// Routes one system action, from any input device (user 2026-10-18), to the panel it
    /// belongs to: a key to the panel in focus, a pointer to the panel it was on. `None`: not
    /// a panel action (the application handles it); `Some(changed)`: taken here, done or ignored.
    pub fn route(
        &mut self,
        event: impl Into<ActionEvent>,
        pieces: (
            &mut SourcePreview,
            &mut WrapSession,
            &mut ProgramSegments,
            &mut TimelineProjection,
        ),
    ) -> Option<bool> {
        let event = event.into();
        let action_id = event.action_id.as_str();
        let panel = self.panel;
        // The transcript's own actions (Media Assist Audio AI, user 2026-10-26).
        if let Some(done) = self.transcript.act(action_id, event.value, event.target.as_deref(), pieces.0, pieces.3, self.source.view().timecode.as_ref()) {
            return Some(done);
        }
        // A click on a shot of the Video AI panel puts the playhead on its start (user 2026-10-27).
        if action_id == qnc_clip_shots::CUE_SHOT {
            let frame = event.target.as_deref().and_then(|frame| frame.parse::<u64>().ok());
            return Some(frame.is_some_and(|frame| pieces.0.cue(frame)));
        }
        // A click on a usable part opens it with its IN/OUT, for Add SubClip (user 2026-10-27).
        if action_id == qnc_clip_shots::OPEN_PART {
            return Some(self.shots.open_part(event.target.as_deref()));
        }
        // A double click on a word is a cut (user 2026-10-26): IN at its start, OUT at the end
        // of a word after the first cut; the source timeline keeps the pair as for scissors.
        if action_id == qnc_transcript::CUT_WORD {
            let word = event.value.and_then(|from| u64::try_from(from).ok()).zip(event.target.as_deref().and_then(|to| to.parse().ok()));
            let Some((from, to)) = word else { return Some(false) };
            // In the trim zoom of a taken IN or OUT the word moves that mark: IN to its start,
            // OUT to its end; the mark stays taken.
            if self.source.mark_taken(pieces.3) {
                let in_mark = pieces.3.source_mark_focus == qnc_timeline::TimelineSourceMarkFocus::In;
                return Some(self.source.drag(in_mark, if in_mark { from } else { to }, pieces.0, pieces.3));
            }
            return Some(self.source.cut_word(pieces.3, from, to));
        }
        // The Export button opens the box of the project's export settings (user 2026-10-24);
        // while it is open its keys are its own.
        if action_id == "export_open" {
            if let Some(export) = pieces.0.export_settings() {
                pieces.2.open_export(&export, pieces.0.story_outputs());
            }
            return Some(true);
        }
        if pieces.2.export_open() && event.at.is_none() {
            use qnc_program_segments::SegmentCommand;
            match action_id {
                "step_prev_part" | "step_prev_segment" | "page_prev" => pieces.2.export_step(true),
                "step_next_part" | "step_next_segment" | "page_next" => pieces.2.export_step(false),
                // Left and right: the PAL and NTSC tabs (user 2026-10-25).
                "step_back_frame" => pieces.2.export_tab(false),
                "step_forward_frame" => pieces.2.export_tab(true),
                "activate_focused_item" => _ = pieces.2.apply(SegmentCommand::ExportConfirm),
                "clear_focus" | "close_player" => _ = pieces.2.apply(SegmentCommand::ExportCancel),
                _ => {}
            }
            return Some(true);
        }
        let routed = match self.pointer(&event, (&mut *pieces.0, &mut *pieces.1, &mut *pieces.2, &mut *pieces.3)) {
            Some(answer) => answer,
            None => self.route_on_panel(action_id, (&mut *pieces.0, &mut *pieces.1, &mut *pieces.2, &mut *pieces.3)),
        };
        // Every key of the story in the player log (live 2026-10-16: IN/OUT not taken
        // after some action): the action, the panel it met, what came of it and the source
        // marks and playhead as the key found them.
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            let (preview, timeline) = (&*pieces.0, &*pieces.3);
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "story-key action={action_id} value={:?} target={:?} at={:?} panel={panel:?}->{:?} routed={routed:?} in={:?} out={:?} playhead={:?} program={} part_sound={} marker={} edge={} message={:?}",
                    event.value,
                    event.target,
                    event.at,
                    self.panel,
                    timeline.source_in_frame,
                    timeline.source_out_frame,
                    timeline.playhead_frame,
                    preview.in_program(),
                    pieces.2.part_sound_taken(),
                    pieces.2.editing_marker(),
                    pieces.2.trimming_edge(),
                    pieces.2.view().message,
                ),
            );
        }
        routed
    }

    /// An event of a pointer (a click, a drag, the wheel) on a panel: the panel takes the
    /// keyboard (a click in a panel focuses it); what names its object or frame is answered
    /// here, the rest goes on as the same key on that panel. `None`: go on as a key.
    pub(crate) fn pointer(
        &mut self,
        event: &ActionEvent,
        (preview, wrap, segments, timeline): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> Option<Option<bool>> {
        if event.is_at(qnc_segment_panel_name()) {
            self.panel = Panel::Segments;
            // A left click while something is taken on a Wrap row keeps it (as Enter, without
            // walking on), then the playhead goes where it was clicked (user rule 2026-10-20).
            if event.action_id == "segments_keep_edit" {
                let kept = qnc_segments_keys::confirm(preview, segments);
                let cued = event.unsigned().is_some_and(|frame| segments.apply(qnc_program_segments::SegmentCommand::Cue(frame)));
                return Some(Some(kept || cued));
            }
            // The marker, slot, cover, frame or channel the pointer named (piece
            // `qnc-program-commands`: one map for keys and pointers).
            return match qnc_program_segments::SegmentCommand::from_event(event)? {
                qnc_program_segments::SegmentCommand::PickChannel { lane, channel } => {
                    Some(Some(qnc_segments_keys::pick(lane, channel, (preview, wrap, segments))))
                }
                command => Some(Some(segments.apply(command))),
            };
        }
        if !event.is_at(qnc_source_timeline::PANEL) {
            return None;
        }
        // A click on the source timeline: the Source view, the playhead where it points (in
        // every form the same); a dragged IN/OUT or a channel picked with its value.
        let id = event.action_id.as_str();
        match id {
            "timeline_cue_frame" | "playlist_input_start" => {
                self.to_source(wrap);
                let intent = event.unsigned().filter(|_| id == "timeline_cue_frame").map_or(TimelineIntent::GoHome, TimelineIntent::CueFrame);
                Some(Some(self.source.cue(&intent, preview)))
            }
            // A left click while IN/OUT or A1/A2 is taken keeps it, then the playhead goes there.
            qnc_source_timeline::KEEP_MARK => {
                self.panel = Panel::SourceTimeline;
                Some(self.source.route_input(id, event.value, preview, timeline))
            }
            // A stopped Sync's OUT is the end of its cover: the mouse moves it as the arrows do.
            qnc_source_timeline::DRAG_OUT if qnc_sync_session::dragged(preview, segments, event.value) => Some(Some(true)),
            qnc_source_timeline::DRAG_IN | qnc_source_timeline::DRAG_OUT | qnc_source_timeline::CHOOSE_A1_CHANNEL | qnc_source_timeline::CHOOSE_A2_CHANNEL => {
                self.panel = Panel::SourceTimeline;
                Some(self.source.route_input(id, event.value, preview, timeline))
            }
            // Scissors (user 2026-10-18): the source timeline keeps the pair of cuts; a cut
            // marks IN, so it arms Sync/B-roll as I does.
            qnc_source_timeline::CUT => {
                self.panel = Panel::SourceTimeline;
                qnc_sync_session::armed_by_in(segments, preview, wrap);
                Some(self.source.route_input(id, event.value, preview, timeline))
            }
            _ => {
                self.panel = Panel::SourceTimeline;
                None
            }
        }
    }

    pub(crate) fn route_on_panel(
        &mut self,
        action_id: &str,
        (preview, wrap, segments, timeline): (
            &mut SourcePreview,
            &mut WrapSession,
            &mut ProgramSegments,
            &mut TimelineProjection,
        ),
    ) -> Option<bool> {
        let panel = self.panel;
        // The dock buttons are part of the source timeline panel (user 2026-10-13): its keys work there too.
        let on_source = matches!(panel, Panel::SourceTimeline | Panel::SourceButtons);
        let on_segments = panel == Panel::Segments;
        preview.hands_on(); // every key of the montage holds the background work off
        // The order below is the priority of the blocks: the first that takes a key answers it
        // (the module keeps only this order; each block does its own work).
        // 1. Whatever the focus: the monitor switch, a click that keeps a source mark, the wheel.
        match action_id {
            // Monitor Auto | On under the monitor: whatever the focus.
            qnc_source_preview::MONITOR_AUTO | qnc_source_preview::MONITOR_ON => return Some(preview.set_monitor_mode(action_id)),
            // A left click on the source timeline while IN/OUT is drawn taken keeps it (piece
            // `qnc-source-timeline`): never Enter, which walks the panels.
            qnc_source_timeline::KEEP_MARK => {
                self.panel = Panel::SourceTimeline;
                return self.source.route_input(action_id, None, preview, timeline);
            }
            // The wheel over a panel (user 2026-10-16, piece `qnc-wheel-jog`): that panel takes
            // the keyboard and the wheel is its left/right arrow.
            // Ctrl+wheel is Ctrl+left/right: the level of a taken A1/A2 sound, up louder.
            wheel if wheel.starts_with("wheel_") && wheel.contains("_ctrl_") => {
                self.panel = if wheel.starts_with("wheel_source") { Panel::SourceTimeline } else { Panel::Segments };
                // On the source timeline with no lane taken, Ctrl+wheel moves the taken IN/OUT or
                // segment edge, as on the source timeline of every form (audit 2026-10-20: in
                // Story it did nothing); with nothing taken it does nothing.
                if self.panel == Panel::SourceTimeline && !self.source.lane_taken() {
                    if !(self.taken(timeline) || segments.trimming_edge()) {
                        return Some(false);
                    }
                    let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                    let arrow = if forward { "step_forward_frame" } else { "step_back_frame" };
                    return self.route_on_panel(arrow, (preview, wrap, segments, timeline));
                }
                let level = if wheel.ends_with("back") { "lane_level_down" } else { "lane_level_up" };
                return self.route_on_panel(level, (preview, wrap, segments, timeline));
            }
            // The wheel alone is the left/right arrow, its direction this computer's choice;
            // where those arrows turn a taken sound off/on, the wheel moves the playhead
            // (user 2026-10-16: the level never goes straight to the wheel, the playhead does).
            wheel if wheel.starts_with("wheel_") => {
                // A segment edge (Ctrl+Shift+I/O) is taken on the source timeline: the wheel
                // moves it wherever the pointer is (live 2026-10-16: over the segment panel the
                // wheel did nothing).
                self.panel = if wheel.starts_with("wheel_source") || segments.trimming_edge() { Panel::SourceTimeline } else { Panel::Segments };
                let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                let frames = if forward { 1 } else { -1 };
                if self.panel == Panel::Segments && segments.part_sound_taken() {
                    return Some(wrap.step(frames));
                }
                if self.panel == Panel::SourceTimeline && self.source.lane_taken() {
                    return self.source.route(wheel, preview, timeline); // the playhead moves
                }
                let arrow = if forward { "step_forward_frame" } else { "step_back_frame" };
                return self.route_on_panel(arrow, (preview, wrap, segments, timeline));
            }
            _ => {}
        }
        // 2. The segment panel: the sound of the stored segment (A1) or cover (A2) under the
        // Wrap playhead (piece `qnc-segments-keys`).
        if on_segments {
            if let ControlFlow::Break(answer) = qnc_segments_keys::overlap(action_id, (preview, wrap, segments)) {
                return answer;
            }
            if let ControlFlow::Break(answer) = qnc_segments_keys::sound(action_id, (preview, wrap, segments)) {
                return answer;
            }
        }
        // 3. A1/A2 of the source timeline are its own (piece `qnc-source-timeline`): Ctrl+1,
        // Ctrl+2 take a lane there; while it is taken its keys belong to it.
        match action_id {
            "select_audio_a1" | "select_audio_a2" => {
                self.panel = Panel::SourceTimeline; // the lane is on the source timeline
                return self.source.route(action_id, preview, timeline);
            }
            lane if on_source && self.source.lane_taken() && LANE_KEYS.contains(&lane) => {
                return self.source.route(lane, preview, timeline);
            }
            _ => {}
        }
        // 4. Sync/B-roll: Space, and a stopped Sync's arrows, Enter, Delete, O (piece `qnc-sync-session`).
        if let ControlFlow::Break(answer) = qnc_sync_session::route(action_id, (preview, wrap, segments), panel != Panel::Segments) {
            return answer;
        }
        // 5. The pool tabs, the media pool and the source dock buttons (pieces `qnc-key-row`,
        // `qnc-key-list`).
        if let Some(answer) = self.rows_and_pool(action_id, (preview, wrap, segments, timeline)) {
            return answer;
        }
        // 6. The source timeline and the segment panel.
        match action_id {
            // Up/Down on a taken edit (user 2026-10-11): to the key picture before or after it,
            // half a second when every picture is one (intra). The answer comes in `poll`.
            "step_prev_part" | "step_next_part"
                if on_source && (self.taken(timeline) || (segments.trimming_edge() && qnc_source_mark_focus::is_taken(timeline))) =>
            {
                let (clip, frame) = (preview.view().clip_id.clone(), qnc_source_mark_focus::taken_picture(timeline));
                self.jump = Some(Jump::SourceMark);
                let forward = action_id == "step_next_part";
                return Some(clip.zip(frame).is_some_and(|(clip, frame)| self.keyframes.request(&clip, frame, forward, preview.still_locator())));
            }
            "step_prev_part" | "step_next_part" if on_segments && segments.keyframe_reference().is_some() => {
                let (clip, frame) = segments.keyframe_reference().unwrap_or_default();
                self.jump = Some(Jump::Program);
                return Some(self.keyframes.request(&clip, frame, action_id == "step_next_part", preview.still_locator()));
            }
            // Enter is the confirmation of the panel in focus only.
            "activate_focused_item" if on_source => {
                let confirmed = segments.confirm_source();
                // A taken mark is let go; with nothing to confirm Enter goes on round the walk.
                let released = self.taken(timeline) && self.source.route("clear_focus", preview, timeline).unwrap_or(false);
                let back = self.back_to_wrap(segments, wrap, timeline);
                return Some(confirmed || released || back || self.advance(preview, wrap, segments));
            }
            "activate_focused_item" if on_segments => {
                return Some(qnc_segments_keys::confirm(preview, segments) || self.advance(preview, wrap, segments));
            }
            "activate_focused_item" => return Some(false),
            // A click on a Wrap row keeps what is taken there; nothing taken, nothing happens.
            "segments_keep_edit" => return Some(qnc_segments_keys::confirm(preview, segments)),
            // A virtual shot (Shift+S or Add SubClip), not by an Enter that confirmed.
            "save_virtual_shot" if segments.source_waits_enter() || self.source.holds_enter() => return Some(false),
            // Down closes an open large wave off the segment panel (there down is the next segment).
            "step_next_part" if on_source && self.source.view().wave_over_video.is_some() => {
                return self.source.route(action_id, preview, timeline);
            }
            // Home on the source timeline: the start of the clip (the V label does the same).
            "playlist_input_start" if on_source => return Some(preview.cue(timeline.range_start_frame)),
            "focus_next" | "focus_prev" => {
                let next = cycle(panel, action_id == "focus_next", &self.walk());
                return Some(self.enter(next, preview, wrap, segments));
            }
            _ => {}
        }
        // I/O, Ctrl+I/O and Ctrl+Shift+I/O of the segment panel (piece `qnc-segments-keys`).
        if on_segments {
            if let ControlFlow::Break(answer) = qnc_segments_keys::marks(action_id, segments, &mut self.source) {
                return answer;
            }
        }
        Some(match action_id {
            // The source timeline marks IN/OUT, as in every form (one owner, piece
            // `qnc-source-panel`; the stills follow the monitor, `qnc-virtual-short-stills`). I/O mark
            // the chosen clip from the pool too (user 2026-10-16).
            "mark_in" | "mark_out" => {
                qnc_source_mark_focus::release(timeline); // a new mark lets go of a taken one
                if action_id == "mark_in" {
                    qnc_sync_session::armed_by_in(segments, preview, wrap); // v5: every IN arms Sync/B-roll
                }
                return self.source.route(action_id, preview, timeline);
            }
            // v5 navigate_adjacent_source_object: start, IN and OUT in order on the source.
            "navigate_prev_object" | "navigate_next_object" if on_source => return self.source.route(action_id, preview, timeline),
            // v5 mark_in_fit_duration: IN at the playhead, OUT the length of the slot.
            "mark_in_fit_duration" if on_source => match segments.fit_slot() {
                Some(frames) => {
                    self.source.set_mark_taken(false);
                    mark_cue(preview, qnc_source_mark_focus::fit(timeline, frames))
                }
                None => false,
            },
            // Escape drops a taken segment edge at once and goes back to Wrap.
            "clear_focus" if segments.trimming_edge() => {
                segments.apply_action(action_id);
                self.back_to_wrap(segments, wrap, timeline)
            }
            // v5 select_mark_in / select_mark_out: Ctrl+I/O take IN/OUT of the chosen clip from
            // the pool too (as I/O mark it there, user 2026-10-17); the keyboard goes to the
            // source timeline, where the arrows and the wheel move the taken mark.
            "select_mark_in" | "select_mark_out" => {
                self.panel = Panel::SourceTimeline;
                return self.source.route(action_id, preview, timeline);
            }
            "step_back_frame" | "step_forward_frame" if segments.trimming_edge() && qnc_source_mark_focus::is_taken(timeline) => {
                self.panel = Panel::SourceTimeline; // the edge is on the source timeline
                self.source.set_mark_taken(true); // a segment edge opened with its mark taken
                return self.source.route(action_id, preview, timeline);
            }
            "clear_focus" if self.taken(timeline) => return self.source.route(action_id, preview, timeline),
            // Escape gives up what is not saved (user 2026-10-21): after a taken mark, a Sync draft.
            "clear_focus" if segments.sync_drafting() || segments.sync_waiting() => segments.discard_sync(),
            // Escape drops the edit of a segment wherever the keyboard is.
            "clear_focus" if on_source && segments.view().editing.is_some() => {
                segments.apply_action(action_id)
            }
            "step_back_frame" | "step_forward_frame" => match panel {
                Panel::Segments => qnc_segments_keys::step(action_id, wrap, segments),
                // The pool has no arrow keys: they step the source playhead (user rule
                // 2026-10-01: the arrows move the playhead one frame, no other key needed).
                Panel::SourceTimeline => return self.source.route(action_id, preview, timeline),
                Panel::Pool | Panel::PoolTabs | Panel::SourceButtons => false, // the keys act on the panel in focus only
            },
            action if SOURCE_TO_PROGRAM.contains(&action) => segments.apply_action(action),
            action if segments.handles(action) => on_segments && segments.apply_action(action),
            _ => return None,
        })
    }

    /// The pool tabs, the media pool and the source dock buttons: `Some(answer)` when the
    /// key is theirs.
    pub(crate) fn rows_and_pool(
        &mut self,
        action_id: &str,
        (preview, wrap, segments, timeline): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> Option<Option<bool>> {
        let panel = self.panel;
        let arrow = matches!(action_id, "step_back_frame" | "step_forward_frame");
        let back = action_id == "step_back_frame";
        // A pool tab, by click or Alt+letter, puts the keyboard on the pool tabs.
        if self.walk.select_in(qnc_focus_walk::POOL_TABS, action_id) {
            self.panel = Panel::PoolTabs;
            return Some(None);
        }
        Some(match (panel, action_id) {
            // The arrows on the pool tabs choose the tab to the left or right.
            (Panel::PoolTabs, _) if arrow => {
                self.pick = self.walk.choose_in(qnc_focus_walk::POOL_TABS, back);
                return Some(None);
            }
            // The arrows on the dock buttons move between them, Enter presses one.
            (Panel::SourceButtons, _) if arrow => {
                self.walk.choose_in(qnc_focus_walk::SOURCE_BUTTONS, back);
                Some(true)
            }
            (Panel::SourceButtons, "activate_focused_item") => {
                let Some(id) = self.walk.chosen_in(qnc_focus_walk::SOURCE_BUTTONS).map(str::to_string) else { return Some(Some(false)) };
                if !segments.view().action_enabled(&id) {
                    return Some(Some(false));
                }
                // The button acts as its catalog action from the source timeline it heads.
                self.panel = Panel::SourceTimeline;
                let routed = self.route(&id, (preview, wrap, segments, timeline));
                if self.panel == Panel::SourceTimeline {
                    self.panel = Panel::SourceButtons;
                }
                if routed.is_none() {
                    self.pick = Some(id);
                }
                self.advance(preview, wrap, segments); // Enter goes on round the walk (user 2026-10-13)
                routed.map(|_| true)
            }
            // Enter on the pool tabs goes on to the pool.
            (Panel::PoolTabs, "activate_focused_item") => Some(self.advance(preview, wrap, segments)),
            // The media pool: the arrows move the card focus, Enter opens the card (and the
            // clip chosen puts the keyboard on its source timeline). The tick key is Ingest's.
            (Panel::Pool, key) if qnc_key_list::step_of(key).is_some_and(|step| step != PoolStep::Toggle) => {
                self.walk.press_list(qnc_focus_walk::CARDS, qnc_key_list::step_of(key).unwrap_or(PoolStep::Open), true);
                Some(true)
            }
            (Panel::PoolTabs | Panel::SourceButtons, "step_prev_part" | "step_next_part") => Some(false),
            _ => return None,
        })
    }
}
