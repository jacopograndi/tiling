use crate::*;

pub struct Ui<'a> {
    pub tile_commands: &'a mut RenderTileCommands,
    pub input: &'a Input,
    pub ui_defaults: &'a UiDefaults,
}

impl<'a> Ui<'a> {
    pub fn new(
        tile_commands: &'a mut RenderTileCommands,
        input: &'a Input,
        ui_defaults: &'a UiDefaults,
    ) -> Ui<'a> {
        Ui {
            tile_commands,
            input,
            ui_defaults,
        }
    }

    pub fn vertical<const LEN: usize>(&mut self, rect: Rect, weights: &[f32]) -> [Rect; LEN] {
        assert_eq!(weights.len(), LEN);
        let mut rects = [Rect::default(); LEN];
        rect.slice_vertical_weight_array(&mut rects, weights);
        rects
    }

    pub fn horizontal<const LEN: usize>(&mut self, rect: Rect, weights: &[f32]) -> [Rect; LEN] {
        assert_eq!(weights.len(), LEN);
        let mut rects = [Rect::default(); LEN];
        rect.slice_horizontal_weight_array(&mut rects, weights);
        rects
    }

    pub fn label(&mut self, text: &str, rect: Rect) {
        draw_text(self.tile_commands, text, rect, 0.1, &self.ui_defaults.text);
    }

    pub fn button(&mut self, text: &str, rect: Rect) -> bool {
        match draw_button_text(
            &self.ui_defaults.button,
            &self.ui_defaults.text,
            self.tile_commands,
            text,
            rect,
            0.1,
            self.input,
        ) {
            UiInteraction::Clicked { just } => just,
            _ => false,
        }
    }
}

// ascii ordering
#[rustfmt::skip]
const LITTLEFONT_KERNING: [u8; 128] = [
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,4,2,0,0,0,0,4,3,3,0,0,4,2,4,1,0,0,0,0,0,0,0,0,0,0,4,4,2,0,2,0,
    0,0,0,0,0,0,0,0,0,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3,1,3,0,0,
    3,0,0,0,0,0,1,0,0,4,2,0,4,0,0,0,0,0,0,0,1,0,0,0,0,0,0,3,4,3,0,0
];

#[derive(Clone, Debug)]
pub struct UiDefaults {
    pub text: UiText,
    pub button: UiButton,
}

impl UiDefaults {
    pub fn new(handles: &Handles, engine: &EngineContext) -> Option<Self> {
        let font_image = engine.assets.images.get(&handles.font)?;
        Some(UiDefaults {
            text: UiText {
                image_font_size: UVec2::new(font_image.width, font_image.height).as_vec2(),
                image_font_id: handles.font.clone(),
                image_font_char_size: Vec2::new(6., 12.),
                image_font_kerning: LITTLEFONT_KERNING,
                layout: UiTextLayout::Right,
                char_scale: Vec2::new(5., 5.),
                color: Vec4::splat(1.),
            },
            button: UiButton {
                padding: 3.,
                color_normal: Vec4::new(0.3, 0.2, 0.2, 1.0),
                color_hover: Vec4::new(0.5, 0.2, 0.2, 1.0),
                color_pressed: Vec4::new(0.8, 0.4, 0.0, 1.0),
                color_just_pressed: Vec4::new(1.0, 0.5, 0.0, 1.0),
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct UiText {
    pub image_font_size: Vec2,
    pub image_font_id: AssetId,
    pub image_font_char_size: Vec2,
    pub image_font_kerning: [u8; 128],
    pub layout: UiTextLayout,
    pub char_scale: Vec2,
    pub color: Vec4,
}

#[derive(Debug, Clone)]
pub struct UiButton {
    pub padding: f32,
    pub color_normal: Vec4,
    pub color_hover: Vec4,
    pub color_pressed: Vec4,
    pub color_just_pressed: Vec4,
}

#[derive(Clone, Debug)]
pub enum UiTextLayout {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiInteraction {
    None,
    Hovered,
    Clicked { just: bool },
}

pub fn draw_button_text(
    ui_button: &UiButton,
    ui_text: &UiText,
    tile_commands: &mut RenderTileCommands,
    text: &str,
    bounds_rect: Rect,
    z: f32,
    input: &Input,
) -> UiInteraction {
    let padded_rect = bounds_rect.pad(ui_button.padding);
    let drawn_rect = draw_text(tile_commands, text, padded_rect, z, ui_text);
    let inflated_rect = drawn_rect.pad(-ui_button.padding);
    draw_button(ui_button, tile_commands, input, inflated_rect, z + 0.001)
}

pub fn draw_button(
    ui_button: &UiButton,
    tile_commands: &mut RenderTileCommands,
    input: &Input,
    rect: Rect,
    z: f32,
) -> UiInteraction {
    let mut interaction = if rect.contains_point(&input.mouse_position) {
        if input.mouse_just_pressed.0 {
            UiInteraction::Clicked { just: true }
        } else {
            if input.mouse_pressed.0 {
                UiInteraction::Clicked { just: false }
            } else {
                UiInteraction::Hovered
            }
        }
    } else {
        UiInteraction::None
    };
    for touch in input.just_touched.iter() {
        if rect.contains_point(touch) {
            interaction = UiInteraction::Clicked { just: true }
        }
    }
    tile_commands.draw(RenderTile {
        world_rect: rect,
        color: match interaction {
            UiInteraction::None => ui_button.color_normal,
            UiInteraction::Hovered => ui_button.color_hover,
            UiInteraction::Clicked { just: true } => ui_button.color_just_pressed,
            UiInteraction::Clicked { just: false } => ui_button.color_pressed,
        },
        z,
        ..Default::default()
    });
    interaction
}

pub fn draw_text(
    tile_commands: &mut RenderTileCommands,
    text: &str,
    bounds_rect: Rect,
    z: f32,
    ui_text: &UiText,
) -> Rect {
    let mut tiles: Vec<RenderTile> = vec![];
    let font_char_size = ui_text.image_font_char_size;
    let mut row = -1;
    for line in text.lines().take(100) {
        row += 1;
        let mut kerning: u32 = 0;
        for c in line.chars() {
            // conversion from char to ascii bitmap position
            let (x, y) = (c as u8 % 32, c as u8 / 32);
            let sheet_xy = Vec2::new(x as f32, y as f32);

            let font_image_size = Vec2::new(
                ui_text.image_font_size.x as f32,
                ui_text.image_font_size.y as f32,
            );

            let world_rect = Rect {
                pos: Vec2::new(kerning as f32, row as f32 * font_char_size.y + 1.)
                    * ui_text.char_scale,
                size: font_char_size * ui_text.char_scale,
            };
            let clip_rect = Rect {
                pos: (font_char_size * sheet_xy + Vec2::X) / font_image_size,
                size: (font_char_size - Vec2::new(1., 0.)) / font_image_size,
            };
            tiles.push(RenderTile {
                world_rect,
                clip_rect,
                z,
                color: ui_text.color,
            });
            kerning += 5 - ui_text.image_font_kerning[c as usize] as u32 + 2;
        }
    }

    let mut drawn_rect = get_drawn_rect(tiles.as_slice());

    if drawn_rect.size.x > bounds_rect.size.x || drawn_rect.size.y > bounds_rect.size.y {
        let spill_x = bounds_rect.size.x / drawn_rect.size.x;
        let spill_y = bounds_rect.size.y / drawn_rect.size.y;
        let shrink_factor = spill_x.min(spill_y).max(0.0);
        for tile in &mut tiles {
            tile.world_rect.pos *= shrink_factor;
            tile.world_rect.size *= shrink_factor;
        }
        drawn_rect.size *= shrink_factor;
    }

    drawn_rect.pos += bounds_rect.pos;
    for tile in &mut tiles {
        tile.world_rect.pos += bounds_rect.pos;
    }

    match ui_text.layout {
        UiTextLayout::Left => {
            drawn_rect.pos.x -= drawn_rect.size.x - bounds_rect.size.x;
            for tile in &mut tiles {
                tile.world_rect.pos.x -= drawn_rect.size.x - bounds_rect.size.x;
            }
        }
        UiTextLayout::Center => {
            drawn_rect.pos.x -= drawn_rect.size.x * 0.5 - bounds_rect.size.x * 0.5;
            for tile in &mut tiles {
                tile.world_rect.pos.x -= drawn_rect.size.x * 0.5 - bounds_rect.size.x * 0.5;
            }
        }
        _ => {}
    }

    for tile in tiles {
        tile_commands.draw_textured(tile, ui_text.image_font_id.clone());
    }

    return drawn_rect;
}

pub fn get_drawn_rect(tiles: &[RenderTile]) -> Rect {
    let mut min = Vec2::INFINITY;
    let mut max = Vec2::NEG_INFINITY;
    for tile in tiles.iter() {
        min = min.min(tile.world_rect.pos);
        max = max.max(tile.world_rect.pos + tile.world_rect.size);
    }
    Rect::new(min, max - min)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub pos: Vec2,
    pub size: Vec2,
}

impl Rect {
    pub fn new(pos: Vec2, size: Vec2) -> Self {
        Self { pos, size }
    }

    pub fn xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            pos: Vec2::new(x, y),
            size: Vec2::new(w, h),
        }
    }

    pub fn x(x: f32) -> Self {
        Self::xywh(x, 0., 0., 0.)
    }
    pub fn y(y: f32) -> Self {
        Self::xywh(0., y, 0., 0.)
    }
    pub fn z(z: f32) -> Self {
        Self::xywh(0., 0., z, 0.)
    }
    pub fn w(w: f32) -> Self {
        Self::xywh(0., 0., 0., w)
    }

    pub fn contains_point(&self, xy: &Vec2) -> bool {
        let x = self.pos.x < xy.x && xy.x < self.pos.x + self.size.x;
        let y = self.pos.y < xy.y && xy.y < self.pos.y + self.size.y;
        x && y
    }

    pub fn pad(&self, pad: f32) -> Self {
        Self {
            pos: self.pos + Vec2::splat(pad),
            size: self.size - Vec2::splat(pad) * 2.,
        }
    }

    pub fn pad_rect(&self, rect: Rect) -> Self {
        Self {
            pos: self.pos + rect.pos,
            size: self.size - rect.size - rect.pos,
        }
    }

    pub fn slice_vertical_in_twain(&self, amt: f32) -> (Rect, Rect) {
        (
            Rect::new(
                Vec2::new(self.pos.x, self.pos.y),
                Vec2::new(self.size.x, amt),
            ),
            Rect::new(
                Vec2::new(self.pos.x, self.pos.y + amt),
                Vec2::new(self.size.x, self.size.y - amt),
            ),
        )
    }
    pub fn slice_vertical_in_twain_weight(&self, amt: f32) -> (Rect, Rect) {
        self.slice_vertical_in_twain(self.size.y * amt)
    }

    pub fn slice_horizontal_in_twain(&self, amt: f32) -> (Rect, Rect) {
        (
            Rect::new(
                Vec2::new(self.pos.x, self.pos.y),
                Vec2::new(amt, self.size.y),
            ),
            Rect::new(
                Vec2::new(self.pos.x + amt, self.pos.y),
                Vec2::new(self.size.x - amt, self.size.y),
            ),
        )
    }
    pub fn slice_horizontal_in_twain_weight(&self, amt: f32) -> (Rect, Rect) {
        self.slice_horizontal_in_twain(self.size.x * amt)
    }

    pub fn slice_vertical_array(&self, rects: &mut [Rect]) {
        if rects.is_empty() {
            return;
        }
        let fragment_size = Vec2::new(self.size.x, self.size.y / rects.len() as f32);
        for (y, rect) in rects.into_iter().enumerate() {
            rect.pos = self.pos + fragment_size * Vec2::Y * y as f32;
            rect.size = fragment_size;
        }
    }

    pub fn slice_horizontal_array(&self, rects: &mut [Rect]) {
        if rects.is_empty() {
            return;
        }
        let fragment_size = Vec2::new(self.size.x / rects.len() as f32, self.size.y);
        for (x, rect) in rects.into_iter().enumerate() {
            rect.pos = self.pos + fragment_size * Vec2::X * x as f32;
            rect.size = fragment_size;
        }
    }

    pub fn slice_vertical_weight_array(&self, rects: &mut [Rect], weights: &[f32]) {
        if rects.is_empty() {
            return;
        }
        assert_eq!(rects.len(), weights.len());
        let sum: f32 = weights.iter().sum();
        let mut partial_sum: f32 = 0.0;
        for (y, rect) in rects.into_iter().enumerate() {
            let fragment_size = Vec2::new(self.size.x, self.size.y * (weights[y] / sum));
            rect.pos = self.pos + Vec2::Y * partial_sum;
            rect.size = fragment_size;
            partial_sum += fragment_size.y;
        }
    }

    pub fn slice_horizontal_weight_array(&self, rects: &mut [Rect], weights: &[f32]) {
        if rects.is_empty() {
            return;
        }
        assert_eq!(rects.len(), weights.len());
        let sum: f32 = weights.iter().sum();
        let mut partial_sum: f32 = 0.0;
        for (x, rect) in rects.into_iter().enumerate() {
            let fragment_size = Vec2::new(self.size.x * (weights[x] / sum), self.size.y);
            rect.pos = self.pos + Vec2::X * partial_sum;
            rect.size = fragment_size;
            partial_sum += fragment_size.x;
        }
    }

    pub fn slice_vertical(&self, num: usize) -> Vec<Rect> {
        let mut rects = vec![Rect::default(); num];
        self.slice_vertical_array(&mut rects);
        rects
    }

    pub fn slice_horizontal(&self, num: usize) -> Vec<Rect> {
        let mut rects = vec![Rect::default(); num];
        self.slice_horizontal_array(&mut rects);
        rects
    }

    pub fn slice_vertical_weight(&self, weights: &[f32]) -> Vec<Rect> {
        let mut rects = vec![Rect::default(); weights.len()];
        self.slice_vertical_weight_array(&mut rects, weights);
        rects
    }

    pub fn slice_horizontal_weight(&self, weights: &[f32]) -> Vec<Rect> {
        let mut rects = vec![Rect::default(); weights.len()];
        self.slice_horizontal_weight_array(&mut rects, weights);
        rects
    }
}
