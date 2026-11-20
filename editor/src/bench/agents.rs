//! The CPU test: many agents steering round each other on a grid of cells, on one core and on all of them. The same
//! work a crowd, a flock or a fleet of ships asks of the CPU each tick (neighbour lookup, separation, heading).
use std::time::Instant;

const CELL: f32 = 4.0;
const SIDE: f32 = 1000.0;
const STEPS: usize = 30;

#[derive(Clone, Copy, Default)]
struct Agent { p: [f32; 2], v: [f32; 2] }

fn hash(i: usize, k: usize) -> f32 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (k as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    ((x >> 40) as f32) / ((1u64 << 24) as f32)
}

/// Agents sorted into cells: `start[c]..start[c + 1]` of `order` are the agents in cell `c`.
struct Grid { n: usize, start: Vec<u32>, order: Vec<u32> }

impl Grid {
    fn cell(p: [f32; 2], n: usize) -> (usize, usize) { (((p[0] / CELL) as usize).min(n - 1), ((p[1] / CELL) as usize).min(n - 1)) }
    fn build(agents: &[Agent]) -> Grid {
        let n = (SIDE / CELL) as usize;
        let mut start = vec![0u32; n * n + 1];
        for a in agents { let (x, y) = Grid::cell(a.p, n); start[y * n + x + 1] += 1; }
        for i in 0..n * n { start[i + 1] += start[i]; }
        let mut fill = start.clone();
        let mut order = vec![0u32; agents.len()];
        for (i, a) in agents.iter().enumerate() { let (x, y) = Grid::cell(a.p, n); order[fill[y * n + x] as usize] = i as u32; fill[y * n + x] += 1; }
        Grid { n, start, order }
    }
}

/// One tick for agents `first..first + out.len()`: steer toward a wandering heading, push off neighbours within 2 m.
fn tick(agents: &[Agent], grid: &Grid, first: usize, out: &mut [Agent], step: usize) {
    for (k, o) in out.iter_mut().enumerate() {
        let (i, a) = (first + k, agents[first + k]);
        let turn = hash(i, step / 10) * std::f32::consts::TAU;
        let mut v = [turn.cos() * 3.0, turn.sin() * 3.0];
        let (cx, cy) = Grid::cell(a.p, grid.n);
        for y in cy.saturating_sub(1)..=(cy + 1).min(grid.n - 1) {
            for x in cx.saturating_sub(1)..=(cx + 1).min(grid.n - 1) {
                let c = y * grid.n + x;
                for &j in &grid.order[grid.start[c] as usize..grid.start[c + 1] as usize] {
                    let b = agents[j as usize];
                    let (dx, dy) = (a.p[0] - b.p[0], a.p[1] - b.p[1]);
                    let d2 = dx * dx + dy * dy;
                    if j as usize != i && d2 < 4.0 { let w = 4.0 / (d2 + 0.1); v[0] += dx * w; v[1] += dy * w; }
                }
            }
        }
        let v = [a.v[0] * 0.8 + v[0] * 0.2, a.v[1] * 0.8 + v[1] * 0.2];
        *o = Agent { p: [(a.p[0] + v[0] / 20.0).clamp(0.0, SIDE - 0.01), (a.p[1] + v[1] / 20.0).clamp(0.0, SIDE - 0.01)], v };
    }
}

/// Milliseconds per tick for `count` agents on one core and on every core.
pub fn run(count: usize) -> (f64, f64, usize) {
    let start: Vec<Agent> = (0..count).map(|i| Agent { p: [hash(i, 1000) * SIDE, hash(i, 2000) * SIDE], v: [0.0; 2] }).collect();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let time = |threads: usize| {
        let (mut now, mut next) = (start.clone(), start.clone());
        let t = Instant::now();
        for step in 0..STEPS {
            let grid = Grid::build(&now);
            let chunk = now.len().div_ceil(threads).max(1);
            std::thread::scope(|s| {
                for (c, out) in next.chunks_mut(chunk).enumerate() {
                    let (now, grid) = (&now, &grid);
                    if threads == 1 { tick(now, grid, c * chunk, out, step); } else { s.spawn(move || tick(now, grid, c * chunk, out, step)); }
                }
            });
            std::mem::swap(&mut now, &mut next);
        }
        t.elapsed().as_secs_f64() * 1000.0 / STEPS as f64
    };
    (time(1), time(cores), cores)
}

#[cfg(test)]
mod tests {
    #[test]
    fn agents_run_on_one_core_and_on_all_of_them() {
        let (one, all, cores) = super::run(2000);
        assert!(one > 0.0 && all > 0.0 && cores >= 1);
    }
}
