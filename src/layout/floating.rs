use std::cmp::max;
use std::iter::zip;
use std::rc::Rc;

use zen_config::utils::MergeWith as _;
use zen_config::{PresetSize, RelativeTo};
use zen_ipc::{PositionChange, SizeChange, WindowLayout};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Logical, Point, Rectangle, Scale, Serial, Size};

use super::closing_window::{ClosingWindow, ClosingWindowRenderElement};
use super::island::{
    self as island, Direction, Island, IslandId, IslandLayout, IslandSpace, SpawnContext,
    SpawnTarget,
};
use super::scrolling::ColumnWidth;
use super::tile::{Tile, TileRenderElement, TileRenderSnapshot};
use super::workspace::{InteractiveResize, ResolvedSize};
use super::{
    Canvas, ConfigureIntent, InteractiveResizeData, LayoutElement, Options, RemovedTile,
    CANVAS_LIMIT,
};
use crate::animation::{Animation, Clock};
use crate::layout::RenderLayer;
use crate::zen_render_elements;
use zen_config::{Color, CornerRadius, GradientInterpolation};

use smithay::backend::renderer::element::Kind;

use crate::render_helpers::border::BorderRenderElement;
use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::texture::TextureRenderElement;
use crate::ui::app_icons::AppIcons;

// map
// Bubbles have to be on when the map settles at exactly map-zoom, and reading a
// float back out of an animation does not land on the target to the bit. The slack
// also fades them in a frame or two early instead of popping at the stop.
const BUBBLE_ZOOM_SLACK: f64 = 1.08;

// The size of one app icon on the map, in canvas pixels. A cluster divides this down.
const ICON_SIZE: f64 = 132.;

const BUBBLE_PAD: f64 = 56.;


const BUBBLE_FILL: Color = Color::new_unpremul(1., 1., 1., 0.10);

// The fill alone is invisible over a light wallpaper, and a darker one would be
// invisible over a dark wallpaper. The ring is what actually draws the boundary; the
// fill only separates the inside from the canvas.
const BUBBLE_RING: Color = Color::new_unpremul(1., 1., 1., 0.38);
const BUBBLE_RING_WIDTH: f32 = 6.;

use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::xray::XrayPos;
use crate::render_helpers::RenderCtx;
use crate::utils::transaction::TransactionBlocker;
use crate::utils::{
    center_preferring_top_left_in_area, clamp_preferring_top_left_in_area, ensure_min_max_size,
    ensure_min_max_size_maybe_zero, ResizeEdge,
};
use crate::window::ResolvedWindowRules;

pub const DIRECTIONAL_MOVE_PX: f64 = 50.;

#[derive(Debug)]
pub struct FloatingSpace<W: LayoutElement> {
    tiles: Vec<Tile<W>>,

    data: Vec<Data>,

    active_window_id: Option<W::Id>,

    interactive_resize: Option<InteractiveResize<W>>,

    closing_windows: Vec<ClosingWindow>,

    spawn_center: Option<Point<f64, Canvas>>,

    islands: IslandSpace<W::Id>,
    bubbles: Vec<(BorderRenderElement, BorderRenderElement)>,
    icons: AppIcons,

    view_size: Size<f64, Logical>,

    working_area: Rectangle<f64, Logical>,

    scale: f64,

    clock: Clock,

    options: Rc<Options>,
}

zen_render_elements! {
    FloatingSpaceRenderElement<R> => {
        Tile = TileRenderElement<R>,
        ClosingWindow = ClosingWindowRenderElement,
        Bubble = BorderRenderElement,
        Icon = PrimaryGpuTextureRenderElement,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Data {
    pos: Point<f64, Canvas>,

    logical_pos: Point<f64, Logical>,

    size: Size<f64, Logical>,

    working_area: Rectangle<f64, Logical>,

    infinite_canvas: bool,

    center_on: Option<Point<f64, Canvas>>,
}

impl Data {
    pub fn new<W: LayoutElement>(
        working_area: Rectangle<f64, Logical>,
        infinite_canvas: bool,
        tile: &Tile<W>,
        logical_pos: Point<f64, Logical>,
    ) -> Self {
        let mut rv = Self {
            pos: Point::default(),
            logical_pos: Point::default(),
            size: Size::default(),
            working_area,
            infinite_canvas,
            center_on: None,
        };
        rv.update(tile);
        rv.set_logical_pos(logical_pos);
        rv
    }

    pub fn canvas_to_logical(pos: Point<f64, Canvas>) -> Point<f64, Logical> {
        Point::from((
            pos.x.clamp(-CANVAS_LIMIT, CANVAS_LIMIT),
            pos.y.clamp(-CANVAS_LIMIT, CANVAS_LIMIT),
        ))
    }

    pub fn logical_to_canvas(logical_pos: Point<f64, Logical>) -> Point<f64, Canvas> {
        Point::from((
            logical_pos.x.clamp(-CANVAS_LIMIT, CANVAS_LIMIT),
            logical_pos.y.clamp(-CANVAS_LIMIT, CANVAS_LIMIT),
        ))
    }

    fn recompute_logical_pos(&mut self) {
        let mut logical_pos = Self::canvas_to_logical(self.pos);

        if self.infinite_canvas {
            self.logical_pos = logical_pos;
            return;
        }

        let min_on_screen_hor = f64::clamp(self.size.w / 4., 10., 75.);
        let min_on_screen_ver = f64::clamp(self.size.h / 4., 10., 75.);
        let max_off_screen_hor = f64::max(0., self.size.w - min_on_screen_hor);
        let max_off_screen_ver = f64::max(0., self.size.h - min_on_screen_ver);

        logical_pos -= self.working_area.loc;
        logical_pos.x = f64::max(logical_pos.x, -max_off_screen_hor);
        logical_pos.y = f64::max(logical_pos.y, -max_off_screen_ver);
        logical_pos.x = f64::min(
            logical_pos.x,
            self.working_area.size.w - self.size.w + max_off_screen_hor,
        );
        logical_pos.y = f64::min(
            logical_pos.y,
            self.working_area.size.h - self.size.h + max_off_screen_ver,
        );
        logical_pos += self.working_area.loc;

        self.logical_pos = logical_pos;
    }

    pub fn update_config(&mut self, working_area: Rectangle<f64, Logical>, infinite_canvas: bool) {
        if self.working_area == working_area && self.infinite_canvas == infinite_canvas {
            return;
        }

        self.working_area = working_area;
        self.infinite_canvas = infinite_canvas;
        self.recompute_logical_pos();
    }

    pub fn update<W: LayoutElement>(&mut self, tile: &Tile<W>) {
        let size = tile.tile_size();
        if self.size == size {
            return;
        }

        self.size = size;

        if let Some(center) = self.center_on {
            self.pos = Self::logical_to_canvas(Point::from((
                center.x - size.w / 2.,
                center.y - size.h / 2.,
            )));

            if tile.window().expected_size().is_some() {
                self.center_on = None;
            }
        }

        self.recompute_logical_pos();
    }

    pub fn set_logical_pos(&mut self, logical_pos: Point<f64, Logical>) {
        self.center_on = None;
        self.pos = Self::logical_to_canvas(logical_pos);

        self.recompute_logical_pos();
    }

    #[cfg(test)]
    fn verify_invariants(&self) {
        let mut temp = *self;
        temp.recompute_logical_pos();
        assert_eq!(
            self.logical_pos, temp.logical_pos,
            "cached logical pos must be up to date"
        );
    }
}

impl<W: LayoutElement> FloatingSpace<W> {
    pub fn new(
        view_size: Size<f64, Logical>,
        working_area: Rectangle<f64, Logical>,
        scale: f64,
        clock: Clock,
        options: Rc<Options>,
    ) -> Self {
        Self {
            tiles: Vec::new(),
            data: Vec::new(),
            active_window_id: None,
            interactive_resize: None,
            closing_windows: Vec::new(),
            spawn_center: None,
            islands: IslandSpace::new(),
            bubbles: Vec::new(),
            icons: AppIcons::new(),
            view_size,
            working_area,
            scale,
            clock,
            options,
        }
    }

    pub fn update_config(
        &mut self,
        view_size: Size<f64, Logical>,
        working_area: Rectangle<f64, Logical>,
        scale: f64,
        options: Rc<Options>,
    ) {
        for (tile, data) in zip(&mut self.tiles, &mut self.data) {
            tile.update_config(view_size, scale, options.clone());
            data.update(tile);
            data.update_config(working_area, options.camera.infinite_canvas);
        }

        self.view_size = view_size;
        self.working_area = working_area;
        self.scale = scale;
        self.options = options;
    }

    pub fn update_shaders(&mut self) {
        for tile in &mut self.tiles {
            tile.update_shaders();
        }
    }

    pub fn advance_animations(&mut self) {
        self.sync_islands();

        for tile in &mut self.tiles {
            tile.advance_animations();
        }

        self.closing_windows.retain_mut(|closing| {
            closing.advance_animations();
            closing.are_animations_ongoing()
        });
    }

    pub fn are_animations_ongoing(&self) -> bool {
        self.tiles.iter().any(Tile::are_animations_ongoing) || !self.closing_windows.is_empty()
    }

    pub fn are_transitions_ongoing(&self) -> bool {
        self.tiles.iter().any(Tile::are_transitions_ongoing) || !self.closing_windows.is_empty()
    }

    // Pulled far enough back, a window is a few unreadable pixels. Drawing the island
    // it belongs to as a container keeps the grouping legible when the contents are
    // not, which is the whole reason the map is a different picture and not just a
    // smaller one.
    fn update_bubbles(&mut self) {
        // Sized to the icon cluster, not to the windows. On the map the windows are not
        // drawn at all, so a bubble shaped to their geometry would be a big empty box
        // with a few icons rattling around in the middle of it.
        let counts: Vec<usize> = self.islands.islands().map(|i| i.items().len()).collect();

        self.bubbles.clear();
        for count in counts {
            let (_, _, bubble_radius) = cluster_geometry(count);
            let side = bubble_radius * 2.;
            let size = Size::from((side, side));

            let bubble = |color, width| {
                BorderRenderElement::new(
                    size,
                    Rectangle::from_size(size),
                    GradientInterpolation::default(),
                    color,
                    color,
                    0.,
                    Rectangle::from_size(size),
                    width,
                    // Half the side is a circle, which is what a bubble is.
                    CornerRadius::from(bubble_radius as f32),
                    self.scale as f32,
                    1.,
                )
            };

            self.bubbles.push((
                bubble(BUBBLE_FILL, 0.),
                bubble(BUBBLE_RING, BUBBLE_RING_WIDTH),
            ));
        }
    }

    pub fn update_render_elements(
        &mut self,
        is_active: bool,
        view_rect: Rectangle<f64, Logical>,
        layer: RenderLayer,
    ) {
        self.update_bubbles();

        let active = self.active_window_id.clone();
        for (tile, offset) in self.tiles_with_offsets_mut() {
            if layer.is_normal() == tile.is_moving_between_workspaces() {
                continue;
            }

            let id = tile.window().id();
            let is_active = is_active && Some(id) == active.as_ref();

            let mut tile_view_rect = view_rect;
            tile_view_rect.loc -= offset + tile.render_offset();
            tile.update_render_elements(is_active, tile_view_rect);
        }
    }

    pub fn tiles(&self) -> impl Iterator<Item = &Tile<W>> + '_ {
        self.tiles.iter()
    }

    pub fn tiles_mut(&mut self) -> impl Iterator<Item = &mut Tile<W>> + '_ {
        self.tiles.iter_mut()
    }

    pub fn tiles_with_offsets(&self) -> impl Iterator<Item = (&Tile<W>, Point<f64, Logical>)> + '_ {
        let offsets = self.data.iter().map(|d| d.logical_pos);
        zip(&self.tiles, offsets)
    }

    pub fn tiles_with_offsets_mut(
        &mut self,
    ) -> impl Iterator<Item = (&mut Tile<W>, Point<f64, Logical>)> + '_ {
        let offsets = self.data.iter().map(|d| d.logical_pos);
        zip(&mut self.tiles, offsets)
    }

    pub fn tiles_with_render_positions(
        &self,
    ) -> impl Iterator<Item = (&Tile<W>, Point<f64, Logical>)> {
        let scale = self.scale;
        self.tiles_with_offsets().map(move |(tile, offset)| {
            let pos = offset + tile.render_offset();
            let pos = pos.to_physical_precise_round(scale).to_logical(scale);
            (tile, pos)
        })
    }

    pub fn tiles_with_render_positions_mut(
        &mut self,
        round: bool,
    ) -> impl Iterator<Item = (&mut Tile<W>, Point<f64, Logical>)> {
        let scale = self.scale;
        self.tiles_with_offsets_mut().map(move |(tile, offset)| {
            let mut pos = offset + tile.render_offset();
            if round {
                pos = pos.to_physical_precise_round(scale).to_logical(scale);
            }
            (tile, pos)
        })
    }

    pub fn tiles_with_ipc_layouts(&self) -> impl Iterator<Item = (&Tile<W>, WindowLayout)> {
        let scale = self.scale;
        self.tiles_with_offsets().map(move |(tile, offset)| {
            let pos = offset;
            let pos = pos.to_physical_precise_round(scale).to_logical(scale);

            let layout = WindowLayout {
                tile_pos_in_workspace_view: Some(pos.into()),
                ..tile.ipc_layout_template()
            };
            (tile, layout)
        })
    }

    pub fn new_window_toplevel_bounds(&self, rules: &ResolvedWindowRules) -> Size<i32, Logical> {
        let border_config = self.options.layout.border.merged_with(&rules.border);
        compute_toplevel_bounds(border_config, self.working_area.size)
    }

    pub fn active_window_visual_rectangle(&self) -> Option<Rectangle<f64, Logical>> {
        let active_id = self.active_window_id.as_ref()?;
        let (tile, offset) = self
            .tiles_with_offsets()
            .find(|(tile, _)| tile.window().id() == active_id)?;

        let window_pos = offset + tile.window_loc();
        let window_size = tile.window_size();
        let window_rect = Rectangle::new(window_pos, window_size);

        self.working_area.intersection(window_rect)
    }

    pub fn popup_target_rect(&self, id: &W::Id) -> Option<Rectangle<f64, Logical>> {
        for (tile, pos) in self.tiles_with_offsets() {
            if tile.window().id() == id {
                let mut target = self.working_area;
                target.loc -= pos;
                target.loc -= tile.window_loc();

                return Some(target);
            }
        }
        None
    }

    fn idx_of(&self, id: &W::Id) -> Option<usize> {
        self.tiles.iter().position(|tile| tile.window().id() == id)
    }

    fn contains(&self, id: &W::Id) -> bool {
        self.idx_of(id).is_some()
    }

    pub fn active_window(&self) -> Option<&W> {
        let id = self.active_window_id.as_ref()?;
        self.tiles
            .iter()
            .find(|tile| tile.window().id() == id)
            .map(Tile::window)
    }

    pub fn active_window_mut(&mut self) -> Option<&mut W> {
        let id = self.active_window_id.as_ref()?;
        self.tiles
            .iter_mut()
            .find(|tile| tile.window().id() == id)
            .map(Tile::window_mut)
    }

    pub fn has_window(&self, id: &W::Id) -> bool {
        self.tiles.iter().any(|tile| tile.window().id() == id)
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn add_tile(&mut self, tile: Tile<W>, activate: bool) {
        self.add_tile_at(0, tile, activate, None);
    }

    pub fn add_tile_to_island(&mut self, island: IslandId, tile: Tile<W>, activate: bool) {
        self.add_tile_at(0, tile, activate, Some(island));
    }

    fn add_tile_at(
        &mut self,
        mut idx: usize,
        mut tile: Tile<W>,
        activate: bool,
        join: Option<IslandId>,
    ) {
        tile.update_config(self.view_size, self.scale, self.options.clone());

        let floating_size = tile.floating_window_size;
        let win = tile.window_mut();
        let mut size = if !win.pending_sizing_mode().is_normal() {
            floating_size.unwrap_or_default()
        } else {
            floating_size.unwrap_or_else(|| win.expected_size().unwrap_or_default())
        };

        let min_size = win.min_size();
        let max_size = win.max_size();
        size.w = ensure_min_max_size_maybe_zero(size.w, min_size.w, max_size.w);
        size.h = ensure_min_max_size_maybe_zero(size.h, min_size.h, max_size.h);

        win.request_size_once(size, true);

        if activate || self.tiles.is_empty() {
            self.active_window_id = Some(win.id().clone());
        }

        for (i, tile_above) in self.tiles.iter().enumerate().take(idx) {
            if win.is_child_of(tile_above.window()) {
                idx = i;
                break;
            }
        }

        let stored = self.stored_or_default_tile_pos(&tile);
        let anchor = stored.is_none().then(|| self.spawn_anchor(tile.tile_size()));
        let pos = stored.unwrap_or_else(|| {
            let anchor = anchor.unwrap();
            Point::from((
                anchor.x - tile.tile_size().w / 2.,
                anchor.y - tile.tile_size().h / 2.,
            ))
        });

        let mut data = Data::new(
            self.working_area,
            self.options.camera.infinite_canvas,
            &tile,
            pos,
        );
        data.center_on = anchor.map(Data::logical_to_canvas);
        let win_id = tile.window().id().clone();

        self.data.insert(idx, data);
        self.tiles.insert(idx, tile);

        self.bring_up_descendants_of(idx);

        let idx = self.idx_of(&win_id).unwrap();
        self.register_island(idx, join);
    }

    pub fn add_tile_above(&mut self, above: &W::Id, mut tile: Tile<W>, activate: bool) {
        let idx = self.idx_of(above).unwrap();

        let above_pos = self.data[idx].logical_pos;
        let above_size = self.data[idx].size;
        let tile_size = tile.tile_size();
        let pos = above_pos + (above_size.to_point() - tile_size.to_point()).downscale(2.);
        let pos = self.clamp_within_working_area(pos, tile_size);
        tile.floating_pos = Some(self.logical_to_canvas(pos));

        self.add_tile_at(idx, tile, activate, None);
    }

    fn bring_up_descendants_of(&mut self, idx: usize) {
        let tile = &self.tiles[idx];
        let win = tile.window();

        let mut descendants: Vec<usize> = Vec::new();
        for (i, tile_below) in self.tiles.iter().enumerate().skip(idx + 1).rev() {
            let win_below = tile_below.window();
            if win_below.is_child_of(win)
                || descendants
                    .iter()
                    .any(|idx| win_below.is_child_of(self.tiles[*idx].window()))
            {
                descendants.push(i);
            }
        }

        let mut idx = idx;
        #[allow(clippy::explicit_counter_loop)]
        for descendant_idx in descendants.into_iter().rev() {
            self.raise_window(descendant_idx, idx);
            idx += 1;
        }
    }

    pub fn remove_tile(&mut self, id: &W::Id) -> RemovedTile<W> {
        let idx = self.idx_of(id).unwrap();
        self.remove_tile_by_idx(idx)
    }

    fn remove_tile_by_idx(&mut self, idx: usize) -> RemovedTile<W> {
        let mut tile = self.tiles.remove(idx);
        let data = self.data.remove(idx);

        self.unregister_island(tile.window().id());

        if self.tiles.is_empty() {
            self.active_window_id = None;
        } else if Some(tile.window().id()) == self.active_window_id.as_ref() {
            self.active_window_id = Some(self.tiles[0].window().id().clone());
        }

        if let Some(resize) = &self.interactive_resize {
            if tile.window().id() == &resize.window {
                self.interactive_resize = None;
            }
        }

        if let Some(size) = tile.window().expected_size() {
            tile.floating_window_size = Some(size);
        }
        tile.floating_pos = Some(data.pos);

        let width = ColumnWidth::Fixed(tile.tile_expected_or_current_size().w);
        RemovedTile {
            tile,
            width,
            is_full_width: false,
            is_floating: true,
        }
    }

    pub fn start_close_animation_for_window(
        &mut self,
        renderer: &mut GlesRenderer,
        id: &W::Id,
        blocker: TransactionBlocker,
    ) {
        let (tile, tile_pos) = self
            .tiles_with_render_positions_mut(false)
            .find(|(tile, _)| tile.window().id() == id)
            .unwrap();

        let Some(snapshot) = tile.take_unmap_snapshot() else {
            return;
        };

        let tile_size = tile.tile_size();

        self.start_close_animation_for_tile(renderer, snapshot, tile_size, tile_pos, blocker);
    }

    pub fn activate_window_without_raising(&mut self, id: &W::Id) -> bool {
        if !self.contains(id) {
            return false;
        }

        self.active_window_id = Some(id.clone());
        self.activate_island_of(id);
        true
    }

    pub fn activate_window(&mut self, id: &W::Id) -> bool {
        let Some(idx) = self.idx_of(id) else {
            return false;
        };

        self.raise_window(idx, 0);
        self.active_window_id = Some(id.clone());
        self.bring_up_descendants_of(0);
        self.activate_island_of(id);
        if let Some(island) = self.island_of(id) {
            self.islands.raise(island);
        }

        true
    }

    fn raise_window(&mut self, from_idx: usize, to_idx: usize) {
        assert!(to_idx <= from_idx);

        let tile = self.tiles.remove(from_idx);
        let data = self.data.remove(from_idx);
        self.tiles.insert(to_idx, tile);
        self.data.insert(to_idx, data);
    }

    pub fn start_close_animation_for_tile(
        &mut self,
        renderer: &mut GlesRenderer,
        snapshot: TileRenderSnapshot,
        tile_size: Size<f64, Logical>,
        tile_pos: Point<f64, Logical>,
        blocker: TransactionBlocker,
    ) {
        let anim = Animation::new(
            self.clock.clone(),
            0.,
            1.,
            0.,
            self.options.animations.window_close.anim,
        );

        let blocker = if self.options.disable_transactions {
            TransactionBlocker::completed()
        } else {
            blocker
        };

        let scale = Scale::from(self.scale);
        let res = ClosingWindow::new(
            renderer, snapshot, scale, tile_size, tile_pos, blocker, anim,
        );
        match res {
            Ok(closing) => {
                self.closing_windows.push(closing);
            }
            Err(err) => {
                warn!("error creating a closing window animation: {err:?}");
            }
        }
    }

    pub fn toggle_window_width(&mut self, id: Option<&W::Id>, forwards: bool) {
        let Some(id) = id.or(self.active_window_id.as_ref()).cloned() else {
            return;
        };
        let idx = self.idx_of(&id).unwrap();

        let available_size = self.working_area.size.w;

        let len = self.options.layout.preset_column_widths.len();
        let tile = &mut self.tiles[idx];
        let preset_idx = if let Some(idx) = tile.floating_preset_width_idx {
            (idx + if forwards { 1 } else { len - 1 }) % len
        } else {
            let current_window = tile.window_expected_or_current_size().w;
            let current_tile = tile.tile_expected_or_current_size().w;

            let mut it = self
                .options
                .layout
                .preset_column_widths
                .iter()
                .map(|preset| resolve_preset_size(*preset, available_size));

            if forwards {
                it.position(|resolved| {
                    match resolved {
                        ResolvedSize::Tile(resolved) => current_tile + 1. < resolved,
                        ResolvedSize::Window(resolved) => current_window + 1. < resolved,
                    }
                })
                .unwrap_or(0)
            } else {
                it.rposition(|resolved| {
                    match resolved {
                        ResolvedSize::Tile(resolved) => resolved + 1. < current_tile,
                        ResolvedSize::Window(resolved) => resolved + 1. < current_window,
                    }
                })
                .unwrap_or(len - 1)
            }
        };

        let preset = self.options.layout.preset_column_widths[preset_idx];
        self.set_window_width(Some(&id), SizeChange::from(preset), true);

        self.tiles[idx].floating_preset_width_idx = Some(preset_idx);

        self.interactive_resize_end(Some(&id));
    }

    pub fn start_open_animation(&mut self, id: &W::Id) -> bool {
        let Some(idx) = self.idx_of(id) else {
            return false;
        };

        self.tiles[idx].start_open_animation();
        true
    }

    pub fn toggle_window_height(&mut self, id: Option<&W::Id>, forwards: bool) {
        let Some(id) = id.or(self.active_window_id.as_ref()).cloned() else {
            return;
        };
        let idx = self.idx_of(&id).unwrap();

        let available_size = self.working_area.size.h;

        let len = self.options.layout.preset_window_heights.len();
        let tile = &mut self.tiles[idx];
        let preset_idx = if let Some(idx) = tile.floating_preset_height_idx {
            (idx + if forwards { 1 } else { len - 1 }) % len
        } else {
            let current_window = tile.window_expected_or_current_size().h;
            let current_tile = tile.tile_expected_or_current_size().h;

            let mut it = self
                .options
                .layout
                .preset_window_heights
                .iter()
                .map(|preset| resolve_preset_size(*preset, available_size));

            if forwards {
                it.position(|resolved| {
                    match resolved {
                        ResolvedSize::Tile(resolved) => current_tile + 1. < resolved,
                        ResolvedSize::Window(resolved) => current_window + 1. < resolved,
                    }
                })
                .unwrap_or(0)
            } else {
                it.rposition(|resolved| {
                    match resolved {
                        ResolvedSize::Tile(resolved) => resolved + 1. < current_tile,
                        ResolvedSize::Window(resolved) => resolved + 1. < current_window,
                    }
                })
                .unwrap_or(len - 1)
            }
        };

        let preset = self.options.layout.preset_window_heights[preset_idx];
        self.set_window_height(Some(&id), SizeChange::from(preset), true);

        let tile = &mut self.tiles[idx];
        tile.floating_preset_height_idx = Some(preset_idx);

        self.interactive_resize_end(Some(&id));
    }

    pub fn set_window_width(&mut self, id: Option<&W::Id>, change: SizeChange, animate: bool) {
        let Some(id) = id.or(self.active_window_id.as_ref()) else {
            return;
        };
        let idx = self.idx_of(id).unwrap();

        let tile = &mut self.tiles[idx];
        tile.floating_preset_width_idx = None;

        let available_size = self.working_area.size.w;
        let win = tile.window();
        let current_window = win.expected_size().unwrap_or_else(|| win.size()).w;
        let current_tile = tile.tile_expected_or_current_size().w;

        const MAX_PX: f64 = 100000.;
        const MAX_F: f64 = 10000.;

        let win_width = match change {
            SizeChange::SetFixed(win_width) => f64::from(win_width),
            SizeChange::SetProportion(prop) => {
                let prop = (prop / 100.).clamp(0., MAX_F);
                let tile_width = available_size * prop;
                tile.window_width_for_tile_width(tile_width)
            }
            SizeChange::AdjustFixed(delta) => f64::from(current_window.saturating_add(delta)),
            SizeChange::AdjustProportion(delta) => {
                let current_prop = current_tile / available_size;
                let prop = (current_prop + delta / 100.).clamp(0., MAX_F);
                let tile_width = available_size * prop;
                tile.window_width_for_tile_width(tile_width)
            }
        };
        let win_width = win_width.round().clamp(1., MAX_PX) as i32;

        let win = tile.window_mut();
        let min_size = win.min_size();
        let max_size = win.max_size();

        let win_width = ensure_min_max_size(win_width, min_size.w, max_size.w);

        let win_height = win.expected_size().unwrap_or_default().h;
        let win_height = ensure_min_max_size(win_height, min_size.h, max_size.h);

        let win_size = Size::from((win_width, win_height));
        win.request_size_once(win_size, animate);
    }

    pub fn set_window_height(&mut self, id: Option<&W::Id>, change: SizeChange, animate: bool) {
        let Some(id) = id.or(self.active_window_id.as_ref()) else {
            return;
        };
        let idx = self.idx_of(id).unwrap();

        let tile = &mut self.tiles[idx];
        tile.floating_preset_height_idx = None;

        let available_size = self.working_area.size.h;
        let win = tile.window();
        let current_window = win.expected_size().unwrap_or_else(|| win.size()).h;
        let current_tile = tile.tile_expected_or_current_size().h;

        const MAX_PX: f64 = 100000.;
        const MAX_F: f64 = 10000.;

        let win_height = match change {
            SizeChange::SetFixed(win_height) => f64::from(win_height),
            SizeChange::SetProportion(prop) => {
                let prop = (prop / 100.).clamp(0., MAX_F);
                let tile_height = available_size * prop;
                tile.window_height_for_tile_height(tile_height)
            }
            SizeChange::AdjustFixed(delta) => f64::from(current_window.saturating_add(delta)),
            SizeChange::AdjustProportion(delta) => {
                let current_prop = current_tile / available_size;
                let prop = (current_prop + delta / 100.).clamp(0., MAX_F);
                let tile_height = available_size * prop;
                tile.window_height_for_tile_height(tile_height)
            }
        };
        let win_height = win_height.round().clamp(1., MAX_PX) as i32;

        let win = tile.window_mut();
        let min_size = win.min_size();
        let max_size = win.max_size();

        let win_height = ensure_min_max_size(win_height, min_size.h, max_size.h);

        let win_width = win.expected_size().unwrap_or_default().w;
        let win_width = ensure_min_max_size(win_width, min_size.w, max_size.w);

        let win_size = Size::from((win_width, win_height));
        win.request_size_once(win_size, animate);
    }

    fn tile_rect(&self, idx: usize) -> Rectangle<f64, Logical> {
        Rectangle::new(self.data[idx].logical_pos, self.data[idx].size)
    }

    fn focus_directional(&mut self, dir: Direction) -> bool {
        let Some(active_id) = self.active_window_id.clone() else {
            return false;
        };
        let Some(active_idx) = self.idx_of(&active_id) else {
            return false;
        };
        let from = self.tile_rect(active_idx);

        let candidates: Vec<(usize, Rectangle<f64, Logical>)> = (0..self.tiles.len())
            .filter(|idx| *idx != active_idx)
            .map(|idx| (idx, self.tile_rect(idx)))
            .collect();

        let Some(idx) = island::nearest_in_direction(from, &candidates, dir) else {
            return false;
        };

        let id = self.tiles[idx].window().id().clone();
        self.activate_window(&id)
    }

    pub fn focus_left(&mut self) -> bool {
        self.focus_directional(Direction::Left)
    }

    pub fn focus_right(&mut self) -> bool {
        self.focus_directional(Direction::Right)
    }

    pub fn focus_up(&mut self) -> bool {
        self.focus_directional(Direction::Up)
    }

    pub fn focus_down(&mut self) -> bool {
        self.focus_directional(Direction::Down)
    }

    pub fn focus_leftmost(&mut self) {
        let result = self
            .tiles_with_offsets()
            .min_by(|(_, pos_a), (_, pos_b)| f64::total_cmp(&pos_a.x, &pos_b.x));
        if let Some((tile, _)) = result {
            let id = tile.window().id().clone();
            self.activate_window(&id);
        }
    }

    pub fn focus_rightmost(&mut self) {
        let result = self
            .tiles_with_offsets()
            .max_by(|(_, pos_a), (_, pos_b)| f64::total_cmp(&pos_a.x, &pos_b.x));
        if let Some((tile, _)) = result {
            let id = tile.window().id().clone();
            self.activate_window(&id);
        }
    }

    pub fn focus_topmost(&mut self) {
        let result = self
            .tiles_with_offsets()
            .min_by(|(_, pos_a), (_, pos_b)| f64::total_cmp(&pos_a.y, &pos_b.y));
        if let Some((tile, _)) = result {
            let id = tile.window().id().clone();
            self.activate_window(&id);
        }
    }

    pub fn focus_bottommost(&mut self) {
        let result = self
            .tiles_with_offsets()
            .max_by(|(_, pos_a), (_, pos_b)| f64::total_cmp(&pos_a.y, &pos_b.y));
        if let Some((tile, _)) = result {
            let id = tile.window().id().clone();
            self.activate_window(&id);
        }
    }

    fn move_to(&mut self, idx: usize, new_pos: Point<f64, Logical>, animate: bool) {
        if animate {
            self.move_and_animate(idx, new_pos);
        } else {
            self.data[idx].set_logical_pos(new_pos);
        }

        self.interactive_resize_end(None);
    }

    fn move_by(&mut self, amount: Point<f64, Logical>) {
        let Some(active_id) = self.active_window_id.clone() else {
            return;
        };

        if let Some(island) = self.island_of(&active_id) {
            if self.islands.get(island).is_some_and(|i| i.len() > 1) {
                self.move_island_by(island, amount);
                return;
            }
        }

        let idx = self.idx_of(&active_id).unwrap();
        let new_pos = self.data[idx].logical_pos + amount;
        self.move_to(idx, new_pos, true)
    }

    pub fn move_left(&mut self) {
        self.move_by(Point::from((-DIRECTIONAL_MOVE_PX, 0.)));
    }

    pub fn move_right(&mut self) {
        self.move_by(Point::from((DIRECTIONAL_MOVE_PX, 0.)));
    }

    pub fn move_up(&mut self) {
        self.move_by(Point::from((0., -DIRECTIONAL_MOVE_PX)));
    }

    pub fn move_down(&mut self) {
        self.move_by(Point::from((0., DIRECTIONAL_MOVE_PX)));
    }

    pub fn move_window(
        &mut self,
        id: Option<&W::Id>,
        x: PositionChange,
        y: PositionChange,
        animate: bool,
    ) {
        let Some(id) = id.or(self.active_window_id.as_ref()) else {
            return;
        };
        let idx = self.idx_of(id).unwrap();

        let mut pos = self.data[idx].logical_pos;

        let available_width = self.working_area.size.w;
        let available_height = self.working_area.size.h;
        let working_area_loc = self.working_area.loc;

        const MAX_F: f64 = 10000.;

        match x {
            PositionChange::SetFixed(x) => pos.x = x + working_area_loc.x,
            PositionChange::SetProportion(prop) => {
                let prop = (prop / 100.).clamp(0., MAX_F);
                pos.x = available_width * prop + working_area_loc.x;
            }
            PositionChange::AdjustFixed(x) => pos.x += x,
            PositionChange::AdjustProportion(prop) => {
                let current_prop = (pos.x - working_area_loc.x) / available_width.max(1.);
                let prop = (current_prop + prop / 100.).clamp(0., MAX_F);
                pos.x = available_width * prop + working_area_loc.x;
            }
        }
        match y {
            PositionChange::SetFixed(y) => pos.y = y + working_area_loc.y,
            PositionChange::SetProportion(prop) => {
                let prop = (prop / 100.).clamp(0., MAX_F);
                pos.y = available_height * prop + working_area_loc.y;
            }
            PositionChange::AdjustFixed(y) => pos.y += y,
            PositionChange::AdjustProportion(prop) => {
                let current_prop = (pos.y - working_area_loc.y) / available_height.max(1.);
                let prop = (current_prop + prop / 100.).clamp(0., MAX_F);
                pos.y = available_height * prop + working_area_loc.y;
            }
        }

        self.move_to(idx, pos, animate);
    }

    pub fn center_window(&mut self, id: Option<&W::Id>) {
        let Some(id) = id.or(self.active_window_id.as_ref()).cloned() else {
            return;
        };
        let idx = self.idx_of(&id).unwrap();

        let new_pos = center_preferring_top_left_in_area(self.working_area, self.data[idx].size);
        self.move_to(idx, new_pos, true);
    }

    pub fn descendants_added(&mut self, id: &W::Id) -> bool {
        let Some(idx) = self.idx_of(id) else {
            return false;
        };

        self.bring_up_descendants_of(idx);
        true
    }

    pub fn update_window(&mut self, id: &W::Id, serial: Option<Serial>) -> bool {
        let Some(tile_idx) = self.idx_of(id) else {
            return false;
        };

        let tile = &mut self.tiles[tile_idx];
        let data = &mut self.data[tile_idx];

        let resize = tile.window_mut().interactive_resize_data();

        if let Some(serial) = serial {
            tile.window_mut().on_commit(serial);
        }

        let prev_size = data.size;

        tile.update_window();
        data.update(tile);

        if let Some(resize) = resize {
            let mut offset = Point::from((0., 0.));
            if resize.edges.contains(ResizeEdge::LEFT) {
                offset.x += prev_size.w - data.size.w;
            }
            if resize.edges.contains(ResizeEdge::TOP) {
                offset.y += prev_size.h - data.size.h;
            }
            data.set_logical_pos(data.logical_pos + offset);
        }

        true
    }

    pub fn render<R: ZenRenderer>(
        &self,
        mut ctx: RenderCtx<R>,
        xray_pos: XrayPos,
        view_rect: Rectangle<f64, Logical>,
        focus_ring: bool,
        layer: RenderLayer,
        push: &mut dyn FnMut(FloatingSpaceRenderElement<R>),
    ) {
        let scale = Scale::from(self.scale);

        if layer.is_normal() {
            for closing in self.closing_windows.iter().rev() {
                let elem = closing.render(ctx.as_gles(), view_rect, scale);
                push(elem.into());
            }
        }

        // On the map the windows themselves are a few unreadable pixels, so they are
        // replaced outright by the icons of the apps they belong to, clustered inside
        // the island they are in. That is the whole difference between the map and a
        // small view of the canvas.
        let on_map = xray_pos.zoom <= self.options.camera.map_zoom * BUBBLE_ZOOM_SLACK;

        let active = self.active_window_id.clone();
        for (tile, tile_pos) in self.tiles_with_render_positions() {
            if layer.is_normal() == tile.is_moving_between_workspaces() {
                continue;
            }

            if on_map && layer.is_normal() {
                continue;
            }

            let focus_ring = focus_ring && Some(tile.window().id()) == active.as_ref();

            let xray_pos = xray_pos.offset(tile_pos);
            tile.render(ctx.r(), tile_pos, xray_pos, focus_ring, &mut |elem| {
                push(elem.into())
            });
        }

        if on_map && layer.is_normal() {
            // Computed once here and handed down, so the icons and the bubbles of a
            // single frame cannot be laid out from two different answers.
            let map_at = self.gathered_positions();
            self.render_map_icons(ctx.r(), xray_pos.zoom, &map_at, push);

            // Pushed after the icons, so the bubbles sit behind them.
            for ((island, (fill, ring)), centre) in self
                .islands
                .islands()
                .zip(self.bubbles.iter())
                .zip(map_at.iter())
            {
                let (_, _, bubble_radius) = cluster_geometry(island.items().len());
                let at = Point::from((centre.x - bubble_radius, centre.y - bubble_radius));
                push(ring.clone().with_location(at).into());
                push(fill.clone().with_location(at).into());
            }
        }
    }

    // The icons of everything in an island, pulled into a cluster at its centre.
    //
    // Laid out on a phyllotaxis spiral rather than a grid: it packs evenly at any count,
    // stays centred, and adding a window nudges the others outward instead of reflowing
    // the whole thing into a different shape.
    fn render_map_icons<R: ZenRenderer>(
        &self,
        mut ctx: RenderCtx<R>,
        zoom: f64,
        map_at: &[Point<f64, Logical>],
        push: &mut dyn FnMut(FloatingSpaceRenderElement<R>),
    ) {
        for (island, centre) in self.islands.islands().zip(map_at.iter()) {
            let members: Vec<&Tile<W>> = island
                .items()
                .iter()
                .filter_map(|id| self.tiles.iter().find(|t| t.window().id() == id))
                .collect();
            if members.is_empty() {
                continue;
            }

            let (diameter, spread, _) = cluster_geometry(members.len());
            let px = (diameter * zoom * self.scale).round().max(16.) as u32;

            for (i, tile) in members.iter().enumerate() {
                let offset = spiral_offset(i, spread);
                let at = Point::from((
                    centre.x + offset.x - diameter / 2.,
                    centre.y + offset.y - diameter / 2.,
                ));

                let app_id = tile.window().app_id();
                let Some(buffer) = self.icons.get::<R>(ctx.r().renderer, &app_id, px) else {
                    continue;
                };

                push(
                    PrimaryGpuTextureRenderElement(TextureRenderElement::from_texture_buffer(
                        buffer,
                        at,
                        1.,
                        None,
                        Some(Size::from((diameter, diameter))),
                        Kind::Unspecified,
                    ))
                    .into(),
                );
            }
        }
    }

    fn island_centre(&self, island: &crate::layout::island::Island<W::Id>) -> Point<f64, Logical> {
        let rect = island.rect();
        let loc = self.canvas_to_logical(rect.loc);
        Point::from((loc.x + rect.size.w / 2., loc.y + rect.size.h / 2.))
    }

    // What everything is pulled toward on the map: the middle of all the islands, so
    // the gathered cluster sits where the content is rather than at some fixed origin.
    fn map_anchor(&self) -> Point<f64, Logical> {
        let mut n = 0.;
        let mut sum = Point::from((0., 0.));
        for island in self.islands.islands() {
            sum += self.island_centre(island);
            n += 1.;
        }
        if n == 0. {
            return Point::from((0., 0.));
        }
        sum.downscale(n)
    }

    // Where the bubbles sit on the map.
    //
    // Two steps. First everything contracts toward the anchor, because islands are
    // spread over a canvas much larger than the screen and drawing them where they
    // really are puts the bubbles exactly where the windows already were, which is no
    // more legible than the windows.
    //
    // Contracting alone collapses islands that were already close into one another, so
    // a relaxation pass then pushes any overlapping pair apart along the line between
    // them until they only touch. The result reads as a cluster pulled together and
    // held just short of collision, and it keeps every bubble separately clickable.
    fn gathered_positions(&self) -> Vec<Point<f64, Logical>> {
        let gather = self.options.camera.map_gather.clamp(0.01, 1.);
        let anchor = self.map_anchor();

        let mut out: Vec<Point<f64, Logical>> = Vec::new();
        let mut radii: Vec<f64> = Vec::new();
        for island in self.islands.islands() {
            let real = self.island_centre(island);
            out.push(anchor + (real - anchor).upscale(gather));
            radii.push(cluster_geometry(island.items().len()).2);
        }

        // Deterministic and bounded: the same islands always relax to the same place, so
        // nothing jitters between frames.
        for _ in 0..24 {
            let mut moved = false;
            for i in 0..out.len() {
                for j in (i + 1)..out.len() {
                    let dx = out[j].x - out[i].x;
                    let dy = out[j].y - out[i].y;
                    let want = radii[i] + radii[j];
                    let mut distance = (dx * dx + dy * dy).sqrt();

                    // Exactly coincident centres have no line to push along, so they are
                    // nudged onto one before the usual correction takes over.
                    let (ux, uy) = if distance < 1e-6 {
                        distance = 0.;
                        let angle = i as f64 * 2.399_963_229_728_653;
                        (angle.cos(), angle.sin())
                    } else {
                        (dx / distance, dy / distance)
                    };

                    if distance >= want {
                        continue;
                    }

                    let push = (want - distance) / 2.;
                    out[i].x -= ux * push;
                    out[i].y -= uy * push;
                    out[j].x += ux * push;
                    out[j].y += uy * push;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }

        out
    }

    // The island under a point on the map, hit against the drawn bubble rather than the
    // window geometry: on the map the bubble is the only thing there is to click.
    pub fn map_bubble_at(&self, local: Point<f64, Logical>) -> Option<Rectangle<f64, Logical>> {
        let mut best: Option<(f64, Rectangle<f64, Logical>)> = None;

        // Recomputed rather than read from a cache the render fills in: a click can
        // arrive before the first map frame has been built, and a stale or empty cache
        // would silently make every bubble unclickable.
        let map_at = self.gathered_positions();

        for (island, centre) in self.islands.islands().zip(map_at.iter()) {
            let (_, _, radius) = cluster_geometry(island.items().len());
            let dx = local.x - centre.x;
            let dy = local.y - centre.y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > radius {
                continue;
            }

            // Bubbles can overlap when islands are close, so the nearest centre wins
            // rather than whichever comes first.
            if best.as_ref().is_none_or(|(d, _)| distance < *d) {
                let rect = island.rect();
                let loc = self.canvas_to_logical(rect.loc);
                let size = Size::from((rect.size.w, rect.size.h));
                best = Some((distance, Rectangle::new(loc, size)));
            }
        }

        best.map(|(_, rect)| rect)
    }

    pub fn interactive_resize_begin(&mut self, window: W::Id, edges: ResizeEdge) -> bool {
        if self.interactive_resize.is_some() {
            return false;
        }

        let tile = self
            .tiles
            .iter_mut()
            .find(|tile| tile.window().id() == &window)
            .unwrap();

        let original_window_size = tile.window_size();

        let resize = InteractiveResize {
            window,
            original_window_size,
            data: InteractiveResizeData { edges },
        };
        self.interactive_resize = Some(resize);

        true
    }

    pub fn interactive_resize_update(
        &mut self,
        window: &W::Id,
        delta: Point<f64, Logical>,
    ) -> bool {
        let Some(resize) = &self.interactive_resize else {
            return false;
        };

        if window != &resize.window {
            return false;
        }

        let original_window_size = resize.original_window_size;
        let edges = resize.data.edges;

        if edges.intersects(ResizeEdge::LEFT_RIGHT) {
            let mut dx = delta.x;
            if edges.contains(ResizeEdge::LEFT) {
                dx = -dx;
            };

            let window_width = (original_window_size.w + dx).round() as i32;
            self.set_window_width(Some(window), SizeChange::SetFixed(window_width), false);
        }

        if edges.intersects(ResizeEdge::TOP_BOTTOM) {
            let mut dy = delta.y;
            if edges.contains(ResizeEdge::TOP) {
                dy = -dy;
            };

            let window_height = (original_window_size.h + dy).round() as i32;
            self.set_window_height(Some(window), SizeChange::SetFixed(window_height), false);
        }

        true
    }

    pub fn interactive_resize_end(&mut self, window: Option<&W::Id>) {
        let Some(resize) = &self.interactive_resize else {
            return;
        };

        if let Some(window) = window {
            if window != &resize.window {
                return;
            }
        }

        self.interactive_resize = None;
    }

    pub fn refresh(&mut self, is_active: bool, is_focused: bool) {
        let active = self.active_window_id.clone();
        for tile in &mut self.tiles {
            let win = tile.window_mut();

            win.set_active_in_column(true);
            win.set_floating(true);

            let mut is_active = is_active && Some(win.id()) == active.as_ref();
            if self.options.deactivate_unfocused_windows {
                is_active &= is_focused;
            }
            win.set_activated(is_active);

            let resize_data = self
                .interactive_resize
                .as_ref()
                .filter(|resize| &resize.window == win.id())
                .map(|resize| resize.data);
            win.set_interactive_resize(resize_data);

            let border_config = self.options.layout.border.merged_with(&win.rules().border);
            let bounds = compute_toplevel_bounds(border_config, self.working_area.size);
            win.set_bounds(bounds);

            let intent = if self.options.disable_resize_throttling {
                ConfigureIntent::CanSend
            } else {
                win.configure_intent()
            };

            if matches!(
                intent,
                ConfigureIntent::CanSend | ConfigureIntent::ShouldSend
            ) {
                win.send_pending_configure();
            }

            win.refresh();
        }
    }

    pub fn clamp_within_working_area(
        &self,
        pos: Point<f64, Logical>,
        size: Size<f64, Logical>,
    ) -> Point<f64, Logical> {
        let mut rect = Rectangle::new(pos, size);
        clamp_preferring_top_left_in_area(self.working_area, &mut rect);
        rect.loc
    }

    pub fn tiles_bbox(&self) -> Option<Rectangle<f64, Logical>> {
        let mut acc: Option<Rectangle<f64, Logical>> = None;
        for data in &self.data {
            let r = Rectangle::new(data.logical_pos, data.size);
            acc = Some(match acc {
                None => r,
                Some(a) => {
                    let x0 = a.loc.x.min(r.loc.x);
                    let y0 = a.loc.y.min(r.loc.y);
                    let x1 = (a.loc.x + a.size.w).max(r.loc.x + r.size.w);
                    let y1 = (a.loc.y + a.size.h).max(r.loc.y + r.size.h);
                    Rectangle::new(Point::from((x0, y0)), Size::from((x1 - x0, y1 - y0)))
                }
            });
        }
        acc
    }

    pub fn canvas_to_logical(&self, pos: Point<f64, Canvas>) -> Point<f64, Logical> {
        Data::canvas_to_logical(pos)
    }

    pub fn logical_to_canvas(&self, logical_pos: Point<f64, Logical>) -> Point<f64, Canvas> {
        Data::logical_to_canvas(logical_pos)
    }

    fn move_and_animate(&mut self, idx: usize, new_pos: Point<f64, Logical>) {
        const ANIMATION_THRESHOLD_SQ: f64 = 10. * 10.;

        let tile = &mut self.tiles[idx];
        let data = &mut self.data[idx];

        let prev_pos = data.logical_pos;
        data.set_logical_pos(new_pos);
        let new_pos = data.logical_pos;

        let diff = prev_pos - new_pos;
        if diff.x * diff.x + diff.y * diff.y > ANIMATION_THRESHOLD_SQ {
            tile.animate_move_from(prev_pos - new_pos);
        }
    }

    pub fn new_window_size(
        &self,
        width: Option<PresetSize>,
        height: Option<PresetSize>,
        rules: &ResolvedWindowRules,
    ) -> Size<i32, Logical> {
        let border = self.options.layout.border.merged_with(&rules.border);

        let resolve = |size: Option<PresetSize>, working_area_size: f64| {
            if let Some(size) = size {
                let size = match resolve_preset_size(size, working_area_size) {
                    ResolvedSize::Tile(mut size) => {
                        if !border.off {
                            size -= border.width * 2.;
                        }
                        size
                    }
                    ResolvedSize::Window(size) => size,
                };

                max(1, size.floor() as i32)
            } else {
                0
            }
        };

        let width = resolve(width, self.working_area.size.w);
        let height = resolve(height, self.working_area.size.h);

        Size::from((width, height))
    }

    pub fn stored_or_default_tile_pos(&self, tile: &Tile<W>) -> Option<Point<f64, Logical>> {
        let pos = tile.floating_pos.map(|pos| self.canvas_to_logical(pos));
        pos.or_else(|| {
            tile.window().rules().default_floating_position.map(|pos| {
                let relative_to = pos.relative_to;
                let size = tile.tile_size();
                let area = self.working_area;

                let mut pos = Point::from((pos.x.0, pos.y.0));
                if relative_to == RelativeTo::TopRight
                    || relative_to == RelativeTo::BottomRight
                    || relative_to == RelativeTo::Right
                {
                    pos.x = area.size.w - size.w - pos.x;
                }
                if relative_to == RelativeTo::BottomLeft
                    || relative_to == RelativeTo::BottomRight
                    || relative_to == RelativeTo::Bottom
                {
                    pos.y = area.size.h - size.h - pos.y;
                }
                if relative_to == RelativeTo::Top || relative_to == RelativeTo::Bottom {
                    pos.x += area.size.w / 2.0 - size.w / 2.0
                }
                if relative_to == RelativeTo::Left || relative_to == RelativeTo::Right {
                    pos.y += area.size.h / 2.0 - size.h / 2.0
                }

                pos + self.working_area.loc
            })
        })
    }

    #[cfg(test)]
    pub fn view_size(&self) -> Size<f64, Logical> {
        self.view_size
    }

    pub fn working_area(&self) -> Rectangle<f64, Logical> {
        self.working_area
    }

    #[cfg(test)]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    #[cfg(test)]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    #[cfg(test)]
    pub fn options(&self) -> &Rc<Options> {
        &self.options
    }

    // -
    // -

    pub fn set_spawn_center(&mut self, center: Option<Point<f64, Canvas>>) {
        self.spawn_center = center;
    }

    fn spawn_anchor(&self, size: Size<f64, Logical>) -> Point<f64, Logical> {
        let Some(center) = self.spawn_center else {
            let pos = center_preferring_top_left_in_area(self.working_area, size);
            return Point::from((pos.x + size.w / 2., pos.y + size.h / 2.));
        };

        let preferred = Point::from((center.x - size.w / 2., center.y - size.h / 2.));
        let spot = island::free_spot_near(&self.islands, preferred, size, 48.);
        Point::from((
            center.x + (spot.x - preferred.x),
            center.y + (spot.y - preferred.y),
        ))
    }

    pub fn islands(&self) -> &IslandSpace<W::Id> {
        &self.islands
    }

    pub fn island_of(&self, id: &W::Id) -> Option<IslandId> {
        self.islands
            .islands()
            .find(|i| i.items().iter().any(|item| item == id))
            .map(|i| i.id())
    }

    pub fn island_windows(&self, island: IslandId) -> &[W::Id] {
        self.islands.get(island).map_or(&[], |i| i.items())
    }

    fn activate_island_of(&mut self, id: &W::Id) {
        let Some(island) = self.island_of(id) else {
            return;
        };
        self.islands.set_active(island);
        if let Some(island) = self.islands.get_mut(island) {
            if let Some(idx) = island.items().iter().position(|item| item == id) {
                island.set_active_idx(idx);
            }
        }
    }

    fn grown_for(
        &self,
        current: Size<f64, Logical>,
        size: Size<f64, Logical>,
    ) -> Size<f64, Logical> {
        let gap = self.options.layout.gaps;
        let max: Size<f64, Logical> = Size::from((
            (self.working_area.size.w - gap * 2.).max(0.),
            (self.working_area.size.h - gap * 2.).max(0.),
        ));
        Size::from((
            (current.w + gap + size.w).min(max.w.max(current.w)),
            current.h.max(size.h).min(max.h.max(current.h)),
        ))
    }

    fn grow_island(&mut self, target: IslandId, size: Size<f64, Logical>) {
        let Some(island) = self.islands.get(target) else {
            return;
        };
        let before = island.size();
        let pos = island.pos();
        let after = self.grown_for(before, size);

        let Some(island) = self.islands.get_mut(target) else {
            return;
        };
        island.set_size(after);
        island.set_pos(Point::from((
            pos.x - (after.w - before.w) / 2.,
            pos.y - (after.h - before.h) / 2.,
        )));
    }

    fn register_island(&mut self, idx: usize, join: Option<IslandId>) {
        let id = self.tiles[idx].window().id().clone();
        let data = self.data[idx];

        if let Some(target) = join {
            if self.islands.get(target).is_some() {
                self.grow_island(target, data.size);
                self.islands.get_mut(target).unwrap().add(id);

                self.islands.set_active(target);
                self.sync_islands();
                return;
            }
        }

        let mut island = Island::new(data.pos, data.size, IslandLayout::Columns);
        island.add(id);
        self.islands.add(island);
    }

    fn unregister_island(&mut self, id: &W::Id) {
        let Some(island_id) = self.island_of(id) else {
            return;
        };
        let Some(island) = self.islands.get_mut(island_id) else {
            return;
        };
        if let Some(idx) = island.items().iter().position(|item| item == id) {
            island.remove(idx);
        }
        if island.is_empty() {
            self.islands.remove(island_id);
        }
    }

    fn sync_islands(&mut self) {
        let gap = self.options.layout.gaps;

        let mut follows: Vec<(IslandId, Point<f64, Canvas>, Size<f64, Logical>)> = Vec::new();
        let mut moves: Vec<(usize, Point<f64, Logical>)> = Vec::new();
        let mut resizes: Vec<(usize, Size<f64, Logical>)> = Vec::new();

        for island in self.islands.islands() {
            let items = island.items();
            if items.len() == 1 {
                if let Some(idx) = self.idx_of(&items[0]) {
                    follows.push((island.id(), self.data[idx].pos, self.data[idx].size));
                }
                continue;
            }

            let origin = island.pos();
            for (item, rect) in zip(items, island.item_rects(gap)) {
                let Some(idx) = self.idx_of(item) else {
                    continue;
                };

                let pos = Data::canvas_to_logical(Point::from((
                    origin.x + rect.loc.x,
                    origin.y + rect.loc.y,
                )));
                if self.data[idx].logical_pos != pos {
                    moves.push((idx, pos));
                }

                let have = self.tiles[idx].tile_expected_or_current_size();
                if (rect.size.w - have.w).abs() > 0.5 || (rect.size.h - have.h).abs() > 0.5 {
                    resizes.push((idx, rect.size));
                }
            }
        }

        for (id, pos, size) in follows {
            if let Some(island) = self.islands.get_mut(id) {
                island.set_pos(pos);
                island.set_size(size);
            }
        }
        for (idx, pos) in moves {
            self.move_and_animate(idx, pos);
        }
        for (idx, size) in resizes {
            self.tiles[idx].request_tile_size(size, true, None);
        }
    }

    pub fn join_island(&mut self, window: &W::Id, target: IslandId) -> bool {
        let Some(from) = self.island_of(window) else {
            return false;
        };
        if from == target {
            return false;
        }
        let Some(idx) = self.idx_of(window) else {
            return false;
        };
        let size = self.data[idx].size;

        if self.islands.get(target).is_none() {
            return false;
        }

        self.unregister_island(window);

        if self.islands.get(target).is_none() {
            return false;
        }
        self.grow_island(target, size);
        self.islands.get_mut(target).unwrap().add(window.clone());

        self.islands.set_active(target);
        self.sync_islands();
        true
    }

    pub fn split_island(&mut self, window: &W::Id) -> bool {
        let Some(from) = self.island_of(window) else {
            return false;
        };
        if self.islands.get(from).map(|i| i.len()) == Some(1) {
            return false;
        }
        let Some(idx) = self.idx_of(window) else {
            return false;
        };
        let (pos, size) = (self.data[idx].pos, self.data[idx].size);

        self.unregister_island(window);

        let mut island = Island::new(pos, size, IslandLayout::Columns);
        island.add(window.clone());
        self.islands.add(island);

        self.sync_islands();
        true
    }

    pub fn set_island_layout(&mut self, island: IslandId, layout: IslandLayout) -> bool {
        let Some(island) = self.islands.get_mut(island) else {
            return false;
        };
        island.set_layout(layout);
        self.sync_islands();
        true
    }

    pub fn move_island_by(&mut self, island: IslandId, delta: Point<f64, Logical>) -> bool {
        let Some(island) = self.islands.get_mut(island) else {
            return false;
        };
        let pos = island.pos();
        island.set_pos(Point::from((pos.x + delta.x, pos.y + delta.y)));
        self.sync_islands();
        true
    }

    pub fn focus_island_in_direction(&mut self, dir: Direction) -> bool {
        if let Some(active) = self.active_window_id.clone() {
            self.activate_island_of(&active);
        }

        let Some(next) = self.islands.neighbour(dir) else {
            return false;
        };
        let Some(win) = self
            .islands
            .get(next)
            .and_then(|i| i.active())
            .filter(|id| self.contains(id))
            .cloned()
        else {
            return false;
        };

        self.islands.set_active(next);
        self.activate_window(&win)
    }

    pub fn move_active_to_island(&mut self, dir: Direction) -> bool {
        let Some(active) = self.active_window_id.clone() else {
            return false;
        };

        self.activate_island_of(&active);

        let Some(target) = self.islands.neighbour(dir) else {
            return false;
        };
        self.join_island(&active, target)
    }

    pub fn split_active_from_island(&mut self) -> bool {
        let Some(active) = self.active_window_id.clone() else {
            return false;
        };
        self.split_island(&active)
    }

    pub fn spawn_target(&self, ctx: &SpawnContext) -> SpawnTarget {
        island::spawn_target(&self.islands, ctx)
    }

    pub fn free_island_spot(
        &self,
        preferred: Point<f64, Canvas>,
        size: Size<f64, Logical>,
    ) -> Point<f64, Canvas> {
        island::free_spot_near(&self.islands, preferred, size, 48.)
    }

    pub fn islands_bbox(&self) -> Option<Rectangle<f64, Canvas>> {
        self.islands.bbox()
    }

    #[cfg(test)]
    pub fn verify_invariants(&self) {
        assert!(self.scale > 0.);
        assert!(self.scale.is_finite());
        assert_eq!(self.tiles.len(), self.data.len());

        for (i, (tile, data)) in zip(&self.tiles, &self.data).enumerate() {
            use crate::layout::SizingMode;

            assert!(Rc::ptr_eq(&self.options, &tile.options));
            assert_eq!(self.view_size, tile.view_size());
            assert_eq!(self.clock, tile.clock);
            assert_eq!(self.scale, tile.scale());
            tile.verify_invariants();

            if let Some(idx) = tile.floating_preset_width_idx {
                assert!(idx < self.options.layout.preset_column_widths.len());
            }
            if let Some(idx) = tile.floating_preset_height_idx {
                assert!(idx < self.options.layout.preset_window_heights.len());
            }

            assert_eq!(
                tile.window().pending_sizing_mode(),
                SizingMode::Normal,
                "floating windows cannot be maximized or fullscreen"
            );

            data.verify_invariants();

            let mut data2 = *data;
            data2.update(tile);
            data2.update_config(self.working_area, self.options.camera.infinite_canvas);
            assert_eq!(data, &data2, "tile data must be up to date");

            for tile_below in &self.tiles[i + 1..] {
                assert!(
                    !tile_below.window().is_child_of(tile.window()),
                    "children must be stacked above parents"
                );
            }
        }

        if let Some(id) = &self.active_window_id {
            assert!(!self.tiles.is_empty());
            assert!(self.contains(id), "active window must be present in tiles");
        } else {
            assert!(self.tiles.is_empty());
        }

        if let Some(resize) = &self.interactive_resize {
            assert!(
                self.contains(&resize.window),
                "interactive resize window must be present in tiles"
            );
        }

        let mut seen = 0usize;
        for island in self.islands.islands() {
            assert!(!island.is_empty(), "an empty island must be removed, not kept");
            for id in island.items() {
                assert!(
                    self.contains(id),
                    "island {:?} holds {id:?}, which is not in this space",
                    island.id()
                );
                seen += 1;
            }
        }
        assert_eq!(
            seen,
            self.tiles.len(),
            "every tile must be in exactly one island"
        );
        for tile in &self.tiles {
            assert!(
                self.island_of(tile.window().id()).is_some(),
                "tile {:?} is in no island",
                tile.window().id()
            );
        }
    }
}

fn compute_toplevel_bounds(
    border_config: zen_config::Border,
    working_area_size: Size<f64, Logical>,
) -> Size<i32, Logical> {
    let mut border = 0.;
    if !border_config.off {
        border = border_config.width * 2.;
    }

    Size::from((
        f64::max(working_area_size.w - border, 1.),
        f64::max(working_area_size.h - border, 1.),
    ))
    .to_i32_floor()
}

fn resolve_preset_size(preset: PresetSize, view_size: f64) -> ResolvedSize {
    match preset {
        PresetSize::Proportion(proportion) => ResolvedSize::Tile(view_size * proportion),
        PresetSize::Fixed(width) => ResolvedSize::Window(f64::from(width)),
    }
}


// One icon size, the radius the cluster spreads over, and the radius of the bubble
// around it. Shared by the bubble and the icons so the two cannot drift apart.
//
// The cluster grows as the square root of the count, which is what keeps the icons at a
// constant size while the bubble grows: area per icon stays the same.
fn cluster_geometry(count: usize) -> (f64, f64, f64) {
    // A sunflower's radius grows as sqrt(index), so the outermost icon of a cluster of
    // n sits at sqrt(n-1) times the step. The step is a little under one icon width,
    // which is what makes them touch and overlap rather than sit in tidy rings.
    let step = ICON_SIZE * 0.72;
    let outermost = (count.max(1) - 1) as f64;
    let spread = step * outermost.sqrt();
    let bubble = spread + ICON_SIZE / 2. + BUBBLE_PAD;
    (ICON_SIZE, step, bubble)
}

// A point on the phyllotaxis spiral, which is how a sunflower packs seeds: evenly, from
// the middle out, with no ring boundaries to fall on. Index 0 is dead centre, so a lone
// window sits in the middle of its bubble instead of off to one side.
fn spiral_offset(index: usize, step: f64) -> Point<f64, Logical> {
    if index == 0 {
        return Point::from((0., 0.));
    }

    const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
    let r = step * (index as f64).sqrt();
    let angle = index as f64 * GOLDEN_ANGLE;
    Point::from((r * angle.cos(), r * angle.sin()))
}

#[cfg(test)]
mod map_cluster_tests {
    use super::*;

    // The bubble is drawn from cluster_geometry and the icons are placed from
    // spiral_offset. If those two ever disagree the icons hang outside their bubble,
    // which looks like a bug in the layout rather than in the arithmetic.
    #[test]
    fn every_icon_fits_inside_its_bubble() {
        for count in 1..40usize {
            let (diameter, step, bubble) = cluster_geometry(count);
            for i in 0..count {
                let offset = spiral_offset(i, step);
                let reach = (offset.x * offset.x + offset.y * offset.y).sqrt() + diameter / 2.;
                assert!(
                    reach <= bubble + 0.001,
                    "count {count}, icon {i}: reaches {reach} but the bubble is {bubble}"
                );
            }
        }
    }

    #[test]
    fn a_lone_icon_sits_in_the_middle() {
        let (_, step, _) = cluster_geometry(1);
        let at = spiral_offset(0, step);
        assert_eq!((at.x, at.y), (0., 0.));
    }

    // Constant icon size is the point of the sqrt growth: a busy island gets a bigger
    // bubble, not smaller icons.
    #[test]
    fn the_bubble_grows_but_the_icons_do_not() {
        let (small_icon, _, small_bubble) = cluster_geometry(2);
        let (big_icon, _, big_bubble) = cluster_geometry(20);
        assert_eq!(small_icon, big_icon);
        assert!(big_bubble > small_bubble);
    }
}
