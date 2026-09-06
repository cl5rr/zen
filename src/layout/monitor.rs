use std::cmp::min;
use std::iter::zip;
use std::rc::Rc;
use std::time::Duration;

use zen_config::{CornerRadius, LayoutPart};
use smithay::backend::renderer::element::utils::{
    CropRenderElement, Relocate, RelocateRenderElement, RescaleRenderElement,
};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Size};

use super::camera::Camera;
use crate::ui::canvas_clock::CanvasClock;
use super::insert_hint_element::{InsertHintElement, InsertHintRenderElement};
use super::island::{IslandId, SpawnContext, SpawnTarget};
use super::scrolling::{Column, ColumnWidth};
use super::tile::Tile;
use super::workspace::{
    compute_working_area, OutputId, Workspace, WorkspaceAddWindowTarget, WorkspaceId,
    WorkspaceRenderElement,
};
use super::{compute_overview_zoom, ActivateWindow, Canvas, HitType, LayoutElement, Options};
use crate::animation::{Animation, Clock};
use crate::input::swipe_tracker::SwipeTracker;
use crate::layout::RenderLayer;
use crate::zen_render_elements;
use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::shadow::ShadowRenderElement;
use crate::render_helpers::solid_color::SolidColorRenderElement;
use crate::render_helpers::xray::XrayPos;
use crate::render_helpers::RenderCtx;
use crate::rubber_band::RubberBand;
use crate::utils::transaction::Transaction;
use crate::utils::{
    output_size, round_logical_in_physical, round_logical_in_physical_max1, ResizeEdge,
};

const WORKSPACE_GESTURE_MOVEMENT: f64 = 300.;

const WORKSPACE_GESTURE_RUBBER_BAND: RubberBand = RubberBand {
    stiffness: 0.5,
    limit: 0.05,
};

const WORKSPACE_DND_EDGE_SCROLL_MOVEMENT: f64 = 1500.;

#[derive(Debug)]
pub struct Monitor<W: LayoutElement> {
    pub(super) output: Output,
    output_name: String,
    scale: smithay::output::Scale,
    view_size: Size<f64, Logical>,
    working_area: Rectangle<f64, Logical>,
    pub(super) workspaces: Vec<Workspace<W>>,
    pub(super) active_workspace_idx: usize,
    pub(super) previous_workspace_id: Option<WorkspaceId>,
    pub(super) workspace_switch: Option<WorkspaceSwitch>,
    pub(super) insert_hint: Option<InsertHint>,
    insert_hint_element: InsertHintElement,
    insert_hint_render_loc: Option<InsertHintRenderLoc>,
    pub(super) overview_open: bool,
    overview_progress: Option<OverviewProgress>,
    camera_focus: Option<(W::Id, Rectangle<f64, Logical>)>,

    canvas_clock: CanvasClock,
    pub(super) camera: Camera,
    pub(super) clock: Clock,
    pub(super) base_options: Rc<Options>,
    pub(super) options: Rc<Options>,
    layout_config: Option<zen_config::LayoutPart>,
}

#[derive(Debug)]
pub enum WorkspaceSwitch {
    Animation(Animation),
    Gesture(WorkspaceSwitchGesture),
}

#[derive(Debug)]
pub struct WorkspaceSwitchGesture {
    center_idx: usize,
    start_idx: f64,
    pub(super) current_idx: f64,
    animation: Option<Animation>,
    tracker: SwipeTracker,
    is_touchpad: bool,
    is_clamped: bool,

    dnd_last_event_time: Option<Duration>,
    dnd_nonzero_start_time: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InsertPosition {
    NewColumn(usize),
    InColumn(usize, usize),
    Floating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InsertWorkspace {
    Existing(WorkspaceId),
    NewAt(usize),
}

#[derive(Debug)]
pub(super) struct InsertHint {
    pub workspace: InsertWorkspace,
    pub position: InsertPosition,
    pub corner_radius: CornerRadius,
}

#[derive(Debug, Clone, Copy)]
struct InsertHintRenderLoc {
    workspace: InsertWorkspace,
    location: Point<f64, Logical>,
}

#[derive(Debug)]
pub(super) enum OverviewProgress {
    Animation(Animation),
    Value(f64),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum MonitorAddWindowTarget<'a, W: LayoutElement> {
    #[default]
    Auto,
    Workspace {
        id: WorkspaceId,
        column_idx: Option<usize>,
    },
    NextTo(&'a W::Id),
    Island(IslandId),
}

impl<'a, W: LayoutElement> Copy for MonitorAddWindowTarget<'a, W> {}

impl<'a, W: LayoutElement> Clone for MonitorAddWindowTarget<'a, W> {
    fn clone(&self) -> Self {
        *self
    }
}

zen_render_elements! {
    MonitorInnerRenderElement<R> => {
        Workspace = CropRenderElement<WorkspaceRenderElement<R>>,
        InsertHint = CropRenderElement<InsertHintRenderElement>,
        UncroppedInsertHint = InsertHintRenderElement,
        Shadow = ShadowRenderElement,
        SolidColor = SolidColorRenderElement,
        Texture = PrimaryGpuTextureRenderElement,
    }
}

pub type MonitorRenderElement<R> =
    RelocateRenderElement<RescaleRenderElement<MonitorInnerRenderElement<R>>>;

impl WorkspaceSwitch {
    pub fn current_idx(&self) -> f64 {
        match self {
            WorkspaceSwitch::Animation(anim) => anim.value(),
            WorkspaceSwitch::Gesture(gesture) => {
                gesture.current_idx + gesture.animation.as_ref().map_or(0., |anim| anim.value())
            }
        }
    }

    pub fn target_idx(&self) -> f64 {
        match self {
            WorkspaceSwitch::Animation(anim) => anim.to(),
            WorkspaceSwitch::Gesture(gesture) => gesture.current_idx,
        }
    }

    pub fn offset(&mut self, delta: isize) {
        match self {
            WorkspaceSwitch::Animation(anim) => anim.offset(delta as f64),
            WorkspaceSwitch::Gesture(gesture) => {
                if delta >= 0 {
                    gesture.center_idx += delta as usize;
                } else {
                    gesture.center_idx -= (-delta) as usize;
                }
                gesture.start_idx += delta as f64;
                gesture.current_idx += delta as f64;
            }
        }
    }

    fn is_animation_ongoing(&self) -> bool {
        match self {
            WorkspaceSwitch::Animation(_) => true,
            WorkspaceSwitch::Gesture(gesture) => gesture.animation.is_some(),
        }
    }
}

impl WorkspaceSwitchGesture {
    fn min_max(&self, workspace_count: usize) -> (f64, f64) {
        if self.is_clamped {
            let min = self.center_idx.saturating_sub(1) as f64;
            let max = (self.center_idx + 1).min(workspace_count - 1) as f64;
            (min, max)
        } else {
            (0., (workspace_count - 1) as f64)
        }
    }

    fn animate_from(&mut self, from: f64, clock: Clock, config: zen_config::Animation) {
        let current = self.animation.as_ref().map_or(0., Animation::value);
        self.animation = Some(Animation::new(clock, from + current, 0., 0., config));
    }
}

impl InsertWorkspace {
    fn existing_id(self) -> Option<WorkspaceId> {
        match self {
            InsertWorkspace::Existing(id) => Some(id),
            InsertWorkspace::NewAt(_) => None,
        }
    }
}

impl OverviewProgress {
    pub fn value(&self) -> f64 {
        match self {
            OverviewProgress::Animation(anim) => anim.value(),
            OverviewProgress::Value(v) => *v,
        }
    }

    pub fn clamped_value(&self) -> f64 {
        match self {
            OverviewProgress::Animation(anim) => anim.clamped_value(),
            OverviewProgress::Value(v) => *v,
        }
    }
}

impl From<&super::OverviewProgress> for OverviewProgress {
    fn from(value: &super::OverviewProgress) -> Self {
        match value {
            super::OverviewProgress::Animation(anim) => Self::Animation(anim.clone()),
            super::OverviewProgress::Gesture(gesture) => Self::Value(gesture.value),
            super::OverviewProgress::Open => Self::Value(1.),
        }
    }
}

pub const CAMERA_FOCUS_PADDING: f64 = 24.;

pub(crate) fn quantize_camera_scale(zoom: f64) -> f64 {
    let zoom = zoom.max(1.0);
    (zoom * 4.).round() / 4.
}

impl<W: LayoutElement> Monitor<W> {
    pub fn new(
        output: Output,
        mut workspaces: Vec<Workspace<W>>,
        ws_id_to_activate: Option<WorkspaceId>,
        clock: Clock,
        base_options: Rc<Options>,
        layout_config: Option<LayoutPart>,
    ) -> Self {
        let options =
            Rc::new(Options::clone(&base_options).with_merged_layout(layout_config.as_ref()));

        let scale = output.current_scale();
        let view_size = output_size(&output);
        let working_area = compute_working_area(&output);

        let mut active_workspace_idx = 0;

        for (idx, ws) in workspaces.iter_mut().enumerate() {
            assert!(ws.has_windows_or_name());

            ws.set_output(Some(output.clone()));
            ws.update_config(options.clone());

            if ws_id_to_activate.is_some_and(|id| ws.id() == id) {
                active_workspace_idx = idx;
            }
        }

        if options.layout.empty_workspace_above_first && !workspaces.is_empty() {
            let ws = Workspace::new(output.clone(), clock.clone(), options.clone());
            workspaces.insert(0, ws);
            active_workspace_idx += 1;
        }

        let ws = Workspace::new(output.clone(), clock.clone(), options.clone());
        workspaces.push(ws);

        Self {
            output_name: output.name(),
            output,
            scale,
            view_size,
            working_area,
            workspaces,
            active_workspace_idx,
            previous_workspace_id: None,
            insert_hint: None,
            insert_hint_element: InsertHintElement::new(options.layout.insert_hint),
            insert_hint_render_loc: None,
            overview_open: false,
            overview_progress: None,
            camera_focus: None,
            canvas_clock: CanvasClock::new(options.widgets.clock.clone()),
            camera: Camera::new(
                clock.clone(),
                view_size,
                options.camera.min_zoom,
                options.camera.max_zoom,
            ),
            workspace_switch: None,
            clock,
            base_options,
            options,
            layout_config,
        }
    }

    pub fn into_workspaces(mut self) -> Vec<Workspace<W>> {
        self.workspaces.retain(|ws| ws.has_windows_or_name());

        for ws in &mut self.workspaces {
            ws.set_output(None);
        }

        self.workspaces
    }

    pub fn output(&self) -> &Output {
        &self.output
    }

    pub fn output_name(&self) -> &String {
        &self.output_name
    }

    pub fn active_workspace_idx(&self) -> usize {
        self.active_workspace_idx
    }

    pub fn active_workspace_ref(&self) -> &Workspace<W> {
        &self.workspaces[self.active_workspace_idx]
    }

    pub fn find_named_workspace(&self, workspace_name: &str) -> Option<&Workspace<W>> {
        self.workspaces.iter().find(|ws| {
            ws.name
                .as_ref()
                .is_some_and(|name| name.eq_ignore_ascii_case(workspace_name))
        })
    }

    pub fn find_named_workspace_index(&self, workspace_name: &str) -> Option<usize> {
        self.workspaces.iter().position(|ws| {
            ws.name
                .as_ref()
                .is_some_and(|name| name.eq_ignore_ascii_case(workspace_name))
        })
    }

    pub fn active_workspace(&mut self) -> &mut Workspace<W> {
        &mut self.workspaces[self.active_workspace_idx]
    }

    pub fn idx_of_ws(&self, id: WorkspaceId) -> Option<usize> {
        self.workspaces.iter().position(|ws| ws.id() == id)
    }

    pub fn has_ws(&self, id: WorkspaceId) -> bool {
        self.idx_of_ws(id).is_some()
    }

    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.workspaces.iter().flat_map(|ws| ws.windows())
    }

    pub fn has_window(&self, window: &W::Id) -> bool {
        self.windows().any(|win| win.id() == window)
    }

    pub fn add_workspace_at(&mut self, idx: usize) {
        let ws = Workspace::new(
            self.output.clone(),
            self.clock.clone(),
            self.options.clone(),
        );

        self.workspaces.insert(idx, ws);
        if idx <= self.active_workspace_idx {
            self.active_workspace_idx += 1;
        }

        if let Some(switch) = &mut self.workspace_switch {
            if idx as f64 <= switch.target_idx() {
                switch.offset(1);
            }
        }
    }

    pub fn add_workspace_top(&mut self) {
        self.add_workspace_at(0);
    }

    pub fn add_workspace_bottom(&mut self) {
        self.add_workspace_at(self.workspaces.len());
    }

    pub fn activate_workspace(&mut self, idx: usize) {
        self.activate_workspace_with_anim_config(idx, None);
    }

    pub fn activate_workspace_with_anim_config(
        &mut self,
        idx: usize,
        config: Option<zen_config::Animation>,
    ) {
        let current_idx = self.workspace_render_idx();

        if self.active_workspace_idx != idx {
            self.previous_workspace_id = Some(self.workspaces[self.active_workspace_idx].id());
        }

        let prev_active_idx = self.active_workspace_idx;
        self.active_workspace_idx = idx;

        let config = config.unwrap_or(self.options.animations.workspace_switch.0);

        match &mut self.workspace_switch {
            Some(WorkspaceSwitch::Gesture(gesture)) if gesture.dnd_last_event_time.is_some() => {
                gesture.center_idx = idx;

                let current_pos = gesture.current_idx - gesture.start_idx;
                gesture.start_idx = idx as f64 - current_pos;
                let prev_current_idx = gesture.current_idx;
                gesture.current_idx = idx as f64;

                let current_idx_delta = gesture.current_idx - prev_current_idx;
                gesture.animate_from(-current_idx_delta, self.clock.clone(), config);
            }
            _ => {
                if prev_active_idx == idx {
                    return;
                }

                self.workspace_switch = Some(WorkspaceSwitch::Animation(Animation::new(
                    self.clock.clone(),
                    current_idx,
                    idx as f64,
                    0.,
                    config,
                )));
            }
        }
    }

    pub(super) fn resolve_add_window_target<'a>(
        &mut self,
        target: MonitorAddWindowTarget<'a, W>,
    ) -> (usize, WorkspaceAddWindowTarget<'a, W>) {
        match target {
            MonitorAddWindowTarget::Auto => {
                (self.active_workspace_idx, WorkspaceAddWindowTarget::Auto)
            }
            MonitorAddWindowTarget::Workspace { id, column_idx } => {
                let idx = self.idx_of_ws(id).unwrap();
                let target = if let Some(column_idx) = column_idx {
                    WorkspaceAddWindowTarget::NewColumnAt(column_idx)
                } else {
                    WorkspaceAddWindowTarget::Auto
                };
                (idx, target)
            }
            MonitorAddWindowTarget::NextTo(win_id) => {
                let idx = self
                    .workspaces
                    .iter_mut()
                    .position(|ws| ws.has_window(win_id))
                    .unwrap();
                (idx, WorkspaceAddWindowTarget::NextTo(win_id))
            }
            MonitorAddWindowTarget::Island(island) => {
                match self.workspaces.iter().position(|ws| ws.has_island(island)) {
                    Some(idx) => (idx, WorkspaceAddWindowTarget::Island(island)),
                    None => (self.active_workspace_idx, WorkspaceAddWindowTarget::Auto),
                }
            }
        }
    }

    pub fn add_window(
        &mut self,
        window: W,
        target: MonitorAddWindowTarget<W>,
        activate: ActivateWindow,
        width: ColumnWidth,
        is_full_width: bool,
        is_floating: bool,
    ) {
        self.sync_spawn_center();

        let tile = self.workspaces[0].make_tile(window);

        self.add_tile(
            tile,
            target,
            activate,
            true,
            width,
            is_full_width,
            is_floating,
            None,
        );
    }

    pub fn add_column(
        &mut self,
        mut workspace_idx: usize,
        column: Column<W>,
        activate: bool,
        anim: Option<zen_config::Animation>,
    ) {
        let workspace = &mut self.workspaces[workspace_idx];

        workspace.add_column(column, activate, anim);

        if workspace.name().is_none() {
            workspace.original_output = OutputId::new(&self.output);
        }

        if workspace_idx == self.workspaces.len() - 1 {
            self.add_workspace_bottom();
        }
        if self.options.layout.empty_workspace_above_first && workspace_idx == 0 {
            self.add_workspace_top();
            workspace_idx += 1;
        }

        if activate {
            self.activate_workspace(workspace_idx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_tile(
        &mut self,
        tile: Tile<W>,
        target: MonitorAddWindowTarget<W>,
        activate: ActivateWindow,
        allow_to_activate_workspace: bool,
        width: ColumnWidth,
        is_full_width: bool,
        is_floating: bool,
        anim: Option<zen_config::Animation>,
    ) {
        let (mut workspace_idx, target) = self.resolve_add_window_target(target);

        let workspace = &mut self.workspaces[workspace_idx];

        workspace.add_tile(
            tile,
            target,
            activate,
            width,
            is_full_width,
            is_floating,
            anim,
        );

        if workspace.name().is_none() {
            workspace.original_output = OutputId::new(&self.output);
        }

        if workspace_idx == self.workspaces.len() - 1 {
            self.add_workspace_bottom();
        }

        if self.options.layout.empty_workspace_above_first && workspace_idx == 0 {
            self.add_workspace_top();
            workspace_idx += 1;
        }

        if allow_to_activate_workspace && activate.map_smart(|| false) {
            self.activate_workspace(workspace_idx);
        }
    }

    pub fn add_tile_to_column(
        &mut self,
        workspace_idx: usize,
        column_idx: usize,
        tile_idx: Option<usize>,
        tile: Tile<W>,
        activate: bool,
        allow_to_activate_workspace: bool,
    ) {
        let workspace = &mut self.workspaces[workspace_idx];

        workspace.add_tile_to_column(column_idx, tile_idx, tile, activate);

        if workspace.name().is_none() {
            workspace.original_output = OutputId::new(&self.output);
        }

        if allow_to_activate_workspace && activate {
            self.activate_workspace(workspace_idx);
        }
    }

    pub fn clean_up_workspaces(&mut self) {
        assert!(self.workspace_switch.is_none());

        let range_start = if self.options.layout.empty_workspace_above_first {
            1
        } else {
            0
        };
        for idx in (range_start..self.workspaces.len() - 1).rev() {
            if self.active_workspace_idx == idx {
                continue;
            }

            if !self.workspaces[idx].has_windows_or_name() {
                self.workspaces.remove(idx);
                if self.active_workspace_idx > idx {
                    self.active_workspace_idx -= 1;
                }
            }
        }

        if self.options.layout.empty_workspace_above_first && self.workspaces.len() == 2 {
            assert!(!self.workspaces[0].has_windows_or_name());
            assert!(!self.workspaces[1].has_windows_or_name());
            self.workspaces.remove(1);
            self.active_workspace_idx = 0;
        }
    }

    pub fn unname_workspace(&mut self, id: WorkspaceId) -> bool {
        let Some(idx) = self.idx_of_ws(id) else {
            return false;
        };
        let ws = &mut self.workspaces[idx];

        ws.unname();

        if self.workspace_switch.is_none() {
            self.clean_up_workspaces();
        }

        true
    }

    pub fn remove_workspace_by_idx(&mut self, mut idx: usize) -> Workspace<W> {
        if idx == self.workspaces.len() - 1 {
            self.add_workspace_bottom();
        }
        if self.options.layout.empty_workspace_above_first && idx == 0 {
            self.add_workspace_top();
            idx += 1;
        }

        let mut ws = self.workspaces.remove(idx);
        ws.set_output(None);

        if idx <= self.active_workspace_idx && self.active_workspace_idx > 0 {
            self.active_workspace_idx -= 1;
        }

        self.workspace_switch = None;
        self.clean_up_workspaces();

        ws
    }

    pub fn insert_workspace(&mut self, mut ws: Workspace<W>, mut idx: usize, activate: bool) {
        ws.set_output(Some(self.output.clone()));
        ws.update_config(self.options.clone());

        if idx == self.workspaces.len() {
            idx -= 1;
        }
        if idx == 0 && self.options.layout.empty_workspace_above_first {
            self.add_workspace_top();
            idx += 1;
        }

        self.workspaces.insert(idx, ws);

        if idx <= self.active_workspace_idx {
            self.active_workspace_idx += 1;
        }

        if activate {
            self.workspace_switch = None;
            self.activate_workspace(idx);
        }

        self.workspace_switch = None;
        self.clean_up_workspaces();
    }

    pub fn append_workspaces(&mut self, mut workspaces: Vec<Workspace<W>>) {
        if workspaces.is_empty() {
            return;
        }

        for ws in &mut workspaces {
            ws.set_output(Some(self.output.clone()));
            ws.update_config(self.options.clone());
        }

        let empty_was_focused = self.active_workspace_idx == self.workspaces.len() - 1;

        let empty = self.workspaces.remove(self.workspaces.len() - 1);
        self.workspaces.extend(workspaces);
        self.workspaces.push(empty);

        if self.options.layout.empty_workspace_above_first
            && self.workspaces[0].has_windows_or_name()
        {
            self.add_workspace_top();
        }

        if empty_was_focused {
            self.active_workspace_idx = self.workspaces.len() - 1;
        }

        self.workspace_switch = None;
        self.clean_up_workspaces();
    }

    pub fn move_down_or_to_workspace_down(&mut self) {
        if !self.active_workspace().move_down() {
            self.move_to_workspace_down(ActivateWindow::Smart);
        }
    }

    pub fn move_up_or_to_workspace_up(&mut self) {
        if !self.active_workspace().move_up() {
            self.move_to_workspace_up(ActivateWindow::Smart);
        }
    }

    pub fn focus_window_or_workspace_down(&mut self) {
        if !self.active_workspace().focus_down() {
            self.switch_workspace_down();
        }
    }

    pub fn focus_window_or_workspace_up(&mut self) {
        if !self.active_workspace().focus_up() {
            self.switch_workspace_up();
        }
    }

    pub fn move_to_workspace_up(&mut self, activate: ActivateWindow) {
        let new_idx = self.active_workspace_idx.saturating_sub(1);
        self.move_to_workspace(None, new_idx, activate);
    }

    pub fn move_to_workspace_down(&mut self, activate: ActivateWindow) {
        let new_idx = min(self.active_workspace_idx + 1, self.workspaces.len() - 1);
        self.move_to_workspace(None, new_idx, activate);
    }

    pub fn move_to_workspace(
        &mut self,
        window: Option<&W::Id>,
        idx: usize,
        activate: ActivateWindow,
    ) {
        let source_workspace_idx = if let Some(window) = window {
            self.workspaces
                .iter()
                .position(|ws| ws.has_window(window))
                .unwrap()
        } else {
            self.active_workspace_idx
        };
        let source_id = self.workspaces[source_workspace_idx].id();

        let new_idx = min(idx, self.workspaces.len() - 1);
        if new_idx == source_workspace_idx {
            return;
        }
        let new_id = self.workspaces[new_idx].id();

        let activate = activate.map_smart(|| {
            window.is_none_or(|win| self.active_window().map(|win| win.id()) == Some(win))
        });

        let workspace = &mut self.workspaces[source_workspace_idx];
        let Some(window) = window.or_else(|| workspace.active_window().map(|win| win.id())) else {
            return;
        };
        let window = window.clone();

        let mut old_render_pos = workspace
            .tiles_with_render_positions()
            .find_map(|(tile, offset, _visible)| (tile.window().id() == &window).then_some(offset))
            .unwrap();

        let transaction = Transaction::new();
        let removed = workspace.remove_tile(&window, transaction);

        let config = if activate {
            self.options.animations.workspace_switch.0
        } else {
            self.options.animations.window_movement.0
        };

        self.add_tile(
            removed.tile,
            MonitorAddWindowTarget::Workspace {
                id: new_id,
                column_idx: None,
            },
            if activate {
                ActivateWindow::Yes
            } else {
                ActivateWindow::No
            },
            true,
            removed.width,
            removed.is_full_width,
            removed.is_floating,
            Some(config),
        );

        if self.workspace_switch.is_none() {
            self.clean_up_workspaces();
        }

        let new_idx = self.idx_of_ws(new_id).unwrap();

        if let Some(source_workspace_idx) = self.idx_of_ws(source_id) {
            old_render_pos.y +=
                self.workspace_size_with_gap(1.).h * (source_workspace_idx as f64 - new_idx as f64);
        }

        let (tile, new_render_pos) = self.workspaces[new_idx]
            .tiles_with_render_positions_mut(false)
            .find(|(tile, _)| tile.window().id() == &window)
            .unwrap();
        tile.animate_move_from_with_config(old_render_pos - new_render_pos, config);
        tile.set_anim_y_between_workspaces();
    }

    pub fn move_column_to_workspace_up(&mut self, activate: bool) {
        let new_idx = self.active_workspace_idx.saturating_sub(1);
        self.move_column_to_workspace(new_idx, activate);
    }

    pub fn move_column_to_workspace_down(&mut self, activate: bool) {
        let new_idx = min(self.active_workspace_idx + 1, self.workspaces.len() - 1);
        self.move_column_to_workspace(new_idx, activate);
    }

    pub fn move_column_to_workspace(&mut self, idx: usize, activate: bool) {
        let source_workspace_idx = self.active_workspace_idx;

        let new_idx = min(idx, self.workspaces.len() - 1);
        if new_idx == source_workspace_idx {
            return;
        }

        let workspace = &mut self.workspaces[source_workspace_idx];
        if workspace.floating_is_active() {
            let activate = if activate {
                ActivateWindow::Smart
            } else {
                ActivateWindow::No
            };
            self.move_to_workspace(None, idx, activate);
            return;
        }

        let Some(id) = workspace.scrolling().active_column().map(Column::id) else {
            return;
        };
        let mut old_render_pos = workspace
            .scrolling()
            .columns_with_render_positions()
            .find_map(|(col, pos)| (col.id() == id).then_some(pos))
            .unwrap();

        let column = workspace.remove_active_column().unwrap();

        old_render_pos.y +=
            self.workspace_size_with_gap(1.).h * (source_workspace_idx as f64 - new_idx as f64);

        let config = if activate {
            self.options.animations.workspace_switch.0
        } else {
            self.options.animations.window_movement.0
        };

        let new_id = self.workspaces[new_idx].id();
        self.add_column(new_idx, column, activate, Some(config));

        let new_idx = self.idx_of_ws(new_id).unwrap();
        let (column, new_render_pos) = self.workspaces[new_idx]
            .scrolling_mut()
            .columns_with_render_positions_mut()
            .find(|(col, _pos)| col.id() == id)
            .unwrap();
        column.animate_move_from_with_config(old_render_pos - new_render_pos, config);
        column.set_anim_y_between_workspaces();
    }

    pub fn switch_workspace_up(&mut self) {
        let new_idx = match &self.workspace_switch {
            Some(WorkspaceSwitch::Gesture(gesture)) if gesture.dnd_last_event_time.is_some() => {
                let current = gesture.current_idx;
                let new = current.ceil() - 1.;
                new.clamp(0., (self.workspaces.len() - 1) as f64) as usize
            }
            _ => self.active_workspace_idx.saturating_sub(1),
        };

        self.activate_workspace(new_idx);
    }

    pub fn switch_workspace_down(&mut self) {
        let new_idx = match &self.workspace_switch {
            Some(WorkspaceSwitch::Gesture(gesture)) if gesture.dnd_last_event_time.is_some() => {
                let current = gesture.current_idx;
                let new = current.floor() + 1.;
                new.clamp(0., (self.workspaces.len() - 1) as f64) as usize
            }
            _ => min(self.active_workspace_idx + 1, self.workspaces.len() - 1),
        };

        self.activate_workspace(new_idx);
    }

    fn previous_workspace_idx(&self) -> Option<usize> {
        let id = self.previous_workspace_id?;
        self.idx_of_ws(id)
    }

    pub fn switch_workspace(&mut self, idx: usize) {
        self.activate_workspace(min(idx, self.workspaces.len() - 1));
    }

    pub fn switch_workspace_auto_back_and_forth(&mut self, idx: usize) {
        let idx = min(idx, self.workspaces.len() - 1);

        if idx == self.active_workspace_idx {
            if let Some(prev_idx) = self.previous_workspace_idx() {
                self.switch_workspace(prev_idx);
            }
        } else {
            self.switch_workspace(idx);
        }
    }

    pub fn switch_workspace_previous(&mut self) {
        if let Some(idx) = self.previous_workspace_idx() {
            self.switch_workspace(idx);
        }
    }

    pub fn active_window(&self) -> Option<&W> {
        self.active_workspace_ref().active_window()
    }

    pub fn advance_animations(&mut self) {
        self.sync_camera_focus(self.options.animations.overview_open_close.0);
        self.camera.advance_animations();
        self.sync_camera_scale();

        match &mut self.workspace_switch {
            Some(WorkspaceSwitch::Animation(anim)) => {
                if anim.is_done() {
                    self.workspace_switch = None;
                    self.clean_up_workspaces();
                }
            }
            Some(WorkspaceSwitch::Gesture(gesture)) => {
                if let Some(last_time) = &mut gesture.dnd_last_event_time {
                    let now = self.clock.now_unadjusted();
                    if *last_time != now {
                        *last_time = now;

                        gesture.dnd_nonzero_start_time = None;
                    }
                }

                if let Some(anim) = &mut gesture.animation {
                    if anim.is_done() {
                        gesture.animation = None;
                    }
                }
            }
            None => (),
        }

        for ws in &mut self.workspaces {
            ws.advance_animations();
        }
    }

    pub(super) fn are_animations_ongoing(&self) -> bool {
        self.camera.is_animating()
            || self
                .workspace_switch
                .as_ref()
                .is_some_and(|s| s.is_animation_ongoing())
            || self.workspaces.iter().any(|ws| ws.are_animations_ongoing())
    }

    pub fn are_transitions_ongoing(&self) -> bool {
        self.workspace_switch.is_some()
            || self
                .workspaces
                .iter()
                .any(|ws| ws.are_transitions_ongoing())
    }

    pub fn update_render_elements(&mut self, is_active: bool) {
        let mut insert_hint_ws_geo = None;
        let insert_hint_ws_id = self
            .insert_hint
            .as_ref()
            .and_then(|hint| hint.workspace.existing_id());

        for ws in &mut self.workspaces {
            ws.update_render_elements(is_active, RenderLayer::MovingBetweenWorkspaces);
        }

        for (ws, geo) in self.workspaces_with_render_geo_mut(true) {
            ws.update_render_elements(is_active, RenderLayer::Normal);

            if Some(ws.id()) == insert_hint_ws_id {
                insert_hint_ws_geo = Some(geo);
            }
        }

        self.insert_hint_render_loc = None;
        if let Some(hint) = &self.insert_hint {
            match hint.workspace {
                InsertWorkspace::Existing(ws_id) => {
                    if let Some(idx) = self.idx_of_ws(ws_id) {
                        let ws = &self.workspaces[idx];
                        if let Some(mut area) = ws.insert_hint_area(hint.position) {
                            let scale = ws.scale().fractional_scale();
                            let view_size = ws.view_size();

                            if matches!(hint.position, InsertPosition::NewColumn(_)) {
                                let zoom = self.overview_zoom();
                                let geo = insert_hint_ws_geo.unwrap();
                                let geo = geo.downscale(zoom);

                                area.loc.x = area.loc.x.max(-geo.loc.x - area.size.w / 2.);
                                area.loc.x =
                                    area.loc.x.min(geo.loc.x + geo.size.w - area.size.w / 2.);
                            }

                            area = area.to_physical_precise_round(scale).to_logical(scale);

                            let view_rect = Rectangle::new(area.loc.upscale(-1.), view_size);
                            self.insert_hint_element.update_render_elements(
                                area.size,
                                view_rect,
                                hint.corner_radius,
                                scale,
                            );
                            self.insert_hint_render_loc = Some(InsertHintRenderLoc {
                                workspace: hint.workspace,
                                location: area.loc,
                            });
                        }
                    } else {
                        error!("insert hint workspace missing from monitor");
                    }
                }
                InsertWorkspace::NewAt(ws_idx) => {
                    let scale = self.scale.fractional_scale();
                    let zoom = self.overview_zoom();
                    let gap = self.workspace_gap(zoom);

                    let hint_gap = round_logical_in_physical(scale, gap * 0.1);
                    let hint_height = gap - hint_gap * 2.;

                    let next_ws_geo = self.workspaces_render_geo().nth(ws_idx).unwrap();
                    let hint_width = round_logical_in_physical(scale, next_ws_geo.size.w * 0.75);
                    let hint_x =
                        round_logical_in_physical(scale, (next_ws_geo.size.w - hint_width) / 2.);

                    let hint_loc_diff = Point::from((-hint_x, hint_height + hint_gap));
                    let hint_loc = next_ws_geo.loc - hint_loc_diff;
                    let hint_size = Size::from((hint_width, hint_height));

                    let view_rect = Rectangle::new(hint_loc_diff, next_ws_geo.size);

                    self.insert_hint_element.update_render_elements(
                        hint_size,
                        view_rect,
                        CornerRadius::default(),
                        scale,
                    );
                    self.insert_hint_render_loc = Some(InsertHintRenderLoc {
                        workspace: hint.workspace,
                        location: hint_loc,
                    });
                }
            }
        }
    }

    pub fn update_config(&mut self, base_options: Rc<Options>) {
        let options =
            Rc::new(Options::clone(&base_options).with_merged_layout(self.layout_config.as_ref()));

        if self.options.layout.empty_workspace_above_first
            != options.layout.empty_workspace_above_first
            && self.workspaces.len() > 1
        {
            if options.layout.empty_workspace_above_first {
                self.add_workspace_top();
            } else if self.workspace_switch.is_none() && self.active_workspace_idx != 0 {
                self.workspaces.remove(0);
                self.active_workspace_idx = self.active_workspace_idx.saturating_sub(1);
            }
        }

        self.camera
            .update_config(options.camera.min_zoom, options.camera.max_zoom);
        self.canvas_clock
            .update_config(options.widgets.clock.clone());

        for ws in &mut self.workspaces {
            ws.update_config(options.clone());
        }

        self.insert_hint_element
            .update_config(options.layout.insert_hint);

        self.base_options = base_options;
        self.options = options;
    }

    pub fn update_layout_config(&mut self, layout_config: Option<zen_config::LayoutPart>) -> bool {
        if self.layout_config == layout_config {
            return false;
        }

        self.layout_config = layout_config;
        self.update_config(self.base_options.clone());

        true
    }

    pub fn update_shaders(&mut self) {
        for ws in &mut self.workspaces {
            ws.update_shaders();
        }

        self.insert_hint_element.update_shaders();
    }

    pub fn update_output_size(&mut self) {
        self.scale = self.output.current_scale();
        self.view_size = output_size(&self.output);
        self.working_area = compute_working_area(&self.output);
        self.camera.set_view_size(self.view_size);

        for ws in &mut self.workspaces {
            ws.update_output_size();
        }
    }

    pub fn move_workspace_down(&mut self) {
        let mut new_idx = min(self.active_workspace_idx + 1, self.workspaces.len() - 1);
        if new_idx == self.active_workspace_idx {
            return;
        }

        self.workspaces.swap(self.active_workspace_idx, new_idx);

        if new_idx == self.workspaces.len() - 1 {
            self.add_workspace_bottom();
        }

        if self.options.layout.empty_workspace_above_first && self.active_workspace_idx == 0 {
            self.add_workspace_top();
            new_idx += 1;
        }

        let previous_workspace_id = self.previous_workspace_id;
        self.activate_workspace(new_idx);
        self.workspace_switch = None;
        self.previous_workspace_id = previous_workspace_id;

        self.clean_up_workspaces();
    }

    pub fn move_workspace_up(&mut self) {
        let mut new_idx = self.active_workspace_idx.saturating_sub(1);
        if new_idx == self.active_workspace_idx {
            return;
        }

        self.workspaces.swap(self.active_workspace_idx, new_idx);

        if self.active_workspace_idx == self.workspaces.len() - 1 {
            self.add_workspace_bottom();
        }

        if self.options.layout.empty_workspace_above_first && new_idx == 0 {
            self.add_workspace_top();
            new_idx += 1;
        }

        let previous_workspace_id = self.previous_workspace_id;
        self.activate_workspace(new_idx);
        self.workspace_switch = None;
        self.previous_workspace_id = previous_workspace_id;

        self.clean_up_workspaces();
    }

    pub fn move_workspace_to_idx(&mut self, old_idx: usize, new_idx: usize) {
        if self.workspaces.len() <= old_idx {
            return;
        }

        let mut new_idx = new_idx.clamp(0, self.workspaces.len() - 1);
        if old_idx == new_idx {
            return;
        }

        let ws = self.workspaces.remove(old_idx);
        self.workspaces.insert(new_idx, ws);

        if new_idx > old_idx {
            if new_idx == self.workspaces.len() - 1 {
                self.add_workspace_bottom();
            }

            if self.options.layout.empty_workspace_above_first && old_idx == 0 {
                self.add_workspace_top();
                new_idx += 1;
            }
        } else {
            if old_idx == self.workspaces.len() - 1 {
                self.add_workspace_bottom();
            }

            if self.options.layout.empty_workspace_above_first && new_idx == 0 {
                self.add_workspace_top();
                new_idx += 1;
            }
        }

        if self.active_workspace_idx == old_idx {
            self.active_workspace_idx = new_idx;
        } else if new_idx <= self.active_workspace_idx && old_idx > self.active_workspace_idx {
            self.active_workspace_idx += 1;
        } else if new_idx >= self.active_workspace_idx && old_idx < self.active_workspace_idx {
            self.active_workspace_idx = self.active_workspace_idx.saturating_sub(1);
        }

        self.workspace_switch = None;

        self.clean_up_workspaces();
    }

    pub fn active_window_visual_rectangle(&self) -> Option<Rectangle<f64, Logical>> {
        if self.overview_open {
            return None;
        }

        self.active_workspace_ref().active_window_visual_rectangle()
    }

    fn workspace_size(&self, zoom: f64) -> Size<f64, Logical> {
        let ws_size = self.view_size.upscale(zoom);
        let scale = self.scale.fractional_scale();
        ws_size.to_physical_precise_ceil(scale).to_logical(scale)
    }

    pub fn fit_camera_to(
        &mut self,
        rect: Rectangle<f64, Logical>,
        padding: f64,
        config: zen_config::Animation,
    ) {
        if rect.size.w <= 0. || rect.size.h <= 0. {
            return;
        }

        let avail_w = (self.view_size.w - padding * 2.).max(1.);
        let avail_h = (self.view_size.h - padding * 2.).max(1.);
        let zoom = (avail_w / rect.size.w).min(avail_h / rect.size.h);
        let zoom = zoom.clamp(self.options.camera.min_zoom, self.options.camera.max_zoom);

        let ws_size = self.workspace_size(zoom);
        let static_offset = (self.view_size.to_point() - ws_size.to_point()).downscale(2.);

        let rect_center = rect.loc + Point::from((rect.size.w / 2., rect.size.h / 2.));
        let view_center = Point::from((self.view_size.w / 2., self.view_size.h / 2.));
        let pan = view_center - rect_center.upscale(zoom) - static_offset;

        self.camera.animate_zoom_to(zoom, config);
        self.camera.animate_pan_to(pan, config);
    }

    pub fn set_camera_focus(&mut self, config: zen_config::Animation) -> bool {
        let Some(rect) = self.active_window_visual_rectangle() else {
            return false;
        };
        let Some(id) = self.active_workspace_ref().active_window().map(|w| w.id().clone()) else {
            return false;
        };

        self.camera_focus = Some((id, rect));
        self.fit_camera_to(rect, CAMERA_FOCUS_PADDING, config);
        true
    }

    pub fn clear_camera_focus(&mut self) {
        self.camera_focus = None;
    }

    fn sync_camera_focus(&mut self, config: zen_config::Animation) {
        let Some((id, last_rect)) = self.camera_focus.clone() else {
            return;
        };

        let still_active = self
            .active_workspace_ref()
            .active_window()
            .is_some_and(|w| *w.id() == id);
        if !still_active {
            return;
        }

        let Some(rect) = self.active_window_visual_rectangle() else {
            self.camera_focus = None;
            return;
        };

        if rect != last_rect {
            self.camera_focus = Some((id, rect));
            self.fit_camera_to(rect, CAMERA_FOCUS_PADDING, config);
        }
    }

    pub fn viewport_center_on_canvas(&self) -> Point<f64, Canvas> {
        let zoom = self.overview_zoom();
        let ws_size = self.workspace_size(zoom);
        let static_offset = (self.view_size.to_point() - ws_size.to_point()).downscale(2.)
            + self.camera.pan_offset_view();
        let view_center = Point::from((self.view_size.w / 2., self.view_size.h / 2.));

        let p = (view_center - static_offset).downscale(zoom);
        Point::from((p.x, p.y))
    }

    fn sync_spawn_center(&mut self) {
        let zoom = self.overview_zoom();
        let center = self.viewport_center_on_canvas();
        let view_center = Point::from((self.view_size.w / 2., self.view_size.h / 2.));

        for ws in &mut self.workspaces {
            let wa = ws.working_area();
            let wa_center = wa.loc + Point::from((wa.size.w / 2., wa.size.h / 2.));
            let delta = (wa_center - view_center).downscale(zoom);

            ws.set_spawn_center(Some(Point::from((center.x + delta.x, center.y + delta.y))));
        }
    }

    pub fn spawn_island_target(&self, parent: Option<IslandId>) -> Option<IslandId> {
        let ws = self.active_workspace_ref();
        let ctx = SpawnContext {
            zoom: self.overview_zoom(),
            viewport_center: self.viewport_center_on_canvas(),
            last_focused: ws.active_island(),
            parent_island: parent,
            camera_is_framing: self.camera_focus.is_some(),
            ..Default::default()
        };

        match ws.spawn_target(&ctx) {
            SpawnTarget::Join(id) => Some(id),
            SpawnTarget::New => None,
        }
    }

    fn sync_camera_scale(&mut self) {
        if self.camera.is_animating() {
            return;
        }

        let factor = quantize_camera_scale(self.overview_zoom());
        for ws in &mut self.workspaces {
            ws.set_camera_scale(factor);
        }
    }

    fn workspace_gap(&self, zoom: f64) -> f64 {
        let scale = self.scale.fractional_scale();
        let gap = self.view_size.h * 0.1 * zoom;
        round_logical_in_physical_max1(scale, gap)
    }

    fn workspace_size_with_gap(&self, zoom: f64) -> Size<f64, Logical> {
        let gap = self.workspace_gap(zoom);
        self.workspace_size(zoom) + Size::from((0., gap))
    }

    pub fn overview_zoom(&self) -> f64 {
        let progress = self.overview_progress.as_ref().map(|p| p.value());
        compute_overview_zoom(&self.options, progress) * self.camera.zoom()
    }

    pub fn is_view_transformed(&self) -> bool {
        (self.overview_zoom() - 1.).abs() > 1e-9
    }

    pub(super) fn set_overview_progress(&mut self, progress: Option<&super::OverviewProgress>) {
        let prev_render_idx = self.workspace_render_idx();
        self.overview_progress = progress.map(OverviewProgress::from);
        let new_render_idx = self.workspace_render_idx();

        if prev_render_idx != new_render_idx {
            if let Some(WorkspaceSwitch::Animation(anim)) = &mut self.workspace_switch {
                *anim = anim.restarted(prev_render_idx, anim.to(), 0.);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn overview_progress_value(&self) -> Option<f64> {
        self.overview_progress.as_ref().map(|p| p.value())
    }

    pub fn workspace_render_idx(&self) -> f64 {
        if let (
            Some(WorkspaceSwitch::Animation(switch_anim)),
            Some(OverviewProgress::Animation(progress_anim)),
        ) = (&self.workspace_switch, &self.overview_progress)
        {
            if switch_anim.start_time() == progress_anim.start_time()
                && (switch_anim.duration().as_secs_f64() - progress_anim.duration().as_secs_f64())
                    .abs()
                    <= 0.001
            {
                #[rustfmt::skip]
                let from = progress_anim.from();
                let from_zoom = compute_overview_zoom(&self.options, Some(from));
                let from_ws_height_with_gap = self.workspace_size_with_gap(from_zoom).h;

                let zoom = self.overview_zoom();
                let ws_height_with_gap = self.workspace_size_with_gap(zoom).h;

                let first_ws_y = -switch_anim.value() * from_ws_height_with_gap
                    + switch_anim.to() * (from_ws_height_with_gap - ws_height_with_gap);

                return -first_ws_y / ws_height_with_gap;
            }
        };

        if let Some(switch) = &self.workspace_switch {
            switch.current_idx()
        } else {
            self.active_workspace_idx as f64
        }
    }

    pub fn workspaces_render_geo(&self) -> impl Iterator<Item = Rectangle<f64, Logical>> {
        let scale = self.scale.fractional_scale();
        let zoom = self.overview_zoom();

        let ws_size = self.workspace_size(zoom);
        let gap = self.workspace_gap(zoom);
        let ws_height_with_gap = ws_size.h + gap;

        let static_offset = (self.view_size.to_point() - ws_size.to_point()).downscale(2.)
            + self.camera.pan_offset_view();
        let static_offset = static_offset
            .to_physical_precise_round(scale)
            .to_logical(scale);

        let first_ws_y = -self.workspace_render_idx() * ws_height_with_gap;
        let first_ws_y = round_logical_in_physical(scale, first_ws_y);

        let content_extent: Vec<Option<Rectangle<f64, Logical>>> = self
            .workspaces
            .iter()
            .map(|ws| ws.floating_tiles_bbox())
            .collect();

        (0..=self.workspaces.len()).map(move |idx| {
            let y = first_ws_y + idx as f64 * ws_height_with_gap;
            let loc = Point::from((0., y)) + static_offset;

            let loc = loc.to_physical_precise_round(scale).to_logical(scale);

            let mut size = ws_size;
            if let Some(Some(bbox)) = content_extent.get(idx) {
                let far_x = (bbox.loc.x + bbox.size.w) * zoom;
                let far_y = (bbox.loc.y + bbox.size.h) * zoom;
                size = Size::from((size.w.max(far_x), size.h.max(far_y)));
            }

            Rectangle::new(loc, size)
        })
    }

    fn cull_coverage(
        geo: Rectangle<f64, Logical>,
        content: Option<Rectangle<f64, Logical>>,
        zoom: f64,
    ) -> Rectangle<f64, Logical> {
        let Some(content) = content else {
            return geo;
        };

        let near_x = (content.loc.x * zoom).min(0.);
        let near_y = (content.loc.y * zoom).min(0.);
        let far_x = ((content.loc.x + content.size.w) * zoom).max(geo.size.w);
        let far_y = ((content.loc.y + content.size.h) * zoom).max(geo.size.h);

        Rectangle::new(
            Point::from((geo.loc.x + near_x, geo.loc.y + near_y)),
            Size::from((far_x - near_x, far_y - near_y)),
        )
    }

    fn content_extents(&self) -> Vec<Option<Rectangle<f64, Logical>>> {
        self.workspaces
            .iter()
            .map(|ws| ws.floating_tiles_bbox())
            .collect()
    }

    pub fn workspaces_with_render_geo_cull(
        &self,
        cull: bool,
    ) -> impl Iterator<Item = (&Workspace<W>, Rectangle<f64, Logical>)> {
        let output_geo = Rectangle::from_size(self.view_size);
        let zoom = self.overview_zoom();
        let extents = self.content_extents();

        let geo = self.workspaces_render_geo();
        zip(self.workspaces.iter().enumerate(), geo)
            .filter(move |((idx, _ws), geo)| {
                !cull
                    || Self::cull_coverage(*geo, extents[*idx], zoom)
                        .intersection(output_geo)
                        .is_some()
            })
            .map(|((_idx, ws), geo)| (ws, geo))
    }

    pub fn workspaces_with_render_geo(
        &self,
    ) -> impl Iterator<Item = (&Workspace<W>, Rectangle<f64, Logical>)> {
        self.workspaces_with_render_geo_cull(true)
    }

    pub fn workspaces_with_render_geo_idx(
        &self,
    ) -> impl Iterator<Item = ((usize, &Workspace<W>), Rectangle<f64, Logical>)> {
        let output_geo = Rectangle::from_size(self.view_size);
        let zoom = self.overview_zoom();
        let extents = self.content_extents();

        let geo = self.workspaces_render_geo();
        zip(self.workspaces.iter().enumerate(), geo)
            .filter(move |((idx, _ws), geo)| {
                Self::cull_coverage(*geo, extents[*idx], zoom)
                    .intersection(output_geo)
                    .is_some()
            })
    }

    pub fn workspaces_with_render_geo_mut(
        &mut self,
        cull: bool,
    ) -> impl Iterator<Item = (&mut Workspace<W>, Rectangle<f64, Logical>)> {
        let output_geo = Rectangle::from_size(self.view_size);
        let zoom = self.overview_zoom();
        let extents = self.content_extents();

        let geo = self.workspaces_render_geo();
        zip(self.workspaces.iter_mut().enumerate(), geo)
            .filter(move |((idx, _ws), geo)| {
                !cull
                    || Self::cull_coverage(*geo, extents[*idx], zoom)
                        .intersection(output_geo)
                        .is_some()
            })
            .map(|((_idx, ws), geo)| (ws, geo))
    }

    pub fn workspace_under(
        &self,
        pos_within_output: Point<f64, Logical>,
    ) -> Option<(&Workspace<W>, Rectangle<f64, Logical>)> {
        let found = self.workspaces_with_render_geo().find_map(|(ws, geo)| {
            let loc = Point::from((0., geo.loc.y));
            let size = Size::from((self.view_size.w, geo.size.h));
            let bounds = Rectangle::new(loc, size);

            bounds.contains(pos_within_output).then_some((ws, geo))
        });
        if let Some((ws, geo)) = found {
            return Some((ws, geo));
        }

        let dist_y = |geo: &Rectangle<f64, Logical>| {
            let top = geo.loc.y;
            let bottom = geo.loc.y + geo.size.h;
            if pos_within_output.y < top {
                top - pos_within_output.y
            } else if pos_within_output.y > bottom {
                pos_within_output.y - bottom
            } else {
                0.
            }
        };
        self.workspaces_with_render_geo_cull(false)
            .min_by(|(_, a), (_, b)| dist_y(a).total_cmp(&dist_y(b)))
    }

    pub fn workspace_under_narrow(
        &self,
        pos_within_output: Point<f64, Logical>,
    ) -> Option<&Workspace<W>> {
        self.workspaces_with_render_geo()
            .find_map(|(ws, geo)| geo.contains(pos_within_output).then_some(ws))
    }

    pub fn window_under(&self, pos_within_output: Point<f64, Logical>) -> Option<(&W, HitType)> {
        let (ws, geo) = self.workspace_under(pos_within_output)?;

        if self.is_view_transformed() {
            let zoom = self.overview_zoom();
            let pos_within_workspace = (pos_within_output - geo.loc).downscale(zoom);
            let (win, hit) = ws.window_under(pos_within_workspace)?;
            Some((win, hit.scaled_by(zoom).offset_win_pos(geo.loc)))
        } else {
            let (win, hit) = ws.window_under(pos_within_output - geo.loc)?;
            Some((win, hit.offset_win_pos(geo.loc)))
        }
    }

    pub fn resize_edges_under(&self, pos_within_output: Point<f64, Logical>) -> Option<ResizeEdge> {
        let (ws, geo) = self.workspace_under(pos_within_output)?;
        let pos_within_workspace = pos_within_output - geo.loc;
        if self.is_view_transformed() {
            ws.resize_edges_under(pos_within_workspace.downscale(self.overview_zoom()))
        } else {
            ws.resize_edges_under(pos_within_workspace)
        }
    }

    pub(super) fn insert_position(
        &self,
        pos_within_output: Point<f64, Logical>,
    ) -> (InsertWorkspace, Rectangle<f64, Logical>) {
        let mut iter = self.workspaces_with_render_geo_idx();

        let dummy = Rectangle::default();

        let ((idx, ws), geo) = iter.next().unwrap();

        if pos_within_output.y < geo.loc.y {
            return (InsertWorkspace::NewAt(idx), dummy);
        }

        let contains = move |geo: Rectangle<f64, Logical>| {
            geo.loc.y <= pos_within_output.y && pos_within_output.y < geo.loc.y + geo.size.h
        };

        if contains(geo) {
            return (InsertWorkspace::Existing(ws.id()), geo);
        }

        let mut last_geo = geo;
        let mut last_idx = idx;
        for ((idx, ws), geo) in iter {
            let gap_loc = Point::from((last_geo.loc.x, last_geo.loc.y + last_geo.size.h));
            let gap_size = Size::from((geo.size.w, geo.loc.y - gap_loc.y));
            let gap_geo = Rectangle::new(gap_loc, gap_size);
            if contains(gap_geo) {
                return (InsertWorkspace::NewAt(idx), dummy);
            }

            if contains(geo) {
                return (InsertWorkspace::Existing(ws.id()), geo);
            }

            last_geo = geo;
            last_idx = idx;
        }

        (InsertWorkspace::NewAt(last_idx + 1), dummy)
    }

    pub fn in_overview(&self) -> bool {
        self.overview_open || self.overview_progress.is_some()
    }

    pub fn render_above_top_layer(&self) -> bool {
        if self.workspace_switch.is_some() || self.overview_progress.is_some() {
            return false;
        }

        let ws = &self.workspaces[self.active_workspace_idx];
        ws.render_above_top_layer()
    }

    pub fn render_insert_hint_between_workspaces<R: ZenRenderer>(
        &self,
        renderer: &mut R,
        push: &mut dyn FnMut(MonitorRenderElement<R>),
    ) {
        if self.options.layout.insert_hint.off {
            return;
        }
        let Some(render_loc) = self.insert_hint_render_loc else {
            return;
        };
        let InsertWorkspace::NewAt(_) = render_loc.workspace else {
            return;
        };

        self.insert_hint_element
            .render(renderer, render_loc.location, &mut |elem| {
                let elem = MonitorInnerRenderElement::UncroppedInsertHint(elem);
                let elem = RescaleRenderElement::from_element(elem, Point::default(), 1.);
                let elem =
                    RelocateRenderElement::from_element(elem, Point::default(), Relocate::Relative);
                push(elem);
            });
    }

    pub fn render_workspaces<R: ZenRenderer>(
        &self,
        mut ctx: RenderCtx<R>,
        focus_ring: bool,
        push: &mut dyn FnMut(MonitorRenderElement<R>),
    ) {
        let _span = tracy_client::span!("Monitor::render_workspaces");

        let scale = self.scale.fractional_scale();
        let height = (self.view_size.h * scale).ceil() as i32;

        let zoom = self.overview_zoom();

        let insert_hint_render_loc = self
            .insert_hint_render_loc
            .filter(|_| !self.options.layout.insert_hint.off);

        let scale_relocate = move |geo: Rectangle<f64, Logical>, elem| {
            let elem = RescaleRenderElement::from_element(elem, Point::from((0, 0)), zoom);
            RelocateRenderElement::from_element(
                elem,
                geo.loc.to_physical_precise_round(scale),
                Relocate::Relative,
            )
        };

        for pass in 0..4 {
            let cull = matches!(pass, 1 | 3);

            let crop_bounds =
                if cull && (self.workspace_switch.is_some() || self.overview_progress.is_some()) {
                    Rectangle::new(
                        Point::from((-i32::MAX / 2, 0)),
                        Size::from((i32::MAX, height)),
                    )
                } else {
                    Rectangle::new(
                        Point::from((-i32::MAX / 2, -i32::MAX / 2)),
                        Size::from((i32::MAX, i32::MAX)),
                    )
                };

            for (ws, geo) in self.workspaces_with_render_geo_cull(cull) {
                macro_rules! push {
                    () => {{
                        &mut |elem| {
                            let elem = CropRenderElement::from_element(elem, scale, crop_bounds);
                            if let Some(elem) = elem {
                                let elem = MonitorInnerRenderElement::from(elem);
                                push(scale_relocate(geo, elem));
                            }
                        }
                    }};
                }

                let xray_pos = XrayPos::new(geo.loc, zoom);

                match pass {
                    0 => {
                        ws.render_floating(
                            ctx.r(),
                            xray_pos,
                            focus_ring,
                            RenderLayer::MovingBetweenWorkspaces,
                            push!(),
                        );
                    }
                    1 => {
                        ws.render_floating(
                            ctx.r(),
                            xray_pos,
                            focus_ring,
                            RenderLayer::Normal,
                            push!(),
                        );

                        if let Some(loc) = insert_hint_render_loc {
                            if loc.workspace == InsertWorkspace::Existing(ws.id()) {
                                self.insert_hint_element.render(
                                    ctx.renderer,
                                    loc.location,
                                    push!(),
                                );
                            }
                        }
                    }
                    2 => {
                        ws.render_scrolling(
                            ctx.r(),
                            xray_pos,
                            focus_ring,
                            RenderLayer::MovingBetweenWorkspaces,
                            push!(),
                        );
                    }
                    _ => {
                        ws.render_scrolling(
                            ctx.r(),
                            xray_pos,
                            focus_ring,
                            RenderLayer::Normal,
                            push!(),
                        );
                    }
                }
            }
        }

        if self.canvas_clock.is_enabled() {
            if let Some((_, geo)) = self.workspaces_with_render_geo().next() {
                if let Some(elem) = self.canvas_clock.render(ctx.renderer, scale * zoom) {
                    push(scale_relocate(geo, MonitorInnerRenderElement::Texture(elem)));
                }
            }
        }
    }

    pub fn render_workspace_shadows<R: ZenRenderer>(
        &self,
        renderer: &mut R,
        push: &mut dyn FnMut(MonitorRenderElement<R>),
    ) {
        let Some(progress) = self.overview_progress.as_ref().map(|p| p.clamped_value()) else {
            return;
        };
        let alpha = progress.clamp(0., 1.) as f32;

        let _span = tracy_client::span!("Monitor::render_workspace_shadows");

        let scale = self.scale.fractional_scale();
        let zoom = self.overview_zoom();

        for (ws, geo) in self.workspaces_with_render_geo() {
            ws.render_shadow(renderer, &mut |elem| {
                let elem = elem.with_alpha(alpha);
                let elem = MonitorInnerRenderElement::Shadow(elem);
                let elem = RescaleRenderElement::from_element(elem, Point::from((0, 0)), zoom);
                let elem = RelocateRenderElement::from_element(
                    elem,
                    geo.loc.to_physical_precise_round(scale),
                    Relocate::Relative,
                );
                push(elem);
            });
        }
    }

    pub fn workspace_switch_gesture_begin(&mut self, is_touchpad: bool) {
        let center_idx = self.active_workspace_idx;
        let current_idx = self.workspace_render_idx();

        let gesture = WorkspaceSwitchGesture {
            center_idx,
            start_idx: current_idx,
            current_idx,
            animation: None,
            tracker: SwipeTracker::new(),
            is_touchpad,
            is_clamped: !self.overview_open,
            dnd_last_event_time: None,
            dnd_nonzero_start_time: None,
        };
        self.workspace_switch = Some(WorkspaceSwitch::Gesture(gesture));
    }

    pub fn dnd_scroll_gesture_begin(&mut self) {
        if let Some(WorkspaceSwitch::Gesture(WorkspaceSwitchGesture {
            dnd_last_event_time: Some(_),
            ..
        })) = &self.workspace_switch
        {
            return;
        }

        if !self.overview_open {
            return;
        }

        let center_idx = self.active_workspace_idx;
        let current_idx = self.workspace_render_idx();

        let gesture = WorkspaceSwitchGesture {
            center_idx,
            start_idx: current_idx,
            current_idx,
            animation: None,
            tracker: SwipeTracker::new(),
            is_touchpad: false,
            is_clamped: false,
            dnd_last_event_time: Some(self.clock.now_unadjusted()),
            dnd_nonzero_start_time: None,
        };
        self.workspace_switch = Some(WorkspaceSwitch::Gesture(gesture));
    }

    pub fn workspace_switch_gesture_update(
        &mut self,
        delta_y: f64,
        timestamp: Duration,
        is_touchpad: bool,
    ) -> Option<bool> {
        let Some(WorkspaceSwitch::Gesture(gesture)) = &self.workspace_switch else {
            return None;
        };

        if gesture.is_touchpad != is_touchpad || gesture.dnd_last_event_time.is_some() {
            return None;
        }

        let zoom = self.overview_zoom();
        let total_height = if gesture.is_touchpad {
            WORKSPACE_GESTURE_MOVEMENT
        } else {
            self.workspace_size_with_gap(1.).h
        };

        let Some(WorkspaceSwitch::Gesture(gesture)) = &mut self.workspace_switch else {
            return None;
        };

        let delta_scale = if gesture.is_touchpad {
            (zoom - 1.) / 2.5 + 1.
        } else {
            zoom
        };

        let delta_y = delta_y / delta_scale;
        let mut rubber_band = WORKSPACE_GESTURE_RUBBER_BAND;
        rubber_band.limit /= zoom;

        gesture.tracker.push(delta_y, timestamp);

        let pos = gesture.tracker.pos() / total_height;

        let (min, max) = gesture.min_max(self.workspaces.len());
        let new_idx = gesture.start_idx + pos;
        let new_idx = rubber_band.clamp(min, max, new_idx);

        if gesture.current_idx == new_idx {
            return Some(false);
        }

        gesture.current_idx = new_idx;
        Some(true)
    }

    pub fn dnd_scroll_gesture_scroll(&mut self, pos: Point<f64, Logical>, speed: f64) -> bool {
        let zoom = self.overview_zoom();

        let Some(WorkspaceSwitch::Gesture(gesture)) = &mut self.workspace_switch else {
            return false;
        };

        let Some(last_time) = gesture.dnd_last_event_time else {
            return false;
        };

        let config = &self.options.gestures.dnd_edge_workspace_switch;
        let trigger_height = config.trigger_height;

        let width = self.view_size.w * zoom;
        let x = pos.x - (self.view_size.w - width) / 2.;

        let y = pos.y - self.working_area.loc.y;
        let height = self.working_area.size.h;

        let y = y.clamp(0., height);
        let trigger_height = trigger_height.clamp(0., height / 2.);

        let delta = if x < 0. || width <= x {
            0.
        } else if y < trigger_height {
            -(trigger_height - y)
        } else if height - y < trigger_height {
            trigger_height - (height - y)
        } else {
            0.
        };

        let delta = if trigger_height < 0.01 {
            0.
        } else {
            delta / trigger_height
        };
        let delta = delta * speed;

        let now = self.clock.now_unadjusted();
        gesture.dnd_last_event_time = Some(now);

        if delta == 0. {
            gesture.dnd_nonzero_start_time = None;
            return false;
        }

        let nonzero_start = *gesture.dnd_nonzero_start_time.get_or_insert(now);

        let delay = Duration::from_millis(u64::from(config.delay_ms));
        if now.saturating_sub(nonzero_start) < delay {
            return true;
        }

        let time_delta = now.saturating_sub(last_time).as_secs_f64();

        let delta = delta * time_delta * config.max_speed;

        gesture.tracker.push(delta, now);

        let total_height = WORKSPACE_DND_EDGE_SCROLL_MOVEMENT;
        let pos = gesture.tracker.pos() / total_height;
        let unclamped = gesture.start_idx + pos;

        let (min, max) = gesture.min_max(self.workspaces.len());
        let clamped = unclamped.clamp(min, max);

        gesture.start_idx += clamped - unclamped;
        gesture.current_idx = clamped;

        true
    }

    pub fn workspace_switch_gesture_end(&mut self, is_touchpad: Option<bool>) -> bool {
        let Some(WorkspaceSwitch::Gesture(gesture)) = &self.workspace_switch else {
            return false;
        };

        if is_touchpad.is_some_and(|x| gesture.is_touchpad != x) {
            return false;
        }

        let zoom = self.overview_zoom();
        let total_height = if gesture.dnd_last_event_time.is_some() {
            WORKSPACE_DND_EDGE_SCROLL_MOVEMENT
        } else if gesture.is_touchpad {
            WORKSPACE_GESTURE_MOVEMENT
        } else {
            self.workspace_size_with_gap(1.).h
        };

        let Some(WorkspaceSwitch::Gesture(gesture)) = &mut self.workspace_switch else {
            return false;
        };

        let now = self.clock.now_unadjusted();
        gesture.tracker.push(0., now);

        let mut rubber_band = WORKSPACE_GESTURE_RUBBER_BAND;
        rubber_band.limit /= zoom;

        let mut velocity = gesture.tracker.velocity() / total_height;
        let current_pos = gesture.tracker.pos() / total_height;
        let pos = gesture.tracker.projected_end_pos() / total_height;

        let (min, max) = gesture.min_max(self.workspaces.len());
        let new_idx = gesture.start_idx + pos;

        let new_idx = new_idx.clamp(min, max);
        let new_idx = new_idx.round() as usize;

        velocity *= rubber_band.clamp_derivative(min, max, gesture.start_idx + current_pos);

        if self.active_workspace_idx != new_idx {
            self.previous_workspace_id = Some(self.workspaces[self.active_workspace_idx].id());
        }

        self.active_workspace_idx = new_idx;
        self.workspace_switch = Some(WorkspaceSwitch::Animation(Animation::new(
            self.clock.clone(),
            gesture.current_idx,
            new_idx as f64,
            velocity,
            self.options.animations.workspace_switch.0,
        )));

        true
    }

    pub fn dnd_scroll_gesture_end(&mut self) {
        if !matches!(
            self.workspace_switch,
            Some(WorkspaceSwitch::Gesture(WorkspaceSwitchGesture {
                dnd_last_event_time: Some(_),
                ..
            }))
        ) {
            return;
        };

        self.workspace_switch_gesture_end(None);
    }

    pub fn scale(&self) -> smithay::output::Scale {
        self.scale
    }

    pub fn view_size(&self) -> Size<f64, Logical> {
        self.view_size
    }

    pub fn working_area(&self) -> Rectangle<f64, Logical> {
        self.working_area
    }

    pub fn layout_config(&self) -> Option<&zen_config::LayoutPart> {
        self.layout_config.as_ref()
    }

    #[cfg(test)]
    pub(super) fn verify_invariants(&self) {
        use approx::assert_abs_diff_eq;

        let options =
            Options::clone(&self.base_options).with_merged_layout(self.layout_config.as_ref());
        assert_eq!(&*self.options, &options);

        assert!(
            !self.workspaces.is_empty(),
            "monitor must have at least one workspace"
        );
        assert!(self.active_workspace_idx < self.workspaces.len());

        if let Some(WorkspaceSwitch::Animation(anim)) = &self.workspace_switch {
            let before_idx = anim.from() as usize;
            let after_idx = anim.to() as usize;

            assert!(before_idx < self.workspaces.len());
            assert!(after_idx < self.workspaces.len());
        }

        assert!(
            !self.workspaces.last().unwrap().has_windows(),
            "monitor must have an empty workspace in the end"
        );
        if self.options.layout.empty_workspace_above_first {
            assert!(
                !self.workspaces.first().unwrap().has_windows(),
                "first workspace must be empty when empty_workspace_above_first is set"
            )
        }

        assert!(
            self.workspaces.last().unwrap().name.is_none(),
            "monitor must have an unnamed workspace in the end"
        );
        if self.options.layout.empty_workspace_above_first {
            assert!(
                self.workspaces.first().unwrap().name.is_none(),
                "first workspace must be unnamed when empty_workspace_above_first is set"
            )
        }

        if self.options.layout.empty_workspace_above_first {
            assert!(
                self.workspaces.len() != 2,
                "if empty_workspace_above_first is set there must be just 1 or 3+ workspaces"
            )
        }

        let pre_skip = if self.options.layout.empty_workspace_above_first {
            1
        } else {
            0
        };
        if self.workspace_switch.is_none() {
            for (idx, ws) in self
                .workspaces
                .iter()
                .enumerate()
                .skip(pre_skip)
                .rev()
                .skip(1)
            {
                if idx != self.active_workspace_idx {
                    assert!(
                        ws.has_windows_or_name(),
                        "non-active workspace can't be empty and unnamed except the last one"
                    );
                }
            }
        }

        for workspace in &self.workspaces {
            assert_eq!(self.clock, workspace.clock);

            assert_eq!(
                self.scale().integer_scale(),
                workspace.scale().integer_scale()
            );
            assert_eq!(
                self.scale().fractional_scale(),
                workspace.scale().fractional_scale()
            );
            assert_eq!(self.view_size, workspace.view_size());
            assert_eq!(self.working_area, workspace.working_area());

            assert_eq!(
                workspace.base_options, self.options,
                "workspace options must be synchronized with monitor"
            );
        }

        let scale = self.scale().fractional_scale();
        let iter = self.workspaces_with_render_geo();
        for (_ws, ws_geo) in iter {
            let pos = ws_geo.loc;
            let rounded_pos = pos.to_physical_precise_round(scale).to_logical(scale);

            assert_abs_diff_eq!(pos.x, rounded_pos.x, epsilon = 1e-5);
            assert_abs_diff_eq!(pos.y, rounded_pos.y, epsilon = 1e-5);
        }
    }
}
