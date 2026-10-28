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
        items: vec![
            Item { mark: "Lead-in".into(), role: Some(Role::Anchor), text: "Radnici su danas blokirali promet.".into(), clip: None, length: "0:06".into(), fields: vec![], cues: None, recorded_tc: None },
            Item { mark: "OFF 1".into(), role: Some(Role::Spoken), text: "Unatoč lošem vremenu, više od petsto radnika <okupilo> se.".into(), clip: None, length: "0:12".into(), fields: vec![], cues: None, recorded_tc: Some("18:01:02:03".into()) },
            Item {
                mark: "SOT 1".into(),
                role: Some(Role::Clip),
                text: "Ne odustajemo od naših zahtjeva.".into(),
                clip: Some(("Mironik 2002".into(), "10:00:02:00".into(), "10:00:06:49".into())),
                length: "0:15".into(),
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
    assert!(text.contains("OFF 1 (B-ROLL) (0:12)\nVO TC 18:01:02:03\nUnatoč lošem vremenu"));
    assert!(text.contains("SOT 1: MIRONIK 2002 10:00:02:00 - 10:00:06:49 (0:15)\nCAPTION: NAME: MARKO MARKOVIĆ\n\"Ne odustajemo od naših zahtjeva.\"\nIN: Ne odustajemo od...   OUT: ...od naših zahtjeva."));
}

#[test]
fn the_av_script_has_picture_left_sound_right_and_escapes_the_text() {
    let page = av_html(&story());
    assert!(page.contains("<th>VIDEO</th><th>AUDIO</th><th>DUR</th>"));
    assert!(page.contains("&lt;okupilo&gt;"));
    assert!(!page.contains("<okupilo>"));
    assert!(page.contains("<td class=\"pic\">LEAD-IN (ON CAM)</td><td>Radnici su danas blokirali promet.</td><td class=\"len\">0:06</td>"));
}

#[test]
fn a_mos_story_reads_the_words_as_paragraphs_and_the_cues_as_instructions() {
    let ids = MosIds { mos_id: "qnc.mos".into(), ncs_id: "newsroom".into(), ro_id: "p1".into(), story_id: "p1-script".into(), message_id: 7 };
    let message = mos_xml(&story(), &ids);
    assert!(message.contains("<roStorySend>\n<roID>p1</roID>\n<storyID>p1-script</storyID>\n<storySlug>Prosvjed Split</storySlug>"));
    assert!(message.contains("<p><pi>SOT 1: MIRONIK 2002 10:00:02:00 - 10:00:06:49</pi></p>"));
    assert!(message.contains("<p>Unatoč lošem vremenu, više od petsto radnika &lt;okupilo&gt; se.</p>"));
    assert!(message.contains("<field name=\"IN-CUE\">Unatoč lošem vremenu...</field>"));
    assert_eq!(message.matches("<p>").count(), message.matches("</p>").count());
}
