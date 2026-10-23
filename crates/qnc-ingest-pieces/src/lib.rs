//! The pieces of the Ingest form, in the order they are asked (user 2026-10-23, way B: a
//! new possibility of Ingest is a new piece here, not new lines of the application). This
//! is only the list, like the shell's table of desktop adapters; each piece is its own block.

use qnc_form_pieces::Pieces;

pub const MODULE_ID: &str = "qnc.module.ingest-pieces";

/// The pieces of Ingest with the action ids of its keyboard catalog and layout contract.
#[derive(Debug)]
pub struct IngestPieces(pub Pieces);

impl Default for IngestPieces {
    fn default() -> Self {
        Self(Pieces::new(vec![
            // The parts first: a tick of a part card is the part's.
            Box::new(qnc_ingest_parts::PartsPiece::new(qnc_ingest_parts::PartActions {
                save: "save_virtual_shot",
                delete: "ingest_subclip_delete",
                open: "ingest_preview_focus",
                tab: "ingest_tab_virtual",
                toggle: "ingest_clip_toggle",
                import: "ingest_import_selected",
            })),
            Box::new(qnc_pool_marks::PoolMarks {
                toggle: "ingest_clip_toggle",
                all: "ingest_select_all",
                clear: "ingest_clear_selection",
                tabs: &["ingest_tab_all", "ingest_tab_virtual"],
            }),
        ]))
    }
}

impl std::ops::Deref for IngestPieces {
    type Target = Pieces;
    fn deref(&self) -> &Pieces {
        &self.0
    }
}

impl std::ops::DerefMut for IngestPieces {
    fn deref_mut(&mut self) -> &mut Pieces {
        &mut self.0
    }
}
