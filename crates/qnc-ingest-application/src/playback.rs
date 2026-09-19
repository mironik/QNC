use super::*;
use qnc_player_client::{Action, Player};
use qnc_player_input::{InputReader, PlayerClipRecord, PlayerContentRead};
use qnc_player_launcher::SourceTransportBinding;
use std::sync::Arc;

#[derive(Clone)]
struct IngestPlayerContentReader {
    content_target: qnc_ingest_store::content::ContentTarget,
}

impl PlayerContentRead for IngestPlayerContentReader {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let stored = self
            .content_target
            .open(qnc_ingest_store::content::Access::ReadOnly)?
            .read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
        }))
    }
}

impl IngestApplication {
    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        if let Some(player) = &self.player {
            player.notify_on_change(notify);
        }
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
        self.play_when_ready = false;
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
        let workspace = plan.settings.workspace_db_uri.clone();
        if self.player.is_none() {
            match Player::new() {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    let mut result = IngestDispatchResult::rejected(error);
                    result.request_repaint = true;
                    return result;
                }
            }
        }
        self.player.as_ref().unwrap().prepare(move || {
            let sources = config
                .sources
                .iter()
                .map(|source| {
                    if let Some(root) = &source.location.file {
                        return Ok(SourceTransportBinding::local(
                            source.location.uri.clone(),
                            root.clone(),
                        ));
                    }
                    SourceTransportBinding::network(
                        source.location.uri.clone(),
                        source
                            .location
                            .endpoint
                            .clone()
                            .ok_or("Source endpoint missing.")?,
                        source
                            .location
                            .token()
                            .map_err(|e| e.to_string())?
                            .ok_or("Source credential missing.")?,
                    )
                })
                .collect::<Result<Vec<_>, String>>()?;
            let executable = qnc_player_launcher::sibling_executable("qnc-broadcast-player")?;
            let input = InputReader::with_content_reader(
                reader,
                Arc::new(IngestPlayerContentReader { content_target }),
            )
            .load(&workspace, &clip_id)
            .map_err(|e| e.to_string())?;
            qnc_player_launcher::prepare_launch(input, &sources, executable)
        });
        self.view.playback = self.player.as_ref().unwrap().view();
        self.view.timeline = playback_timeline_projection(&self.view.playback);
        self.update_timeline_artifact_playback_priority();
        IngestDispatchResult::accepted(None, true)
    }
    pub(super) fn stop_player(&mut self) {
        self.play_when_ready = false;
        if let Some(player) = &self.player {
            player.close();
        }
        self.view.playback = Default::default();
        self.view.timeline = Default::default();
        self.set_timeline_artifact_playback_priority(false);
    }
    pub(super) fn player_action(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        if self.player.is_none() {
            let message = "Broadcast Player nije povezan.";
            self.view.message = message.into();
            let mut result = IngestDispatchResult::rejected(message);
            result.request_repaint = true;
            return result;
        }
        let action = match intent.action_id.as_str() {
            action_ids::PLAY_PAUSE => Action::TogglePlayPause,
            action_ids::STEP_BACK_FRAME => Action::Step(-1),
            action_ids::STEP_FORWARD_FRAME => Action::Step(1),
            action_ids::INGEST_CUE_FRAME => match intent.payload {
                IngestPayload::Frame(frame) if frame >= 0 => Action::Cue(frame as u64),
                _ => return IngestDispatchResult::rejected("Neispravan frame."),
            },
            _ => return IngestDispatchResult::rejected("Nepoznata player akcija."),
        };
        if matches!(action, Action::TogglePlayPause) {
            let playback = self.player.as_ref().unwrap().view();
            if !playback.playing() && !playback.can_start_playback() && playback.error.is_none() {
                self.play_when_ready = true;
                self.apply_playback_guard();
                return IngestDispatchResult::accepted(None, true);
            }
            if !playback.playing() {
                self.apply_playback_guard();
            }
        }
        let result = self
            .player
            .as_ref()
            .ok_or_else(|| "Broadcast Player nije povezan.".to_string())
            .and_then(|player| player.send(action));
        match result {
            Ok(()) => {
                self.play_when_ready = false;
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.message = error.clone();
                let mut result = IngestDispatchResult::rejected(error);
                result.request_repaint = true;
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_pending_clip_cuts_old_session_and_keeps_thumbnail_selection() {
        let mut component = IngestApplication::default();
        component.player = Some(Player::new().unwrap());
        component.play_when_ready = true;
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
            component.player.as_ref().unwrap().view(),
            qnc_player_client::View::default()
        );
        assert!(!component.play_when_ready);
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
        component.player = Some(player);

        let result = component.player_action(IngestIntent::empty(action_ids::PLAY_PAUSE));

        assert!(result.accepted);
        assert!(result.request_repaint);
        assert!(component.play_when_ready);
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
