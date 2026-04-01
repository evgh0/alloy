use wasm_bindgen::JsValue;
use web_sys::{WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram};

pub struct CameraUniform {
    buffer: WebGlBuffer,
    position: [f32; 4],
    target: [f32; 4],
    up: [f32; 4],
    projection: [f32; 4],
    pivot: [f32; 3],
    pan_offset: [f32; 3],
    yaw: f32,
    pitch: f32,
    distance: f32,
    freeflight: bool,
    orbit_sensitivity: f32,
    pan_sensitivity: f32,
    dolly_sensitivity: f32,
    mouse_look_sensitivity: f32,
    move_speed: f32,
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
        let camera_block_index = gl.get_uniform_block_index(program, "Camera");
        gl.uniform_block_binding(program, camera_block_index, 0);
        gl.bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&buffer));

        let mut camera = Self {
            buffer,
            position: [0.0, 0.0, 10.5, 1.0],
            target: [0.0, 0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0, 0.0],
            projection: [std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0],
            pivot: [0.0, 0.0, 0.0],
            pan_offset: [0.0, 0.0, 0.0],
            yaw: std::f32::consts::PI,
            pitch: 0.0,
            distance: 10.5,
            freeflight: false,
            orbit_sensitivity: 0.005,
            pan_sensitivity: 0.002,
            dolly_sensitivity: 0.0015,
            mouse_look_sensitivity: 0.005,
            move_speed: 6.0,
        };

        camera.update_orbit_from_orientation();
        camera.upload(gl);

        Ok(camera)
    }

    pub fn update_aspect(&mut self, gl: &GL, aspect: f32) {
        self.projection[1] = aspect;
        self.upload(gl);
    }

    pub fn is_freeflight(&self) -> bool {
        self.freeflight
    }

    pub fn set_freeflight(&mut self, freeflight: bool) {
        if self.freeflight == freeflight {
            return;
        }

        if freeflight {
            self.freeflight = true;
            self.update_freeflight_target();
        } else {
            self.freeflight = false;
            self.pivot = [self.target[0], self.target[1], self.target[2]];
            self.distance = vec3_len(vec3_sub(self.target_xyz(), self.position_xyz())).max(0.1);
            self.update_orbit_from_orientation();
        }
    }

    pub fn toggle_freeflight(&mut self) {
        self.set_freeflight(!self.freeflight);
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * self.orbit_sensitivity;
        self.pitch = clamp(
            self.pitch - dy * self.mouse_look_sensitivity,
            -1.55,
            1.55,
        );
        if self.freeflight {
            self.update_freeflight_target();
        } else {
            self.update_orbit_from_orientation();
        }
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        if self.freeflight {
            return;
        }

        let forward = self.forward();
        let right = vec3_normalize(vec3_cross(forward, self.world_up()));
        let up = vec3_normalize(vec3_cross(right, forward));
        let scale = self.distance.max(0.1) * self.pan_sensitivity;
        let offset = vec3_add(
            vec3_scale(right, -dx * scale),
            vec3_scale(up, dy * scale),
        );

        self.pan_offset = vec3_add(self.pan_offset, offset);
        self.update_orbit_from_orientation();
    }

    pub fn dolly(&mut self, amount: f32) {
        if self.freeflight {
            let forward = self.forward();
            let movement = amount * self.move_speed * 0.2;
            let position = vec3_add(self.position_xyz(), vec3_scale(forward, movement));
            self.position = [position[0], position[1], position[2], 1.0];
            self.update_freeflight_target();
        } else {
            let next = self.distance * (1.0 + amount * self.dolly_sensitivity);
            self.distance = next.clamp(0.25, 500.0);
            self.update_orbit_from_orientation();
        }
    }

    pub fn freeflight_move(&mut self, forward_amount: f32, strafe_amount: f32, vertical_amount: f32, dt_seconds: f32) {
        if !self.freeflight {
            return;
        }

        let forward = self.forward();
        let right = vec3_normalize(vec3_cross(forward, self.world_up()));
        let up = self.world_up();
        let velocity = self.move_speed * dt_seconds;
        let movement = vec3_add(
            vec3_add(vec3_scale(forward, forward_amount * velocity), vec3_scale(right, strafe_amount * velocity)),
            vec3_scale(up, vertical_amount * velocity),
        );

        let position = vec3_add(self.position_xyz(), movement);
        self.position = [position[0], position[1], position[2], 1.0];
        self.update_freeflight_target();
    }

    fn upload(&self, gl: &GL) {
        gl.bind_buffer(GL::UNIFORM_BUFFER, Some(&self.buffer));

        let camera_data: [f32; 16] = [
            self.position[0], self.position[1], self.position[2], self.position[3],
            self.target[0], self.target[1], self.target[2], self.target[3],
            self.up[0], self.up[1], self.up[2], self.up[3],
            self.projection[0], self.projection[1], self.projection[2], self.projection[3],
        ];

        let camera_array = js_sys::Float32Array::from(camera_data.as_slice());
        gl.buffer_data_with_array_buffer_view(GL::UNIFORM_BUFFER, &camera_array, GL::STATIC_DRAW);
    }

    fn update_orbit_from_orientation(&mut self) {
        let forward = self.forward();
        let orbit_center = vec3_add(self.pivot, self.pan_offset);
        let position = vec3_sub(orbit_center, vec3_scale(forward, self.distance.max(0.1)));
        self.position = [position[0], position[1], position[2], 1.0];
        self.target = [orbit_center[0], orbit_center[1], orbit_center[2], 1.0];
    }

    fn update_freeflight_target(&mut self) {
        let forward = self.forward();
        let target = vec3_add(self.position_xyz(), vec3_scale(forward, self.distance.max(0.1)));
        self.target = [target[0], target[1], target[2], 1.0];
    }

    fn forward(&self) -> [f32; 3] {
        let pitch = self.pitch;
        let yaw = self.yaw;
        [
            pitch.cos() * yaw.sin(),
            pitch.sin(),
            pitch.cos() * yaw.cos(),
        ]
    }

    fn world_up(&self) -> [f32; 3] {
        [self.up[0], self.up[1], self.up[2]]
    }

    fn position_xyz(&self) -> [f32; 3] {
        [self.position[0], self.position[1], self.position[2]]
    }

    fn target_xyz(&self) -> [f32; 3] {
        [self.target[0], self.target[1], self.target[2]]
    }
}

fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec3_scale(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = vec3_len(v);
    if len <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        vec3_scale(v, 1.0 / len)
    }
}

fn clamp(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}

