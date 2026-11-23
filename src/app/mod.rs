//! Application state, the Activity Manager, island choreography and input.

pub mod activity;
pub mod events;
pub mod input;
pub mod island;
pub mod state;

use crate::services::media::MediaCmd;
use crate::theme::tokens::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageId {
    Dashboard,
    Music,
    Lyrics,
    Timer,
    Water,
    Todo,
    Shelf,
    Clipboard,
    Prayer,
    Ocr,
    Ask,
    Weather,
    Calendar,
    Notifications,
    Privacy,
    SysMon,
    Notes,
    Meeting,
    Downloads,
    Live,
    Onboarding,
    Annotate,
    Widgets,
    Control,
}

impl PageId {
    pub fn shape(self) -> ShapeSpec {
        use PageId::*;
        match self {
            Lyrics | Shelf | Annotate => SHAPE_WIDE,
            Ask => SHAPE_WIDE,
            Todo | Clipboard | Calendar | Notifications | Privacy | Notes | Widgets | Downloads => {
                SHAPE_TALL
            }
            Onboarding => ShapeSpec {
                h: 176.0,
                ..SHAPE_EXPANDED
            },
            Prayer => ShapeSpec {
                h: 172.0,
                ..SHAPE_EXPANDED
            },
            Dashboard => ShapeSpec {
                h: 188.0,
                ..SHAPE_EXPANDED
            },
            Weather => ShapeSpec {
                w: 540.0,
                h: 256.0,
                ..SHAPE_WIDE
            },
            Control => ShapeSpec {
                w: 520.0,
                h: 244.0,
                ..SHAPE_WIDE
            },
            Ocr => ShapeSpec {
                h: 200.0,
                ..SHAPE_WIDE
            },
            _ => SHAPE_EXPANDED,
        }
    }
    pub fn title(self) -> &'static str {
        use PageId::*;
        match self {
            Dashboard => "Home",
            Control => "Control Center",
            Music => "Now playing",
            Lyrics => "Lyrics",
            Timer => "Focus timer",
            Water => "Water",
            Todo => "To-do",
            Shelf => "Shelf",
            Clipboard => "Clipboard",
            Prayer => "Prayer times",
            Ocr => "Text from screen",
            Ask => "Ask",
            Weather => "Weather",
            Calendar => "Agenda",
            Notifications => "Notifications",
            Privacy => "Privacy",
            SysMon => "System",
            Notes => "Notes",
            Meeting => "Meeting",
            Downloads => "Downloads",
            Live => "Live activity",
            Onboarding => "Welcome",
            Annotate => "Screenshot",
            Widgets => "Widgets",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SliderKind {
    Seek,
    Volume,
    Brightness,
    AppVolume,
    SettingsSlider(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimerCmd {
    StartFocus,
    Pause,
    Resume,
    Skip,
    Reset,
    Preset(u32),
    Plus5,
    StartBreak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputId {
    Todo,
    ClipSearch,
    Ask,
    Note,
    Settings(u16),
}

/// Everything a click, key or pop-up button can do.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    None,
    Open(PageId),
    PageDot(usize),
    Collapse,
    TogglePin,
    SwapBubble,
    OpenSettings,
    Media(MediaCmdKey),
    Session(usize),
    Slider(SliderKind),
    LyricLine(usize),
    LyricsTranslate,
    PopupButton(u32, u8),
    PopupDismiss,
    Timer(TimerCmd),
    WaterAdd,
    WaterUndo,
    WaterDrank,
    WaterSnooze,
    WaterSkip,
    TodoToggle(u64),
    TodoDelete(u64),
    TodoSnooze(u64),
    /// A to-do row (drag handle for reordering).
    TodoRow(u64),
    /// To-do filter tab.
    TodoFilter(u8),
    /// Removes every completed to-do.
    TodoClearDone,
    /// Clear unpinned clipboard items.
    ClipClear,
    Focus(InputId),
    ShelfSelect(u64),
    ShelfOpen(u64),
    ShelfReveal(u64),
    ShelfCopy(u64),
    ShelfShare(u64),
    ShelfRemove(u64),
    ShelfClear,
    ShelfReceive,
    ShelfSendPhone,
    ClipCopy(u64),
    ClipPin(u64),
    ClipDelete(u64),
    PrayerPause,
    OcrStart,
    OcrCopy,
    OcrSearch,
    OcrTranslate,
    OcrOpenUrl(u8),
    OcrSaveImage,
    AskSend,
    AskStop,
    AskCopy,
    AskInsert,
    AskRetry,
    AskNew,
    AskMode(u8),
    AskChip(u8),
    AskSwapLang,
    PaletteItem(u16),
    Tile(u8),
    NotifClear,
    NotifEnable,
    LiveButton(String, String),
    LiveAllow(String, bool),
    MeetingMute,
    MeetingLeave,
    MeetingCamera,
    DownloadOpen(u64),
    DownloadReveal(u64),
    DownloadShelf(u64),
    NoteNew,
    NoteSelect(u64),
    NotePin(u64),
    NoteExport(u64),
    NoteDelete(u64),
    Onboarding(u8),
    OnboardingToggle(u8),
    Mascot,
    WeatherRefresh,
    CalendarJoin(u64),
    CalendarRefresh,
    AnnotateTool(u8),
    AnnotateUndo,
    AnnotateDone,
    AnnotateSave,
    WidgetToggle(u8),
    Privacy,
    ScrollTo(i32),
    /// Settings window control: (item id, value/sub-index).
    Settings(u16, u16),
    /// Opens a URL, ms-settings: link or folder.
    OpenUrl(String),
    /// Copies text to the clipboard (with a small confirmation).
    CopyText(String),
    /// Installs the downloaded, verified update and restarts.
    InstallUpdate,
    /// A passive list row (hover springs only): (list, item key).
    Row(u8, u64),
    /// Control Center switch (see `ui::control`).
    Ctl(u8),
    LockPc,
    Screenshot,
}

/// Hashable media command (MediaCmd carries data that isn't hashable).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaCmdKey {
    PlayPause,
    Next,
    Prev,
    Shuffle,
    Repeat,
}

impl MediaCmdKey {
    pub fn to_cmd(self, shuffle: Option<bool>, repeat: Option<u8>) -> MediaCmd {
        match self {
            MediaCmdKey::PlayPause => MediaCmd::PlayPause,
            MediaCmdKey::Next => MediaCmd::Next,
            MediaCmdKey::Prev => MediaCmd::Prev,
            MediaCmdKey::Shuffle => MediaCmd::Shuffle(!shuffle.unwrap_or(false)),
            MediaCmdKey::Repeat => MediaCmd::Repeat((repeat.unwrap_or(0) + 1) % 3),
        }
    }
}

/// Requests from state to the platform layer (windows, focus, OS calls).
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    OpenSettings,
    TakeFocus,
    RestoreFocus,
    StartCapture(CaptureMode),
    ApplySettings,
    RegisterHotkeys,
    PasteToPrevious,
    CopyText(String),
    CopyImage(u64),
    OpenPath(String),
    RevealPath(String),
    OpenUrl(String),
    InstallUpdate,
    ShareShelf(u64),
    DragOutShelf(Vec<u64>),
    Chime(Chime),
    Announce(String),
    UpdateTray,
    TrimMemory,
    SendKeys(Vec<u16>),
    DesktopWidgets,
    SaveImage(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    Ocr,
    Screenshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chime {
    TimerEnd,
    Reminder,
    Tap,
    Success,
}
