use super::*;

const MEDIAPRO_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MediaProfile xmlns="http://xmlns.sony.net/pro/metadata/mediaprofile" version="2.30">
	<Properties>
		<System systemKind="ILME-FX6V ver.6.000"/>
	</Properties>
	<Contents>
		<Material uri="./Clip/Mironik 3396.MXF" type="MXF" offset="0" dur="1376" fps="50p" ch="4"
			umid="060A2B340101010501010D43130000005C4CD04B201306DE5026EFFFFEB8895C"
			status="none">
			<Proxy uri="./Sub/Mironik 3396S03.MP4" type="MP4" dur="1376"
				umid="060A2B340101010501010D4313FF00005C4CD04B201306DE5026EFFFFEB8895C"/>
			<RelevantInfo uri="./Clip/Mironik 3396M01.XML" type="XML"/>
			<RelevantInfo uri="./Thmbnl/Mironik 3396T01.JPG" type="JPG"/>
		</Material>
	</Contents>
</MediaProfile>
"#;

const SIDECAR: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20">
	<TargetMaterial umidRef="060A2B340101010501010D43130000005C4CD04B201306DE5026EFFFFEB8895C"/>
	<Duration value="1376"/>
	<LtcChangeTable tcFps="25" halfStep="true">
		<LtcChange frameCount="0" value="08491406" status="increment"/>
		<LtcChange frameCount="1375" value="20161586" status="end"/>
	</LtcChangeTable>
	<BodyUmidWhenChangeTable>
		<BodyUmidWhenChange frameCount="0" value="0A61E104201306A5" status="increment"/>
		<BodyUmidWhenChange frameCount="1375" value="CAB8E204201306A5" status="end"/>
	</BodyUmidWhenChangeTable>
</NonRealTimeMeta>
"#;

fn card() -> (tempfile::TempDir, CardClip, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("card/XDROOT");
    fs::create_dir_all(source.join("Clip")).unwrap();
    fs::write(source.join("MEDIAPRO.XML"), MEDIAPRO_XML).unwrap();
    fs::write(source.join("Clip/Mironik 3396.MXF"), b"mxf").unwrap();
    fs::write(source.join("Clip/Mironik 3396M01.XML"), SIDECAR).unwrap();
    let clip = card_of(&dir.path().join("card"), &source.join("Clip/Mironik 3396.MXF")).unwrap();
    let dest = dir.path().join("parts").join(&clip.card_relative);
    (dir, clip, dest)
}

#[test]
fn a_clip_is_found_on_its_card_and_its_part_is_named_after_it() {
    let (_dir, clip, dest) = card();
    assert_eq!((clip.card_relative.to_str(), clip.material_uri.as_str()), (Some("XDROOT"), "./Clip/Mironik 3396.MXF"));
    let files = part_files(&clip, &dest, 1).unwrap();
    assert_eq!((files.stem.as_str(), files.material_uri.as_str()), ("Mironik 3396_01", "./Clip/Mironik 3396_01.MXF"));
    assert!(files.media.ends_with("XDROOT/Clip/Mironik 3396_01.MXF"));
}

#[test]
fn the_documents_of_a_part_say_its_length_umid_and_timecode() {
    let (_dir, clip, dest) = card();
    let files = part_files(&clip, &dest, 2).unwrap();
    write_documents(&clip, &dest, &files, PartFrames { number: 2, first: 434, frames: 526 }, None).unwrap();
    let index = fs::read_to_string(dest.join("MEDIAPRO.XML")).unwrap();
    assert!(index.contains(r#"<Material uri="./Clip/Mironik 3396_02.MXF""#) && index.contains(r#"dur="526""#));
    assert!(index.contains("060A2B340101010501010D4313000002"), "a UMID of its own");
    assert!(!index.contains("<Proxy") && index.contains("./Clip/Mironik 3396_02M01.XML") && index.contains("ILME-FX6V"));
    let sidecar = fs::read_to_string(dest.join("Clip/Mironik 3396_02M01.XML")).unwrap();
    assert!(sidecar.contains(r#"<Duration value="526"/>"#));
    // 06:14:49:16 at 50p + 434 frames = 06:14:58:00; its last frame 06:15:08:25 (+525).
    assert!(sidecar.contains(r#"<LtcChange frameCount="0" value="00581406" status="increment"/>"#), "{sidecar}");
    assert!(sidecar.contains(r#"<LtcChange frameCount="525" value="12081586" status="end"/>"#), "{sidecar}");
    assert!(sidecar.contains(r#"<BodyUmidWhenChange frameCount="525" value="CAB8E204201306A5" status="end"/>"#));
    // A second part goes to the same card beside it.
    let other = part_files(&clip, &dest, 1).unwrap();
    write_documents(&clip, &dest, &other, PartFrames { number: 1, first: 0, frames: 100 }, None).unwrap();
    let index = fs::read_to_string(dest.join("MEDIAPRO.XML")).unwrap();
    assert!(index.contains("Mironik 3396_01.MXF") && index.contains("Mironik 3396_02.MXF"));
}

#[test]
fn the_ltc_of_a_frame_is_written_as_the_camera_writes_it() {
    for frames in [0, 1, 49, 50, 1_123_456] {
        assert_eq!(ltc_to_frames(&frames_to_ltc(frames, 50, 25, true), 25, true).unwrap(), frames);
    }
    assert_eq!(ltc_to_frames("08491406", 25, true).unwrap(), ((6 * 60 + 14) * 60 + 49) * 50 + 16);
}
