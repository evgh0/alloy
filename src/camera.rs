use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram};

pub struct CameraUniform {
    buffer: WebGlBuffer,
    position: [f32; 4],
    target: [f32; 4],
    up: [f32; 4],
    projection: [f32; 4],
}

impl CameraUniform {
    pub fn new(gl: &GL, program: &WebGlProgram) -> Result<Self, JsValue> {
        let buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create camera buffer"))?;

        gl.bind_buffer(GL::UNIFORM_BUFFER, Some(&buffer));

        // Packed std140 layout for the shader's `Camera` block:
        // - position: vec4
        // - target: vec4
        // - up: vec4
        // - projection: vec4(fovy, aspect, near, far)
        let camera_data: [f32; 16] = [
            10.0, 0.0, 50.5, 1.0, // position
            0.0, 0.0, 0.0, 1.0, // target + padding
            0.0, 1.0, 0.0, 0.0, // up + padding
            std::f32::consts::FRAC_PI_3, // fovy (60 deg)
            1.0, // aspect
            0.1, // near
            100.0, // far
        ];

        let camera_array = js_sys::Float32Array::from(camera_data.as_slice());
        gl.buffer_data_with_array_buffer_view(GL::UNIFORM_BUFFER, &camera_array, GL::STATIC_DRAW);

        let camera_block_index = gl.get_uniform_block_index(program, "Camera");
        gl.uniform_block_binding(program, camera_block_index, 0);
        gl.bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&buffer));

        Ok(Self {
            buffer,
            position: [0.0, 0.0, 10.5, 1.0],
            target: [0.0, 0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0, 0.0],
            projection: [std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0],
        })
    }

    pub fn update_aspect(&self, gl: &GL, aspect: f32) {
        gl.bind_buffer(GL::UNIFORM_BUFFER, Some(&self.buffer));

        let camera_data: [f32; 16] = [
            self.position[0], self.position[1], self.position[2], self.position[3],
            self.target[0], self.target[1], self.target[2], self.target[3],
            self.up[0], self.up[1], self.up[2], self.up[3],
            self.projection[0], aspect, self.projection[2], self.projection[3],
        ];

        let camera_array = js_sys::Float32Array::from(camera_data.as_slice());
        gl.buffer_data_with_array_buffer_view(GL::UNIFORM_BUFFER, &camera_array, GL::STATIC_DRAW);
    }
}

