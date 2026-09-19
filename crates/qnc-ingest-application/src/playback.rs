//! The application only decides *when* to preview and what the playback guard allows;
//! the neutral `qnc-source-preview` holds the player, and `qnc-ingest-preview` tells it
//! where Ingest keeps its clips.

use super::*;
#[cfg(test)]
use qnc_player_client::Player;

impl IngestApplication {
    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.preview.notify_on_change(notify);
    }

    /// Copies what the preview reports into the view the form reads.
    pub(super) fn sync_playback_view(&mut self) {
        self.view.playback = self.preview.player_view().clone();
        self.view.timeline = playback_timeline_projection(&self.view.playback);
    }

    pub(super) fn prepare_preview(&mut self, clip_id: String) -> IngestDispatchResult {
        if self.view.preview_clip_id.as_deref() == Some(&clip_id)
            && self.view.playback.error.is_none()
            && (self.view.playback.preparing || self.view.playback.reply.is_some())
        {
            // The player session is already right, but the artifacts of the clip may
            // have changed (filmstrip or wave finished): never keep a stale timeline.
            self.focus_timeline_assets(&clip_id);
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
        self.set_timeline_artifact_playback_priority(true);
        self.view.preview_clip_id = Some(clip_id.clone());
        if save_state != SaveState::Saved {
            let mut result = IngestDispatchResult::rejected("Klip jos nije spremljen u bazu.");
            result.request_repaint = true;
            return result;
        }
        self.focus_timeline_assets(&clip_id);
        let (Some(reader), Some(plan), Some(config), Some(content_target)) = (
            self.settings_reader.clone(),
            self.work_plan(),
            self.selection_config.clone(),
            self.catalog_target.clone(),
        ) else {
            return IngestDispatchResult::accepted(None, true);
        };
        self.preview.configure(qnc_ingest_preview::preview_context(
            reader,
            plan.settings.clone(),
            &config,
            content_target,
        ));
        self.preview.open(&clip_id);
        let error = self.preview.take_message();
        self.sync_playback_view();
        self.update_timeline_artifact_playback_priority();
        if !error.is_empty() {
            let mut result = IngestDispatchResult::rejected(error);
            result.request_repaint = true;
            return result;
        }
        IngestDispatchResult::accepted(None, true)
    }

    pub(super) fn stop_player(&mut self) {
        self.preview.close();
        self.view.playback = Default::default();
        self.view.timeline = Default::default();
        self.set_timeline_artifact_playback_priority(false);
    }

    pub(super) fn player_action(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        if !self.preview.has_player() {
            let message = "Broadcast Player nije povezan.";
            self.view.message = message.into();
            let mut result = IngestDispatchResult::rejected(message);
            result.request_repaint = true;
            return result;
        }
        match intent.action_id.as_str() {
            action_ids::PLAY_PAUSE => {
                let was_playing = self.preview.player_view().playing();
                self.preview.toggle_play();
                if !was_playing {
                    self.apply_playback_guard();
                }
            }
            action_ids::STEP_BACK_FRAME => {
                self.preview.step(-1);
            }
            action_ids::STEP_FORWARD_FRAME => {
                self.preview.step(1);
            }
            action_ids::INGEST_CUE_FRAME => match intent.payload {
                IngestPayload::Frame(frame) if frame >= 0 => {
                    self.preview.cue(frame as u64);
                }
                _ => return IngestDispatchResult::rejected("Neispravan frame."),
            },
            _ => return IngestDispatchResult::rejected("Nepoznata player akcija."),
        }
        let error = self.preview.take_message();
        if error.is_empty() {
            return IngestDispatchResult::accepted(None, true);
        }
        self.view.message = error.clone();
        let mut result = IngestDispatchResult::rejected(error);
        result.request_repaint = true;
        result
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
        assert!(!result.accepted);
        assert_eq!(component.view.preview_clip_id.as_deref(), Some("new"));
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
