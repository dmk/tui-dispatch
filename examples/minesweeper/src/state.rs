use rand::seq::SliceRandom;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Expert,
}

impl Difficulty {
    pub fn params(self) -> (usize, usize, usize) {
        match self {
            Difficulty::Beginner => (9, 9, 10),
            Difficulty::Intermediate => (16, 16, 40),
            Difficulty::Expert => (30, 16, 99),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Beginner => "Beginner",
            Difficulty::Intermediate => "Intermediate",
            Difficulty::Expert => "Expert",
        }
    }
}

#[derive(Clone, Default)]
pub struct Cell {
    pub mine: bool,
    pub revealed: bool,
    pub flagged: bool,
    pub adjacent: u8,
}

#[derive(Clone, PartialEq)]
pub enum GameState {
    Playing,
    Won,
    Lost,
}

pub struct AppState {
    pub grid: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub game_state: GameState,
    pub difficulty: Difficulty,
    pub mine_count: usize,
    pub flags_placed: usize,
    pub cells_revealed: usize,
    pub total_safe: usize,
}

impl AppState {
    pub fn new(difficulty: Difficulty) -> Self {
        let (width, height, mines) = difficulty.params();
        let mut grid = vec![vec![Cell::default(); width]; height];

        // Place mines randomly
        let mut positions: Vec<(usize, usize)> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .collect();
        positions.shuffle(&mut rand::rng());

        for &(x, y) in positions.iter().take(mines) {
            grid[y][x].mine = true;
        }

        // Calculate adjacent mine counts
        for y in 0..height {
            for x in 0..width {
                if grid[y][x].mine {
                    continue;
                }
                grid[y][x].adjacent = count_adjacent_mines(&grid, x, y, width, height);
            }
        }

        Self {
            grid,
            width,
            height,
            cursor_x: width / 2,
            cursor_y: height / 2,
            game_state: GameState::Playing,
            difficulty,
            mine_count: mines,
            flags_placed: 0,
            cells_revealed: 0,
            total_safe: width * height - mines,
        }
    }
}

fn count_adjacent_mines(grid: &[Vec<Cell>], x: usize, y: usize, width: usize, height: usize) -> u8 {
    neighbors(x, y, width, height)
        .iter()
        .filter(|&&(nx, ny)| grid[ny][nx].mine)
        .count() as u8
}

/// Return valid neighbor coordinates within the grid bounds.
pub fn neighbors(x: usize, y: usize, width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(8);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                result.push((nx as usize, ny as usize));
            }
        }
    }
    result
}
