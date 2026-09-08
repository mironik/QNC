use super::*;

#[test]
fn checkbox_and_preview_outline_are_independent() {
    let contracts = IngestContracts::load().unwrap();
    let theme = Theme::from_contract(&contracts.shell);
    for checked in [false, true] {
        for focused in [false, true] {
            let ctx = egui::Context::default();
            let clip = ClipView {
                selected: checked,
                ..Default::default()
            };
            let mut card = Rect::NOTHING;
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    card =
                        render_clip_card(ui, &clip, focused, Vec2::new(200.0, 146.5), &theme).rect;
                });
            });
            let outline = if focused {
                Stroke::new(2.0, theme.danger)
            } else {
                Stroke::new(1.0, theme.border)
            };
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.rect == card && rect.stroke == outline
            )));
            let check = selection_check_rect(card);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.rect == check && rect.fill.is_opaque() == checked
            )), "checkbox appearance must follow checked, not focused");
        }
    }
}

#[test]
fn grid_highlights_only_preview_and_routes_checkbox_separately() {
    let contracts = IngestContracts::load().unwrap();
    let theme = Theme::from_contract(&contracts.shell);
    let ctx = egui::Context::default();
    let view = IngestViewModel {
        clips: vec![
            ClipView {
                clip_id: "a".into(),
                name: "a.mxf".into(),
                selected: true,
                ..Default::default()
            },
            ClipView {
                clip_id: "b".into(),
                name: "b.mxf".into(),
                selected: true,
                ..Default::default()
            },
        ],
        preview_clip_id: Some("b".into()),
        ..Default::default()
    };
    let before = view.clone();
    let paint = |events| {
        let mut intent = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600.0, 400.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    intent = render_clip_grid(ui, &contracts, &theme, &view);
                });
            },
        );
        let cards = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.width() > 160.0
                        && [theme.border, theme.danger].contains(&rect.stroke.color) =>
                {
                    Some((rect.rect, rect.stroke.color))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(cards.len(), 2);
        assert_eq!(
            cards[0].1, theme.border,
            "checked a must not get a red outline"
        );
        assert_eq!(
            cards[1].1, theme.danger,
            "only preview b gets the red outline"
        );
        (cards[0].0, intent)
    };
    let (card, intent) = paint(vec![]);
    assert!(intent.is_none());
    for (point, action) in [
        (
            selection_check_rect(card).center(),
            action_ids::INGEST_CLIP_TOGGLE,
        ),
        (card.center(), action_ids::INGEST_PREVIEW_FOCUS),
    ] {
        let event = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        paint(vec![egui::Event::PointerMoved(point), event(true)]);
        let (_, intent) = paint(vec![event(false)]);
        assert_eq!(
            intent,
            Some(IngestIntent::new(action, IngestPayload::ClipId("a".into())))
        );
    }
    assert_eq!(view, before, "paint only emits intents");
}
