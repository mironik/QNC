use super::*;

fn story() -> Story {
    Story {
        title: "Prosvjed Split".into(),
        header: vec![("Slug".into(), "PROTEST_SPLIT".into()), ("Show".into(), "Vijesti u 19".into())],
        in_cue: "Unatoč lošem vremenu...".into(),
        out_cue: "...Mironik, Vijesti, Split.".into(),
        total: "0:27".into(),
        target: Some("1:30".into()),
        anchor: "0:06".into(),
        labels: Labels { cut_to_package: "[Cut to package]".into(), ..Labels::default() },
        items: vec![
            Item { heading: "Lead-in (on cam)".into(), role: Some(Role::Anchor), text: "Radnici su danas blokirali promet.".into(), speaker: "Anchor (live)".into(), length: "0:06".into(), seconds: 6.0, in_story: false, fields: vec![("Camera".into(), "Kamera 1 (srednji plan)".into())], cues: None, recorded_tc: None },
            Item { heading: "OFF 1 (B-roll)".into(), role: Some(Role::Spoken), text: "Unatoč lošem vremenu, više od petsto radnika <okupilo> se.".into(), speaker: "Reporter (VO)".into(), length: "0:12".into(), seconds: 12.0, in_story: true, fields: vec![("B-roll".into(), "Gužva na tržnici".into())], cues: None, recorded_tc: Some("18:01:02:03".into()) },
            Item {
                heading: "SOT 1: Mironik 2002 10:00:02:00 - 10:00:06:49".into(),
                role: Some(Role::Clip),
                text: "Ne odustajemo od naših zahtjeva.".into(),
                speaker: "Ivan Marić (SOT)".into(),
                length: "0:15".into(),
                seconds: 15.0,
                in_story: true,
                fields: vec![("Caption: name".into(), "Marko Marković".into())],
                cues: Some(("Ne odustajemo od...".into(), "...od naših zahtjeva.".into())),
                recorded_tc: None,
            },
        ],
    }
}

#[test]
fn directions_are_in_capitals_and_the_words_read_as_written() {
    let text = text(&story());
    assert!(text.starts_with("PROSVJED SPLIT\nSLUG: PROTEST_SPLIT\n"));
    assert!(text.contains("TRT: 0:27 / 1:30"));
    assert!(text.contains("LEAD-IN (ON CAM) (0:06)\nCAMERA: Kamera 1 (srednji plan)\nANCHOR (LIVE):\nRadnici su"));
    assert!(text.contains("[CUT TO PACKAGE]\n0:00 - 0:12\nOFF 1 (B-ROLL) (0:12)\nB-ROLL: Gužva na tržnici\nVO TC 18:01:02:03\nREPORTER (VO):\nUnatoč lošem vremenu"));
    assert!(text.contains("0:12 - 0:27\nSOT 1: MIRONIK 2002 10:00:02:00 - 10:00:06:49 (0:15)\nCAPTION: NAME: Marko Marković\nIVAN MARIĆ (SOT):\n\"Ne odustajemo od naših zahtjeva.\"\nIN: Ne odustajemo od...   OUT: ...od naših zahtjeva."));
}

#[test]
fn the_av_script_has_picture_left_sound_right_and_escapes_the_text() {
    let page = av_html(&story());
    assert!(page.contains("<th>VIDEO</th><th>AUDIO</th><th>DUR</th>"));
    assert!(page.contains("&lt;okupilo&gt;"));
    assert!(!page.contains("<okupilo>"));
    assert!(!page.contains("text-transform"), "only directions are in capitals, what a field says is as written");
    assert!(page.contains("<td class=\"pic\">LEAD-IN (ON CAM)<br>CAMERA: Kamera 1 (srednji plan)</td><td><b>ANCHOR (LIVE):</b><br>Radnici su danas blokirali promet.</td><td class=\"len\">0:06</td>"));
    assert!(page.contains("<td class=\"pic\">[CUT TO PACKAGE]<br>0:00 - 0:12<br>OFF 1 (B-ROLL)"));
}

#[test]
fn a_mos_story_reads_the_words_as_paragraphs_and_the_cues_as_instructions() {
    let ids = MosIds { mos_id: "qnc.mos".into(), ncs_id: "newsroom".into(), ro_id: "p1".into(), story_id: "p1-script".into(), message_id: 7 };
    let message = mos_xml(&story(), &ids);
    assert!(message.contains("<roStorySend>\n<roID>p1</roID>\n<storyID>p1-script</storyID>\n<storySlug>Prosvjed Split</storySlug>"));
    assert!(message.contains("<p><pi>SOT 1: MIRONIK 2002 10:00:02:00 - 10:00:06:49</pi></p>"));
    assert!(message.contains("<p><pi>IVAN MARIĆ (SOT):</pi></p>"));
    assert!(message.contains("<p>Unatoč lošem vremenu, više od petsto radnika &lt;okupilo&gt; se.</p>"));
    assert!(message.contains("<field name=\"IN-CUE\">Unatoč lošem vremenu...</field>"));
    assert_eq!(message.matches("<p>").count(), message.matches("</p>").count());
}

#[test]
fn every_word_of_the_written_out_script_comes_from_the_profile() {
    let mut story = story();
    story.labels = Labels {
        video: "SLIKA".into(),
        audio: "TON".into(),
        duration: "TRAJ".into(),
        in_cue: "ULAZ".into(),
        out_cue: "IZLAZ".into(),
        trt: "UKUPNO".into(),
        anchor: "VODITELJ".into(),
        recorded: "SNIMKA {tc}".into(),
        cues: "od {in} do {out}".into(),
        cut_to_package: "".into(),
    };
    let page = av_html(&story);
    assert!(page.contains("<th>SLIKA</th><th>TON</th><th>TRAJ</th>"));
    let text = text(&story);
    assert!(text.contains("UKUPNO: 0:27 / 1:30"));
    assert!(text.contains("SNIMKA 18:01:02:03"));
    assert!(text.contains("od Ne odustajemo od... do ...od naših zahtjeva."));
    assert!(!text.contains("CUT TO PACKAGE"));
    assert!(!text.contains("VO TC") && !text.contains("IN-CUE"));
}
