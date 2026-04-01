use wasm_bindgen::prelude::*;

use crate::{canvas, Batcher, Phong, Scene, Skybox};

struct CubeScene {
    time: f32,
}

impl CubeScene {
    fn new() -> Self {
        Self { time: 0.0 }
    }
}

impl Scene for CubeScene {
    fn update(&mut self, time: f32) {
        self.time = time;
    }

    fn draw(&self, batcher: &mut Batcher) {
        let spacing = 1.2_f32;

        for z in -2_i32..=2_i32 {
            for y in -2_i32..=2_i32 {
                for x in -2_i32..=2_i32 {
                    let position = [
                        x as f32 * spacing,
                        y as f32 * spacing,
                        z as f32 * spacing,
                    ];
                    let yaw = ((x + z) as f32) * 0.35 + self.time * 0.0015;
                    let half_yaw = yaw * 0.5;
                    let rotation = [0.0, half_yaw.sin(), 0.0, half_yaw.cos()];

                    batcher.add_cube(position, rotation);
                }
            }
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    canvas()?
        .skybox(Skybox::hdri_from_url("/source/autumn_field_puresky_2k.hdr"))
        .shading(Phong)
        .scene(CubeScene::new())
        .update_frequency(60)
        .enable_logging()
        .start()
}

