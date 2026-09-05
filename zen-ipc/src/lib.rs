
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub mod socket;
pub mod state;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Request {
    Version,
    Outputs,
    Workspaces,
    Windows,
    Layers,
    KeyboardLayouts,
    FocusedOutput,
    FocusedWindow,
    PickWindow,
    PickColor,
    Action(Action),
    Output {
        output: String,
        action: OutputAction,
    },
    EventStream,
    ReturnError,
    OverviewState,
    Casts,
}

pub type Reply = Result<Response, String>;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Response {
    Handled,
    Version(String),
    Outputs(HashMap<String, Output>),
    Workspaces(Vec<Workspace>),
    Windows(Vec<Window>),
    Layers(Vec<LayerSurface>),
    KeyboardLayouts(KeyboardLayouts),
    FocusedOutput(Option<Output>),
    FocusedWindow(Option<Window>),
    PickedWindow(Option<Window>),
    PickedColor(Option<PickedColor>),
    OutputConfigChanged(OutputConfigChanged),
    OverviewState(Overview),
    Casts(Vec<Cast>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Overview {
    pub is_open: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PickedColor {
    pub rgb: [f64; 3],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "clap", command(subcommand_value_name = "ACTION"))]
#[cfg_attr(feature = "clap", command(subcommand_help_heading = "Actions"))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Action {
    Quit {
        #[cfg_attr(feature = "clap", arg(short, long))]
        skip_confirmation: bool,
    },
    PowerOffMonitors {},
    PowerOnMonitors {},
    Spawn {
        #[cfg_attr(feature = "clap", arg(last = true, required = true))]
        command: Vec<String>,
    },
    SpawnSh {
        #[cfg_attr(feature = "clap", arg(last = true, required = true))]
        command: String,
    },
    DoScreenTransition {
        #[cfg_attr(feature = "clap", arg(short, long))]
        delay_ms: Option<u16>,
    },
    Screenshot {
        #[cfg_attr(feature = "clap", arg(short = 'p', long, action = clap::ArgAction::Set, default_value_t = true))]
        show_pointer: bool,

        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set))]
        path: Option<String>,
    },
    ScreenshotScreen {
        #[cfg_attr(feature = "clap", arg(short = 'd', long, action = clap::ArgAction::Set, default_value_t = true))]
        write_to_disk: bool,

        #[cfg_attr(feature = "clap", arg(short = 'p', long, action = clap::ArgAction::Set, default_value_t = true))]
        show_pointer: bool,

        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set))]
        path: Option<String>,
    },
    #[cfg_attr(feature = "clap", clap(about = "Screenshot the focused window"))]
    ScreenshotWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
        #[cfg_attr(feature = "clap", arg(short = 'd', long, action = clap::ArgAction::Set, default_value_t = true))]
        write_to_disk: bool,

        #[cfg_attr(feature = "clap", arg(short = 'p', long, action = clap::ArgAction::Set, default_value_t = false))]
        show_pointer: bool,

        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set))]
        path: Option<String>,
    },
    ToggleKeyboardShortcutsInhibit {},
    #[cfg_attr(feature = "clap", clap(about = "Close the focused window"))]
    CloseWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Toggle fullscreen on the focused window")
    )]
    FullscreenWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Toggle windowed (fake) fullscreen on the focused window")
    )]
    ToggleWindowedFullscreen {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    FocusWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: u64,
    },
    FocusWindowInColumn {
        #[cfg_attr(feature = "clap", arg())]
        index: u8,
    },
    FocusWindowPrevious {},
    FocusColumnLeft {},
    FocusColumnRight {},
    FocusColumnFirst {},
    FocusColumnLast {},
    FocusColumnRightOrFirst {},
    FocusColumnLeftOrLast {},
    FocusColumn {
        #[cfg_attr(feature = "clap", arg())]
        index: usize,
    },
    FocusWindowOrMonitorUp {},
    FocusWindowOrMonitorDown {},
    FocusColumnOrMonitorLeft {},
    FocusColumnOrMonitorRight {},
    FocusWindowDown {},
    FocusWindowUp {},
    FocusWindowDownOrColumnLeft {},
    FocusWindowDownOrColumnRight {},
    FocusWindowUpOrColumnLeft {},
    FocusWindowUpOrColumnRight {},
    FocusWindowOrWorkspaceDown {},
    FocusWindowOrWorkspaceUp {},
    FocusWindowTop {},
    FocusWindowBottom {},
    FocusWindowDownOrTop {},
    FocusWindowUpOrBottom {},
    MoveColumnLeft {},
    MoveColumnRight {},
    MoveColumnToFirst {},
    MoveColumnToLast {},
    MoveColumnLeftOrToMonitorLeft {},
    MoveColumnRightOrToMonitorRight {},
    MoveColumnToIndex {
        #[cfg_attr(feature = "clap", arg())]
        index: usize,
    },
    MoveWindowDown {},
    MoveWindowUp {},
    MoveWindowDownOrToWorkspaceDown {},
    MoveWindowUpOrToWorkspaceUp {},
    #[cfg_attr(
        feature = "clap",
        clap(about = "Consume or expel the focused window left")
    )]
    ConsumeOrExpelWindowLeft {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Consume or expel the focused window right")
    )]
    ConsumeOrExpelWindowRight {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    ConsumeWindowIntoColumn {},
    ExpelWindowFromColumn {},
    SwapWindowRight {},
    SwapWindowLeft {},
    ToggleColumnTabbedDisplay {},
    SetColumnDisplay {
        #[cfg_attr(feature = "clap", arg())]
        display: ColumnDisplay,
    },
    CenterColumn {},
    #[cfg_attr(
        feature = "clap",
        clap(about = "Center the focused window on the screen")
    )]
    CenterWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    CenterVisibleColumns {},
    FocusWorkspaceDown {},
    FocusWorkspaceUp {},
    FocusWorkspace {
        #[cfg_attr(feature = "clap", arg())]
        reference: WorkspaceReferenceArg,
    },
    FocusWorkspacePrevious {},
    MoveWindowToWorkspaceDown {
        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set, default_value_t = true))]
        focus: bool,
    },
    MoveWindowToWorkspaceUp {
        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set, default_value_t = true))]
        focus: bool,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Move the focused window to a workspace by reference (index or name)")
    )]
    MoveWindowToWorkspace {
        #[cfg_attr(feature = "clap", arg(long))]
        window_id: Option<u64>,

        #[cfg_attr(feature = "clap", arg())]
        reference: WorkspaceReferenceArg,

        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set, default_value_t = true))]
        focus: bool,
    },
    MoveColumnToWorkspaceDown {
        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set, default_value_t = true))]
        focus: bool,
    },
    MoveColumnToWorkspaceUp {
        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set, default_value_t = true))]
        focus: bool,
    },
    MoveColumnToWorkspace {
        #[cfg_attr(feature = "clap", arg())]
        reference: WorkspaceReferenceArg,

        #[cfg_attr(feature = "clap", arg(long, action = clap::ArgAction::Set, default_value_t = true))]
        focus: bool,
    },
    MoveWorkspaceDown {},
    MoveWorkspaceUp {},
    #[cfg_attr(
        feature = "clap",
        clap(about = "Move the focused workspace to a specific index on its monitor")
    )]
    MoveWorkspaceToIndex {
        #[cfg_attr(feature = "clap", arg())]
        index: usize,

        #[cfg_attr(feature = "clap", arg(long))]
        reference: Option<WorkspaceReferenceArg>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Set the name of the focused workspace")
    )]
    SetWorkspaceName {
        #[cfg_attr(feature = "clap", arg())]
        name: String,

        #[cfg_attr(feature = "clap", arg(long))]
        workspace: Option<WorkspaceReferenceArg>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Unset the name of the focused workspace")
    )]
    UnsetWorkspaceName {
        #[cfg_attr(feature = "clap", arg())]
        reference: Option<WorkspaceReferenceArg>,
    },
    FocusMonitorLeft {},
    FocusMonitorRight {},
    FocusMonitorDown {},
    FocusMonitorUp {},
    FocusMonitorPrevious {},
    FocusMonitorNext {},
    FocusMonitor {
        #[cfg_attr(feature = "clap", arg())]
        output: String,
    },
    MoveWindowToMonitorLeft {},
    MoveWindowToMonitorRight {},
    MoveWindowToMonitorDown {},
    MoveWindowToMonitorUp {},
    MoveWindowToMonitorPrevious {},
    MoveWindowToMonitorNext {},
    #[cfg_attr(
        feature = "clap",
        clap(about = "Move the focused window to a specific monitor")
    )]
    MoveWindowToMonitor {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,

        #[cfg_attr(feature = "clap", arg())]
        output: String,
    },
    MoveColumnToMonitorLeft {},
    MoveColumnToMonitorRight {},
    MoveColumnToMonitorDown {},
    MoveColumnToMonitorUp {},
    MoveColumnToMonitorPrevious {},
    MoveColumnToMonitorNext {},
    MoveColumnToMonitor {
        #[cfg_attr(feature = "clap", arg())]
        output: String,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Change the width of the focused window")
    )]
    SetWindowWidth {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,

        #[cfg_attr(feature = "clap", arg(allow_hyphen_values = true))]
        change: SizeChange,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Change the height of the focused window")
    )]
    SetWindowHeight {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,

        #[cfg_attr(feature = "clap", arg(allow_hyphen_values = true))]
        change: SizeChange,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Reset the height of the focused window back to automatic")
    )]
    ResetWindowHeight {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    SwitchPresetColumnWidth {},
    SwitchPresetColumnWidthBack {},
    SwitchPresetWindowWidth {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    SwitchPresetWindowWidthBack {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    SwitchPresetWindowHeight {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    SwitchPresetWindowHeightBack {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    MaximizeColumn {},
    MaximizeWindowToEdges {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    SetColumnWidth {
        #[cfg_attr(feature = "clap", arg(allow_hyphen_values = true))]
        change: SizeChange,
    },
    ExpandColumnToAvailableWidth {},
    SwitchLayout {
        #[cfg_attr(feature = "clap", arg())]
        layout: LayoutSwitchTarget,
    },
    ShowHotkeyOverlay {},
    MoveWorkspaceToMonitorLeft {},
    MoveWorkspaceToMonitorRight {},
    MoveWorkspaceToMonitorDown {},
    MoveWorkspaceToMonitorUp {},
    MoveWorkspaceToMonitorPrevious {},
    MoveWorkspaceToMonitorNext {},
    #[cfg_attr(
        feature = "clap",
        clap(about = "Move the focused workspace to a specific monitor")
    )]
    MoveWorkspaceToMonitor {
        #[cfg_attr(feature = "clap", arg())]
        output: String,

        #[cfg_attr(feature = "clap", arg(long))]
        reference: Option<WorkspaceReferenceArg>,
    },
    ToggleDebugTint {},
    DebugToggleOpaqueRegions {},
    DebugToggleDamage {},
    ToggleWindowFloating {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    MoveWindowToFloating {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    MoveWindowToTiling {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    FocusFloating {},
    FocusTiling {},
    SwitchFocusBetweenFloatingAndTiling {},
    #[cfg_attr(feature = "clap", clap(about = "Move the floating window on screen"))]
    MoveFloatingWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,

        #[cfg_attr(
            feature = "clap",
            arg(short, long, default_value = "+0", allow_hyphen_values = true)
        )]
        x: PositionChange,

        #[cfg_attr(
            feature = "clap",
            arg(short, long, default_value = "+0", allow_hyphen_values = true)
        )]
        y: PositionChange,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Toggle the opacity of the focused window")
    )]
    ToggleWindowRuleOpacity {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Set the dynamic cast target to the focused window")
    )]
    SetDynamicCastWindow {
        #[cfg_attr(feature = "clap", arg(long))]
        id: Option<u64>,
    },
    #[cfg_attr(
        feature = "clap",
        clap(about = "Set the dynamic cast target to the focused monitor")
    )]
    SetDynamicCastMonitor {
        #[cfg_attr(feature = "clap", arg())]
        output: Option<String>,
    },
    ClearDynamicCastTarget {},
    StopCast {
        #[cfg_attr(feature = "clap", arg(long))]
        session_id: u64,
    },
    ToggleOverview {},
    OpenOverview {},
    CloseOverview {},
    ZoomIn {},
    ZoomOut {},
    ResetZoom {},
    PanCameraLeft {},
    PanCameraRight {},
    PanCameraUp {},
    PanCameraDown {},
    FitAllWindows {},
    CameraMaximize {},
    FocusIslandLeft {},
    FocusIslandRight {},
    FocusIslandUp {},
    FocusIslandDown {},
    MoveWindowToIslandLeft {},
    MoveWindowToIslandRight {},
    MoveWindowToIslandUp {},
    MoveWindowToIslandDown {},
    SplitWindowFromIsland {},
    ToggleWindowUrgent {
        #[cfg_attr(feature = "clap", arg(long))]
        id: u64,
    },
    SetWindowUrgent {
        #[cfg_attr(feature = "clap", arg(long))]
        id: u64,
    },
    UnsetWindowUrgent {
        #[cfg_attr(feature = "clap", arg(long))]
        id: u64,
    },
    LoadConfigFile {
        #[cfg_attr(feature = "clap", arg(long))]
        path: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum SizeChange {
    SetFixed(i32),
    SetProportion(f64),
    AdjustFixed(i32),
    AdjustProportion(f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum PositionChange {
    SetFixed(f64),
    SetProportion(f64),
    AdjustFixed(f64),
    AdjustProportion(f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum WorkspaceReferenceArg {
    Id(u64),
    Index(u8),
    Name(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum LayoutSwitchTarget {
    Next,
    Prev,
    Index(u8),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum ColumnDisplay {
    Normal,
    Tabbed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "clap", command(subcommand_value_name = "ACTION"))]
#[cfg_attr(feature = "clap", command(subcommand_help_heading = "Actions"))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum OutputAction {
    Off,
    On,
    Mode {
        #[cfg_attr(feature = "clap", arg())]
        mode: ModeToSet,
    },
    CustomMode {
        #[cfg_attr(feature = "clap", arg())]
        mode: ConfiguredMode,
    },
    #[cfg_attr(feature = "clap", arg())]
    Modeline {
        #[cfg_attr(feature = "clap", arg())]
        clock: f64,
        #[cfg_attr(feature = "clap", arg())]
        hdisplay: u16,
        #[cfg_attr(feature = "clap", arg())]
        hsync_start: u16,
        #[cfg_attr(feature = "clap", arg())]
        hsync_end: u16,
        #[cfg_attr(feature = "clap", arg())]
        htotal: u16,

        #[cfg_attr(feature = "clap", arg())]
        vdisplay: u16,
        #[cfg_attr(feature = "clap", arg())]
        vsync_start: u16,
        #[cfg_attr(feature = "clap", arg())]
        vsync_end: u16,
        #[cfg_attr(feature = "clap", arg())]
        vtotal: u16,
        #[cfg_attr(feature = "clap", arg(allow_hyphen_values = true))]
        hsync_polarity: HSyncPolarity,
        #[cfg_attr(feature = "clap", arg(allow_hyphen_values = true))]
        vsync_polarity: VSyncPolarity,
    },
    Scale {
        #[cfg_attr(feature = "clap", arg())]
        scale: ScaleToSet,
    },
    Transform {
        #[cfg_attr(feature = "clap", arg())]
        transform: Transform,
    },
    Position {
        #[cfg_attr(feature = "clap", command(subcommand))]
        position: PositionToSet,
    },
    Vrr {
        #[cfg_attr(feature = "clap", command(flatten))]
        vrr: VrrToSet,
    },
    MaxBpc {
        #[cfg_attr(feature = "clap", arg())]
        max_bpc: MaxBpc,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum ModeToSet {
    Automatic,
    Specific(ConfiguredMode),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ConfiguredMode {
    pub width: u16,
    pub height: u16,
    pub refresh: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum HSyncPolarity {
    PHSync,
    NHSync,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum VSyncPolarity {
    PVSync,
    NVSync,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum ScaleToSet {
    Automatic,
    Specific(f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "clap", derive(clap::Subcommand))]
#[cfg_attr(feature = "clap", command(subcommand_value_name = "POSITION"))]
#[cfg_attr(feature = "clap", command(subcommand_help_heading = "Position Values"))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum PositionToSet {
    #[cfg_attr(feature = "clap", command(name = "auto"))]
    Automatic,
    #[cfg_attr(feature = "clap", command(name = "set"))]
    Specific(ConfiguredPosition),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ConfiguredPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct VrrToSet {
    #[cfg_attr(
        feature = "clap",
        arg(
            value_name = "ON|OFF",
            action = clap::ArgAction::Set,
            value_parser = clap::builder::BoolishValueParser::new(),
            hide_possible_values = true,
        ),
    )]
    pub vrr: bool,
    #[cfg_attr(feature = "clap", arg(long))]
    pub on_demand: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Output {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    pub physical_size: Option<(u32, u32)>,
    pub modes: Vec<Mode>,
    pub current_mode: Option<usize>,
    pub is_custom_mode: bool,
    pub vrr_supported: bool,
    pub vrr_enabled: bool,
    pub logical: Option<LogicalOutput>,
    pub max_bpc: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Mode {
    pub width: u16,
    pub height: u16,
    pub refresh_rate: u32,
    pub is_preferred: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct LogicalOutput {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub transform: Transform,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Transform {
    Normal,
    #[serde(rename = "90")]
    _90,
    #[serde(rename = "180")]
    _180,
    #[serde(rename = "270")]
    _270,
    Flipped,
    #[cfg_attr(feature = "clap", value(name("flipped-90")))]
    Flipped90,
    #[cfg_attr(feature = "clap", value(name("flipped-180")))]
    Flipped180,
    #[cfg_attr(feature = "clap", value(name("flipped-270")))]
    Flipped270,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum MaxBpc {
    #[serde(rename = "6")]
    _6 = 6,
    #[default]
    #[serde(rename = "8")]
    _8 = 8,
    #[serde(rename = "10")]
    _10 = 10,
    #[serde(rename = "12")]
    _12 = 12,
    #[serde(rename = "14")]
    _14 = 14,
    #[serde(rename = "16")]
    _16 = 16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Window {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub pid: Option<i32>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    pub is_floating: bool,
    pub is_urgent: bool,
    pub layout: WindowLayout,
    pub focus_timestamp: Option<Timestamp>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Timestamp {
    pub secs: u64,
    pub nanos: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct WindowLayout {
    pub pos_in_scrolling_layout: Option<(usize, usize)>,
    pub tile_size: (f64, f64),
    pub window_size: (i32, i32),
    pub tile_pos_in_workspace_view: Option<(f64, f64)>,
    pub window_offset_in_tile: (f64, f64),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum OutputConfigChanged {
    Applied,
    OutputWasMissing,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Workspace {
    pub id: u64,
    pub idx: u8,
    pub name: Option<String>,
    pub output: Option<String>,
    pub is_urgent: bool,
    pub is_active: bool,
    pub is_focused: bool,
    pub active_window_id: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct KeyboardLayouts {
    pub names: Vec<String>,
    pub current_idx: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Layer {
    Background,
    Bottom,
    Top,
    Overlay,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum LayerSurfaceKeyboardInteractivity {
    None,
    Exclusive,
    OnDemand,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct LayerSurface {
    pub namespace: String,
    pub output: String,
    pub layer: Layer,
    pub keyboard_interactivity: LayerSurfaceKeyboardInteractivity,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct Cast {
    pub stream_id: u64,
    pub session_id: u64,
    pub kind: CastKind,
    pub target: CastTarget,
    pub is_dynamic_target: bool,
    pub is_active: bool,
    pub pid: Option<i32>,
    pub pw_node_id: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum CastKind {
    PipeWire,
    WlrScreencopy,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum CastTarget {
    Nothing {},
    Output {
        name: String,
    },
    Window {
        id: u64,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum Event {
    WorkspacesChanged {
        workspaces: Vec<Workspace>,
    },
    WorkspaceUrgencyChanged {
        id: u64,
        urgent: bool,
    },
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WorkspaceActiveWindowChanged {
        workspace_id: u64,
        active_window_id: Option<u64>,
    },
    WindowsChanged {
        windows: Vec<Window>,
    },
    WindowOpenedOrChanged {
        window: Window,
    },
    WindowClosed {
        id: u64,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WindowFocusTimestampChanged {
        id: u64,
        focus_timestamp: Option<Timestamp>,
    },
    WindowUrgencyChanged {
        id: u64,
        urgent: bool,
    },
    WindowLayoutsChanged {
        changes: Vec<(u64, WindowLayout)>,
    },
    KeyboardLayoutsChanged {
        keyboard_layouts: KeyboardLayouts,
    },
    KeyboardLayoutSwitched {
        idx: u8,
    },
    OverviewOpenedOrClosed {
        is_open: bool,
    },
    ConfigLoaded {
        failed: bool,
    },
    ScreenshotCaptured {
        path: Option<String>,
    },
    CastsChanged {
        casts: Vec<Cast>,
    },
    CastStartedOrChanged {
        cast: Cast,
    },
    CastStopped {
        stream_id: u64,
    },
}

impl From<Duration> for Timestamp {
    fn from(value: Duration) -> Self {
        Timestamp {
            secs: value.as_secs(),
            nanos: value.subsec_nanos(),
        }
    }
}

impl From<Timestamp> for Duration {
    fn from(value: Timestamp) -> Self {
        Duration::new(value.secs, value.nanos)
    }
}

impl FromStr for WorkspaceReferenceArg {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let reference = if let Ok(index) = s.parse::<i32>() {
            if let Ok(idx) = u8::try_from(index) {
                Self::Index(idx)
            } else {
                return Err("workspace index must be between 0 and 255");
            }
        } else {
            Self::Name(s.to_string())
        };

        Ok(reference)
    }
}

impl FromStr for SizeChange {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('%') {
            Some((value, empty)) => {
                if !empty.is_empty() {
                    return Err("trailing characters after '%' are not allowed");
                }

                match value.bytes().next() {
                    Some(b'-' | b'+') => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::AdjustProportion(value))
                    }
                    Some(_) => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::SetProportion(value))
                    }
                    None => Err("value is missing"),
                }
            }
            None => {
                let value = s;
                match value.bytes().next() {
                    Some(b'-' | b'+') => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::AdjustFixed(value))
                    }
                    Some(_) => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::SetFixed(value))
                    }
                    None => Err("value is missing"),
                }
            }
        }
    }
}

impl FromStr for PositionChange {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('%') {
            Some((value, empty)) => {
                if !empty.is_empty() {
                    return Err("trailing characters after '%' are not allowed");
                }

                match value.bytes().next() {
                    Some(b'-' | b'+') => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::AdjustProportion(value))
                    }
                    Some(_) => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::SetProportion(value))
                    }
                    None => Err("value is missing"),
                }
            }
            None => {
                let value = s;
                match value.bytes().next() {
                    Some(b'-' | b'+') => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::AdjustFixed(value))
                    }
                    Some(_) => {
                        let value = value.parse().map_err(|_| "error parsing value")?;
                        Ok(Self::SetFixed(value))
                    }
                    None => Err("value is missing"),
                }
            }
        }
    }
}

impl FromStr for LayoutSwitchTarget {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "next" => Ok(Self::Next),
            "prev" => Ok(Self::Prev),
            other => match other.parse() {
                Ok(layout) => Ok(Self::Index(layout)),
                _ => Err(r#"invalid layout action, can be "next", "prev" or a layout index"#),
            },
        }
    }
}

impl FromStr for ColumnDisplay {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(Self::Normal),
            "tabbed" => Ok(Self::Tabbed),
            _ => Err(r#"invalid column display, can be "normal" or "tabbed""#),
        }
    }
}

impl FromStr for Transform {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(Self::Normal),
            "90" => Ok(Self::_90),
            "180" => Ok(Self::_180),
            "270" => Ok(Self::_270),
            "flipped" => Ok(Self::Flipped),
            "flipped-90" => Ok(Self::Flipped90),
            "flipped-180" => Ok(Self::Flipped180),
            "flipped-270" => Ok(Self::Flipped270),
            _ => Err(concat!(
                r#"invalid transform, can be "90", "180", "270", "#,
                r#""flipped", "flipped-90", "flipped-180" or "flipped-270""#
            )),
        }
    }
}

impl TryFrom<u8> for MaxBpc {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            6 => Ok(MaxBpc::_6),
            8 => Ok(MaxBpc::_8),
            10 => Ok(MaxBpc::_10),
            12 => Ok(MaxBpc::_12),
            14 => Ok(MaxBpc::_14),
            16 => Ok(MaxBpc::_16),
            _ => Err("invalid max-bpc, can be 6, 8, 10, 12, 14, 16"),
        }
    }
}

impl FromStr for MaxBpc {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.parse::<u8>().unwrap_or_default())
    }
}

impl FromStr for Layer {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "background" => Ok(Self::Background),
            "bottom" => Ok(Self::Bottom),
            "top" => Ok(Self::Top),
            "overlay" => Ok(Self::Overlay),
            _ => Err("invalid layer, can be \"background\", \"bottom\", \"top\" or \"overlay\""),
        }
    }
}

impl FromStr for ModeToSet {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("auto") {
            return Ok(Self::Automatic);
        }

        let mode = s.parse()?;
        Ok(Self::Specific(mode))
    }
}

impl FromStr for ConfiguredMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((width, rest)) = s.split_once('x') else {
            return Err("no 'x' separator found");
        };

        let (height, refresh) = match rest.split_once('@') {
            Some((height, refresh)) => (height, Some(refresh)),
            None => (rest, None),
        };

        let width = width.parse().map_err(|_| "error parsing width")?;
        let height = height.parse().map_err(|_| "error parsing height")?;
        let refresh = refresh
            .map(str::parse)
            .transpose()
            .map_err(|_| "error parsing refresh rate")?;

        Ok(Self {
            width,
            height,
            refresh,
        })
    }
}

impl FromStr for HSyncPolarity {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+hsync" => Ok(Self::PHSync),
            "-hsync" => Ok(Self::NHSync),
            _ => Err(r#"invalid horizontal sync polarity, can be "+hsync" or "-hsync"#),
        }
    }
}

impl FromStr for VSyncPolarity {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+vsync" => Ok(Self::PVSync),
            "-vsync" => Ok(Self::NVSync),
            _ => Err(r#"invalid vertical sync polarity, can be "+vsync" or "-vsync"#),
        }
    }
}

impl FromStr for ScaleToSet {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("auto") {
            return Ok(Self::Automatic);
        }

        let scale = s.parse().map_err(|_| "error parsing scale")?;
        Ok(Self::Specific(scale))
    }
}

macro_rules! ensure {
    ($cond:expr, $fmt:literal $($arg:tt)* ) => {
        if !$cond {
            return Err(format!($fmt $($arg)*));
        }
    };
}

impl OutputAction {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            OutputAction::Modeline {
                hdisplay,
                hsync_start,
                hsync_end,
                htotal,
                vdisplay,
                vsync_start,
                vsync_end,
                vtotal,
                ..
            } => {
                ensure!(
                    hdisplay < hsync_start,
                    "hdisplay {} must be < hsync_start {}",
                    hdisplay,
                    hsync_start
                );
                ensure!(
                    hsync_start < hsync_end,
                    "hsync_start {} must be < hsync_end {}",
                    hsync_start,
                    hsync_end
                );
                ensure!(
                    hsync_end < htotal,
                    "hsync_end {} must be < htotal {}",
                    hsync_end,
                    htotal
                );
                ensure!(0 < *htotal, "htotal {} must be > 0", htotal);
                ensure!(
                    vdisplay < vsync_start,
                    "vdisplay {} must be < vsync_start {}",
                    vdisplay,
                    vsync_start
                );
                ensure!(
                    vsync_start < vsync_end,
                    "vsync_start {} must be < vsync_end {}",
                    vsync_start,
                    vsync_end
                );
                ensure!(
                    vsync_end < vtotal,
                    "vsync_end {} must be < vtotal {}",
                    vsync_end,
                    vtotal
                );
                ensure!(0 < *vtotal, "vtotal {} must be > 0", vtotal);
                Ok(())
            }
            OutputAction::CustomMode {
                mode: ConfiguredMode { refresh, .. },
            } => {
                if refresh.is_none() {
                    return Err("refresh rate is required for custom modes".to_string());
                }
                if let Some(refresh) = refresh {
                    if *refresh <= 0. {
                        return Err(format!("custom mode refresh rate {refresh} must be > 0"));
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_change() {
        assert_eq!(
            "10".parse::<SizeChange>().unwrap(),
            SizeChange::SetFixed(10),
        );
        assert_eq!(
            "+10".parse::<SizeChange>().unwrap(),
            SizeChange::AdjustFixed(10),
        );
        assert_eq!(
            "-10".parse::<SizeChange>().unwrap(),
            SizeChange::AdjustFixed(-10),
        );
        assert_eq!(
            "10%".parse::<SizeChange>().unwrap(),
            SizeChange::SetProportion(10.),
        );
        assert_eq!(
            "+10%".parse::<SizeChange>().unwrap(),
            SizeChange::AdjustProportion(10.),
        );
        assert_eq!(
            "-10%".parse::<SizeChange>().unwrap(),
            SizeChange::AdjustProportion(-10.),
        );

        assert!("-".parse::<SizeChange>().is_err());
        assert!("10% ".parse::<SizeChange>().is_err());
    }

    #[test]
    fn parse_position_change() {
        assert_eq!(
            "10".parse::<PositionChange>().unwrap(),
            PositionChange::SetFixed(10.),
        );
        assert_eq!(
            "+10".parse::<PositionChange>().unwrap(),
            PositionChange::AdjustFixed(10.),
        );
        assert_eq!(
            "-10".parse::<PositionChange>().unwrap(),
            PositionChange::AdjustFixed(-10.),
        );

        assert_eq!(
            "10%".parse::<PositionChange>().unwrap(),
            PositionChange::SetProportion(10.)
        );
        assert_eq!(
            "+10%".parse::<PositionChange>().unwrap(),
            PositionChange::AdjustProportion(10.)
        );
        assert_eq!(
            "-10%".parse::<PositionChange>().unwrap(),
            PositionChange::AdjustProportion(-10.)
        );
        assert!("-".parse::<PositionChange>().is_err());
        assert!("10% ".parse::<PositionChange>().is_err());
    }
}
