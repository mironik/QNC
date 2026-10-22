use qnc_broadcast_engine::{BroadcastEngineErrorKind, Runtime};
use qnc_player_contract::{
    BroadcastPlayerProtocolCommand as Command, BroadcastPlayerProtocolEvent as Event, VERSION,
    envelope::EventEnvelope,
    map_broadcast_player_protocol_event,
    session::{SessionReply, SessionRequest},
};

pub struct Session {
    pub player: Runtime,
    pub closed: bool,
    id: String,
    generation: u64,
    command_sequence: u64,
    event_sequence: u64,
    failure: Option<String>,
    failure_reported: bool,
    last_tick: std::time::Instant,
    max_tick_gap_us: u128,
}
impl Session {
    pub fn new(player: Runtime, id: String, generation: u64) -> Self {
        Self {
            player,
            closed: false,
            id,
            generation,
            command_sequence: 0,
            event_sequence: 0,
            failure: None,
            failure_reported: false,
            last_tick: std::time::Instant::now(),
            max_tick_gap_us: 0,
        }
    }
    pub fn keeps_process_alive(&self) -> bool {
        matches!(
            self.player.state().status,
            qnc_player_contract::TransportStatus::Preparing
                | qnc_player_contract::TransportStatus::Playing
        )
    }

    pub fn tick(&mut self) {
        let now = std::time::Instant::now();
        self.max_tick_gap_us = self
            .max_tick_gap_us
            .max(now.duration_since(self.last_tick).as_micros());
        self.last_tick = now;
        let audio = self.player.audio_telemetry();
        if self.failure.is_none()
            && let Err(error) = self.player.tick()
        {
            let message = format!(
                "{error}; frame={}; tick_us={}; max_gap_us={}; audio={audio:?}",
                self.player.state().carrier_frame,
                now.elapsed().as_micros(),
                self.max_tick_gap_us
            );
            if error.kind == BroadcastEngineErrorKind::NotReady {
                if qnc_dev_diagnostics::player_diagnostics_enabled() {
                    qnc_dev_diagnostics::log_line(
                        qnc_dev_diagnostics::DiagnosticsStream::Player,
                        format!("player-rebuffer-pending {message}"),
                    );
                }
                return;
            }
            if qnc_dev_diagnostics::player_diagnostics_enabled() {
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!("player-failure {message}"),
                );
            }
            let _ = self.player.pause();
            self.failure = Some(message);
            self.failure_reported = false;
        }
    }
    pub fn handle(&mut self, request: SessionRequest) -> SessionReply {
        let sequence = self
            .event_sequence
            .checked_add(1)
            .ok_or("player event sequence exhausted")?;
        let mut events = Vec::new();
        match request {
            SessionRequest::State(query) => query.validate_for(&self.id, self.generation)?,
            SessionRequest::Command(envelope) => {
                envelope.validate_for(&self.id, self.generation, self.command_sequence)?;
                self.command_sequence = envelope.sequence;
                let command_name = envelope.command.command_name().to_owned();
                let shutdown = matches!(envelope.command, Command::Shutdown);
                let cue = matches!(envelope.command, Command::CueFrame { .. });
                if self.failure.is_some() && matches!(envelope.command, Command::Play) {
                    return self.rejected(
                        sequence,
                        envelope.request_id,
                        command_name,
                        "player preparation failed".into(),
                    );
                }
                let result = match envelope.command {
                    Command::Play => self.player.play(),
                    Command::Pause | Command::Shutdown => self.player.pause(),
                    Command::Stop => self.player.stop(),
                    Command::CueFrame {
                        frame,
                        present_frame,
                    } => self.player.cue_frame(frame, present_frame),
                    Command::SetOutputLevels { levels_percent } => {
                        self.player.set_output_levels(&levels_percent);
                        Ok(Vec::new())
                    }
                    Command::StartSyncBed { source_frame, bed_frame } => match self.player.start_sync_bed(source_frame, bed_frame) {
                        Ok(true) => Ok(Vec::new()),
                        Ok(false) => return self.rejected(sequence, envelope.request_id, command_name, "this session has no Sync bed".into()),
                        Err(error) => Err(error),
                    },
                    Command::StopSyncBed => {
                        self.player.stop_sync_bed();
                        Ok(Vec::new())
                    }
                    Command::SetPartLevel { output, from_frame, to_frame, gain_db, off } => {
                        let part = (from_frame < to_frame).then_some((output, from_frame, to_frame, gain_db, off));
                        match self.player.set_part_level(part) {
                            Ok(true) => Ok(Vec::new()),
                            Ok(false) => return self.rejected(sequence, envelope.request_id, command_name, "this session plays no program".into()),
                            Err(error) => Err(error),
                        }
                    }
                    Command::SetOutputRoutes { channels } => {
                        if !self.player.set_output_routes(&channels) {
                            return self.rejected(sequence, envelope.request_id, command_name, "output route outside the decoded channels".into());
                        }
                        Ok(Vec::new())
                    }
                    _ => {
                        return self.rejected(
                            sequence,
                            envelope.request_id,
                            command_name,
                            "command is not supported by this bound-source player".into(),
                        );
                    }
                };
                match result {
                    Ok(produced) => {
                        if cue {
                            self.failure = None;
                            self.failure_reported = false;
                        }
                        self.closed = shutdown;
                        events.push(Event::CommandAccepted {
                            command_id: envelope.request_id,
                            command_name,
                        });
                        events.extend(
                            produced
                                .iter()
                                .map(map_broadcast_player_protocol_event)
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    }
                    Err(error) => events.push(Event::CommandRejected {
                        command_id: envelope.request_id,
                        command_name,
                        reason: error.to_string(),
                    }),
                }
            }
        }
        self.reply(sequence, events)
    }
    fn rejected(
        &mut self,
        sequence: u64,
        command_id: String,
        command_name: String,
        reason: String,
    ) -> SessionReply {
        self.reply(
            sequence,
            vec![Event::CommandRejected {
                command_id,
                command_name,
                reason,
            }],
        )
    }
    fn reply(&mut self, sequence: u64, mut events: Vec<Event>) -> SessionReply {
        let state = self.player.state();
        let source_id = state.source.as_ref().map(|source| source.source_id.clone());
        events.extend([
            Event::ActiveSourceChanged {
                source_id: source_id.clone(),
            },
            Event::TransportStatusChanged {
                status: state.status,
            },
            Event::CarrierPositionChanged {
                source_id: source_id.clone(),
                frame: state.carrier_frame,
                range: state.active_range,
                timebase: state.source.as_ref().map(|source| source.timebase),
                status: state.status,
            },
            Event::PlaybackReadinessChanged {
                source_id,
                frame: state.carrier_frame,
                ready: state.play_ready,
            },
            Event::ExecutionRangeChanged {
                range: state.active_range,
            },
        ]);
        if let Some(frame) = state.submitted_frame {
            events.push(Event::VideoFrameSubmitted { frame });
        }
        // The level of each output heard since the last reply (output 1 is A1, output 2 A2).
        for (output, peak) in self.player.take_output_peaks().into_iter().enumerate() {
            events.push(Event::AudioLevelChanged {
                track_id: format!("output_{}", output + 1),
                peak_dbfs_x100: peak_dbfs_x100(peak),
            });
        }
        if let Some(frame) = state.presented_frame {
            events.push(Event::FramePresented { frame });
        }
        if state.at_end
            && let Some(range) = state.active_range
        {
            events.push(Event::PlaybackBoundaryReached {
                frame: range.end_frame,
            });
        }
        if let Some(message) = &self.failure
            && !self.failure_reported
        {
            events.push(Event::PlaybackError {
                message: message.clone(),
            });
            self.failure_reported = true;
        }
        self.event_sequence = sequence;
        Ok(EventEnvelope {
            contract_version: VERSION.into(),
            session_id: self.id.clone(),
            source_generation: self.generation,
            sequence,
            events,
        })
    }
}

/// A sample peak (0..1) in hundredths of dBFS; silence is -100 dBFS.
fn peak_dbfs_x100(peak: f32) -> i32 {
    if peak <= 0.00001 {
        return -10_000;
    }
    (20.0 * peak.log10() * 100.0).round().max(-10_000.0) as i32
}
