use std::cell::RefCell;
use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram,
    WebGlShader, WebGlVertexArrayObject,
};

mod camera;

const VERTEX_SHADER: &str = include_str!("shaders/triangle.vert.glsl");
const FRAGMENT_SHADER: &str = include_str!("shaders/triangle.frag.glsl");

#[derive(Clone, Copy)]
struct InstanceRecord {
    position: [f32; 3],
    rotation: [f32; 4],
    mesh_index: u32,
}

struct MeshBatch {
    _vao: WebGlVertexArrayObject,
    _vertex_buffer: WebGlBuffer,
    _instance_buffer: WebGlBuffer,
    vertex_count: i32,
    instance_count: Cell<i32>,
}

impl MeshBatch {
    fn new(
        gl: &GL,
        program: &WebGlProgram,
        vertices: &[f32],
        instance_data: &[f32],
    ) -> Result<Self, JsValue> {
        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create vertex array"))?;
        gl.bind_vertex_array(Some(&vao));

        let vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create mesh vertex buffer"))?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));

        let vertex_array = js_sys::Float32Array::from(vertices);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vertex_array, GL::STATIC_DRAW);

        let position = gl.get_attrib_location(program, "a_position") as u32;
        gl.enable_vertex_attrib_array(position);
        gl.vertex_attrib_pointer_with_i32(position, 3, GL::FLOAT, false, 24, 0);

        let normal = gl.get_attrib_location(program, "a_normal") as u32;
        gl.enable_vertex_attrib_array(normal);
        gl.vertex_attrib_pointer_with_i32(normal, 3, GL::FLOAT, false, 24, 12);

        let instance_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create instance buffer"))?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&instance_buffer));

        let instance_array = js_sys::Float32Array::from(instance_data);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &instance_array, GL::DYNAMIC_DRAW);

        let instance_position = gl.get_attrib_location(program, "a_instance_position") as u32;
        gl.enable_vertex_attrib_array(instance_position);
        gl.vertex_attrib_pointer_with_i32(instance_position, 3, GL::FLOAT, false, 32, 0);
        gl.vertex_attrib_divisor(instance_position, 1);

        let instance_rotation = gl.get_attrib_location(program, "a_instance_rotation") as u32;
        gl.enable_vertex_attrib_array(instance_rotation);
        gl.vertex_attrib_pointer_with_i32(instance_rotation, 4, GL::FLOAT, false, 32, 12);
        gl.vertex_attrib_divisor(instance_rotation, 1);

        let instance_mesh_index = gl.get_attrib_location(program, "a_instance_mesh_index") as u32;
        gl.enable_vertex_attrib_array(instance_mesh_index);
        gl.vertex_attrib_pointer_with_i32(instance_mesh_index, 1, GL::FLOAT, false, 32, 28);
        gl.vertex_attrib_divisor(instance_mesh_index, 1);

        Ok(Self {
            _vao: vao,
            _vertex_buffer: vertex_buffer,
            _instance_buffer: instance_buffer,
            vertex_count: (vertices.len() / 6) as i32,
            instance_count: Cell::new((instance_data.len() / 8) as i32),
        })
    }

    fn upload_instances(&self, gl: &GL, instance_data: &[f32]) {
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self._instance_buffer));

        let instance_array = js_sys::Float32Array::from(instance_data);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &instance_array, GL::DYNAMIC_DRAW);
        self.instance_count.set((instance_data.len() / 8) as i32);
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();

    let gl = canvas
        .get_context("webgl2")?
        .unwrap()
        .dyn_into::<GL>()
        .unwrap();

    let vert_shader = compile_shader(&gl, GL::VERTEX_SHADER, VERTEX_SHADER).unwrap();
    let frag_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, FRAGMENT_SHADER).unwrap();

    let program = link_program(&gl, &vert_shader, &frag_shader).unwrap();
    gl.use_program(Some(&program));
    let camera = camera::CameraUniform::new(&gl, &program).unwrap();

    fn push_instance_record(
        transforms: &mut Vec<InstanceRecord>,
        position: [f32; 3],
        rotation: [f32; 4],
        mesh_index: u32,
    ) {
        transforms.push(InstanceRecord {
            position,
            rotation,
            mesh_index,
        });
    }

    fn build_instance_records(time: f32) -> Vec<InstanceRecord> {
        let mut transforms = Vec::with_capacity(5 * 5 * 5);
        let spacing = 1.2_f32;

        for z in -2_i32..=2_i32 {
            for y in -2_i32..=2_i32 {
                for x in -2_i32..=2_i32 {
                    let position = [
                        x as f32 * spacing,
                        y as f32 * spacing,
                        z as f32 * spacing,
                    ];
                    let yaw = ((x + z) as f32) * 0.35 + time * 0.0015;
                    let half_yaw = yaw * 0.5;
                    let rotation = [0.0, half_yaw.sin(), 0.0, half_yaw.cos()];
                    let mesh_index = (x + y + z).rem_euclid(2) as u32;

                    push_instance_record(&mut transforms, position, rotation, mesh_index);
                }
            }
        }

        transforms
    }

    fn pack_instances_for_mesh(instances: &[InstanceRecord], mesh_index: u32) -> Vec<f32> {
        let mut packed = Vec::with_capacity(instances.len() * 8);

        for instance in instances.iter().filter(|instance| instance.mesh_index == mesh_index) {
            packed.extend_from_slice(&[
                instance.position[0],
                instance.position[1],
                instance.position[2],
                instance.rotation[0],
                instance.rotation[1],
                instance.rotation[2],
                instance.rotation[3],
                instance.mesh_index as f32,
            ]);
        }

        packed
    }

    fn push_vertex(vertices: &mut Vec<f32>, position: [f32; 3], normal: [f32; 3]) {
        vertices.extend_from_slice(&[
            position[0],
            position[1],
            position[2],
            normal[0],
            normal[1],
            normal[2],
        ]);
    }

    fn push_face(
        vertices: &mut Vec<f32>,
        a: [f32; 3],
        b: [f32; 3],
        c: [f32; 3],
        d: [f32; 3],
        normal: [f32; 3],
    ) {
        push_vertex(vertices, a, normal);
        push_vertex(vertices, b, normal);
        push_vertex(vertices, c, normal);
        push_vertex(vertices, a, normal);
        push_vertex(vertices, c, normal);
        push_vertex(vertices, d, normal);
    }

    fn push_cube(vertices: &mut Vec<f32>) {
        let p000 = [-0.5, -0.5, -0.5];
        let p001 = [-0.5, -0.5, 0.5];
        let p010 = [-0.5, 0.5, -0.5];
        let p011 = [-0.5, 0.5, 0.5];
        let p100 = [0.5, -0.5, -0.5];
        let p101 = [0.5, -0.5, 0.5];
        let p110 = [0.5, 0.5, -0.5];
        let p111 = [0.5, 0.5, 0.5];

        push_face(vertices, p001, p101, p111, p011, [0.0, 0.0, 1.0]);
        push_face(vertices, p100, p110, p111, p101, [1.0, 0.0, 0.0]);
        push_face(vertices, p000, p001, p011, p010, [-1.0, 0.0, 0.0]);
        push_face(vertices, p010, p011, p111, p110, [0.0, 1.0, 0.0]);
        push_face(vertices, p000, p100, p101, p001, [0.0, -1.0, 0.0]);
        push_face(vertices, p000, p010, p110, p100, [0.0, 0.0, -1.0]);
    }

    fn subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn normalize(vector: [f32; 3]) -> [f32; 3] {
        let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
        [vector[0] / length, vector[1] / length, vector[2] / length]
    }

    fn push_triangle(vertices: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
        let normal = normalize(cross(subtract(b, a), subtract(c, a)));
        push_vertex(vertices, a, normal);
        push_vertex(vertices, b, normal);
        push_vertex(vertices, c, normal);
    }

    fn push_icosahedron(vertices: &mut Vec<f32>) {
        let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;
        let scale = 0.5 / (1.0 + phi * phi).sqrt();

        let points = [
            normalize([-1.0, phi, 0.0]),
            normalize([1.0, phi, 0.0]),
            normalize([-1.0, -phi, 0.0]),
            normalize([1.0, -phi, 0.0]),
            normalize([0.0, -1.0, phi]),
            normalize([0.0, 1.0, phi]),
            normalize([0.0, -1.0, -phi]),
            normalize([0.0, 1.0, -phi]),
            normalize([phi, 0.0, -1.0]),
            normalize([phi, 0.0, 1.0]),
            normalize([-phi, 0.0, -1.0]),
            normalize([-phi, 0.0, 1.0]),
        ];

        let faces: [[usize; 3]; 20] = [
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];

        for face in faces {
            let a = [points[face[0]][0] * scale, points[face[0]][1] * scale, points[face[0]][2] * scale];
            let b = [points[face[1]][0] * scale, points[face[1]][1] * scale, points[face[1]][2] * scale];
            let c = [points[face[2]][0] * scale, points[face[2]][1] * scale, points[face[2]][2] * scale];
            push_triangle(vertices, a, b, c);
        }
    }

    let mut cube_vertices: Vec<f32> = Vec::with_capacity(36 * 6);
    push_cube(&mut cube_vertices);

    let mut icosahedron_vertices: Vec<f32> = Vec::with_capacity(60 * 6);
    push_icosahedron(&mut icosahedron_vertices);

    let initial_instances = build_instance_records(0.0);
    let cube_instances = pack_instances_for_mesh(&initial_instances, 0);
    let icosahedron_instances = pack_instances_for_mesh(&initial_instances, 1);

    let cube_batch = MeshBatch::new(&gl, &program, &cube_vertices, &cube_instances).unwrap();
    let icosahedron_batch = MeshBatch::new(&gl, &program, &icosahedron_vertices, &icosahedron_instances).unwrap();

    gl.enable(GL::DEPTH_TEST);
    gl.depth_func(GL::LESS);
    gl.enable(GL::CULL_FACE);
    gl.cull_face(GL::BACK);

    let gl = Rc::new(gl);
    let canvas = Rc::new(canvas);
    let window = Rc::new(window);
    let camera = Rc::new(camera);

    let raf_handle: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let raf_handle_inner = Rc::clone(&raf_handle);
    let gl_inner = Rc::clone(&gl);
    let canvas_inner = Rc::clone(&canvas);
    let window_inner = Rc::clone(&window);
    let camera_inner = Rc::clone(&camera);

    *raf_handle_inner.borrow_mut() = Some(Closure::wrap(Box::new(move |time: f64| {
        let logical_width = window_inner
            .inner_width()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(canvas_inner.width() as f64)
            .max(1.0) as u32;
        let logical_height = window_inner
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(canvas_inner.height() as f64)
            .max(1.0) as u32;
        let dpr = window_inner.device_pixel_ratio().max(1.0);
        let width = (logical_width as f64 * dpr).round().max(1.0) as u32;
        let height = (logical_height as f64 * dpr).round().max(1.0) as u32;

        if canvas_inner.width() != width || canvas_inner.height() != height {
            canvas_inner.set_width(width);
            canvas_inner.set_height(height);
        }

        camera_inner.update_aspect(gl_inner.as_ref(), logical_width as f32 / logical_height as f32);

        let instances = build_instance_records(time as f32);
        let cube_instances = pack_instances_for_mesh(&instances, 0);
        let icosahedron_instances = pack_instances_for_mesh(&instances, 1);

        cube_batch.upload_instances(gl_inner.as_ref(), &cube_instances);
        icosahedron_batch.upload_instances(gl_inner.as_ref(), &icosahedron_instances);

        gl_inner.viewport(0, 0, width as i32, height as i32);
        gl_inner.clear_color(0.02, 0.02, 0.05, 1.0);
        gl_inner.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        gl_inner.bind_vertex_array(Some(&cube_batch._vao));
        gl_inner.draw_arrays_instanced(
            GL::TRIANGLES,
            0,
            cube_batch.vertex_count,
            cube_batch.instance_count.get(),
        );

        gl_inner.bind_vertex_array(Some(&icosahedron_batch._vao));
        gl_inner.draw_arrays_instanced(
            GL::TRIANGLES,
            0,
            icosahedron_batch.vertex_count,
            icosahedron_batch.instance_count.get(),
        );

        window_inner
            .request_animation_frame(
                raf_handle
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unchecked_ref(),
            )
            .unwrap();
    }) as Box<dyn FnMut(f64)>));

    window
        .request_animation_frame(
            raf_handle_inner
                .borrow()
                .as_ref()
                .unwrap()
                .as_ref()
                .unchecked_ref(),
        )
        .unwrap();

    Ok(())
}

fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("failed to create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "shader compilation failed".into()))
    }
}

fn link_program(gl: &GL, vert: &WebGlShader, frag: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("failed to create program")?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| "program link failed".into()))
    }
}
