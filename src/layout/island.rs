//! Islands: free-positioned clusters of windows that tile internally.
//!
//! This module holds the parts that are pure geometry, deliberately separated from anything
//! generic over `LayoutElement` so they can be tested on their own. Two things live here:
//!
//! - **Internal layout.** How an island arranges the tiles it holds. Reuses the shape of zen's
//!   column model rather than inventing one.
//! - **Spatial navigation.** `nearest_in_direction`, which is what `focus_left` and its 23
//!   siblings become on a canvas. On a strip those were index arithmetic; a canvas has no index
//!   to do arithmetic on, so direction becomes a real spatial query -- and a better-defined one.
//!
//! See `docs/zen/PHASE4-TRIAGE.md` for how this fits the rest of the phase.

use smithay::utils::{Logical, Rectangle};

/// Identifies an island within a space.
///
/// Monotonic and never reused, so a stale id fails to resolve rather than silently naming
/// whatever took its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IslandId(u32);

impl IslandId {
    pub fn next() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// How an island arranges the tiles it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IslandLayout {
    /// One tile filling the island. What a freshly spawned island is.
    #[default]
    Single,
    /// Side-by-side columns, the way zen's scrolling space tiles -- minus the scrolling.
    Columns,
    /// Stacked, one visible at a time.
    Tabbed,
}

/// A direction for spatial navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    /// Whether this direction runs along the x axis.
    fn is_horizontal(self) -> bool {
        matches!(self, Direction::Left | Direction::Right)
    }

    /// -1 or +1 along the direction's axis.
    fn sign(self) -> f64 {
        match self {
            Direction::Left | Direction::Up => -1.,
            Direction::Right | Direction::Down => 1.,
        }
    }
}

/// Lays out `count` tiles inside an island of `size`, with `gap` between them.
///
/// Returns rectangles in island-local coordinates. Columns divide the width evenly, which is
/// the sane default; per-column widths come later, lifted from `ScrollingSpace`'s `ColumnWidth`
/// rather than reinvented.
pub fn layout_tiles(
    layout: IslandLayout,
    size: (f64, f64),
    count: usize,
    gap: f64,
) -> Vec<Rectangle<f64, Logical>> {
    use smithay::utils::{Point, Size};

    if count == 0 {
        return Vec::new();
    }

    let (w, h) = size;

    match layout {
        // Tabbed shows one at a time, so every tile gets the full island. Which one is visible
        // is the island's business, not the geometry's.
        IslandLayout::Single | IslandLayout::Tabbed => (0..count)
            .map(|_| {
                Rectangle::new(Point::from((0., 0.)), Size::from((w.max(0.), h.max(0.))))
            })
            .collect(),

        IslandLayout::Columns => {
            let n = count as f64;
            let total_gap = gap * (n - 1.);
            // Never hand back a negative width: a heavily gapped island with many tiles would
            // otherwise produce inverted rectangles that break hit-testing downstream.
            let col_w = ((w - total_gap) / n).max(0.);

            (0..count)
                .map(|i| {
                    let x = (col_w + gap) * i as f64;
                    Rectangle::new(
                        Point::from((x, 0.)),
                        Size::from((col_w, h.max(0.))),
                    )
                })
                .collect()
        }
    }
}

/// Finds the nearest candidate in `dir` from `from`.
///
/// This replaces index arithmetic, and is a better definition than the thing it replaces: on a
/// strip `focus_left` means "index - 1", which cannot express a two-dimensional arrangement at
/// all.
///
/// Candidates whose perpendicular extent overlaps `from` win over ones that do not, so moving
/// left from a window prefers the window actually beside it over something further away
/// diagonally. Among equals, nearest along the direction's axis wins.
pub fn nearest_in_direction<T: Copy>(
    from: Rectangle<f64, Logical>,
    candidates: &[(T, Rectangle<f64, Logical>)],
    dir: Direction,
) -> Option<T> {
    let horizontal = dir.is_horizontal();
    let sign = dir.sign();

    // Along the direction's axis, and across it.
    let along = |r: Rectangle<f64, Logical>| if horizontal { r.loc.x } else { r.loc.y };
    let along_size = |r: Rectangle<f64, Logical>| if horizontal { r.size.w } else { r.size.h };
    let across = |r: Rectangle<f64, Logical>| if horizontal { r.loc.y } else { r.loc.x };
    let across_size = |r: Rectangle<f64, Logical>| if horizontal { r.size.h } else { r.size.w };

    let from_centre_along = along(from) + along_size(from) / 2.;
    let from_lo = across(from);
    let from_hi = across(from) + across_size(from);

    let mut best: Option<(bool, f64, f64, T)> = None;

    for &(item, rect) in candidates {
        let centre_along = along(rect) + along_size(rect) / 2.;
        let delta = (centre_along - from_centre_along) * sign;

        // Strictly beyond us in this direction. Equal centres are ambiguous rather than
        // adjacent, so they are skipped.
        if delta <= 0. {
            continue;
        }

        let lo = across(rect);
        let hi = across(rect) + across_size(rect);
        let overlaps = lo < from_hi && from_lo < hi;

        // Distance across the axis, zero when they overlap.
        let across_gap = if overlaps {
            0.
        } else if hi <= from_lo {
            from_lo - hi
        } else {
            lo - from_hi
        };

        // Overlapping beats non-overlapping outright; then nearest along; then nearest across.
        let key = (!overlaps, delta, across_gap, item);
        let better = match &best {
            None => true,
            Some((b_no_overlap, b_delta, b_across, _)) => {
                (key.0, key.1, key.2) < (*b_no_overlap, *b_delta, *b_across)
            }
        };
        if better {
            best = Some(key);
        }
    }

    best.map(|(_, _, _, item)| item)
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Point, Size};

    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
        Rectangle::new(Point::from((x, y)), Size::from((w, h)))
    }

    #[test]
    fn ids_are_unique_and_monotonic() {
        let a = IslandId::next();
        let b = IslandId::next();
        assert_ne!(a, b);
        assert!(b.get() > a.get());
    }

    #[test]
    fn single_and_tabbed_give_every_tile_the_whole_island() {
        for layout in [IslandLayout::Single, IslandLayout::Tabbed] {
            let tiles = layout_tiles(layout, (800., 600.), 3, 8.);
            assert_eq!(tiles.len(), 3);
            for t in tiles {
                assert_eq!(t.loc, Point::from((0., 0.)));
                assert_eq!(t.size, Size::from((800., 600.)));
            }
        }
    }

    #[test]
    fn columns_divide_the_width_and_respect_gaps() {
        let tiles = layout_tiles(IslandLayout::Columns, (800., 600.), 3, 10.);
        assert_eq!(tiles.len(), 3);

        // Two gaps of 10 leave 780 for three columns.
        for t in &tiles {
            assert!((t.size.w - 260.).abs() < 1e-9, "got {}", t.size.w);
            assert_eq!(t.size.h, 600.);
        }
        assert_eq!(tiles[0].loc.x, 0.);
        assert!((tiles[1].loc.x - 270.).abs() < 1e-9);
        assert!((tiles[2].loc.x - 540.).abs() < 1e-9);

        // The last column ends exactly at the island's edge.
        let last = tiles[2];
        assert!((last.loc.x + last.size.w - 800.).abs() < 1e-9);
    }

    /// A heavily gapped island must not produce inverted rectangles.
    #[test]
    fn columns_never_go_negative() {
        let tiles = layout_tiles(IslandLayout::Columns, (100., 200.), 20, 40.);
        for t in tiles {
            assert!(t.size.w >= 0., "negative width would break hit-testing");
            assert!(t.size.h >= 0.);
        }
    }

    #[test]
    fn empty_island_lays_out_nothing() {
        assert!(layout_tiles(IslandLayout::Columns, (800., 600.), 0, 8.).is_empty());
    }

    #[test]
    fn nearest_picks_the_adjacent_one() {
        let from = rect(0., 0., 100., 100.);
        let candidates = [
            (1u32, rect(200., 0., 100., 100.)),  // directly right, adjacent
            (2u32, rect(500., 0., 100., 100.)),  // directly right, further
            (3u32, rect(-200., 0., 100., 100.)), // left
        ];
        assert_eq!(
            nearest_in_direction(from, &candidates, Direction::Right),
            Some(1)
        );
        assert_eq!(
            nearest_in_direction(from, &candidates, Direction::Left),
            Some(3)
        );
    }

    /// An aligned neighbour beats a nearer diagonal one -- the reason for the overlap rule.
    #[test]
    fn aligned_beats_diagonal() {
        let from = rect(0., 0., 100., 100.);
        let candidates = [
            (1u32, rect(150., 400., 100., 100.)), // nearer along x, but way off across
            (2u32, rect(300., 0., 100., 100.)),   // further, but level with us
        ];
        assert_eq!(
            nearest_in_direction(from, &candidates, Direction::Right),
            Some(2),
            "should prefer the window actually beside it over a closer diagonal"
        );
    }

    #[test]
    fn nothing_in_that_direction_is_none() {
        let from = rect(0., 0., 100., 100.);
        let candidates = [(1u32, rect(-300., 0., 100., 100.))];
        assert_eq!(nearest_in_direction(from, &candidates, Direction::Right), None);
    }

    /// Something sitting exactly on top of us is ambiguous, not adjacent.
    #[test]
    fn identical_position_is_not_a_neighbour() {
        let from = rect(0., 0., 100., 100.);
        let candidates = [(1u32, rect(0., 0., 100., 100.))];
        for dir in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            assert_eq!(nearest_in_direction(from, &candidates, dir), None);
        }
    }

    #[test]
    fn vertical_directions_work_the_same_way() {
        let from = rect(0., 0., 100., 100.);
        let candidates = [
            (1u32, rect(0., 200., 100., 100.)),
            (2u32, rect(0., -200., 100., 100.)),
        ];
        assert_eq!(
            nearest_in_direction(from, &candidates, Direction::Down),
            Some(1)
        );
        assert_eq!(
            nearest_in_direction(from, &candidates, Direction::Up),
            Some(2)
        );
    }

    /// Every direction from the middle of a ring should find its own side.
    #[test]
    fn a_ring_resolves_in_all_four_directions() {
        let from = rect(0., 0., 100., 100.);
        let candidates = [
            (1u32, rect(300., 0., 100., 100.)),
            (2u32, rect(-300., 0., 100., 100.)),
            (3u32, rect(0., 300., 100., 100.)),
            (4u32, rect(0., -300., 100., 100.)),
        ];
        assert_eq!(nearest_in_direction(from, &candidates, Direction::Right), Some(1));
        assert_eq!(nearest_in_direction(from, &candidates, Direction::Left), Some(2));
        assert_eq!(nearest_in_direction(from, &candidates, Direction::Down), Some(3));
        assert_eq!(nearest_in_direction(from, &candidates, Direction::Up), Some(4));
    }
}

// ---------------------------------------------------------------------------------------------
// The container
//
// Generic over what an island holds rather than over `LayoutElement`, for the same reason the
// geometry above is separate: it can be tested without dragging a mock window into scope. The
// real thing is `IslandSpace<Tile<W>>`; the tests below use `IslandSpace<u32>`.
// ---------------------------------------------------------------------------------------------

use smithay::utils::{Point, Size};

use super::Canvas;

/// A cluster of windows at a place on the canvas, tiled internally.
#[derive(Debug)]
pub struct Island<T> {
    id: IslandId,
    /// Top-left corner on the canvas.
    pos: Point<f64, Canvas>,
    /// Size in logical pixels.
    size: Size<f64, Logical>,
    layout: IslandLayout,
    /// Contents, in the island's internal order.
    items: Vec<T>,
    /// Index into `items`. Meaningless when empty; kept in range at all times so callers never
    /// have to reason about a stale index.
    active_idx: usize,
}

impl<T> Island<T> {
    pub fn new(pos: Point<f64, Canvas>, size: Size<f64, Logical>, layout: IslandLayout) -> Self {
        Self {
            id: IslandId::next(),
            pos,
            size,
            layout,
            items: Vec::new(),
            active_idx: 0,
        }
    }

    pub fn id(&self) -> IslandId {
        self.id
    }

    pub fn pos(&self) -> Point<f64, Canvas> {
        self.pos
    }

    pub fn set_pos(&mut self, pos: Point<f64, Canvas>) {
        self.pos = pos;
    }

    pub fn size(&self) -> Size<f64, Logical> {
        self.size
    }

    pub fn set_size(&mut self, size: Size<f64, Logical>) {
        self.size = Size::from((size.w.max(0.), size.h.max(0.)));
    }

    pub fn layout(&self) -> IslandLayout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: IslandLayout) {
        self.layout = layout;
    }

    /// The island's footprint on the canvas.
    ///
    /// `size` is stored island-local; canvas and logical are both plain logical pixels, so this
    /// is a unit relabel rather than a conversion.
    pub fn rect(&self) -> Rectangle<f64, Canvas> {
        Rectangle::new(self.pos, Size::from((self.size.w, self.size.h)))
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [T] {
        &mut self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Adds an item next to the active one and focuses it.
    ///
    /// Inserting beside the active item rather than appending is what makes a new window appear
    /// where you were looking within the island, instead of always at the far end.
    pub fn add(&mut self, item: T) -> usize {
        let idx = if self.items.is_empty() {
            0
        } else {
            self.active_idx + 1
        };
        self.items.insert(idx, item);
        self.active_idx = idx;
        idx
    }

    pub fn remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.items.len() {
            return None;
        }
        let item = self.items.remove(idx);

        // Keep the active index in range, preferring the item that slid into the gap.
        if self.items.is_empty() {
            self.active_idx = 0;
        } else if self.active_idx >= self.items.len() {
            self.active_idx = self.items.len() - 1;
        } else if idx < self.active_idx {
            self.active_idx -= 1;
        }

        Some(item)
    }

    pub fn active_idx(&self) -> Option<usize> {
        (!self.items.is_empty()).then_some(self.active_idx)
    }

    pub fn active(&self) -> Option<&T> {
        self.items.get(self.active_idx)
    }

    pub fn set_active_idx(&mut self, idx: usize) -> bool {
        if idx < self.items.len() {
            self.active_idx = idx;
            true
        } else {
            false
        }
    }

    /// Where each item sits, in island-local coordinates.
    pub fn item_rects(&self, gap: f64) -> Vec<Rectangle<f64, Logical>> {
        layout_tiles(
            self.layout,
            (self.size.w, self.size.h),
            self.items.len(),
            gap,
        )
    }

    /// The item under a point given in island-local coordinates.
    ///
    /// Tabbed islands stack every item in the same place, so only the active one can be hit --
    /// otherwise a click would land on whichever happened to be first in the list.
    pub fn item_at(&self, local: Point<f64, Logical>, gap: f64) -> Option<usize> {
        let rects = self.item_rects(gap);
        match self.layout {
            IslandLayout::Tabbed => {
                let idx = self.active_idx;
                rects.get(idx).and_then(|r| r.contains(local).then_some(idx))
            }
            _ => rects.iter().position(|r| r.contains(local)),
        }
    }

    #[cfg(test)]
    fn verify_invariants(&self) {
        assert!(
            self.size.w >= 0. && self.size.h >= 0.,
            "negative island size"
        );
        assert!(self.pos.x.is_finite() && self.pos.y.is_finite());
        if self.items.is_empty() {
            assert_eq!(self.active_idx, 0, "empty island must park active_idx at 0");
        } else {
            assert!(
                self.active_idx < self.items.len(),
                "active_idx {} out of range for {} items",
                self.active_idx,
                self.items.len()
            );
        }
    }
}

/// The islands on one canvas, in top-to-bottom Z order.
///
/// Flat `Vec` with explicit Z-order and linear hit-testing, exactly like `FloatingSpace`. A
/// quadtree buys nothing here: a linear scan over a few hundred rectangles is microseconds, and
/// the ordering has to be explicit anyway because Z is a user-visible property rather than a
/// derived one.
#[derive(Debug)]
pub struct IslandSpace<T> {
    islands: Vec<Island<T>>,
    /// Which island has focus. Not necessarily the topmost: focus-follows-mouse should focus
    /// without raising, because raising on hover is maddening.
    active: Option<IslandId>,
}

impl<T> Default for IslandSpace<T> {
    fn default() -> Self {
        Self {
            islands: Vec::new(),
            active: None,
        }
    }
}

impl<T> IslandSpace<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Islands from top to bottom.
    pub fn islands(&self) -> impl Iterator<Item = &Island<T>> {
        self.islands.iter()
    }

    pub fn islands_mut(&mut self) -> impl Iterator<Item = &mut Island<T>> {
        self.islands.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.islands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.islands.is_empty()
    }

    /// Adds an island on top and focuses it.
    pub fn add(&mut self, island: Island<T>) -> IslandId {
        let id = island.id();
        self.islands.insert(0, island);
        self.active = Some(id);
        id
    }

    pub fn remove(&mut self, id: IslandId) -> Option<Island<T>> {
        let idx = self.islands.iter().position(|i| i.id() == id)?;
        let island = self.islands.remove(idx);

        if self.active == Some(id) {
            // Focus whatever is now topmost, rather than leaving focus dangling.
            self.active = self.islands.first().map(|i| i.id());
        }

        Some(island)
    }

    pub fn get(&self, id: IslandId) -> Option<&Island<T>> {
        self.islands.iter().find(|i| i.id() == id)
    }

    pub fn get_mut(&mut self, id: IslandId) -> Option<&mut Island<T>> {
        self.islands.iter_mut().find(|i| i.id() == id)
    }

    pub fn active_id(&self) -> Option<IslandId> {
        self.active
    }

    pub fn active(&self) -> Option<&Island<T>> {
        self.active.and_then(|id| self.get(id))
    }

    pub fn set_active(&mut self, id: IslandId) -> bool {
        if self.islands.iter().any(|i| i.id() == id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    /// Brings an island to the front. Does not change focus.
    pub fn raise(&mut self, id: IslandId) -> bool {
        let Some(idx) = self.islands.iter().position(|i| i.id() == id) else {
            return false;
        };
        let island = self.islands.remove(idx);
        self.islands.insert(0, island);
        true
    }

    /// The topmost island containing a canvas point.
    pub fn island_at(&self, point: Point<f64, Canvas>) -> Option<&Island<T>> {
        self.islands.iter().find(|i| i.rect().contains(point))
    }

    /// The nearest island in a direction from the active one.
    ///
    /// This is what the 24 direction functions route through. See `nearest_in_direction`.
    pub fn neighbour(&self, dir: Direction) -> Option<IslandId> {
        let from = self.active()?.rect();

        // The spatial helper works in `Logical`; islands live in `Canvas`. Both are plain
        // logical pixels, so this is a unit change rather than a conversion.
        let strip = |r: Rectangle<f64, Canvas>| -> Rectangle<f64, Logical> {
            Rectangle::new(
                Point::from((r.loc.x, r.loc.y)),
                Size::from((r.size.w, r.size.h)),
            )
        };

        let candidates: Vec<(IslandId, Rectangle<f64, Logical>)> = self
            .islands
            .iter()
            .filter(|i| Some(i.id()) != self.active)
            .map(|i| (i.id(), strip(i.rect())))
            .collect();

        nearest_in_direction(strip(from), &candidates, dir)
    }

    /// Bounding box of every island, for framing the lot.
    pub fn bbox(&self) -> Option<Rectangle<f64, Canvas>> {
        let mut acc: Option<Rectangle<f64, Canvas>> = None;
        for island in &self.islands {
            let r = island.rect();
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

    #[cfg(test)]
    fn verify_invariants(&self) {
        for island in &self.islands {
            island.verify_invariants();
        }

        let mut ids: Vec<_> = self.islands.iter().map(|i| i.id()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "island ids must be unique within a space");

        if let Some(active) = self.active {
            assert!(
                self.islands.iter().any(|i| i.id() == active),
                "active island must exist"
            );
        } else {
            assert!(
                self.islands.is_empty(),
                "a non-empty space must have an active island"
            );
        }
    }
}

#[cfg(test)]
mod container_tests {
    use super::*;

    fn island_at(x: f64, y: f64, w: f64, h: f64, layout: IslandLayout) -> Island<u32> {
        Island::new(
            Point::from((x, y)),
            Size::from((w, h)),
            layout,
        )
    }

    fn space_with_three() -> (IslandSpace<u32>, [IslandId; 3]) {
        let mut space = IslandSpace::new();
        let a = space.add(island_at(0., 0., 200., 200., IslandLayout::Single));
        let b = space.add(island_at(400., 0., 200., 200., IslandLayout::Single));
        let c = space.add(island_at(0., 400., 200., 200., IslandLayout::Single));
        (space, [a, b, c])
    }

    #[test]
    fn adding_puts_it_on_top_and_focuses_it() {
        let (space, [_, _, c]) = space_with_three();
        assert_eq!(space.len(), 3);
        assert_eq!(space.islands().next().unwrap().id(), c, "last added is topmost");
        assert_eq!(space.active_id(), Some(c));
        space.verify_invariants();
    }

    #[test]
    fn raising_does_not_steal_focus() {
        let (mut space, [a, _, c]) = space_with_three();
        assert!(space.raise(a));
        assert_eq!(space.islands().next().unwrap().id(), a, "raised to the front");
        assert_eq!(
            space.active_id(),
            Some(c),
            "raising must not change focus -- focus-follows-mouse would be unbearable otherwise"
        );
        space.verify_invariants();
    }

    #[test]
    fn removing_the_active_island_refocuses() {
        let (mut space, [_, _, c]) = space_with_three();
        assert!(space.remove(c).is_some());
        assert_eq!(space.len(), 2);
        assert!(space.active_id().is_some(), "focus must not dangle");
        assert_ne!(space.active_id(), Some(c));
        space.verify_invariants();
    }

    #[test]
    fn emptying_the_space_clears_focus() {
        let (mut space, ids) = space_with_three();
        for id in ids {
            space.remove(id);
        }
        assert!(space.is_empty());
        assert_eq!(space.active_id(), None);
        space.verify_invariants();
    }

    #[test]
    fn hit_testing_returns_the_topmost() {
        let mut space: IslandSpace<u32> = IslandSpace::new();
        let under = space.add(island_at(0., 0., 200., 200., IslandLayout::Single));
        let over = space.add(island_at(50., 50., 200., 200., IslandLayout::Single));

        // Overlapping region: the later addition is on top.
        let hit = space.island_at(Point::from((100., 100.))).unwrap().id();
        assert_eq!(hit, over);

        // Only the lower one covers this.
        let hit = space.island_at(Point::from((10., 10.))).unwrap().id();
        assert_eq!(hit, under);

        assert!(space.island_at(Point::from((9999., 9999.))).is_none());
        space.verify_invariants();
    }

    #[test]
    fn neighbour_uses_the_spatial_search() {
        let (mut space, [a, b, c]) = space_with_three();
        space.set_active(a);

        assert_eq!(space.neighbour(Direction::Right), Some(b));
        assert_eq!(space.neighbour(Direction::Down), Some(c));
        assert_eq!(space.neighbour(Direction::Left), None);
        assert_eq!(space.neighbour(Direction::Up), None);
        space.verify_invariants();
    }

    #[test]
    fn bbox_covers_everything() {
        let (space, _) = space_with_three();
        let bbox = space.bbox().unwrap();
        assert_eq!(bbox.loc, Point::from((0., 0.)));
        assert_eq!(bbox.size, Size::from((600., 600.)));

        let empty: IslandSpace<u32> = IslandSpace::new();
        assert!(empty.bbox().is_none());
    }

    #[test]
    fn items_are_added_beside_the_active_one() {
        let mut island = island_at(0., 0., 900., 300., IslandLayout::Columns);
        island.add(1);
        island.add(2);
        assert_eq!(island.items(), &[1, 2]);

        // Focus the first, then add: the new item lands beside it, not at the end.
        assert!(island.set_active_idx(0));
        island.add(3);
        assert_eq!(island.items(), &[1, 3, 2]);
        assert_eq!(island.active_idx(), Some(1));
        island.verify_invariants();
    }

    #[test]
    fn removing_keeps_the_active_index_sane() {
        let mut island = island_at(0., 0., 900., 300., IslandLayout::Columns);
        for i in 0..4 {
            island.add(i);
        }
        assert_eq!(island.active_idx(), Some(3));

        // Removing something before the active item shifts it down.
        island.remove(0);
        assert_eq!(island.active_idx(), Some(2));
        island.verify_invariants();

        // Removing the last item, while it is active, must not leave the index past the end.
        assert!(island.set_active_idx(2));
        island.remove(2);
        assert_eq!(island.active_idx(), Some(1));
        island.verify_invariants();

        // Emptying it parks the index.
        while island.remove(0).is_some() {}
        assert_eq!(island.active_idx(), None);
        island.verify_invariants();
    }

    #[test]
    fn columns_hit_test_by_position() {
        let mut island = island_at(0., 0., 900., 300., IslandLayout::Columns);
        for i in 0..3 {
            island.add(i);
        }
        // Three 300-wide columns, no gap.
        assert_eq!(island.item_at(Point::from((10., 10.)), 0.), Some(0));
        assert_eq!(island.item_at(Point::from((450., 10.)), 0.), Some(1));
        assert_eq!(island.item_at(Point::from((890., 10.)), 0.), Some(2));
        assert_eq!(island.item_at(Point::from((10., 999.)), 0.), None);
    }

    /// Tabbed islands stack every item in one place, so only the visible one is hittable.
    #[test]
    fn tabbed_only_hits_the_active_item() {
        let mut island = island_at(0., 0., 400., 300., IslandLayout::Tabbed);
        for i in 0..3 {
            island.add(i);
        }
        assert!(island.set_active_idx(1));

        assert_eq!(
            island.item_at(Point::from((200., 150.)), 0.),
            Some(1),
            "a click should reach the visible item, not whichever is first in the list"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Spawn placement
//
// Where a new window goes. The rule has to be stateless and predictable -- "it appears where I
// am looking" is learnable in one use, and every cleverer alternative (remember per-app
// position, auto-arrange, pack into a grid) fails because you cannot predict it, and
// unpredictable placement on an unbounded canvas is hostile.
// ---------------------------------------------------------------------------------------------

/// Where a newly opened window should go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnTarget {
    /// Join an existing island, tiling into it.
    Join(IslandId),
    /// Start a new island. The position is carried separately by [`spawn_target`]'s caller.
    New,
}

/// Everything the placement rule needs to decide.
#[derive(Debug, Clone, Copy)]
pub struct SpawnContext {
    /// Current camera zoom.
    pub zoom: f64,
    /// Where the viewport centre lands on the canvas.
    pub viewport_center: Point<f64, Canvas>,
    /// The island that had focus, if any.
    pub last_focused: Option<IslandId>,
    /// For a transient (a dialog, a file picker), the island holding its parent.
    pub parent_island: Option<IslandId>,
    /// Whether the camera is deliberately framing something.
    ///
    /// This gates rule 3 below, and it exists because the viewport centre alone is the wrong
    /// evidence for "I am looking at this island". A window opens at the viewport centre, so on
    /// the very next spawn the centre is sitting on top of it -- and rule 3 would join, and the
    /// one after that, until an unbounded canvas has collapsed into a single island. Three
    /// terminals opened in a row have to be three islands.
    ///
    /// What distinguishes meaning it is the camera *holding* that island: camera-maximize and
    /// focus-follow are deliberate acts, an unmoved viewport is not. So when you have framed
    /// your editor and open a terminal, it tiles beside the editor; when you are just looking at
    /// the canvas, you get an island of your own.
    pub camera_is_framing: bool,
    /// Below this zoom, the viewport centre stops being a meaningful place to put things.
    pub zoom_threshold: f64,
    /// How far outside an island still counts as "over" it, in canvas pixels.
    pub snap_margin: f64,
}

impl Default for SpawnContext {
    fn default() -> Self {
        Self {
            zoom: 1.,
            viewport_center: Point::from((0., 0.)),
            last_focused: None,
            parent_island: None,
            camera_is_framing: true,
            zoom_threshold: 0.6,
            snap_margin: 32.,
        }
    }
}

/// Decides where a new window goes.
///
/// The order of the rules is the whole design:
///
/// 1. **A transient joins its parent's island.** This is checked first, ahead of the zoom rule,
///    which is a deliberate departure from the original sketch. A Save dialog must appear with
///    the window that opened it whatever the camera is doing; sending it to the last-focused
///    island because you happened to be zoomed out would detach it from its parent, which is
///    the one thing a dialog must never do.
/// 2. **Zoomed far out, join the last-focused island.** At map scale the viewport centre covers
///    a thousand canvas pixels of nothing, and a new window placed there is an unreachable
///    speck.
/// 3. **Over an island the camera is framing, join it.** With a margin, so "nearly on it"
///    counts. The framing condition is what keeps this from swallowing the whole canvas -- see
///    `SpawnContext::camera_is_framing`.
/// 4. **Otherwise, a new island** where you are looking.
pub fn spawn_target<T>(space: &IslandSpace<T>, ctx: &SpawnContext) -> SpawnTarget {
    // 1. A dialog belongs with its parent, always.
    if let Some(parent) = ctx.parent_island {
        if space.get(parent).is_some() {
            return SpawnTarget::Join(parent);
        }
    }

    // 2. Too far out for "where I am looking" to mean anything.
    if ctx.zoom < ctx.zoom_threshold {
        if let Some(last) = ctx.last_focused {
            if space.get(last).is_some() {
                return SpawnTarget::Join(last);
            }
        }
    }

    // 3. Looking at an island on purpose, grown by the snap margin.
    if ctx.camera_is_framing {
        let m = ctx.snap_margin.max(0.);
        for island in space.islands() {
            let r = island.rect();
            let grown = Rectangle::new(
                Point::from((r.loc.x - m, r.loc.y - m)),
                Size::from((r.size.w + m * 2., r.size.h + m * 2.)),
            );
            if grown.contains(ctx.viewport_center) {
                return SpawnTarget::Join(island.id());
            }
        }
    }

    // 4. Somewhere empty: start fresh.
    SpawnTarget::New
}

/// Finds a spot for a new island near `preferred` that does not sit exactly on another.
///
/// Exact overlap is the thing worth avoiding: open three terminals without moving and all three
/// land on the same point, perfectly stacked, and you drag two apart every single time. A
/// spiral outward is enough -- it keeps the window near where you were looking, which is the
/// whole point, while guaranteeing you can see that a new one appeared.
pub fn free_spot_near<T>(
    space: &IslandSpace<T>,
    preferred: Point<f64, Canvas>,
    size: Size<f64, Logical>,
    step: f64,
) -> Point<f64, Canvas> {
    let step = if step > 0. { step } else { 48. };

    let overlaps_at = |p: Point<f64, Canvas>| {
        let candidate = Rectangle::new(p, Size::from((size.w, size.h)));
        space.islands().any(|i| {
            // Only *near-exact* overlap counts. Islands are expected to overlap on a canvas;
            // what is unusable is one hidden precisely behind another.
            let r = i.rect();
            (r.loc.x - candidate.loc.x).abs() < step && (r.loc.y - candidate.loc.y).abs() < step
        })
    };

    if !overlaps_at(preferred) {
        return preferred;
    }

    // Square spiral: right, down, left, up, growing every two legs.
    let (mut dx, mut dy) = (1i32, 0i32);
    let (mut x, mut y) = (0i32, 0i32);
    let mut leg = 1i32;
    let mut legs_at_len = 0;

    for _ in 0..256 {
        for _ in 0..leg {
            x += dx;
            y += dy;
            let p = Point::from((
                preferred.x + f64::from(x) * step,
                preferred.y + f64::from(y) * step,
            ));
            if !overlaps_at(p) {
                return p;
            }
        }
        // Turn right.
        let (ndx, ndy) = (-dy, dx);
        dx = ndx;
        dy = ndy;
        legs_at_len += 1;
        if legs_at_len == 2 {
            legs_at_len = 0;
            leg += 1;
        }
    }

    // Everything nearby is taken. Better to stack than to refuse to open a window.
    preferred
}

#[cfg(test)]
mod spawn_tests {
    use super::*;

    fn space_with_one_at(x: f64, y: f64) -> (IslandSpace<u32>, IslandId) {
        let mut space = IslandSpace::new();
        let id = space.add(Island::new(
            Point::from((x, y)),
            Size::from((400., 300.)),
            IslandLayout::Single,
        ));
        (space, id)
    }

    #[test]
    fn empty_canvas_makes_a_new_island() {
        let space: IslandSpace<u32> = IslandSpace::new();
        let ctx = SpawnContext::default();
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::New);
    }

    #[test]
    fn looking_at_an_island_joins_it() {
        let (space, id) = space_with_one_at(0., 0.);
        let ctx = SpawnContext {
            viewport_center: Point::from((200., 150.)),
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::Join(id));
    }

    /// The rule that keeps a canvas a canvas: sitting on an island by accident is not the same
    /// as looking at one on purpose.
    #[test]
    fn an_unframed_island_under_the_viewport_is_not_joined() {
        let (space, _) = space_with_one_at(0., 0.);
        let ctx = SpawnContext {
            viewport_center: Point::from((200., 150.)),
            camera_is_framing: false,
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::New);
    }

    /// ...but the zoom rule still outranks it, because a speck on a map is worse.
    #[test]
    fn zoomed_out_still_joins_even_unframed() {
        let (space, id) = space_with_one_at(0., 0.);
        let ctx = SpawnContext {
            zoom: 0.3,
            viewport_center: Point::from((9000., 9000.)),
            last_focused: Some(id),
            camera_is_framing: false,
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::Join(id));
    }

    #[test]
    fn looking_at_empty_canvas_makes_a_new_island() {
        let (space, _) = space_with_one_at(0., 0.);
        let ctx = SpawnContext {
            viewport_center: Point::from((5000., 5000.)),
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::New);
    }

    #[test]
    fn the_snap_margin_counts_as_over_it() {
        let (space, id) = space_with_one_at(0., 0.);
        // 10px outside the right edge, inside the default 32px margin.
        let ctx = SpawnContext {
            viewport_center: Point::from((410., 150.)),
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::Join(id));
    }

    /// At map scale the viewport centre is meaningless, so fall back to the last focus.
    #[test]
    fn zoomed_far_out_joins_the_last_focused_island() {
        let (space, id) = space_with_one_at(0., 0.);
        let ctx = SpawnContext {
            zoom: 0.3,
            viewport_center: Point::from((9000., 9000.)),
            last_focused: Some(id),
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::Join(id));
    }

    /// The rule that matters most: a dialog must never detach from its parent.
    #[test]
    fn a_transient_follows_its_parent_even_when_zoomed_out() {
        let mut space: IslandSpace<u32> = IslandSpace::new();
        let parent = space.add(Island::new(
            Point::from((0., 0.)),
            Size::from((400., 300.)),
            IslandLayout::Single,
        ));
        let other = space.add(Island::new(
            Point::from((2000., 0.)),
            Size::from((400., 300.)),
            IslandLayout::Single,
        ));

        let ctx = SpawnContext {
            // Every other rule points somewhere else: zoomed out, focus elsewhere, looking
            // at empty canvas.
            zoom: 0.2,
            viewport_center: Point::from((9000., 9000.)),
            last_focused: Some(other),
            parent_island: Some(parent),
            ..Default::default()
        };
        assert_eq!(
            spawn_target(&space, &ctx),
            SpawnTarget::Join(parent),
            "a dialog must appear with the window that opened it, whatever the camera is doing"
        );
    }

    #[test]
    fn a_stale_parent_or_focus_falls_through() {
        let (space, id) = space_with_one_at(0., 0.);
        let gone = IslandId::next();

        // Parent island no longer exists: fall through to the remaining rules.
        let ctx = SpawnContext {
            viewport_center: Point::from((200., 150.)),
            parent_island: Some(gone),
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::Join(id));

        // Stale focus while zoomed out: fall through rather than joining nothing.
        let ctx = SpawnContext {
            zoom: 0.2,
            viewport_center: Point::from((9000., 9000.)),
            last_focused: Some(gone),
            ..Default::default()
        };
        assert_eq!(spawn_target(&space, &ctx), SpawnTarget::New);
    }

    #[test]
    fn a_free_spot_is_the_preferred_one_when_nothing_is_there() {
        let space: IslandSpace<u32> = IslandSpace::new();
        let p = Point::from((100., 100.));
        assert_eq!(free_spot_near(&space, p, Size::from((400., 300.)), 48.), p);
    }

    /// Three windows opened without moving must not stack perfectly on each other.
    #[test]
    fn exact_overlap_is_nudged_away() {
        let mut space: IslandSpace<u32> = IslandSpace::new();
        let preferred = Point::<f64, Canvas>::from((100., 100.));
        let size = Size::from((400., 300.));

        let mut placed = Vec::new();
        for _ in 0..3 {
            let p = free_spot_near(&space, preferred, size, 48.);
            placed.push(p);
            space.add(Island::new(p, size, IslandLayout::Single));
        }

        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                let (a, b) = (placed[i], placed[j]);
                assert!(
                    (a.x - b.x).abs() >= 48. || (a.y - b.y).abs() >= 48.,
                    "islands {i} and {j} landed on top of each other at {a:?} / {b:?}"
                );
            }
        }

        // And they should stay near where you were looking, not fly off.
        for p in placed {
            assert!((p.x - preferred.x).abs() < 500.);
            assert!((p.y - preferred.y).abs() < 500.);
        }
    }
}
