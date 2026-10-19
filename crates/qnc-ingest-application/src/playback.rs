//! The application only decides *when* to preview and what the playback guard allows;
//! the neutral `qnc-source-preview` holds the player, and `qnc-ingest-preview-source`
//! adapts the Ingest content database to that universal preview.

use super::*;
#[cfg(test)]
use qnc_player_client::Player;

impl IngestApplication {
    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.preview.notify_on_change(notify);
    }

    /// Copies what the preview reports into the view the form reads.
    pub(super) fn sync_playback_view(&mut self) {
        let preview = self.preview.view();
        (self.view.playback, self.view.monitor_auto) = (preview.playback, preview.monitor_auto);
        self.view.timeline = preview.timeline.preserving_source_marks_from(&self.view.timeline);
        // Filmstrip and wave come from the universal preview, read again while the worker makes them.
        let clip = self.view.preview_clip_id.clone().unwrap_or_default();
        let assets = if preview.assets.clip_id == clip { preview.assets } else { SourceTimelineAssets::empty_for(&clip) };
        let posters = self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.thumb_image.as_deref()));
        self.view.timeline_assets = qnc_timeline_assets::with_clip_poster(assets, posters);
        self.view.source_timeline = self.source_timeline.sync(self.view.preview_clip_id.as_deref(), self.catalog_target.as_ref()).clone();
    }

    pub(super) fn prepare_preview(&mut self, clip_id: String) -> IngestDispatchResult {
        if self.view.preview_clip_id.as_deref() == Some(&clip_id)
            && self.view.playback.error.is_none()
            && (self.view.playback.preparing || self.view.playback.reply.is_some())
        {
            // The player session is already right, but the artifacts of the clip may
            // have changed (filmstrip or wave finished): never keep a stale timeline.
            self.preview.refresh_assets();
            self.sync_playback_view();
            return IngestDispatchResult::accepted(None, true);
        }
        let Some(save_state) = self
            .view
            .clips
            .iter()
            .find(|c| c.clip_id == clip_id)
            .map(|c| c.save_state)
        else {
            return IngestDispatchResult::rejected("Clip nije pronadjen.");
        };
        // Cut the old session even when the new clip/settings cannot be prepared.
        self.stop_player();
        self.view.preview_clip_id = Some(clip_id.clone());
        self.view.timeline_assets = SourceTimelineAssets::empty_for(&clip_id); // the preview reads them
        if save_state != SaveState::Saved {
            self.view.message = "Klip jos nije spremljen u bazu.".into();
            return IngestDispatchResult::accepted(None, true);
        }
        let (Some(reader), Some(plan), Some(config), Some(content_target)) = (
            self.settings_reader.clone(),
            self.work_plan(),
            self.selection_config.clone(),
            self.catalog_target.clone(),
        ) else {
            return IngestDispatchResult::accepted(None, true);
        };
        self.preview
            .configure(reader, plan.settings.clone(), &config, content_target);
        let error = self
            .preview
            .open_saved_clip(&clip_id, save_state == SaveState::Saved)
            .err()
            .unwrap_or_default();
        self.sync_playback_view();
        if !error.is_empty() {
            return IngestDispatchResult::rejected_shown(error);
        }
        IngestDispatchResult::accepted(None, true)
    }

    pub(super) fn stop_player(&mut self) {
        self.preview.close();
        self.view.playback = Default::default();
        self.view.timeline = Default::default();
    }

    pub(super) fn player_action(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        // Without a player only Escape still lets go of what the source timeline took.
        let escape = matches!(intent.action_id.as_str(), "clear_focus" | "close_player");
        if !self.preview.has_player() && !escape {
            let message = "Broadcast Player nije povezan.";
            self.view.message = message.into();
            return IngestDispatchResult::rejected_shown(message);
        }
        // The source timeline's keys are the piece every form takes (Play keeps the guard below).
        if intent.action_id != action_ids::PLAY_PAUSE {
            if let Some(changed) = self.source_timeline.route(&intent.action_id, self.preview.source_preview_mut(), &mut self.view.timeline) {
                return IngestDispatchResult::accepted(None, changed);
            }
        }
        let command = match intent.action_id.as_str() {
            action_ids::PLAY_PAUSE => qnc_ingest_preview_source::PreviewCommand::TogglePlay,
            action_ids::INGEST_CUE_FRAME => match intent.payload {
                IngestPayload::Frame(frame) if frame >= 0 => {
                    let cue = TimelineIntent::CueFrame(frame as u64); // the same in every form
                    return IngestDispatchResult::accepted(None, self.source_timeline.cue(&cue, self.preview.source_preview_mut()));
                }
                _ => return IngestDispatchResult::rejected("Neispravan frame."),
            },
            _ => return IngestDispatchResult::accepted(None, false), // a source timeline key with nothing to do (Escape)
        };
        let outcome = match self.preview.send_command(command) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.view.message = error.clone();
                return IngestDispatchResult::rejected_shown(error);
            }
        };
        if outcome.started_playback {
            self.apply_playback_guard();
        }
        if outcome.message.is_empty() {
            return IngestDispatchResult::accepted(None, true);
        }
        self.view.message = outcome.message.clone();
        IngestDispatchResult::rejected_shown(outcome.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_pending_clip_cuts_old_session_and_keeps_thumbnail_selection() {
        let mut component = IngestApplication::default();
        queue_play(&mut component);
        component.view.preview_clip_id = Some("old".into());
        component.view.playback.video_visible = true;
        component.view.clips = vec![ClipView {
            clip_id: "new".into(),
            save_state: SaveState::Pending,
            ..Default::default()
        }];
        let result = component.prepare_preview("new".into());
        assert!(result.accepted);
        assert_eq!(component.view.preview_clip_id.as_deref(), Some("new"));
        assert_eq!(component.view.timeline_assets.clip_id, "new");
        assert_eq!(component.view.playback, qnc_player_client::View::default());
        assert_eq!(
            component.preview.player_view(),
            &qnc_player_client::View::default()
        );
        assert!(!component.preview.play_when_ready());
    }

    #[test]
    fn play_pause_during_prepare_is_queued_until_player_is_ready() {
        let mut component = IngestApplication::default();
        let player = Player::new().unwrap();
        let (resume, wait) = mpsc::sync_channel(1);
        player.prepare(move || {
            let _ = wait.recv_timeout(std::time::Duration::from_secs(1));
            Err("test prepare stopped".into())
        });
        component.preview.attach_player(player);

        let result = component.player_action(IngestIntent::empty(action_ids::PLAY_PAUSE));

        assert!(result.accepted);
        assert!(result.request_repaint);
        assert!(component.preview.play_when_ready());
        assert_ne!(component.view.message, "Play ceka spreman player.");
        let _ = resume.send(());
    }
}

#[cfg(test)]
mod preview_refresh_tests {
    use super::*;

    #[test]
    fn choosing_the_clip_that_is_already_prepared_still_refreshes_its_timeline_artifacts() {
        let mut app = IngestApplication::new();
        app.view.preview_clip_id = Some("clip-1".into());
        app.view.playback.preparing = true;
        assert!(app.view.timeline_assets.clip_id.is_empty());
        let result = app.prepare_preview("clip-1".into());
        assert!(result.accepted);
        assert_eq!(app.view.timeline_assets.clip_id, "clip-1");
    }
}

#[cfg(test)]
pub(crate) fn queue_play(component: &mut IngestApplication) {
    let player = Player::new().unwrap();
    player.prepare(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        Err("test prepare stopped".into())
    });
    component.preview.attach_player(player);
    component.preview.toggle_play();
    assert!(component.preview.play_when_ready());
}
