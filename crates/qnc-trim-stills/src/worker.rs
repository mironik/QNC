use super::*;

/// Makes the first wanted picture with its neighbours, sends the pass and takes the next
/// one, until nothing is wanted (or the job is cancelled or let go).
pub(crate) fn work(
    send: &mpsc::Sender<Decoded>,
    wanted: &Mutex<Vec<Want>>,
    pass: &Mutex<Option<Instant>>,
    locator: &StillLocator,
    located: &Located,
    cancel: &AtomicBool,
) {
    while !cancel.load(Ordering::Acquire) {
        // Every picture wanted alone at once, one decoder each, each sent when made.
        let alone: Vec<Want> = wanted
            .lock()
            .map(|wanted| wanted.iter().filter(|(_, _, count)| *count == 1).cloned().collect())
            .unwrap_or_default();
        if !alone.is_empty() {
            if let Ok(mut pass) = pass.lock() {
                *pass = Some(Instant::now());
            }
            std::thread::scope(|scope| {
                for (picture, first, count) in &alone {
                    let send = send.clone();
                    scope.spawn(move || {
                        let started = Instant::now();
                        let made: Decoded = match decode_run(picture, *first, *count, locator, located, cancel) {
                            Ok(run) => {
                                log(&format!(
                                    "trim-stills picture {}:{} ms={}",
                                    picture.clip_id,
                                    picture.source_frame,
                                    started.elapsed().as_millis()
                                ));
                                run.into_iter().map(|(frame, image)| (frame, Ok(image))).collect()
                            }
                            Err(_) if cancel.load(Ordering::Acquire) => return,
                            Err(error) => vec![(picture.clone(), Err(error))],
                        };
                        let _ = send.send(made);
                    });
                }
            });
            if let Ok(mut wanted) = wanted.lock() {
                wanted.retain(|want| !alone.contains(want));
            }
            continue;
        }
        let Some((picture, first, count)) = wanted.lock().ok().and_then(|wanted| wanted.first().cloned()) else {
            return;
        };
        let started = Instant::now();
        if let Ok(mut pass) = pass.lock() {
            *pass = Some(started);
        }
        let made: Decoded = match decode_run(&picture, first, count, locator, located, cancel) {
            Ok(run) => {
                log(&format!(
                    "trim-stills run {}:{} pictures={} ms={}",
                    picture.clip_id,
                    picture.source_frame,
                    run.len(),
                    started.elapsed().as_millis()
                ));
                run.into_iter().map(|(frame, image)| (frame, Ok(image))).collect()
            }
            // A cancelled pass is not a failure of the picture: it is asked again later.
            Err(_) if cancel.load(Ordering::Acquire) => return,
            Err(error) => vec![(picture.clone(), Err(error))],
        };
        // Until `fill` brings the next draft, what this pass made is not wanted again.
        if let Ok(mut wanted) = wanted.lock() {
            wanted.retain(|(want, want_first, want_count)| {
                !(made.iter().any(|(done, _)| done == want) || (*want == picture && *want_first == first && *want_count == count))
            });
        }
        if send.send(made).is_err() {
            return;
        }
    }
}

/// `count` pictures of the clip from `first` (the pass that makes `picture`), as decoded.
pub(crate) fn decode_run(
    picture: &StillPicture,
    first: u64,
    count: u32,
    locator: &StillLocator,
    located: &Located,
    cancel: &AtomicBool,
) -> Result<Vec<(StillPicture, Arc<RgbaImage>)>, String> {
    let known = located.lock().ok().and_then(|map| map.get(&picture.clip_id).cloned());
    let place = match known {
        Some(place) => place,
        None => {
            let place = locator.locate(&picture.clip_id)?;
            if let Ok(mut map) = located.lock() {
                map.insert(picture.clip_id.clone(), place.clone());
            }
            place
        }
    };
    // A decoder that stays open (in-process) first; a program without it uses the extractor.
    match open_run::decode(picture, first, count, &place, &open_run::shared(), cancel) {
        Some(Ok(made)) => return Ok(made),
        // Never a black place: the extractor makes it the way it always did.
        Some(Err(error)) => log(&format!("trim-stills open decoder {}:{} {error}", picture.clip_id, picture.source_frame)),
        None => {}
    }
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor()
        .map_err(|error| error.to_string())?
        .ok_or(qnc_ui_text::text("story.msg.no_still_extractor"))?;
    let (num, den) = place.fps;
    // A quarter of a frame early: the first picture at or after it is the frame itself.
    let seek = ((first as f64 - 0.25) * f64::from(den) / f64::from(num.max(1))).max(0.0);
    let scratch = scratch_dir();
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
    let result = (|| {
        extractor.extract_run_with_cancel(&place.file, seek, count, STILL_SIZE, &scratch, cancel)?;
        let mut run = Vec::new();
        for index in 0..u64::from(count) {
            let file = scratch.join(format!("{index:03}.jpg"));
            if !file.is_file() {
                break; // the end of the media
            }
            let frame = StillPicture { clip_id: picture.clip_id.clone(), source_frame: first + index };
            run.push((frame, read_picture(&file)?));
        }
        if !run.iter().any(|(frame, _)| frame == picture) {
            return Err(qnc_ui_text::text("story.msg.still_failed").to_string());
        }
        Ok(run)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

pub(crate) fn read_picture(file: &Path) -> Result<Arc<RgbaImage>, String> {
    let bytes = std::fs::read(file).map_err(|error| qnc_ui_text::fill("story.msg.still_failed_because", &[("error", &error.to_string())]))?;
    qnc_image_assets::decode_thumbnail(&bytes).map(Arc::new)
}

pub(crate) fn scratch_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("qnc-trim-still-{}-{unique}", std::process::id()))
}

pub(crate) fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

/// What a view asks for, for the log: name, marker frame and picture of every edit picture.
pub(crate) fn describe(view: &SegmentsView) -> String {
    let stills = view.edit_stills.iter().chain(&view.source_edit_stills);
    let parts: Vec<String> = stills
        .map(|still| {
            let picture = still.picture.as_ref().map_or("-".into(), |p| format!("{}:{}", p.clip_id, p.source_frame));
            format!("[{} @{} {}]", still.label, still.marker_frame, picture)
        })
        .collect();
    parts.join(" ")
}
