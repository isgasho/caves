mod grid_size;
mod grid;
mod room;
mod tile_pos;
mod tile_rect;
mod tile;

pub use self::grid_size::*;
pub use self::grid::*;
pub use self::room::*;
pub use self::tile_pos::*;
pub use self::tile_rect::*;
pub use self::tile::*;

use std::fmt;
use std::cmp;

use sdl2::rect::{Rect, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(usize);

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A type that represents the static floor plan of a map
#[derive(Clone, PartialEq)]
pub struct FloorMap {
    grid: TileGrid,
    /// The RoomId is the index into this field
    rooms: Vec<Room>,
    /// The width and height of every tile
    tile_size: u32,
}

impl fmt::Debug for FloorMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only apply the special formatting if {:#?} is used. This is so that assertion output
        // does not look super bad on CI.
        if !f.alternate() {
            // Return the normal debug output
            return f.debug_struct("FloorMap")
                .field("grid", &self.grid)
                .field("rooms", &self.rooms)
                .field("tile_size", &self.tile_size)
                .finish();
        }

        use colored::*;

        for row in self.grid().rows() {
            for tile in row {
                use self::Tile::*;
                write!(f, "{}", match tile {
                    &Floor {room_id, ..} => {
                        match self.room(room_id).room_type() {
                            RoomType::Normal => " ".on_blue(),
                            RoomType::Challenge => " ".on_red(),
                            RoomType::PlayerStart => " ".on_bright_blue(),
                            RoomType::TreasureChamber => " ".on_yellow(),
                        }
                    },
                    Wall {..} => "\u{25a2}".on_black(),
                    Empty => " ".on_black(),
                })?;
                write!(f, "{}", " ".on_black())?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

impl FloorMap {
    /// Create a new FloorMap with the given number of rows and columns
    pub fn new(size: GridSize, tile_size: u32) -> Self {
        FloorMap {
            grid: TileGrid::new(size),
            rooms: Vec::new(),
            tile_size,
        }
    }

    /// Returns the size of each tile on this map
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }

    /// Returns the level boundary in pixels of the current map
    pub fn level_boundary(&self) -> Rect {
        self.grid.dimensions().to_rect(self.tile_size)
    }

    /// Returns the number of rooms on this map
    pub fn nrooms(&self) -> usize {
        self.rooms.len()
    }

    /// Returns an iterator over the rooms in the map and their IDs
    pub fn rooms(&self) -> impl Iterator<Item=(RoomId, &Room)> {
        self.rooms.iter().enumerate().map(|(i, room)| (RoomId(i), room))
    }

    /// Returns the room with the specified room ID
    pub fn room(&self, room_id: RoomId) -> &Room {
        &self.rooms[room_id.0]
    }

    /// Returns the exact area of the room, not just the width * height
    /// Counts the number of tiles within the room area that are actually floor tiles in that room
    pub fn room_exact_area(&self, room_id: RoomId) -> usize {
        self.room(room_id).boundary().tile_positions()
            .filter(|&pos| self.grid().get(pos).is_room_floor(room_id))
            .count()
    }

    /// Returns the room with the specified room ID
    /// Not for use after map generation is complete.
    pub(in super) fn room_mut(&mut self, room_id: RoomId) -> &mut Room {
        &mut self.rooms[room_id.0]
    }

    /// Returns an iterator over mutable references to all of the rooms.
    /// Not for use after map generation is complete.
    pub(in super) fn rooms_mut(&mut self) -> impl Iterator<Item=(RoomId, &mut Room)> {
        self.rooms.iter_mut().enumerate().map(|(i, room)| (RoomId(i), room))
    }

    /// Add a room with the given boundary rectangle to the map.
    /// Rooms should not be added after map generation is complete.
    pub(in super) fn add_room(&mut self, boundary: TileRect) -> RoomId {
        let room = Room::new(boundary);
        self.rooms.push(room);
        RoomId(self.rooms.len() - 1)
    }

    /// Returns a reference to this level's grid of tiles
    pub fn grid(&self) -> &TileGrid {
        &self.grid
    }

    /// Returns a mutable reference to this level's grid of tiles
    pub fn grid_mut(&mut self) -> &mut TileGrid {
        &mut self.grid
    }

    /// Returns the rectangle in world coordinates contained by the given top-left and bottom-right
    /// tiles. The entirity of both corners will be included in the rectangle.
    pub fn tile_rect(&self, top_left: TilePos, bottom_right: TilePos) -> Rect {
        debug_assert!(top_left.row <= bottom_right.row && top_left.col <= bottom_right.col,
            "bug: expected top_left to be above and to the left of bottom right");
        let top_left = top_left.top_left(self.tile_size as i32);
        let bottom_right = bottom_right.bottom_right(self.tile_size as i32);

        Rect::new(
            top_left.x(),
            top_left.y(),
            (bottom_right.x() - top_left.x()) as u32,
            (bottom_right.y() - top_left.y()) as u32,
        )
    }

    /// Finds the tile position on the grid that the given point in world coordinates represents.
    /// Panics if the point is outside of the grid.
    pub fn world_to_tile_pos(&self, point: Point) -> TilePos {
        let x = point.x();
        let y = point.y();

        assert!(x >= 0 && y >= 0, "bug: point was not on the grid");

        let row = y as usize / self.tile_size as usize;
        let col = x as usize / self.tile_size as usize;

        assert!(row < self.grid().rows_len() && col < self.grid().cols_len(),
            "bug: point was not on the grid");

        TilePos {row, col}
    }

    /// Returns the tiles within (or around) the region defined by bounds
    pub fn tiles_within(&self, bounds: Rect) -> impl Iterator<Item=(Point, TilePos, &Tile)> {
        let (pos, size) = self.grid_area_within(bounds);

        self.grid().tile_positions_within(pos, size).map(move |pos| {
            // The position of the tile in world coordinates
            (pos.top_left(self.tile_size as i32), pos, self.grid().get(pos))
        })
    }

    /// Returns the top left tile position and grid size of the area within (or around) the region
    /// defined by the given bounds
    pub fn grid_area_within(&self, bounds: Rect) -> (TilePos, GridSize) {
        // While the caller is allowed to ask for tiles within a boundary Rect that starts at
        // negative coordinates, the top left of the map is defined as (0, 0). That means that we
        // can at most request tiles up to that top left corner. The calls to `max()` here help
        // enforce that by making sure we don't convert a negative number to an unsigned type.
        let x = cmp::max(bounds.x(), 0) as usize;
        let y = cmp::max(bounds.y(), 0) as usize;
        let width = bounds.width() as usize;
        let height = bounds.height() as usize;

        let clamp_row = |row| cmp::min(cmp::max(row, 0), self.grid().rows_len()-1);
        let clamp_col = |col| cmp::min(cmp::max(col, 0), self.grid().cols_len()-1);

        let start_row = clamp_row(y / self.tile_size as usize);
        let start_col = clamp_col(x / self.tile_size as usize);

        let end_row = clamp_row((y + height) / self.tile_size as usize);
        let end_col = clamp_col((x + width) / self.tile_size as usize);

        let rows = end_row - start_row + 1;
        let cols = end_col - start_col + 1;

        (
            TilePos {row: start_row, col: start_col},
            GridSize {rows, cols},
        )
    }
}
