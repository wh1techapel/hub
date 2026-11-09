//! The closed vocabularies Hub stores as text, and the change notice every write returns.

use serde::Serialize;
use ts_rs::TS;

/// An enum stored as text: the same words in SQLite, in JSON, and in the generated TypeScript.
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, ts_rs::TS)]
        #[ts(export)]
        pub enum $name {
            $(#[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(self.as_str().into())
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
                let text = value.as_str()?;
                Self::parse(text).ok_or_else(|| {
                    rusqlite::types::FromSqlError::Other(
                        format!("unknown {} {text:?}", stringify!($name)).into(),
                    )
                })
            }
        }
    };
}

pub(crate) use text_enum;

text_enum!(
    /// Who made a change. Claude (over MCP) and Gemini act only in Studio (docs/studio.md).
    Actor {
        Owner => "owner",
        Ultron => "ultron",
        Hub => "hub",
        Claude => "claude",
        Gemini => "gemini",
    }
);

text_enum!(
    /// Where a row came from: someone who acts, or an import (ULTRON's old data in CP5).
    /// `claude` and `gemini` exist so every actor has a source, but the tables outside Studio
    /// refuse them (their CHECK lists the first four): Claude and Gemini never write there.
    Source {
        Owner => "owner",
        Ultron => "ultron",
        Hub => "hub",
        Import => "import",
        Claude => "claude",
        Gemini => "gemini",
    }
);

impl From<Actor> for Source {
    fn from(actor: Actor) -> Source {
        match actor {
            Actor::Owner => Source::Owner,
            Actor::Ultron => Source::Ultron,
            Actor::Hub => Source::Hub,
            Actor::Claude => Source::Claude,
            Actor::Gemini => Source::Gemini,
        }
    }
}

text_enum!(
    /// The watercolour hues for projects, routines, and calendar blocks (docs/design.md).
    Pigment {
        Lazuardi => "lazuardi",
        Toska => "toska",
        Lumut => "lumut",
        Kunyit => "kunyit",
        Terakota => "terakota",
        Mawar => "mawar",
        Lavender => "lavender",
        Batu => "batu",
    }
);

// Not derivable: the enum comes from text_enum!.
#[allow(clippy::derivable_impls)]
impl Default for Pigment {
    fn default() -> Pigment {
        Pigment::Batu
    }
}

text_enum!(
    ProjectStatus {
        Active => "active",
        Paused => "paused",
        Done => "done",
        Archived => "archived",
    }
);

text_enum!(
    RoutineKind {
        Lesson => "lesson",
        Habit => "habit",
        Study => "study",
        Other => "other",
    }
);

text_enum!(
    JournalKind {
        Note => "note",
        Done => "done",
        Reflection => "reflection",
    }
);

text_enum!(
    TaskStatus {
        Todo => "todo",
        Doing => "doing",
        Done => "done",
        Cancelled => "cancelled",
    }
);

text_enum!(
    ReminderState {
        Scheduled => "scheduled",
        Fired => "fired",
        Snoozed => "snoozed",
        Done => "done",
        Cancelled => "cancelled",
    }
);

text_enum!(
    /// What happened, in Jejak.
    Verb {
        Created => "created",
        Updated => "updated",
        Completed => "completed",
        Reopened => "reopened",
        Deleted => "deleted",
        Restored => "restored",
        Focused => "focused",
        Attached => "attached",
        Detached => "detached",
        Moved => "moved",
        Reminded => "reminded",
        Snoozed => "snoozed",
        Cancelled => "cancelled",
        Closed => "closed",
        // Studio: a PRD (later a roadmap or a checkpoint) the owner approved.
        Approved => "approved",
    }
);

text_enum!(
    /// What it happened to.
    Entity {
        Settings => "settings",
        Area => "area",
        Project => "project",
        Milestone => "milestone",
        Task => "task",
        Focus => "focus",
        Event => "event",
        Routine => "routine",
        Reminder => "reminder",
        File => "file",
        Collection => "collection",
        Track => "track",
        LearningItem => "learning_item",
        LearningSession => "learning_session",
        CtfEvent => "ctf_event",
        Journal => "journal",
        StudioProject => "studio_project",
        StudioMessage => "studio_message",
        StudioLedger => "studio_ledger",
        StudioStack => "studio_stack",
        StudioPrd => "studio_prd",
        StudioRoadmap => "studio_roadmap",
        StudioCheckpoint => "studio_checkpoint",
        StudioTask => "studio_task",
        StudioReport => "studio_report",
        StudioRequest => "studio_request",
        // R5: Claude Code's sessions (never in Jejak; the windows follow them).
        ClaudeSession => "claude_session",
    }
);

text_enum!(
    /// The Library's kinds, from the extension (files.rs).
    FileKind {
        Document => "document",
        Image => "image",
        Code => "code",
        Archive => "archive",
        Media => "media",
        Folder => "folder",
        Other => "other",
    }
);

text_enum!(
    /// Whether a kept file is where Hub last saw it.
    FileState {
        Ok => "ok",
        Moved => "moved",
        Missing => "missing",
    }
);

text_enum!(
    /// What a file can be attached to.
    LinkKind {
        Project => "project",
        Task => "task",
        LearningItem => "learning_item",
        Event => "event",
        Journal => "journal",
    }
);

text_enum!(
    /// A learning source: a platform (TryHackMe), a CTF, or a lab (school, homelab).
    TrackKind {
        Platform => "platform",
        Ctf => "ctf",
        Lab => "lab",
    }
);

text_enum!(
    LearningKind {
        Room => "room",
        Path => "path",
        Machine => "machine",
        Challenge => "challenge",
        Practical => "practical",
    }
);

text_enum!(
    LearningStatus {
        Todo => "todo",
        Doing => "doing",
        Done => "done",
        Dropped => "dropped",
    }
);

text_enum!(
    Difficulty {
        Easy => "easy",
        Medium => "medium",
        Hard => "hard",
        Insane => "insane",
    }
);

text_enum!(
    /// The security domains Belajar's bars are drawn by (docs/design.md, "Belajar").
    Domain {
        Network => "network",
        Web => "web",
        Linux => "linux",
        Windows => "windows",
        Crypto => "crypto",
        Forensics => "forensics",
        Osint => "osint",
        Reversing => "reversing",
        Pwn => "pwn",
        Other => "other",
    }
);

text_enum!(
    /// How the light is chosen (docs/design.md, "Light").
    ThemeMode {
        Auto => "auto",
        System => "system",
        Light => "light",
        Dark => "dark",
    }
);

text_enum!(
    /// The accent ink the owner picked.
    Ink {
        Biru => "biru",
        Hijau => "hijau",
        Sepia => "sepia",
        Ungu => "ungu",
        Grafit => "grafit",
    }
);

/// What a committed write changed, so the app can tell every window (and later ULTRON) to look
/// again. Ids are the rows touched; an empty list means "the whole kind".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Change {
    pub entity: Entity,
    pub ids: Vec<String>,
}

impl Change {
    pub fn one(entity: Entity, id: impl Into<String>) -> Change {
        Change {
            entity,
            ids: vec![id.into()],
        }
    }

    pub fn all(entity: Entity) -> Change {
        Change {
            entity,
            ids: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_round_trip() {
        for pigment in Pigment::ALL {
            assert_eq!(Pigment::parse(pigment.as_str()), Some(*pigment));
            assert_eq!(
                serde_json::to_string(pigment).unwrap(),
                format!("\"{}\"", pigment.as_str())
            );
        }
        assert_eq!(Entity::LearningItem.as_str(), "learning_item");
        assert_eq!(Verb::parse("nonsense"), None);
    }
}
