use crate::{ui::*, *};

pub struct GameState {
    rand: RandLCG,
    ui_defaults: Option<UiDefaults>,
    board: Board,
    hand: Option<(usize, usize)>,
    available_tiles: Vec<KripkeTile>,
    restart: bool,
    grid_size: IVec2,
    win_timer: Option<f64>,
}

#[derive(Clone)]
pub struct Board {
    grid_tiles: Vec<KripkeTile>,
    grid_size: IVec2,
}

impl Board {
    fn randomized(
        rand: &mut RandLCG,
        size: IVec2,
        available_tiles: &Vec<KripkeTile>,
    ) -> Option<Self> {
        let mut board = Self {
            grid_tiles: vec![available_tiles[0].clone(); (size.x * size.y) as usize],
            grid_size: size,
        };

        for _ in 0..1000 {
            if board.construct(rand, available_tiles) {
                return Some(board);
            }
        }
        None
    }

    fn construct(&mut self, rand: &mut RandLCG, available_tiles: &Vec<KripkeTile>) -> bool {
        let mut sparse_board = SparseBoard {
            tiles: vec![None; self.grid_tiles.len()],
            grid_size: self.grid_size,
        };
        let mut matchings: Vec<(usize, usize)> = vec![];
        for y in 0..self.grid_size.y as usize {
            for x in 0..self.grid_size.x as usize {
                // find all possible matching
                // pick one
                let tile_i = self.xy_i(x, y);
                matchings.clear();
                for index in 0..available_tiles.len() {
                    for rot in 0..4 {
                        sparse_board.tiles[tile_i] = Some((index, rot));
                        if sparse_board.is_consistent(available_tiles) {
                            matchings.push((index, rot))
                        }
                        sparse_board.tiles[tile_i] = None;
                    }
                }
                if matchings.is_empty() {
                    return false;
                }
                let choice = rand.next() as usize % matchings.len();
                sparse_board.tiles[tile_i] = Some(matchings[choice]);
            }
        }

        // convert to Board
        // shuffle and random rotation
        self.grid_tiles = sparse_board
            .tiles
            .iter()
            .map(|opt| {
                let (index, rotation) = opt.expect("sparse grid is filled");
                available_tiles[index].rotated_left_by(rotation)
            })
            .collect();
        assert!(self.is_solved());
        for i in (1..self.grid_tiles.len()).rev() {
            let j = rand.next() as usize % (i + 1);
            let t = self.grid_tiles[i].clone();
            self.grid_tiles[i] = self.grid_tiles[j].clone();
            self.grid_tiles[j] = t;
        }
        for i in 0..self.grid_tiles.len() {
            let rotation = rand.next() as usize % 4;
            self.grid_tiles[i] = self.grid_tiles[i].rotated_left_by(rotation);
        }

        true
    }

    fn is_solvable(&self) -> bool {
        // PERF: this sucks

        // indices are referring to these tiles
        let reference_tiles = &self.grid_tiles;
        let mut branches: Vec<SparseBoard> = vec![];
        branches.push(SparseBoard {
            tiles: vec![None; self.grid_tiles.len()],
            grid_size: self.grid_size,
        });
        for _ in 0..10000 {
            if branches.is_empty() {
                return false;
            }
            let mut current = branches.pop().unwrap();
            // if current is filled -> true
            // if not, branch
            let missing_indices = current.missing_indices(&reference_tiles);
            if missing_indices.is_empty() {
                return true;
            }
            for index in missing_indices {
                // place the tile index so that it satisfies the sides requirements.
                // if it can't, drop this branch
                for y in 0..self.grid_size.y as usize {
                    for x in 0..self.grid_size.x as usize {
                        let i = self.xy_i(x, y);
                        if current.tiles[i].is_none() {
                            for rot in 0..4 {
                                // sides requirements
                                current.tiles[i] = Some((index, rot));
                                if current.is_consistent(reference_tiles) {
                                    let branch = current.clone();
                                    branches.push(branch);
                                }
                                current.tiles[i] = None;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn is_solved(&self) -> bool {
        for y in 0..self.grid_size.y as usize {
            for x in 0..self.grid_size.x as usize {
                for (dir, check) in SIDE_ADJACENT.iter().zip(SIDE_CHECK.iter()) {
                    if !self.contains(IVec2::new(x as i32, y as i32) + *dir) {
                        continue;
                    }
                    let a = self.grid_tiles[self.xy_i(x, y)].sides[check.0];
                    let b = self.grid_tiles
                        [self.xy_i((x as i32 + dir.x) as usize, (y as i32 + dir.y) as usize)]
                    .sides[check.1];
                    if a != b {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn xy_i(&self, x: usize, y: usize) -> usize {
        x + y * self.grid_size.x as usize
    }

    fn contains(&self, IVec2 { x, y }: IVec2) -> bool {
        x >= 0 && x < self.grid_size.x && y >= 0 && y < self.grid_size.y
    }
}

const SIDE_ADJACENT: [IVec2; 4] = [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y];
const SIDE_CHECK: [(usize, usize); 4] = [(0, 2), (1, 3), (2, 0), (3, 1)];

#[derive(Clone)]
struct SparseBoard {
    tiles: Vec<Option<(usize, usize)>>,
    grid_size: IVec2,
}

impl SparseBoard {
    fn missing_indices(&self, reference_tiles: &Vec<KripkeTile>) -> Vec<usize> {
        (0..reference_tiles.len())
            .filter_map(|index| {
                if !self.tiles.iter().any(|o| match o {
                    Some((used_index, _)) if *used_index == index => true,
                    _ => false,
                }) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn is_consistent(&self, reference_tiles: &Vec<KripkeTile>) -> bool {
        for y in 0..self.grid_size.y as usize {
            for x in 0..self.grid_size.x as usize {
                let Some(tile_i) = self.tiles[self.xy_i(x, y)] else {
                    continue;
                };
                for (dir, check) in SIDE_ADJACENT.iter().zip(SIDE_CHECK.iter()) {
                    if !self.contains(IVec2::new(x as i32, y as i32) + *dir) {
                        continue;
                    }
                    if let Some(oth_i) = self.tiles
                        [self.xy_i((x as i32 + dir.x) as usize, (y as i32 + dir.y) as usize)]
                    {
                        let a = reference_tiles[tile_i.0].rotated_left_by(tile_i.1).sides[check.0];
                        let b = reference_tiles[oth_i.0].rotated_left_by(oth_i.1).sides[check.1];
                        if a != b {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn xy_i(&self, x: usize, y: usize) -> usize {
        x + y * self.grid_size.x as usize
    }

    fn contains(&self, IVec2 { x, y }: IVec2) -> bool {
        x >= 0 && x < self.grid_size.x && y >= 0 && y < self.grid_size.y
    }
}

const TILES: [(&'static str, [u8; 4]); 17] = [
    ("tile_0000.obj", [0, 0, 0, 0]),
    ("tile_0020.obj", [0, 0, 0, 2]),
    ("tile_1000.obj", [0, 1, 0, 0]),
    ("tile_1100.obj", [1, 1, 0, 0]),
    ("tile_0202.obj", [2, 0, 2, 0]),
    ("tile_1111.obj", [1, 1, 1, 1]),
    ("tile_1110.obj", [1, 1, 0, 1]),
    ("tile_1112.obj", [1, 1, 2, 1]),
    ("tile_2211.obj", [1, 1, 2, 2]),
    ("tile_1010.obj", [1, 0, 1, 0]),
    ("tile_1022.obj", [0, 1, 2, 2]),
    ("tile_1202.obj", [2, 1, 2, 0]),
    ("tile_1220.obj", [2, 1, 0, 2]),
    ("tile_1222.obj", [2, 1, 2, 2]),
    ("tile_0022.obj", [0, 0, 2, 2]),
    ("tile_0222.obj", [2, 0, 2, 2]),
    ("tile_2222.obj", [2, 2, 2, 2]),
];

pub struct Handles {
    pub font: AssetId,
    pub tiles: Vec<AssetId>,
    pub tiles_atlas: AssetId,
    pub base: AssetId,
    pub selector: AssetId,
}

impl Handles {
    fn request_load(engine: &mut EngineContext) -> Option<Handles> {
        let tiles: Vec<AssetId> = TILES
            .iter()
            .filter_map(|(asset_name, _)| engine.assets.request_id(asset_name.to_string()))
            .collect();
        if tiles.len() != TILES.len() {
            return None;
        }
        Some(Handles {
            font: engine.assets.request_id("littlefont.png".to_string())?,
            tiles,
            tiles_atlas: engine.assets.request_id("tiles_atlas.png".to_string())?,
            base: engine.assets.request_id("base.obj".to_string())?,
            selector: engine.assets.request_id("selector.obj".to_string())?,
        })
    }
}

impl GameState {
    pub fn new() -> Self {
        let seed = (miniquad::date::now() * 1000000.) as u128;
        let rand = RandLCG { seed };

        Self {
            rand,
            ui_defaults: None,
            board: Board {
                grid_tiles: Vec::new(),
                grid_size: IVec2::ZERO,
            },
            hand: None,
            available_tiles: Vec::new(),
            restart: true,
            grid_size: IVec2::splat(3),
            win_timer: None,
        }
    }

    pub fn update<'a>(&'a mut self, engine: &'a mut EngineContext<'a>) {
        let Some(handles) = Handles::request_load(engine) else {
            return;
        };

        let Some(mut ui_defaults) = UiDefaults::new(&handles, engine) else {
            return;
        };

        if self.available_tiles.is_empty() {
            // load tiles
            assert_eq!(TILES.len(), handles.tiles.len());
            for (i, tile) in TILES.iter().enumerate() {
                self.available_tiles.push(KripkeTile {
                    sides: tile.1,
                    rotation: 0,
                    asset_id: handles.tiles[i].clone(),
                });
            }
        }

        let camera_mode = CameraMode::Perspective {
            fov: f32::to_radians(60.),
            near: 0.01,
            far: 100.,
        };
        let camera_normal = Vec3::new(0., 0., 1.).normalize();
        let camera_distance = {
            let resolution_ratio = engine.resolution.x / engine.resolution.y;
            let longest_grid = self.grid_size.x.max(self.grid_size.y) as f32;
            (1. / resolution_ratio.min(1.)) * longest_grid
        };

        let mut camera_transform = Transform {
            translation: -camera_normal * camera_distance,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        if self.restart {
            let Some(board) =
                Board::randomized(&mut self.rand, self.grid_size, &self.available_tiles)
            else {
                return;
            };
            self.restart = false;
            self.board = board;
            self.hand = None;
        }

        let solved = self.board.is_solved();
        if solved {
            if let Some(ref win_timer) = self.win_timer {
                let duration = engine.current_time - win_timer;
                let keyframe_updown = 2.;
                let keyframe_spin = 1.;
                let keyframe_spin_fullspeed = 10.;
                let speed_updown = 0.2;
                let speed_spin = 3.;
                let mut angle_updown = 0.;
                let mut angle_spin = 0.;
                if duration > keyframe_updown {
                    angle_updown =
                        (-f32::cos((duration - keyframe_updown) as f32 * speed_updown) * 0.5 + 0.5)
                            * 60.;
                }
                if duration > keyframe_spin {
                    let runup = speed_spin * 3.;
                    if duration > keyframe_spin_fullspeed {
                        angle_spin =
                            (duration - keyframe_spin_fullspeed) as f32 * speed_spin + runup;
                    } else {
                        let delta = keyframe_spin_fullspeed - keyframe_spin;
                        let perc = (duration - keyframe_spin) / delta;
                        let easing = perc * perc * perc;
                        angle_spin = easing as f32 * runup;
                    }
                }
                camera_transform.translation = -(camera_normal) * camera_distance;
                camera_transform.rotation = Quat::from_euler(
                    EulerRot::XZY,
                    -angle_updown.to_radians(),
                    angle_spin.to_radians(),
                    0.,
                );
            } else {
                self.win_timer = Some(engine.current_time);
            }
        } else {
            self.win_timer = None;
        }

        engine.renderer.camera = Camera {
            mode: camera_mode,
            view: camera_transform.to_mat4(),
        };

        let mut input_used = false;

        let screen_rect = Rect::new(Vec2::ZERO, *engine.resolution);
        if solved {
            ui_defaults.text.layout = UiTextLayout::Center;
            let mut ui = Ui::new(engine.tile_commands, engine.input, &ui_defaults);
            let [header, _, footer] = ui.vertical(screen_rect, &[1., 4., 1.]);
            let [_, h1, h2] = ui.vertical(header, &[1., 1., 1.]);
            ui.label("All matched!", h1);
            if ui.button("Restart", h2) {
                self.restart = true;
                input_used = true;
            }

            let [f1, f2, _] = ui.vertical(footer, &[1., 1., 1.]);
            ui.label(format!("Size: {}", self.grid_size).as_str(), f1);
            let [_, x, y, nx, ny, _] = ui.horizontal(f2, &[2., 1., 1., 1., 1., 2.]);
            if ui.button("x++", x) {
                self.grid_size.x = 10.min(self.grid_size.x + 1);
                input_used = true;
            }
            if ui.button("y++", y) {
                self.grid_size.y = 10.min(self.grid_size.y + 1);
                input_used = true;
            }
            if ui.button("x--", nx) {
                self.grid_size.x = 1.max(self.grid_size.x - 1);
                input_used = true;
            }
            if ui.button("y--", ny) {
                self.grid_size.y = 1.max(self.grid_size.y - 1);
                input_used = true;
            }
        }

        let mut rays = vec![];
        if !input_used {
            if engine.input.mouse_just_pressed.0 {
                rays.push(
                    engine
                        .renderer
                        .camera
                        .ray_from_cursor(&engine.input.mouse_position, engine.resolution),
                );
            }
            for touch in engine.input.just_touched.iter() {
                rays.push(
                    engine
                        .renderer
                        .camera
                        .ray_from_cursor(touch, engine.resolution),
                );
            }
        }

        for y in 0..self.board.grid_size.y as usize {
            for x in 0..self.board.grid_size.x as usize {
                let selected = match &self.hand {
                    Some(hand) if *hand == (x, y) => true,
                    _ => false,
                };

                let pos = Vec2::new(x as f32, y as f32);
                let size = self.board.grid_size.as_vec2();

                let origin =
                    Vec3::new(pos.x - (size.x - 1.) * 0.5, pos.y - (size.y - 1.) * 0.5, 0.);

                let kripke_tile = &self.board.grid_tiles[self.board.xy_i(x, y)];
                let rot = kripke_tile.rotation as f32 * 90.;

                let padding = if solved { 0.502 } else { 0.47 };

                engine.mesh_commands.draw(RenderMesh {
                    mesh_id: kripke_tile.asset_id.clone(),
                    transform: Transform {
                        scale: Vec3::ONE * padding,
                        translation: origin,
                        rotation: Quat::from_euler(
                            EulerRot::XYZ,
                            f32::to_radians(90.),
                            f32::to_radians(rot),
                            0.,
                        ),
                    },
                    color: Vec4::new(1., 1., 1., 1.),
                    image_id: Some(handles.tiles_atlas.clone()),
                });

                engine.mesh_commands.draw(RenderMesh {
                    mesh_id: handles.base.clone(),
                    transform: Transform {
                        scale: Vec3::ONE * padding,
                        translation: origin,
                        rotation: Quat::from_euler(
                            EulerRot::XYZ,
                            f32::to_radians(90.),
                            f32::to_radians(rot),
                            0.,
                        ),
                    },
                    color: Vec4::new(0.2, 0.2, 0.2, 1.),
                    image_id: Some(handles.tiles_atlas.clone()),
                });

                if selected {
                    engine.mesh_commands.draw(RenderMesh {
                        mesh_id: handles.selector.clone(),
                        transform: Transform {
                            scale: Vec3::ONE * padding,
                            translation: origin,
                            rotation: Quat::from_euler(
                                EulerRot::XYZ,
                                f32::to_radians(90.),
                                f32::to_radians(rot),
                                0.,
                            ),
                        },
                        color: Vec4::new(1., 1., 1., 1.),
                        image_id: None,
                    });
                }

                let (quad_origin, quad_axis_x, quad_axis_y) = (origin, Vec3::X, Vec3::Y);
                let raycast_intersection = rays.iter().any(|(ray_pos, ray_dir)| {
                    ray_rect_intersect(*ray_pos, *ray_dir, quad_origin, quad_axis_x, quad_axis_y)
                });

                if raycast_intersection && !solved {
                    if let Some((hand_x, hand_y)) = self.hand.take() {
                        if hand_x == x && hand_y == y {
                            // rotate
                            let i = self.board.xy_i(x, y);
                            self.board.grid_tiles[i].rotate_left();
                        } else {
                            // swap
                            let temp_i = self.board.xy_i(x, y);
                            let hand_i = self.board.xy_i(hand_x, hand_y);
                            let temp = self.board.grid_tiles[temp_i].clone();
                            self.board.grid_tiles[temp_i] = self.board.grid_tiles[hand_i].clone();
                            self.board.grid_tiles[hand_i] = temp;
                        }
                    } else {
                        self.hand = Some((x, y));
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct KripkeTile {
    sides: [u8; 4],
    rotation: usize,
    asset_id: AssetId,
}

impl KripkeTile {
    fn rotate_left(&mut self) {
        self.sides.rotate_right(1);
        self.rotation += 1;
        self.rotation %= 4;
    }
    fn rotated_left_by(&self, rotate: usize) -> Self {
        let mut rotated = self.clone();
        for _ in 0..rotate {
            rotated.rotate_left();
        }
        rotated
    }
}

/// Simple random generator
struct RandLCG {
    seed: u128,
}

impl RandLCG {
    // https://en.wikipedia.org/wiki/Linear_congruential_generator#Parameters_in_common_use
    const INCREMENT_C: u128 = 1;
    const MULTIPLIER_A: u128 = 6364136223846793005;
    const MODULUS_M: u128 = 18446744073709551616;

    fn next(&mut self) -> u32 {
        self.seed = (Self::MULTIPLIER_A * self.seed + Self::INCREMENT_C) % Self::MODULUS_M;
        (self.seed >> 32) as u32
    }
}

struct Timer {
    start_time: f64,
    end_time: f64,
}

impl Timer {
    fn from_range(start_time: f64, end_time: f64) -> Self {
        Self {
            start_time,
            end_time,
        }
    }
    fn from_duration(current_time: f64, duration: f64) -> Self {
        Self {
            start_time: current_time,
            end_time: current_time + duration,
        }
    }
    fn percent(&self, current_time: f64) -> f64 {
        (current_time - self.start_time) / (self.end_time - self.start_time)
    }
    fn contains(&self, current_time: f64) -> bool {
        current_time <= self.end_time && current_time >= self.start_time
    }
    fn is_finished(&self, current_time: f64) -> bool {
        current_time >= self.end_time
    }
}

fn distance_ray_plane(ray_pos: Vec3, ray_dir: Vec3, plane: Vec4) -> f32 {
    return -(Vec3::dot(ray_pos, plane.truncate()) + plane.w)
        / Vec3::dot(ray_dir, plane.truncate());
}

fn plane_from_quad(quad_pos: Vec3, quad_axis_x: Vec3, quad_axis_y: Vec3) -> Vec4 {
    assert!(
        quad_axis_x.dot(quad_axis_y) < f32::EPSILON,
        "quad axis are not orthogonal"
    );
    let normal = Vec3::cross(quad_axis_x, quad_axis_y).normalize();
    let distance = Vec3::dot(normal, quad_pos);
    return normal.extend(distance);
}

fn ray_rect_intersect(
    ray_pos: Vec3,
    ray_dir: Vec3,
    quad_pos: Vec3,
    quad_axis_x: Vec3,
    quad_axis_y: Vec3,
) -> bool {
    let plane = plane_from_quad(quad_pos, quad_axis_x, quad_axis_y);
    let distance = distance_ray_plane(ray_pos, ray_dir, plane);
    if !distance.is_finite() {
        return false;
    }
    let intersection = ray_pos + ray_dir * distance;
    let intersection_planar = intersection - quad_pos;
    let projected_on_x = Vec3::dot(intersection_planar, quad_axis_x.normalize());
    let projected_on_y = Vec3::dot(intersection_planar, quad_axis_y.normalize());
    let planar = Vec2::new(projected_on_x, projected_on_y);
    let size = Vec2::new(quad_axis_x.length(), quad_axis_y.length());
    Rect::new(-size * 0.5, size).contains_point(&planar)
}
