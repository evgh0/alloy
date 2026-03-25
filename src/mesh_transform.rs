use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram};

pub struct MeshTransformUniform {
    buffer: WebGlBuffer,
    position: [f32; 4],
    scale: [f32; 4],
}

impl MeshTransformUniform {
    pub fn new(gl: &GL, program: &WebGlProgram) -> Result<Self, JsValue> {
        let buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create mesh transform buffer"))?;

        gl.bind_buffer(GL::UNIFORM_BUFFER, Some(&buffer));

        // Packed std140 layout for the shader's `MeshTransform` block:
        // vec4 position;
        // vec4 scale;
        // vec4 rotation;
        let mesh_data: [f32; 12] = [
            0.0, 0.0, 0.0, 1.0, // position
            0.75, 0.75, 0.75, 0.0, // scale
            0.0, 0.0, 0.0, 1.0, // rotation (identity quaternion)
        ];

        let mesh_array = js_sys::Float32Array::from(mesh_data.as_slice());
        gl.buffer_data_with_array_buffer_view(GL::UNIFORM_BUFFER, &mesh_array, GL::DYNAMIC_DRAW);

        let mesh_block_index = gl.get_uniform_block_index(program, "MeshTransform");
        gl.uniform_block_binding(program, mesh_block_index, 1);
        gl.bind_buffer_base(GL::UNIFORM_BUFFER, 1, Some(&buffer));

        Ok(Self {
            buffer,
            position: [0.0, 0.0, 0.0, 1.0],
            scale: [0.75, 0.75, 0.75, 0.0],
        })
    }

    pub fn update_rotation(&self, gl: &GL, rotation: [f32; 4]) {
        gl.bind_buffer(GL::UNIFORM_BUFFER, Some(&self.buffer));

        let mesh_data: [f32; 12] = [
            self.position[0], self.position[1], self.position[2], self.position[3],
            self.scale[0], self.scale[1], self.scale[2], self.scale[3],
            rotation[0], rotation[1], rotation[2], rotation[3],
        ];

        let mesh_array = js_sys::Float32Array::from(mesh_data.as_slice());
        gl.buffer_data_with_array_buffer_view(GL::UNIFORM_BUFFER, &mesh_array, GL::DYNAMIC_DRAW);
    }
}

