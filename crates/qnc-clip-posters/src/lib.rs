//! The posters of clips, loaded in the background.
//!
//! Procedure of QNC v5: the chosen clip is asked for first and the others follow in the
//! order of the list; a poster is read from where the project database says it is, in the
//! project folder when it was copied, otherwise on the source (link). A source that is not
//! there (no card, another card) leaves the placeholder; nothing is invented and nothing
//! is written. The caller only says which clips, which is chosen, and polls for pictures.

use qnc_image_assets::RgbaImage;
use qnc_media_thumbnail::{ProjectFolder, ThumbnailBatchService, ThumbnailEvent, ThumbnailRequest};
use qnc_source_bindings::SourceBinding;
use qnc_source_reader::SourceReader;
use std::{collections::HashSet, sync::Arc};

pub const MODULE_ID: &str = "qnc.module.clip-posters";
pub const VERSION: &str = "0.1.0";

/// A loaded poster.
#[derive(Debug, Clone)]
pub struct Poster {
    pub clip_id: String,
    pub image: Arc<RgbaImage>,
}

#[derive(Default)]
pub struct ClipPosters {
    service: ThumbnailBatchService,
    sources: Vec<SourceBinding>,
    project: Option<ProjectFolder>,
    /// (clip id, poster address) still to load, in the order they are asked for.
    wanted: Vec<(String, String)>,
    loaded: HashSet<String>,
}

impl ClipPosters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where posters can be read from: the sources and the project folder of this machine.
    pub fn configure(&mut self, sources: &[SourceBinding], project: Option<ProjectFolder>) {
        self.sources = sources.to_vec();
        self.project = project;
    }

    /// Forgets everything loaded (another project).
    pub fn reset(&mut self) {
        self.service.cancel();
        self.wanted.clear();
        self.loaded.clear();
    }

    /// The clips to show, in list order, with the address of their poster. Clips whose
    /// poster is already loaded are not asked for again; the chosen clip is asked first.
    pub fn request(&mut self, clips: Vec<(String, String)>, chosen: Option<&str>) {
        let present: HashSet<&str> = clips.iter().map(|(id, _)| id.as_str()).collect();
        self.loaded.retain(|id| present.contains(id.as_str()));
        self.wanted = clips
            .into_iter()
            .filter(|(id, _)| !self.loaded.contains(id))
            .collect();
        self.start(chosen);
    }

    /// The user chose a clip: its poster goes first, unless it is loaded or being read.
    pub fn prioritize(&mut self, clip_id: &str) {
        if self
            .wanted
            .first()
            .is_some_and(|(first, _)| first == clip_id)
        {
            return;
        }
        if self.wanted.iter().any(|(id, _)| id == clip_id) {
            self.start(Some(clip_id));
        }
    }

    fn start(&mut self, chosen: Option<&str>) {
        if let Some(chosen) = chosen {
            if let Some(index) = self.wanted.iter().position(|(id, _)| id == chosen) {
                let entry = self.wanted.remove(index);
                self.wanted.insert(0, entry);
            }
        }
        if self.wanted.is_empty() {
            self.service.cancel();
            return;
        }
        let sources: Vec<SourceReader> = self.sources.iter().filter_map(reader).collect();
        let requests = self
            .wanted
            .iter()
            .map(|(id, uri)| ThumbnailRequest {
                item_id: id.clone(),
                uri: uri.clone(),
            })
            .collect();
        // A failed start leaves the placeholders; the next request tries again.
        let _ = self
            .service
            .start_with_project(sources, self.project.clone(), requests);
    }

    /// Pictures that arrived since the last poll.
    pub fn poll(&mut self) -> Vec<Poster> {
        let mut posters = Vec::new();
        for event in self.service.poll(16) {
            if let ThumbnailEvent::Ready { item_id, image, .. } = event {
                self.wanted.retain(|(id, _)| id != &item_id);
                self.loaded.insert(item_id.clone());
                posters.push(Poster {
                    clip_id: item_id,
                    image,
                });
            }
        }
        posters
    }

    pub fn has_pending_work(&self) -> bool {
        self.service.has_pending_work()
    }
}

/// A reader of one source; a source that is not there is skipped, never guessed.
fn reader(binding: &SourceBinding) -> Option<SourceReader> {
    binding.resolver().ok()?;
    if let Some(path) = &binding.file {
        // The address of a volume names its serial number: another card in the same slot
        // must not show its pictures under the clips of this one.
        let serial = binding
            .uri
            .rsplit('/')
            .next()
            .and_then(|id| id.strip_prefix("volume-"))
            .unwrap_or("");
        qnc_dir_browser::verify_local_volume_serial(path, serial).ok()?;
        SourceReader::local(&binding.uri, path).ok()
    } else {
        SourceReader::remote(
            &binding.uri,
            binding.endpoint.as_deref()?,
            binding.token().ok()??.as_str(),
        )
        .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8,
        0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60,
    ];

    const SOURCE: &str = "qnc://local/source/card";
    const ROOT: &str = "qnc://local/project/p1";

    fn folder() -> (tempfile::TempDir, ProjectFolder) {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a", "b", "c"] {
            let posters = dir.path().join("ingest").join("thumbnails").join(name);
            std::fs::create_dir_all(&posters).unwrap();
            std::fs::write(posters.join("poster.jpg"), PNG_1X1).unwrap();
        }
        let project = ProjectFolder {
            root_uri: ROOT.into(),
            dir: dir.path().to_path_buf(),
        };
        (dir, project)
    }

    fn address(name: &str) -> String {
        format!("{ROOT}/ingest/thumbnails/{name}/poster.jpg")
    }

    fn wait_for(posters: &mut ClipPosters, count: usize) -> Vec<String> {
        let started = Instant::now();
        let mut order = Vec::new();
        while order.len() < count {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "posters never arrived"
            );
            order.extend(posters.poll().into_iter().map(|p| p.clip_id));
            std::thread::sleep(Duration::from_millis(5));
        }
        order
    }

    #[test]
    fn the_chosen_clip_comes_first_then_the_list_in_order() {
        let (_dir, project) = folder();
        let mut posters = ClipPosters::new();
        posters.configure(&[], Some(project));
        posters.request(
            vec![
                ("a".into(), address("a")),
                ("b".into(), address("b")),
                ("c".into(), address("c")),
            ],
            Some("c"),
        );
        assert_eq!(wait_for(&mut posters, 3), ["c", "a", "b"]);
    }

    #[test]
    fn a_loaded_poster_is_not_asked_for_again() {
        let (_dir, project) = folder();
        let mut posters = ClipPosters::new();
        posters.configure(&[], Some(project));
        let clips = vec![
            ("a".to_string(), address("a")),
            ("b".to_string(), address("b")),
        ];
        posters.request(clips.clone(), None);
        wait_for(&mut posters, 2);
        posters.request(clips, Some("b"));
        std::thread::sleep(Duration::from_millis(100));
        assert!(posters.poll().is_empty());
        assert!(!posters.has_pending_work() || posters.wanted.is_empty());
    }

    #[test]
    fn a_poster_on_a_source_that_is_not_there_leaves_the_placeholder() {
        let mut posters = ClipPosters::new();
        posters.configure(&[], None);
        posters.request(
            vec![("a".into(), format!("{SOURCE}/file/Thmbnl/a.JPG"))],
            Some("a"),
        );
        std::thread::sleep(Duration::from_millis(100));
        assert!(posters.poll().is_empty());
    }

    #[test]
    fn choosing_a_clip_moves_it_to_the_front_of_the_rest() {
        let (_dir, project) = folder();
        let mut posters = ClipPosters::new();
        posters.configure(&[], Some(project));
        posters.wanted = vec![
            ("a".into(), address("a")),
            ("b".into(), address("b")),
            ("c".into(), address("c")),
        ];
        posters.prioritize("c");
        assert_eq!(posters.wanted[0].0, "c");
        assert_eq!(posters.wanted.len(), 3);
        wait_for(&mut posters, 3);
    }
}
