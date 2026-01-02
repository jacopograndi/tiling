use std::{cell::RefCell, collections::HashMap, sync::Arc};

pub mod audio_pcm;
pub mod image;
pub mod mesh;
pub mod zlib_inflate;

pub use audio_pcm::*;
pub use image::*;
pub use mesh::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(pub u64);

const ASSET_FOLDER: &str = "assets/";

/// Loads assets from the local `ASSET_FOLDER`.
/// Provides access to assets via `AssetId`.
/// Assets are parsed into game-ready formats into the `states` value.
/// Raw data is not retained after parsing.
#[derive(Default, Debug)]
pub struct Assets {
    pub states: HashMap<String, AssetState>,
    pub images: HashMap<AssetId, Image>,
    pub meshes: HashMap<AssetId, Mesh>,
    pub audio_pcm: HashMap<AssetId, AudioPcm>,
    pub id_sequential: u64,
}

#[derive(Clone, Debug)]
pub enum AssetState {
    Idle,
    Requested(AssetRequest),
    Loaded(AssetId),
}

/// A pointer to a [u8] that can be shared.
#[derive(Clone, Debug)]
pub struct AssetRequest(Arc<RefCell<Option<Vec<u8>>>>);

impl Assets {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            images: HashMap::new(),
            meshes: HashMap::new(),
            audio_pcm: HashMap::new(),
            id_sequential: 1,
        }
    }

    pub fn from_paths(paths: &[&str]) -> Self {
        let mut new = Self::new();
        new.load(paths);
        new
    }

    /// Request to load the assets from the provided paths.
    pub fn load(&mut self, paths: &[&str]) {
        self.states
            .extend(paths.into_iter().map(|p| (p.to_string(), AssetState::Idle)));
    }

    /// Returns the id if it's loaded.
    /// Request to load a single asset from the provided paths.
    pub fn request_id(&mut self, path: String) -> Option<AssetId> {
        match self.states.entry(path).or_insert(AssetState::Idle) {
            AssetState::Loaded(asset_id) => Some(asset_id.clone()),
            _ => None,
        }
    }

    /// Loads the requested assets
    pub fn update(&mut self) -> Vec<AssetId> {
        let mut loaded: Vec<(String, Vec<u8>)> = vec![];
        for (path, state) in &mut self.states {
            match state {
                AssetState::Idle => {
                    let request = Arc::new(RefCell::new(None));
                    // Save a pointer to the request, which will be inspected every frame
                    *state = AssetState::Requested(AssetRequest(request.clone()));
                    let actual_path = if cfg!(target_os = "android") {
                        // Android expects assets to be in the same folder as the apk
                        path.to_string()
                    } else {
                        format!("{}{}", ASSET_FOLDER, path)
                    };
                    miniquad::fs::load_file(&actual_path.clone(), move |data| {
                        if let Ok(data) = data {
                            *request.borrow_mut() = Some(data);
                        } else {
                            panic!("Failed to load: {}", actual_path);
                        };
                    });
                }
                AssetState::Requested(AssetRequest(request)) => {
                    if request.borrow().is_some() {
                        let data: Vec<u8> = request.borrow_mut().take().unwrap();
                        loaded.push((path.clone(), data));
                    }
                }
                AssetState::Loaded(_) => {}
            }
        }
        let mut loaded_assets = vec![];
        for (path, data) in loaded {
            let id = self.process_asset(&path, &data);
            loaded_assets.push(id.clone());
            self.states.insert(path, AssetState::Loaded(id));
        }
        loaded_assets
    }

    /// Parses the raw asset data into a game-ready format
    pub fn process_asset(&mut self, path: &str, data: &Vec<u8>) -> AssetId {
        let id = AssetId(self.id_sequential);
        self.id_sequential += 1;

        if path.ends_with(".png") {
            let image = Image::from_png(data).unwrap();
            self.images.insert(id.clone(), image);
        }
        if path.ends_with(".obj") {
            let mesh = Mesh::from_obj(data).unwrap();
            self.meshes.insert(id.clone(), mesh);
        }
        if path.ends_with(".wav") {
            let audio_pcm = AudioPcm::from_wav(data).unwrap();
            self.audio_pcm.insert(id.clone(), audio_pcm);
        }

        id
    }

    /// AssetId from the path
    pub fn get_id(&self, path: &str) -> Option<&AssetId> {
        match self.states.get(path)? {
            AssetState::Loaded(asset_id) => Some(asset_id),
            _ => None,
        }
    }

    /// path from AssetId
    pub fn get_path(&self, id: &AssetId) -> Option<&String> {
        self.states.iter().find_map(|(path, state)| match state {
            AssetState::Loaded(asset_id) if asset_id == id => Some(path),
            _ => None,
        })
    }

    /// Image access from the path
    pub fn get_image(&self, path: &str) -> Option<(&Image, &AssetId)> {
        match self.states.get(path)? {
            AssetState::Loaded(asset_id) => Some((self.images.get(&asset_id)?, asset_id)),
            _ => None,
        }
    }

    /// Mesh access from the path
    pub fn get_mesh(&self, path: &str) -> Option<(&Mesh, &AssetId)> {
        match self.states.get(path)? {
            AssetState::Loaded(asset_id) => Some((self.meshes.get(&asset_id)?, asset_id)),
            _ => None,
        }
    }

    /// Sound access from the path
    pub fn get_sound(&self, path: &str) -> Option<(&AudioPcm, &AssetId)> {
        match self.states.get(path)? {
            AssetState::Loaded(asset_id) => Some((self.audio_pcm.get(&asset_id)?, asset_id)),
            _ => None,
        }
    }
}

pub struct ByteDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ByteDecoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    pub fn check_bytes(&mut self, reference: &[u8]) -> Result<(), String> {
        if &self.bytes[(self.cursor)..(self.cursor + reference.len())] != reference {
            return Err(format!(
                "Not a .wav ('{}' bytes missing at {})",
                std::str::from_utf8(reference).unwrap(),
                self.cursor
            ));
        };
        self.cursor += reference.len();
        Ok(())
    }

    pub fn decode_u32_le(&mut self) -> u32 {
        let v = u32::from_le_bytes(
            self.bytes[(self.cursor)..(self.cursor + 4)]
                .try_into()
                .unwrap(),
        );
        self.cursor += 4;
        v
    }
    pub fn decode_u16_le(&mut self) -> u16 {
        let v = u16::from_le_bytes(
            self.bytes[(self.cursor)..(self.cursor + 2)]
                .try_into()
                .unwrap(),
        );
        self.cursor += 2;
        v
    }
    pub fn decode_i16_le(&mut self) -> i16 {
        let v = i16::from_le_bytes(
            self.bytes[(self.cursor)..(self.cursor + 2)]
                .try_into()
                .unwrap(),
        );
        self.cursor += 2;
        v
    }
}
