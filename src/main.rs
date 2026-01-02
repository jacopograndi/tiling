#![allow(dead_code)]
// TODO: remove this when done

use std::collections::HashMap;

use glam::*;
use miniquad::*;
mod quad_snd;

mod assets;
mod gamestate;
mod net;
mod render;
mod ui;

use assets::*;
use gamestate::*;
use net::*;
use render::*;

use crate::quad_snd::{AudioContext, Sound};

fn main() {
    // Load icons
    let icon = load_icon().ok();

    // Miniquad window config
    let conf = conf::Conf {
        window_title: env!("CARGO_PKG_NAME").to_string(),
        sample_count: 16,
        high_dpi: true,
        window_resizable: true,
        icon,
        ..conf::Conf::default()
    };

    // Load asset requests
    let assets = Assets::new();

    // Start the event loop
    miniquad::start(conf, move || Box::new(Engine::new(assets)));
}

fn load_icon() -> Result<conf::Icon, String> {
    let icon_big_bytes = include_bytes!("../assets/icon_64x64.png");
    let icon_medium_bytes = include_bytes!("../assets/icon_32x32.png");
    let icon_small_bytes = include_bytes!("../assets/icon_16x16.png");
    let icon_big = Image::from_png(icon_big_bytes)?;
    let icon_medium = Image::from_png(icon_medium_bytes)?;
    let icon_small = Image::from_png(icon_small_bytes)?;
    let mut icon_big_array = [0; 64 * 64 * 4];
    let mut icon_medium_array = [0; 32 * 32 * 4];
    let mut icon_small_array = [0; 16 * 16 * 4];
    for i in 0..icon_big_array.len() {
        icon_big_array[i] = icon_big.raw[i];
    }
    for i in 0..icon_medium_array.len() {
        icon_medium_array[i] = icon_medium.raw[i];
    }
    for i in 0..icon_small_array.len() {
        icon_small_array[i] = icon_small.raw[i];
    }
    if cfg!(target_arch = "wasm32") {
        // Apparently setting an icon on wasm causes a crash
        Err(format!("Not supported on wasm"))
    } else {
        Ok(conf::Icon {
            small: icon_small_array,
            medium: icon_medium_array,
            big: icon_big_array,
        })
    }
}

struct Input {
    mouse_position: Vec2,
    mouse_frame_delta: Vec2,
    mouse_frame_last: Vec2,
    mouse_pressed: (bool, bool, bool),
    mouse_just_pressed: (bool, bool, bool),
    mouse_wheel: (f32, f32),
    key_pressed: Vec<KeyCode>,
    key_just_pressed: Vec<KeyCode>,
    just_touched: Vec<Vec2>,
}

impl Input {
    fn new() -> Self {
        Self {
            mouse_position: Vec2::ZERO,
            mouse_frame_delta: Vec2::ZERO,
            mouse_frame_last: Vec2::ZERO,
            mouse_pressed: (false, false, false),
            mouse_just_pressed: (false, false, false),
            mouse_wheel: (0., 0.),
            key_pressed: vec![],
            key_just_pressed: vec![],
            just_touched: vec![],
        }
    }

    fn frame_start(&mut self) {
        self.mouse_frame_delta = self.mouse_position - self.mouse_frame_last;
        self.mouse_frame_last = self.mouse_position;
    }

    fn frame_end_reset(&mut self) {
        self.mouse_just_pressed.0 = false;
        self.mouse_just_pressed.1 = false;
        self.mouse_just_pressed.2 = false;
        self.mouse_wheel = (0., 0.);
        self.key_just_pressed.clear();
        self.just_touched.clear();
    }
}

// Platform abstraction (using miniquad for now)
struct Engine {
    ctx: Box<dyn RenderingBackend>,
    renderer: Renderer,
    resolution: Vec2,
    tile_commands: RenderTileCommands,
    mesh_commands: RenderMeshCommands,
    frame_time: Option<f64>,
    assets: Assets,
    input: Input,
    gamestate: GameState,
    audio_ctx: AudioContext,
    sounds: HashMap<AssetId, Sound>,
    server: NetServer,
    client: NetClient,
}

// Passed to the GameState
struct EngineContext<'a> {
    ctx: &'a Box<dyn RenderingBackend>,
    resolution: &'a Vec2,
    renderer: &'a mut Renderer,
    tile_commands: &'a mut RenderTileCommands,
    mesh_commands: &'a mut RenderMeshCommands,
    assets: &'a mut Assets,
    input: &'a mut Input,
    sounds: &'a mut HashMap<AssetId, Sound>,
    current_time: f64,
    delta_time: f64,
    audio_ctx: &'a AudioContext,
    server: &'a mut NetServer,
    client: &'a mut NetClient,
}

impl Engine {
    pub fn new(assets: Assets) -> Engine {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let res = miniquad::window::screen_size();
        let renderer = Renderer::new(&mut ctx, Camera::ui());

        let audio_ctx = AudioContext::new();

        Engine {
            renderer,
            ctx,
            resolution: Vec2::new(res.0, res.1),
            frame_time: None,
            tile_commands: RenderTileCommands::default(),
            mesh_commands: RenderMeshCommands::default(),
            assets,
            input: Input::new(),
            gamestate: GameState::new(),
            audio_ctx,
            sounds: HashMap::new(),
            server: NetServer::new(),
            client: NetClient::new(),
        }
    }
}

impl EventHandler for Engine {
    fn update(&mut self) {
        let current_time = miniquad::date::now();
        let delta_time = if let Some(frame_time) = self.frame_time {
            current_time - frame_time
        } else {
            0.
        };
        self.frame_time = Some(current_time);

        self.input.frame_start();
        self.tile_commands.clear();
        self.mesh_commands.clear();

        let loaded_assets = self.assets.update();

        // Everything that is loaded from disk is immediately loaded to gpu or audio thread
        for id in loaded_assets {
            if let Some(image) = self.assets.images.get(&id) {
                if let Some(path) = self.assets.get_path(&id) {
                    let filter = if path.as_str() == "littlefont.png" {
                        FilterMode::Nearest
                    } else {
                        FilterMode::Linear
                    };
                    self.renderer
                        .check_load_texture(&mut self.ctx, image, &id, filter);
                }
            }
            if let Some(mesh) = self.assets.meshes.get(&id) {
                self.renderer.check_load_mesh(&mut self.ctx, mesh, &id);
            }
            if let Some(audio_pcm) = self.assets.audio_pcm.get(&id) {
                let sound = Sound::load(&self.audio_ctx, &audio_pcm.samples);
                self.sounds.insert(id.clone(), sound);
            }
        }

        let mut engine_context = EngineContext {
            ctx: &mut self.ctx,
            resolution: &mut self.resolution,
            renderer: &mut self.renderer,
            tile_commands: &mut self.tile_commands,
            mesh_commands: &mut self.mesh_commands,
            assets: &mut self.assets,
            input: &mut self.input,
            sounds: &mut self.sounds,
            current_time,
            delta_time,
            audio_ctx: &self.audio_ctx,
            client: &mut self.client,
            server: &mut self.server,
        };

        self.gamestate.update(&mut engine_context);

        self.input.frame_end_reset();
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        self.resolution = Vec2::new(width, height);
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.input.mouse_position = Vec2::new(x, y);
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        match button {
            MouseButton::Left => self.input.mouse_pressed.0 = true,
            MouseButton::Middle => self.input.mouse_pressed.1 = true,
            MouseButton::Right => self.input.mouse_pressed.2 = true,
            MouseButton::Unknown => {}
        }
        self.input.mouse_just_pressed = self.input.mouse_pressed;
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        match button {
            MouseButton::Left => self.input.mouse_pressed.0 = false,
            MouseButton::Middle => self.input.mouse_pressed.1 = false,
            MouseButton::Right => self.input.mouse_pressed.2 = false,
            MouseButton::Unknown => {}
        }
    }

    fn mouse_wheel_event(&mut self, x: f32, y: f32) {
        // Signum because sometimes it's multiplied by 145(wasm) or 120(windows), yikes
        self.input.mouse_wheel = (x.signum(), y.signum());
    }

    fn key_down_event(&mut self, keycode: KeyCode, _keymods: KeyMods, _repeat: bool) {
        if !self.input.key_pressed.contains(&keycode) {
            self.input.key_pressed.push(keycode);
        }
        if !self.input.key_just_pressed.contains(&keycode) {
            self.input.key_just_pressed.push(keycode);
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        self.input.key_pressed.retain(|k| k != &keycode);
    }

    fn touch_event(&mut self, phase: TouchPhase, _id: u64, x: f32, y: f32) {
        if phase == TouchPhase::Started {
            self.input.just_touched.push(Vec2::new(x, y));
        }
    }

    fn draw(&mut self) {
        self.ctx.begin_default_pass(PassAction::Clear {
            color: Some((0.0, 0.0, 0.0, 1.)),
            depth: Some(1.),
            stencil: None,
        });

        self.renderer.draw(
            &mut self.ctx,
            &self.tile_commands,
            &self.mesh_commands,
            self.resolution,
        );

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }
}
