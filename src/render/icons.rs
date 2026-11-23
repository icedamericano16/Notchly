//! Original Notchly line icon set: 20 px grid, 1.75 px stroke, round caps/joins.
//! Each icon is SVG path data parsed once into a Direct2D path geometry, so icons
//! stay crisp at every scale. Paths prefixed with `F:` are filled glyphs.

use std::collections::HashMap;

use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::Vector2;

/// The complete icon set (section 4); some glyphs are reserved for future pages.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Play,
    Pause,
    Next,
    Previous,
    Shuffle,
    Repeat,
    RepeatOne,
    Volume0,
    Volume1,
    Volume2,
    Volume3,
    Mute,
    Brightness,
    Keyboard,
    Caps,
    NumLock,
    Wifi0,
    Wifi1,
    Wifi2,
    Wifi3,
    WifiOff,
    Ethernet,
    Bluetooth,
    Headphones,
    Speaker,
    Bolt,
    Timer,
    Droplet,
    CheckCircle,
    Check,
    List,
    Shelf,
    Clipboard,
    MoonStar,
    ScanText,
    Calculator,
    Sparkles,
    Translate,
    Sun,
    Cloud,
    CloudSun,
    Rain,
    Snow,
    Storm,
    Fog,
    Calendar,
    Widgets,
    Mic,
    MicOff,
    Camera,
    Location,
    Phone,
    Download,
    Settings,
    Pin,
    Close,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Search,
    Plus,
    Minus,
    Trash,
    Copy,
    ExternalLink,
    Hammer,
    Note,
    File,
    Folder,
    Link,
    Image,
    Text,
    Palette,
    Bell,
    Cpu,
    Globe,
    Shield,
    Music,
    Undo,
    Skip,
    Reset,
    Send,
    Qr,
    Crop,
    Arrow,
    Square,
    Highlighter,
    Eye,
    Leaf,
    Star,
    Lock,
    Coffee,
    Contrast,
    Sliders,
    Info,
    Home,
    Heart,
    Airplane,
}

/// Every icon (QA sheets, audits).
pub const ALL: &[Icon] = &[
    Icon::Play,
    Icon::Pause,
    Icon::Next,
    Icon::Previous,
    Icon::Shuffle,
    Icon::Repeat,
    Icon::RepeatOne,
    Icon::Volume0,
    Icon::Volume1,
    Icon::Volume2,
    Icon::Volume3,
    Icon::Mute,
    Icon::Brightness,
    Icon::Keyboard,
    Icon::Caps,
    Icon::NumLock,
    Icon::Wifi0,
    Icon::Wifi1,
    Icon::Wifi2,
    Icon::Wifi3,
    Icon::WifiOff,
    Icon::Ethernet,
    Icon::Bluetooth,
    Icon::Headphones,
    Icon::Speaker,
    Icon::Bolt,
    Icon::Timer,
    Icon::Droplet,
    Icon::CheckCircle,
    Icon::Check,
    Icon::List,
    Icon::Shelf,
    Icon::Clipboard,
    Icon::MoonStar,
    Icon::ScanText,
    Icon::Calculator,
    Icon::Sparkles,
    Icon::Translate,
    Icon::Sun,
    Icon::Cloud,
    Icon::CloudSun,
    Icon::Rain,
    Icon::Snow,
    Icon::Storm,
    Icon::Fog,
    Icon::Calendar,
    Icon::Widgets,
    Icon::Mic,
    Icon::MicOff,
    Icon::Camera,
    Icon::Location,
    Icon::Phone,
    Icon::Download,
    Icon::Settings,
    Icon::Pin,
    Icon::Close,
    Icon::ChevronLeft,
    Icon::ChevronRight,
    Icon::ChevronDown,
    Icon::Search,
    Icon::Plus,
    Icon::Minus,
    Icon::Trash,
    Icon::Copy,
    Icon::ExternalLink,
    Icon::Hammer,
    Icon::Note,
    Icon::File,
    Icon::Folder,
    Icon::Link,
    Icon::Image,
    Icon::Text,
    Icon::Palette,
    Icon::Bell,
    Icon::Cpu,
    Icon::Globe,
    Icon::Shield,
    Icon::Music,
    Icon::Undo,
    Icon::Skip,
    Icon::Reset,
    Icon::Send,
    Icon::Qr,
    Icon::Crop,
    Icon::Arrow,
    Icon::Square,
    Icon::Highlighter,
    Icon::Eye,
    Icon::Leaf,
    Icon::Star,
    Icon::Lock,
    Icon::Coffee,
    Icon::Contrast,
    Icon::Sliders,
    Icon::Info,
    Icon::Home,
    Icon::Heart,
    Icon::Airplane,
];

pub fn path(icon: Icon) -> &'static str {
    use Icon::*;
    match icon {
        Play => "F:M6.5 3.8 Q5.5 3.3 5.5 4.5 L5.5 15.5 Q5.5 16.7 6.5 16.2 L15.8 10.8 Q16.8 10 15.8 9.2 Z",
        Pause => "F:M5 4 Q5 3.2 5.8 3.2 L7.7 3.2 Q8.5 3.2 8.5 4 L8.5 16 Q8.5 16.8 7.7 16.8 L5.8 16.8 Q5 16.8 5 16 Z M11.5 4 Q11.5 3.2 12.3 3.2 L14.2 3.2 Q15 3.2 15 4 L15 16 Q15 16.8 14.2 16.8 L12.3 16.8 Q11.5 16.8 11.5 16 Z",
        Next => "F:M3.5 5 Q3.5 4 4.4 4.6 L11.6 9.3 Q12.4 10 11.6 10.7 L4.4 15.4 Q3.5 16 3.5 15 Z M13.8 4.5 Q13.8 4 14.3 4 L15.8 4 Q16.3 4 16.3 4.5 L16.3 15.5 Q16.3 16 15.8 16 L14.3 16 Q13.8 16 13.8 15.5 Z",
        Previous => "F:M16.5 5 Q16.5 4 15.6 4.6 L8.4 9.3 Q7.6 10 8.4 10.7 L15.6 15.4 Q16.5 16 16.5 15 Z M6.2 4.5 Q6.2 4 5.7 4 L4.2 4 Q3.7 4 3.7 4.5 L3.7 15.5 Q3.7 16 4.2 16 L5.7 16 Q6.2 16 6.2 15.5 Z",
        Shuffle => "M2.5 6 L5 6 C8.5 6 10 14 14 14 L17 14 M2.5 14 L5 14 C6.6 14 7.6 12.6 8.4 11 M11.6 9 C12.3 7.4 13.1 6 14 6 L17 6 M15 4 L17 6 L15 8 M15 12 L17 14 L15 16",
        Repeat => "M3.5 9.5 L3.5 8.5 A3 3 0 0 1 6.5 5.5 L16 5.5 M13.5 3 L16 5.5 L13.5 8 M16.5 10.5 L16.5 11.5 A3 3 0 0 1 13.5 14.5 L4 14.5 M6.5 17 L4 14.5 L6.5 12",
        RepeatOne => "M3.5 9.5 L3.5 8.5 A3 3 0 0 1 6.5 5.5 L16 5.5 M13.5 3 L16 5.5 L13.5 8 M16.5 10.5 L16.5 11.5 A3 3 0 0 1 13.5 14.5 L4 14.5 M6.5 17 L4 14.5 L6.5 12 M9.3 9 L10.3 8.3 L10.3 12",
        Volume0 => "M2.8 7.6 L5.6 7.6 L9.5 4.3 L9.5 15.7 L5.6 12.4 L2.8 12.4 Z",
        Volume1 => "M2.8 7.6 L5.6 7.6 L9.5 4.3 L9.5 15.7 L5.6 12.4 L2.8 12.4 Z M12.4 7.8 A3.2 3.2 0 0 1 12.4 12.2",
        Volume2 => "M2.8 7.6 L5.6 7.6 L9.5 4.3 L9.5 15.7 L5.6 12.4 L2.8 12.4 Z M12.4 7.8 A3.2 3.2 0 0 1 12.4 12.2 M14.6 5.6 A6.3 6.3 0 0 1 14.6 14.4",
        Volume3 => "M2.8 7.6 L5.6 7.6 L9.5 4.3 L9.5 15.7 L5.6 12.4 L2.8 12.4 Z M12.4 7.8 A3.2 3.2 0 0 1 12.4 12.2 M14.6 5.6 A6.3 6.3 0 0 1 14.6 14.4 M16.8 3.4 A9.4 9.4 0 0 1 16.8 16.6",
        Mute => "M2.8 7.6 L5.6 7.6 L9.5 4.3 L9.5 15.7 L5.6 12.4 L2.8 12.4 Z M12.8 7.8 L17.2 12.2 M17.2 7.8 L12.8 12.2",
        Brightness | Sun => "M13 10 A3 3 0 1 1 7 10 A3 3 0 1 1 13 10 Z M10 2.5 L10 4 M10 16 L10 17.5 M2.5 10 L4 10 M16 10 L17.5 10 M4.7 4.7 L5.8 5.8 M14.2 14.2 L15.3 15.3 M4.7 15.3 L5.8 14.2 M14.2 5.8 L15.3 4.7",
        Keyboard => "M3 5.5 L17 5.5 Q18.5 5.5 18.5 7 L18.5 13 Q18.5 14.5 17 14.5 L3 14.5 Q1.5 14.5 1.5 13 L1.5 7 Q1.5 5.5 3 5.5 Z M5 8.5 L5.01 8.5 M8 8.5 L8.01 8.5 M11 8.5 L11.01 8.5 M14 8.5 L14.01 8.5 M6.5 11.5 L13.5 11.5",
        Caps => "M10 3 L3.5 9.8 L7 9.8 L7 13.5 L13 13.5 L13 9.8 L16.5 9.8 Z M7 16.8 L13 16.8",
        NumLock => "M7 3.5 L5.5 16.5 M14 3.5 L12.5 16.5 M3.5 7.5 L16.5 7.5 M3 12.5 L16 12.5",
        Wifi0 => "M10 15.5 L10.01 15.5",
        Wifi1 => "M10 15.5 L10.01 15.5 M7.3 12.7 A4 4 0 0 1 12.7 12.7",
        Wifi2 => "M10 15.5 L10.01 15.5 M7.3 12.7 A4 4 0 0 1 12.7 12.7 M4.7 10.1 A7.6 7.6 0 0 1 15.3 10.1",
        Wifi3 => "M10 15.5 L10.01 15.5 M7.3 12.7 A4 4 0 0 1 12.7 12.7 M4.7 10.1 A7.6 7.6 0 0 1 15.3 10.1 M2.1 7.5 A11.2 11.2 0 0 1 17.9 7.5",
        WifiOff => "M10 15.5 L10.01 15.5 M7.3 12.7 A4 4 0 0 1 12.7 12.7 M4.7 10.1 A7.6 7.6 0 0 1 15.3 10.1 M2.1 7.5 A11.2 11.2 0 0 1 17.9 7.5 M3 3 L17 17",
        Ethernet => "M4 4 L16 4 L16 12 L4 12 Z M10 12 L10 16.5 M6.5 16.5 L13.5 16.5 M7 7 L7 9 M10 7 L10 9 M13 7 L13 9",
        Bluetooth => "M5.8 6.6 L14 13.4 L10 17 L10 3 L14 6.6 L5.8 13.4",
        Headphones => "M3.5 14 L3.5 10.5 A6.5 6.5 0 0 1 16.5 10.5 L16.5 14 M3.5 12.5 L5.5 12.5 Q6.5 12.5 6.5 13.5 L6.5 16 Q6.5 17 5.5 17 L4.5 17 Q3.5 17 3.5 16 Z M16.5 12.5 L14.5 12.5 Q13.5 12.5 13.5 13.5 L13.5 16 Q13.5 17 14.5 17 L15.5 17 Q16.5 17 16.5 16 Z",
        Speaker => "M5.5 2.5 L14.5 2.5 Q16 2.5 16 4 L16 16 Q16 17.5 14.5 17.5 L5.5 17.5 Q4 17.5 4 16 L4 4 Q4 2.5 5.5 2.5 Z M13 12 A3 3 0 1 1 7 12 A3 3 0 1 1 13 12 Z M10 6 L10.01 6",
        Bolt => "F:M11.2 1.8 L4.4 11.2 L9.2 11.2 L8.4 18.2 L15.6 8.6 L10.8 8.6 Z",
        Timer => "M15.5 11 A5.5 5.5 0 1 1 4.5 11 A5.5 5.5 0 1 1 15.5 11 Z M10 11 L10 8 M8 2.5 L12 2.5 M14.6 5.6 L15.6 4.6",
        Droplet => "M10 2.5 C13 6 15 8.7 15 11.5 A5 5 0 0 1 5 11.5 C5 8.7 7 6 10 2.5 Z",
        CheckCircle => "M17.5 10 A7.5 7.5 0 1 1 2.5 10 A7.5 7.5 0 1 1 17.5 10 Z M6.8 10.2 L9 12.4 L13.2 8",
        Check => "M4.5 10.5 L8.2 14 L15.5 6.5",
        List => "M7 5.5 L17 5.5 M7 10 L17 10 M7 14.5 L17 14.5 M3.5 5.5 L3.51 5.5 M3.5 10 L3.51 10 M3.5 14.5 L3.51 14.5",
        Shelf => "M2.5 11 L4.7 5 Q5 4 6 4 L14 4 Q15 4 15.3 5 L17.5 11 L17.5 15 Q17.5 16.5 16 16.5 L4 16.5 Q2.5 16.5 2.5 15 Z M2.5 11 L6.5 11 L7.5 13 L12.5 13 L13.5 11 L17.5 11",
        Clipboard => "M7 2.8 L13 2.8 L13 5.6 L7 5.6 Z M13 4.2 L14.5 4.2 Q16 4.2 16 5.7 L16 16 Q16 17.5 14.5 17.5 L5.5 17.5 Q4 17.5 4 16 L4 5.7 Q4 4.2 5.5 4.2 L7 4.2",
        MoonStar => "M15.8 12.6 A6.6 6.6 0 1 1 7.4 4.2 A5.2 5.2 0 0 0 15.8 12.6 Z M14.5 2.8 L14.5 6.2 M12.8 4.5 L16.2 4.5",
        ScanText => "M3 6.5 L3 4.5 Q3 3 4.5 3 L6.5 3 M13.5 3 L15.5 3 Q17 3 17 4.5 L17 6.5 M17 13.5 L17 15.5 Q17 17 15.5 17 L13.5 17 M6.5 17 L4.5 17 Q3 17 3 15.5 L3 13.5 M6.5 7.5 L13.5 7.5 M6.5 10 L13.5 10 M6.5 12.5 L10.5 12.5",
        Calculator => "M5.5 2.5 L14.5 2.5 Q16 2.5 16 4 L16 16 Q16 17.5 14.5 17.5 L5.5 17.5 Q4 17.5 4 16 L4 4 Q4 2.5 5.5 2.5 Z M7 5.5 L13 5.5 L13 8 L7 8 Z M7.5 11 L7.51 11 M10 11 L10.01 11 M12.5 11 L12.51 11 M7.5 14 L7.51 14 M10 14 L10.01 14 M12.5 14 L12.51 14",
        Sparkles => "M8.5 2.5 L10 6.5 L14 8 L10 9.5 L8.5 13.5 L7 9.5 L3 8 L7 6.5 Z M15 12 L15.8 14.2 L18 15 L15.8 15.8 L15 18 L14.2 15.8 L12 15 L14.2 14.2 Z",
        Translate => "M2.5 4.5 L10.5 4.5 M6.5 2.8 L6.5 4.5 M9 4.5 C8.5 7.5 6.5 10 3.5 11.5 M5 7 C6 9 7.5 10.5 9.5 11.5 M10.5 17.2 L13.5 10 L16.5 17.2 M11.5 15 L15.5 15",
        Cloud => "M5.5 15.5 L14.5 15.5 A3.5 3.5 0 0 0 14.5 8.5 A5 5 0 0 0 4.9 10 A2.8 2.8 0 0 0 5.5 15.5 Z",
        CloudSun => "M7.5 16.5 L14.5 16.5 A3 3 0 0 0 14.5 10.5 A4.4 4.4 0 0 0 6.2 11.6 A2.5 2.5 0 0 0 7.5 16.5 Z M6 2.5 L6 3.6 M1.8 6.8 L2.9 6.8 M3 3.8 L3.8 4.6 M9 3.8 L8.2 4.6 M3.6 9.2 A3 3 0 1 1 8.8 6.6",
        Rain => "M5.5 12.5 L14.5 12.5 A3.5 3.5 0 0 0 14.5 5.5 A5 5 0 0 0 4.9 7 A2.8 2.8 0 0 0 5.5 12.5 Z M7 15 L6 17.5 M10.5 15 L9.5 17.5 M14 15 L13 17.5",
        Snow => "M5.5 12.5 L14.5 12.5 A3.5 3.5 0 0 0 14.5 5.5 A5 5 0 0 0 4.9 7 A2.8 2.8 0 0 0 5.5 12.5 Z M7 15.5 L7.01 15.5 M10 17 L10.01 17 M13 15.5 L13.01 15.5",
        Storm => "M5.5 12.5 L14.5 12.5 A3.5 3.5 0 0 0 14.5 5.5 A5 5 0 0 0 4.9 7 A2.8 2.8 0 0 0 5.5 12.5 Z M10.5 12.5 L8.5 15.5 L11.5 15.5 L9.5 18.5",
        Fog => "M3 7 L17 7 M5 10.5 L15 10.5 M3 14 L17 14",
        Calendar => "M4.5 4 L15.5 4 Q17 4 17 5.5 L17 15.5 Q17 17 15.5 17 L4.5 17 Q3 17 3 15.5 L3 5.5 Q3 4 4.5 4 Z M3 8.5 L17 8.5 M7 2.5 L7 5.5 M13 2.5 L13 5.5",
        Widgets => "M4 3 L7.5 3 Q8.5 3 8.5 4 L8.5 7.5 Q8.5 8.5 7.5 8.5 L4 8.5 Q3 8.5 3 7.5 L3 4 Q3 3 4 3 Z M12.5 3 L16 3 Q17 3 17 4 L17 7.5 Q17 8.5 16 8.5 L12.5 8.5 Q11.5 8.5 11.5 7.5 L11.5 4 Q11.5 3 12.5 3 Z M4 11.5 L7.5 11.5 Q8.5 11.5 8.5 12.5 L8.5 16 Q8.5 17 7.5 17 L4 17 Q3 17 3 16 L3 12.5 Q3 11.5 4 11.5 Z M12.5 11.5 L16 11.5 Q17 11.5 17 12.5 L17 16 Q17 17 16 17 L12.5 17 Q11.5 17 11.5 16 L11.5 12.5 Q11.5 11.5 12.5 11.5 Z",
        Mic => "M7.5 5 A2.5 2.5 0 0 1 12.5 5 L12.5 9.5 A2.5 2.5 0 0 1 7.5 9.5 Z M4.5 9.5 A5.5 5.5 0 0 0 15.5 9.5 M10 15 L10 17.5",
        MicOff => "M7.5 5 A2.5 2.5 0 0 1 12.5 5 L12.5 9.5 A2.5 2.5 0 0 1 7.5 9.5 Z M4.5 9.5 A5.5 5.5 0 0 0 15.5 9.5 M10 15 L10 17.5 M3 3 L17 17",
        Camera => "M3.5 5.5 L12.5 5.5 Q13.5 5.5 13.5 6.5 L13.5 13.5 Q13.5 14.5 12.5 14.5 L3.5 14.5 Q2.5 14.5 2.5 13.5 L2.5 6.5 Q2.5 5.5 3.5 5.5 Z M13.5 9 L17.5 6.5 L17.5 13.5 L13.5 11",
        Location => "M10 17.5 C6.5 13.7 4.5 10.8 4.5 8 A5.5 5.5 0 0 1 15.5 8 C15.5 10.8 13.5 13.7 10 17.5 Z M12 8 A2 2 0 1 1 8 8 A2 2 0 1 1 12 8 Z",
        Phone => "F:M5.8 2.6 L7.9 6.6 L6.3 8.1 A9 9 0 0 0 11.9 13.7 L13.4 12.1 L17.4 14.2 L16.8 16.5 Q16.4 17.7 15.1 17.6 C8.8 17 3 11.2 2.4 4.9 Q2.3 3.6 3.5 3.2 Z",
        Download => "M10 3 L10 13 M6 9.5 L10 13.5 L14 9.5 M3.5 16.5 L16.5 16.5",
        Settings => "M3 6 L10 6 M14 6 L17 6 M3 14 L6 14 M10 14 L17 14 M14 6 A2 2 0 1 1 10 6 A2 2 0 1 1 14 6 Z M10 14 A2 2 0 1 1 6 14 A2 2 0 1 1 10 14 Z",
        Pin => "M7.5 2.5 L12.5 2.5 M8.5 2.5 L8.5 7.5 L5.5 10.5 L5.5 12 L14.5 12 L14.5 10.5 L11.5 7.5 L11.5 2.5 M10 12 L10 17.5",
        Close => "M5.5 5.5 L14.5 14.5 M14.5 5.5 L5.5 14.5",
        ChevronLeft => "M12.5 4.5 L7 10 L12.5 15.5",
        ChevronRight => "M7.5 4.5 L13 10 L7.5 15.5",
        ChevronDown => "M4.5 7.5 L10 13 L15.5 7.5",
        Search => "M14.5 8.5 A6 6 0 1 1 2.5 8.5 A6 6 0 1 1 14.5 8.5 Z M13 13 L17.5 17.5",
        Plus => "M10 4 L10 16 M4 10 L16 10",
        Minus => "M4 10 L16 10",
        Trash => "M3.5 5.5 L16.5 5.5 M8 5.5 L8 3.5 L12 3.5 L12 5.5 M5 5.5 L5.8 16.5 Q5.9 17.5 6.8 17.5 L13.2 17.5 Q14.1 17.5 14.2 16.5 L15 5.5 M8.5 9 L8.5 14 M11.5 9 L11.5 14",
        Copy => "M7.5 6.5 L15.5 6.5 Q17 6.5 17 8 L17 16 Q17 17.5 15.5 17.5 L7.5 17.5 Q6 17.5 6 16 L6 8 Q6 6.5 7.5 6.5 Z M3 13 L3 4.5 Q3 3 4.5 3 L13 3",
        ExternalLink => "M11 3 L17 3 L17 9 M17 3 L9 11 M14.5 11.5 L14.5 15.5 Q14.5 17 13 17 L4.5 17 Q3 17 3 15.5 L3 7 Q3 5.5 4.5 5.5 L8.5 5.5",
        Hammer => "M11.5 3 L16.5 8 L14.5 10 L9.5 5 Z M11.5 7 L3.5 15 Q3 16.5 4.5 16.5 L12.5 8.5",
        Note => "M4.5 2.5 L12 2.5 L16 6.5 L16 16 Q16 17.5 14.5 17.5 L5.5 17.5 Q4 17.5 4 16 L4 3 Q4 2.5 4.5 2.5 Z M11.5 2.5 L11.5 7 L16 7 M7 10.5 L13 10.5 M7 13.5 L11 13.5",
        File => "M4.5 2.5 L12 2.5 L16 6.5 L16 16 Q16 17.5 14.5 17.5 L5.5 17.5 Q4 17.5 4 16 L4 3 Q4 2.5 4.5 2.5 Z M11.5 2.5 L11.5 7 L16 7",
        Folder => "M2.5 5 Q2.5 3.5 4 3.5 L7.5 3.5 L9.5 5.5 L16 5.5 Q17.5 5.5 17.5 7 L17.5 15 Q17.5 16.5 16 16.5 L4 16.5 Q2.5 16.5 2.5 15 Z",
        Link => "M8.5 11.5 A3.2 3.2 0 0 0 13 11.5 L15.5 9 A3.2 3.2 0 0 0 11 4.5 L10 5.5 M11.5 8.5 A3.2 3.2 0 0 0 7 8.5 L4.5 11 A3.2 3.2 0 0 0 9 15.5 L10 14.5",
        Image => "M4.5 3 L15.5 3 Q17 3 17 4.5 L17 15.5 Q17 17 15.5 17 L4.5 17 Q3 17 3 15.5 L3 4.5 Q3 3 4.5 3 Z M3 13.5 L7 9.5 L12 14.5 M11 12.5 L13 10.5 L17 14.5 M13.5 7 L13.51 7",
        Text => "M4 5 L4 3.5 L16 3.5 L16 5 M10 3.5 L10 16.5 M7.5 16.5 L12.5 16.5",
        Palette => "M10 2.5 A7.5 7.5 0 0 0 10 17.5 Q11.5 17.5 11.5 16 Q11.5 15 10.8 14.4 Q10.3 13.5 11.3 12.8 L13.5 12.8 A4 4 0 0 0 17.5 8.8 C17.5 5.3 14.2 2.5 10 2.5 Z M6.5 9 L6.51 9 M9 6 L9.01 6 M13 6.5 L13.01 6.5",
        Bell => "M5 13.5 L5 9 A5 5 0 0 1 15 9 L15 13.5 L16.5 15.5 L3.5 15.5 Z M8.5 17.8 L11.5 17.8",
        Cpu => "M5.5 5.5 L14.5 5.5 L14.5 14.5 L5.5 14.5 Z M8 8 L12 8 L12 12 L8 12 Z M8 3 L8 5.5 M12 3 L12 5.5 M8 14.5 L8 17 M12 14.5 L12 17 M3 8 L5.5 8 M3 12 L5.5 12 M14.5 8 L17 8 M14.5 12 L17 12",
        Globe => "M17.5 10 A7.5 7.5 0 1 1 2.5 10 A7.5 7.5 0 1 1 17.5 10 Z M2.5 10 L17.5 10 M10 2.5 C12.2 4.6 13.2 7.2 13.2 10 C13.2 12.8 12.2 15.4 10 17.5 C7.8 15.4 6.8 12.8 6.8 10 C6.8 7.2 7.8 4.6 10 2.5 Z",
        Shield => "M10 2.5 L16 4.8 L16 9.5 C16 13.2 13.5 15.8 10 17.5 C6.5 15.8 4 13.2 4 9.5 L4 4.8 Z",
        Music => "M7.5 15 L7.5 4.5 L16 3 L16 13.5 M7.5 15 A2 2 0 1 1 3.5 15 A2 2 0 1 1 7.5 15 Z M16 13.5 A2 2 0 1 1 12 13.5 A2 2 0 1 1 16 13.5 Z",
        Undo => "M6 4.5 L3 7.5 L6 10.5 M3 7.5 L12 7.5 A4.5 4.5 0 0 1 12 16.5 L7.5 16.5",
        Skip => "M4 5 L10 10 L4 15 Z M10.5 5 L16.5 10 L10.5 15 Z",
        Reset => "M3.5 10 A6.5 6.5 0 1 0 5.4 5.4 M3.5 3.5 L3.5 7 L7 7",
        Send => "M17.5 2.5 L9 11 M17.5 2.5 L12 17.5 L9 11 L2.5 8 Z",
        Qr => "M3 3 L8 3 L8 8 L3 8 Z M12 3 L17 3 L17 8 L12 8 Z M3 12 L8 12 L8 17 L3 17 Z M12 12 L14 12 L14 14 L12 14 Z M15.5 15.5 L17 15.5 L17 17 L15.5 17 Z",
        Crop => "M5.5 2.5 L5.5 14.5 L17.5 14.5 M2.5 5.5 L14.5 5.5 L14.5 17.5",
        Arrow => "M4 16 L16 4 M8 4 L16 4 L16 12",
        Square => "M4 4 L16 4 L16 16 L4 16 Z",
        Highlighter => "M12.5 3 L17 7.5 L9 15.5 L4.5 11 Z M4.5 11 L3 16.5 L9 15.5",
        Eye => "M1.8 10 C4 5.8 7 4 10 4 C13 4 16 5.8 18.2 10 C16 14.2 13 16 10 16 C7 16 4 14.2 1.8 10 Z M12.5 10 A2.5 2.5 0 1 1 7.5 10 A2.5 2.5 0 1 1 12.5 10 Z",
        Leaf => "M4 16 C4 8 9 3.5 16.5 3.5 C16.5 11 12 16 4 16 Z M4 16 L11 9",
        Star => "M10 2.5 L12.2 7.2 L17.3 7.7 L13.4 11.1 L14.6 16.2 L10 13.5 L5.4 16.2 L6.6 11.1 L2.7 7.7 L7.8 7.2 Z",
        Lock => "M6.5 9 L6.5 6.5 A3.5 3.5 0 0 1 13.5 6.5 L13.5 9 M5.5 9 L14.5 9 Q16 9 16 10.5 L16 16 Q16 17.5 14.5 17.5 L5.5 17.5 Q4 17.5 4 16 L4 10.5 Q4 9 5.5 9 Z M10 12.3 L10 14.3",
        Coffee => "M4 8 L14 8 L14 12.5 A4.5 4.5 0 0 1 9.5 17 L8.5 17 A4.5 4.5 0 0 1 4 12.5 Z M14 9.5 L15.3 9.5 A2 2 0 0 1 15.3 13.5 L14 13.5 M7 2.8 Q6 4.2 7 5.6 M10.5 2.8 Q9.5 4.2 10.5 5.6",
        Contrast => "M10 2.5 A7.5 7.5 0 1 1 9.99 2.5 Z M10 2.5 L10 17.5 M12.7 3 L12.7 17 M15.2 4.6 L15.2 15.4",
        Sliders => "M3.5 6.5 L8.8 6.5 M13.2 6.5 L16.5 6.5 M11 4.3 A2.2 2.2 0 1 0 11.01 4.3 Z M3.5 13.5 L5.8 13.5 M10.2 13.5 L16.5 13.5 M8 11.3 A2.2 2.2 0 1 0 8.01 11.3 Z",
        Info => "M10 2.5 A7.5 7.5 0 1 1 9.99 2.5 Z M10 9.2 L10 14 M10 6.3 L10.01 6.3",
        Home => "M3 9.6 L10 3.4 L17 9.6 L17 16 Q17 17 16 17 L12.5 17 L12.5 12.5 L7.5 12.5 L7.5 17 L4 17 Q3 17 3 16 Z",
        Airplane => "M10 2.4 C10.9 2.4 11.3 3.3 11.3 4.4 L11.3 8 L17.6 11.7 L17.6 13.3 L11.3 11.5 L11.3 15 L13.3 16.5 L13.3 17.7 L10 16.8 L6.7 17.7 L6.7 16.5 L8.7 15 L8.7 11.5 L2.4 13.3 L2.4 11.7 L8.7 8 L8.7 4.4 C8.7 3.3 9.1 2.4 10 2.4 Z",
        Heart => "F:M10 16.8 C4.8 13.4 2.5 10.6 2.5 7.6 A3.9 3.9 0 0 1 10 5.8 A3.9 3.9 0 0 1 17.5 7.6 C17.5 10.6 15.2 13.4 10 16.8 Z",
    }
}

pub fn is_filled(icon: Icon) -> bool {
    path(icon).starts_with("F:")
}

/// Volume icon for a level 0..1.
pub fn volume_icon(level: f32, muted: bool) -> Icon {
    if muted || level <= 0.001 {
        Icon::Mute
    } else if level < 0.34 {
        Icon::Volume1
    } else if level < 0.67 {
        Icon::Volume2
    } else {
        Icon::Volume3
    }
}

pub fn wifi_icon(bars: u8) -> Icon {
    match bars {
        0 => Icon::Wifi0,
        1 => Icon::Wifi1,
        2 => Icon::Wifi2,
        _ => Icon::Wifi3,
    }
}

/// Looks up an icon by its kebab-case name (used by the Live Activities API).
pub fn by_name(name: &str) -> Option<Icon> {
    use Icon::*;
    Some(match name {
        "play" => Play,
        "pause" => Pause,
        "timer" => Timer,
        "download" => Download,
        "hammer" | "build" => Hammer,
        "check" => Check,
        "check-circle" => CheckCircle,
        "music" => Music,
        "mic" => Mic,
        "camera" => Camera,
        "phone" => Phone,
        "bell" => Bell,
        "calendar" => Calendar,
        "cloud" => Cloud,
        "sparkles" => Sparkles,
        "bolt" => Bolt,
        "globe" => Globe,
        "note" => Note,
        "file" => File,
        "folder" => Folder,
        "link" => Link,
        "send" => Send,
        "star" => Star,
        "heart" => Heart,
        "cpu" => Cpu,
        "shield" => Shield,
        "droplet" => Droplet,
        "list" => List,
        "leaf" => Leaf,
        "eye" => Eye,
        _ => return None,
    })
}

#[derive(Default)]
pub struct IconCache {
    map: HashMap<Icon, ID2D1PathGeometry>,
}

impl IconCache {
    pub fn get(&mut self, factory: &ID2D1Factory1, icon: Icon) -> Option<ID2D1PathGeometry> {
        if let Some(g) = self.map.get(&icon) {
            return Some(g.clone());
        }
        let d = path(icon).trim_start_matches("F:");
        let g = build_path(factory, d).ok()?;
        self.map.insert(icon, g.clone());
        Some(g)
    }
}

// ---------------------------------------------------------------- SVG path parsing

#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Quad(f32, f32, f32, f32),
    Arc {
        rx: f32,
        ry: f32,
        rot: f32,
        large: bool,
        sweep: bool,
        x: f32,
        y: f32,
    },
    Close,
}

fn tokenize(d: &str) -> Vec<Tok> {
    let b = d.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() {
            out.push(Tok::Cmd(c));
            i += 1;
        } else if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut seen_dot = c == '.';
            let mut seen_e = false;
            while i < b.len() {
                let ch = b[i] as char;
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !seen_dot && !seen_e {
                    seen_dot = true;
                    i += 1;
                } else if (ch == 'e' || ch == 'E') && !seen_e {
                    seen_e = true;
                    i += 1;
                    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(v) = d[start..i].parse::<f32>() {
                out.push(Tok::Num(v));
            }
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum Tok {
    Cmd(char),
    Num(f32),
}

/// Parses SVG path data into absolute segments (M L H V C S Q T A Z, both cases).
pub fn parse(d: &str) -> Vec<Seg> {
    let toks = tokenize(d);
    let mut segs = Vec::new();
    let (mut cx, mut cy, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut last_ctrl: Option<(f32, f32)> = None;
    let mut i = 0;
    let mut cmd = 'M';
    let num = |i: &mut usize| -> Option<f32> {
        match toks.get(*i) {
            Some(Tok::Num(v)) => {
                *i += 1;
                Some(*v)
            }
            _ => None,
        }
    };
    while i < toks.len() {
        if let Tok::Cmd(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                segs.push(Seg::Close);
                cx = sx;
                cy = sy;
                last_ctrl = None;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        let before = i;
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                cx = ox + x;
                cy = oy + y;
                sx = cx;
                sy = cy;
                segs.push(Seg::Move(cx, cy));
                cmd = if rel { 'l' } else { 'L' };
                last_ctrl = None;
            }
            'L' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                cx = ox + x;
                cy = oy + y;
                segs.push(Seg::Line(cx, cy));
                last_ctrl = None;
            }
            'H' => {
                let Some(x) = num(&mut i) else { break };
                cx = if rel { cx + x } else { x };
                segs.push(Seg::Line(cx, cy));
                last_ctrl = None;
            }
            'V' => {
                let Some(y) = num(&mut i) else { break };
                cy = if rel { cy + y } else { y };
                segs.push(Seg::Line(cx, cy));
                last_ctrl = None;
            }
            'C' => {
                let v: Vec<f32> = (0..6).filter_map(|_| num(&mut i)).collect();
                if v.len() < 6 {
                    break;
                }
                let s = Seg::Cubic(
                    ox + v[0],
                    oy + v[1],
                    ox + v[2],
                    oy + v[3],
                    ox + v[4],
                    oy + v[5],
                );
                last_ctrl = Some((ox + v[2], oy + v[3]));
                cx = ox + v[4];
                cy = oy + v[5];
                segs.push(s);
            }
            'S' => {
                let v: Vec<f32> = (0..4).filter_map(|_| num(&mut i)).collect();
                if v.len() < 4 {
                    break;
                }
                let (c1x, c1y) = match last_ctrl {
                    Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                    None => (cx, cy),
                };
                segs.push(Seg::Cubic(
                    c1x,
                    c1y,
                    ox + v[0],
                    oy + v[1],
                    ox + v[2],
                    oy + v[3],
                ));
                last_ctrl = Some((ox + v[0], oy + v[1]));
                cx = ox + v[2];
                cy = oy + v[3];
            }
            'Q' => {
                let v: Vec<f32> = (0..4).filter_map(|_| num(&mut i)).collect();
                if v.len() < 4 {
                    break;
                }
                segs.push(Seg::Quad(ox + v[0], oy + v[1], ox + v[2], oy + v[3]));
                last_ctrl = Some((ox + v[0], oy + v[1]));
                cx = ox + v[2];
                cy = oy + v[3];
            }
            'T' => {
                let v: Vec<f32> = (0..2).filter_map(|_| num(&mut i)).collect();
                if v.len() < 2 {
                    break;
                }
                let (qx, qy) = match last_ctrl {
                    Some((lx, ly)) => (2.0 * cx - lx, 2.0 * cy - ly),
                    None => (cx, cy),
                };
                segs.push(Seg::Quad(qx, qy, ox + v[0], oy + v[1]));
                last_ctrl = Some((qx, qy));
                cx = ox + v[0];
                cy = oy + v[1];
            }
            'A' => {
                let v: Vec<f32> = (0..7).filter_map(|_| num(&mut i)).collect();
                if v.len() < 7 {
                    break;
                }
                cx = ox + v[5];
                cy = oy + v[6];
                segs.push(Seg::Arc {
                    rx: v[0],
                    ry: v[1],
                    rot: v[2],
                    large: v[3] != 0.0,
                    sweep: v[4] != 0.0,
                    x: cx,
                    y: cy,
                });
                last_ctrl = None;
            }
            _ => {
                i += 1;
            }
        }
        if i == before {
            i += 1;
        }
    }
    segs
}

pub fn build_path(factory: &ID2D1Factory1, d: &str) -> windows::core::Result<ID2D1PathGeometry> {
    let geo: ID2D1PathGeometry = unsafe { factory.CreatePathGeometry()?.into() };
    let sink = unsafe { geo.Open()? };
    let mut open = false;
    let pt = |x: f32, y: f32| Vector2 { X: x, Y: y };
    unsafe {
        sink.SetFillMode(D2D1_FILL_MODE_WINDING);
        for s in parse(d) {
            match s {
                Seg::Move(x, y) => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    }
                    sink.BeginFigure(pt(x, y), D2D1_FIGURE_BEGIN_FILLED);
                    open = true;
                }
                Seg::Line(x, y) => sink.AddLine(pt(x, y)),
                Seg::Cubic(a, b, c, d2, e, f) => sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: pt(a, b),
                    point2: pt(c, d2),
                    point3: pt(e, f),
                }),
                Seg::Quad(a, b, c, d2) => sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                    point1: pt(a, b),
                    point2: pt(c, d2),
                }),
                Seg::Arc {
                    rx,
                    ry,
                    rot,
                    large,
                    sweep,
                    x,
                    y,
                } => sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: pt(x, y),
                    size: D2D_SIZE_F {
                        width: rx,
                        height: ry,
                    },
                    rotationAngle: rot,
                    sweepDirection: if sweep {
                        D2D1_SWEEP_DIRECTION_CLOCKWISE
                    } else {
                        D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                    },
                    arcSize: if large {
                        D2D1_ARC_SIZE_LARGE
                    } else {
                        D2D1_ARC_SIZE_SMALL
                    },
                }),
                Seg::Close => {
                    if open {
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        open = false;
                    }
                }
            }
        }
        if open {
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
        sink.Close()?;
    }
    Ok(geo)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_compact_numbers() {
        let s = parse("M1 2l3-4.5.5.5Z");
        assert_eq!(s[0], Seg::Move(1.0, 2.0));
        assert_eq!(s[1], Seg::Line(4.0, -2.5));
        assert_eq!(s[2], Seg::Line(4.5, -2.0));
        assert_eq!(s[3], Seg::Close);
    }
    #[test]
    fn parses_arcs() {
        let s = parse("M13 10 A3 3 0 1 1 7 10");
        assert!(matches!(
            s[1],
            Seg::Arc {
                large: true,
                sweep: true,
                ..
            }
        ));
    }
    #[test]
    fn all_icons_parse() {
        // every icon must produce at least one segment
        for i in 0..=Icon::Heart as u8 {
            let icon: Icon = unsafe { std::mem::transmute(i) };
            assert!(
                parse(path(icon).trim_start_matches("F:")).len() > 1,
                "{icon:?}"
            );
        }
    }
}
